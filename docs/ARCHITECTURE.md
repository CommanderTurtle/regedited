# Architecture

How Regedited works internally — from memory layout to hex-word encoding.

## Table of Contents

- [Why It's Fast](#why-its-fast)
- [Design Goals](#design-goals)
- [Module Overview](#module-overview)
- [Data Flow](#data-flow)
- [Key Types](#key-types)
- [Hex-Word Format Deep-Dive](#hex-word-format-deep-dive)
- [Document Format Specification](#document-format-specification)
- [Command Reference](#command-reference)
- [Python Integration](#python-integration)
- [Memory Layout](#memory-layout)
- [Performance Characteristics](#performance-characteristics)
- [Error Handling](#error-handling)
- [Windows Compatibility](#windows-compatibility)
- [Testing](#testing)

---

## Why It's Fast

Regedited treats a plaintext markdown file like a **memory-mapped key-value store**. Instead of reading the entire file into RAM, it:

1. **Memory-maps the file** via `memmap2` — the OS handles paging, only accessed pages touch RAM
2. **Scans only headers** — a single pass finds all `## SECTION:` markers, ~100 bytes per section
3. **Builds an index** — a `BTreeMap<String, SectionInfo>` gives O(log n) section lookups
4. **Jumps directly to content** — byte offsets in `SectionInfo` enable O(1) zone extraction
5. **Patches in-place** — content-aware zone manipulation recalculates only affected hex-words

The result: a **10GB file with 1,000 sections uses ~200KB of Rust heap**. The file itself lives in OS-managed virtual memory.

### Comparison

| Approach | 10GB File RAM | Startup | Section Jump |
|----------|--------------|---------|--------------|
| `cat + grep` | 10GB | O(n) | O(n) scan |
| `ripgrep` | streaming | O(n) | O(n) scan |
| Python readlines() | 10GB | O(n) | O(1) index |
| **Regedited** | **~200KB** | **O(headers)** | **O(1) byte offset** |

The Python readlines() approach loads everything into a Vec, giving O(1) jumps — but at the cost of 10GB RAM. Regedited gets the same O(1) jumps with 50,000x less memory.

---

## Design Goals

1. **Safetensors-style speed**: Header-only operations, memory-mapped I/O
2. **Human-readable format**: Plain markdown with hex-word annotations
3. **Python-scriptable**: Clean stdout, subprocess-friendly
4. **Windows-compatible**: Safe echo, clipboard support
5. **Multi-GB capable**: O(1) section jumps, no full-file buffering

---

## Module Overview

```
src/
├── main.rs          # CLI: 30+ commands via clap
├── lib.rs           # Core types, re-exports, 16 modules
├── fast_ops.rs      # Scan, diff, replace, grep (safetensors-style)
├── zone_editor.rs   # Content-aware zone copy/append/replace
├── store.rs         # High-level Store API with caching
├── header.rs        # ## SECTION: scanner with byte offsets
├── zone.rs          # Zone extraction with type prefixes
├── db_line.rs       # 9-value + 3-string parser
├── ascii_store.rs   # Hex-word store (6 typed zone pairs)
├── zone_type.rs     # ZoneType enum + hex-word codec
├── echo.rs          # Windows CMD safe echo (5 strategies)
├── clip.rs          # Cross-platform clipboard (arboard)
├── utf16.rs         # getutf() DWORD encoding utility
├── encapsulate.rs   # Three-mode encapsulation (b/c/d)
├── html_extract.rs  # HTML attribute extraction (GRAB B/C/D)
└── bool_ops.rs      # Boolean AND/NAND/OR/XOR/if-then
```

---

## Data Flow

### Reading

```
File → MmapFile (zero-copy) → scan_content() → DocumentHeader
                                                    ↓
                                            SectionInfo (offsets)
                                                    ↓
                                    extract_zone() → Zone (content + metadata)
```

1. `MmapFile::open()` memory-maps the file
2. `scan_content()` finds all `## SECTION:` headers in one pass
3. `DocumentHeader` stores `SectionInfo` with line numbers and byte offsets
4. `extract_zone()` uses byte offsets for O(1) jumps to content

### Writing

```
Changes → Store.update_*() → content string manipulation
                                    ↓
                          update_lines() (batched line replacement)
                                    ↓
                          apply_line_deltas() (recalculate hex-words)
                                    ↓
                          fs::write() (atomic file replace)
```

1. `Store` caches section data to avoid repeated parsing
2. Changes are batched and applied via `update_lines()`
3. If content size changes, `apply_line_deltas()` shifts all subsequent line numbers
4. File is written atomically (no partial writes visible to readers)

### Zone Content Manipulation

```
Source zone → extract_zone_content() → content string
                                              ↓
Target zone → replace_zone_content() ← new content
                                              ↓
                                calculate delta (new_lines - old_lines)
                                              ↓
                                apply_line_deltas() to entire document
                                              ↓
                                all hex-word stores updated
```

When a zone's content grows or shrinks:
1. The content is spliced in at the correct line range
2. A `LineDelta` is calculated: `(new_line_count - old_line_count)`
3. `apply_line_deltas()` scans all sections and shifts hex-word line numbers
4. Every zone boundary stays consistent with the new document structure

---

## Key Types

### DocumentHeader

```rust
pub struct DocumentHeader {
    pub sections: BTreeMap<String, SectionInfo>,
    pub total_lines: usize,
    pub total_bytes: usize,
}
```

The `BTreeMap` keeps sections in name order. Lookups are O(log n). Each `SectionInfo` contains pre-computed line numbers for every line in the section's metadata block.

### SectionInfo

```rust
pub struct SectionInfo {
    pub name: String,
    pub header_line: usize,       // ## SECTION: Name
    pub index_line: usize,        // 123
    pub ascii_line: usize,        // 0x0000...
    pub numeric_line: usize,      // 1 2 3 4 5...
    pub string1_line: usize,      // "first string"
    pub string2_line: usize,      // "second string"
    pub string3_line: usize,      // "third string"
    pub separator_line: usize,    // ---
    pub content_start: usize,     // first content line
    pub content_end: usize,       // last content line
}
```

All line numbers are pre-computed during the scan phase. No re-scanning needed for O(1) access to any line.

### AsciiStore

```rust
pub struct AsciiStore {
    pub zones: [ZonePair; 3],
}

pub struct ZonePair {
    pub start: u32,           // 28-bit line number
    pub end: u32,             // 28-bit line number
    pub zone_type: ZoneType,  // Markdown, Code, Media, Database
}
```

The hex-word format `0xTLLLLLLL` packs type and line number into a u32. This is parsed via bit masking, not string parsing, for speed.

### DbLine

```rust
pub struct DbLine {
    pub numbers: [i64; 9],
    pub strings: [String; 3],
}
```

Simple fixed-size arrays. Parsed via `split('\t')` — no complex parsing needed.

---

## Hex-Word Format Deep-Dive

Each zone boundary is a single 32-bit value: `0xTLLLLLLL`

### Bit Layout

```
31  28 27                                                   0
+------+----------------------------------------------------+
| Type |              Line Number (28 bits)                |
+------+----------------------------------------------------+
```

| Field | Bits | Range | Description |
|-------|------|-------|-------------|
| `T` | 4 | 0-15 | Type nibble |
| `L` | 28 | 0-268,435,455 | Line number |

### Type Nibbles

| Nibble | Name | Description |
|--------|------|-------------|
| `0x0` | Markdown | Plain text content (default) |
| `0x1` | Code | Code snippets, scripts, shell commands |
| `0x2` | Media | Images, audio, video references |
| `0x3` | Database | Tabular data, structured content |
| `0x4-F` | Reserved | Future expansion |

### Zone Pair Encoding

Three pairs of `(start, end)` define content boundaries:

```
0x00000000 : 0x00000000 : 0x1000003C : 0x10000042 : 0x00000000 : 0x00000000
 \_________/   \_________/   \_________/   \_________/
  Zone 0       (empty)       Zone 1       (Code)      Zone 2 (empty)
```

| Position | Meaning |
|----------|---------|
| Words 0-1 | Zone 0: `(start_0, end_0)` |
| Words 2-3 | Zone 1: `(start_1, end_1)` |
| Words 4-5 | Zone 2: `(start_2, end_2)` |

Both start and end are **inclusive** line numbers (0-indexed into the file). An empty zone is `0x00000000 : 0x00000000`.

### Examples

| Hex-Word | Type | Line | Meaning |
|----------|------|------|---------|
| `0x0000000A` | Markdown | 10 | Plain text at line 10 |
| `0x10000050` | Code | 80 | Code block at line 80 |
| `0x20000A00` | Media | 2560 | Media reference at line 2560 |
| `0x30000001` | Database | 1 | Database content at line 1 |
| `0x0FFFFFFF` | Markdown | 268,435,455 | Max line number |

### Decoding (Rust pseudo-code)

```rust
fn decode_hex_word(hex_word: u32) -> (ZoneType, u32) {
    let type_nibble = (hex_word >> 28) as u8;
    let line_number = hex_word & 0x0FFFFFFF;
    (ZoneType::from_nibble(type_nibble), line_number)
}
```

No string parsing — pure bit masking. This is why zone lookups are O(1).

---

## Document Format Specification

Each section follows a strict structure:

```markdown
## SECTION: <Name>
<Index>
<Hex-Word Store>
<Database Line>
<String 1>
<String 2>
<String 3>
---
<Markdown Content>
```

### Complete Example

```markdown
## SECTION: CodeSnippets
200
0x00000000 : 0x00000000 : 0x1000003C : 0x10000042 : 0x00000000 : 0x00000000
42	7	3	256	1024	4096	100	200	300
main.rs core logic
utility functions
database connection code
---
## Main Logic

```rust
fn main() {
    println!("Hello from Regedited!");
}
```
```

### Line Layout

| Offset from Header | Content | Example |
|--------------------|---------|---------|
| +0 | `## SECTION: <Name>` | `## SECTION: CodeSnippets` |
| +1 | Index number | `200` |
| +2 | Hex-Word Store | `0x00000000 : ...` |
| +3 | Database Line | `42	7	3	...` |
| +4 | String 1 | `main.rs core logic` |
| +5 | String 2 | `utility functions` |
| +6 | String 3 | `database connection code` |
| +7 | Separator | `---` |
| +8 | Content Start | Markdown content begins |

### Content Area

Standard markdown. Content is opaque to Regedited — it is not parsed. Zones point into this area by line number.

### Multi-GB File Considerations

For files larger than available RAM:

1. **Memory mapping**: Files are accessed via `memmap2` for zero-copy reads
2. **Header scan only**: `scan` reads ~100 bytes per section header, not the full file
3. **Line offsets**: The scanner builds an index of `(line_number, byte_offset)` pairs
4. **Zone extraction**: Uses byte offsets for O(1) jumps to any line
5. **No full-file buffering**: Content is sliced from the mmap, never copied

The 28-bit line number limit (268,435,455 lines) supports files up to roughly 50-100GB with average line lengths of 50-100 bytes.

---

## Command Reference

### Document Inspection

| Command | Args | Description |
|---------|------|-------------|
| `list` | `<file>` | List all sections |
| `scan` | `<file> [--filter <pat>] [--value <i:min:max>]` | Header-only scan |
| `db` | `<file> <section>` | Show database table |
| `ascii` | `<file> <section>` | Show hex-word store |
| `info` | `<file>` | Full document info |
| `summary` | `<file>` | Document summary |
| `content` | `<file> <section>` | Section markdown content |

### Grep & Extract

| Command | Args | Description |
|---------|------|-------------|
| `fgrep` | `<file> <pattern> [-s <section>]` | Memory-mapped grep |
| `fgrep-multi` | `<file> <p1> <p2>...` | Multi-pattern OR grep |
| `grep` | `<file> <section> <zone>` | Extract zone by index |
| `zone-extract` | `<file> <section> <zone>` | Raw zone to stdout |
| `zone-info` | `<file> <section> <zone>` | Machine-readable zone meta |
| `lines` | `<file> <start> <end>` | Arbitrary line range |

### Zone Manipulation

| Command | Args | Description |
|---------|------|-------------|
| `zone-copy` | `<file> -f <S> -m <n> -t <T> -n <n>` | Copy zone content |
| `zone-append` | `<file> <S> <z> [--text <t>]` | Append to zone (or stdin) |
| `zone-replace` | `<file> <S> <z> [--text <t>]` | Replace zone (or stdin) |

### Boolean Operations

| Command | Args | Exit 0 when |
|---------|------|-------------|
| `bool-and` | `<file> <S> <p1> [p2]...` | ALL patterns found |
| `bool-nand` | `<file> <S> <must> <mustnot>` | Contains must, NOT mustnot |
| `bool-or` | `<file> <S> <p1> [p2]...` | ANY pattern found |
| `bool-xor` | `<file> <S> <a> <b>` | Exactly ONE found |
| `count` | `<file> <S> <pattern>` | Always 0 (shows count) |
| `if-contains` | `<file> <S> <p> [--then <v>] [--else <v>]` | Always 0 (prints value) |

### Write

| Command | Args | Description |
|---------|------|-------------|
| `set-num` | `<file> <S> <i> <v>` | Update numeric value (0-8) |
| `set-str` | `<file> <S> <i> <v>` | Update string (0-2) |
| `set-zone` | `<file> <S> <z> <s> <e> [-t <type>]` | Update zone range+type |
| `add` | `<file> <section>` | Add new section |
| `rm` | `<file> <section>` | Remove section |
| `new` | `<file> <title>` | Create new document |

### Encapsulation (shel.sh/XML)

| Command | Args | Description |
|---------|------|-------------|
| `encap` | `<text> [-m b/c/d] [--extract] [--to <m>] [--set <v>]` | Encapsulate/extract/convert |

### HTML Extraction

| Command | Args | Description |
|---------|------|-------------|
| `grab-html` | `<file> <attr> [-m b/c/d] [--tag <t>] [--set <b>] [-n]` | Extract HTML attrs |

### Utility

| Command | Args | Description |
|---------|------|-------------|
| `types` | | List zone types |
| `convert` | `<start> <end> [-t <type>]` | Range to hex-words |
| `getutf` | `<number> [--decode <hex>]` | DWORD encode/decode |
| `echo` | `<file> <S> <i>` | Safe echo string |
| `echo-direct` | `<text>` | Safe echo raw text |
| `clip` | `<file> <S> <i>` | Copy string to clipboard |

### Diff & Replace

| Command | Args | Description |
|---------|------|-------------|
| `diff` | `<a> <b>` | Metadata-only diff |
| `replace` | `<target> <source> [-o <out>] [-s <s1> <s2>]` | Patch sections |

---

## Python Integration

Regedited is designed to be called from Python via `subprocess`. All commands return clean stdout suitable for parsing.

### Setup

```python
import subprocess
import shutil

REGEDITED = shutil.which("regedited") or "./target/release/regedited"

def regedited(*args):
    """Call regedited with arguments, return stdout."""
    result = subprocess.run(
        [REGEDITED, *args],
        capture_output=True, text=True, check=True
    )
    return result.stdout
```

### Zone Extraction

```python
# Extract zone content to a variable
result = subprocess.run(
    [REGEDITED, "zone-extract", "document.md", "CodeSnippets", "1"],
    capture_output=True, text=True, check=True
)
code_block = result.stdout

# Machine-readable zone info
result = subprocess.run(
    [REGEDITED, "zone-info", "document.md", "CodeSnippets", "1"],
    capture_output=True, text=True, check=True
)
info = {}
for line in result.stdout.strip().split('\n'):
    if line == '---CONTENT---':
        break
    if '=' in line:
        key, value = line.split('=', 1)
        info[key] = value
```

### Content Manipulation

```python
# Copy zone between sections
subprocess.run([
    REGEDITED, "zone-copy", "document.md",
    "--from", "CodeSnippets", "--from-zone", "1",
    "--to", "MySection", "--to-zone", "0"
], check=True)

# Append from Python string
subprocess.run([
    REGEDITED, "zone-append", "document.md", "CodeSnippets", "1",
    "--text", "\n## New Section\n\nNew content here."
], check=True)
```

### Boolean Checks

```python
# Exit code 0 = TRUE, 1 = FALSE
result = subprocess.run(
    [REGEDITED, "bool-and", "doc.md", "CodeSnippets", "fn", "rust"],
    capture_output=True, text=True
)
if result.returncode == 0:
    print("All patterns found")

# Conditional output
result = subprocess.run(
    [REGEDITED, "if-contains", "doc.md", "__all__", "TODO",
     "--then-val", "INCOMPLETE", "--else-val", "CLEAN"],
    capture_output=True, text=True
)
status = result.stdout.strip()
```

### HTML Extraction

```python
# Extract attributes as set variables
result = subprocess.run(
    [REGEDITED, "grab-html", "page.html", "HREF",
     "--tag", "a", "--mode", "d", "--set", "0"],
    capture_output=True, text=True
)
# set "0aaa=["'https://example.com'"]"
# set "0aab=["'https://another.com'"]"
```

---

## Memory Layout

For a 10GB file with 1,000 sections:

| Component | Memory |
|-----------|--------|
| File mapping | ~0 bytes (OS-managed) |
| DocumentHeader | ~200KB (1,000 x SectionInfo) |
| Section cache | ~0 bytes (on-demand) |
| Zone content | ~0 bytes (sliced from mmap) |
| **Total** | **~200KB** |

The entire file is never loaded into Rust's heap. Only metadata is allocated.

---

## Performance Characteristics

| Operation | Time | Memory | Notes |
|-----------|------|--------|-------|
| `scan` | O(n) on headers | O(1) | Reads ~100 bytes per section |
| `fgrep` | O(n) on matches | O(1) | Memory-mapped, no buffering |
| `zone-extract` | O(1) | O(content) | Byte offset jump |
| `zone-replace` | O(sections) | O(file) | Must rewrite + recalculate |
| `diff` | O(sections) | O(1) | Metadata only |
| `replace` | O(sections) | O(file) | Per-section patches |
| `bool-*` | O(content) | O(1) | Single scan per pattern |
| `grab-html` | O(lines) | O(1) | Streaming per line |

---

## Error Handling

Uses `thiserror` for structured errors:

```rust
pub enum RegeditedError {
    Io(std::io::Error),
    Parse(String),
    SectionNotFound(String),
    InvalidDbLine(String),
    HeaderCorruption(String),
    ZoneOutOfBounds { line: usize, max_lines: usize },
    Clipboard(String),
    EchoEncoding(String),
}
```

All errors include context (section name, line number, etc.) for debugging.

---

## Windows Compatibility

### Safe Echo

Windows CMD has special characters (`&`, `|`, `<`, `>`, `"`, `%`) that break `echo`. Five encapsulation strategies are tried in order:

1. **Standard** (1): `echo "string"` — works for safe strings
2. **DoubleQuote** (2): `echo ""string""` — handles quotes
3. **CaretEscape** (3): `echo "^"string^""` — complex cases
4. **Literal** (4): `echo 'string'` — handles `&` and `|`
5. **DoubleLiteral** (5): `echo ''string''` — ultra-safe fallback

### Clipboard

Uses `arboard` crate for cross-platform clipboard. On Windows, uses the native Win32 clipboard API.

---

## Testing

```
tests: 90+
coverage: core modules fully tested
integration: CLI commands tested via example.md
```

Run tests:
```bash
cargo test --lib        # Unit tests
cargo test              # All tests
cargo test --release    # Release mode (faster)
```

## Future Extensions

- **Regex grep**: Add regex support to `fgrep`
- **Parallel scan**: Multi-threaded section scanning
- **Compression**: Optional gzip for large files
- **Remote**: SSH-backed file access
- **Watch mode**: Auto-reload on file changes
- **LSP**: Language server for IDE integration
