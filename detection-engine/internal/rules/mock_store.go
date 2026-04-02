package rules

import (
	"sync"
	"time"
)

// MockStateStore — in-memory реализация StateStore для тестов.
type MockStateStore struct {
	mu       sync.Mutex
	counters map[string]int64
	sets     map[string]map[string]struct{}
}

func NewMockStateStore() *MockStateStore {
	return &MockStateStore{
		counters: make(map[string]int64),
		sets:     make(map[string]map[string]struct{}),
	}
}

func (m *MockStateStore) Increment(key string, _ time.Duration) (int64, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.counters[key]++
	return m.counters[key], nil
}

func (m *MockStateStore) Get(key string) (int64, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.counters[key], nil
}

func (m *MockStateStore) AddToSet(key string, member string, _ time.Duration) (int64, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.sets[key] == nil {
		m.sets[key] = make(map[string]struct{})
	}
	m.sets[key][member] = struct{}{}
	return int64(len(m.sets[key])), nil
}

func (m *MockStateStore) SetSize(key string) (int64, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	return int64(len(m.sets[key])), nil
}

// Reset очищает все счётчики (удобно между тест-кейсами).
func (m *MockStateStore) Reset() {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.counters = make(map[string]int64)
	m.sets = make(map[string]map[string]struct{})
}

// SetCounter устанавливает счётчик напрямую (для тестов пограничных случаев).
func (m *MockStateStore) SetCounter(key string, val int64) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.counters[key] = val
}
