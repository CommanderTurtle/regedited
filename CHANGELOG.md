# Changelog

Notable changes to Regedited.

## [0.1.0] — 2025-01-15

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
