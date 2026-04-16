use anyhow::{Context, Result};
use async_trait::async_trait;
use std::time::Duration;

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn increment(&self, key: &str, ttl: Duration) -> Result<i64>;
    async fn get(&self, key: &str) -> Result<i64>;
    async fn add_to_set(&self, key: &str, member: &str, ttl: Duration) -> Result<i64>;
    async fn set_size(&self, key: &str) -> Result<i64>;
}

pub struct RedisStore {
    conn: redis::aio::MultiplexedConnection,
}

impl RedisStore {
    pub async fn new(addr: &str, password: &str, db: i64) -> Result<Self> {
        let url = if password.is_empty() {
            format!("redis://{}/{}", addr, db)
        } else {
            format!("redis://:{}@{}/{}", password, addr, db)
        };
        let client = redis::Client::open(url.as_str())
            .with_context(|| format!("invalid redis URL for {}", addr))?;
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .with_context(|| format!("failed to connect to redis at {}", addr))?;
        Ok(Self { conn })
    }

    pub async fn ping(&self) -> Result<()> {
        let mut conn = self.conn.clone();
        let _pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .context("redis PING failed")?;
        Ok(())
    }
}

#[async_trait]
impl StateStore for RedisStore {
    async fn increment(&self, key: &str, ttl: Duration) -> Result<i64> {
        let mut conn = self.conn.clone();
        let ttl_secs = ttl.as_secs() as usize;
        // INCR returns the new value; only set EXPIRE when the key is new (count == 1).
        // This avoids resetting the TTL on every increment, saving a round-trip
        // in the common case where the key already exists.
        let count: i64 = redis::cmd("INCR")
            .arg(key)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("redis increment {}", key))?;
        if count == 1 {
            redis::cmd("EXPIRE")
                .arg(key)
                .arg(ttl_secs)
                .query_async::<()>(&mut conn)
                .await
                .with_context(|| format!("redis expire {}", key))?;
        }
        Ok(count)
    }

    async fn get(&self, key: &str) -> Result<i64> {
        let mut conn = self.conn.clone();
        let val: Option<i64> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("redis get {}", key))?;
        Ok(val.unwrap_or(0))
    }

    async fn add_to_set(&self, key: &str, member: &str, ttl: Duration) -> Result<i64> {
        let mut conn = self.conn.clone();
        let ttl_secs = ttl.as_secs() as usize;
        // SADD returns the number of members added; only set EXPIRE when a new
        // member was added to a new key (added == 1 and set didn't exist before).
        // Check with TTL first to avoid unnecessary EXPIRE calls.
        let added: i64 = redis::cmd("SADD")
            .arg(key)
            .arg(member)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("redis sadd {}", key))?;
        if added == 1 {
            // Only set expiry if the key has no TTL set (-1 means no expiry)
            let current_ttl: i64 = redis::cmd("TTL")
                .arg(key)
                .query_async(&mut conn)
                .await
                .with_context(|| format!("redis ttl {}", key))?;
            if current_ttl == -1 {
                redis::cmd("EXPIRE")
                    .arg(key)
                    .arg(ttl_secs)
                    .query_async::<()>(&mut conn)
                    .await
                    .with_context(|| format!("redis expire {}", key))?;
            }
        }
        Ok(added)
    }

    async fn set_size(&self, key: &str) -> Result<i64> {
        let mut conn = self.conn.clone();
        let size: i64 = redis::cmd("SCARD")
            .arg(key)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("redis scard {}", key))?;
        Ok(size)
    }
}
