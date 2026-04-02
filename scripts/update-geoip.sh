#!/usr/bin/env bash
# Скачивание и обновление MaxMind GeoLite2 баз данных
#
# Использование:
#   bash scripts/update-geoip.sh
#
# Переменные окружения:
#   MAXMIND_LICENSE_KEY  — лицензионный ключ MaxMind (обязателен)
#   GEOIP_DIR            — куда сохранять .mmdb файлы (по умолчанию: /etc/geoip)
#   DOCKER_VOLUME        — имя Docker volume (если задан — копирует туда)
#
# Регистрация для получения ключа (бесплатно):
#   https://www.maxmind.com/en/geolite2/signup
#
# Рекомендуется запускать по cron раз в неделю:
#   0 3 * * 1 MAXMIND_LICENSE_KEY=your_key bash /opt/siem/scripts/update-geoip.sh

set -euo pipefail

# ── Конфигурация ──────────────────────────────────────────────────────────────

MAXMIND_LICENSE_KEY="${MAXMIND_LICENSE_KEY:-}"
GEOIP_DIR="${GEOIP_DIR:-/etc/geoip}"
DOCKER_VOLUME="${DOCKER_VOLUME:-}"
MAXMIND_ACCOUNT_ID="${MAXMIND_ACCOUNT_ID:-}"
TEMP_DIR=$(mktemp -d)
DOWNLOAD_BASE="https://download.maxmind.com/geoip/databases"

# Базы для скачивания
DATABASES=(
    "GeoLite2-City"
    "GeoLite2-ASN"
)

# ── Функции ───────────────────────────────────────────────────────────────────

log() { echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] $*"; }
error() { echo "[ERROR] $*" >&2; exit 1; }

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

check_dependencies() {
    local missing=()
    for cmd in curl tar gzip; do
        command -v "$cmd" &>/dev/null || missing+=("$cmd")
    done
    if [[ ${#missing[@]} -gt 0 ]]; then
        error "Missing dependencies: ${missing[*]}. Install with: apt-get install curl tar gzip"
    fi
}

validate_mmdb() {
    local file="$1"
    # Проверяем magic bytes MaxMind DB (\xab\xcd\xefMaxMind.com)
    if ! head -c 100 "$file" | grep -q "MaxMind"; then
        # Альтернативная проверка — размер файла (минимум 1MB для GeoLite2)
        local size
        size=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null || echo 0)
        if [[ "$size" -lt 1048576 ]]; then
            error "File $file looks invalid (size: ${size} bytes)"
        fi
    fi
    log "Validated: $file ($(du -sh "$file" | cut -f1))"
}

download_database() {
    local db_name="$1"
    local archive_name="${db_name}_$( date +%Y%m%d ).tar.gz"
    local download_url

    if [[ -n "$MAXMIND_ACCOUNT_ID" ]]; then
        # Новый формат URL с account ID
        download_url="${DOWNLOAD_BASE}/${db_name}/download?suffix=tar.gz"
        log "Downloading ${db_name} (account: ${MAXMIND_ACCOUNT_ID})..."
        curl --fail --silent --show-error \
            --user "${MAXMIND_ACCOUNT_ID}:${MAXMIND_LICENSE_KEY}" \
            --output "${TEMP_DIR}/${archive_name}" \
            "$download_url"
    else
        # Старый формат URL (legacy)
        download_url="https://download.maxmind.com/app/geoip_download?edition_id=${db_name}&license_key=${MAXMIND_LICENSE_KEY}&suffix=tar.gz"
        log "Downloading ${db_name}..."
        curl --fail --silent --show-error \
            --output "${TEMP_DIR}/${archive_name}" \
            "$download_url"
    fi

    # Распаковка
    log "Extracting ${archive_name}..."
    tar -xzf "${TEMP_DIR}/${archive_name}" -C "${TEMP_DIR}"

    # Находим .mmdb файл
    local mmdb_file
    mmdb_file=$(find "${TEMP_DIR}" -name "${db_name}.mmdb" -type f | head -1)
    if [[ -z "$mmdb_file" ]]; then
        error "Could not find ${db_name}.mmdb in archive"
    fi

    validate_mmdb "$mmdb_file"

    # Копируем с атомарной заменой
    local dest="${GEOIP_DIR}/${db_name}.mmdb"
    local dest_tmp="${dest}.tmp.$$"
    cp "$mmdb_file" "$dest_tmp"
    mv "$dest_tmp" "$dest"
    chmod 644 "$dest"

    log "Installed: ${dest}"
}

copy_to_docker_volume() {
    if [[ -z "$DOCKER_VOLUME" ]]; then return; fi

    log "Copying to Docker volume: ${DOCKER_VOLUME}..."
    if ! docker volume inspect "$DOCKER_VOLUME" &>/dev/null; then
        log "Volume ${DOCKER_VOLUME} does not exist, skipping copy"
        return
    fi

    docker run --rm \
        -v "${GEOIP_DIR}:/source:ro" \
        -v "${DOCKER_VOLUME}:/target" \
        alpine \
        sh -c "cp /source/GeoLite2-City.mmdb /target/ && cp /source/GeoLite2-ASN.mmdb /target/ && echo 'Copied to volume'"

    log "Docker volume updated: ${DOCKER_VOLUME}"
}

reload_siem_parser() {
    # Посылаем SIGHUP siem-parser если он запущен в Docker
    if docker ps --filter "name=siem-parser" --format "{{.Names}}" 2>/dev/null | grep -q "siem-parser"; then
        log "Sending SIGHUP to siem-parser to reload GeoIP databases..."
        docker kill --signal=SIGUSR1 siem-parser 2>/dev/null || true
    fi
}

# ── Основной скрипт ───────────────────────────────────────────────────────────

main() {
    log "=== GeoIP Database Update Script ==="
    log "Target directory: ${GEOIP_DIR}"

    # Проверка зависимостей
    check_dependencies

    # Проверка лицензионного ключа
    if [[ -z "$MAXMIND_LICENSE_KEY" ]]; then
        echo ""
        echo "ERROR: MAXMIND_LICENSE_KEY is not set."
        echo ""
        echo "Получите бесплатный ключ на: https://www.maxmind.com/en/geolite2/signup"
        echo ""
        echo "Затем запустите:"
        echo "  MAXMIND_LICENSE_KEY=your_key bash scripts/update-geoip.sh"
        echo ""
        echo "Или для Docker volume:"
        echo "  MAXMIND_LICENSE_KEY=your_key DOCKER_VOLUME=siem-lite_geoip-data bash scripts/update-geoip.sh"
        echo ""
        exit 1
    fi

    # Создаём директорию если нет
    mkdir -p "$GEOIP_DIR"

    # Скачиваем каждую базу
    local failed=0
    for db in "${DATABASES[@]}"; do
        if download_database "$db"; then
            log "✓ ${db} updated successfully"
        else
            log "✗ Failed to update ${db}"
            ((failed++)) || true
        fi
    done

    if [[ $failed -gt 0 ]]; then
        error "${failed} database(s) failed to update"
    fi

    # Копируем в Docker volume (если задан)
    copy_to_docker_volume

    # Перезагружаем siem-parser
    reload_siem_parser

    log "=== GeoIP update complete ==="
    log "Files:"
    ls -lh "${GEOIP_DIR}"/*.mmdb 2>/dev/null || log "(no .mmdb files found)"
}

main "$@"
