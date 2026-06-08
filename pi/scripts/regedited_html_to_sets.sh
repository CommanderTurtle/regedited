#!/bin/bash
# Extract HTML attributes as set variables (shel.sh database style)
# Usage: regedited_html_to_sets.sh <html_file> <attr> [tag_filter] [base_name]

set -e

HTML="${1:?Usage: regedited_html_to_sets.sh <html_file> <attr> [tag] [base_name]}"
ATTR="${2:?Usage: regedited_html_to_sets.sh <html_file> <attr> [tag] [base_name]}"
TAG="${3:-}"
BASE="${4:-0}"

MODE="d"  # Always use store mode for set variables

if [ -n "$TAG" ]; then
    regedited grab-html "$HTML" "$ATTR" --tag "$TAG" --mode "$MODE" --set "$BASE"
else
    regedited grab-html "$HTML" "$ATTR" --mode "$MODE" --set "$BASE"
fi
