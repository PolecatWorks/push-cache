use tracing::{info, warn};

use crate::{
    config::{MyConfig, StartupCheckConfig},
    error::MyError,
};

use crate::kafka_utils::{check_kafka_metadata, check_schema_registry};
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
    name: &str,
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
        if let Ok(reply) = make_future().await {
            info!("Check passed: {name}");
            return Ok(reply);
        }

        attempts_remaining -= 1;
        if attempts_remaining > 0 {
            warn!("Check failed: {name}, rerunning in {:?}", config.timeout);
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

    // Run connectivity checks (Schema Registry & Kafka) in parallel
    // These checks ensure the services are reachable and basic requirements are met.
    // run_check handles retries internally.
    tokio::try_join!(
        run_check("Schema Registry Connectivity", checks_config, || {
            check_schema_registry(&config.kafka.schema_registry_url, "AVRO")
        }),
        run_check("Kafka Metadata Connectivity", checks_config, || {
            check_kafka_metadata(&config.kafka)
        }),
    )?;

    info!("All startup checks passed.");

    Ok(())
}
