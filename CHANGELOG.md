# Changelog

Notable changes to Regedited.

## [0.1.0] — 2026-06-08

### What Was Added (5 Major Features, 5 New Modules, 11 New CLI Commands)

|#|Feature|Module|Commands|Copilot Priority|
|:--|:--|:--|:--|:--|
|1|**WAL (Write-Ahead Log)**|`src/wal.rs` (723 lines)|`wal`, `wal-replay`|**#1 — Crash-safe writes**|
|2|**Transactions**|`src/transaction.rs`|`tx begin/commit/rollback/status`|**#3 — Batch atomicity**|
|3|**Schema Enforcement**|`src/schema.rs`|`schema --validate --init`|**#2 — Type safety**|
|4|**Typed Values**|`src/typed_value.rs`|`reg-types`, `reg-parse`|**#8 — Rich types**|
|5|**Registry Container**|`src/serve.rs`|`serve --file --port`|**#10 — Remote access**|

### Project Growth

|Metric|Before|After|
|:--|:--|:--|
|Source modules|16|**21**|
|Source lines (Rust)|8,115|**11,141** (+37%)|
|CLI commands|32|**43**|
|Documentation|2,837|**3,370** lines|

### Feature Checklist — Status

| #   | Feature                               | Status                                       |
| :-- | :------------------------------------ | :------------------------------------------- |
| 1   | Atomic crash-safe writes (WAL)        | **Implemented**                              |
| 2   | Schema enforcement                    | **Implemented**                              |
| 3   | Transactional batch edits             | **Implemented**                              |
| 4   | Offline virtual registry hives        | Documented (`.regd` files ARE virtual hives) |
| 5   | Diff-aware configuration (Git-native) | **Existing feature** — enhanced              |
| 6   | Partial loading + mmap                | **Existing feature** — documented            |
| 7   | Stable documented file format         | **Existing feature** — documented            |
| 8   | Plugin system for typed values        | **Implemented** — 10 registry types          |
| 9   | Windows API compatibility layer       | Design-ready (future DLL project)            |
| 10  | Registry container mode               | **Implemented** — HTTP REST API              |

---

# Pi / Agent Templates:

### SKILL.md — Rewritten (361 lines, was 236)

|Added|Details|
|:--|:--|
|**WAL workflows**|`wal`, `wal-replay --apply`, crash recovery flow|
|**Transaction workflows**|`tx begin` / `commit` / `rollback` / `status` with full example script|
|**Schema workflows**|`schema --init`, `schema --validate` with format reference|
|**Typed values workflows**|`reg-types`, `reg-parse` with all 10 registry types|
|**Serve workflows**|`serve --file --port` with curl examples|
|**WAL file format**|`SEQ|TIMESTAMP|OPERATION|...|CRC32` documented|
|**Schema file format**|Full `.schema` syntax with types and constraints|
|**Python WAL check**|Detect uncommitted WAL in Python subprocess|
|**Expanded trigger table**|14 scenarios (was 9)|

### New Reference Docs (4 files, 384 lines)

|File|Covers|
|:--|:--|
|`references/wal_and_transactions.md`|WAL format, recovery flow, transaction states, safe update script|
|`references/schema_and_types.md`|Schema syntax, field types, constraints, Python validation pattern|
|`references/serve.md`|All 13 REST endpoints, curl examples, Python client, Docker/CI usage|

### Updated Reference Docs

|File|Changes|
|:--|:--|
|`references/commands.md`|Added 11 new commands (wal, wal-replay, tx, schema, reg-types, reg-parse, serve)|

### New Helper Scripts (3 files)

|Script|Purpose|
|:--|:--|
|`scripts/regedited_tx.sh`|`begin` / `commit` / `rollback` / `status` wrapper|
|`scripts/regedited_schema.sh`|`init` / `validate` / `show` wrapper|
|`scripts/regedited_serve.sh`|`serve` with port, read-only flag, endpoint listing|

### Final Pi Package Stats

|Metric|Before|After|
|:--|:--|:--|
|SKILL.md|236 lines|**361 lines**|
|Reference docs|3 files|**6 files** (+3 new)|
|Helper scripts|6 files|**9 files** (+3 new)|
|Total Pi package|~720 lines|**1,360 lines**|

---

---


## [0.0.9b] — 2026-06-08

### Added

- **Core format**: Structured markdown with `## SECTION:` headers, index numbers, hex-word stores, 9 database values, and 3 string lines
- **Hex-word store**: `0xTLLLLLLL` format with embedded type nibbles (Markdown, Code, Media, Database)
- **Fast scan**: Safetensors-style header-only scan that reads metadata without loading content
- **Fast diff**: Metadata-only file comparison
- **Fast replace**: Patch sections from source into target
- **Fast grep**: Memory-mapped line grep (ripgrep-style)
- **Zone content manipulation**: `zone-copy`, `zone-append`, `zone-replace`, `zone-extract`, `zone-info` commands with automatic line number recalculation
- **30+ CLI commands**: list, scan, db, ascii, info, content, diff, replace, fgrep, fgrep-multi, grep, zone-copy, zone-append, zone-replace, zone-extract, zone-info, set-num, set-str, set-zone, add, rm, new, types, convert, getutf, echo, echo-direct, clip, lines, encap, grab-html, bool-and, bool-nand, bool-or, bool-xor, count, if-contains
- **Windows CMD safe echo**: 5 encapsulation strategies for special characters (`&`, `|`, `"`)
- **Cross-platform clipboard**: Copy strings to system clipboard
- **getutf() utility**: DWORD-style line number encoding/decoding
- **Python scripting**: All commands designed for `subprocess` use with clean stdout
- **100% Rust**: Zero-dependency on Python runtime
- **Three-mode encapsulation**: `encap` command with b/c/d modes (shel.sh/XML inspired)
- **HTML attribute extraction**: `grab-html` command — GRAB B/C/D equivalent
- **Boolean operations**: `bool-and`, `bool-nand`, `bool-or`, `bool-xor`, `count`, `if-contains` with exit codes
- **Pi/OMP skill package**: Drop-in skill for Pi agent (`pi/` folder)

### Design Decisions

- **Header-scan approach**: Inspired by safetensors' JSON metadata header. Regedited scans `## SECTION:` headers to build an index of byte offsets, enabling O(1) jumps.
- **Type-embedded hex-words**: Every zone boundary carries its own type nibble, allowing mixed-type zones within a single section.
- **Line number recalculation**: When content changes size, all hex-word line numbers shift automatically via `LineDelta` application.
- **Subprocess-first**: Designed to be called from Python scripts rather than embedded as a library.

### Performance

- Scan: O(n) on headers, O(1) memory per section
- Grep: Memory-mapped, only matching lines read into RAM
- Zone extract: O(1) via byte offset jump
- Zone replace: O(sections) with automatic recalculation
- Diff: O(sections), metadata only
