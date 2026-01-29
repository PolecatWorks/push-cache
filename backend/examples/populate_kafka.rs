use apache_avro::AvroSchema;
use clap::Parser;
use fake::Fake;
use push_cache::config::MyConfig;
use push_cache::model::Customer;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Number of records to produce
    #[arg(short, long, default_value_t = 100)]
    count: usize,

    /// Config file
    #[arg(short, long, value_name = "FILE")]
    config: PathBuf,

    /// Secrets dir
    #[arg(short, long, value_name = "DIR", default_value = "secrets")]
    secrets: PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    // Initialize logger
    let env = EnvFilter::builder()
        .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
        .with_env_var("CAPTURE_LOG")
        .from_env()
        .expect("Failed to load env filter");
    tracing_subscriber::fmt().with_env_filter(env).init();

    // Load Config
    let config_yaml = std::fs::read_to_string(&args.config).expect("Failed to read config");
    let config: MyConfig = MyConfig::figment(&config_yaml, args.secrets)
        .extract()
        .expect("Failed to load config");

    let producer: FutureProducer = ClientConfig::new()
        .set(
            "bootstrap.servers",
            push_cache::kafka_utils::get_broker_string(&config.kafka)
                .expect("Failed to get broker string"),
        )
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    let schema = Customer::get_schema();
    info!("Schema: {:?}", schema);

    info!(
        "Producing {} records to topic {}",
        args.count, config.kafka.topic
    );

    for _ in 0..args.count {
        let customer = manual_fake_customer();

        // Serialize to Avro bytes
        let encoded =
            apache_avro::to_avro_datum(&schema, apache_avro::to_value(customer.clone()).unwrap())
                .unwrap();

        // Add Confluent Magic Byte (0) + Schema ID (4 bytes)
        // Using ID 1 for testing
        let mut payload = vec![0u8, 0, 0, 0, 1];
        payload.extend_from_slice(&encoded);

        // Send to Kafka
        let _ = producer
            .send(
                FutureRecord::to(&config.kafka.topic)
                    .payload(&payload)
                    .key(&customer.accountId),
                Duration::from_secs(0),
            )
            .await;
        debug!("Produced AccountID: {}", customer.accountId);
    }

    info!(
        "Produced {} records to topic {}",
        args.count, config.kafka.topic
    );
}

fn manual_fake_customer() -> Customer {
    use chrono::Utc;
    use fake::faker::address::en::{CityName, StreetName};
    use fake::faker::name::en::Name;
    use fake::faker::phone_number::en::PhoneNumber;

    let name: String = Name().fake();
    let city: String = CityName().fake();
    let street: String = StreetName().fake();
    let address = format!("{}, {}", street, city);
    let phone: String = PhoneNumber().fake();
    let account_id = uuid::Uuid::new_v4().to_string();
    let created_at: i64 = Utc::now().timestamp_millis();
    let updated_at: i64 = Utc::now().timestamp_millis();

    Customer {
        accountId: account_id,
        name,
        address,
        phone,
        createdAt: created_at,
        updatedAt: updated_at,
    }
}
