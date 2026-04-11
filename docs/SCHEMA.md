# Нормализованная схема событий

Указатель документации: [`README.md`](README.md). Архитектура пайплайна: [`ARCHITECTURE.md`](ARCHITECTURE.md).

## JSON Schema (ECS-совместимая)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://siem-lite/schemas/normalized-event/v1",
  "title": "NormalizedEvent",
  "description": "ECS-compatible normalized security event",
  "type": "object",
  "required": ["@timestamp", "event_id", "source_type", "event_type", "severity", "message", "host"],
  "properties": {
    "@timestamp": {
      "type": "string",
      "format": "date-time",
      "description": "RFC3339 timestamp события (UTC, ms precision)"
    },
    "event_id": {
      "type": "string",
      "format": "uuid",
      "description": "UUID v4 уникального события"
    },
    "source_type": {
      "type": "string",
      "enum": ["dotnet", "postgresql", "redis", "nginx", "kubernetes", "syslog", "unknown"],
      "description": "Тип источника лога"
    },
    "event_type": {
      "type": "string",
      "enum": ["application", "database", "cache", "network", "auth", "syslog", "raw", "generic"],
      "description": "Класс события"
    },
    "severity": {
      "type": "string",
      "enum": ["debug", "info", "warning", "error", "critical"],
      "description": "Уровень серьёзности (ECS-совместимый)"
    },
    "message": {
      "type": "string",
      "maxLength": 65536,
      "description": "Основное сообщение (PII замаскировано)"
    },
    "host": {
      "type": "string",
      "description": "Hostname или container name источника"
    },
    "source_ip": {
      "type": ["string", "null"],
      "format": "ipv4",
      "description": "IP-адрес источника запроса (после X-Forwarded-For парсинга)"
    },
    "user_id": {
      "type": ["string", "null"],
      "description": "ID пользователя из JWT claims или pg_user"
    },
    "action": {
      "type": ["string", "null"],
      "description": "HTTP метод, SQL команда, Redis операция"
    },
    "status_code": {
      "type": ["integer", "null"],
      "minimum": 100,
      "maximum": 599,
      "description": "HTTP статус или код ответа"
    },
    "url_path": {
      "type": ["string", "null"],
      "description": "URL path без query string (PII в параметрах удалён)"
    },
    "http_method": {
      "type": ["string", "null"],
      "enum": ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", null]
    },
    "duration_ms": {
      "type": ["number", "null"],
      "minimum": 0,
      "description": "Длительность операции в миллисекундах"
    },
    "geo": {
      "type": ["object", "null"],
      "properties": {
        "country_iso": { "type": "string", "pattern": "^[A-Z]{2}$" },
        "country_name": { "type": "string" },
        "city": { "type": ["string", "null"] },
        "latitude": { "type": ["number", "null"] },
        "longitude": { "type": ["number", "null"] },
        "asn": { "type": ["integer", "null"] },
        "org": { "type": ["string", "null"] }
      }
    },
    "metadata": {
      "type": "object",
      "description": "Source-specific дополнительные поля",
      "additionalProperties": true
    },
    "agent_version": {
      "type": "string",
      "description": "Версия siem-parser"
    },
    "ingest_ts": {
      "type": "string",
      "format": "date-time",
      "description": "Время поступления события в SIEM"
    }
  }
}
```

## Пример нормализации: до / после

### Сырой лог .NET (Serilog JSON)

```json
{
  "Timestamp": "2024-01-15T10:30:42.1234567Z",
  "Level": "Warning",
  "MessageTemplate": "HTTP {RequestMethod} {RequestPath} responded {StatusCode} in {Elapsed:0.0000} ms",
  "Message": "HTTP POST /api/auth/login responded 401 in 45.2300 ms",
  "Properties": {
    "RequestMethod": "POST",
    "RequestPath": "/api/auth/login",
    "StatusCode": 401,
    "Elapsed": 45.23,
    "ClientIp": "203.0.113.42",
    "UserId": null,
    "Email": "john.doe@example.com",
    "Password": "SuperSecret123!",
    "Authorization": "Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyMTIzIn0.signature",
    "TraceId": "abc123def456"
  }
}
```

### Нормализованное событие (после siem-parser)

```json
{
  "@timestamp": "2024-01-15T10:30:42.123Z",
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "source_type": "dotnet",
  "event_type": "application",
  "severity": "warning",
  "message": "HTTP POST /api/auth/login responded 401 in 45.2300 ms",
  "host": "api-server-01",
  "source_ip": "203.0.113.42",
  "user_id": null,
  "action": "POST",
  "status_code": 401,
  "url_path": "/api/auth/login",
  "http_method": "POST",
  "duration_ms": 45.23,
  "geo": {
    "country_iso": "NL",
    "country_name": "Netherlands",
    "city": "Amsterdam",
    "latitude": 52.3667,
    "longitude": 4.8945,
    "asn": 64512,
    "org": "AS64512 Example Hosting BV"
  },
  "metadata": {
    "TraceId": "abc123def456",
    "Email": "[REDACTED]",
    "Password": "[REDACTED]",
    "Authorization": "[REDACTED]"
  },
  "agent_version": "0.1.0",
  "ingest_ts": "2024-01-15T10:30:42.156Z"
}
```

### Что изменилось

| Поле | Сырое | Нормализованное | Комментарий |
|------|-------|-----------------|-------------|
| `Timestamp` → `@timestamp` | `"2024-01-15T10:30:42.1234567Z"` | `"2024-01-15T10:30:42.123Z"` | Усечение до ms, UTC |
| `Level` → `severity` | `"Warning"` | `"warning"` | lowercase, ECS enum |
| `Properties.ClientIp` → `source_ip` | `"203.0.113.42"` | `"203.0.113.42"` | Извлечено из Properties |
| `Properties.Email` | `"john.doe@example.com"` | `"[REDACTED]"` | **PII маскирование** |
| `Properties.Password` | `"SuperSecret123!"` | `"[REDACTED]"` | **Sensitive key** |
| `Properties.Authorization` | `"Bearer eyJ..."` | `"[REDACTED]"` | **Token маскирование** |
| `geo` | отсутствует | `{country_iso: "NL", ...}` | **GeoIP обогащение** |
| `metadata.threat_intel_*` | отсутствует | `match` / `ioc_type` | Если задан `SIEM__INTEL__REDIS_URL` и IP в SET `siem:intel:ipv4` |
| `event_id` | отсутствует | UUID v4 | **Добавлен для дедупликации** |
| `ingest_ts` | отсутствует | timestamp | **Для расчёта lag** |

## Правила маскирования PII

| Тип PII | Regex паттерн | Замена | Где применяется |
|---------|---------------|--------|-----------------|
| Email | `[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}` | `***@***.***` | message, metadata values |
| Телефон | `\+?[0-9]{1,3}[\s\-]?\(?[0-9]{3}\)?[\s\-]?[0-9]{3}[\s\-]?[0-9]{4,}` | `[PHONE]` | message |
| Bearer токен / JWT | `(?i)(bearer\s+\|token[=:\s]+)[A-Za-z0-9\-_\.]{20,}` | `[REDACTED_TOKEN]` | message, headers |
| Банковская карта | `\b(?:\d[ \-]?){13,19}\b` | `[CARD_REDACTED]` | message |
| Sensitive keys | password, secret, token, api_key, cvv, ssn | `[REDACTED]` | metadata object keys |
| URL query params | `?token=...`, `?password=...` | Query string удаляется | url_path |

## Threat intelligence: `siem.threat_intel`

Таблица IoC в ClickHouse: ручной `INSERT`, сиды и сервис **`intel-connector`** (MISP / HTTP JSON / файл — см. [`INTEL_CONNECTOR.md`](INTEL_CONNECTOR.md)). События **`siem.events`** сопоставляют в Grafana по `toString(source_ip)` с `ioc_type = 'ipv4'`.

| Колонка | Назначение |
|---------|------------|
| `ioc_type` | `ipv4`, `domain`, `sha256`, `ipv6` |
| `ioc_value` | Каноническое значение (IPv4 dotted, FQDN, hex SHA-256) |
| `feed` | Источник: `manual`, `misp`, `local_feed`, `http_feed`, … |
| `threat_label`, `tags`, `confidence` | Контекст для triage |
| `first_seen`, `last_seen` | Версия строки для `ReplacingMergeTree` |

**Обогащение в потоке (siem-parser):** при `SIEM__INTEL__REDIS_URL` и зеркале Redis из коннектора в **`metadata`** могут появиться `threat_intel_match` (bool) и `threat_intel_ioc_type` (строка, сейчас `ipv4`).

Дашборд Grafana: **SIEM-Lite — SOC Workbench** (`/d/siem-soc-workbench`).
