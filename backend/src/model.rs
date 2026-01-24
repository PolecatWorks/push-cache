use apache_avro::AvroSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, AvroSchema, Clone)]
#[avro(namespace = "com.polecatworks.billing")]
#[allow(non_snake_case)]
pub struct Customer {
    pub accountId: String,
    pub name: String,
    pub address: String,
    pub phone: String,
    pub createdAt: i64,
    pub updatedAt: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use apache_avro::{from_value, to_value};

    #[test]
    fn test_customer_avro_serialization() {
        let customer = Customer {
            accountId: "123".to_string(),
            name: "John Doe".to_string(),
            address: "123 Main St".to_string(),
            phone: "555-1234".to_string(),
            createdAt: 1000,
            updatedAt: 2000,
        };

        let _schema = Customer::get_schema();
        let value = to_value(&customer).expect("Failed to convert to value");
        let result = from_value::<Customer>(&value).expect("Failed to deserialize");

        assert_eq!(result.accountId, "123");
        assert_eq!(result.name, "John Doe");
        assert_eq!(result.address, "123 Main St");
    }
}
