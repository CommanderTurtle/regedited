---
name: regedited
description: Fast plaintext parse-ment database for structured markdown documents. Use when working with large markdown files (>10MB), structured data extraction from markdown headers, zone-based content manipulation, boolean content analysis (AND/NAND/OR/XOR), HTML attribute extraction, or Windows CMD-safe text workflows. Replaces slow grep/awk/sed for structured documents with O(1) section jumps via hex-word metadata.
---

# Regedited — Fast Plaintext Parse-Ment Database

A safetensors-inspired plaintext database. Structured headers with typed hex-word offsets enable instant section jumps and memory-mapped operations on multi-GB files.

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
```

## Document Format Reference

Sections follow this structure:

```markdown
## SECTION: Name
123                              <!-- Index number -->
0x00000000 : 0x00000000 : 0x1000003C : 0x10000042 : 0x00000000 : 0x00000000
42	7	3	256	1024	4096	100	200	300   <!-- 9 tab-separated values -->
First string line description
Second string line description
Third string line description
---
... markdown content ...
```

### Hex-Word Format: `0xTLLLLLLL`

| Nibble | Type |
|--------|------|
| 0x0 | Markdown |
| 0x1 | Code |
| 0x2 | Media |
| 0x3 | Database |

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
# → 0x10000032 : 0x10000050
```

## Exit Codes for Scripting

| Command | Exit 0 | Exit 1 |
|---------|--------|--------|
| `bool-and` | ALL patterns found | One or more missing |
| `bool-nand` | Contains A, NOT B | Contains B or missing A |
| `bool-or` | ANY pattern found | None found |
| `bool-xor` | Exactly ONE found | Both or neither |

All other commands: 0 = success, 1 = error.
