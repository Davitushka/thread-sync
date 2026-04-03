-- ClickHouse 24.x — инициализация схемы SIEM-Lite
-- Документация: https://clickhouse.com/docs/en/engines/table-engines/mergetree-family/

-- Создаём базу данных
CREATE DATABASE IF NOT EXISTS siem
    COMMENT 'SIEM-Lite event storage';

-- Read-only пользователь для Grafana (пароль задаётся через env CLICKHOUSE_GRAFANA_PASSWORD
-- или fallback 'GrafanaReadOnly!' для dev-окружения).
-- В prod: передавать через Docker secret или vault, не хардкодить.
CREATE USER IF NOT EXISTS grafana_ro
    IDENTIFIED WITH plaintext_password BY 'GrafanaReadOnly!'
    HOST IP '172.28.0.0/24', IP '127.0.0.1', IP '::1';

GRANT SELECT ON siem.* TO grafana_ro;

-- ══════════════════════════════════════════════════════════════════════════════
-- Основная таблица событий
-- ══════════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS siem.events
(
    -- Временна́я метка (партиция)
    timestamp         DateTime64(3, 'UTC')      COMMENT 'Event timestamp (RFC3339, ms precision)',
    event_id          UUID                       COMMENT 'Unique event ID (UUID v4)',

    -- Классификация
    source_type       LowCardinality(String)     COMMENT 'dotnet|postgresql|redis|nginx|kubernetes',
    event_type        LowCardinality(String)     COMMENT 'application|database|cache|network|auth|syslog|raw',
    severity          Enum8(
                          'debug'    = 0,
                          'info'     = 1,
                          'warning'  = 2,
                          'error'    = 3,
                          'critical' = 4
                      )                          COMMENT 'Event severity level',

    -- Основные поля
    message           String                     COMMENT 'Log message (PII masked)',
    host              LowCardinality(String)     COMMENT 'Source host/container',

    -- Сетевые поля
    source_ip         Nullable(IPv4)             COMMENT 'Source IP address (nullable)',
    user_id           Nullable(String)           COMMENT 'Authenticated user ID',
    action            Nullable(String)           COMMENT 'HTTP method / SQL command / Redis cmd',
    status_code       Nullable(UInt16)           COMMENT 'HTTP status or response code',
    url_path          Nullable(String)           COMMENT 'URL path (no query string)',
    http_method       Nullable(String)           COMMENT 'GET|POST|PUT|DELETE|PATCH',
    duration_ms       Nullable(Float32)          COMMENT 'Request duration in milliseconds',

    -- GeoIP обогащение
    geo_country_iso   Nullable(FixedString(2))   COMMENT 'ISO 3166-1 alpha-2 country code',
    geo_country_name  Nullable(String)           COMMENT 'Country name (English)',
    geo_city          Nullable(String)           COMMENT 'City name',
    geo_lat           Nullable(Float32)          COMMENT 'Latitude',
    geo_lon           Nullable(Float32)          COMMENT 'Longitude',
    geo_asn           Nullable(UInt32)           COMMENT 'Autonomous System Number',
    geo_org           Nullable(String)           COMMENT 'ASN organization name',

    -- Metadata (key-value для source-specific полей)
    metadata          Map(String, String)        COMMENT 'Additional event-specific fields',

    -- Системные поля
    agent_version     LowCardinality(String)     COMMENT 'siem-parser version',
    ingest_ts         DateTime64(3, 'UTC')       COMMENT 'Timestamp when event entered SIEM',

    -- Производные поля для быстрого поиска (materialized)
    severity_num      UInt8 MATERIALIZED toUInt8(severity),
    ingest_lag_ms     Float32 MATERIALIZED dateDiff('millisecond', timestamp, ingest_ts)
)
ENGINE = ReplacingMergeTree(ingest_ts)
-- Партиционирование по дню — баланс между количеством партиций и retention granularity
PARTITION BY toYYYYMMDD(timestamp)
-- Сортировочный ключ: source_type + timestamp — оптимален для запросов по источнику за период
ORDER BY (source_type, timestamp, event_id)
-- Первичный ключ (подмножество sort key) — используется для sparse index
PRIMARY KEY (source_type, timestamp)
-- TTL: хранить 365 дней, затем удалять
-- (tiered storage TO VOLUME требует storage policy в config.xml — для локального запуска упрощено)
TTL toDateTime(timestamp) + INTERVAL 365 DAY DELETE
SETTINGS
    index_granularity = 8192,
    -- Дедупликация через ReplacingMergeTree: 100ms окно для схлопывания дубликатов
    replicated_deduplication_window = 1000,
    -- Сжатие по умолчанию для всех колонок
    min_bytes_for_wide_part = 10485760,  -- 10MB — выше этого wide format
    min_rows_for_wide_part = 100000;

-- ══════════════════════════════════════════════════════════════════════════════
-- Skip Indexes для ускорения фильтрации по полям вне primary key
-- ══════════════════════════════════════════════════════════════════════════════
--
-- ЗАЧЕМ НУЖНЫ:
--   PRIMARY KEY (source_type, timestamp) — сортировочный ключ, эффективен для
--   запросов вида WHERE source_type=? AND timestamp BETWEEN ... .
--   Для фильтрации по source_ip, severity_num, url_path, status_code нужны
--   дополнительные skip-индексы, которые позволяют пропускать гранулы (8192 строк).
--
-- bloom_filter — вероятностный фильтр: нулевые false negative, низкий false positive.
-- minmax — хранит min/max значения: идеален для числовых диапазонов.
-- set(N) — хранит уникальные значения (до N): идеален для low-cardinality фильтров.

-- source_ip: топ-запросы — WHERE source_ip = '...', GROUP BY source_ip
ALTER TABLE siem.events
    ADD INDEX IF NOT EXISTS idx_source_ip source_ip TYPE bloom_filter(0.01) GRANULARITY 4;

-- severity_num: WHERE severity_num >= 3 (error/critical), фильтры безопасности
ALTER TABLE siem.events
    ADD INDEX IF NOT EXISTS idx_severity_num severity_num TYPE set(10) GRANULARITY 1;

-- status_code: WHERE status_code IN (401, 403, 500), HTTP distribution
ALTER TABLE siem.events
    ADD INDEX IF NOT EXISTS idx_status_code status_code TYPE set(100) GRANULARITY 2;

-- url_path: LIKE '%auth%', bloom_filter для substring matching (ClickHouse 24.x supports this)
ALTER TABLE siem.events
    ADD INDEX IF NOT EXISTS idx_url_path url_path TYPE bloom_filter(0.025) GRANULARITY 4;

-- geo_country_name: WHERE geo_country_name IS NOT NULL (GeoIP таблица)
ALTER TABLE siem.events
    ADD INDEX IF NOT EXISTS idx_geo_country geo_country_name TYPE bloom_filter(0.05) GRANULARITY 8;

-- Применить индексы к существующим данным (materialize)
-- Выполнить вручную после ALTER ADD INDEX на production:
-- ALTER TABLE siem.events MATERIALIZE INDEX idx_source_ip;
-- ALTER TABLE siem.events MATERIALIZE INDEX idx_severity_num;
-- ALTER TABLE siem.events MATERIALIZE INDEX idx_status_code;
-- ALTER TABLE siem.events MATERIALIZE INDEX idx_url_path;
-- ALTER TABLE siem.events MATERIALIZE INDEX idx_geo_country;

-- ══════════════════════════════════════════════════════════════════════════════
-- Tiered Storage политики (настраивается в clickhouse-config.xml)
-- ══════════════════════════════════════════════════════════════════════════════
-- Дополнительно: в config.xml добавить:
-- <storage_configuration>
--   <disks>
--     <hot>  <type>local</type><path>/var/lib/clickhouse/hot/</path></hot>
--     <warm> <type>local</type><path>/var/lib/clickhouse/warm/</path></warm>
--     <s3>
--       <type>s3</type>
--       <endpoint>http://minio:9000/siem-cold/</endpoint>
--       <access_key_id>ACCESS_KEY</access_key_id>
--       <secret_access_key>SECRET_KEY</secret_access_key>
--     </s3>
--   </disks>
--   <policies>
--     <siem_tiered>
--       <volumes>
--         <hot>  <disk>hot</disk>  <max_data_part_size_bytes>1073741824</max_data_part_size_bytes></hot>
--         <warm> <disk>warm</disk> <max_data_part_size_bytes>5368709120</max_data_part_size_bytes></warm>
--         <cold> <disk>s3</disk></cold>
--       </volumes>
--     </siem_tiered>
--   </policies>
-- </storage_configuration>

-- ══════════════════════════════════════════════════════════════════════════════
-- Материализованные представления для быстрых агрегатов
-- ══════════════════════════════════════════════════════════════════════════════

-- Агрегат: счётчики событий по severity и источнику (по минутам)
CREATE TABLE IF NOT EXISTS siem.events_per_minute_agg
(
    minute        DateTime                    COMMENT 'Truncated to minute',
    source_type   LowCardinality(String),
    severity      Enum8('debug'=0,'info'=1,'warning'=2,'error'=3,'critical'=4),
    event_count   AggregateFunction(count)
)
ENGINE = AggregatingMergeTree()
PARTITION BY toYYYYMM(minute)
ORDER BY (minute, source_type, severity)
TTL minute + INTERVAL 90 DAY DELETE;

CREATE MATERIALIZED VIEW IF NOT EXISTS siem.events_per_minute_mv
TO siem.events_per_minute_agg
AS
SELECT
    toStartOfMinute(timestamp)  AS minute,
    source_type,
    severity,
    countState()                AS event_count
FROM siem.events
GROUP BY minute, source_type, severity;

-- Агрегат: топ IP по количеству событий (по часам)
CREATE TABLE IF NOT EXISTS siem.top_ips_agg
(
    hour          DateTime,
    source_ip     IPv4,
    event_count   AggregateFunction(count),
    error_count   AggregateFunction(countIf, UInt8)
)
ENGINE = AggregatingMergeTree()
PARTITION BY toYYYYMM(hour)
ORDER BY (hour, source_ip)
TTL hour + INTERVAL 30 DAY DELETE;

CREATE MATERIALIZED VIEW IF NOT EXISTS siem.top_ips_mv
TO siem.top_ips_agg
AS
SELECT
    toStartOfHour(timestamp)        AS hour,
    source_ip,
    countState()                    AS event_count,
    countIfState(severity_num >= 3) AS error_count
FROM siem.events
WHERE source_ip IS NOT NULL
GROUP BY hour, source_ip;

-- ══════════════════════════════════════════════════════════════════════════════
-- Таблица алертов
-- ══════════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS siem.alerts
(
    alert_id        UUID                    DEFAULT generateUUIDv4(),
    triggered_at    DateTime64(3, 'UTC'),
    rule_id         String,
    rule_title      String,
    severity        Enum8('low'=1,'medium'=2,'high'=3,'critical'=4),
    description     String,
    source_ip       Nullable(IPv4),
    user_id         Nullable(String),
    event_ids       Array(UUID)             COMMENT 'Correlated event IDs',
    mitre_tags      Array(String),
    status          Enum8('new'=0,'acknowledged'=1,'resolved'=2,'false_positive'=3) DEFAULT 'new',
    acknowledged_by Nullable(String),
    acknowledged_at Nullable(DateTime64(3, 'UTC')),
    notes           String                  DEFAULT ''
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(triggered_at)
ORDER BY (triggered_at, severity, rule_id)
TTL toDateTime(triggered_at) + INTERVAL 365 DAY DELETE
SETTINGS index_granularity = 1024;

-- ══════════════════════════════════════════════════════════════════════════════
-- Вспомогательный запрос: проверка производительности
-- ══════════════════════════════════════════════════════════════════════════════

-- Запрос для Grafana: EPS за последние 24 часа
-- SELECT
--     toStartOfMinute(timestamp) AS t,
--     count() AS eps_per_min,
--     countIf(severity IN ('error','critical')) AS errors_per_min
-- FROM siem.events
-- WHERE timestamp >= now() - INTERVAL 24 HOUR
-- GROUP BY t
-- ORDER BY t
-- FORMAT JSONCompact;
