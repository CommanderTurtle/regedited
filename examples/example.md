# Regedited Example Document

This document demonstrates the structured plaintext database format (v3).
Each index record has:
- An **index** in `index: N` format (human-readable, Obsidian-friendly)
- A **hex-word store** (6 hex-words with embedded type nibbles, colon-separated)
- **9 pipe-separated exact decimal values** (` | ` — renders in any markdown viewer)
- **3 string lines** (one per line)

Index records do not own nearby prose. Zone hex-words point to absolute lines
anywhere in the shared file.

## Hex-Word Format

```
TxLLLLLLL : TxLLLLLLL : TxLLLLLLL : TxLLLLLLL : TxLLLLLLL : TxLLLLLLL
```

Where `T` = type nibble, `LLLLLLL` = line number (28 bits = 268M max)

| Type | Nibble | Description |
|------|--------|-------------|
| Markdown | `0` | Plain text content |
| Code | `1` | Code snippets, scripts |
| Media | `2` | Images, audio, video |
| Database | `3` | Tabular data, structured content |

regedited open
index: 100
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
1 | 100 | 50 | 200 | 25 | 75 | 10 | 20 | 30
project root path
config notes here
https://github.com/user/project
# Project Configuration

This content is addressable by absolute zone ranges.

```json
{
  "name": "my-project",
  "version": "1.0.0"
}
```

## Build Instructions

Run `cargo build --release` to build.

regedited open
index: 200
0x0000000 : 0x0000000 : 1x000003B : 1x000004B : 0x0000000 : 0x0000000
42 | 7 | 3 | 256 | 1024 | 4096 | 100 | 200 | 300
main.rs core logic
utility functions
database connection code
# Code Snippets

```rust
fn main() {
    println!("Hello from Regedited!");
}
```

## Database Access

```python
import subprocess

# Extract zone 1 content
result = subprocess.run(
    ["regedited", "zone-extract", "example.md", "200", "1"],
    capture_output=True, text=True
)
print(result.stdout)
```

regedited open
index: 300
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900
data summary line
data notes here
data reference url
# Data Table

| ID | Name    | Value | Status |
|----|---------|-------|--------|
| 1  | Item A  | 100   | active |
| 2  | Item B  | 200   | active |
| 3  | Item C  | 300   | draft  |

regedited open
index: 400
0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0



# Empty Record Example

This record has empty strings and zero values — useful as a template.
