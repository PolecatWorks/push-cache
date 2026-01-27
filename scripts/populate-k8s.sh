#!/bin/bash
set -e

NAMESPACE="dev"
POD_NAME="populate-kafka-script"
IMAGE="python:3.13.0-slim-bookworm"
PYTHON_SCRIPT="backend/test-data/populate_kafka.py"
REMOTE_SCRIPT="/tmp/populate_kafka.py"

# Kafka and Schema Registry Configs (internal K8s DNS)
BOOTSTRAP_SERVERS="kafka.confluent.svc:9092"
SCHEMA_REGISTRY="http://schemaregistry.confluent.svc:8081"
TOPIC="pcache-users"
COUNT=100

echo "Starting Kafka population script..."

# Cleanup previous run if exists
if kubectl get pod $POD_NAME -n $NAMESPACE > /dev/null 2>&1; then
    echo "Cleaning up existing pod $POD_NAME..."
    kubectl delete pod $POD_NAME -n $NAMESPACE --force --grace-period=0
fi

echo "Launching temporary pod $POD_NAME in namespace $NAMESPACE..."
kubectl run $POD_NAME -n $NAMESPACE --image=$IMAGE --restart=Never -- sleep infinity

echo "Waiting for pod to be ready..."
kubectl wait --for=condition=Ready pod/$POD_NAME -n $NAMESPACE --timeout=60s

echo "Copying population script..."
kubectl cp $PYTHON_SCRIPT $NAMESPACE/$POD_NAME:$REMOTE_SCRIPT

echo "Installing dependencies..."
kubectl exec -n $NAMESPACE $POD_NAME -- pip install confluent-kafka faker click requests certifi httpx Authlib fastavro jsonschema cachetools attrs

echo "Executing population script..."
kubectl exec -n $NAMESPACE $POD_NAME -- python $REMOTE_SCRIPT \
    --bootstrap-servers $BOOTSTRAP_SERVERS \
    --schema-registry $SCHEMA_REGISTRY \
    --topic $TOPIC \
    --count $COUNT

echo "Cleaning up..."
kubectl delete pod $POD_NAME -n $NAMESPACE

echo "Done!"
