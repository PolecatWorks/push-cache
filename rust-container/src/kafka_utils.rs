use apache_avro::{AvroSchema, Schema, schema::RecordSchema};
use rdkafka::{
    ClientConfig, Offset, TopicPartitionList,
    consumer::{BaseConsumer, CommitMode, Consumer},
};
use schema_registry_converter::{
    async_impl::schema_registry::{SrSettings, post_schema},
    schema_registry_common::{SchemaType, SuppliedSchema},
};
use tracing::{error, info, warn};
use url::Url;

use crate::{config::MyKafkaConfig, error::MyError};

/// Constructs a Kafka broker connection string from the configuration.
///
/// # Arguments
///
/// * `config` - The Kafka configuration containing broker URL details.
///
/// # Returns
///
/// A connection string in the format "host:port".
///
/// # Errors
///
/// Returns `MyError` if:
/// * The broker URL does not contain a valid host.
/// * The broker URL does not contain a valid port.
pub fn get_broker_string(config: &MyKafkaConfig) -> Result<String, MyError> {
    let host = config.brokers.host_str().ok_or_else(|| {
        MyError::Message(format!("Kafka broker host not defined {}", config.brokers))
    })?;
    let port = config.brokers.port().ok_or_else(|| {
        MyError::Message(format!("Kafka broker port not defined {}", config.brokers))
    })?;
    Ok(format!("{host}:{port}"))
}

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

/// Resets consumer group offsets to the earliest available offset for all partitions.
///
/// This function forces the consumer group to start reading from the beginning of the topic
/// by setting the committed offset for each partition to the earliest available offset (low watermark).
///
/// # Arguments
///
/// * `config` - The Kafka configuration containing broker, topic, and consumer group details.
///
/// # Returns
///
/// `Ok(())` if the offsets were successfully reset.
///
/// # Errors
///
/// Returns `MyError` if:
/// * The broker connection string cannot be constructed.
/// * The consumer cannot be created.
/// * Metadata for the topic cannot be fetched.
/// * The specified topic does not exist.
/// * The topic has metadata errors.
/// * The topic has no partitions.
/// * Watermarks cannot be fetched for any partition.
/// * The offset commit operation fails.
pub async fn reset_consumer_offsets(config: &MyKafkaConfig) -> Result<(), MyError> {
    info!(
        "Forcing consumer group offsets to earliest for topic: {}",
        config.topic
    );

    let consumer: BaseConsumer = ClientConfig::new()
        .set(
            "group.id",
            &config.get_group_id().map_err(MyError::Message)?,
        )
        .set("bootstrap.servers", &get_broker_string(config)?)
        .set("enable.auto.commit", "false")
        .create()?;

    // Fetch metadata to find partitions
    let metadata = consumer.fetch_metadata(Some(&config.topic), config.fetch_metadata_timeout)?;

    let topic_metadata = metadata
        .topics()
        .iter()
        .find(|t| t.name() == config.topic)
        .ok_or_else(|| MyError::Message(format!("Topic {} not found", config.topic)))?;

    if let Some(err) = topic_metadata.error() {
        return Err(MyError::Message(format!(
            "Metadata error for topic {}: {:?}",
            config.topic, err
        )));
    }

    let partitions = topic_metadata.partitions();
    if partitions.is_empty() {
        return Err(MyError::Message(format!(
            "Topic {} has no partitions",
            config.topic
        )));
    }

    let mut tpl = TopicPartitionList::new();
    for p in partitions {
        tpl.add_partition(&config.topic, p.id());
    }

    // Log current offsets

    let offsets = consumer.committed_offsets(tpl.clone(), config.fetch_metadata_timeout)?;
    for element in offsets.elements() {
        info!(
            "Current committed offset for partition {}: {:?}",
            element.partition(),
            element.offset()
        );
    }

    for p in partitions {
        let partition_id = p.id();
        // Fetch watermarks (low, high). Low is earliest available offset.
        let (low, _high) = consumer.fetch_watermarks(
            &config.topic,
            partition_id,
            config.fetch_metadata_timeout,
        )?;
        info!("Partition {}: resetting to offset {}", partition_id, low);
        tpl.set_partition_offset(&config.topic, partition_id, Offset::Offset(low))?;
    }

    consumer.commit(&tpl, CommitMode::Sync)?;
    info!("Successfully reset consumer group offsets to earliest.");

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
        .set("bootstrap.servers", &get_broker_string(config)?)
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

/// Registers an Avro schema with the Schema Registry and retrieves its schema ID.
///
/// This function takes a type implementing `AvroSchema`, extracts its schema definition,
/// and registers it with the Schema Registry using the topic-record naming pattern.
///
/// # Type Parameters
///
/// * `T` - A type that implements `AvroSchema` trait.
///
/// # Arguments
///
/// * `registry` - The URL of the Schema Registry.
/// * `topic` - The Kafka topic name used to construct the schema subject.
///
/// # Returns
///
/// A tuple containing:
/// * `u32` - The schema ID assigned by the Schema Registry.
/// * `Schema` - The Avro schema object.
///
/// # Errors
///
/// Returns `MyError` if:
/// * The schema is not a Record schema.
/// * The schema registration with the Schema Registry fails.
/// * Network communication with the Schema Registry fails.
pub async fn get_schema_id<T: AvroSchema>(
    registry: &str,
    topic: &str,
) -> Result<(u32, Schema), MyError> {
    let testme_schema = T::get_schema();
    let canonical_form = testme_schema.canonical_form();
    info!("Schema is {}", canonical_form);
    info!("Registery URL: {}", registry);

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

        let result = post_schema(&sr_settings, subject.clone(), schema_query)
            .await
            .map_err(|e| {
                error!("Failed to register schema for subject {subject}: {e:?}");
                MyError::Message(format!(
                    "Failed to register schema for subject {subject}: {e:?}"
                ))
            })?;

        info!("Registry replied: {result:?}");
        return Ok((result.id, my_schema));
    }

    Err(MyError::Message(
        "Got a schema that was not Record".to_string(),
    ))
}

pub async fn fetch_schema_by_id(registry_url: &str, id: u32) -> Result<Schema, MyError> {
    info!(
        "Fetching schema ID {} from Schema Registry at {}",
        id, registry_url
    );

    // Build the URL directly - Schema Registry API: GET /schemas/ids/{id}
    // NOTE: We use direct HTTP instead of schema_registry_converter because
    // the library has a bug parsing responses even in v4.7
    let url = format!("{}schemas/ids/{}", registry_url, id);

    // Make direct HTTP request
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await.map_err(|e| {
        error!("Failed to fetch schema {} from {}: {}", id, url, e);
        MyError::Message(format!("Failed to fetch schema {}: {}", id, e))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        error!(
            "Schema Registry returned status {} for schema {}: {}",
            status, id, body
        );
        return Err(MyError::Message(format!(
            "Schema Registry returned status {} for schema {}: {}",
            status, id, body
        )));
    }

    // Parse response JSON - Schema Registry returns {"schema": "..."}
    #[derive(serde::Deserialize)]
    struct SchemaResponse {
        schema: String,
    }

    let schema_response: SchemaResponse = response.json().await.map_err(|e| {
        error!(
            "Failed to parse Schema Registry response for schema {}: {}",
            id, e
        );
        MyError::Message(format!("Failed to parse schema response: {}", e))
    })?;

    // Parse the schema JSON string into an Avro Schema
    let schema = Schema::parse_str(&schema_response.schema).map_err(|e| {
        error!("Failed to parse Avro schema {}: {}", id, e);
        MyError::Message(format!("Failed to parse fetched schema {}: {}", id, e))
    })?;

    info!(
        "Successfully fetched and parsed schema ID {} from Schema Registry",
        id
    );
    Ok(schema)
}
