#!/bin/bash
set -e

# Configuration
CONFIG_FILE="rust-container/test-data/config-localhost.yaml"
SECRETS_DIR="rust-container/test-data/secrets" # Assuming secrets might be here or not needed if config doesn't use them (localhost usually doesn't need real secrets)

# Aligned with populate-k8s.sh
MESSAGE_TYPE="${MESSAGE_TYPE:-customer}"
TOPIC="${TOPIC:-pcache-data}"
COUNT="${COUNT:-100}"

# 0. Ensure local cargo bin is in PATH
export PATH="$HOME/.cargo/bin:$PATH"

# 1. Populate Kafka
echo "Populating Kafka with $COUNT records of type '$MESSAGE_TYPE' to topic '$TOPIC'..."

# Create a dummy secrets dir if missing to satisfy the arg
mkdir -p "$SECRETS_DIR"

POPULATE_OUTPUT=$(RUST_LOG=warn cargo run --manifest-path rust-container/Cargo.toml --release --example populate_kafka -- \
    --count $COUNT \
    --config "$CONFIG_FILE" \
    --secrets "$SECRETS_DIR" \
    --message-type "$MESSAGE_TYPE" \
    --topic "$TOPIC")

# Print output for visibility (logs go to stderr automatically, this prints stdout including our key)
echo "$POPULATE_OUTPUT"

# Extract Key (optional for this script, but good confirmation)
GENERATED_KEY=$(echo "$POPULATE_OUTPUT" | grep "Produced AccountID:" | tail -n 1 | awk '{print $3}')

if [ -n "$GENERATED_KEY" ]; then
    echo "Last Produced Key: $GENERATED_KEY"
else
    echo "Warning: Could not capture a produced key from output. Check logs."
fi

echo "Done!"
echo "Summary:"
echo "  - Message Type: ${MESSAGE_TYPE}"
echo "  - Topic: ${TOPIC}"
echo "  - Records: ${COUNT}"
