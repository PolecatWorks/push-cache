use apache_avro::{AvroSchema, from_avro_datum};
use futures::TryStreamExt;
use rdkafka::Message;
use rdkafka::client::ClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, ConsumerContext, StreamConsumer};
use rdkafka::statistics::Statistics;
use schema_registry_converter::schema_registry_common::BytesResult::Valid;
use schema_registry_converter::schema_registry_common::get_bytes_result;
use std::io::Cursor;
use tracing::{error, info, warn};

use crate::MyState;
use crate::model::Customer;

// Context to handle statistics callbacks
struct ConsumerStatsContext {
    state: MyState,
}

impl ClientContext for ConsumerStatsContext {
    fn stats(&self, statistics: Statistics) {
        let mut lag_total = 0;
        for (_topic, topic_stats) in statistics.topics {
            for (_partition, part_stats) in topic_stats.partitions {
                let lag = part_stats.consumer_lag;
                if lag > 0 {
                    lag_total += lag;
                }
            }
        }
        self.state.consumer_lag.set(lag_total);
    }
}

impl ConsumerContext for ConsumerStatsContext {}

pub async fn start_consumer(state: MyState) {
    let kafka_config = &state.config.kafka;
    info!("Starting Kafka Consumer for topic: {}", kafka_config.topic);

    let group_id = std::env::var("HOSTNAME").unwrap_or_else(|_| kafka_config.group_id.clone());
    info!("Using consumer group id: {}", group_id);

    let context = ConsumerStatsContext {
        state: state.clone(),
    };

    let consumer: StreamConsumer<ConsumerStatsContext> = ClientConfig::new()
        .set("group.id", &group_id)
        .set("bootstrap.servers", &kafka_config.brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set("statistics.interval.ms", "5000")
        .create_with_context(context)
        .expect("Consumer creation failed");

    consumer
        .subscribe(&[&kafka_config.topic])
        .expect("Can't subscribe to specified topic");

    let stream_processor = consumer.stream().try_for_each(|borrowed_message| {
        let state = state.clone();

        async move {
            match borrowed_message.payload() {
                Some(payload) => {
                    let bytes_result = get_bytes_result(Some(payload));

                    if let Valid(msg_id, payload) = bytes_result {
                        if msg_id != state.expected_schema_id {
                            error!(
                                "Schema mismatch! Expected: {}, Found: {}",
                                state.expected_schema_id, msg_id
                            );
                            state.schema_mismatch_count.inc();
                            return Ok(());
                        }

                        // Use static schema for deserialization
                        // Note: from_avro_datum requires the Writer Schema (which we assume matches Customer::get_schema)
                        // If schema registry returns a different ID, technically we should fetch THAT schema to read.
                        // But per requirements, we are using Static Schema "Customer".
                        // Safest path: from_avro_datum(&Customer::get_schema(), &mut Cursor::new(payload), None)

                        match from_avro_datum(
                            &Customer::get_schema(),
                            &mut Cursor::new(payload),
                            None,
                        ) {
                            Ok(val) => match apache_avro::from_value::<Customer>(&val) {
                                Ok(customer) => {
                                    state.updates_received.inc();
                                    use dashmap::mapref::entry::Entry;
                                    match state.cache.entry(customer.accountId.clone()) {
                                        Entry::Vacant(entry) => {
                                            entry.insert(customer);
                                            state.cache_size.inc();
                                        }
                                        Entry::Occupied(mut entry) => {
                                            entry.insert(customer);
                                        }
                                    }
                                }
                                Err(e) => error!("Failed to convert Avro value to Customer: {}", e),
                            },
                            Err(e) => error!("Failed to deserialize Avro datum: {}", e),
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
                            info!("Removed record for key: {}", key_str);
                        }
                    }
                }
            }
            Ok(())
        }
    });

    info!("Starting event loop");
    match stream_processor.await {
        Ok(_) => info!("Stream processing terminated"),
        Err(e) => error!("Stream processing failed: {}", e),
    }
}
