# Создаёт файлы секретов для deploy/docker (Windows / PowerShell).
# Из корня репозитория:  pwsh -File scripts/init-secrets.ps1
# Значения совпадают с дефолтами README; для своих паролей отредактируйте переменные ниже.

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$secretsDir = Join-Path $repoRoot "deploy\docker\secrets"

New-Item -ItemType Directory -Force -Path $secretsDir | Out-Null

$clickhouse = "ClickHousePass123!"
$minio = "MinIOSecret456!"
$grafana = "GrafanaAdmin123!"

# UTF8Encoding($false) — без BOM; PowerShell 5.x Set-Content добавляет BOM,
# который ломает Docker Secrets (MinIO, ClickHouse и др. читают лишние байты).
$utf8NoBom = New-Object System.Text.UTF8Encoding $false

[System.IO.File]::WriteAllText((Join-Path $secretsDir "clickhouse_password.txt"), $clickhouse, $utf8NoBom)
[System.IO.File]::WriteAllText((Join-Path $secretsDir "minio_secret_key.txt"), $minio, $utf8NoBom)
[System.IO.File]::WriteAllText((Join-Path $secretsDir "grafana_admin_password.txt"), $grafana, $utf8NoBom)
[System.IO.File]::WriteAllText((Join-Path $secretsDir "smtp_password.txt"), "placeholder-smtp", $utf8NoBom)
[System.IO.File]::WriteAllText((Join-Path $secretsDir "slack_webhook_url.txt"), "https://hooks.slack.com/services/placeholder", $utf8NoBom)
[System.IO.File]::WriteAllText((Join-Path $secretsDir "pagerduty_key.txt"), "placeholder-pd", $utf8NoBom)

Write-Host "Secrets written to $secretsDir"
Write-Host "Start stack: docker compose -f deploy/docker/docker-compose.yml up -d --build"
