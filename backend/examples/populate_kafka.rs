use apache_avro::AvroSchema;
use clap::{Parser, ValueEnum};
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
    #[arg(short = 'n', long, default_value_t = 100)]
    count: usize,

    /// Config file
    #[arg(short = 'c', long, value_name = "FILE")]
    config: PathBuf,

    /// Secrets dir
    #[arg(short, long, value_name = "DIR", default_value = "secrets")]
    secrets: PathBuf,

    /// Message type to produce
    #[arg(short, long, value_enum, default_value_t = MessageType::Customer, env = "MESSAGE_TYPE")]
    message_type: MessageType,

    /// Kafka Topic (overrides config)
    #[arg(short, long, env = "KAFKA_TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "lowercase")]
enum MessageType {
    Customer,
    Bill,
    Usage,
    Ticket,
}

/// Represents a payment transaction.
///
/// This structure models a single payment made by a customer, including
/// the payment date, amount, and payment method.
#[derive(Debug, Serialize, Deserialize, AvroSchema, Clone)]
#[avro(namespace = "com.polecatworks.billing")]
#[allow(non_snake_case)]
pub struct Payment {
    pub date: String,
    pub amount: f64,
    pub method: String,
}

/// Represents a customer billing statement.
///
/// This structure aggregates billing information for a customer account,
/// including the total amount due and a list of payments made.
#[derive(Debug, Serialize, Deserialize, AvroSchema, Clone)]
#[avro(namespace = "com.polecatworks.billing")]
#[allow(non_snake_case)]
pub struct CustomerBill {
    pub accountId: String,
    pub year: i32,
    pub totalAmount: f64,
    pub payments: Vec<Payment>,
}

/// Represents a service usage record.
///
/// This structure tracks customer usage of a particular service,
/// including the service type, amount consumed, unit of measurement,
/// and timestamp of the usage.
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

/// Represents a customer support ticket.
///
/// This structure models a support ticket raised by a customer,
/// including the ticket ID, associated account, issue description,
/// current status, and creation timestamp.
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
        "Producing {} records of type '{:?}' to topic {}",
        args.count, args.message_type, topic
    );

    let result = match args.message_type {
        MessageType::Customer => {
            produce_records::<Customer, _>(&config, topic, args.count, manual_fake_customer).await
        }
        MessageType::Bill => {
            produce_records::<CustomerBill, _>(&config, topic, args.count, manual_fake_bill).await
        }
        MessageType::Usage => {
            produce_records::<UsageRecord, _>(&config, topic, args.count, manual_fake_usage).await
        }
        MessageType::Ticket => {
            produce_records::<SupportTicket, _>(&config, topic, args.count, manual_fake_ticket)
                .await
        }
    };

    if let Err(e) = result {
        error!("Error producing records: {:?}", e);
    }
}

/// Produces a specified number of Avro-encoded records to a Kafka topic.
///
/// This generic function handles the complete workflow of:
/// 1. Creating a Kafka producer
/// 2. Registering the schema with Schema Registry
/// 3. Generating and encoding records using the provided generator function
/// 4. Publishing records to Kafka with proper Avro framing (magic byte + schema ID)
/// 5. Tracking and reporting statistics about the generated messages
///
/// # Type Parameters
///
/// * `T` - The message type to produce. Must implement `AvroSchema`, `Serialize`, `Clone`, and `Debug`.
/// * `F` - A function that generates instances of type `T`.
///
/// # Arguments
///
/// * `config` - Application configuration containing Kafka broker and Schema Registry details.
/// * `topic` - The Kafka topic name to publish messages to.
/// * `count` - The number of records to generate and publish.
/// * `generator` - A function that generates a single record of type `T`.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if:
/// - The Kafka producer cannot be created
/// - Schema registration fails
/// - Record encoding fails
/// - Message publishing fails
///
/// # Statistics
///
/// After completion, logs the following statistics:
/// - Total size of all Avro payloads (bytes)
/// - Maximum record size (bytes)
/// - Minimum record size (bytes)
/// - Average record size (bytes)
///
/// # Examples
///
/// ```no_run
/// use push_cache::model::Customer;
/// use push_cache::config::MyConfig;
///
/// async fn example(config: &MyConfig) {
///     produce_records::<Customer, _>(
///         config,
///         "my-topic",
///         100,
///         || generate_fake_customer()
///     ).await.expect("Failed to produce records");
/// }
/// ```
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
                FutureRecord::to(topic).payload(&payload).key(&key),
                Duration::from_secs(0),
            )
            .await;

        if i == 0 {
            println!("Serialized Key: {}", key);
        }

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

/// Generates a fake `Customer` record using the `fake` crate.
///
/// Creates a realistic-looking customer with randomly generated:
/// - Name
/// - Address (street and city)
/// - Phone number
/// - Unique account ID (UUID)
/// - Creation and update timestamps (current time)
///
/// # Returns
///
/// A `Customer` instance with randomly generated data.
///
/// # Examples
///
/// ```
/// let customer = manual_fake_customer();
/// println!("Generated customer: {}", customer.name);
/// ```
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

/// Generates a fake `CustomerBill` record with multiple payments.
///
/// Creates a billing statement with:
/// - Random number of payments (1-11)
/// - Each payment has a random amount between $10 and $500
/// - Total amount is the sum of all payments
/// - Current date for payment timestamps
/// - Random payment methods
///
/// # Returns
///
/// A `CustomerBill` instance with randomly generated payment data.
///
/// # Examples
///
/// ```
/// let bill = manual_fake_bill();
/// println!("Total amount: ${}", bill.totalAmount);
/// println!("Number of payments: {}", bill.payments.len());
/// ```
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

/// Generates a fake `UsageRecord` for service usage tracking.
///
/// Creates a usage record with:
/// - Random account ID (UUID)
/// - Random service type
/// - Random usage amount between 1.0 and 100.0
/// - Unit set to "GB"
/// - Current timestamp
///
/// # Returns
///
/// A `UsageRecord` instance with randomly generated usage data.
///
/// # Examples
///
/// ```
/// let usage = manual_fake_usage();
/// println!("Service: {}, Amount: {} {}", usage.serviceType, usage.amount, usage.unit);
/// ```
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

/// Generates a fake `SupportTicket` for customer support tracking.
///
/// Creates a support ticket with:
/// - Random ticket ID (UUID)
/// - Random account ID (UUID)
/// - Random issue description (5-10 word sentence)
/// - Random status
/// - Current timestamp
///
/// # Returns
///
/// A `SupportTicket` instance with randomly generated ticket data.
///
/// # Examples
///
/// ```
/// let ticket = manual_fake_ticket();
/// println!("Ticket {}: {}", ticket.ticketId, ticket.issue);
/// ```
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
