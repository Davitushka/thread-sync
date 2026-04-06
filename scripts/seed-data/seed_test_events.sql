-- ============================================================
-- SIEM-Lite: тестовые данные для проверки дашбордов
-- Запуск: docker exec -i siem-clickhouse clickhouse-client < scripts/seed-data/seed_test_events.sql
-- ============================================================

-- 1. Базовые события (1000 шт., 24 часа, правильное распределение severity)
-- 60% info, 20% warning, 15% error, 5% critical
-- Source types: dotnet(50%), nginx(20%), postgresql(15%), redis(15%)
-- 10 уникальных IP, geo данные у 5 из них

INSERT INTO siem.events
  (timestamp, event_id, source_type, event_type, severity, message, host,
   source_ip, user_id, action, status_code, url_path, http_method, duration_ms,
   geo_country_iso, geo_country_name, geo_city, geo_lat, geo_lon,
   metadata, agent_version, ingest_ts)
SELECT
    -- timestamp: равномерно за последние 24 часа
    now() - INTERVAL toUInt32((rand() % 86400)) SECOND                                           AS timestamp,
    generateUUIDv4()                                                                              AS event_id,

    -- source_type: dotnet 50%, nginx 20%, postgresql 15%, redis 15%
    CAST(
        multiIf(
            (rowNumberInAllBlocks() % 100) < 50, 'dotnet',
            (rowNumberInAllBlocks() % 100) < 70, 'nginx',
            (rowNumberInAllBlocks() % 100) < 85, 'postgresql',
            'redis'
        ),
        'LowCardinality(String)'
    )                                                                                             AS source_type,

    CAST('application', 'LowCardinality(String)')                                                AS event_type,

    -- severity: 60% info, 20% warning, 15% error, 5% critical
    CAST(
        multiIf(
            (rowNumberInAllBlocks() % 100) < 60, 'info',
            (rowNumberInAllBlocks() % 100) < 80, 'warning',
            (rowNumberInAllBlocks() % 100) < 95, 'error',
            'critical'
        ),
        'Enum8(\'debug\'=0,\'info\'=1,\'warning\'=2,\'error\'=3,\'critical\'=4)'
    )                                                                                             AS severity,

    concat('Test event #', toString(rowNumberInAllBlocks()), ' from ', source_type)              AS message,
    CAST('siem-app-01', 'LowCardinality(String)')                                                AS host,

    -- 10 уникальных IP
    CAST(
        multiIf(
            (rowNumberInAllBlocks() % 10) = 0, toIPv4('192.168.1.10'),
            (rowNumberInAllBlocks() % 10) = 1, toIPv4('10.0.0.101'),
            (rowNumberInAllBlocks() % 10) = 2, toIPv4('172.16.0.55'),
            (rowNumberInAllBlocks() % 10) = 3, toIPv4('203.0.113.5'),
            (rowNumberInAllBlocks() % 10) = 4, toIPv4('198.51.100.7'),
            (rowNumberInAllBlocks() % 10) = 5, toIPv4('185.220.101.1'),
            (rowNumberInAllBlocks() % 10) = 6, toIPv4('91.108.4.200'),
            (rowNumberInAllBlocks() % 10) = 7, toIPv4('8.8.8.8'),
            (rowNumberInAllBlocks() % 10) = 8, toIPv4('1.1.1.1'),
            toIPv4('45.55.210.33')
        ),
        'Nullable(IPv4)'
    )                                                                                             AS source_ip,

    -- user_id у половины событий
    if((rowNumberInAllBlocks() % 2) = 0, concat('user-', toString(rowNumberInAllBlocks() % 5 + 1)), NULL) AS user_id,

    -- action
    CAST(
        multiIf(
            source_type = 'dotnet', arrayElement(['GET','POST','PUT','DELETE'], (rowNumberInAllBlocks() % 4) + 1),
            source_type = 'nginx',  arrayElement(['GET','POST'], (rowNumberInAllBlocks() % 2) + 1),
            source_type = 'postgresql', arrayElement(['SELECT','INSERT','UPDATE'], (rowNumberInAllBlocks() % 3) + 1),
            'GET'
        ),
        'Nullable(String)'
    )                                                                                             AS action,

    -- status_code: у dotnet/nginx
    if(source_type IN ('dotnet', 'nginx'),
        CAST(
            multiIf(
                (rowNumberInAllBlocks() % 20) < 14, 200,  -- 70% 200
                (rowNumberInAllBlocks() % 20) < 16, 201,  -- 10% 201
                (rowNumberInAllBlocks() % 20) < 18, 400,  -- 10% 400
                (rowNumberInAllBlocks() % 20) < 19, 404,  -- 5%  404
                500                                        -- 5%  500
            ),
            'Nullable(UInt16)'
        ),
        CAST(NULL, 'Nullable(UInt16)')
    )                                                                                             AS status_code,

    -- url_path: у dotnet/nginx
    if(source_type IN ('dotnet', 'nginx'),
        CAST(
            arrayElement(['/api/health','/api/v1/users','/api/v1/data','/metrics','/api/v1/reports'], (rowNumberInAllBlocks() % 5) + 1),
            'Nullable(String)'
        ),
        CAST(NULL, 'Nullable(String)')
    )                                                                                             AS url_path,

    if(source_type IN ('dotnet', 'nginx'), CAST('GET', 'Nullable(String)'), CAST(NULL, 'Nullable(String)')) AS http_method,
    if(source_type IN ('dotnet', 'nginx'),
        CAST(toFloat32(5.0 + rand() % 995), 'Nullable(Float32)'),
        CAST(NULL, 'Nullable(Float32)')
    )                                                                                             AS duration_ms,

    -- GeoIP у 5 IP (первые 5)
    if((rowNumberInAllBlocks() % 10) < 5,
        CAST(arrayElement(['US','DE','RU','CN','GB'], (rowNumberInAllBlocks() % 5) + 1), 'Nullable(FixedString(2))'),
        CAST(NULL, 'Nullable(FixedString(2))')
    )                                                                                             AS geo_country_iso,
    if((rowNumberInAllBlocks() % 10) < 5,
        CAST(arrayElement(['United States','Germany','Russia','China','United Kingdom'], (rowNumberInAllBlocks() % 5) + 1), 'Nullable(String)'),
        CAST(NULL, 'Nullable(String)')
    )                                                                                             AS geo_country_name,
    CAST(NULL, 'Nullable(String)')                                                               AS geo_city,
    CAST(NULL, 'Nullable(Float32)')                                                              AS geo_lat,
    CAST(NULL, 'Nullable(Float32)')                                                              AS geo_lon,
    CAST(NULL, 'Nullable(UInt32)')                                                               AS geo_asn,
    CAST(NULL, 'Nullable(String)')                                                               AS geo_org,
    CAST(map(), 'Map(String, String)')                                                           AS metadata,
    CAST('seed-v1', 'LowCardinality(String)')                                                    AS agent_version,
    now64(3)                                                                                     AS ingest_ts

FROM numbers(1000);

-- ============================================================
-- 2. 20 событий auth/login (для детектирования brute-force)
-- ============================================================
INSERT INTO siem.events
  (timestamp, event_id, source_type, event_type, severity, message, host,
   source_ip, user_id, action, status_code, url_path, http_method, duration_ms,
   geo_country_iso, geo_country_name, geo_city, geo_lat, geo_lon,
   metadata, agent_version, ingest_ts)
SELECT
    now() - INTERVAL toUInt32(rand() % 300) SECOND,
    generateUUIDv4(),
    CAST('dotnet', 'LowCardinality(String)'),
    CAST('auth', 'LowCardinality(String)'),
    CAST('warning', 'Enum8(\'debug\'=0,\'info\'=1,\'warning\'=2,\'error\'=3,\'critical\'=4)'),
    'Failed authentication attempt',
    CAST('siem-app-01', 'LowCardinality(String)'),
    CAST(toIPv4('185.220.101.1'), 'Nullable(IPv4)'),
    CAST('user-3', 'Nullable(String)'),
    CAST('POST', 'Nullable(String)'),
    CAST(if((rowNumberInAllBlocks() % 2) = 0, 401, 403), 'Nullable(UInt16)'),
    CAST(arrayElement(['/api/auth/login','/api/auth/token','/api/v1/auth/hubs'], rowNumberInAllBlocks() % 3 + 1), 'Nullable(String)'),
    CAST('POST', 'Nullable(String)'),
    CAST(toFloat32(12.5), 'Nullable(Float32)'),
    CAST(NULL, 'Nullable(FixedString(2))'),
    CAST(NULL, 'Nullable(String)'),
    CAST(NULL, 'Nullable(String)'),
    CAST(NULL, 'Nullable(Float32)'),
    CAST(NULL, 'Nullable(Float32)'),
    CAST(NULL, 'Nullable(UInt32)'),
    CAST(NULL, 'Nullable(String)'),
    CAST(map(), 'Map(String, String)'),
    CAST('seed-v1', 'LowCardinality(String)'),
    now64(3)
FROM numbers(20);

-- ============================================================
-- 3. 3 события SQL injection (для алерта SQLInjectionAttempt)
-- ============================================================
INSERT INTO siem.events
  (timestamp, event_id, source_type, event_type, severity, message, host,
   source_ip, user_id, action, status_code, url_path, http_method, duration_ms,
   geo_country_iso, geo_country_name, geo_city, geo_lat, geo_lon,
   metadata, agent_version, ingest_ts)
VALUES
    (now() - INTERVAL 60 SECOND, generateUUIDv4(), 'dotnet', 'security', 'critical',
     'Possible SQL injection detected in url_path', 'siem-app-01',
     toIPv4('91.108.4.200'), 'anonymous', 'GET', 400,
     '/api/v1/users?id=1%27%20UNION%20SELECT%20*%20FROM%20users--',
     'GET', toFloat32(2.1),
     NULL, NULL, NULL, NULL, NULL, NULL, NULL,
     map(), 'seed-v1', now64(3)),
    (now() - INTERVAL 30 SECOND, generateUUIDv4(), 'dotnet', 'security', 'critical',
     'SQL injection attempt via POST body', 'siem-app-01',
     toIPv4('91.108.4.200'), 'anonymous', 'POST', 500,
     '/api/v1/data;DROP TABLE users--',
     'POST', toFloat32(1.8),
     NULL, NULL, NULL, NULL, NULL, NULL, NULL,
     map(), 'seed-v1', now64(3)),
    (now() - INTERVAL 10 SECOND, generateUUIDv4(), 'dotnet', 'security', 'error',
     'XP_CMDSHELL execution attempt', 'siem-app-01',
     toIPv4('91.108.4.200'), 'anonymous', 'GET', 422,
     '/api/exec?cmd=xp_cmdshell%28%27whoami%27%29',
     'GET', toFloat32(3.5),
     NULL, NULL, NULL, NULL, NULL, NULL, NULL,
     map(), 'seed-v1', now64(3));

-- ============================================================
-- 4. 2 события privilege escalation (401 → 200 на admin)
-- ============================================================
INSERT INTO siem.events
  (timestamp, event_id, source_type, event_type, severity, message, host,
   source_ip, user_id, action, status_code, url_path, http_method, duration_ms,
   geo_country_iso, geo_country_name, geo_city, geo_lat, geo_lon,
   metadata, agent_version, ingest_ts)
VALUES
    -- Попытка доступа к admin → 401
    (now() - INTERVAL 90 SECOND, generateUUIDv4(), 'dotnet', 'auth', 'warning',
     'Unauthorized admin access attempt', 'siem-app-01',
     toIPv4('45.55.210.33'), 'user-2', 'GET', 401,
     '/api/admin/users/roles',
     'GET', toFloat32(5.0),
     NULL, NULL, NULL, NULL, NULL, NULL, NULL,
     map(), 'seed-v1', now64(3)),
    -- Успешный доступ после попытки → 200 (escalation)
    (now() - INTERVAL 20 SECOND, generateUUIDv4(), 'dotnet', 'auth', 'critical',
     'Privilege escalation: admin access granted after failed attempts', 'siem-app-01',
     toIPv4('45.55.210.33'), 'user-2', 'POST', 200,
     '/api/admin/manage/permissions',
     'POST', toFloat32(45.0),
     NULL, NULL, NULL, NULL, NULL, NULL, NULL,
     map(), 'seed-v1', now64(3));

-- ============================================================
-- 5. Тестовый алерт в siem.alerts
-- ============================================================
INSERT INTO siem.alerts
  (alert_id, fingerprint, triggered_at, rule_id, rule_title, severity, description,
   source_ip, user_id, event_ids, mitre_tags, status)
VALUES
    (generateUUIDv4(), 'seed-brute-1', now() - INTERVAL 5 MINUTE,
     'brute_force_api', 'Brute-force on API auth', 'critical',
     '18 failed auth attempts in 2 minutes from 185.220.101.1',
     toIPv4('185.220.101.1'), 'user-3',
     [],
     ['T1110', 'T1110.001'],
     'new'),
    (generateUUIDv4(), 'seed-sqli-1', now() - INTERVAL 2 MINUTE,
     'sql_injection', 'SQL injection attempt', 'critical',
     'SQL keywords in URL from 91.108.4.200: UNION SELECT, DROP TABLE, xp_cmdshell',
     toIPv4('91.108.4.200'), NULL,
     [],
     ['T1190'],
     'new'),
    (generateUUIDv4(), 'seed-priv-1', now() - INTERVAL 1 MINUTE,
     'privilege_escalation', 'Privilege escalation attempt', 'high',
     'IP 45.55.210.33 accessed /api/admin after 401 denial',
     toIPv4('45.55.210.33'), 'user-2',
     [],
     ['T1068', 'T1078.003'],
     'acknowledged');

-- ============================================================
-- 6. Threat intelligence (SOC Workbench: JOIN с siem.events)
--    feed='seed' — чтобы отличать демо-данные от продовых MISP/manual
-- ============================================================
INSERT INTO siem.threat_intel
  (ioc_type, ioc_value, feed, threat_label, tags, confidence, first_seen, last_seen)
VALUES
    ('ipv4', '198.51.100.7',  'seed', 'TEST-NET scanner',     ['demo','scanner'],        60, now64(3), now64(3)),
    ('ipv4', '185.220.101.1', 'seed', 'Brute-force scenario', ['demo','bruteforce'],     95, now64(3), now64(3)),
    ('ipv4', '91.108.4.200',  'seed', 'SQLi scenario',        ['demo','sqli'],           90, now64(3), now64(3)),
    ('ipv4', '45.55.210.33',  'seed', 'Priv-esc scenario',    ['demo','privesc'],        85, now64(3), now64(3)),
    ('ipv4', '203.0.113.5',   'seed', 'TEST-NET noise',      ['demo','test-net'],       40, now64(3), now64(3)),
    ('domain', 'evil-seed.example', 'seed', 'Phishing C2 (demo)', ['demo','domain'],     75, now64(3), now64(3)),
    ('sha256', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'seed', 'Dummy hash', ['demo'], 10, now64(3), now64(3));

-- ============================================================
-- 7. Всплеск export/download (дашборды / правила data exfil в Prometheus при живом ingest)
-- ============================================================
INSERT INTO siem.events
  (timestamp, event_id, source_type, event_type, severity, message, host,
   source_ip, user_id, action, status_code, url_path, http_method, duration_ms,
   geo_country_iso, geo_country_name, geo_city, geo_lat, geo_lon, geo_asn, geo_org,
   metadata, agent_version, ingest_ts)
SELECT
    now() - INTERVAL toUInt32(rand() % 120) SECOND,
    generateUUIDv4(),
    CAST('dotnet', 'LowCardinality(String)'),
    CAST('application', 'LowCardinality(String)'),
    CAST('warning', 'Enum8(\'debug\'=0,\'info\'=1,\'warning\'=2,\'error\'=3,\'critical\'=4)'),
    'Bulk export request',
    CAST('siem-app-01', 'LowCardinality(String)'),
    CAST(toIPv4('203.0.113.12'), 'Nullable(IPv4)'),
    CAST('user-export-test', 'Nullable(String)'),
    CAST('GET', 'Nullable(String)'),
    CAST(200, 'Nullable(UInt16)'),
    CAST(arrayElement([
        '/api/v1/reports/export',
        '/api/download/bulk',
        '/api/v1/data/csv'
    ], (rowNumberInAllBlocks() % 3) + 1), 'Nullable(String)'),
    CAST('GET', 'Nullable(String)'),
    CAST(toFloat32(50 + rand() % 200), 'Nullable(Float32)'),
    CAST(NULL, 'Nullable(FixedString(2))'),
    CAST(NULL, 'Nullable(String)'),
    CAST(NULL, 'Nullable(String)'),
    CAST(NULL, 'Nullable(Float32)'),
    CAST(NULL, 'Nullable(Float32)'),
    CAST(NULL, 'Nullable(UInt32)'),
    CAST(NULL, 'Nullable(String)'),
    CAST(map(), 'Map(String, String)'),
    CAST('seed-v1', 'LowCardinality(String)'),
    now64(3)
FROM numbers(40);

-- ============================================================
-- ПРОВЕРОЧНЫЕ ЗАПРОСЫ (запускать после вставки)
-- ============================================================

-- Проверка 1: Всего событий за 24 часа (ожидаем >= 1025)
SELECT 'Total events 24h' AS check, count() AS result
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR;

-- Проверка 2: Распределение severity
SELECT 'Severity distribution' AS check, toString(severity) AS severity, count() AS cnt
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR
GROUP BY severity ORDER BY cnt DESC;

-- Проверка 3: Распределение source_type
SELECT 'Source types' AS check, source_type, count() AS cnt
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR
GROUP BY source_type ORDER BY cnt DESC;

-- Проверка 4: Уникальные IP
SELECT 'Unique IPs' AS check, uniqExact(source_ip) AS ip_count
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR AND source_ip IS NOT NULL;

-- Проверка 5: Auth/login события (ожидаем 20+)
SELECT 'Auth events' AS check, count() AS cnt
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR
  AND (url_path LIKE '%auth%' OR url_path LIKE '%login%' OR url_path LIKE '%token%' OR url_path LIKE '%hubs%');

-- Проверка 6: Brute-force события 401/403 на auth (ожидаем 20)
SELECT 'Brute-force candidates' AS check, source_ip, count() AS cnt
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR
  AND status_code IN (401, 403)
  AND (url_path LIKE '%auth%' OR url_path LIKE '%login%' OR url_path LIKE '%token%')
GROUP BY source_ip;

-- Проверка 7: SQL injection паттерны (ожидаем 3)
SELECT 'SQL injection patterns' AS check, count() AS cnt
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR
  AND (url_path LIKE '%union%' OR url_path LIKE '%select%'
       OR url_path LIKE '%27%' OR url_path LIKE '%DROP%' OR url_path LIKE '%xp_%');

-- Проверка 8: Privilege escalation (401 → 200 на admin)
SELECT 'Privilege escalation events' AS check, source_ip, status_code, url_path, timestamp
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR
  AND url_path LIKE '%admin%'
ORDER BY source_ip, timestamp;

-- Проверка 9: GeoIP события (ожидаем ~500 строк с geo данными)
SELECT 'GeoIP events' AS check, geo_country_name, count() AS cnt
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR
  AND geo_country_name IS NOT NULL
GROUP BY geo_country_name;

-- Проверка 10: Алерты (ожидаем 3)
SELECT 'Alerts' AS check, rule_id, toString(severity) AS sev, toString(status) AS st
FROM siem.alerts WHERE triggered_at >= now() - INTERVAL 1 HOUR;

-- Проверка 11: Error Rate (должна быть ~20%)
SELECT 'Error rate' AS check,
  round(toFloat64(countIf(severity IN ('error','critical'))) / nullIf(count(), 0) * 100, 1) AS pct
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR;

-- Проверка 12: Ingestion lag (должен быть < 5 секунд для seed)
SELECT 'Avg ingest lag ms' AS check,
  round(avg(greatest(dateDiff('millisecond', timestamp, ingest_ts), 0)), 1) AS lag_ms
FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR;

-- Проверка 13: Threat intel (seed)
SELECT 'Threat intel (feed=seed)' AS check, count() AS cnt
FROM siem.threat_intel WHERE feed = 'seed';

-- Проверка 14: События, пересекающиеся с IoC IPv4
SELECT 'Events hitting IOC ipv4' AS check, count() AS cnt
FROM siem.events e
WHERE e.timestamp >= now() - INTERVAL 24 HOUR
  AND e.source_ip IS NOT NULL
  AND EXISTS (
    SELECT 1 FROM siem.threat_intel t
    WHERE t.ioc_type = 'ipv4' AND t.ioc_value = toString(e.source_ip) AND t.feed = 'seed'
  );
