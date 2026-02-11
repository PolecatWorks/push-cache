#!/bin/bash
set -e

# Configuration
DURATION="30s"
CONCURRENCY=100
BASE_URL="http://localhost:8080/cache/users"


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

# 2. Fetch an ID
echo "Fetching a valid ID from $BASE_URL/..."

# Fetch the list of keys (assumes JSON array of strings: ["key1", "key2", ...])
# We use python3 to parse because jq might not be available, matching the constraint of minimal dependencies if possible,
# but python3 is standard on mac.
KEYS_JSON=$(curl -s "$BASE_URL")

if [ -z "$KEYS_JSON" ]; then
    echo "Failed to fetch keys from $BASE_URL. Is the server running?"
    exit 1
fi

# Extract the first key using python
TARGET_KEY=$(echo "$KEYS_JSON" | python3 -c "import sys, json; keys = json.load(sys.stdin); print(keys[0] if keys else '')")

if [ -z "$TARGET_KEY" ]; then
    echo "No keys found in the response. Please populate data first using ./scripts/populate-local.sh"
    exit 1
fi

echo "Found Key: $TARGET_KEY"

# 3. Run Load Test
echo "Running Load Test against $BASE_URL/$TARGET_KEY"
echo "Concurrency: $CONCURRENCY, Duration: $DURATION"

oha -c $CONCURRENCY -z $DURATION "$BASE_URL/$TARGET_KEY"

echo "Load Test Complete."
