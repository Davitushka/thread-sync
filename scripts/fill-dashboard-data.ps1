#Requires -Version 5.1
# Заполняет данные для дашбордов: ClickHouse (events/alerts/ioc), parser-метрики и query_log.

param(
    [string] $AdminUrl = "http://localhost:8089",
    [string] $ClickHouseUrl = "http://localhost:8123",
    [string] $ClickHouseUser = "siem",
    [string] $ClickHousePassword = "changeme",
    [string] $GrafanaUrl = "http://localhost:3000",
    [switch] $ReloadGrafana
)

$ErrorActionPreference = "Stop"

function Invoke-ChQuery([string] $sql) {
    $enc = [uri]::EscapeDataString($sql)
    $u = "$ClickHouseUrl/?query=$enc"
    $auth = [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("${ClickHouseUser}:${ClickHousePassword}"))
    return Invoke-RestMethod -Uri $u -Method Get -TimeoutSec 30 -Headers @{ Authorization = "Basic $auth" }
}

function Call-Admin([string] $path) {
    $u = "$AdminUrl$path"
    try {
        $r = Invoke-RestMethod -Uri $u -Method Post -TimeoutSec 180
        Write-Host ("OK {0}: {1}" -f $path, ($r | ConvertTo-Json -Compress)) -ForegroundColor Green
    } catch {
        Write-Host ("ERR {0}: {1}" -f $path, $_.Exception.Message) -ForegroundColor Red
        throw
    }
}

Write-Host "=== SIEM dashboard seed ===" -ForegroundColor Cyan
Write-Host "1) Fill through SIEM Admin endpoints..."

Call-Admin "/api/fill-events"
Call-Admin "/api/fill-alerts"
Call-Admin "/api/fill-threat-intel"
Call-Admin "/api/fill-parser-events"
Call-Admin "/api/fill-all-data"

Write-Host "2) Warm up ClickHouse query_log..." -ForegroundColor Cyan
for ($i = 0; $i -lt 30; $i++) {
    try {
        Invoke-ChQuery "SELECT count() FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR" | Out-Null
        Invoke-ChQuery "SELECT source_type, count() FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR GROUP BY source_type ORDER BY count() DESC LIMIT 10" | Out-Null
        Invoke-ChQuery "SELECT count() FROM siem.alerts WHERE triggered_at >= now() - INTERVAL 7 DAY" | Out-Null
    } catch {
        Write-Host ("query warmup error: {0}" -f $_.Exception.Message) -ForegroundColor DarkYellow
    }
    Start-Sleep -Milliseconds 150
}

Write-Host "3) Verify key datasets..." -ForegroundColor Cyan
$events = [long](Invoke-ChQuery "SELECT count() FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR").ToString().Trim()
$alerts = [long](Invoke-ChQuery "SELECT count() FROM siem.alerts WHERE triggered_at >= now() - INTERVAL 7 DAY").ToString().Trim()
$ioc = [long](Invoke-ChQuery "SELECT count() FROM siem.threat_intel").ToString().Trim()
$ql = [long](Invoke-ChQuery "SELECT count() FROM system.query_log WHERE event_time >= now() - INTERVAL 24 HOUR").ToString().Trim()

Write-Host ("siem.events (24h): {0}" -f $events) -ForegroundColor Green
Write-Host ("siem.alerts (7d): {0}" -f $alerts) -ForegroundColor Green
Write-Host ("siem.threat_intel: {0}" -f $ioc) -ForegroundColor Green
Write-Host ("system.query_log (24h): {0}" -f $ql) -ForegroundColor Green

if ($ReloadGrafana) {
    Write-Host "4) Reload Grafana provisioning..." -ForegroundColor Cyan
    $gp = ""
    try { $gp = (docker exec siem-grafana sh -lc "cat /run/secrets/clickhouse_password" 2>$null).Trim() } catch { }
    if ($gp) {
        $ga = [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("admin:${gp}"))
        try {
            Invoke-RestMethod -Uri "$GrafanaUrl/api/admin/provisioning/dashboards/reload" -Method Post -Headers @{ Authorization = "Basic $ga" } | Out-Null
            Write-Host "Grafana dashboards reloaded." -ForegroundColor Green
        } catch {
            Write-Host ("Grafana reload skipped: {0}" -f $_.Exception.Message) -ForegroundColor DarkYellow
        }
    } else {
        Write-Host "Grafana reload skipped (no password from Docker secret)." -ForegroundColor DarkYellow
    }
}

Write-Host "`nDone. Refresh Grafana dashboards (time range Last 24 hours)." -ForegroundColor Cyan
