# Document Format Specification

See [ARCHITECTURE.md](ARCHITECTURE.md#document-format-specification) for the complete format specification, including the hex-word deep-dive.

This file is kept for backward compatibility. All documentation has been consolidated into ARCHITECTURE.md.
t>
```

### Example

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

---

## Line 1: `## SECTION: <Name>`

The section header. Must start with exactly `## SECTION:` followed by a space and the section name. Names are case-sensitive but lookups are case-insensitive.

**Rules:**
- Must be `## SECTION:` (not `# SECTION:` or `### SECTION:`)
- Name cannot contain newlines
- Must be at the start of a line (no leading whitespace)

---

## Line 2: `<Index>`

A base-10 integer serving as the section's unique identifier.

```
200
```

**Rules:**
- Must parse as a 64-bit unsigned integer
- Zero is valid (represents an unindexed section)
- Indexes need not be sequential or unique (they're advisory)

---

## Line 3: `<Hex-Word Store>`

Six hex-words separated by ` : ` defining three zone pairs.

```
0x00000000 : 0x00000000 : 0x1000003C : 0x10000042 : 0x00000000 : 0x00000000
```

### Hex-Word Format

Each hex-word is `0xTLLLLLLL` where:

| Field | Bits | Description |
|-------|------|-------------|
| `T` | 4 | Type nibble (0-15) |
| `LLLLLLL` | 28 | Line number (0 to 268,435,455) |

### Type Nibbles

| Nibble | Name | Description |
|--------|------|-------------|
| `0` | Markdown | Plain text content (default) |
| `1` | Code | Code snippets, scripts, shell commands |
| `2` | Media | Images, audio, video references |
| `3` | Database | Tabular data, structured content |
| `4-F` | Reserved | Future expansion |

### Zone Pairs

Three pairs of `(start, end)` define content boundaries:

| Position | Meaning |
|----------|---------|
| Words 0-1 | Zone 0: `(start_0, end_0)` |
| Words 2-3 | Zone 1: `(start_1, end_1)` |
| Words 4-5 | Zone 2: `(start_2, end_2)` |

Both start and end are **inclusive** line numbers (0-indexed into the file).

An empty zone is `0x00000000 : 0x00000000`.

### Examples

| Hex-Word | Meaning |
|----------|---------|
| `0x0000000A` | Markdown, line 10 |
| `0x10000050` | Code, line 80 |
| `0x20000A00` | Media, line 2560 |
| `0x30000001` | Database, line 1 |
| `0x0FFFFFFF` | Markdown, line 268,435,455 (max) |

---

## Line 4: `<Database Line>`

Nine tab-separated numeric values:

```
42	7	3	256	1024	4096	100	200	300
```

These are displayed as a pipes-and-dashes markdown table:

```
| Val1 | Val2 | Val3 | Val4 | Val5 | Val6 | Val7 | Val8 | Val9 |
|------|------|------|------|------|------|------|------|------|
|   42 |    7 |    3 |  256 | 1024 | 4096 |  100 |  200 |  300 |
```

Values are signed 64-bit integers. Common conventions:
- Val1-3: counts, sizes, flags
- Val4-6: offsets, positions, ranges
- Val7-9: user-defined data

---

## Lines 5-7: `<String 1>`, `<String 2>`, `<String 3>`

Three string values, one per line. Trimmed whitespace on read.

```
main.rs core logic
utility functions
database connection code
```

These can hold:
- Labels and descriptions
- File paths and URLs
- Tags and categories
- Notes and references

---

## Line 8: `---`

The content separator. Three hyphens on their own line.

All lines after `---` until the next `## SECTION:` header (or EOF) constitute the section's markdown content.

---

## Content Area

Standard markdown content:

```markdown
## Main Logic

```rust
fn main() {
    println!("Hello from Regedited!");
}
```

## Utilities

```rust
pub fn checksum(data: &[u8]) -> u32 {
    use std::hash::Hasher;
    let mut hasher = fxhash::FxHasher32::default();
    hasher.write(data);
    hasher.finish() as u32
}
```
```

Content is opaque to Regedited — it is not parsed. Zones point into this area by line number.

---

## Line Number Calculation

The scanner counts lines from 0 (first line of the file):

| Position | Offset from Header |
|----------|-------------------|
| Header | `+0` |
| Index | `+1` |
| Hex-Word Store | `+2` |
| Database Line | `+3` |
| String 1 | `+4` |
| String 2 | `+5` |
| String 3 | `+6` |
| Separator (`---`) | `+7` |
| Content Start | `+8` |

For example, if a section header is at line 50:
- Index is at line 51
- Hex-word store is at line 52
- Database line is at line 53
- Strings are at lines 54, 55, 56
- Content starts at line 58

---

## Multi-GB File Considerations

For files larger than available RAM:

1. **Memory mapping**: Files are accessed via `memmap2` for zero-copy reads
2. **Header scan only**: `scan` reads ~100 bytes per section header, not the full file
3. **Line offsets**: The scanner builds an index of `(line_number, byte_offset)` pairs
4. **Zone extraction**: Uses byte offsets for O(1) jumps to any line
5. **No full-file buffering**: Content is sliced from the mmap, never copied

The 28-bit line number limit (268,435,455 lines) supports files up to roughly 50-100GB with average line lengths of 50-100 bytes.

---

## Empty Zones

When a zone has `0x00000000 : 0x00000000`:
- Zone extraction returns empty content
- Grep operations skip the zone
- Zone info shows `(empty)`

This is the default state for unused zones.

---

## Migration Between Versions

### v1 → v2: Added index number, changed 6→9 DB values

Old format:
```
## SECTION: Name
<ASCII store (33 chars)>
<6 tab-separated numbers>
```

New format:
```
## SECTION: Name
<Index number>
<Hex-word store (6 hex-words)>
<9 tab-separated numbers>
```

The v2 scanner is backward-compatible: if the index line doesn't parse as a number, it treats it as part of the old format.
