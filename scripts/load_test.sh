#!/bin/bash
set -e

# Configuration
CONFIG_FILE="backend/test-data/config-localhost.yaml"
SECRETS_DIR="backend/test-data/secrets" # Assuming secrets might be here or not needed if config doesn't use them (localhost usually doesn't need real secrets)
RECORD_COUNT=1000
DURATION="30s"
CONCURRENCY=100

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
echo "Populating Kafka with $RECORD_COUNT records..."
# Create a dummy secrets dir if missing to satisfy the arg
mkdir -p "$SECRETS_DIR"
RUST_LOG=warn cargo run --manifest-path backend/Cargo.toml --release --example populate_kafka -- \
    --count $RECORD_COUNT \
    --config "$CONFIG_FILE" \
    --secrets "$SECRETS_DIR"

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
# We need a valid account ID. Since populate generates random IDs, we can't easily guess one.
# But `populate_kafka` prints "Produced AccountID: <id>". We didn't capture it.
# Alternative: Query /api/users (list) to get an ID.
echo "Fetching a valid user ID..."
USER_ID=$(curl -s "http://localhost:8080/cache/users?limit=1" | grep -o '"[^"]*"' | head -n 1 | tr -d '"')

if [ -z "$USER_ID" ]; then
    echo "Could not find any users in cache. Did population fail?"
    exit 1
fi

echo "Running Load Test against /users/$USER_ID"
echo "Concurrency: $CONCURRENCY, Duration: $DURATION"

oha -c $CONCURRENCY -z $DURATION "http://localhost:8080/cache/users/$USER_ID"

echo "Load Test Complete."
