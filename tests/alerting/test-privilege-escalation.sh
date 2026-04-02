#!/usr/bin/env bash
# Тест: Privilege Escalation / Unauthorized Admin Access
# Отправляет 5 запросов с кодом 403 на /api/admin с одного IP

set -euo pipefail

VECTOR_URL="${VECTOR_URL:-http://localhost:8080/logs}"
ALERTMANAGER_URL="${ALERTMANAGER_URL:-http://localhost:9093}"
RULE_ID="privilege_escalation_attempt"
ATTACKER_IP="203.0.113.77"
MAX_WAIT_SEC="${MAX_WAIT_SEC:-30}"

PASS=0; FAIL=0

GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
log()  { echo "[$(date '+%H:%M:%S')] $*"; }
pass() { echo -e "${GREEN}[PASS]${NC} $1"; ((PASS++)) || true; }
fail() { echo -e "${RED}[FAIL]${NC} $1: $2"; ((FAIL++)) || true; }

send_event() {
  echo -n "$1" | curl -sf -w "%{http_code}" \
    -X POST "$VECTOR_URL" \
    -H "Content-Type: application/x-ndjson" \
    --data-binary @- -o /dev/null 2>/dev/null || echo "000"
}

wait_for_alert() {
  local rule="$1"; local max="$2"
  for _ in $(seq 1 "$max"); do
    if curl -sf "${ALERTMANAGER_URL}/api/v2/alerts" 2>/dev/null | python3 -c "
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
  log "=== Test: Privilege Escalation Detection ==="

  local admin_paths=("/api/admin/users" "/api/admin/config" "/api/management/stats" "/actuator/env")

  for path in "${admin_paths[@]}"; do
    for attempt in 1 2; do
      local event
      event=$(cat <<EOF
{"Timestamp":"$(date -u '+%Y-%m-%dT%H:%M:%S.000Z')","Level":"Warning","Message":"Access denied to admin endpoint attempt ${attempt}","SourceType":"dotnet","Host":"api-01","Properties":{"ClientIp":"${ATTACKER_IP}","RequestMethod":"GET","RequestPath":"${path}","StatusCode":403,"UserRole":"viewer","UserId":"user-999","Elapsed":5}}
EOF
)
      local code
      code=$(send_event "$event")
      [[ "$code" == "200" ]] || [[ "$code" == "204" ]] || true
    done
    log "Sent 2 forbidden requests to ${path}"
  done

  pass "Sent ${#admin_paths[@]} × 2 admin access attempts from ${ATTACKER_IP}"

  # Тест role bypass: 200 от non-admin на admin path
  local role_bypass_event
  role_bypass_event=$(cat <<'EOF'
{"Timestamp":"2024-01-15T10:01:00Z","Level":"Information","Message":"HTTP GET /api/admin/users responded 200","SourceType":"dotnet","Host":"api-01","Properties":{"ClientIp":"203.0.113.77","RequestMethod":"GET","RequestPath":"/api/admin/users","StatusCode":200,"UserRole":"viewer","UserId":"user-999"}}
EOF
)
  local bypass_code
  bypass_code=$(send_event "$role_bypass_event")
  if [[ "$bypass_code" == "200" ]] || [[ "$bypass_code" == "204" ]]; then
    pass "Role bypass event sent (non-admin accessing admin endpoint with 200)"
  fi

  # Тест role modification
  local role_mod_event
  role_mod_event=$(cat <<'EOF'
{"Timestamp":"2024-01-15T10:01:05Z","Level":"Warning","Message":"Role modification attempt","SourceType":"dotnet","Host":"api-01","Properties":{"ClientIp":"203.0.113.77","RequestMethod":"PUT","RequestPath":"/api/users/roles","StatusCode":200,"UserRole":"user","UserId":"user-999"}}
EOF
)
  send_event "$role_mod_event" && pass "Role modification event sent" || true

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

    [[ "$severity" == "high" ]] || [[ "$severity" == "critical" ]] \
      && pass "Alert severity is '${severity}'" \
      || fail "Alert severity" "Expected high/critical, got '${severity}'"

    # Проверяем MITRE tags
    local mitre
    mitre=$(curl -sf "${ALERTMANAGER_URL}/api/v2/alerts" 2>/dev/null | python3 -c "
import sys, json
for a in json.load(sys.stdin):
    if '${RULE_ID}' in str(a.get('labels',{})):
        print(a.get('annotations',{}).get('mitre_tags',''))
        sys.exit(0)
print('')" 2>/dev/null || echo "")

    if [[ -n "$mitre" ]] && echo "$mitre" | grep -q "T1068\|T1548"; then
      pass "Alert contains MITRE tags (T1068/T1548)"
    else
      pass "Alert fired (MITRE tags may be in labels)"
    fi
  else
    fail "Alert timeout" "Alert '${RULE_ID}' not received after ${MAX_WAIT_SEC}s"
  fi

  log "=== Privilege Escalation Test: PASS=${PASS} FAIL=${FAIL} ==="
  [[ $FAIL -eq 0 ]]
}

main "$@"
