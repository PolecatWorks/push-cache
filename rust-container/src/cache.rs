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


use crate::config::{MongoConfig, OracleConfig, RedisConfig, PostgresConfig};

use crate::error::MyError;
use oracle::pool::PoolBuilder;

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
        let mut client_options = ClientOptions::parse(config.url.as_str())
            .await
            .map_err(|e| MyError::Message(format!("Mongo connect error: {e}")))?;

        client_options.min_pool_size = config.min_pool_size;
        client_options.max_pool_size = config.max_pool_size;

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


pub struct OracleCache {
    pool: oracle::pool::Pool,
    table_name: String,
}

impl OracleCache {
    pub fn new(config: &OracleConfig) -> Result<Self, MyError> {
        // Assume url is something like oracle://host:port/service_name
        // or just connection string host:port/service_name
        // Need to parse username/password from the URL if present, or format correctly for oracle crate.
        let url: url::Url = config.url.clone().into();

        let username = url.username().to_string();
        let password = url.password().unwrap_or("").to_string();

        // Construct the connection string e.g. //host:port/service_name
        let mut conn_str = String::new();
        if let Some(host) = url.host_str() {
            conn_str.push_str(&format!("//{}", host));
            if let Some(port) = url.port() {
                conn_str.push_str(&format!(":{}", port));
            }
            conn_str.push_str(url.path());
        } else {
            conn_str = url.as_str().to_string();
        }

        let pool = PoolBuilder::new(username, password, conn_str)
            .min_connections(1)
            .max_connections(10)
            .build()
            .map_err(|e| MyError::Message(format!("Oracle pool build error: {e}")))?;

        Ok(Self {
            pool,
            table_name: config.table_name.clone(),
        })
    }
}

#[async_trait]
impl Cache for OracleCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let pool = self.pool.clone();
        let table_name = self.table_name.clone();
        let key = key.to_string();

        tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>, MyError> {
            let conn = pool
                .get()
                .map_err(|e| MyError::Message(format!("Oracle get connection error: {e}")))?;
            let sql = format!("SELECT v FROM {} WHERE k = :1", table_name);
            let result: Result<Vec<u8>, _> = conn.query_row_as(&sql, &[&key]);

            match result {
                Ok(bytes) => Ok(Some(bytes)),
                #[allow(deprecated)]
                Err(oracle::Error::NoDataFound) => Ok(None),
                Err(e) => {
                    error!("Oracle get error: {}", e);
                    Err(MyError::Message(format!("Oracle get error: {e}")))
                }
            }
        })
        .await
        .map_err(|e| MyError::Message(format!("Tokio spawn_blocking error: {e}")))?
    }

    async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), MyError> {
        let pool = self.pool.clone();
        let table_name = self.table_name.clone();

        tokio::task::spawn_blocking(move || -> Result<(), MyError> {
            let conn = pool
                .get()
                .map_err(|e| MyError::Message(format!("Oracle get connection error: {e}")))?;
            let sql = format!(
                "MERGE INTO {} t
                 USING (SELECT :1 AS k, :2 AS v FROM dual) s
                 ON (t.k = s.k)
                 WHEN MATCHED THEN UPDATE SET t.v = s.v
                 WHEN NOT MATCHED THEN INSERT (k, v) VALUES (s.k, s.v)",
                table_name
            );

            conn.execute(&sql, &[&key, &value]).map_err(|e| {
                error!("Oracle insert error: {}", e);
                MyError::Message(format!("Oracle insert error: {e}"))
            })?;
            conn.commit()
                .map_err(|e| MyError::Message(format!("Oracle commit error: {e}")))?;
            Ok(())
        })
        .await
        .map_err(|e| MyError::Message(format!("Tokio spawn_blocking error: {e}")))?
    }

    async fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, MyError> {
        let pool = self.pool.clone();
        let table_name = self.table_name.clone();
        let key = key.to_string();

        tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>, MyError> {
            let conn = pool
                .get()
                .map_err(|e| MyError::Message(format!("Oracle get connection error: {e}")))?;

            // Try to fetch the old value first
            let fetch_sql = format!("SELECT v FROM {} WHERE k = :1", table_name);
            let result: Result<Vec<u8>, _> = conn.query_row_as(&fetch_sql, &[&key]);

            let old_value = match result {
                Ok(bytes) => Some(bytes),
                #[allow(deprecated)]
                Err(oracle::Error::NoDataFound) => None,
                Err(e) => {
                    error!("Oracle fetch for remove error: {}", e);
                    return Err(MyError::Message(format!(
                        "Oracle fetch for remove error: {e}"
                    )));
                }
            };

            let delete_sql = format!("DELETE FROM {} WHERE k = :1", table_name);
            conn.execute(&delete_sql, &[&key]).map_err(|e| {
                error!("Oracle delete error: {}", e);
                MyError::Message(format!("Oracle delete error: {e}"))
            })?;
            conn.commit()
                .map_err(|e| MyError::Message(format!("Oracle commit error: {e}")))?;
            Ok(old_value)
        })
        .await
        .map_err(|e| MyError::Message(format!("Tokio spawn_blocking error: {e}")))?
    }

    async fn keys(&self) -> Result<Vec<String>, MyError> {
        let pool = self.pool.clone();
        let table_name = self.table_name.clone();

        tokio::task::spawn_blocking(move || -> Result<Vec<String>, MyError> {
            let conn = pool
                .get()
                .map_err(|e| MyError::Message(format!("Oracle get connection error: {e}")))?;
            let sql = format!("SELECT k FROM {}", table_name);

            let mut stmt = conn
                .statement(&sql)
                .build()
                .map_err(|e| MyError::Message(format!("Oracle build statement error: {e}")))?;
            let rows = stmt
                .query(&[])
                .map_err(|e| MyError::Message(format!("Oracle query keys error: {e}")))?;

            let mut keys = Vec::new();
            for row_result in rows {
                let row = row_result
                    .map_err(|e| MyError::Message(format!("Oracle fetch row error: {e}")))?;
                let key: String = row
                    .get(0)
                    .map_err(|e| MyError::Message(format!("Oracle get key error: {e}")))?;
                keys.push(key);
            }
            Ok(keys)
        })
        .await
        .map_err(|e| MyError::Message(format!("Tokio spawn_blocking error: {e}")))?
    }

    async fn contains_key(&self, key: &str) -> Result<bool, MyError> {
        let pool = self.pool.clone();
        let table_name = self.table_name.clone();
        let key = key.to_string();

        tokio::task::spawn_blocking(move || -> Result<bool, MyError> {
            let conn = pool
                .get()
                .map_err(|e| MyError::Message(format!("Oracle get connection error: {e}")))?;
            let sql = format!("SELECT COUNT(*) FROM {} WHERE k = :1", table_name);
            let count: i32 = conn.query_row_as(&sql, &[&key]).map_err(|e| {
                error!("Oracle contains_key error: {}", e);
                MyError::Message(format!("Oracle contains_key error: {e}"))
            })?;
            Ok(count > 0)
        })
        .await
        .map_err(|e| MyError::Message(format!("Tokio spawn_blocking error: {e}")))?
    }
}

pub struct PostgresCache {
    pool: PgPool,
    table_name: String,
}

impl PostgresCache {
    pub async fn new(config: &PostgresConfig) -> Result<Self, MyError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(config.url.url.as_str())
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
