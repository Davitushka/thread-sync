# multi-stage build для detection-engine (Go)
FROM golang:1.23-alpine AS builder

WORKDIR /app

# Зависимости для confluent-kafka-go (CGO)
RUN apk add --no-cache gcc musl-dev librdkafka-dev

COPY go.mod go.sum ./
RUN go mod download

COPY . .
RUN CGO_ENABLED=1 go build -ldflags="-s -w" -o /detection-engine ./cmd/detector

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM alpine:3.21

RUN apk add --no-cache librdkafka ca-certificates

COPY --from=builder /detection-engine /usr/local/bin/detection-engine

EXPOSE 9110

HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://localhost:9110/health || exit 1

USER nobody:nobody

ENTRYPOINT ["/usr/local/bin/detection-engine"]
