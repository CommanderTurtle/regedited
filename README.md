### So I rewrote the windows registry in rust.

What makes `regedit` so great? Built from an initial joke on “Why need a DB? You should dangerously grep a million-line markdown file” 

The registry, edited. A fast plaintext parse-ment database with structured headers, typed hex-word offsets, and O(1) section jumps on multi-GB files.

# Regedited

> The best way to predict the future is to invent it.
 	- Alan Kay

Inspired by the [safetensors](https://github.com/huggingface/safetensors) format's ability to scan, diff, and replace keys in multi-gigabyte files without loading them into RAM — applied to structured markdown documents with full key-value semantics.

## Why It's Fast

Regedited memory-maps your file and builds an index of section headers. A **10GB file with 1,000 sections uses ~200KB of Rust heap** — the file lives in OS-managed virtual memory, not your process RAM.

| Approach | 10GB File RAM | Section Jump |
|----------|--------------|--------------|
| `cat + grep` | 10GB | O(n) scan |
| Python `readlines()` | 10GB | O(1) — at a cost |
| **Regedited** | **~200KB** | **O(1) byte offset** |

The trick is the **hex-word store**: each section header contains typed line-number pointers (`0xTLLLLLLL`) that encode both *where* content lives and *what type* it is. Change content, and all pointers recalculate automatically.

---

## Hex-Word Range Replacement — The Killer Feature

Each section carries 6 hex-words encoding 3 typed zone pairs. Change a zone's content, and every hex-word in the document shifts to stay consistent.

### Example: Replace a Code Zone

```markdown
## SECTION: CodeSnippets
index: 200
0x00000000 : 0x00000000 : 0x1000003C : 0x10000042 : 0x00000000 : 0x00000000
42 | 7 | 3 | 256 | 1024 | 4096 | 100 | 200 | 300
main.rs core logic
utility functions
database connection code
---
fn old_function() { ... }      <-- lines 60-66 (zone 1)
```

Replace zone 1 with new content (grows from 7 to 15 lines):

```bash
regedited zone-replace doc.md CodeSnippets 1 --text "$(cat new_code.rs)"
# Zone 1 replaced: 7 lines → 15 lines (+8 line delta)
# All subsequent hex-words shifted by +8
```

The document is automatically updated:

```markdown
## SECTION: CodeSnippets
index: 200
0x00000000 : 0x00000000 : 0x1000003C : 0x1000004A : 0x00000000 : 0x00000000
42 | 7 | 3 | 256 | 1024 | 4096 | 100 | 200 | 300
main.rs core logic
utility functions
database connection code
---
fn new_function() { ... }      <-- lines 60-74 (zone 1, recalculated)
## SECTION: NextSection
index: 300
0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000
...                            <-- this section's line numbers also shifted +8
```

### Convert Line Ranges to Hex-Words

```bash
# What hex-words do I need for lines 50-80 of code?
regedited convert 50 80 --zone-type code
# Start: 0x10000032
# End:   0x10000050
#
# Paste into your .md:
# 0x10000032 : 0x10000050 : 0x00000000 : 0x00000000 : 0x00000000 : 0x00000000
```

### Hex-Word Format

Each value is `0xTLLLLLLL` where `T` = type nibble, `L` = line number:

| Hex-Word | Type | Line | Meaning |
|----------|------|------|---------|
| `0x0000000A` | Markdown | 10 | Text at line 10 |
| `0x10000050` | Code | 80 | Code at line 80 |
| `0x20000A00` | Media | 2560 | Media at line 2560 |
| `0x30000001` | Database | 1 | Data at line 1 |

---

### File Overview:

```plain
regedited/
├── src/                          # 21 modules, 11,221 lines
│   ├── main.rs                   # 1,984 lines — CLI (43 commands)
│   ├── lib.rs                    # 392 lines — core types & re-exports
│   ├── wal.rs                    # 723 lines, 6 tests — crash-safe writes
│   ├── transaction.rs            # 436 lines, 4 tests — batch atomicity
│   ├── schema.rs                 # 590 lines, 4 tests — type enforcement
│   ├── typed_value.rs            # 433 lines, 8 tests — registry types
│   ├── serve.rs                  # 460 lines — HTTP container
│   ├── fast_ops.rs               # 808 lines, 9 tests — scan/diff/replace
│   ├── zone_editor.rs            # 467 lines, 6 tests — content manipulation
│   ├── store.rs                  # 657 lines, 11 tests — high-level API
│   ├── header.rs                 # 555 lines, 9 tests — section scanner
│   ├── db_line.rs                # 549 lines, 13 tests — 9-value parser
│   ├── zone_type.rs              # 377 lines, 11 tests — hex-word codec
│   ├── echo.rs                   # 530 lines, 15 tests — safe echo
│   ├── encapsulate.rs            # 306 lines, 8 tests — b/c/d modes
│   ├── html_extract.rs           # 399 lines, 13 tests — GRAB B/C/D
│   ├── bool_ops.rs               # 356 lines, 10 tests — AND/NAND/OR/XOR
│   ├── zone.rs                   # 462 lines, 6 tests — zone extraction
│   ├── ascii_store.rs            # 240 lines, 5 tests — hex-word store
│   ├── utf16.rs                  # 273 lines, 13 tests — DWORD encoding
│   └── clip.rs                   # 224 lines, 6 tests — clipboard
├── docs/                         # 5 docs (compatibility reference), ~2,800 lines
│   ├── ARCHITECTURE.md           # comprehensive reference (500+ lines)
│   ├── FLOWCHART.md              # 7 conceptual diagrams
│   ├── USAGE.md                  # command redirect
│   ├── FORMAT.md                 # format redirect
│   └── PYTHON.md                 # Python redirect
├── pi/                           # Pi/OMP skill package
│   ├── SKILL.md                  # 361 lines, 12 workflow categories
│   ├── README.md                 # skill package docs
│   ├── install.sh                # global/local/OMP install
│   ├── scripts/                  # 9 helper scripts
│   ├── references/               # 6 reference docs
│   └── assets/template.md        # v3 format template
├── examples/                     # 2 examples
│   ├── example.md                # v3 format demo
│   └── python_workflow.py        # Python integration demo
├── Cargo.toml                    # v0.2.0, AGPL-3.0
├── CHANGELOG.md                  # full v0.1.0 + v0.2.0 history
├── README.md                     # landing page with tutorial
├── CONTRIBUTING.md               # contributor guide
├── LICENSE                       # AGPL-3.0
└── .gitignore                    # standard Rust
```

## Quick Start

```bash
# Build
cargo build --release

# List sections in a document
./target/release/regedited list myfile.md

# Header-only scan (reads ~100 bytes per section)
./target/release/regedited scan myfile.md

# Extract zone 1 from a section (O(1) jump)
./target/release/regedited grep myfile.md MySection 1

# Replace zone content (automatic line recalculation)
cat new_code.rs | ./target/release/regedited zone-replace myfile.md MySection 1

# Diff two files (metadata only)
./target/release/regedited diff base.md patched.md

# Boolean: contains ALL patterns?
./target/release/regedited bool-and myfile.md MySection "rust" "fn" "main"

# Extract HTML attributes (GRAB B/C/D equivalent)
./target/release/regedited grab-html page.html HREF --tag a --mode d --set 0

# Three-mode encapsulation (Windows CMD safe)
./target/release/regedited encap "hello" --mode d   # → ["'hello'"]
```

---

## Beginner Tutorial

### From Python (Complete Walkthrough)

```python
import subprocess
import shutil

# Path to the binary (adjust as needed)
RE = shutil.which("regedited") or "./target/release/regedited"

def re(*args):
    """Run any regedited command. Returns stdout string."""
    result = subprocess.run(
        [RE, *args], capture_output=True, text=True, check=True
    )
    return result.stdout

# ---- Step 1: Create your first document ----
re("new", "tutorial.md", "My First Document")

# ---- Step 2: Add sections ----
re("add", "tutorial.md", "Introduction")
re("add", "tutorial.md", "CodeSamples")
re("add", "tutorial.md", "Notes")

# ---- Step 3: List what we have ----
print(re("list", "tutorial.md"))
# Sections: 3 sections in tutorial.md
#   - CodeSamples (header @ line 9)
#   - Introduction (header @ line 1)
#   - Notes (header @ line 17)

# ---- Step 4: Set database values ----
re("set-num", "tutorial.md", "Introduction", "0", "42")
re("set-str", "tutorial.md", "Introduction", "0",
   "This is the intro section")

# ---- Step 5: Define a zone (lines 20-25, code type) ----
re("set-zone", "tutorial.md", "CodeSamples", "0", "20", "25",
   "--zone-type", "code")

# ---- Step 6: View the database table ----
print(re("db", "tutorial.md", "CodeSamples"))

# ---- Step 7: Extract zone content ----
code = re("zone-extract", "tutorial.md", "CodeSamples", "0")
print(f"Extracted {len(code)} characters of code")

# ---- Step 8: Boolean check ----
result = subprocess.run(
    [RE, "bool-and", "tutorial.md", "Introduction", "intro", "section"]
)
print("Found both words!" if result.returncode == 0 else "Missing words")

# ---- Step 9: Check if section has "TODO" but NOT "DONE" ----
result = subprocess.run(
    [RE, "bool-nand", "tutorial.md", "Notes", "TODO", "DONE"]
)
print("Has TODOs remaining!" if result.returncode == 0 else "All clear")

# ---- Step 10: HTML extraction ----
# Given an HTML file, extract all HREFs as set variables
subprocess.run([RE, "grab-html", "page.html", "HREF",
                "--tag", "a", "--mode", "d", "--set", "0"])
# Output: set "0aaa=["'https://example.com'"]"
```

### From evcxr_repl (Rust Jupyter Notebook)

[evexr](https://github.com/evcxr/evcxr) is a Rust REPL that works in Jupyter notebooks. Here's how to use Regedited interactively:

```rust
:dep regedited = { path = "/path/to/regedited" }

use regedited::*;
use regedited::header::scan_content;
use regedited::zone_type::{ZoneType, encode_hex_word, decode_hex_word};
use regedited::zone_editor::{extract_zone_content, replace_zone_content};
use regedited::encapsulate::{encapsulate, EncapMode};

// ---- Read a document ----
let content = std::fs::read_to_string("tutorial.md").unwrap();
let header = scan_content(&content).unwrap();

// ---- List sections ----
for name in header.section_names() {
    println!("Section: {}", name);
}

// ---- Get section info ----
let intro = header.get_section("Introduction").unwrap();
println!("Intro header at line {}, content at lines {}-{}",
         intro.header_line, intro.content_start, intro.content_end);

// ---- Encode/decode hex-words ----
let hw = encode_hex_word(50, ZoneType::Code);
println!("Line 50 as Code hex-word: {}", hw);  // 0x10000032

let (line, zt) = decode_hex_word("0x10000032").unwrap();
println!("Decoded: line={}, type={:?}", line, zt);

// ---- Extract zone content ----
let code = extract_zone_content(&content, intro, 0).unwrap();
println!("Zone content: {} chars", code.len());

// ---- Replace zone content ----
let new_doc = replace_zone_content(
    &content, intro, 0, "fn new_main() {\n    println!(\"Updated!\");\n}"
).unwrap();
std::fs::write("tutorial_updated.md", new_doc).unwrap();

// ---- Three-mode encapsulation ----
let encap_b = encapsulate("hello world", EncapMode::Search);
println!("{}", encap_b);  // ["hello world"]

let encap_d = encapsulate("hello world", EncapMode::Store);
println!("{}", encap_d);  // ["'hello world'"]

// ---- Boolean operations ----
let result = regedited::bool_ops::bool_and(&content,
    &["fn".to_string(), "main".to_string()]);
println!("AND result: {} (found {} matches)", result.value, result.matches.len());
```

---

## Performance: How Fast Is It?

These are estimated timings on a typical NVMe SSD, single-threaded:

### vs `grep` / `Select-String` (PowerShell)

| File Size | Sections | `grep` full scan | `Select-String` | **Regedited `scan`** | Speedup |
|-----------|----------|-----------------|----------------|---------------------|---------|
| 1 MB | 10 | 5 ms | 80 ms | **0.5 ms** | 10-160x |
| 100 MB | 100 | 200 ms | 3,000 ms | **2 ms** | 100-1,500x |
| 1 GB | 500 | 2,000 ms | 30,000 ms | **5 ms** | 400-6,000x |
| 10 GB | 1,000 | 20,000 ms | 300,000 ms | **10 ms** | 2,000-30,000x |

Why: `grep` scans every byte. Regedited scans only headers (~100 bytes each).

### Zone Extraction: Regedited vs Line-by-Line

| File Size | Zone Jump | Python `readlines()` | **Regedited `grep`** | Memory |
|-----------|-----------|---------------------|---------------------|--------|
| 1 MB | O(1) | 10 MB RAM | **0.01 ms** | ~10 KB |
| 100 MB | O(1) | 100 MB RAM | **0.01 ms** | ~50 KB |
| 1 GB | O(1) | 1 GB RAM | **0.01 ms** | ~100 KB |
| 10 GB | O(1) | Crash / swap | **0.01 ms** | ~200 KB |

Why: Python `readlines()` loads everything. Regedited uses byte-offset jumps.

### Zone Replace: Regedited vs Manual Editing

| Operation | Manual (find + edit) | **Regedited** |
|-----------|---------------------|---------------|
| Replace zone content | 2-5 minutes | **50 ms** |
| Fix all line numbers | 10-30 minutes (error-prone) | **automatic** |
| Copy zone A → zone B | 3-5 minutes | **30 ms** |

### Key Insight

Regedited is not "faster grep" — it's a **different approach entirely**:

- **Traditional tools** treat files as byte streams → O(n) on every operation
- **Regedited** treats files as indexed databases → O(1) jumps after an O(n) scan

The scan is a one-time cost. After that, every section jump, zone extract, and content replace is effectively instant.

---

## Document Format

```markdown
## SECTION: Name
index: 123                       <!-- Index number -->
0x00000000 : 0x00000000 : 0x1000003C : 0x10000042 : 0x00000000 : 0x00000000
42 | 7 | 3 | 256 | 1024 | 4096 | 100 | 200 | 300   <!-- 9 pipe-separated values -->
First string line
Second string line
Third string line
---
... markdown content ...
```

- **Index**: `index: N` — human-readable, Obsidian-friendly
- **6 hex-words** = 3 typed zone pairs (`0xTLLLLLLL` format, colon-separated)
- **9 values** = configurable numeric database fields (pipe ` | ` separated — renders in any markdown viewer)
- **3 strings** = labels, paths, descriptions
- **Content area** = opaque markdown, accessed via zone pointers

---

## Command Overview

| Category | Key Commands | Purpose |
|----------|-------------|---------|
| **Scan** | `list`, `scan`, `db`, `ascii` | Inspect documents |
| **Grep** | `fgrep`, `fgrep-multi`, `grep` | Memory-mapped search |
| **Zone** | `zone-copy`, `zone-append`, `zone-replace`, `zone-extract` | Content manipulation |
| **Write** | `set-num`, `set-str`, `set-zone`, `add`, `rm` | Edit values |
| **Diff** | `diff`, `replace` | Safetensors-style patch |
| **Bool** | `bool-and`, `bool-nand`, `bool-or`, `bool-xor`, `count`, `if-contains` | Content logic |
| **HTML** | `grab-html` | Attribute extraction |
| **Encap** | `encap` | Three-mode quoting (b/c/d) |
| **Util** | `types`, `convert`, `getutf`, `echo`, `clip` | Helpers |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the complete command reference, format specification, Python integration guide, and internal architecture.

---

## Pi/OMP Skill Package

The `pi/` folder is a drop-in skill for [Pi](https://pi.ai) and Oh My Pi (OMP):

```bash
cd pi && ./install.sh        # ~/.pi/agent/skills/
cd pi && ./install.sh --omp  # ~/.omp/agent/skills/
```

Then `/reload` in Pi — it auto-discovers the skill from `SKILL.md`.

---

## Python Integration

```python
import subprocess

# Extract zone content into a variable
result = subprocess.run(
    ["regedited", "zone-extract", "doc.md", "CodeSnippets", "1"],
    capture_output=True, text=True
)
code_block = result.stdout  # Just the content, no headers

# Boolean check via exit code
result = subprocess.run(
    ["regedited", "bool-and", "doc.md", "MySection", "fn", "rust"]
)
if result.returncode == 0:
    print("All patterns found")
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full Python guide.

---

## Building

```bash
cargo build --release
```

Minimum Rust version: 1.70

---

## License

A databasing tool built under Turtle Protect Inc.'s [XML project](https://shel.sh/projects/XML)

This tool is for educational purposes, licensed under the GNU Affero General Public License v3.0 (AGPL-3.0).
- https://www.gnu.org/licenses/agpl-3.0.html

*The registry, edited.*

*Project may receive further updates.*
