#!/usr/bin/env bash
# Тест: Rate Limit Evasion Detection
# Отправляет 600 запросов с одного IP за 1 минуту

set -euo pipefail

VECTOR_URL="${VECTOR_URL:-http://localhost:8080/logs}"
ALERTMANAGER_URL="${ALERTMANAGER_URL:-http://localhost:9093}"
RULE_ID="rate_limit_evasion"
ATTACKER_IP="198.51.100.55"
MAX_WAIT_SEC="${MAX_WAIT_SEC:-70}"
BURST_COUNT="${BURST_COUNT:-600}"

PASS=0; FAIL=0

GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
log()  { echo "[$(date '+%H:%M:%S')] $*"; }
pass() { echo -e "${GREEN}[PASS]${NC} $1"; ((PASS++)) || true; }
fail() { echo -e "${RED}[FAIL]${NC} $1: $2"; ((FAIL++)) || true; }

generate_burst() {
  local count="$1"
  log "Generating ${count} requests burst from ${ATTACKER_IP}..."

  local batch_size=50
  local sent=0

  while [[ $sent -lt $count ]]; do
    local this_batch=$((count - sent < batch_size ? count - sent : batch_size))
    local batch=""

    for _ in $(seq 1 "$this_batch"); do
      local endpoints=("/api/products" "/api/users" "/api/search" "/api/orders")
      local ep="${endpoints[$((RANDOM % 4))]}"
      batch+=$(cat <<EOF
{"Timestamp":"$(date -u '+%Y-%m-%dT%H:%M:%S.000Z')","Level":"Information","Message":"HTTP GET ${ep} responded 200 in 12ms","SourceType":"dotnet","Host":"api-01","Properties":{"ClientIp":"${ATTACKER_IP}","RequestMethod":"GET","RequestPath":"${ep}","StatusCode":200,"Elapsed":12,"UserAgent":"python-requests/2.28.0"}}
EOF
)$'\n'
    done

    curl -sf -X POST "$VECTOR_URL" \
      -H "Content-Type: application/x-ndjson" \
      --data-binary "$batch" \
      -o /dev/null 2>/dev/null || true

    sent=$((sent + this_batch))
    echo -n "."
  done
  echo " ${count} events sent"
}

wait_for_alert() {
  local rule="$1"
  local max="$2"
  for _ in $(seq 1 "$max"); do
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
  log "=== Test: Rate Limit Evasion (${BURST_COUNT} requests from ${ATTACKER_IP}) ==="

  generate_burst "$BURST_COUNT"

  pass "Burst of ${BURST_COUNT} requests sent from ${ATTACKER_IP}"

  if wait_for_alert "$RULE_ID" "$MAX_WAIT_SEC"; then
    pass "Alert '${RULE_ID}' fired within ${MAX_WAIT_SEC}s"

    local severity
    severity=$(curl -sf "${ALERTMANAGER_URL}/api/v2/alerts" 2>/dev/null | python3 -c "
import sys, json
for a in json.load(sys.stdin):
    if '${RULE_ID}' in str(a.get('labels',{})):
        print(a['labels'].get('severity','unknown'))
        sys.exit(0)
print('not_found')" 2>/dev/null || echo "unknown")

    if [[ "$severity" == "medium" ]] || [[ "$severity" == "high" ]]; then
      pass "Alert severity is '${severity}' (expected medium or high)"
    else
      fail "Alert severity" "Expected medium/high, got '${severity}'"
    fi
  else
    fail "Alert timeout" "Alert '${RULE_ID}' not received after ${MAX_WAIT_SEC}s"
  fi

  log "=== Rate Limit Test: PASS=${PASS} FAIL=${FAIL} ==="
  [[ $FAIL -eq 0 ]]
}

main "$@"
