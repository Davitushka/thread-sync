#!/usr/bin/env bash
# Load Test Runner для SIEM-Lite
# Поддерживает k6 и vegeta
#
# Использование:
#   bash run-test.sh                    # сценарий 1k (default)
#   bash run-test.sh --scenario 10k     # 10 000 EPS
#   bash run-test.sh --scenario 50k     # 50 000 EPS
#   bash run-test.sh --tool vegeta      # использовать vegeta вместо k6
#   bash run-test.sh --quick            # быстрый smoke test (30s)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESULTS_DIR="${SCRIPT_DIR}/results"
SCENARIO="1k"
TOOL="k6"
QUICK=false
BASE_URL="${BASE_URL:-http://localhost:7000}"
VECTOR_URL="${VECTOR_URL:-http://localhost:8080/logs}"

# ── Цветной вывод ─────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log()    { echo -e "${BLUE}[$(date '+%H:%M:%S')]${NC} $*"; }
ok()     { echo -e "${GREEN}[OK]${NC} $*"; }
warn()   { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail()   { echo -e "${RED}[FAIL]${NC} $*"; }

# ── Аргументы ─────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case $1 in
    --scenario) SCENARIO="$2"; shift 2;;
    --tool)     TOOL="$2"; shift 2;;
    --quick)    QUICK=true; shift;;
    --url)      BASE_URL="$2"; shift 2;;
    *)          echo "Unknown option: $1"; exit 1;;
  esac
done

mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date '+%Y%m%d_%H%M%S')
RUN_DIR="${RESULTS_DIR}/${SCENARIO}_${TIMESTAMP}"
mkdir -p "$RUN_DIR"

# ── Проверка зависимостей ──────────────────────────────────────────────────────
check_deps() {
  if [[ "$TOOL" == "k6" ]]; then
    command -v k6 &>/dev/null || {
      fail "k6 not found. Install: https://k6.io/docs/getting-started/installation/"
      exit 1
    }
    ok "k6 $(k6 version | head -1)"
  elif [[ "$TOOL" == "vegeta" ]]; then
    command -v vegeta &>/dev/null || {
      fail "vegeta not found. Install: go install github.com/tsenart/vegeta@latest"
      exit 1
    }
    ok "vegeta $(vegeta -version 2>&1 | head -1)"
  fi
}

# ── Health check ───────────────────────────────────────────────────────────────
wait_for_service() {
  local url="$1"
  local name="$2"
  local max=30
  log "Waiting for $name at $url..."
  for i in $(seq 1 $max); do
    if curl -sf "$url" &>/dev/null; then
      ok "$name is ready"
      return 0
    fi
    sleep 1
    echo -n "."
  done
  fail "$name not ready after ${max}s"
  return 1
}

# ── k6 runner ─────────────────────────────────────────────────────────────────
run_k6() {
  local scenario="$1"
  log "Running k6 scenario: $scenario"

  local k6_args=(
    run
    --env "SCENARIO=${scenario}"
    --env "BASE_URL=${BASE_URL}"
    --out "json=${RUN_DIR}/raw.json"
    "${SCRIPT_DIR}/k6-script.js"
  )

  if [[ "$QUICK" == "true" ]]; then
    # Быстрый smoke test: 5 VUs, 30s
    k6_args+=(--vus 5 --duration 30s)
    log "Quick mode: 5 VUs, 30s"
  fi

  set +e
  k6 "${k6_args[@]}" 2>&1 | tee "${RUN_DIR}/k6.log"
  local exit_code=$?
  set -e

  # HTML отчёт (если k6-reporter установлен)
  if command -v k6-reporter &>/dev/null; then
    k6-reporter "${RUN_DIR}/raw.json" --output "${RUN_DIR}/report.html" 2>/dev/null || true
  fi

  return $exit_code
}

# ── vegeta runner ──────────────────────────────────────────────────────────────
run_vegeta() {
  local scenario="$1"

  # Маппинг сценария → rate (requests/sec)
  declare -A rates=( ["1k"]=1000 ["10k"]=10000 ["50k"]=50000 )
  declare -A durations=( ["1k"]="60s" ["10k"]="120s" ["50k"]="180s" )

  local rate="${rates[$scenario]:-1000}"
  local dur="${durations[$scenario]:-60s}"

  if [[ "$QUICK" == "true" ]]; then
    rate=100; dur="30s"
    log "Quick mode: rate=100, duration=30s"
  fi

  log "Running vegeta: rate=${rate} rps, duration=${dur}"

  vegeta attack \
    -targets="${SCRIPT_DIR}/vegeta-targets.txt" \
    -rate="${rate}" \
    -duration="${dur}" \
    -max-body=65536 \
    | tee "${RUN_DIR}/vegeta-results.bin" \
    | vegeta report \
    | tee "${RUN_DIR}/vegeta-report.txt"

  # Генерация plot (если возможно)
  cat "${RUN_DIR}/vegeta-results.bin" \
    | vegeta plot \
    > "${RUN_DIR}/vegeta-plot.html" 2>/dev/null || true

  # Проверяем p99 threshold
  local p99
  p99=$(cat "${RUN_DIR}/vegeta-results.bin" \
    | vegeta report -type json \
    | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['latencies']['99th']/1e6)" 2>/dev/null || echo "999")

  log "p99 latency: ${p99} ms"
  if (( $(echo "$p99 < 5" | python3 -c "import sys; print(int(eval(sys.stdin.read())))") )); then
    ok "p99 ${p99}ms < 5ms SLA ✓"
    return 0
  else
    fail "p99 ${p99}ms >= 5ms SLA ✗"
    return 1
  fi
}

# ── Prometheus metrics snapshot ────────────────────────────────────────────────
collect_prometheus_metrics() {
  local prom_url="${PROMETHEUS_URL:-http://localhost:9090}"
  log "Collecting Prometheus metrics snapshot..."

  local queries=(
    "rate(siem_events_parsed_total[1m])"
    "histogram_quantile(0.99, rate(siem_parse_duration_seconds_bucket[1m]))"
    "rate(siem_parse_errors_total[1m])"
  )

  local metrics_file="${RUN_DIR}/prometheus-metrics.json"
  echo "{" > "$metrics_file"

  for query in "${queries[@]}"; do
    local result
    result=$(curl -sf "${prom_url}/api/v1/query?query=$(python3 -c "import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1]))" "$query")" 2>/dev/null || echo '{"status":"error"}')
    echo "\"$query\": $result," >> "$metrics_file"
  done
  echo "}" >> "$metrics_file"
  ok "Prometheus metrics saved to ${metrics_file}"
}

# ── Main ───────────────────────────────────────────────────────────────────────
main() {
  log "=== SIEM-Lite Load Test ==="
  log "Scenario: ${SCENARIO}, Tool: ${TOOL}, URL: ${BASE_URL}"
  log "Results: ${RUN_DIR}"

  check_deps
  wait_for_service "${BASE_URL}/health" "siem-parser"

  local exit_code=0

  if [[ "$TOOL" == "k6" ]]; then
    run_k6 "$SCENARIO" || exit_code=$?
  elif [[ "$TOOL" == "vegeta" ]]; then
    run_vegeta "$SCENARIO" || exit_code=$?
  else
    fail "Unknown tool: $TOOL (use k6 or vegeta)"
    exit 1
  fi

  collect_prometheus_metrics || true

  log "=== Results saved to: ${RUN_DIR} ==="

  if [[ $exit_code -eq 0 ]]; then
    ok "Load test PASSED ✓"
  else
    fail "Load test FAILED ✗"
  fi

  return $exit_code
}

main "$@"
