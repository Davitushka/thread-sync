#Requires -Version 5.1
# Удаляет дашборды в Grafana, UID которых НЕ в grafana/dashboards/*.json (дубликаты импорта).
# Провиженные дашборды с теми же UID подтянутся при следующем reload provisioning.

param(
    [string] $GrafanaUrl = "http://localhost:3000",
    [string] $User = "admin",
    [string] $Password = "",
    [string] $RepoRoot = ""
)

$ErrorActionPreference = "Stop"

if (-not $RepoRoot) {
    $RepoRoot = Split-Path $PSScriptRoot -Parent
    if (-not (Test-Path (Join-Path $RepoRoot "grafana\dashboards"))) {
        throw "Run from repo: grafana/dashboards not under $RepoRoot"
    }
}

$dashDir = Join-Path $RepoRoot "grafana\dashboards"
if (-not (Test-Path $dashDir)) {
    throw "Not found: $dashDir"
}

if (-not $Password) {
    try {
        $Password = (docker exec siem-grafana sh -lc "cat /run/secrets/clickhouse_password" 2>$null).Trim()
    } catch { }
}
if (-not $Password) {
    throw "Pass -Password или положите пароль Grafana admin (как в Docker secret clickhouse_password)."
}

$keep = @{}
Get-ChildItem -Path $dashDir -Filter "*.json" -File | ForEach-Object {
    try {
        $j = Get-Content $_.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($j.uid) { $keep[$j.uid] = $_.Name }
    } catch {
        Write-Warning "Skip $($_.Name): $($_.Exception.Message)"
    }
}

Write-Host ("Provisioned UIDs in repo: {0}" -f $keep.Count) -ForegroundColor Cyan

$auth = [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("${User}:${Password}"))
$headers = @{ Authorization = "Basic $auth" }

$all = Invoke-RestMethod -Uri "$GrafanaUrl/api/search?type=dash-db&limit=5000" -Headers $headers -Method Get
$removed = @()
foreach ($d in $all) {
    $uid = $d.uid
    if (-not $uid) { continue }
    if ($keep.ContainsKey($uid)) { continue }
    try {
        Invoke-RestMethod -Uri "$GrafanaUrl/api/dashboards/uid/$uid" -Headers $headers -Method Delete | Out-Null
        $removed += "${uid}: $($d.title)"
        Write-Host "Deleted: $uid  ($($d.title))" -ForegroundColor Yellow
    } catch {
        Write-Warning "Failed delete $uid : $($_.Exception.Message)"
    }
}

try {
    Invoke-RestMethod -Uri "$GrafanaUrl/api/admin/provisioning/dashboards/reload" -Headers $headers -Method Post | Out-Null
    Write-Host "Provisioning dashboards reloaded." -ForegroundColor Green
} catch {
    Write-Warning "Reload failed: $($_.Exception.Message)"
}

Write-Host ("Done. Removed: {0}" -f $removed.Count) -ForegroundColor Cyan
if ($removed.Count -gt 0) {
    $removed | ForEach-Object { Write-Host "  $_" }
}
