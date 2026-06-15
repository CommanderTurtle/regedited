#!/bin/bash
# Serve (registry container) helper for Regedited
# Usage: regedited_serve.sh <file> [port] [--writable]

set -e

FILE="${1:?Usage: regedited_serve.sh <file> [port] [--writable]}"
PORT="${2:-5000}"
READ_ONLY="true"

if [ "$3" == "--writable" ] || [ "$3" == "-w" ]; then
    READ_ONLY="false"
fi

echo "Starting Regedited Registry Container"
echo "  File:      $FILE"
echo "  Port:      $PORT"
echo "  Read-only: $READ_ONLY"
echo "  URL:       http://localhost:$PORT"
echo ""
echo "  Endpoints:"
echo "    GET  /               — Status"
echo "    GET  /sections       — List sections"
echo "    GET  /section/{name} — Section metadata"
echo "    GET  /section/{name}/db    — Database table"
echo "    GET  /section/{name}/zone/{i} — Zone content"
echo "    GET  /grep?pattern=  — Search"
echo "    GET  /health         — Health check"
echo ""
echo "  Press Ctrl+C to stop"
echo ""

regedited serve --file "$FILE" --port "$PORT" --read-only "$READ_ONLY"
