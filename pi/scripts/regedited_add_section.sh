#!/bin/bash
# Add a section to a Regedited document
# Usage: regedited_add_section.sh <file> <section_name>

set -e

FILE="${1:?Usage: regedited_add_section.sh <file> <section_name>}"
SECTION="${2:?Usage: regedited_add_section.sh <file> <section_name>}"

regedited add "$FILE" "$SECTION"
echo "Added section '$SECTION' to $FILE"

# Show the new section's empty structure
regedited db "$FILE" "$SECTION" 2>/dev/null || true
