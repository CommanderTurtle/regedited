#!/bin/bash
# Initialize a new Regedited document
# Usage: regedited_init.sh <filename> <title>

set -e

FILE="${1:-document.md}"
TITLE="${2:-Regedited Document}"

if [ -f "$FILE" ]; then
    echo "Error: File already exists: $FILE"
    exit 1
fi

regedited new "$FILE" "$TITLE"
echo "Created: $FILE"
