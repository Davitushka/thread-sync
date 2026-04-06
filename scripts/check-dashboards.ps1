#Requires -Version 5.1
<#
.SYNOPSIS
  Проверка данных для дашбордов Grafana SIEM-Lite (Windows / Docker Desktop).

.DESCRIPTION
  - ClickHouse HTTP: строки в siem.events, siem.alerts, siem.threat_intel
  - Prometheus: /api/v1/query?query=up, список targets
  - Краткий отчёт: что будет пустым и рекомендации (рус.)

.PARAMETER ClickHouseUrl
  Например http://localhost:8123

.PARAMETER PrometheusUrl
  Например http://localhost:9090
#>
param(
    [string] $ClickHouseUrl = "http://localhost:8123",
    [string] $PrometheusUrl = "http://localhost:9090"
)

$ErrorActionPreference = "Stop"

function Write-Section($t) { Write-Host "`n=== $t ===" -ForegroundColor Cyan }

function Invoke-ChQuery([string] $sql) {
    $enc = [uri]::EscapeDataString($sql)
    $u = "$ClickHouseUrl/?query=$enc"
    try {
        return (Invoke-RestMethod -Uri $u -Method Get -TimeoutSec 15)
    } catch {
        return $null
    }
}

Write-Section "ClickHouse ($ClickHouseUrl)"
$alive = Invoke-ChQuery "SELECT 1"
if ($null -eq $alive) {
    Write-Host "Недоступен или нет ответа. Все SQL-дашборды будут пустыми / с ошибкой." -ForegroundColor Red
    Write-Host "Рекомендация: docker compose up, проверьте проброс порта 8123." -ForegroundColor Yellow
} else {
    Write-Host "Ответ OK."
    foreach ($pair in @(
        @("siem.events (24h)", "SELECT count() FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR"),
        @("siem.alerts (7d)", "SELECT count() FROM siem.alerts WHERE triggered_at >= now() - INTERVAL 7 DAY"),
        @("siem.threat_intel", "SELECT count() FROM siem.threat_intel")
    )) {
        $label = $pair[0]
        $q = $pair[1]
        $v = Invoke-ChQuery $q
        if ($null -eq $v) {
            Write-Host "  $label : ошибка запроса" -ForegroundColor Red
        } else {
            $n = [long]($v.ToString().Trim())
            $color = if ($n -gt 0) { "Green" } else { "Yellow" }
            Write-Host "  $label : $n" -ForegroundColor $color
            if ($n -eq 0 -and $label -match "events") {
                Write-Host "    → Пусто: выполните scripts/seed-data/bootstrap_clickhouse.sh или Fill All Data в SIEM Admin." -ForegroundColor DarkYellow
            }
            if ($n -eq 0 -and $label -match "threat_intel") {
                Write-Host "    → IoC пусты: INSERT в siem.threat_intel или сид (bootstrap)." -ForegroundColor DarkYellow
            }
        }
    }
}

Write-Section "Prometheus ($PrometheusUrl)"
try {
    $up = Invoke-RestMethod -Uri "$PrometheusUrl/api/v1/query?query=up" -TimeoutSec 15
    $series = 0
    if ($up.data -and $up.data.result) { $series = $up.data.result.Count }
    Write-Host "Запрос up: рядов = $series" -ForegroundColor $(if ($series -gt 0) { "Green" } else { "Red" })
    if ($series -eq 0) {
        Write-Host "Prometheus без рядов up — проверьте scrape_config и что контейнер siem-prometheus запущен." -ForegroundColor Yellow
    }
} catch {
    Write-Host "Prometheus недоступен: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host "Все PromQL-панели будут пустыми." -ForegroundColor Yellow
}

try {
    $tg = Invoke-RestMethod -Uri "$PrometheusUrl/api/v1/targets?state=active" -TimeoutSec 15
    $active = $tg.data.activeTargets
    if ($active) {
        $upN = ($active | Where-Object { $_.health -eq "up" }).Count
        $downN = ($active | Where-Object { $_.health -ne "up" }).Count
        Write-Host "Targets: UP=$upN DOWN=$downN"
        $active | Where-Object { $_.health -ne "up" } | Select-Object -First 8 | ForEach-Object {
            Write-Host ("  DOWN job={0} {1}" -f $_.labels.job, $_.scrapeUrl) -ForegroundColor DarkYellow
        }
    }
} catch {
    Write-Host "Не удалось прочитать /api/v1/targets" -ForegroundColor DarkYellow
}

Write-Section "Эвристика «пустые дашборды»"
Write-Host @"
- Обзор / Detection / Alert Management / SOC / Data quality: нужны строки в ClickHouse (см. выше).
- Операции / Infrastructure / cAdvisor / Correlator / CH Prometheus-дашборды: нужен Prometheus и UP-таргеты (clickhouse, vector, detection-engine, …).
- Grafana internal / Prometheus stats: job grafana и prometheus в scrape; после добавления job grafana перезапустите Prometheus.
- Query / Data analysis (SQL): нужен query_log в ClickHouse; иначе таблицы query_log пусты — включите лог в конфиге CH.
- Windows Docker Desktop: node-exporter отражает ВМ Docker, не хост — см. панель 99 на Infrastructure.
"@

Write-Host "`nГотово."
