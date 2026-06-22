#!/bin/bash
# Install Regedited Pi Skill into ~/.pi/agent/skills/
# Usage: ./install.sh [--global|--local|--omp]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_NAME="regedited"

# Determine install location
if [ "$1" == "--local" ]; then
    INSTALL_DIR="./.pi/skills/$SKILL_NAME"
elif [ "$1" == "--omp" ]; then
    INSTALL_DIR="${HOME}/.omp/agent/skills/$SKILL_NAME"
else
    INSTALL_DIR="${HOME}/.pi/agent/skills/$SKILL_NAME"
fi

echo "Installing Regedited Pi skill to: $INSTALL_DIR"

# Create directory
mkdir -p "$INSTALL_DIR"

# Copy skill files
cp -r "$SCRIPT_DIR"/* "$INSTALL_DIR/"

# Make scripts executable
chmod +x "$INSTALL_DIR/scripts/"*.sh

echo ""
echo "=== Regedited Pi Skill Installed ==="
echo "Location: $INSTALL_DIR"
echo ""
echo "Files:"
find "$INSTALL_DIR" -type f | sort
echo ""

# Check if regedited binary is available
if command -v regedited &> /dev/null; then
    echo "Regedited binary: $(command -v regedited)"
    regedited --version 2>/dev/null || echo "(version check failed)"
else
    echo "WARNING: regedited binary not found in PATH"
    echo "Install with: cargo build --release (from regedited repo)"
    echo "Then link: ln -s $(pwd)/../target/release/regedited ~/.pi/agent/bin/regedited"
fi

echo ""
echo "=== Features (43+ commands) ==="
echo "Core:     scan, list, db, ascii, fgrep, grep, zone-*"
echo "Write:    set-num, set-str, set-zone, add, rm, new"
echo "Bool:     bool-and, bool-nand, bool-or, bool-xor, count, if-contains"
echo "WAL:      wal, wal-replay (crash safety)"
echo "Tx:       tx begin/commit/rollback/status (batch atomicity)"
echo "Schema:   schema --init --validate (type enforcement)"
echo "Types:    reg-types, reg-parse (registry typed values)"
echo "Serve:    serve --file --port (registry container)"
echo "Encap:    encap --mode b/c/d (Windows CMD safe)"
echo "HTML:     grab-html (attribute extraction)"
echo "Util:     types, convert, getutf, echo, clip"
echo ""
echo "Usage in Pi:"
echo "  /reload                    # Reload skills"
echo "  regedited list doc.md      # List sections"
echo "  regedited scan doc.md      # Header scan"
echo "  regedited wal doc.md       # Check WAL status"
echo "  regedited tx begin doc.md  # Start transaction"
echo "  regedited schema doc.md --init   # Generate schema"
echo "  regedited serve --file doc.md    # Start container"
echo ""
echo "Or simply ask Pi to 'inspect this markdown file with regedited'"
