# Changelog

All notable changes to Regedited.

## [0.3.0] — 2025-06-22

### Added

- **`"regedited open"` trigger**: Scanner finds `regedited open` anywhere in a line — inside HTML comments (`<!-- regedited open SectionName -->`), JS/CSS comments (`// regedited open`), shell comments (`# regedited open`), etc. Enables embedding Regedited indexes in any file format. Auto-generates names (`OpenTrigger-1`, `OpenTrigger-2`) when no name follows.
- **Enhanced clipboard commands**: 5 new CLI commands for copying to system clipboard:
  - `clip-hexword <start> <end> --zone-type <type>` — Copy hex-word range
  - `clip-zone <file> <section> <zone_idx>` — Copy zone content
  - `clip-db <file> <section> <value_idx>` — Copy database value
  - `clip-dbline <file> <section>` — Copy full database line
  - `clip-ascii <file> <section>` — Copy ASCII store line

### Changed — Hex-Word Format Revolution

- **Format `TxLLLLLLL`**: The type digit is now the FIRST CHARACTER, not embedded in a `0x` prefix.
  - Old: `0x1000003C` (10 chars, type 1 at bits 28-31)
  - New: `1x000003C` (9 chars, type `1` immediately visible)
  - `0x000000A` → `0x000000A` (type 0 stays visually similar)
  - `1x0000050` → Code, line 80
  - `2x0000A00` → Media, line 2560
  - `3x0000001` → Database, line 1
- **Backward compatibility**: `decode_hex_word()` auto-detects both formats. All existing documents continue to work.


## [0.2.0] — 2025-06-16

### Added — 

- **WAL (Write-Ahead Log)**: `wal`, `wal-replay` commands. Every mutation is logged to `.wal` before touching the main file. CRC32 checksums per entry. Automatic crash recovery on replay. (`src/wal.rs`, 723 lines, 6 tests)
- **Transactions**: `tx begin`, `tx commit`, `tx rollback`, `tx status` commands. Batch multiple operations into a single atomic unit with all-or-nothing semantics. Uses WAL internally for durability. (`src/transaction.rs`, 436 lines, 4 tests)
- **Schema enforcement**: `schema --init`, `schema --validate` commands. Optional per-section schemas for type-safe configuration. Supports string/int/bool/path/enum/array/hex types with required/optional/range/one_of/default constraints. Stored as `.schema` files. (`src/schema.rs`, 590 lines, 4 tests)
- **Typed registry values**: `reg-types`, `reg-parse` commands. 10 registry types: REG_SZ, REG_DWORD, REG_QWORD, REG_BINARY, REG_MULTI_SZ, REG_EXPAND_SZ, REG_JSON, REG_TOML, REG_BOOL. Windows Registry compatible with Regedited extensions. (`src/typed_value.rs`, 433 lines, 8 tests)
- **Registry container mode**: `serve --file --port` command. HTTP server with REST API exposing all document operations via endpoints: `/`, `/sections`, `/section/{name}`, `/section/{name}/db`, `/section/{name}/ascii`, `/section/{name}/zone/{i}`, `/grep`, `/types`, `/wal`, `/health`. (`src/serve.rs`, 460 lines)
- **43+ CLI commands** (was 32)

### Changed —

- **Index line**: Changed from plain `123` to `index: 123`. Human-readable, renders correctly in all markdown viewers.
- **Database line**: Changed from tab-separated `42\t7\t3\t...` to pipe-separated `42 | 7 | 3 | ...`. Renders correctly in Obsidian, GitHub, VS Code. Both formats accepted when reading (auto-detection).
- **ASCII store**: Verified colon ` : ` separator consistency across all modules.
- **Backward compatibility**: Readers auto-detect both old (tab) and new (pipe) formats. Writers emit v3 format.

### Fixed —

- **Critical bug in zone_editor.rs**: Delta threshold was `start_line`, should be `end_line + 1`. Zone's own hex-words were being corrupted on content replace. Fixed to only shift lines AFTER the zone.
- **header.rs**: Removed dead `content_end_byte_offset` field (declared, never set, never used).
- **header.rs**: Updated outdated doc comment referencing "UTF-16LE" (was from early prototype).
- **store.rs add_section template**: Was malformed — only 6 zeros with `\t`, missing `index:` prefix, missing string lines. Fixed to proper v3 format.
- **fast_ops.rs index parsing**: `fast_scan_content` used plain `.parse::<u64>()` which would fail on `index: 100`. Added `index:` prefix detection with legacy fallback.
- **fast_ops.rs numeric parsing**: `parse_numeric_line_fast` only split on `\t`. Added auto-detection of ` | ` vs `\t`.
- **main.rs db handler**: Numeric line split on `\t` only. Added pipe separator auto-detection.

### Documentation

- **README.md**: Updated all examples to v3 format. Added beginner tutorial section with Python and evcxr walkthroughs. Added performance comparison tables (vs grep, vs PowerShell, vs Python readlines).
- **docs/ARCHITECTURE.md**: Consolidated from 5 separate files. Added "Serious Configuration Substrate Features" section covering WAL, transactions, schema, typed values, and serve mode. Updated module overview (21 modules). Updated command reference (43 commands).
- **docs/FLOWCHART.md**: 7 comprehensive mermaid diagrams (module dependencies, CLI router, Python integration, evcxr integration, abilities map, read/write sequence diagrams).
- **pi/SKILL.md**: Updated for v3 format and all new features. 361 lines covering 12 workflow categories.
- **pi/references/**: Added `wal_and_transactions.md`, `schema_and_types.md`, `serve.md`.
- **pi/scripts/**: Added `regedited_tx.sh`, `regedited_schema.sh`, `regedited_serve.sh`.
- **examples/example.md**: Updated to v3 format.
- **pi/assets/template.md**: Updated to v3 format.
- **All source files**: Added AGPL-3.0 SPDX headers (21/21 files).

### Performance (tested on 14,032-line test database markdown file)

- 102/102 tests passed
- Scan: 164 scans/sec on 14K lines
- Hex-word encode: 1.7M encodes/sec
- Grep: 97 matches in 15.5ms
- Metadata overhead: 0.33% for 3 appended sections

## [0.1.0] — 2026-06-08

### Added

- **Core format**: Structured markdown with `## SECTION:` headers, index numbers, hex-word stores, 9 database values, and 3 string lines
- **Hex-word store**: format with embedded type nibbles (Markdown, Code, Media, Database)
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
