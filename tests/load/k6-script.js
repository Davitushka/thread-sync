/**
 * k6 Load Test для SIEM-Lite parser endpoint.
 *
 * Сценарии:
 *   SCENARIO=1k   — 1 000 EPS, 60s
 *   SCENARIO=10k  — 10 000 EPS, 120s
 *   SCENARIO=50k  — 50 000 EPS, 180s (требует k6 Cloud или много VUs)
 *
 * Запуск:
 *   k6 run --env SCENARIO=1k k6-script.js
 *   k6 run --env SCENARIO=10k --out json=results-10k.json k6-script.js
 *
 * Thresholds:
 *   - http_req_duration p(99) < 5ms  (SLA парсера)
 *   - http_req_failed < 1%
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';
import { randomItem, randomIntBetween } from 'https://jslib.k6.io/k6-utils/1.4.0/index.js';

// ── Кастомные метрики ─────────────────────────────────────────────────────────
const parseErrors     = new Counter('siem_parse_errors');
const throughputEPS   = new Counter('siem_events_total');
const parseDuration   = new Trend('siem_parse_duration_ms', true);
const batchSize       = new Trend('siem_batch_size');

// ── Конфигурация сценариев ────────────────────────────────────────────────────
const SCENARIOS = {
  '1k': {
    vus: 10,
    duration: '60s',
    targetEPS: 1000,
    batchSize: 10,
  },
  '10k': {
    vus: 50,
    duration: '120s',
    targetEPS: 10000,
    batchSize: 50,
  },
  '50k': {
    vus: 200,
    duration: '180s',
    targetEPS: 50000,
    batchSize: 100,
  },
};

const SCENARIO_NAME = __ENV.SCENARIO || '1k';
const SCENARIO      = SCENARIOS[SCENARIO_NAME] || SCENARIOS['1k'];
const BASE_URL      = __ENV.BASE_URL || 'http://localhost:7000';

export const options = {
  scenarios: {
    load_test: {
      executor: 'constant-vus',
      vus: SCENARIO.vus,
      duration: SCENARIO.duration,
    },
    // Дополнительный сценарий: spike test (2x EPS на 10s)
    spike: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '10s', target: SCENARIO.vus * 2 },
        { duration: '20s', target: SCENARIO.vus * 2 },
        { duration: '10s', target: 0 },
      ],
      startTime: `${parseInt(SCENARIO.duration) + 5}s`,
      gracefulRampDown: '10s',
    },
  },

  thresholds: {
    // SLA: p99 парсинга < 5ms
    'http_req_duration{scenario:load_test}': ['p(99)<5', 'p(95)<3'],
    // Ошибок < 1%
    'http_req_failed{scenario:load_test}': ['rate<0.01'],
    // Кастомные метрики
    'siem_parse_duration_ms': ['p(99)<5'],
  },

  summaryTrendStats: ['min', 'med', 'avg', 'p(90)', 'p(95)', 'p(99)', 'max', 'count'],
};

// ── Генераторы событий ────────────────────────────────────────────────────────

const LEVELS    = ['Verbose', 'Debug', 'Information', 'Warning', 'Error', 'Fatal'];
const METHODS   = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH'];
const ENDPOINTS = [
  '/api/auth/login', '/api/users', '/api/products',
  '/api/orders', '/api/search', '/hubs/notifications',
];
const STATUS_CODES = [200, 200, 200, 201, 301, 400, 401, 403, 404, 500];
const SOURCE_IPS   = [
  '192.168.1.10', '192.168.1.11', '10.0.0.1', '10.0.0.2',
  '203.0.113.1', '203.0.113.2', '198.51.100.1',
];

function generateDotnetEvent() {
  const level      = randomItem(LEVELS);
  const method     = randomItem(METHODS);
  const endpoint   = randomItem(ENDPOINTS);
  const statusCode = randomItem(STATUS_CODES);
  const duration   = randomIntBetween(1, 2000);

  return {
    Timestamp: new Date().toISOString(),
    Level: level,
    Message: `HTTP ${method} ${endpoint} responded ${statusCode} in ${duration}ms`,
    Properties: {
      ClientIp: randomItem(SOURCE_IPS),
      RequestMethod: method,
      RequestPath: endpoint,
      StatusCode: statusCode,
      Elapsed: duration,
      UserId: Math.random() > 0.3 ? `user-${randomIntBetween(1, 10000)}` : null,
      CorrelationId: `${Math.random().toString(36).substr(2, 9)}`,
    },
  };
}

function generatePostgresEvent() {
  const duration = randomIntBetween(1, 5000);
  const table    = randomItem(['users', 'orders', 'products', 'sessions']);
  return {
    Timestamp: new Date().toISOString(),
    Level: duration > 1000 ? 'Warning' : 'Information',
    Message: `duration: ${duration} ms  statement: SELECT * FROM ${table} WHERE id=$1`,
    SourceType: 'postgresql',
    Host: 'db-01',
    Properties: { duration_ms: duration, table },
  };
}

function generateBatch(size) {
  const events = [];
  for (let i = 0; i < size; i++) {
    events.push(Math.random() < 0.7 ? generateDotnetEvent() : generatePostgresEvent());
  }
  return events;
}

// ── Setup ─────────────────────────────────────────────────────────────────────

export function setup() {
  // Проверяем доступность сервиса
  const res = http.get(`${BASE_URL}/health`);
  if (res.status !== 200) {
    console.error(`siem-parser health check failed: ${res.status}`);
  }
  console.log(`Starting ${SCENARIO_NAME} scenario: ${SCENARIO.vus} VUs, target ${SCENARIO.targetEPS} EPS`);
  return { baseUrl: BASE_URL };
}

// ── Основной сценарий ─────────────────────────────────────────────────────────

export default function(data) {
  const batchSz = SCENARIO.batchSize;
  const batch   = generateBatch(batchSz);
  const payload = batch.map(e => JSON.stringify(e)).join('\n');

  const startTime = Date.now();
  const res = http.post(
    `${data.baseUrl}/api/v1/parse`,
    payload,
    {
      headers: {
        'Content-Type': 'application/x-ndjson',
        'X-Source-Type': 'dotnet',
        'X-Host': 'load-test-01',
      },
      timeout: '5s',
    }
  );

  const elapsed = Date.now() - startTime;

  // Счётчики
  throughputEPS.add(batchSz);
  parseDuration.add(elapsed);
  batchSize.add(batchSz);

  const ok = check(res, {
    'status is 200': (r) => r.status === 200,
    'response time < 5ms': (r) => r.timings.duration < 5,
    'response is JSON': (r) => r.headers['Content-Type'] && r.headers['Content-Type'].includes('json'),
  });

  if (!ok || res.status >= 400) {
    parseErrors.add(1);
    if (__ENV.VERBOSE) {
      console.warn(`Error response: ${res.status} - ${res.body?.substr(0, 200)}`);
    }
  }

  // Контролируем межзапросный интервал для точного EPS
  const targetIntervalMs = (batchSz / SCENARIO.targetEPS) * 1000;
  const remaining = targetIntervalMs - elapsed;
  if (remaining > 0) {
    sleep(remaining / 1000);
  }
}

// ── Teardown / Summary ────────────────────────────────────────────────────────

export function handleSummary(data) {
  const summary = {
    scenario: SCENARIO_NAME,
    timestamp: new Date().toISOString(),
    metrics: {
      p99_ms: data.metrics.http_req_duration?.values?.['p(99)'],
      p95_ms: data.metrics.http_req_duration?.values?.['p(95)'],
      avg_ms: data.metrics.http_req_duration?.values?.avg,
      error_rate: data.metrics.http_req_failed?.values?.rate,
      total_events: data.metrics.siem_events_total?.values?.count,
      throughput_eps: data.metrics.siem_events_total?.values?.count /
        (parseInt(SCENARIO.duration) || 60),
    },
    thresholds_passed: Object.entries(data.metrics).every(
      ([, m]) => !m.thresholds || Object.values(m.thresholds).every(t => t.ok)
    ),
  };

  return {
    'stdout': JSON.stringify(summary, null, 2),
    [`results-${SCENARIO_NAME}.json`]: JSON.stringify(data, null, 2),
    [`summary-${SCENARIO_NAME}.html`]: htmlReport(data),
  };
}

// Встроенный HTML отчёт (минимальный)
function htmlReport(data) {
  const p99 = data.metrics.http_req_duration?.values?.['p(99)']?.toFixed(3) || 'N/A';
  const errRate = ((data.metrics.http_req_failed?.values?.rate || 0) * 100).toFixed(2);
  const total = data.metrics.siem_events_total?.values?.count || 0;

  return `<!DOCTYPE html>
<html>
<head><title>SIEM-Lite Load Test: ${SCENARIO_NAME}</title>
<style>
  body{font-family:monospace;max-width:900px;margin:2rem auto;padding:0 1rem}
  h1{color:#333}.pass{color:green}.fail{color:red}
  table{width:100%;border-collapse:collapse}
  td,th{border:1px solid #ddd;padding:.5rem .8rem;text-align:left}
  th{background:#f0f0f0}
</style>
</head>
<body>
  <h1>SIEM-Lite Load Test Report — Scenario: ${SCENARIO_NAME}</h1>
  <p>Generated: ${new Date().toISOString()}</p>
  <h2>Key Metrics</h2>
  <table>
    <tr><th>Metric</th><th>Value</th><th>SLA</th><th>Status</th></tr>
    <tr>
      <td>p99 latency</td><td>${p99} ms</td><td>&lt; 5ms</td>
      <td class="${parseFloat(p99) < 5 ? 'pass' : 'fail'}">${parseFloat(p99) < 5 ? '✓ PASS' : '✗ FAIL'}</td>
    </tr>
    <tr>
      <td>Error rate</td><td>${errRate}%</td><td>&lt; 1%</td>
      <td class="${parseFloat(errRate) < 1 ? 'pass' : 'fail'}">${parseFloat(errRate) < 1 ? '✓ PASS' : '✗ FAIL'}</td>
    </tr>
    <tr>
      <td>Total events</td><td>${total.toLocaleString()}</td><td>—</td><td>—</td>
    </tr>
  </table>
</body>
</html>`;
}
