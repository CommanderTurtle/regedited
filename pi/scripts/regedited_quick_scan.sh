#!/bin/bash
# Quick scan with human-readable summary
# Usage: regedited_quick_scan.sh <file> [filter]

set -e

FILE="${1:?Usage: regedited_quick_scan.sh <file> [filter]}"
FILTER="${2:-}"

echo "=== Regedited Quick Scan: $FILE ==="
echo ""

if [ -n "$FILTER" ]; then
    regedited scan "$FILE" --filter "$FILTER"
else
    regedited scan "$FILE"
fi

echo ""
echo "=== Document Summary ==="
regedited summary "$FILE"
