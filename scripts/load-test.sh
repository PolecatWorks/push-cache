#!/bin/bash
set -e

# Configuration
DURATION=${DURATION:="30s"}
CONCURRENCY=${CONCURRENCY:="100"}
BASE_URL=${BASE_URL:="http://127.0.0.1:8080/cache/customers"}
SAMPLE_SIZE=${SAMPLE_SIZE:-100}
URLS_FILE=$(mktemp)

cleanup() {
    rm -f "$URLS_FILE"
}
trap cleanup EXIT

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

# 2. Fetch Keys
echo "Fetching keys from $BASE_URL/..."

# Fetch the list of keys (assumes JSON array of strings: ["key1", "key2", ...])
# We use python3 to parse because jq might not be available, matching the constraint of minimal dependencies if possible,
# but python3 is standard on mac.
KEYS_JSON=$(curl -s "$BASE_URL")

if [ -z "$KEYS_JSON" ]; then
    echo "Failed to fetch keys from $BASE_URL. Is the server running?"
    exit 1
fi

# Generate URLs file using python
# Sample SAMPLE_SIZE keys randomly if available, otherwise all keys.
echo "$KEYS_JSON" | python3 -c "
import sys, json, random
try:
    keys = json.load(sys.stdin)
    if keys:
        sample_size = min(len(keys), int('$SAMPLE_SIZE'))
        sampled_keys = random.sample(keys, sample_size)
        for key in sampled_keys:
            print(f'$BASE_URL/{key}')
except json.JSONDecodeError:
    pass # Handle invalid JSON gracefully (empty file result check below catches this)
" > "$URLS_FILE"

if [ ! -s "$URLS_FILE" ]; then
    echo "No keys found in the response or failed to parse keys. Please ensure the server is running and populated."
    # Helpful hint if curl returned empty body or error page
    if [ ${#KEYS_JSON} -lt 100 ]; then
        echo "Response was: $KEYS_JSON"
    fi
    exit 1
fi

COUNT=$(wc -l < "$URLS_FILE" | tr -d ' ')
echo "Generated $COUNT URLs for load testing."

# 3. Run Load Test
echo "Running Load Test using URLs from file"
echo "Concurrency: $CONCURRENCY, Duration: $DURATION, Sample Size: $SAMPLE_SIZE"

oha -c $CONCURRENCY -z $DURATION --urls-from-file "$URLS_FILE"

echo "Load Test Complete."
