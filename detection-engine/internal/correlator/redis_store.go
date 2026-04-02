// Package correlator содержит реализацию StateStore на базе Redis.
package correlator

import (
	"context"
	"fmt"
	"time"

	"github.com/redis/go-redis/v9"
)

var ctx = context.Background()

// RedisStore реализует StateStore через Redis для хранения sliding window счётчиков.
type RedisStore struct {
	client *redis.Client
}

// NewRedisStore создаёт новый RedisStore.
func NewRedisStore(addr, password string, db int) *RedisStore {
	client := redis.NewClient(&redis.Options{
		Addr:         addr,
		Password:     password,
		DB:           db,
		DialTimeout:  5 * time.Second,
		ReadTimeout:  3 * time.Second,
		WriteTimeout: 3 * time.Second,
		PoolSize:     10,
	})
	return &RedisStore{client: client}
}

// Ping проверяет доступность Redis.
func (s *RedisStore) Ping() error {
	return s.client.Ping(ctx).Err()
}

// Increment атомарно увеличивает счётчик и устанавливает TTL при создании ключа.
// Это sliding window: каждый инкремент обновляет только если ключ уже существует,
// иначе создаёт новый с TTL.
func (s *RedisStore) Increment(key string, ttl time.Duration) (int64, error) {
	pipe := s.client.Pipeline()
	incr := pipe.Incr(ctx, key)
	pipe.Expire(ctx, key, ttl)
	_, err := pipe.Exec(ctx)
	if err != nil {
		return 0, fmt.Errorf("redis increment %s: %w", key, err)
	}
	return incr.Val(), nil
}

// Get возвращает текущее значение счётчика (0 если ключ не существует).
func (s *RedisStore) Get(key string) (int64, error) {
	val, err := s.client.Get(ctx, key).Int64()
	if err == redis.Nil {
		return 0, nil
	}
	if err != nil {
		return 0, fmt.Errorf("redis get %s: %w", key, err)
	}
	return val, nil
}

// AddToSet добавляет элемент в Redis Set и возвращает размер множества.
// Используется для подсчёта уникальных IP/пользователей.
func (s *RedisStore) AddToSet(key string, member string, ttl time.Duration) (int64, error) {
	pipe := s.client.Pipeline()
	sadd := pipe.SAdd(ctx, key, member)
	pipe.Expire(ctx, key, ttl)
	_, err := pipe.Exec(ctx)
	if err != nil {
		return 0, fmt.Errorf("redis sadd %s: %w", key, err)
	}
	return sadd.Val(), nil
}

// SetSize возвращает количество уникальных элементов в Set.
func (s *RedisStore) SetSize(key string) (int64, error) {
	val, err := s.client.SCard(ctx, key).Result()
	if err != nil {
		return 0, fmt.Errorf("redis scard %s: %w", key, err)
	}
	return val, nil
}

// Close закрывает соединение с Redis.
func (s *RedisStore) Close() error {
	return s.client.Close()
}
