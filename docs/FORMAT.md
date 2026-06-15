# Document Format Specification

See [ARCHITECTURE.md](ARCHITECTURE.md#document-format-specification) for the complete format specification.

**Format v3** (Obsidian-friendly):
- Index: `index: 123` (human-readable)
- Database values: pipe-separated `42 | 7 | 3 | ...` (renders in all markdown viewers)
- ASCII store: colon-separated `0x00000000 : 0x00000000 : ...`
- Legacy tab-separated format still accepted when reading

This file is kept for backward compatibility. All documentation has been consolidated into ARCHITECTURE.md.
