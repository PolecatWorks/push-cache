use async_trait::async_trait;
use dashmap::DashMap;
use prometheus::IntGauge;
use redis::AsyncCommands;
use tracing::error;

use crate::config::RedisConfig;
use crate::error::MyError;

#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MyError>;
    async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), MyError>;
    async fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, MyError>;
    async fn keys(&self) -> Result<Vec<String>, MyError>;
    async fn contains_key(&self, key: &str) -> Result<bool, MyError>;
}

pub struct InMemoryCache {
    map: DashMap<String, Vec<u8>>,
    cache_size: Box<IntGauge>,
}

impl InMemoryCache {
    pub fn new(cache_size: Box<IntGauge>) -> Self {
        Self {
            map: DashMap::new(),
            cache_size,
        }
    }
}

#[async_trait]
impl Cache for InMemoryCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        Ok(self.map.get(key).map(|v| v.value().clone()))
    }

    async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), MyError> {
        if self.map.insert(key, value).is_none() {
            self.cache_size.inc();
        }
        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        if let Some((_, v)) = self.map.remove(key) {
            self.cache_size.dec();
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }

    async fn keys(&self) -> Result<Vec<String>, MyError> {
        Ok(self.map.iter().map(|kv| kv.key().clone()).collect())
    }

    async fn contains_key(&self, key: &str) -> Result<bool, MyError> {
        Ok(self.map.contains_key(key))
    }
}

#[derive(Clone)]
pub struct RedisCache {
    manager: redis::aio::ConnectionManager,
}

impl RedisCache {
    pub async fn new(config: &RedisConfig) -> Result<Self, MyError> {
        let url: url::Url = config.url.clone().into();
        let client = redis::Client::open(url.as_str())
            .map_err(|e| MyError::Message(format!("Redis connect error: {}", e)))?;

        let manager = client.get_connection_manager().await
            .map_err(|e| MyError::Message(format!("Redis connection manager error: {}", e)))?;

        Ok(Self { manager })
    }
}

#[async_trait]
impl Cache for RedisCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let mut conn = self.manager.clone();
        conn.get::<_, Option<Vec<u8>>>(key).await
            .map_err(|e| {
                error!("Redis get error: {}", e);
                MyError::Message(format!("Redis get error: {}", e))
            })
    }

    async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), MyError> {
        let mut conn = self.manager.clone();
        conn.set::<_, _, ()>(key, value).await
            .map_err(|e| {
                error!("Redis set error: {}", e);
                MyError::Message(format!("Redis set error: {}", e))
            })
    }

    async fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let mut conn = self.manager.clone();
        // Try GETDEL (Redis 6.2+)
        conn.get_del::<_, Option<Vec<u8>>>(key).await
            .map_err(|e| {
                error!("Redis get_del error: {}", e);
                MyError::Message(format!("Redis get_del error: {}", e))
            })
    }

    async fn keys(&self) -> Result<Vec<String>, MyError> {
        let mut conn = self.manager.clone();
        let mut keys = Vec::new();

        // Use SCAN to avoid blocking
        // Note: redis::AsyncIter needs the connection to live long enough
        // Also handling pattern matching

        let mut iter: redis::AsyncIter<String> = conn.scan_match("*").await
             .map_err(|e| {
                 error!("Redis scan error: {}", e);
                 MyError::Message(format!("Redis scan error: {}", e))
             })?;

        while let Some(key_result) = iter.next_item().await {
            match key_result {
                Ok(k) => keys.push(k),
                Err(e) => {
                    error!("Redis scan error: {}", e);
                    // Return error immediately?
                    return Err(MyError::Message(format!("Redis scan error during iteration: {}", e)));
                }
            }
        }
        Ok(keys)
    }

    async fn contains_key(&self, key: &str) -> Result<bool, MyError> {
        let mut conn = self.manager.clone();
        conn.exists::<_, bool>(key).await
            .map_err(|e| {
                error!("Redis exists error: {}", e);
                MyError::Message(format!("Redis exists error: {}", e))
            })
    }
}
