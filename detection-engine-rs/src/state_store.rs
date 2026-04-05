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
            .get_multiplexed_tokio_connection()
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
        let mut pipe = redis::pipe();
        pipe.cmd("INCR")
            .arg(key)
            .cmd("EXPIRE")
            .arg(key)
            .arg(ttl_secs)
            .ignore();
        let (count,): (i64,) = pipe
            .query_async(&mut conn)
            .await
            .with_context(|| format!("redis increment {}", key))?;
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
        let mut pipe = redis::pipe();
        pipe.cmd("SADD")
            .arg(key)
            .arg(member)
            .cmd("EXPIRE")
            .arg(key)
            .arg(ttl_secs)
            .ignore();
        let (added,): (i64,) = pipe
            .query_async(&mut conn)
            .await
            .with_context(|| format!("redis sadd {}", key))?;
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
