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
//!   address: "0.0.0.0:8080"
//!   forwarding_headers: []
//! kafka:
//!   brokers: "localhost:9092"
//!   group_id: "push-cache"
//!   topic: "users"
//!   schema_registry_url: "http://localhost:8081"
//! "#;
//!
//! let secrets_path = "/path/to/secrets";
//! let figment = MyConfig::figment(yaml_config, secrets_path);
//! let config: MyConfig = figment.extract().expect("Failed to load configuration");
//! ```
//!
//! This example demonstrates how to load a YAML configuration string and merge it
//! with secrets stored in a specified directory.
use std::path::Path;

use figment::{
    Figment,
    providers::{Env, Format, Yaml},
};
use figment_file_provider_adapter::FileAdapter;
use hamsrs::hams::config::HamsConfig;
use serde::Deserialize;
use url::Url;

use crate::{tokio_tools::ThreadRuntime, webserver::WebServiceConfig};

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

#[derive(Deserialize, Debug, Clone, Default)]
pub struct MyConfig {
    /// Config of my web service
    pub hams: HamsConfig,
    pub runtime: ThreadRuntime,
    pub webservice: WebServiceConfig,
    pub kafka: MyKafkaConfig,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct MyKafkaConfig {
    pub brokers: String,
    pub group_id: String,
    pub topic: String,
    pub schema_registry_url: String,
    // Default to 300 seconds (5 minutes)
    #[serde(default = "default_cache_max_age")]
    pub cache_max_age_seconds: u64,
}

fn default_cache_max_age() -> u64 {
    300
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
}
