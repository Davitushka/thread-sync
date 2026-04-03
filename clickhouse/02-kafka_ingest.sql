-- Kafka → siem.events: потребитель Redpanda для сообщений от Vector (JSON).
-- Grafana читает merge-таблицу siem.events; без этого слоя пайплайн обрывался на очереди.

DROP TABLE IF EXISTS siem.events_kafka_mv;
DROP TABLE IF EXISTS siem.events_kafka_queue;

CREATE TABLE IF NOT EXISTS siem.events_kafka_queue
(
    data String
)
ENGINE = Kafka
SETTINGS
    kafka_broker_list = 'redpanda:9092',
    kafka_topic_list = 'siem.events',
    kafka_group_name = 'clickhouse_siem_events',
    kafka_format = 'RawBLOB',
    kafka_num_consumers = 1,
    kafka_max_block_size = 65536,
    kafka_skip_broken_messages = 1000;

CREATE MATERIALIZED VIEW IF NOT EXISTS siem.events_kafka_mv TO siem.events AS
SELECT
    coalesce(
        parseDateTime64BestEffortOrNull(JSONExtractString(data, '@timestamp'), 3),
        parseDateTime64BestEffortOrNull(JSONExtractString(data, 'timestamp'), 3),
        now64(3)
    ) AS timestamp,
    coalesce(toUUIDOrNull(JSONExtractString(data, 'event_id')), generateUUIDv4()) AS event_id,
    if(length(trimBoth(JSONExtractString(data, 'source_type'))) = 0, 'unknown', JSONExtractString(data, 'source_type')) AS source_type,
    if(length(trimBoth(JSONExtractString(data, 'event_type'))) = 0, 'generic', JSONExtractString(data, 'event_type')) AS event_type,
    CAST(
        multiIf(
            length(lower(JSONExtractString(data, 'severity'))) = 0,
            'info',
            lower(JSONExtractString(data, 'severity')) IN ('fatal', 'critical'),
            'critical',
            lower(JSONExtractString(data, 'severity')) IN ('warn', 'warning'),
            'warning',
            lower(JSONExtractString(data, 'severity')) IN ('err', 'error'),
            'error',
            lower(JSONExtractString(data, 'severity')) IN ('information', 'info'),
            'info',
            lower(JSONExtractString(data, 'severity')) = 'debug',
            'debug',
            'info'
        ),
        'Enum8(\'debug\' = 0, \'info\' = 1, \'warning\' = 2, \'error\' = 3, \'critical\' = 4)'
    ) AS severity,
    JSONExtractString(data, 'message') AS message,
    if(length(trimBoth(JSONExtractString(data, 'host'))) = 0, 'unknown', JSONExtractString(data, 'host')) AS host,
    toIPv4OrNull(JSONExtractString(data, 'source_ip')) AS source_ip,
    nullIf(JSONExtractString(data, 'user_id'), '') AS user_id,
    nullIf(JSONExtractString(data, 'action'), '') AS action,
    if(
        JSONHas(data, 'status_code'),
        toUInt16(least(JSONExtractUInt(data, 'status_code'), toUInt64(65535))),
        NULL
    ) AS status_code,
    nullIf(JSONExtractString(data, 'url_path'), '') AS url_path,
    nullIf(JSONExtractString(data, 'http_method'), '') AS http_method,
    if(JSONHas(data, 'duration_ms'), toFloat32(JSONExtractFloat(data, 'duration_ms')), NULL) AS duration_ms,
    CAST(NULL, 'Nullable(FixedString(2))') AS geo_country_iso,
    CAST(NULL, 'Nullable(String)') AS geo_country_name,
    CAST(NULL, 'Nullable(String)') AS geo_city,
    CAST(NULL, 'Nullable(Float32)') AS geo_lat,
    CAST(NULL, 'Nullable(Float32)') AS geo_lon,
    CAST(NULL, 'Nullable(UInt32)') AS geo_asn,
    CAST(NULL, 'Nullable(String)') AS geo_org,
    if(
        JSONHas(data, 'metadata'),
        map('json', assumeNotNull(JSONExtractRaw(data, 'metadata'))),
        CAST(map(), 'Map(String, String)')
    ) AS metadata,
    if(length(trimBoth(JSONExtractString(data, 'agent_version'))) = 0, 'vector', JSONExtractString(data, 'agent_version')) AS agent_version,
    coalesce(
        parseDateTime64BestEffortOrNull(JSONExtractString(data, 'ingest_ts'), 3),
        now64(3)
    ) AS ingest_ts
FROM siem.events_kafka_queue;
