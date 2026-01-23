use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};

// use ffi_log2::log_param;
// use hamsrs::hams_logger_init;

use ffi_log2::log_param;
use hamsrs::hams_logger_init;
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
}

fn main() -> Result<ExitCode, MyError> {
    // let env = EnvFilter::from_env("CAPTURE_LOG");
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
            println!("HaMs Version: {}", hamsrs::hams_version());
        }
        Commands::Start { config, secrets } => {
            info!("Starting {NAME}:{VERSION}");
            // info!("Starting {}", hamsrs::hams_version());

            hams_logger_init(log_param()).unwrap();

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

            debug!("Loaded config {:?}", config);

            service_start(&config)?;
        }
        Commands::ConfigCheck { config, secrets } => {
            info!("Config check {NAME} for {VERSION}");

            let config_yaml = std::fs::read_to_string(config.clone())?;

            let config: MyConfig = MyConfig::figment(&config_yaml, secrets).extract()?;

            debug!("Loaded config {:#?}", config);
        }
    }

    Ok(ExitCode::SUCCESS)
}
