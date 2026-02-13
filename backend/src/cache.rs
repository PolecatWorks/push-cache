use async_trait::async_trait;
use dashmap::DashMap;
use prometheus::IntGauge;
use redis::AsyncCommands;
use tracing::error;

use crate::config::{RedisConfig, PostgresConfig};
use crate::error::MyError;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

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
    prefix: Option<String>,
}

impl RedisCache {
    pub async fn new(config: &RedisConfig) -> Result<Self, MyError> {
        let url: url::Url = config.url.clone();
        let client = redis::Client::open(url.as_str())
            .map_err(|e| MyError::Message(format!("Redis connect error: {e}")))?;

        let manager = client
            .get_connection_manager()
            .await
            .map_err(|e| MyError::Message(format!("Redis connection manager error: {e}")))?;

        Ok(Self {
            manager,
            prefix: config.prefix.clone(),
        })
    }

    fn format_key(&self, key: &str) -> String {
        match &self.prefix {
            Some(p) => format!("{p}:{key}"),
            None => key.to_string(),
        }
    }
}

#[async_trait]
impl Cache for RedisCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let mut conn = self.manager.clone();
        let query_key = self.format_key(key);
        conn.get::<_, Option<Vec<u8>>>(query_key)
            .await
            .map_err(|e| {
                error!("Redis get error: {}", e);
                MyError::Message(format!("Redis get error: {e}"))
            })
    }

    async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), MyError> {
        let mut conn = self.manager.clone();
        let query_key = self.format_key(&key);
        conn.set::<_, _, ()>(query_key, value).await.map_err(|e| {
            error!("Redis set error: {}", e);
            MyError::Message(format!("Redis set error: {e}"))
        })
    }

    async fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let mut conn = self.manager.clone();
        let query_key = self.format_key(key);
        // Try GETDEL (Redis 6.2+)
        conn.get_del::<_, Option<Vec<u8>>>(query_key)
            .await
            .map_err(|e| {
                error!("Redis get_del error: {}", e);
                MyError::Message(format!("Redis get_del error: {e}"))
            })
    }

    async fn keys(&self) -> Result<Vec<String>, MyError> {
        let mut conn = self.manager.clone();
        let mut keys = Vec::new();

        // Use SCAN to avoid blocking
        // Note: redis::AsyncIter needs the connection to live long enough
        // Also handling pattern matching

        let match_pattern = match &self.prefix {
            Some(p) => format!("{p}:*"),
            None => "*".to_string(),
        };

        let mut iter: redis::AsyncIter<String> =
            conn.scan_match(&match_pattern).await.map_err(|e| {
                error!("Redis scan error: {}", e);
                MyError::Message(format!("Redis scan error: {e}"))
            })?;

        while let Some(key_result) = iter.next_item().await {
            match key_result {
                Ok(k) => {
                    if let Some(prefix) = &self.prefix {
                        if let Some(stripped) = k.strip_prefix(&format!("{prefix}:")) {
                            keys.push(stripped.to_string());
                        } else {
                            // Should match scan pattern, but just in case
                            keys.push(k);
                        }
                    } else {
                        keys.push(k);
                    }
                }
                Err(e) => {
                    error!("Redis scan error: {}", e);
                    // Return error immediately?
                    return Err(MyError::Message(format!(
                        "Redis scan error during iteration: {e}"
                    )));
                }
            }
        }
        Ok(keys)
    }

    async fn contains_key(&self, key: &str) -> Result<bool, MyError> {
        let mut conn = self.manager.clone();
        let query_key = self.format_key(key);
        conn.exists::<_, bool>(query_key).await.map_err(|e| {
            error!("Redis exists error: {e}");
            MyError::Message(format!("Redis exists error: {e}"))
        })
    }
}

#[derive(Clone)]
pub struct PostgresCache {
    pool: Pool<Postgres>,
    table_name: String,
}

impl PostgresCache {
    pub async fn new(config: &PostgresConfig) -> Result<Self, MyError> {
        let pool = PgPoolOptions::new()
            .max_connections(config.pool_size.unwrap_or(5))
            .connect(config.url.as_str())
            .await
            .map_err(|e| MyError::Message(format!("Postgres connect error: {e}")))?;

        let table_name = &config.table_name;
        // Simple validation
        if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(MyError::Message(format!("Invalid table name: {table_name}")));
        }

        let create_query = format!(
            "CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY, value BYTEA)",
            table_name
        );
        sqlx::query(&create_query)
            .execute(&pool)
            .await
            .map_err(|e| MyError::Message(format!("Postgres create table error: {e}")))?;

        Ok(Self {
            pool,
            table_name: table_name.clone(),
        })
    }
}

#[async_trait]
impl Cache for PostgresCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let query = format!("SELECT value FROM {} WHERE key = $1", self.table_name);
        let result: Option<Vec<u8>> = sqlx::query_scalar(&query)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres get error: {}", e);
                MyError::Message(format!("Postgres get error: {e}"))
            })?;
        Ok(result)
    }

    async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), MyError> {
        let query = format!(
            "INSERT INTO {} (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = $2",
            self.table_name
        );
        sqlx::query(&query)
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres insert error: {}", e);
                MyError::Message(format!("Postgres insert error: {e}"))
            })?;
        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let query = format!("DELETE FROM {} WHERE key = $1 RETURNING value", self.table_name);
        let result: Option<Vec<u8>> = sqlx::query_scalar(&query)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres remove error: {}", e);
                MyError::Message(format!("Postgres remove error: {e}"))
            })?;
        Ok(result)
    }

    async fn keys(&self) -> Result<Vec<String>, MyError> {
        let query = format!("SELECT key FROM {}", self.table_name);
        let keys: Vec<String> = sqlx::query_scalar(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres keys error: {}", e);
                MyError::Message(format!("Postgres keys error: {e}"))
            })?;
        Ok(keys)
    }

    async fn contains_key(&self, key: &str) -> Result<bool, MyError> {
        let query = format!("SELECT EXISTS(SELECT 1 FROM {} WHERE key = $1)", self.table_name);
        let exists: bool = sqlx::query_scalar(&query)
            .bind(key)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres contains_key error: {}", e);
                MyError::Message(format!("Postgres contains_key error: {e}"))
            })?;
        Ok(exists)
    }
}
