#!/bin/bash
set -e

NAMESPACE="dev"
POD_NAME="populate-kafka-script"
# Use Rust builder image
IMAGE="rust:1.75-slim"
BACKEND_DIR="rust-container"
SCRIPT_NAME="populate_kafka"

# Kafka and Schema Registry Configs (internal K8s DNS)
BOOTSTRAP_SERVERS="kafka.confluent.svc:9092"
SCHEMA_REGISTRY="http://schemaregistry.confluent.svc:8081"
TOPIC="pcache-data"
COUNT=100
MESSAGE_TYPE="${MESSAGE_TYPE:-customer}"  # Default to customer, override with env var

echo "Starting Kafka population script (Rust version)..."
echo "Message Type: ${MESSAGE_TYPE}"
echo "Topic: ${TOPIC}"
echo "Count: ${COUNT}"

# Cleanup previous run if exists
if kubectl get pod $POD_NAME -n $NAMESPACE > /dev/null 2>&1; then
    echo "Cleaning up existing pod $POD_NAME..."
    kubectl delete pod $POD_NAME -n $NAMESPACE --force --grace-period=0
    sleep 2
fi

echo "Launching temporary pod $POD_NAME in namespace $NAMESPACE..."
kubectl run $POD_NAME -n $NAMESPACE --image=$IMAGE --restart=Never -- sleep infinity

echo "Waiting for pod to be ready..."
kubectl wait --for=condition=Ready pod/$POD_NAME -n $NAMESPACE --timeout=90s

echo "Copying backend source code..."
kubectl cp $BACKEND_DIR $NAMESPACE/$POD_NAME:/tmp/rust-container

echo "Installing system dependencies..."
kubectl exec -n $NAMESPACE $POD_NAME -- apt-get update -qq
kubectl exec -n $NAMESPACE $POD_NAME -- apt-get install -y -qq pkg-config libssl-dev

# Create a minimal config for K8s environment
echo "Creating config file..."
kubectl exec -n $NAMESPACE $POD_NAME -- bash -c "cat > /tmp/config.yaml << 'EOF'
kafka:
  brokers: ${BOOTSTRAP_SERVERS}
  group_id: populate-kafka-group
  topic: ${TOPIC}
  schema_registry_url: ${SCHEMA_REGISTRY}
  cache_max_age: 60s
  fetch_metadata_timeout: 5s
  offset_reset: earliest
  force_reset_earliest: false
webservice:
  address: 0.0.0.0:8080
  prefix: /api
  forwarding_headers: []
hams:
  address: 0.0.0.0:8079
  prefix: hams
  logging: true
  checks:
    timeout: 5
    fails: 2
    preflights: []
    shutdowns: []
runtime:
  threads: 4
  stack_size: 3145728
  name: populate-kafka
startup_checks:
  fails: 2
  timeout: 5s
  enabled: false
EOF
"

echo "Creating empty secrets directory..."
kubectl exec -n $NAMESPACE $POD_NAME -- mkdir -p /tmp/secrets

echo "Building populate_kafka example..."
kubectl exec -n $NAMESPACE $POD_NAME -- bash -c "cd /tmp/rust-container && cargo build --release --example populate_kafka"

echo "Executing population script..."
kubectl exec -n $NAMESPACE $POD_NAME -- bash -c "cd /tmp/rust-container && ./target/release/examples/populate_kafka \
    --config /tmp/config.yaml \
    --secrets /tmp/secrets \
    --message-type ${MESSAGE_TYPE} \
    --topic ${TOPIC} \
    --count ${COUNT}"

echo "Cleaning up..."
kubectl delete pod $POD_NAME -n $NAMESPACE

echo ""
echo "Done!"
echo "Summary:"
echo "  - Message Type: ${MESSAGE_TYPE}"
echo "  - Topic: ${TOPIC}"
echo "  - Records: ${COUNT}"
echo ""
echo "To use different message types, set MESSAGE_TYPE environment variable:"
echo "  MESSAGE_TYPE=bill ./scripts/populate-k8s.sh"
echo "  MESSAGE_TYPE=usage ./scripts/populate-k8s.sh"
echo "  MESSAGE_TYPE=ticket ./scripts/populate-k8s.sh"
