#!/usr/bin/env bash
# Запуск всех alerting тестов с генерацией JUnit XML отчёта
#
# Использование:
#   bash run-all-tests.sh
#   bash run-all-tests.sh --junit-output results/junit.xml
#   bash run-all-tests.sh --skip rate_limit   # пропустить тест по имени

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JUNIT_OUTPUT="${JUNIT_OUTPUT:-${SCRIPT_DIR}/results/junit.xml}"
RESULTS_DIR="$(dirname "$JUNIT_OUTPUT")"
SKIP_TESTS=()

export VECTOR_URL="${VECTOR_URL:-http://localhost:8080/logs}"
export ALERTMANAGER_URL="${ALERTMANAGER_URL:-http://localhost:9093}"
export DETECTION_URL="${DETECTION_URL:-http://localhost:9110}"
export MAX_WAIT_SEC="${MAX_WAIT_SEC:-30}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[$(date '+%H:%M:%S')]${NC} $*"; }
ok()   { echo -e "${GREEN}[PASS]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; }
warn() { echo -e "${YELLOW}[SKIP]${NC} $*"; }

while [[ $# -gt 0 ]]; do
  case $1 in
    --junit-output) JUNIT_OUTPUT="$2"; shift 2;;
    --skip) SKIP_TESTS+=("$2"); shift 2;;
    *) echo "Unknown: $1"; exit 1;;
  esac
done

mkdir -p "$RESULTS_DIR"

# ── Проверка сервисов ──────────────────────────────────────────────────────────
check_services() {
  log "Checking required services..."
  local all_ok=true

  for svc_name in "Vector:${VECTOR_URL%/logs}/health" "Alertmanager:${ALERTMANAGER_URL}/-/healthy" "Detection:${DETECTION_URL}/health"; do
    local name="${svc_name%%:*}"
    local url="${svc_name#*:}"
    if curl -sf "$url" &>/dev/null 2>/dev/null; then
      ok "${name} is healthy (${url})"
    else
      warn "${name} not responding at ${url} — test may fail"
      all_ok=false
    fi
  done

  if [[ "$all_ok" == "false" ]]; then
    warn "Some services are not available. Tests will run but may fail."
    warn "Start the stack with: docker compose -f deploy/docker/docker-compose.yml up -d"
    sleep 2
  fi
}

# ── Определение тестов ────────────────────────────────────────────────────────
declare -A TESTS=(
  ["brute_force"]="${SCRIPT_DIR}/test-brute-force.sh"
  ["sql_injection"]="${SCRIPT_DIR}/test-sql-injection.sh"
  ["rate_limit"]="${SCRIPT_DIR}/test-rate-limit.sh"
  ["privilege_escalation"]="${SCRIPT_DIR}/test-privilege-escalation.sh"
)

# ── Запуск тестов ─────────────────────────────────────────────────────────────
run_tests() {
  local total=0
  local passed=0
  local failed=0
  local skipped=0
  declare -A test_results=()
  declare -A test_durations=()
  declare -A test_outputs=()

  for test_name in "brute_force" "sql_injection" "rate_limit" "privilege_escalation"; do
    local script="${TESTS[$test_name]}"

    # Проверяем skip
    local skip=false
    for s in "${SKIP_TESTS[@]:-}"; do
      [[ "$s" == "$test_name" ]] && skip=true
    done

    if [[ "$skip" == "true" ]]; then
      warn "Test: ${test_name} (SKIPPED)"
      ((skipped++)) || true
      ((total++)) || true
      test_results[$test_name]="skipped"
      continue
    fi

    log "Running test: ${test_name}..."
    ((total++)) || true
    local start_time
    start_time=$(date +%s)

    local output_file="${RESULTS_DIR}/${test_name}.log"
    local exit_code=0

    set +e
    bash "$script" 2>&1 | tee "$output_file"
    exit_code=${PIPESTATUS[0]}
    set -e

    local duration=$(( $(date +%s) - start_time ))
    test_durations[$test_name]=$duration

    if [[ $exit_code -eq 0 ]]; then
      ok "Test '${test_name}' PASSED in ${duration}s"
      test_results[$test_name]="passed"
      ((passed++)) || true
    else
      fail "Test '${test_name}' FAILED in ${duration}s (exit code: ${exit_code})"
      test_results[$test_name]="failed"
      ((failed++)) || true
    fi

    # Пауза между тестами чтобы алерты не перепутались
    sleep 5
  done

  # ── JUnit XML отчёт ────────────────────────────────────────────────────────
  local timestamp
  timestamp=$(date -u '+%Y-%m-%dT%H:%M:%SZ')

  cat > "$JUNIT_OUTPUT" <<XML
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="SIEM-Lite Alerting Tests" time="$(( $(date +%s) ))" tests="${total}" failures="${failed}" skipped="${skipped}">
  <testsuite name="alerting" tests="${total}" failures="${failed}" skipped="${skipped}" timestamp="${timestamp}">
XML

  for test_name in "brute_force" "sql_injection" "rate_limit" "privilege_escalation"; do
    local result="${test_results[$test_name]:-skipped}"
    local duration="${test_durations[$test_name]:-0}"

    if [[ "$result" == "passed" ]]; then
      echo "    <testcase name=\"${test_name}\" classname=\"alerting\" time=\"${duration}\"/>" >> "$JUNIT_OUTPUT"
    elif [[ "$result" == "failed" ]]; then
      echo "    <testcase name=\"${test_name}\" classname=\"alerting\" time=\"${duration}\">" >> "$JUNIT_OUTPUT"
      echo "      <failure message=\"Test failed\">See ${RESULTS_DIR}/${test_name}.log</failure>" >> "$JUNIT_OUTPUT"
      echo "    </testcase>" >> "$JUNIT_OUTPUT"
    else
      echo "    <testcase name=\"${test_name}\" classname=\"alerting\" time=\"0\">" >> "$JUNIT_OUTPUT"
      echo "      <skipped/>" >> "$JUNIT_OUTPUT"
      echo "    </testcase>" >> "$JUNIT_OUTPUT"
    fi
  done

  cat >> "$JUNIT_OUTPUT" <<XML
  </testsuite>
</testsuites>
XML

  log "=== RESULTS: ${passed}/${total} passed, ${failed} failed, ${skipped} skipped ==="
  log "JUnit report: ${JUNIT_OUTPUT}"

  if [[ $failed -gt 0 ]]; then
    fail "Some alerting tests FAILED"
    return 1
  else
    ok "All alerting tests PASSED ✓"
    return 0
  fi
}

main() {
  log "=== SIEM-Lite Alerting Test Suite ==="
  log "Vector: ${VECTOR_URL}"
  log "Alertmanager: ${ALERTMANAGER_URL}"
  log "Detection Engine: ${DETECTION_URL}"

  check_services
  run_tests
}

main "$@"
