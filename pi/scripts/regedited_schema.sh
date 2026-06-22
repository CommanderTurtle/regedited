#!/bin/bash
# Schema helper for Regedited
# Usage: regedited_schema.sh <file> [init|validate|show]

set -e

FILE="${1:?Usage: regedited_schema.sh <file> [init|validate|show]}"
ACTION="${2:-show}"

case "$ACTION" in
    init)
        regedited schema "$FILE" --init
        echo "Schema initialized: $FILE.schema"
        ;;
    validate|check)
        regedited schema "$FILE" --validate
        ;;
    show|*)
        regedited schema "$FILE"
        ;;
esac
