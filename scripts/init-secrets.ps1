# Создаёт файлы секретов для deploy/docker (Windows / PowerShell).
# Из корня репозитория:  pwsh -File scripts/init-secrets.ps1
# Значения совпадают с deploy/docker/.env.example (локальные плейсхолдеры changeme).
# Для своих паролей задайте CLICKHOUSE_PASSWORD / MINIO_SECRET_KEY / GRAFANA_ADMIN_PASSWORD в окружении.

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$secretsDir = Join-Path $repoRoot "deploy\docker\secrets"

New-Item -ItemType Directory -Force -Path $secretsDir | Out-Null

# Local dev placeholders — override via env or edit before non-local use.
$clickhouse = if ($env:CLICKHOUSE_PASSWORD) { $env:CLICKHOUSE_PASSWORD } else { "changeme" }
$minio = if ($env:MINIO_SECRET_KEY) { $env:MINIO_SECRET_KEY } else { "changeme" }
$grafana = if ($env:GRAFANA_ADMIN_PASSWORD) { $env:GRAFANA_ADMIN_PASSWORD } else { "changeme" }

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
