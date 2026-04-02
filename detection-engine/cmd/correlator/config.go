package main

import (
	"fmt"
	"os"
	"strconv"
	"time"
)

// Config содержит конфигурацию Correlator сервиса.
type Config struct {
	// HTTP server
	HTTPAddr string

	// Redis
	RedisAddr     string
	RedisPassword string
	RedisDB       int

	// Kafka
	KafkaBrokers string
	KafkaTopic   string
	KafkaGroupID string

	// Alertmanager endpoint для отправки алертов
	AlertmanagerURL string

	// Correlator tuning
	BruteForceThreshold      int
	BruteForceWindow         time.Duration
	RateLimitThreshold       int
	RateLimitWindow          time.Duration
	PrivEscThreshold         int
	PrivEscWindow            time.Duration

	// Graceful shutdown timeout
	ShutdownTimeout time.Duration

	// Logging
	LogLevel string
}

// DefaultConfig возвращает конфигурацию с разумными дефолтами.
func DefaultConfig() *Config {
	return &Config{
		HTTPAddr:                 ":9111",
		RedisAddr:                "redis:6379",
		RedisPassword:            "",
		RedisDB:                  0,
		KafkaBrokers:             "redpanda:9092",
		KafkaTopic:               "siem.events",
		KafkaGroupID:             "correlator",
		AlertmanagerURL:          "http://alertmanager:9093",
		BruteForceThreshold:      10,
		BruteForceWindow:         2 * time.Minute,
		RateLimitThreshold:       500,
		RateLimitWindow:          time.Minute,
		PrivEscThreshold:         3,
		PrivEscWindow:            5 * time.Minute,
		ShutdownTimeout:          30 * time.Second,
		LogLevel:                 "info",
	}
}

// LoadFromEnv заполняет конфигурацию из переменных окружения.
// Переменные с префиксом CORRELATOR_ переопределяют дефолты.
func LoadFromEnv() (*Config, error) {
	cfg := DefaultConfig()

	if v := os.Getenv("CORRELATOR_HTTP_ADDR"); v != "" {
		cfg.HTTPAddr = v
	}
	if v := os.Getenv("REDIS_ADDR"); v != "" {
		cfg.RedisAddr = v
	}
	if v := os.Getenv("REDIS_PASSWORD"); v != "" {
		cfg.RedisPassword = v
	}
	if v := os.Getenv("REDIS_DB"); v != "" {
		db, err := strconv.Atoi(v)
		if err != nil {
			return nil, fmt.Errorf("invalid REDIS_DB: %w", err)
		}
		cfg.RedisDB = db
	}
	if v := os.Getenv("KAFKA_BOOTSTRAP_SERVERS"); v != "" {
		cfg.KafkaBrokers = v
	}
	if v := os.Getenv("KAFKA_TOPIC"); v != "" {
		cfg.KafkaTopic = v
	}
	if v := os.Getenv("KAFKA_GROUP_ID"); v != "" {
		cfg.KafkaGroupID = v
	}
	if v := os.Getenv("ALERTMANAGER_URL"); v != "" {
		cfg.AlertmanagerURL = v
	}
	if v := os.Getenv("CORRELATOR_HTTP_ADDR"); v != "" {
		cfg.HTTPAddr = v
	}

	// Tuning параметры
	if v := os.Getenv("BRUTE_FORCE_THRESHOLD"); v != "" {
		n, err := strconv.Atoi(v)
		if err != nil {
			return nil, fmt.Errorf("invalid BRUTE_FORCE_THRESHOLD: %w", err)
		}
		cfg.BruteForceThreshold = n
	}
	if v := os.Getenv("RATE_LIMIT_THRESHOLD"); v != "" {
		n, err := strconv.Atoi(v)
		if err != nil {
			return nil, fmt.Errorf("invalid RATE_LIMIT_THRESHOLD: %w", err)
		}
		cfg.RateLimitThreshold = n
	}
	if v := os.Getenv("LOG_LEVEL"); v != "" {
		cfg.LogLevel = v
	}

	return cfg, nil
}

// Validate проверяет конфигурацию на корректность.
func (c *Config) Validate() error {
	if c.HTTPAddr == "" {
		return fmt.Errorf("HTTPAddr is required")
	}
	if c.RedisAddr == "" {
		return fmt.Errorf("RedisAddr is required")
	}
	if c.KafkaBrokers == "" {
		return fmt.Errorf("KafkaBrokers is required")
	}
	if c.BruteForceThreshold < 1 {
		return fmt.Errorf("BruteForceThreshold must be >= 1")
	}
	if c.RateLimitThreshold < 1 {
		return fmt.Errorf("RateLimitThreshold must be >= 1")
	}
	return nil
}
