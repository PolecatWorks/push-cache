use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};

use push_cache::config::MyConfig;
use push_cache::error::MyError;
use tracing::level_filters::LevelFilter;
use tracing::{Level, debug, error, info};
use tracing_subscriber::EnvFilter;

use push_cache::{NAME, VERSION, service_start};

/// Application definition to defer to set of commands under [Commands]
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Commands to run inside this program
#[derive(Debug, Subcommand)]
enum Commands {
    /// Show version of application
    Version,
    /// Start the http service
    Start {
        /// Sets a custom config file
        #[arg(short, long, value_name = "FILE")]
        config: PathBuf,
        /// Sets a custom secrets directory
        #[arg(short, long, value_name = "DIR", default_value = PathBuf::from("secrets").into_os_string())]
        secrets: PathBuf,
    },
    ConfigCheck {
        /// Sets a custom config file
        #[arg(short, long, value_name = "FILE")]
        config: PathBuf,
        /// Sets a custom secrets directory
        #[arg(short, long, value_name = "DIR", default_value = PathBuf::from("secrets").into_os_string())]
        secrets: PathBuf,
    },
    /// Create schemas for databases (e.g. Oracle, Postgres) before starting the service
    CreateSchemas {
        /// Sets a custom config file
        #[arg(short, long, value_name = "FILE")]
        config: PathBuf,
        /// Sets a custom secrets directory
        #[arg(short, long, value_name = "DIR", default_value = PathBuf::from("secrets").into_os_string())]
        secrets: PathBuf,
    },
}

fn main() -> Result<ExitCode, MyError> {
    let env = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .with_env_var("CAPTURE_LOG")
        .from_env()?;

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_env_filter(env)
        .init();

    let args = Cli::parse();
    match args.command {
        Commands::Version => {
            println!("{NAME} Version: :{VERSION}");
        }
        Commands::Start { config, secrets } => {
            info!("Starting {NAME}:{VERSION}");

            let config_yaml = match std::fs::read_to_string(config.clone()) {
                Ok(content) => content,
                Err(e) => {
                    error!("Failed to read config file {:?}: {}", config, e);
                    return Err(MyError::Io(e));
                }
            };

            let config: MyConfig = MyConfig::figment(&config_yaml, secrets)
                .extract()
                .unwrap_or_else(|err| {
                    error!("Config file {config:?} failed with error \n{err:#?}");
                    panic!("Config failed to load");
                });

            debug!("Loaded config");

            service_start(&config)?;
        }
        Commands::ConfigCheck { config, secrets } => {
            info!("Config check {NAME} for {VERSION}");

            let config_yaml = std::fs::read_to_string(config.clone())?;

            let _config: MyConfig = MyConfig::figment(&config_yaml, secrets).extract()?;

            debug!("Loaded config successfully");
        }
        Commands::CreateSchemas { config, secrets } => {
            info!("Creating schemas for {NAME}:{VERSION}");

            let config_yaml = match std::fs::read_to_string(config.clone()) {
                Ok(content) => content,
                Err(e) => {
                    error!("Failed to read config file {:?}: {}", config, e);
                    return Err(MyError::Io(e));
                }
            };

            let config: MyConfig = MyConfig::figment(&config_yaml, secrets)
                .extract()
                .unwrap_or_else(|err| {
                    error!("Config file {config:?} failed with error \n{err:#?}");
                    panic!("Config failed to load");
                });

            for store_def in &config.cache.stores {
                match &store_def.store_type {
                    push_cache::config::StoreType::Oracle(oracle_config) => {
                        info!(
                            "Creating table {} for Oracle store {}",
                            oracle_config.table_name, store_def.name
                        );

                        let url: url::Url = oracle_config.url.clone().into();
                        let username = url.username().to_string();
                        let password = url.password().unwrap_or("").to_string();

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

                        let conn = oracle::Connection::connect(&username, &password, &conn_str)
                            .map_err(|e| MyError::Message(format!("Oracle connect error: {e}")))?;

                        let sql = format!(
                            "CREATE TABLE {} (k VARCHAR2(255) PRIMARY KEY, v BLOB)",
                            oracle_config.table_name
                        );

                        match conn.execute(&sql, &[]) {
                            Ok(_) => info!("Successfully created table {}", oracle_config.table_name),
                            Err(e) => {
                                if let Some(db_err) = e.db_error() {
                                    if db_err.code() == 955 {
                                        info!("Table {} already exists", oracle_config.table_name);
                                        continue;
                                    }
                                }
                                error!("Failed to create table {}: {}", oracle_config.table_name, e);
                                return Err(MyError::Message(format!(
                                    "Oracle create table error: {e}"
                                )));
                            }
                        }
                    }
                    push_cache::config::StoreType::Postgres(postgres_config) => {
                        info!(
                            "Creating table {} for Postgres store {}",
                            postgres_config.table_name, store_def.name
                        );

                        // Need a runtime to execute sqlx queries
                        let rt = tokio::runtime::Runtime::new()
                            .map_err(|e| MyError::Message(format!("Tokio runtime error: {e}")))?;

                        rt.block_on(async {
                            let pool = sqlx::postgres::PgPoolOptions::new()
                                .max_connections(1)
                                .connect(postgres_config.url.url.as_str())
                                .await
                                .map_err(|e| MyError::Message(format!("Postgres connect error: {e}")))?;

                            let query = format!(
                                "CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY, value BYTEA)",
                                postgres_config.table_name
                            );

                            sqlx::query(&query)
                                .execute(&pool)
                                .await
                                .map_err(|e| MyError::Message(format!("Postgres create table error: {e}")))?;

                            info!("Successfully created table {} (if it didn't exist)", postgres_config.table_name);
                            Ok::<(), MyError>(())
                        })?;
                    }
                    _ => {}
                }
            }

            info!("Finished creating schemas");
        }
    }

    Ok(ExitCode::SUCCESS)
}
