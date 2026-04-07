# Создаёт файлы секретов для deploy/docker (Windows / PowerShell).
# Из корня репозитория:  pwsh -File scripts/init-secrets.ps1
# Значения совпадают с дефолтами README; для своих паролей отредактируйте переменные ниже.

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$secretsDir = Join-Path $repoRoot "deploy\docker\secrets"

New-Item -ItemType Directory -Force -Path $secretsDir | Out-Null

$clickhouse = "ClickHousePass123!"
$minio = "MinIOSecret456!"

Set-Content -Path (Join-Path $secretsDir "clickhouse_password.txt") -Value $clickhouse -NoNewline -Encoding utf8
Set-Content -Path (Join-Path $secretsDir "minio_secret_key.txt")    -Value $minio -NoNewline -Encoding utf8
Set-Content -Path (Join-Path $secretsDir "smtp_password.txt")        -Value "placeholder-smtp" -NoNewline -Encoding utf8
Set-Content -Path (Join-Path $secretsDir "slack_webhook_url.txt")    -Value "https://hooks.slack.com/services/placeholder" -NoNewline -Encoding utf8
Set-Content -Path (Join-Path $secretsDir "pagerduty_key.txt")       -Value "placeholder-pd" -NoNewline -Encoding utf8

Write-Host "Secrets written to $secretsDir"
Write-Host "Start stack: docker compose -f deploy/docker/docker-compose.yml up -d --build"
