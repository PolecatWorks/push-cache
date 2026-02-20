use std::future::Future;

use futures::FutureExt;
use tracing::{info, warn};
use url::Url;

use crate::{
    config::{MyConfig, StartupCheckConfig},
    error::MyError,
};

use crate::kafka_utils::{check_kafka_metadata, check_schema_registry};

async fn check_redis(url: &Url) -> Result<(), MyError> {
    let client = redis::Client::open(url.as_str())
        .map_err(|e| MyError::Message(format!("Redis connect error: {}", e)))?;
    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| MyError::Message(format!("Redis connection error: {}", e)))?;

    redis::cmd("PING")
        .query_async::<()>(&mut con)
        .await
        .map_err(|e| MyError::Message(format!("Redis PING error: {}", e)))?;
    Ok(())
}

/// Executes an asynchronous check with a retry mechanism.
///
/// This function repeatedly calls the `make_future` closure to generate and await a future
/// until it succeeds or the maximum number of attempts specified in `config` is reached.
/// It waits for the duration specified in `config.timeout` between failed attempts.
///
/// # Arguments
///
/// * `name` - A descriptive name for the check, used in logging and error messages.
/// * `config` - Configuration defining the number of retries and the timeout between them.
/// * `make_future` - A closure that produces the future to be executed for each attempt.
///
/// # Errors
///
/// Returns `MyError` if the check fails after all configured attempts.
pub async fn run_check<G, F, T>(
    name: String,
    config: &StartupCheckConfig,
    mut make_future: G,
) -> Result<T, MyError>
where
    G: FnMut() -> F, // G is a generator that creates futures
    F: Future<Output = Result<T, MyError>>,
{
    info!("Running check: {name}");

    let mut attempts_remaining = config.fails;

    while attempts_remaining > 0 {
        // Call the closure to get a fresh future instance for this attempt
        match make_future().await {
            Ok(reply) => {
                info!("Check passed: {name}");
                return Ok(reply);
            }
            Err(err) => {
                warn!(
                    "Check failed: {name}, error= {err} rerunning in {:?}",
                    config.timeout
                );
            }
        }

        attempts_remaining -= 1;
        if attempts_remaining > 0 {
            warn!(
                "Check failed: {name}, {attempts_remaining} attempts remaining, rerunning in {:?}",
                config.timeout
            );
            tokio::time::sleep(config.timeout).await;
        }
    }

    Err(MyError::Message(format!(
        "Check {} failed after {} attempts",
        name, config.fails
    )))
}

pub async fn run_startup_checks(config: &MyConfig) -> Result<(), MyError> {
    let checks_config = &config.startup_checks;
    let mut futures = Vec::new();

    // Run connectivity checks (Schema Registry & Kafka) in parallel
    // These checks ensure the services are reachable and basic requirements are met.
    // run_check handles retries internally.

    futures.push(
        run_check(
            "Schema Registry Connectivity".to_string(),
            checks_config,
            || check_schema_registry(&config.kafka.schema_registry_url, "AVRO"),
        )
        .boxed(),
    );

    futures.push(
        run_check(
            "Kafka Metadata Connectivity".to_string(),
            checks_config,
            || check_kafka_metadata(&config.kafka),
        )
        .boxed(),
    );

    for store in &config.cache.stores {
        if let crate::config::StoreType::Redis(redis_conf) = &store.store_type {
            let url: Url = redis_conf.url.clone().into();
            let name = format!("Redis Store: {}", store.name);
            futures.push(
                run_check(name, checks_config, move || {
                    let u = url.clone();
                    async move { check_redis(&u).await }
                })
                .boxed(),
            );
        }
    }

    futures::future::try_join_all(futures).await?;

    info!("All startup checks passed.");

    Ok(())
}
