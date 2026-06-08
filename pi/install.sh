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
    # Oh My Pi uses same structure
    INSTALL_DIR="${HOME}/.omp/agent/skills/$SKILL_NAME"
else
    # Default: global Pi install
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
find "$INSTALL_DIR" -type f | head -20
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
echo "Usage in Pi:"
echo "  /reload          # Reload skills"
echo "  regedited list doc.md     # List sections"
echo "  regedited scan doc.md     # Header scan"
echo "  regedited encap 'text'    # Encapsulate"
echo ""
echo "Or simply ask Pi to 'use regedited to inspect this markdown file'"
