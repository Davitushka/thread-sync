# multi-stage build для detection-engine (Go)
# kafka-go (segmentio) — pure Go, CGO не требуется.
FROM golang:1.23-alpine AS builder

WORKDIR /app

RUN apk add --no-cache ca-certificates

COPY go.mod go.sum ./
RUN go mod download

COPY . .
# CGO_ENABLED=0 — статически слинкованный бинарник, работает на любом Alpine/scratch.
RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o /detection-engine ./cmd/detector
RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o /correlator ./cmd/correlator

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM alpine:3.21

RUN apk add --no-cache ca-certificates wget

COPY --from=builder /detection-engine /usr/local/bin/detection-engine
COPY --from=builder /correlator /usr/local/bin/correlator

EXPOSE 9110

HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://localhost:9110/health || exit 1

USER nobody:nobody

ENTRYPOINT ["/usr/local/bin/detection-engine"]
