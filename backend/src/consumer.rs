use crate::kafka_utils::get_broker_string;

use futures::TryStreamExt;
use rdkafka::Message;
use rdkafka::client::ClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, ConsumerContext, StreamConsumer};
use rdkafka::statistics::Statistics;
use schema_registry_converter::schema_registry_common::BytesResult::Valid;
use schema_registry_converter::schema_registry_common::get_bytes_result;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info, warn};

use crate::MyState;
use crate::error::MyError;
use hamsrs::probes::ProbeManual;

// Context to handle statistics callbacks
struct ConsumerStatsContext {
    state: MyState,
    was_lagging: AtomicBool,
    lag_probe: ProbeManual,
}

impl ClientContext for ConsumerStatsContext {
    fn stats(&self, statistics: Statistics) {
        let mut lag_total = 0;
        let mut offsets = std::collections::HashMap::new();
        for (_topic, topic_stats) in statistics.topics {
            for (partition, part_stats) in topic_stats.partitions {
                let lag = part_stats.consumer_lag;
                if lag > 0 {
                    lag_total += lag;
                }
                offsets.insert(partition, part_stats.committed_offset);
            }
        }
        self.state.consumer_lag.set(lag_total);

        let was_lagging = self.was_lagging.load(Ordering::Relaxed);
        if lag_total > 0 {
            info!("Consumer lag: {} (Offsets: {:?})", lag_total, offsets);
            if !was_lagging {
                self.was_lagging.store(true, Ordering::Relaxed);
            }
        } else if lag_total == 0 {
            if was_lagging {
                info!("Consumer caught up (lag cleared) (Offsets: {:?})", offsets);
                self.was_lagging.store(false, Ordering::Relaxed);
            }

            if !self.state.startup_lag_cleared.load(Ordering::Relaxed) {
                info!("Startup lag cleared, application is now Ready");
                self.state
                    .startup_lag_cleared
                    .store(true, Ordering::Relaxed);
                if let Err(e) = self.lag_probe.enable() {
                    error!("Failed to enable lag probe: {}", e);
                }
            }
        }
    }
}

impl ConsumerContext for ConsumerStatsContext {}

pub async fn start_consumer(state: MyState, lag_probe: ProbeManual) -> Result<(), MyError> {
    let kafka_config = &state.config.kafka;
    info!("Starting Kafka Consumer for topic: {}", kafka_config.topic);

    let group_id = std::env::var("HOSTNAME").unwrap_or_else(|_| {
        warn!(
            "Failed to get hostname, using default group id {}",
            kafka_config.group_id
        );
        kafka_config.group_id.clone()
    });
    info!("Consumer group id: {group_id}");

    let context = ConsumerStatsContext {
        state: state.clone(),
        was_lagging: AtomicBool::new(false),
        lag_probe,
    };

    let consumer: StreamConsumer<ConsumerStatsContext> = ClientConfig::new()
        .set("group.id", &group_id)
        .set("bootstrap.servers", &get_broker_string(kafka_config)?)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set("statistics.interval.ms", "1000")
        .set("auto.offset.reset", kafka_config.offset_reset.to_string())
        .create_with_context(context)?;

    consumer.subscribe(&[&kafka_config.topic])?;

    let stream_processor = consumer.stream().try_for_each(|borrowed_message| {
        let state = state.clone();

        async move {
            match borrowed_message.payload() {
                Some(payload) => {
                    let bytes_result = get_bytes_result(Some(payload));

                    if let Valid(msg_id, _payload) = bytes_result {
                        // info!("Processing message with ID: {}", msg_id);
                        // info!(
                        //     "Processing message with payload: {:?}",
                        //     state.valid_schema_ids
                        // );
                        // Check if schema is in cache, if not fetch it
                        if !state.schema_cache.contains_key(&msg_id) {
                            // We need registry URL
                            let registry_url = &state.config.kafka.schema_registry_url;
                            match crate::kafka_utils::fetch_schema_by_id(
                                registry_url.as_str(),
                                msg_id,
                            )
                            .await
                            {
                                Ok(schema) => {
                                    state.schema_cache.insert(msg_id, schema);
                                    info!("Cached schema for ID: {}", msg_id);
                                }
                                Err(e) => {
                                    error!("Failed to fetch schema {}: {}", msg_id, e);
                                    // Decide if we should skip processing or continue?
                                    // For dynamic cache we might need the schema later.
                                    // But if we fail here, we can retry later or just log error.
                                }
                            }
                        }

                        // Update dynamic cache with FULL payload
                        if let Some(key_bytes) = borrowed_message.key() {
                            if let Ok(key_str) = std::str::from_utf8(key_bytes) {
                                if let Some(full_payload) = borrowed_message.payload() {
                                    state
                                        .cache
                                        .insert(key_str.to_string(), full_payload.to_vec());
                                }
                            }
                        }
                    } else {
                        // Invalid or Null payload handled here?
                        // get_bytes_result returns Null or Invalid.
                        // Check for tombstone if it was Null?
                        // Actually get_bytes_result handles the magic byte check.
                        warn!("Received invalid or non-confluent message");
                    }
                }
                None => {
                    // Tombstone
                    if let Some(key_bytes) = borrowed_message.key() {
                        if let Ok(key_str) = std::str::from_utf8(key_bytes) {
                            state.tombstones_processed.inc();
                            if state.cache.remove(key_str).is_some() {
                                state.cache_size.dec();
                            }
                            state.cache.remove(key_str);
                            info!("Removed record for key: {}", key_str);
                        }
                    } else {
                        warn!(
                            "Received message with null key: partition: {}, offset: {}",
                            borrowed_message.partition(),
                            borrowed_message.offset()
                        );
                    }
                }
            }
            Ok(())
        }
    });

    Ok(stream_processor.await?)
}
