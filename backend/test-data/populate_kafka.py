import click
import uuid
from faker import Faker
from confluent_kafka import Producer
from confluent_kafka.serialization import SerializationContext, MessageField
from confluent_kafka.schema_registry import SchemaRegistryClient
from confluent_kafka.schema_registry.avro import AvroSerializer
from datetime import datetime, timezone

# Define schema to match Rust struct
SCHEMA_STR = """
{
    "type": "record",
    "name": "Customer",
    "namespace": "com.polecatworks.billing",
    "fields": [
        {"name": "accountId", "type": "string"},
        {"name": "name", "type": "string"},
        {"name": "address", "type": "string"},
        {"name": "phone", "type": "string"},
        {"name": "createdAt", "type": "long"},
        {"name": "updatedAt", "type": "long"}
    ]
}
"""

class Customer(object):
    def __init__(self, account_id, name, address, phone, created_at, updated_at):
        self.accountId = account_id
        self.name = name
        self.address = address
        self.phone = phone
        self.createdAt = created_at
        self.updatedAt = updated_at

def customer_to_dict(customer, ctx):
    return {
        "accountId": customer.accountId,
        "name": customer.name,
        "address": customer.address,
        "phone": customer.phone,
        "createdAt": customer.createdAt,
        "updatedAt": customer.updatedAt
    }

@click.command()
@click.option('--bootstrap-servers', default="localhost:9092", help="Kafka Bootstrap Servers")
@click.option('--schema-registry', default="http://localhost:8081", help="Schema Registry URL")
@click.option('--topic', default="push-cache-users", help="Kafka Topic")
@click.option('--count', default=1, help="Number of records to produce")
def main(bootstrap_servers, schema_registry, topic, count):
    conf = {'bootstrap.servers': bootstrap_servers}
    schema_registry_conf = {'url': schema_registry}

    schema_registry_client = SchemaRegistryClient(schema_registry_conf)
    avro_serializer = AvroSerializer(schema_registry_client,
                                     SCHEMA_STR,
                                     customer_to_dict)

    producer = Producer(conf)
    fake = Faker()

    click.echo(f"Producing {count} records to topic {topic}")

    for _ in range(count):
        account_id = str(uuid.uuid4())
        now_ts = int(datetime.now(timezone.utc).timestamp() * 1000)

        customer = Customer(
            account_id=account_id,
            name=fake.name(),
            address=fake.address().replace('\n', ', '),
            phone=fake.phone_number(),
            created_at=now_ts,
            updated_at=now_ts
        )

        producer.produce(topic=topic,
                         key=account_id,
                         value=avro_serializer(customer, SerializationContext(topic, MessageField.VALUE)))

        click.echo(f"Produced AccountID: {account_id}")
        producer.poll(0)

    producer.flush()
    click.echo("Done!")

if __name__ == '__main__':
    main()
