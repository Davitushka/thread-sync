#!/usr/bin/env bash
# Тест: SQL/NoSQL Injection Detection
# Отправляет события с SQL injection паттернами и проверяет срабатывание

set -euo pipefail

VECTOR_URL="${VECTOR_URL:-http://localhost:8080/logs}"
ALERTMANAGER_URL="${ALERTMANAGER_URL:-http://localhost:9093}"
DETECTION_URL="${DETECTION_URL:-http://localhost:9110}"
RULE_ID="sql_injection_attempt"
MAX_WAIT_SEC="${MAX_WAIT_SEC:-15}"

PASS=0; FAIL=0

GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
log()  { echo "[$(date '+%H:%M:%S')] $*"; }
pass() { echo -e "${GREEN}[PASS]${NC} $1"; ((PASS++)) || true; }
fail() { echo -e "${RED}[FAIL]${NC} $1: $2"; ((FAIL++)) || true; }

send_event() {
  local payload="$1"
  local http_code
  http_code=$(echo -n "$payload" | curl -sf -w "%{http_code}" \
    -X POST "$VECTOR_URL" \
    -H "Content-Type: application/x-ndjson" \
    --data-binary @- -o /dev/null 2>/dev/null || echo "000")
  [[ "$http_code" == "200" ]] || [[ "$http_code" == "204" ]]
}

wait_for_alert() {
  local rule="$1"
  local max="$2"
  for _ in $(seq 1 "$max"); do
    local alerts
    alerts=$(curl -sf "${ALERTMANAGER_URL}/api/v2/alerts" 2>/dev/null || echo "[]")
    if echo "$alerts" | python3 -c "
import sys, json
for a in json.load(sys.stdin):
    if '${rule}' in str(a.get('labels', {})):
        sys.exit(0)
sys.exit(1)" 2>/dev/null; then return 0; fi
    sleep 1
  done
  return 1
}

main() {
  log "=== Test: SQL Injection Detection ==="

  # Сценарий 1: UNION SELECT в message
  log "Testing UNION SELECT injection..."
  local event1
  event1=$(cat <<'EOF'
{"Timestamp":"2024-01-15T10:00:00Z","Level":"Error","Message":"SQL error: UNION SELECT username, password FROM users WHERE '1'='1","SourceType":"dotnet","Host":"api-01","Properties":{"ClientIp":"203.0.113.1","RequestPath":"/api/search","StatusCode":500}}
EOF
)
  if send_event "$event1"; then
    pass "UNION SELECT event sent"
  else
    fail "Send UNION SELECT event" "Vector unreachable"
  fi

  # Сценарий 2: DROP TABLE
  log "Testing DROP TABLE injection..."
  local event2
  event2=$(cat <<'EOF'
{"Timestamp":"2024-01-15T10:00:01Z","Level":"Error","Message":"Query execution failed: '; DROP TABLE users;--","SourceType":"postgresql","Host":"db-01","Properties":{"ClientIp":"203.0.113.2","StatusCode":500,"duration_ms":1200}}
EOF
)
  send_event "$event2" && pass "DROP TABLE event sent" || fail "Send DROP TABLE event" "Vector unreachable"

  # Сценарий 3: NoSQL $where
  log "Testing NoSQL \$where injection..."
  local event3
  event3=$(cat <<'EOF'
{"Timestamp":"2024-01-15T10:00:02Z","Level":"Warning","Message":"MongoDB query: {\"$where\": \"this.password.length > 0\"}","SourceType":"dotnet","Host":"api-02","Properties":{"ClientIp":"203.0.113.3","RequestPath":"/api/users","StatusCode":200}}
EOF
)
  send_event "$event3" && pass "NoSQL \$where event sent" || fail "Send NoSQL event" "Vector unreachable"

  # Сценарий 4: Hex encoding
  log "Testing hex encoding injection..."
  local event4
  event4=$(cat <<'EOF'
{"Timestamp":"2024-01-15T10:00:03Z","Level":"Error","Message":"Suspicious query with 0x41414141 hex encoded payload detected in parameter","SourceType":"dotnet","Host":"api-01","Properties":{"ClientIp":"203.0.113.4","RequestPath":"/api/exec","StatusCode":500}}
EOF
)
  send_event "$event4" && pass "Hex encoding event sent" || fail "Send hex event" "Vector unreachable"

  # Ждём алерт
  if wait_for_alert "$RULE_ID" "$MAX_WAIT_SEC"; then
    pass "Alert '${RULE_ID}' appeared in Alertmanager within ${MAX_WAIT_SEC}s"

    # Проверяем severity
    local severity
    severity=$(curl -sf "${ALERTMANAGER_URL}/api/v2/alerts" 2>/dev/null | python3 -c "
import sys, json
for a in json.load(sys.stdin):
    if '${RULE_ID}' in str(a.get('labels', {})):
        print(a['labels'].get('severity', 'unknown'))
        sys.exit(0)
print('not_found')" 2>/dev/null || echo "unknown")

    if [[ "$severity" == "high" ]] || [[ "$severity" == "critical" ]]; then
      pass "Alert severity is '${severity}'"
    else
      fail "Alert severity" "Expected high/critical, got '${severity}'"
    fi
  else
    fail "Alert timeout" "Alert '${RULE_ID}' not received after ${MAX_WAIT_SEC}s"
  fi

  # Проверяем Prometheus метрики
  local metrics
  metrics=$(curl -sf "${DETECTION_URL}/metrics" 2>/dev/null || echo "")
  if echo "$metrics" | grep -q "${RULE_ID}"; then
    pass "Detection metrics contain rule_id '${RULE_ID}'"
  else
    fail "Detection metrics" "Rule '${RULE_ID}' not in metrics (non-fatal if detection engine not running)"
  fi

  log "=== SQL Injection Test: PASS=${PASS} FAIL=${FAIL} ==="
  [[ $FAIL -eq 0 ]]
}

main "$@"
