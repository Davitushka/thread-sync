#!/bin/bash
# Генерация mTLS сертификатов для Vector Agent <-> Aggregator коммуникации
# Требует: openssl >= 1.1
# Использование: bash scripts/generate-certs.sh

set -euo pipefail

CERTS_DIR="$(dirname "$0")/../deploy/docker/certs"
mkdir -p "$CERTS_DIR"
cd "$CERTS_DIR"

echo "Generating Vector mTLS certificates in $CERTS_DIR"

# 1. Корневой CA
openssl genrsa -out ca.key 4096
openssl req -new -x509 -days 3650 -key ca.key -out ca.crt \
  -subj "/O=SIEM-Lite/CN=SIEM-CA" \
  -addext "basicConstraints=critical,CA:TRUE,pathlen:0"

# 2. Aggregator сертификат
openssl genrsa -out aggregator.key 2048
openssl req -new -key aggregator.key -out aggregator.csr \
  -subj "/O=SIEM-Lite/CN=vector-aggregator"
openssl x509 -req -days 365 -in aggregator.csr \
  -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out aggregator.crt \
  -extfile <(printf "subjectAltName=DNS:vector-aggregator,DNS:localhost,IP:127.0.0.1")

# 3. Agent сертификат
openssl genrsa -out agent.key 2048
openssl req -new -key agent.key -out agent.csr \
  -subj "/O=SIEM-Lite/CN=vector-agent"
openssl x509 -req -days 365 -in agent.csr \
  -CA ca.crt -CAkey ca.key -CAcreateserial \
  -out agent.crt \
  -extfile <(printf "subjectAltName=DNS:vector-agent,DNS:localhost")

# Проверяем сертификаты
echo ""
echo "Verifying certificates..."
openssl verify -CAfile ca.crt aggregator.crt && echo "✓ aggregator.crt valid"
openssl verify -CAfile ca.crt agent.crt && echo "✓ agent.crt valid"

# Удаляем CSR файлы
rm -f *.csr *.srl

# Устанавливаем права
chmod 644 ca.crt aggregator.crt agent.crt
chmod 600 ca.key aggregator.key agent.key

echo ""
echo "Certificates generated in: $CERTS_DIR"
ls -la "$CERTS_DIR"
