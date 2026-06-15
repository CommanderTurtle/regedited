#!/bin/bash
# Transaction helper for Regedited
# Usage: regedited_tx.sh <file> <action> [args...]
# Actions: begin, commit, rollback, status

set -e

FILE="${1:?Usage: regedited_tx.sh <file> <begin|commit|rollback|status>}"
ACTION="${2:?Usage: regedited_tx.sh <file> <begin|commit|rollback|status>}"

case "$ACTION" in
    begin|start)
        regedited tx begin "$FILE"
        echo "Transaction started for $FILE"
        echo "Make changes, then: regedited_tx.sh $FILE commit"
        ;;
    commit)
        regedited tx commit "$FILE"
        echo "Transaction committed"
        ;;
    rollback|abort)
        regedited tx rollback "$FILE"
        echo "Transaction rolled back"
        ;;
    status|st)
        regedited tx status "$FILE"
        ;;
    *)
        echo "Unknown action: $ACTION"
        echo "Valid: begin, commit, rollback, status"
        exit 1
        ;;
esac
