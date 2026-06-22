---
name: regedited
description: Fast plaintext parse-ment database for structured markdown documents with WAL crash safety, transactions, schema enforcement, typed registry values, and HTTP container mode. Use when working with large markdown files, structured data extraction, zone-based content manipulation, boolean content analysis, HTML attribute extraction, Windows CMD-safe workflows, configuration management, or when you need registry-like structured storage with git-friendly diffs. 43+ CLI commands. O(1) section jumps via hex-word metadata.
---

# Regedited — The Registry, Edited

A safetensors-inspired plaintext database. Structured headers with typed hex-word offsets enable instant section jumps and memory-mapped operations on multi-GB files. Now with WAL crash safety, transactions, schema enforcement, typed registry values, and HTTP container mode.

## When to Use

| Scenario | Use Regedited | Instead of |
|----------|-----------|------------|
| Large markdown files (>10MB) | `scan`, `fgrep` | `grep`, `cat` |
| Structured section extraction | `grep`, `zone-extract` | Manual line counting |
| Copy content between sections | `zone-copy` | Copy-paste |
| Check file contains ALL patterns | `bool-and` | `grep && grep && grep` |
| Check contains A but NOT B | `bool-nand` | Complex pipe chains |
| Extract HTML attrs (HREF, SRC) | `grab-html` | `grep -oP` regex |
| Safe Windows CMD output | `encap`, `echo` | Manual escaping |
| Structured data from headers | `db`, `ascii` | Parsing markdown by hand |
| Diff two structured files | `diff` | `diff` (unaware of structure) |
| **Crash-safe writes** | `wal`, `wal-replay` | Hope nothing crashes |
| **Batch atomic edits** | `tx begin` / `tx commit` | Individual commands |
| **Type-safe config** | `schema --validate` | No validation |
| **Registry typed values** | `reg-types`, `reg-parse` | Plain strings only |
| **Remote/container access** | `serve --file` | File copying |

## Binary Location

```bash
# regedited should be in PATH (install via cargo or place in ~/.pi/agent/bin/)
regedited --version

# If not found, build from source:
cd /path/to/regedited && cargo build --release
ln -s $(pwd)/target/release/regedited ~/.pi/agent/bin/regedited
```

## Quick Workflows

### 1. Inspect a Regedited Document

```bash
# List all sections
regedited list document.md

# Fast header scan (O(1), works on multi-GB files)
regedited scan document.md

# Full document info
regedited info document.md

# WAL status (crash safety check)
regedited wal document.md
```

### 2. Extract Content by Zone

```bash
# Extract zone 1 from a section (uses hex-word store for O(1) jump)
regedited grep document.md CodeSnippets 1

# Raw zone content to stdout (for piping)
regedited zone-extract document.md CodeSnippets 1 > extracted.rs

# Machine-readable zone metadata
regedited zone-info document.md CodeSnippets 1
```

### 3. Manipulate Content (with automatic line recalculation)

```bash
# Copy zone content between sections
regedited zone-copy document.md --from CodeSnippets --from-zone 1 --to Backup --to-zone 0

# Append content to a zone
regedited zone-append document.md CodeSnippets 1 --text "## New function"

# Replace zone content from file
cat new_code.rs | regedited zone-replace document.md CodeSnippets 1
```

### 4. Boolean Content Analysis

```bash
# Check if section contains ALL patterns (exit 0 = TRUE, 1 = FALSE)
regedited bool-and document.md MySection "rust" "fn" "main"

# Check contains pattern A but NOT pattern B
regedited bool-nand document.md MySection "fn" "python"

# Check contains ANY of the patterns
regedited bool-or document.md MySection "rust" "go" "typescript"

# Exactly ONE of two patterns
regedited bool-xor document.md MySection "rust" "python"

# Count occurrences
regedited count document.md MySection "TODO"

# Conditional output
regedited if-contains document.md MySection "fn" --then-val "HAS_CODE" --else-val "NO_CODE"
```

### 5. HTML Attribute Extraction

```bash
# Extract HREF attributes in store mode (d)
regedited grab-html page.html HREF --tag a --mode d --set 0

# Extract SRC from images
regedited grab-html page.html SRC --tag img --mode d --set img

# Quick list all links
regedited grab-html page.html HREF --mode b
```

### 6. Three-Mode Encapsulation (Windows CMD safe)

```bash
# Mode b (search): ["..."] — for string matching
regedited encap "hello world" --mode b

# Mode c (delimit): ['...'] — for piping
regedited encap "hello world" --mode c

# Mode d (store): ["'...'"] — for universal storage (DEFAULT)
regedited encap "hello world" --mode d

# Output as set variable (shel.sh database style)
regedited encap "https://example.com" --mode d --set 0aaa

# Extract from encapsulated string
regedited encap "['hello']" --extract

# Convert between modes
regedited encap "['hello']" --to d
```

### 7. Read/Write Database Values

```bash
# Show database table for section
regedited db document.md MySection

# Update numeric value (index 0-8)
regedited set-num document.md MySection 0 42

# Update string (index 0-2)
regedited set-str document.md MySection 0 "/new/path"

# Update zone with type (markdown/code/media/database)
regedited set-zone document.md MySection 0 50 80 --zone-type code
```

### 8. WAL — Crash-Safe Writes

```bash
# Check WAL status (shows if crash recovery is needed)
regedited wal document.md

# Replay uncommitted WAL entries (crash recovery)
regedited wal-replay document.md --apply

# Every write command automatically logs to WAL
# On crash, replay restores consistency
```

### 9. Transactions — Batch Atomicity

```bash
# Begin a transaction
regedited tx begin document.md

# Make changes (staged, not applied yet)
regedited set-num document.md Config 0 42
regedited set-str document.md Config 0 "/new/path"
regedited set-zone document.md Config 0 10 50 --zone-type code

# Commit all at once (atomic)
regedited tx commit document.md

# Or rollback (discard all)
# regedited tx rollback document.md

# Check transaction status
regedited tx status document.md
```

### 10. Schema — Type Enforcement

```bash
# Generate a starter schema from an existing document
regedited schema document.md --init

# Validate document against its schema
regedited schema document.md --validate

# Schema is stored as document.md.schema (human-readable)
```

### 11. Typed Registry Values

```bash
# List all supported registry types
regedited reg-types

# Parse a value as a typed registry value
regedited reg-parse "42" --reg-type REG_DWORD
# → 0x0000002A (42)

regedited reg-parse '{"name":"test","value":42}' --reg-type REG_JSON
# → JSON: {"name":"test","value":42}

# Types: REG_SZ, REG_DWORD, REG_QWORD, REG_BINARY, REG_MULTI_SZ,
#         REG_EXPAND_SZ, REG_JSON, REG_TOML, REG_BOOL
```

### 12. Serve — Registry Container Mode

```bash
# Start HTTP server for remote access
regedited serve --file config.regd --port 5000

# Query from anywhere:
# curl http://localhost:5000/sections
# curl http://localhost:5000/section/Config/db
# curl "http://localhost:5000/grep?pattern=enabled"
```

## Python Integration

When writing Python scripts that use Regedited via subprocess:

```python
import subprocess

# Zone extraction into variable
result = subprocess.run(
    ["regedited", "zone-extract", "doc.md", "CodeSnippets", "1"],
    capture_output=True, text=True
)
code_block = result.stdout

# Boolean check via exit code
result = subprocess.run(
    ["regedited", "bool-and", "doc.md", "MySection", "critical", "required"],
    capture_output=True, text=True
)
if result.returncode == 0:
    print("All patterns found")

# Encapsulation for safe variable passing
result = subprocess.run(
    ["regedited", "encap", variable_value, "--mode", "d", "--set", "0aaa"],
    capture_output=True, text=True
)
set_command = result.stdout.strip()  # set "0aaa=["'value'"]"

# WAL status check
result = subprocess.run(
    ["regedited", "wal", "doc.md"],
    capture_output=True, text=True
)
if "crash detected" in result.stdout.lower():
    print("Warning: uncommitted WAL found, run wal-replay --apply")

# Typed value parsing
result = subprocess.run(
    ["regedited", "reg-parse", "42", "--reg-type", "REG_DWORD"],
    capture_output=True, text=True
)
print(result.stdout)  # Parsed value display
```

## Document Format Reference

Sections follow this structure:

```markdown
## SECTION: Name
index: 123                       <!-- Human-readable index -->
0x0000000 : 0x0000000 : 1x000003C : 1x0000042 : 0x0000000 : 0x0000000
42 | 7 | 3 | 256 | 1024 | 4096 | 100 | 200 | 300   <!-- 9 pipe-separated values -->
First string line description
Second string line description
Third string line description
---
... markdown content ...
```

### Pipe Separators (Obsidian-Friendly)

Unlike tab-separated values that collapse in markdown viewers, pipe separators ` | ` render correctly in Obsidian, GitHub, VS Code, and any markdown preview. Both pipe and tab formats are accepted when reading — Regedited auto-detects.

### Hex-Word Format: `TxLLLLLLL`

| Nibble | Type |
|--------|------|
| 0x0 | Markdown |
| 0x1 | Code |
| 0x2 | Media |
| 0x3 | Database |

### WAL File Format: `document.md.wal`

```
# REGEDITED WAL v1
# file: document.md
---
1|1705312200|set-num|Config|0|42|99|a3f2c1d8
2|1705312200|set-str|Config|0|/old|/new|b7d4e2f1
---
COMMIT|1705312205|d8e7f6a5
```

### Schema File Format: `document.md.schema`

```
# Regedited Schema v1
---
section Config
  field version  : string : required
  field max_size : int    : range(1, 1000000)
  field mode     : string : one_of("auto", "manual", "hybrid")
  field enabled  : bool   : default(true)
---
```

## Advanced

### Safetensors-Style Operations

```bash
# Header-only scan (reads ~100 bytes per section)
regedited scan huge_file.md --filter "Config"

# Metadata-only diff
regedited diff base.md patched.md

# Replace sections from source into target
regedited replace base.md patched.md -o result.md --sections "Config" "Data"
```

### Zone Type Converter

```bash
# Convert line range + type to hex-words for manual editing
regedited convert 50 80 --zone-type code
# → 1x000003C : 1x0000050
```

## Exit Codes for Scripting

| Command | Exit 0 | Exit 1 |
|---------|--------|--------|
| `bool-and` | ALL patterns found | One or more missing |
| `bool-nand` | Contains A, NOT B | Contains B or missing A |
| `bool-or` | ANY pattern found | None found |
| `bool-xor` | Exactly ONE found | Both or neither |
| `tx commit` | All ops committed | Transaction failed |
| `schema --validate` | Document valid | Validation errors |

All other commands: 0 = success, 1 = error.
