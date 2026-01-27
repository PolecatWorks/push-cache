use apache_avro::{AvroSchema, Schema, schema::RecordSchema};
use rdkafka::{
    ClientConfig,
    consumer::{BaseConsumer, Consumer},
};
use schema_registry_converter::{
    async_impl::schema_registry::{SrSettings, post_schema},
    schema_registry_common::{SchemaType, SuppliedSchema},
};
use tracing::{error, info, warn};
use url::Url;

use crate::{config::MyKafkaConfig, error::MyError};

/// Checks if the Schema Registry is reachable and supports the specified schema type.
///
/// # Arguments
///
/// * `config` - The application configuration containing the Schema Registry URL.
/// * `schema_type` - The schema type to check for (e.g., "AVRO", "JSON", "PROTOBUF").
///
/// # Errors
///
/// Returns `MyError` if:
/// * The connection to the Schema Registry fails.
/// * The Schema Registry returns a non-success status code.
/// * The response from the Schema Registry cannot be parsed.
/// * The specified `schema_type` is not supported by the Schema Registry.
pub async fn check_schema_registry(url: &Url, schema_type: &str) -> Result<(), MyError> {
    let mut sr_url = url.clone();

    sr_url.set_path("/schemas/types");

    info!("Checking Schema Registry at {sr_url}");
    let res = reqwest::get(sr_url.clone()).await.map_err(|e| {
        MyError::Message(format!(
            "Failed to connect to Schema Registry at {sr_url}: {e}"
        ))
    })?;

    if !res.status().is_success() {
        return Err(MyError::Message(format!(
            "Schema Registry check failed with status: {}",
            res.status()
        )));
    }
    let schema_types = res
        .json::<Vec<String>>()
        .await
        .map_err(|e| MyError::Message(format!("Failed to parse Schema Registry response: {e}")))?;

    if !schema_types.contains(&schema_type.to_uppercase()) {
        return Err(MyError::Message(format!(
            "Schema type {schema_type} is not supported by the Schema Registry. Supported types: {schema_types:?}"
        )));
    }

    Ok(())
}

/// Checks if the Kafka broker is reachable and the specified topic exists.
///
/// # Arguments
///
/// * `url` - The URL of the Kafka broker.
/// * `config` - The Kafka configuration containing broker and topic details.
///
/// # Errors
///
/// Returns `MyError` if:
/// * The connection to the Kafka broker fails.
/// * The metadata for the specified topic cannot be fetched.
/// * The specified topic does not exist or has no partitions.
pub async fn check_kafka_metadata(config: &MyKafkaConfig) -> Result<(), MyError> {
    let consumer: BaseConsumer = ClientConfig::new()
        .set(
            "bootstrap.servers",
            format!(
                "{}:{}",
                config.brokers.host_str().ok_or_else(|| {
                    warn!("Kafka broker host not defined {:?}", config.brokers.host());
                    MyError::Message(format!("Kafka broker host not defined {}", config.brokers))
                })?,
                config.brokers.port().ok_or_else(|| {
                    warn!("Kafka broker port not defined {:?}", config.brokers.port());
                    MyError::Message(format!("Kafka broker port not defined {}", config.brokers))
                })?,
            ),
        )
        .create()?;

    // Fetch metadata for the specific topic
    // Passing Some(topic_name) limits the request to just that topic

    let metadata = consumer.fetch_metadata(Some(&config.topic), config.fetch_metadata_timeout)?;

    let topics = metadata.topics();
    if !topics
        .iter()
        .any(|t| t.name() == config.topic && t.error().is_none() && !t.partitions().is_empty())
    {
        warn!(
            "Kafka topic {} not found or has no partitions",
            config.topic
        );
        return Err(MyError::Message(format!(
            "Kafka topic {} not found or has no partitions",
            config.topic
        )));
    }

    Ok(())
}

pub async fn get_schema_id<T: AvroSchema>(
    registry: &str,
    topic: &str,
) -> Result<(u32, Schema), MyError> {
    let testme_schema = T::get_schema();
    let canonical_form = testme_schema.canonical_form();
    info!("Schema is {}", canonical_form);
    info!("Registry URL: {}", registry);

    if let Schema::Record(RecordSchema { name, .. }) = testme_schema {
        let my_schema = T::get_schema();

        let schema_query = SuppliedSchema {
            name: None,
            schema_type: SchemaType::Avro,
            schema: canonical_form,
            references: vec![],
            properties: None,
            tags: None,
        };

        // Following topic record name pattern for schemas
        let subject = format!("{topic}-{name}");
        let sr_settings = SrSettings::new(registry.to_owned());

        let result = post_schema(&sr_settings, subject.clone(), schema_query.clone()).await;

        if let Err(e) = &result {
            error!("Failed to register schema for subject {subject}: {e:?}");

            // Debugging probe
            let client = reqwest::Client::new();
            match Url::parse(registry) {
                Ok(mut url) => {
                    if let Ok(mut segments) = url.path_segments_mut() {
                        segments.push("subjects");
                        segments.push(&subject);
                        segments.push("versions");
                    }

                    info!("Probing URL: {}", url);
                    match client
                        .post(url)
                        .header("Content-Type", "application/vnd.schemaregistry.v1+json")
                        .json(&schema_query)
                        .send()
                        .await
                    {
                        Ok(resp) => {
                            let status = resp.status();
                            let body = resp
                                .text()
                                .await
                                .unwrap_or_else(|_| "<no body>".to_string());
                            error!("Manual probe returned status: {}, body: {}", status, body);
                        }
                        Err(probe_err) => {
                            error!("Manual probe failed: {}", probe_err);
                        }
                    }
                }
                Err(url_err) => {
                    error!("Failed to parse registry URL for probe: {}", url_err);
                }
            }

            return Err(MyError::Message(format!(
                "Failed to register schema for subject {subject}: {e:?}"
            )));
        }

        let result = result.unwrap();

        info!("Registry replied: {result:?}");
        return Ok((result.id, my_schema));
    }

    Err(MyError::Message(
        "Got a schema that was not Record".to_string(),
    ))
}
