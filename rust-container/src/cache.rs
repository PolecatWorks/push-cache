use async_trait::async_trait;
use dashmap::DashMap;
use prometheus::IntGauge;
use redis::AsyncCommands;
use tracing::error;

use bson::doc;
use futures_util::stream::StreamExt;
use mongodb::{
    Client, Collection,
    options::{ClientOptions, UpdateOptions},
};
use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::config::{MongoConfig, RedisConfig, PostgresConfig};
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

pub struct MongoCache {
    collection: Collection<bson::Document>,
}

impl MongoCache {
    pub async fn new(config: &MongoConfig) -> Result<Self, MyError> {
        let client_options = ClientOptions::parse(config.url.as_str())
            .await
            .map_err(|e| MyError::Message(format!("Mongo connect error: {e}")))?;

        let client = Client::with_options(client_options)
            .map_err(|e| MyError::Message(format!("Mongo client error: {e}")))?;

        let database = client.database(&config.database);
        let collection = database.collection(&config.collection);

        Ok(Self { collection })
    }
}

#[async_trait]
impl Cache for MongoCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let filter = doc! { "key": key };
        let doc = self.collection.find_one(filter).await.map_err(|e| {
            error!("Mongo find_one error: {}", e);
            MyError::Message(format!("Mongo get error: {e}"))
        })?;

        if let Some(mut d) = doc {
            #[allow(clippy::collapsible_if)]
            if let Some(bson::Bson::Binary(b)) = d.get_mut("value") {
                return Ok(Some(b.bytes.clone()));
            }
        }
        Ok(None)
    }

    async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), MyError> {
        let filter = doc! { "key": &key };
        let update = doc! { "$set": { "key": key, "value": bson::Binary { subtype: bson::spec::BinarySubtype::Generic, bytes: value } } };
        let options = UpdateOptions::builder().upsert(true).build();

        self.collection
            .update_one(filter, update)
            .with_options(options)
            .await
            .map_err(|e| {
                error!("Mongo update_one error: {}", e);
                MyError::Message(format!("Mongo insert error: {e}"))
            })?;

        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        // Return the old value if possible, similar to Redis GETDEL
        let filter = doc! { "key": key };
        let result = self
            .collection
            .find_one_and_delete(filter)
            .await
            .map_err(|e| {
                error!("Mongo find_one_and_delete error: {}", e);
                MyError::Message(format!("Mongo remove error: {e}"))
            })?;

        if let Some(mut d) = result {
            #[allow(clippy::collapsible_if)]
            if let Some(bson::Bson::Binary(b)) = d.get_mut("value") {
                return Ok(Some(b.bytes.clone()));
            }
        }
        Ok(None)
    }

    async fn keys(&self) -> Result<Vec<String>, MyError> {
        let mut cursor = self.collection.find(doc! {}).await.map_err(|e| {
            error!("Mongo find error: {}", e);
            MyError::Message(format!("Mongo keys error: {e}"))
        })?;

        let mut keys = Vec::new();
        while let Some(doc_res) = cursor.next().await {
            match doc_res {
                Ok(doc) => {
                    if let Ok(k) = doc.get_str("key") {
                        keys.push(k.to_string());
                    }
                }
                Err(e) => {
                    error!("Mongo cursor next error: {}", e);
                    return Err(MyError::Message(format!("Mongo iter error: {e}")));
                }
            }
        }
        Ok(keys)
    }

    async fn contains_key(&self, key: &str) -> Result<bool, MyError> {
        let filter = doc! { "key": key };
        let count = self.collection.count_documents(filter).await.map_err(|e| {
            error!("Mongo count_documents error: {}", e);
            MyError::Message(format!("Mongo contains_key error: {e}"))
        })?;

        Ok(count > 0)
    }
}

pub struct PostgresCache {
    pool: PgPool,
    table_name: String,
}

impl PostgresCache {
    pub async fn new(config: &PostgresConfig) -> Result<Self, MyError> {
        let pool = PgPoolOptions::new()
            .max_connections(config.pool_size.unwrap_or(5))
            .connect(config.url.as_str())
            .await
            .map_err(|e| MyError::Message(format!("Postgres connect error: {e}")))?;

        Ok(Self {
            pool,
            table_name: config.table_name.clone(),
        })
    }
}

#[async_trait]
impl Cache for PostgresCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let query = format!("SELECT value FROM {} WHERE key = $1", self.table_name);

        let row: Option<(Vec<u8>,)> = sqlx::query_as(&query)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres get error: {}", e);
                MyError::Message(format!("Postgres get error: {e}"))
            })?;

        Ok(row.map(|(v,)| v))
    }

    async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), MyError> {
        let query = format!(
            "INSERT INTO {} (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
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

        let row: Option<(Vec<u8>,)> = sqlx::query_as(&query)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres remove error: {}", e);
                MyError::Message(format!("Postgres remove error: {e}"))
            })?;

        Ok(row.map(|(v,)| v))
    }

    async fn keys(&self) -> Result<Vec<String>, MyError> {
        let query = format!("SELECT key FROM {}", self.table_name);

        let rows: Vec<(String,)> = sqlx::query_as(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres keys error: {}", e);
                MyError::Message(format!("Postgres keys error: {e}"))
            })?;

        Ok(rows.into_iter().map(|(k,)| k).collect())
    }

    async fn contains_key(&self, key: &str) -> Result<bool, MyError> {
        let query = format!("SELECT 1 FROM {} WHERE key = $1", self.table_name);

        let row: Option<(i32,)> = sqlx::query_as(&query)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres contains_key error: {}", e);
                MyError::Message(format!("Postgres contains_key error: {e}"))
            })?;

        Ok(row.is_some())
    }
}
