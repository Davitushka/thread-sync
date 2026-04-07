#Requires -Version 5.1
# SIEM-Lite: check ClickHouse + Prometheus for Grafana (Windows). Output: RU comments via Write-Host only (ASCII-safe file).

param(
    [string] $ClickHouseUrl = "http://localhost:8123",
    [string] $ClickHouseUser = "siem",
    [string] $ClickHousePassword = "ClickHousePass123!",
    [string] $PrometheusUrl = "http://localhost:9090"
)

$ErrorActionPreference = "Stop"

function Write-Section($t) { Write-Host "`n=== $t ===" -ForegroundColor Cyan }

function Invoke-ChQuery([string] $sql) {
    $enc = [uri]::EscapeDataString($sql)
    $u = "$ClickHouseUrl/?query=$enc"
    $auth = [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("${ClickHouseUser}:${ClickHousePassword}"))
    try {
        return (Invoke-RestMethod -Uri $u -Method Get -TimeoutSec 15 -Headers @{ Authorization = "Basic $auth" })
    } catch {
        return $null
    }
}

Write-Section "ClickHouse ($ClickHouseUrl)"
$alive = Invoke-ChQuery "SELECT 1"
if ($null -eq $alive) {
    Write-Host "ClickHouse nedostupen. SQL-paneli budut pustymi." -ForegroundColor Red
    Write-Host "Sovet: docker compose up; port 8123." -ForegroundColor Yellow
} else {
    Write-Host "ClickHouse: OK."
    foreach ($pair in @(
        @("siem.events (24h)", "SELECT count() FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR"),
        @("siem.alerts (7d)", "SELECT count() FROM siem.alerts WHERE triggered_at >= now() - INTERVAL 7 DAY"),
        @("siem.threat_intel", "SELECT count() FROM siem.threat_intel")
    )) {
        $label = $pair[0]
        $q = $pair[1]
        $v = Invoke-ChQuery $q
        if ($null -eq $v) {
            Write-Host "  $label : error" -ForegroundColor Red
        } else {
            $n = [long]($v.ToString().Trim())
            $color = if ($n -gt 0) { "Green" } else { "Yellow" }
            Write-Host "  $label : $n" -ForegroundColor $color
            if ($n -eq 0 -and $label -match "events") {
                Write-Host "    -> Pusto: seed (bootstrap_clickhouse.sh / Fill All Data)." -ForegroundColor DarkYellow
            }
            if ($n -eq 0 -and $label -match "threat_intel") {
                Write-Host "    -> IoC pusto: INSERT ili seed." -ForegroundColor DarkYellow
            }
        }
    }
}

Write-Section "Prometheus ($PrometheusUrl)"
try {
    $up = Invoke-RestMethod -Uri "$PrometheusUrl/api/v1/query?query=up" -TimeoutSec 15
    $series = 0
    if ($up.data -and $up.data.result) { $series = $up.data.result.Count }
    Write-Host "up series: $series" -ForegroundColor $(if ($series -gt 0) { "Green" } else { "Red" })
} catch {
    Write-Host "Prometheus nedostupen: $($_.Exception.Message)" -ForegroundColor Red
}

try {
    $tg = Invoke-RestMethod -Uri "$PrometheusUrl/api/v1/targets?state=active" -TimeoutSec 15
    $active = $tg.data.activeTargets
    if ($active) {
        $upN = ($active | Where-Object { $_.health -eq "up" }).Count
        $downN = ($active | Where-Object { $_.health -ne "up" }).Count
        Write-Host "targets UP=$upN DOWN=$downN"
        $active | Where-Object { $_.health -ne "up" } | Select-Object -First 8 | ForEach-Object {
            Write-Host ("  DOWN job={0} {1}" -f $_.labels.job, $_.scrapeUrl) -ForegroundColor DarkYellow
        }
    }
} catch {
    Write-Host "targets API error" -ForegroundColor DarkYellow
}

Write-Section "Heuristika (pochiemu malo dannyh na dashboardah)"
Write-Host "- Overview / Detection / Alerts / SOC / Data quality: nuzhny stroki v ClickHouse."
Write-Host "- Operations / Infrastructure / cAdvisor / CH-Prom: nuzhen Prometheus i UP targets."
Write-Host "- Grafana internal / Prom stats: eto INFRA monitoringa, ne SIEM EPS; ploshkie linii - chasto norma."
Write-Host "- Query/Data Analysis (SQL): system.query_log mozhet byt pust bez logirovaniya."
Write-Host "- Docker Desktop: node-exporter = VM Docker, ne host OS."
Write-Host "`nGotovo."
