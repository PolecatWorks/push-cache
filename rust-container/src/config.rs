//! # Configuration Module
//!
//! This module provides the configuration structures and utilities for the application.
//! It includes deserialization of configuration data from YAML files and the ability
//! to handle URLs with optional username and password credentials.
//!
//! ## Structures
//!
//! - `UrlWithUsernamePassword`: Represents a URL with optional username and password fields.
//! - `MyConfig`: The main configuration structure for the application, containing
//!   configurations for the web service, runtime, persistence, and other components.
//!
//! ## Features
//!
//! - **URL Handling**: The `UrlWithUsernamePassword` struct allows for easy handling of URLs
//!   that may include username and password credentials. It provides a conversion
//!   implementation to transform into a `Url` type.
//!
//! - **Configuration Loading**: The `MyConfig` struct provides a method to load configuration
//!   data using the `Figment` library. It supports merging YAML configuration strings
//!   with secrets stored in a specified directory.
//!
//! ## Usage
//!
//! To use this module, you can define your configuration in a YAML file and load it
//! using the `MyConfig::figment` method. This allows for flexible and structured
//! configuration management in your application.
//!
//! ## Dependencies
//!
//! - `figment`: For managing configuration profiles and merging configuration sources.
//! - `figment_file_provider_adapter`: For adapting file-based configuration sources.
//! - `serde`: For deserializing configuration data.
//! - `url`: For handling and manipulating URLs.
//!
//! ## Example
//!
//! ```rust
//! use push_cache::config::MyConfig;
//!
//! let yaml_config = r#"
//! hams:
//!   address: "0.0.0.0:8079"
//!   prefix: "hams"
//!   logging: true
//!   checks:
//!     timeout: 5
//!     fails: 2
//!     preflights: []
//!     shutdowns: []
//! runtime:
//!   threads: 4
//!   stack_size: 3145728
//!   name: "push-cache"
//! webservice:
//!   prefix: "/cache"
//!   address: "http://0.0.0.0:8080"
//!   forwarding_headers: []
//! kafka:
//!   brokers: "tcp://localhost:9092"
//!   group_id: "push-cache"
//!   topic: "users"
//!   schema_registry_url: "http://localhost:8081"
//!   cache_max_age: 60s
//!   fetch_metadata_timeout: 5s
//!   offset_reset: earliest
//!   force_reset_earliest: false
//! startup_checks:
//!   fails: 2
//!   timeout: 5s
//!   enabled: true
//! cache:
//!   stores:
//!     - name: "mem"
//!       type: "in_memory"
//!       schemas: []
//!   routes:
//!     - path: "/users"
//!       store: "mem"
//! "#;
//!
//! let secrets_path = "/path/to/secrets";
//! let figment = MyConfig::figment(yaml_config, secrets_path);
//! let config: MyConfig = figment.extract().expect("Failed to load configuration");
//! ```
//!
//! This example demonstrates how to load a YAML configuration string and merge it
//! with secrets stored in a specified directory.
use std::{path::Path, time::Duration};

use ::hams::hams::config::HamsConfig;
use figment::{
    Figment,
    providers::{Env, Format, Yaml},
};
use figment_file_provider_adapter::FileAdapter;
use serde::Deserialize;
use url::Url;

use crate::{tokio_tools::ThreadRuntime, webserver::WebServiceConfig};

// NOTE: Configs should not use defaults to ensure the user is aware of all the options

#[derive(Deserialize, Debug, Clone)]
pub struct UrlWithUsernamePassword {
    pub url: Url,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl From<UrlWithUsernamePassword> for Url {
    fn from(value: UrlWithUsernamePassword) -> Self {
        let mut return_url = value.url;

        if let Some(password) = value.password {
            return_url.set_password(Some(&password)).unwrap();
        }
        if let Some(username) = value.username {
            return_url.set_username(&username).unwrap();
        }
        return_url
    }
}

#[derive(Deserialize, Clone)]
pub struct MyConfig {
    /// Config of my web service
    pub hams: HamsConfig,
    pub runtime: ThreadRuntime,
    pub webservice: WebServiceConfig,
    pub kafka: MyKafkaConfig,
    pub startup_checks: StartupCheckConfig,
    pub cache: CacheConfig,
}

/// Configuration for the cache behavior and routing.
#[derive(Deserialize, Debug, Clone)]
pub struct CacheConfig {
    /// List of store definitions (e.g., in-memory, Redis).
    pub stores: Vec<StoreDefinition>,
    /// List of routes mapping paths to stores.
    pub routes: Vec<RouteDefinition>,
}

/// Defines a specific cache store.
#[derive(Deserialize, Debug, Clone)]
pub struct StoreDefinition {
    /// Unique name for the store.
    pub name: String,
    /// The type of the store and its specific configuration.
    #[serde(flatten)]
    pub store_type: StoreType,
    /// Optional list of schemas associated with this store.
    pub schemas: Option<Vec<String>>,
}

/// Supported types of cache stores.
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StoreType {
    /// In-memory cache store.
    InMemory,
    /// Redis cache store with specific configuration.
    Redis(RedisConfig),
    /// MongoDB cache store with specific configuration.
    Mongo(MongoConfig),
    /// Oracle cache store with specific configuration.
    Oracle(OracleConfig),
    /// Postgres cache store with specific configuration.
    Postgres(PostgresConfig),
}

/// Defines a route that maps a URL path to a specific cache store.
#[derive(Deserialize, Debug, Clone)]
pub struct RouteDefinition {
    /// The URL path to match.
    pub path: String,
    /// The name of the store to use for this route.
    pub store: String,
    /// Optional field name to extract the ID from the JSON body.
    #[serde(default)]
    pub key_from_body: Option<String>,
}

/// Configuration for a Redis cache store.
#[derive(Deserialize, Debug, Clone)]
pub struct RedisConfig {
    /// The Redis connection URL.
    pub url: Url,
    /// Optional prefix for Redis keys.
    pub prefix: Option<String>,
}

/// Configuration for a Mongo cache store.
#[derive(Deserialize, Debug, Clone)]
pub struct MongoConfig {
    /// The MongoDB connection URL.
    pub url: Url,
    /// The database name to use.
    pub database: String,
    /// The collection name to use.
    pub collection: String,
    /// The minimum number of connections in the connection pool.
    pub min_pool_size: Option<u32>,
    /// The maximum number of connections in the connection pool.
    pub max_pool_size: Option<u32>,
}

/// Configuration for an Oracle cache store.
#[derive(Deserialize, Debug, Clone)]
pub struct OracleConfig {
    /// The Oracle connection URL.
    pub url: UrlWithUsernamePassword,
    /// The table name to use for this cache store.
    pub table_name: String,
}

/// Configuration for a Postgres cache store.
#[derive(Deserialize, Debug, Clone)]
pub struct PostgresConfig {
    /// The Postgres connection URL.
    pub url: UrlWithUsernamePassword,
    /// The table name to use for this cache store.
    pub table_name: String,
}

/// Configuration for the Kafka consumer.
#[derive(Deserialize, Debug, Clone)]
pub struct MyKafkaConfig {
    /// The Kafka brokers URL.
    pub brokers: Url,
    /// The consumer group ID configuration.
    pub group_id: GroupId,
    /// The Kafka topic to consume from.
    pub topic: String,
    /// The Schema Registry URL.
    pub schema_registry_url: Url,
    /// Maximum age of cached items.
    #[serde(with = "humantime_serde")]
    pub cache_max_age: Duration,
    /// Timeout for fetching metadata.
    #[serde(with = "humantime_serde")]
    pub fetch_metadata_timeout: Duration,
    /// Initial offset reset strategy.
    pub offset_reset: KafkaOffsetReset,
    /// Whether to force reset offsets to earliest on startup.
    pub force_reset_earliest: bool,
    /// Optional list of schema IDs to preload at startup.
    pub preload_schemas: Option<Vec<u32>>,
}

impl MyKafkaConfig {
    pub fn get_group_id(&self) -> Result<String, String> {
        match &self.group_id {
            GroupId::Hostname { use_hostname } => {
                if *use_hostname {
                    std::env::var("HOSTNAME").map_err(|_| {
                        "HOSTNAME environment variable is required when use_hostname is true"
                            .to_string()
                    })
                } else {
                    Err("use_hostname must be true if provided as an object".to_string())
                }
            }
            GroupId::Explicit(id) => Ok(id.clone()),
        }
    }
}

/// Configuration for the Kafka consumer group ID.
/// Can be either an explicit string or a directive to use the hostname.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum GroupId {
    /// Use the system's hostname as the group ID.
    Hostname { use_hostname: bool },
    /// Use an explicit string as the group ID.
    Explicit(String),
}

/// Strategy for resetting Kafka offsets when no initial offset is found.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum KafkaOffsetReset {
    /// Start from the earliest available offset.
    Earliest,
    /// Start from the latest available offset.
    Latest,
}

impl std::fmt::Display for KafkaOffsetReset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KafkaOffsetReset::Earliest => write!(f, "earliest"),
            KafkaOffsetReset::Latest => write!(f, "latest"),
        }
    }
}

/// Configuration for startup health checks.
#[derive(Deserialize, Debug, Clone)]
pub struct StartupCheckConfig {
    /// Number of allowed failures before giving up.
    pub fails: u32,
    /// Timeout for each check.
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
    /// Whether startup checks are enabled.
    pub enabled: bool,
}

impl MyConfig {
    // Note the `nested` option on both `file` providers. This makes each
    // top-level dictionary act as a profile.
    pub fn figment<P: AsRef<Path> + Clone>(yaml_string: &str, secrets: P) -> Figment {
        Figment::new()
            .merge(FileAdapter::wrap(Yaml::string(yaml_string)).relative_to_dir(secrets))
            .merge(Env::prefixed("APP_").split("__"))
    }
}

#[cfg(test)]
mod test {
    use url::Url;

    use super::*;

    #[test]
    fn try_out_enum() {
        let temp_url = UrlWithUsernamePassword {
            url: Url::parse("postgres://myuser:mypass@localhost/mydb").unwrap(),
            username: None,
            password: None,
        };
        assert_eq!(
            Into::<Url>::into(temp_url).as_str(),
            "postgres://myuser:mypass@localhost/mydb"
        );

        let temp_url = UrlWithUsernamePassword {
            url: Url::parse("postgres://myuser:mypass@localhost/mydb").unwrap(),
            username: Some("user0".to_owned()),
            password: Some("pass0".to_owned()),
        };
        assert_eq!(
            Into::<Url>::into(temp_url).as_str(),
            "postgres://user0:pass0@localhost/mydb"
        );
    }

    #[test]
    fn test_group_id_deserialization() {
        use figment::providers::Format;

        #[derive(Deserialize)]
        struct ConfigWrapper {
            group_id: GroupId,
        }

        // Test explicit string
        let yaml = r#"
            group_id: "my-group"
        "#;
        let config: ConfigWrapper = Figment::new().merge(Yaml::string(yaml)).extract().unwrap();
        match config.group_id {
            GroupId::Explicit(val) => assert_eq!(val, "my-group"),
            _ => panic!("Expected Explicit variant"),
        }

        // Test hostname object
        let yaml = r#"
            group_id:
              use_hostname: true
        "#;
        let config: ConfigWrapper = Figment::new().merge(Yaml::string(yaml)).extract().unwrap();
        match config.group_id {
            GroupId::Hostname { use_hostname } => assert!(use_hostname),
            _ => panic!("Expected Hostname variant"),
        }
    }

    #[test]
    fn test_preload_schemas_deserialization() {
        use figment::providers::Format;

        #[derive(Deserialize)]
        struct ConfigWrapper {
            preload_schemas: Option<Vec<u32>>,
        }

        // Test with list of IDs
        let yaml = r#"
            preload_schemas: [101, 102, 103]
        "#;
        let config: ConfigWrapper = Figment::new().merge(Yaml::string(yaml)).extract().unwrap();
        assert_eq!(config.preload_schemas.unwrap(), vec![101, 102, 103]);

        // Test empty list
        let yaml = r#"
            preload_schemas: []
        "#;
        let config: ConfigWrapper = Figment::new().merge(Yaml::string(yaml)).extract().unwrap();
        assert_eq!(config.preload_schemas.unwrap(), Vec::<u32>::new());

        // Test missing field
        let yaml = r#"
            other_field: "value"
        "#;
        let config: ConfigWrapper = Figment::new().merge(Yaml::string(yaml)).extract().unwrap();
        assert!(config.preload_schemas.is_none());
    }
}
