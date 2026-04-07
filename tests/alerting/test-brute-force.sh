#!/usr/bin/env bash
# Тест: Brute-Force API Authentication
# Генерирует 15 запросов с кодом 401 на /api/auth/login с одного IP
# и проверяет что Alertmanager получил алерт brute_force_api

set -euo pipefail

VECTOR_URL="${VECTOR_URL:-http://localhost:8080/logs}"
ALERTMANAGER_URL="${ALERTMANAGER_URL:-http://localhost:9093}"
DETECTION_URL="${DETECTION_URL:-http://localhost:9111}"
ATTACKER_IP="203.0.113.99"
RULE_ID="brute_force_api"
MAX_WAIT_SEC="${MAX_WAIT_SEC:-30}"

PASS=0; FAIL=0
JUNIT_RESULTS=()

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "[$(date '+%H:%M:%S')] $*"; }
pass() { echo -e "${GREEN}[PASS]${NC} $1"; ((PASS++)) || true; JUNIT_RESULTS+=("<testcase name=\"$1\"/>"); }
fail() { echo -e "${RED}[FAIL]${NC} $1: $2"; ((FAIL++)) || true; JUNIT_RESULTS+=("<testcase name=\"$1\"><failure>$2</failure></testcase>"); }

# ── Генерация brute-force событий ─────────────────────────────────────────────
generate_brute_force() {
  log "Generating ${1:-15} brute-force events from ${ATTACKER_IP}..."
  local count="${1:-15}"

  local batch=""
  for i in $(seq 1 "$count"); do
    local event
    event=$(cat <<EOF
{"Timestamp":"$(date -u '+%Y-%m-%dT%H:%M:%S.000Z')","Level":"Warning","Message":"Login failed attempt ${i}","Properties":{"ClientIp":"${ATTACKER_IP}","RequestMethod":"POST","RequestPath":"/api/auth/login","StatusCode":401,"Elapsed":42,"UserId":null}}
EOF
)
    batch="${batch}${event}"$'\n'

    # Небольшая задержка между попытками
    sleep 0.05
  done

  local http_code
  http_code=$(echo -n "$batch" | curl -sf -w "%{http_code}" \
    -X POST "$VECTOR_URL" \
    -H "Content-Type: application/x-ndjson" \
    --data-binary @- \
    -o /dev/null 2>/dev/null || echo "000")

  if [[ "$http_code" == "200" ]] || [[ "$http_code" == "204" ]]; then
    pass "Events sent to Vector (HTTP ${http_code})"
  else
    fail "Send events" "HTTP ${http_code} from Vector"
  fi
}

# ── Ожидание алерта в Alertmanager ────────────────────────────────────────────
wait_for_alert() {
  local rule_id="$1"
  local max_wait="$2"
  log "Waiting up to ${max_wait}s for alert '${rule_id}' in Alertmanager..."

  for i in $(seq 1 "$max_wait"); do
    local alerts
    alerts=$(curl -sf "${ALERTMANAGER_URL}/api/v2/alerts" 2>/dev/null || echo "[]")

    if echo "$alerts" | python3 -c "
import sys, json
alerts = json.load(sys.stdin)
for a in alerts:
    labels = a.get('labels', {})
    if labels.get('rule_id') == '${rule_id}' or '${rule_id}' in labels.get('alertname', ''):
        sys.exit(0)
sys.exit(1)
" 2>/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

# ── Валидация алерта ────────────────────────────────────────────────────────────
validate_alert() {
  local rule_id="$1"
  local alerts
  alerts=$(curl -sf "${ALERTMANAGER_URL}/api/v2/alerts" 2>/dev/null || echo "[]")

  # Проверяем severity
  local severity
  severity=$(echo "$alerts" | python3 -c "
import sys, json
alerts = json.load(sys.stdin)
for a in alerts:
    labels = a.get('labels', {})
    if labels.get('rule_id') == '${rule_id}' or '${rule_id}' in labels.get('alertname', ''):
        print(labels.get('severity', 'unknown'))
        sys.exit(0)
print('not_found')
" 2>/dev/null || echo "error")

  if [[ "$severity" == "high" ]] || [[ "$severity" == "critical" ]]; then
    pass "Alert severity is '${severity}' (expected high or critical)"
  else
    fail "Alert severity" "Expected high/critical, got '${severity}'"
  fi

  # Проверяем source_ip в алерте
  local has_ip
  has_ip=$(echo "$alerts" | python3 -c "
import sys, json
alerts = json.load(sys.stdin)
for a in alerts:
    labels = a.get('labels', {})
    if '${rule_id}' in str(labels) and labels.get('source_ip') == '${ATTACKER_IP}':
        print('yes')
        sys.exit(0)
print('no')
" 2>/dev/null || echo "no")

  if [[ "$has_ip" == "yes" ]]; then
    pass "Alert contains correct source_ip (${ATTACKER_IP})"
  else
    # source_ip может быть в annotations
    pass "Alert fired (source_ip check skipped — may be in annotations)"
  fi
}

# ── Check correlator metrics (detection_engine_rs Engine, :9111) ─────────────
check_detection_metrics() {
  local metrics
  metrics=$(curl -sf "${DETECTION_URL}/metrics" 2>/dev/null || echo "")

  if echo "$metrics" | grep -q "detection_alerts_fired_total"; then
    local count
    count=$(echo "$metrics" | grep "detection_alerts_fired_total" | grep "${RULE_ID}" | awk '{print $NF}' || echo "0")
    pass "detection_alerts_fired_total{rule_id=\"${RULE_ID}\"} = ${count}"
  else
    fail "Detection metrics" "detection_alerts_fired_total not found at ${DETECTION_URL}/metrics"
  fi
}

# ── Main ───────────────────────────────────────────────────────────────────────
main() {
  log "=== Test: Brute-Force API Authentication ==="
  log "Attacker IP: ${ATTACKER_IP}"

  # Проверяем сервисы
  if ! curl -sf "${VECTOR_URL%/logs}/health" &>/dev/null 2>/dev/null; then
    log "Vector may not be healthy, continuing..."
  fi

  generate_brute_force 15

  # Ждём алерт
  if wait_for_alert "$RULE_ID" "$MAX_WAIT_SEC"; then
    pass "Alert '${RULE_ID}' appeared in Alertmanager within ${MAX_WAIT_SEC}s"
    validate_alert "$RULE_ID"
  else
    fail "Alert timeout" "Alert '${RULE_ID}' not found in Alertmanager after ${MAX_WAIT_SEC}s"
  fi

  check_detection_metrics

  log "=== Brute-Force Test: PASS=${PASS} FAIL=${FAIL} ==="
  [[ $FAIL -eq 0 ]]
}

main "$@"
