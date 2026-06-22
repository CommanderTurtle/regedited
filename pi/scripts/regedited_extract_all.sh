#!/bin/bash
# Extract all zones from all sections
# Usage: regedited_extract_all.sh <file> [output_dir]

set -e

FILE="${1:?Usage: regedited_extract_all.sh <file> [output_dir]}"
OUTDIR="${2:-./extracted}"

mkdir -p "$OUTDIR"

# Get all section names
sections=$(regedited list "$FILE" 2>/dev/null | grep "^-" | sed 's/^.*- //' | awk '{print $1}')

for section in $sections; do
    for zone in 0 1 2; do
        output="$OUTDIR/${section}_zone${zone}.md"
        if regedited zone-extract "$FILE" "$section" "$zone" > "$output" 2>/dev/null; then
            lines=$(wc -l < "$output" | tr -d ' ')
            if [ "$lines" -gt 0 ]; then
                echo "Extracted: $section zone $zone ($lines lines) → $output"
            else
                rm "$output"
            fi
        else
            rm -f "$output"
        fi
    done
done

echo "Done. Output in $OUTDIR/"
