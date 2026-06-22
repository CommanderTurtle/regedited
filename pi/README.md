# Regedited Pi Skill

Pi/OMP skill package for the [Regedited](https://github.com/yourusername/regedited) fast plaintext parse-ment database — now with WAL crash safety, transactions, schema enforcement, typed registry values, and HTTP container mode.

## Install

### Global (recommended)
```bash
cd /path/to/regedited/pi
./install.sh
```

### For Oh My Pi (OMP)
```bash
./install.sh --omp
```

### Project-local
```bash
./install.sh --local   # Installs to ./.pi/skills/
```

### Manual
```bash
mkdir -p ~/.pi/agent/skills/
cp -r /path/to/regedited/pi ~/.pi/agent/skills/regedited
```

Then reload Pi:
```
/reload
```

## What Pi Gets

### Core (original)
- **Structured document inspection** — fast header scans on multi-GB files
- **Zone-based content extraction** — O(1) section jumps via hex-word metadata
- **Boolean content analysis** — `bool-and`, `bool-nand`, `bool-or`, `bool-xor` with exit codes
- **HTML attribute extraction** — `grab-html` as a `grep -oP` replacement for attrs
- **Three-mode encapsulation** — Windows CMD-safe `["..."]`, `['...']`, `["'...'"]`
- **Content-aware zone manipulation** — copy/append/replace with automatic line recalculation

### Serious Configuration Substrate (new)
- **WAL crash safety** — `wal`, `wal-replay` for atomic durable writes
- **Transactions** — `tx begin/commit/rollback` for batch atomicity
- **Schema enforcement** — `schema --init --validate` for type-safe config
- **Typed registry values** — `reg-types`, `reg-parse` for REG_SZ/DWORD/QWORD/JSON/TOML
- **Registry container** — `serve --file --port` for HTTP REST API access

## Structure

```
pi/
├── SKILL.md                      # Skill definition (read by Pi)
├── scripts/                      # Helper scripts
│   ├── regedited_init.sh         # Initialize new document
│   ├── regedited_add_section.sh  # Add section
│   ├── regedited_quick_scan.sh   # Quick scan + summary
│   ├── regedited_extract_all.sh  # Extract all zones
│   ├── regedited_html_to_sets.sh # HTML → set variables
│   ├── regedited_bool_check.sh   # Boolean check wrapper
│   ├── regedited_tx.sh           # Transaction helper (NEW)
│   ├── regedited_schema.sh       # Schema helper (NEW)
│   └── regedited_serve.sh        # Registry container helper (NEW)
├── references/                   # On-demand docs
│   ├── commands.md               # Full 43+ command reference
│   ├── encapsulation.md          # Three-mode system guide
│   ├── boolean_ops.md            # Boolean operations patterns
│   ├── wal_and_transactions.md   # WAL & transactions guide (NEW)
│   ├── schema_and_types.md       # Schema & typed values guide (NEW)
│   └── serve.md                  # Registry container guide (NEW)
├── assets/
│   └── template.md               # Empty Regedited document template
├── install.sh                    # Install script
└── README.md                     # This file
```

## Usage in Pi

Once installed, just mention "regedited" or describe the operation:

### Basic
> "Use regedited to scan this large markdown file and list all sections"
> "Check if this document contains both 'API' and 'examples'"
> "Extract all HREF attributes from this HTML as set variables"
> "Encapsulate this URL in store mode for a CMD script"

### WAL & Safety
> "Check the WAL status of this document for any crash recovery needed"
> "Replay uncommitted WAL entries to restore consistency"

### Transactions
> "Start a transaction on this file, make some changes, and commit them atomically"

### Schema
> "Generate a schema for this document and validate it against type constraints"

### Typed Values
> "Parse this configuration value as a REG_DWORD and show me the hex representation"

### Serve
> "Start a registry container for this config file on port 5000"

Pi loads the SKILL.md automatically and knows all 43+ commands.

## Feature Count

| Category | Commands |
|----------|----------|
| Core | list, scan, db, ascii, info, summary, content |
| Grep | fgrep, fgrep-multi, grep, zone-extract, zone-info, lines |
| Zone | zone-copy, zone-append, zone-replace |
| Write | set-num, set-str, set-zone, add, rm, new |
| Diff | diff, replace |
| Bool | bool-and, bool-nand, bool-or, bool-xor, count, if-contains |
| WAL | wal, wal-replay |
| Tx | tx |
| Schema | schema |
| Types | reg-types, reg-parse |
| Serve | serve |
| Encap | encap |
| HTML | grab-html |
| Util | types, convert, getutf, echo, echo-direct, clip |
| **Total** | **43+** |
