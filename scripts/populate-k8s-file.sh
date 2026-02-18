#!/bin/bash
set -e

NAMESPACE="dev"
POD_NAME="populate-kafka-script"
BACKEND_DIR="backend"
BINARY_PATH="$BACKEND_DIR/target/release/examples/populate_kafka"
DATA_FILE="data.b64"

# Kafka Configs (internal K8s DNS)
BROKER_ADDRESS="kafka.confluent.svc:9092"
TOPIC="pcache-data"
COUNT=100
MESSAGE_TYPE="${MESSAGE_TYPE:-customer}"
MESSAGE_TYPE="${MESSAGE_TYPE:-customer}"

if [ -z "$SCHEMA_ID" ]; then
    echo "Error: SCHEMA_ID environment variable is required."
    echo "Usage: SCHEMA_ID=123 ./scripts/populate-k8s-file.sh"
    exit 1
fi

# Check for local binary
if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Binary not found at $BINARY_PATH"
    echo "Please run: cargo build --release --example populate_kafka"
    exit 1
fi

echo "Generating data locally..."
echo "Message Type: ${MESSAGE_TYPE}"
echo "Topic: ${TOPIC}"
echo "Count: ${COUNT}"
echo "Schema ID: ${SCHEMA_ID}"

# Generate data to file
$BINARY_PATH \
    --output-file $DATA_FILE \
    --schema-id $SCHEMA_ID \
    --count $COUNT \
    --message-type $MESSAGE_TYPE \
    --config /dev/null \
    --secrets /dev/null

echo "Compressing data..."
gzip -f $DATA_FILE

# Cleanup previous run if exists
if kubectl get pod $POD_NAME -n $NAMESPACE > /dev/null 2>&1; then
    echo "Cleaning up existing pod $POD_NAME..."
    kubectl delete pod $POD_NAME -n $NAMESPACE --force --grace-period=0
    sleep 2
fi

echo "Launching temporary pod $POD_NAME..."
# Use an image that likely has basic shell tools.
# Attempting to use a standard image, we might need to install deps.
kubectl run $POD_NAME -n $NAMESPACE --image=debian:bookworm-slim --restart=Never -- sleep infinity

echo "Waiting for pod to be ready..."
kubectl wait --for=condition=Ready pod/$POD_NAME -n $NAMESPACE --timeout=180s

echo "Copying compressed data..."
kubectl cp ${DATA_FILE}.gz $NAMESPACE/$POD_NAME:/tmp/${DATA_FILE}.gz

echo "Installing runtime dependencies (kcat)..."
# Using kafkacat (kcat) - likely need to install it
kubectl exec -n $NAMESPACE $POD_NAME -- apt-get update -qq
kubectl exec -n $NAMESPACE $POD_NAME -- apt-get install -y -qq kcat ca-certificates

echo "Pushing data to Kafka..."
kubectl exec -n $NAMESPACE $POD_NAME -- bash -c "
    zcat /tmp/${DATA_FILE}.gz | while read line; do
      KEY=\$(echo \$line | cut -d'|' -f1)
      VALUE_B64=\$(echo \$line | cut -d'|' -f2)

      echo \"\$VALUE_B64\" | base64 -d | kcat -b $BROKER_ADDRESS \
        -t $TOPIC \
        -K \"|\" \
        -P -k \"\$KEY\"
    done
"

echo "Cleaning up..."
kubectl delete pod $POD_NAME -n $NAMESPACE
rm ${DATA_FILE}.gz

echo "Done!"
