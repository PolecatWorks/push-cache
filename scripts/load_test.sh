#!/bin/bash
set -e

# Configuration
CONFIG_FILE="backend/test-data/config-localhost.yaml"
SECRETS_DIR="backend/test-data/secrets" # Assuming secrets might be here or not needed if config doesn't use them (localhost usually doesn't need real secrets)
RECORD_COUNT=1000
DURATION="30s"
CONCURRENCY=100
MESSAGE_TYPE="${MESSAGE_TYPE:-customer}"

# 0. Ensure local cargo bin is in PATH
export PATH="$HOME/.cargo/bin:$PATH"

# 1. Check/Install oha
if ! command -v oha &> /dev/null; then
    echo "oha could not be found."
    read -p "Do you want to install oha via cargo? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        cargo install oha
    else
        echo "Please install oha manually or run this script again."
        exit 1
    fi
fi

# # 2. Cleanup previous runs
# echo "Cleaning up..."
# pkill -f "target/release/push-cache" || true

# 3. Populate Kafka
# 3. Populate Kafka
echo "Populating Kafka with $RECORD_COUNT records of type '$MESSAGE_TYPE'..."
# Create a dummy secrets dir if missing to satisfy the arg
mkdir -p "$SECRETS_DIR"
POPULATE_OUTPUT=$(RUST_LOG=warn cargo run --manifest-path backend/Cargo.toml --release --example populate_kafka -- \
    --count $RECORD_COUNT \
    --config "$CONFIG_FILE" \
    --secrets "$SECRETS_DIR" \
    --message-type "$MESSAGE_TYPE")

# Print output for visibility (logs go to stderr automatically, this prints stdout including our key)
echo "$POPULATE_OUTPUT"

# Extract Key
GENERATED_KEY=$(echo "$POPULATE_OUTPUT" | grep "Serialized Key:" | head -n 1 | awk '{print $3}')

if [ -z "$GENERATED_KEY" ]; then
    echo "Failed to capture generated key. Population might have failed."
    exit 1
fi

echo "Captured Key: $GENERATED_KEY"

# # 4. Start Backend
# echo "Starting Backend..."
# cargo run --manifest-path backend/Cargo.toml --release --bin push-cache -- \
#     --config "$CONFIG_FILE" \
#     --secrets "$SECRETS_DIR" &
# SERVER_PID=$!

# Ensure cleanup on exit
# trap "kill $SERVER_PID" EXIT

# 5. Wait for Health
echo "Waiting for server to be healthy..."
max_retries=30
count=0
while ! curl -s http://localhost:8080/cache/hello > /dev/null; do
    sleep 1
    count=$((count+1))
    if [ $count -ge $max_retries ]; then
        echo "Server failed to start."
        exit 1
    fi
done
echo "Server is UP!"

# 6. Run Load Test
echo "Running Load Test against /dynamic/$GENERATED_KEY"
echo "Concurrency: $CONCURRENCY, Duration: $DURATION"

# Use the dynamic endpoint which supports all message types
oha -c $CONCURRENCY -z $DURATION "http://localhost:8080/dynamic/$GENERATED_KEY"

echo "Load Test Complete."
