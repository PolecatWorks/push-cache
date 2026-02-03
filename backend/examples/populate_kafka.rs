use apache_avro::AvroSchema;
use clap::Parser;
use fake::Fake;
use push_cache::config::MyConfig;
use push_cache::model::Customer;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, error, info};
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

    /// Message type to produce (customer, bill, usage, ticket)
    #[arg(short, long, default_value = "customer")]
    message_type: String,

    /// Kafka Topic (overrides config)
    #[arg(short, long)]
    topic: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, AvroSchema, Clone)]
#[avro(namespace = "com.polecatworks.billing")]
#[allow(non_snake_case)]
pub struct Payment {
    pub date: String,
    pub amount: f64,
    pub method: String,
}

#[derive(Debug, Serialize, Deserialize, AvroSchema, Clone)]
#[avro(namespace = "com.polecatworks.billing")]
#[allow(non_snake_case)]
pub struct CustomerBill {
    pub accountId: String,
    pub year: i32,
    pub totalAmount: f64,
    pub payments: Vec<Payment>,
}

#[derive(Debug, Serialize, Deserialize, AvroSchema, Clone)]
#[avro(namespace = "com.polecatworks.billing")]
#[allow(non_snake_case)]
pub struct UsageRecord {
    pub accountId: String,
    pub serviceType: String,
    pub amount: f64,
    pub unit: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, AvroSchema, Clone)]
#[avro(namespace = "com.polecatworks.billing")]
#[allow(non_snake_case)]
pub struct SupportTicket {
    pub ticketId: String,
    pub accountId: String,
    pub issue: String,
    pub status: String,
    pub timestamp: i64,
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

    let topic = args.topic.as_deref().unwrap_or(&config.kafka.topic);

    info!(
        "Producing {} records of type '{}' to topic {}",
        args.count, args.message_type, topic
    );

    let result = match args.message_type.as_str() {
        "customer" => {
            produce_records::<Customer, _>(&config, topic, args.count, manual_fake_customer).await
        }
        "bill" => {
            produce_records::<CustomerBill, _>(&config, topic, args.count, manual_fake_bill).await
        }
        "usage" => {
            produce_records::<UsageRecord, _>(&config, topic, args.count, manual_fake_usage).await
        }
        "ticket" => {
            produce_records::<SupportTicket, _>(&config, topic, args.count, manual_fake_ticket)
                .await
        }
        _ => {
            error!("Unknown message type: {}", args.message_type);
            return;
        }
    };

    if let Err(e) = result {
        error!("Error producing records: {:?}", e);
    }
}

async fn produce_records<T, F>(
    config: &MyConfig,
    topic: &str,
    count: usize,
    generator: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: AvroSchema + Serialize + Clone + std::fmt::Debug,
    F: Fn() -> T,
{
    let producer: FutureProducer = ClientConfig::new()
        .set(
            "bootstrap.servers",
            push_cache::kafka_utils::get_broker_string(&config.kafka)?,
        )
        .set("message.timeout.ms", "5000")
        .create()?;

    // Register Schema
    let registry_url = config
        .kafka
        .schema_registry_url
        .as_str()
        .trim_end_matches('/');
    let (schema_id, schema) =
        push_cache::kafka_utils::get_schema_id::<T>(registry_url, topic).await?;
    info!("Registered/Fetched Schema ID: {}", schema_id);

    let mut total_size = 0;
    let mut max_size = 0;
    let mut min_size = usize::MAX;

    for i in 0..count {
        let record = generator();

        let encoded = apache_avro::to_avro_datum(&schema, apache_avro::to_value(record.clone())?)?;
        let size = encoded.len();

        total_size += size;
        if size > max_size {
            max_size = size;
        }
        if size < min_size {
            min_size = size;
        }

        // Magic Byte + ID + Payload
        let mut payload = vec![0u8];
        payload.extend_from_slice(&schema_id.to_be_bytes());
        payload.extend_from_slice(&encoded);

        // Use random key
        let key = uuid::Uuid::new_v4().to_string();

        let _ = producer
            .send(
                FutureRecord::to(topic)
                    .payload(&payload)
                    .key(&key),
                Duration::from_secs(0),
            )
            .await;

        if (i + 1) % 100 == 0 {
            debug!("Produced {}/{}", i + 1, count);
        }
    }

    if count > 0 {
        info!("Production Complete.");
        info!("Total Size (Avro payload only): {} bytes", total_size);
        info!("Max Record Size: {} bytes", max_size);
        info!("Min Record Size: {} bytes", min_size);
        info!(
            "Average Record Size: {} bytes",
            total_size as f64 / count as f64
        );
    }

    Ok(())
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

fn manual_fake_bill() -> CustomerBill {
    use fake::faker::lorem::en::Word;
    // Generate payments
    let num_payments = (1..12).fake::<usize>();
    let mut payments = Vec::new();
    let mut total = 0.0;

    for _ in 0..num_payments {
        let amount = (10.0..500.0).fake::<f64>();
        total += amount;
        payments.push(Payment {
            date: chrono::Utc::now().to_rfc3339(),
            amount,
            method: Word().fake(),
        });
    }

    CustomerBill {
        accountId: uuid::Uuid::new_v4().to_string(),
        year: 2024,
        totalAmount: total,
        payments,
    }
}

fn manual_fake_usage() -> UsageRecord {
    use fake::faker::lorem::en::Word;
    UsageRecord {
        accountId: uuid::Uuid::new_v4().to_string(),
        serviceType: Word().fake(),
        amount: (1.0..100.0).fake(),
        unit: "GB".to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

fn manual_fake_ticket() -> SupportTicket {
    use fake::faker::lorem::en::{Sentence, Word};
    SupportTicket {
        ticketId: uuid::Uuid::new_v4().to_string(),
        accountId: uuid::Uuid::new_v4().to_string(),
        issue: Sentence(5..10).fake(),
        status: Word().fake(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    }
}
