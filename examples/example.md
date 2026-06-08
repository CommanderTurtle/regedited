# Regedited Example Document

This document demonstrates the structured plaintext database format (v3).
Each section has:
- An **index number** (base10)
- A **hex-word store** (6 hex-words with embedded type nibbles)
- **9 tab-separated numeric values** (displayed as pipes & dashes table)
- **3 string lines** (one per line)
- A `---` separator before content

## Hex-Word Format

```
0xTLLLLLLL : 0xTLLLLLLL : 0xTLLLLLLL : 0xTLLLLLLL : 0xTLLLLLLL : 0xTLLLLLLL
```

Where `T` = type nibble, `LLLLLLL` = line number (28 bits = 268M max)

| Type | Nibble | Description |
|------|--------|-------------|
| Markdown | `0` | Plain text content |
| Code | `1` | Code snippets, scripts |
| Media | `2` | Images, audio, video |
| Database | `3` | Tabular data, structured content |

---

## SECTION: ProjectConfig
100
0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000
1	100	50	200	25	75	10	20	30
project root path
config notes here
https://github.com/user/project
---
# Project Configuration

This section contains the main project configuration.

```json
{
  "name": "my-project",
  "version": "1.0.0"
}
```

## Build Instructions

Run `cargo build --release` to build.

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
    let store = Store::open("data.md").unwrap();
    let zone = store.get_zone("CodeSnippets", 1).unwrap();
    println!("{}", zone.content);
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

## Database

```rust
pub struct Store {
    content: String,
    header: DocumentHeader,
}
```

## SECTION: Documentation
300
0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000
5	10	15	20	25	30	35	40	45
table of contents
api reference
changelog notes
---
# Documentation

## Table of Contents

1. Introduction
2. Quick Start
3. API Reference
4. Examples

## Quick Start

Install with: `cargo install regedited`

## API Reference

### Store::open(path)

Opens a markdown file and parses its structure.

### Store::get_zone(name, index)

Extracts a content zone by index (0-2).

## SECTION: LinksAndRefs
400
0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000
3	0	0	0	0	0	0	0	0
https://rust-lang.org
https://docs.rs
https://crates.io
---
## References

- [Rust Language](https://rust-lang.org)
- [Documentation](https://docs.rs)
- [Crates Registry](https://crates.io)
