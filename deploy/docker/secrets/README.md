# Secrets

Полная инструкция по запуску стека (compose, порты, MinIO, профиль `admin`): [корневой README.md](../../../README.md) → **«Полный запуск (Docker Compose)»**. Указатель документов: [docs/README.md](../../../docs/README.md).

Создайте следующие файлы в этой директории перед первым запуском.

Содержимое `clickhouse_password.txt` должно совпадать с переменной `CLICKHOUSE_PASSWORD`, если вы задаёте её через `deploy/docker/.env` (см. `../.env.example`).

```bash
# SMTP пароль для email алертов
echo -n "your-smtp-password" > smtp_password.txt

# Slack Incoming Webhook URL
echo -n "https://hooks.slack.com/services/YOUR/WEBHOOK/URL" > slack_webhook_url.txt

# PagerDuty Integration Key
echo -n "your-pagerduty-routing-key" > pagerduty_key.txt

# ClickHouse пароль (используется для admin и как пример для grafana)
echo -n "StrongPassword123!" > clickhouse_password.txt

# MinIO secret key
echo -n "MinIOSecretKey456!" > minio_secret_key.txt

# Grafana admin password
echo -n "GrafanaAdmin123!" > grafana_admin_password.txt
```

> **Production**: Используйте HashiCorp Vault, SOPS или Doppler вместо файлов на диске.
> Docker Secrets монтируются в /run/secrets/<name> и не попадают в env vars.
