# WAL & Transactions Reference

## Write-Ahead Log (WAL)

Every mutation is logged to a `.wal` file before touching the main document. On crash, replay restores consistency.

### WAL Line Format

```
SEQ|TIMESTAMP|OPERATION|SECTION|FIELD|OLD_VALUE|NEW_VALUE|CRC32
```

Example:
```
# REGEDITED WAL v1
# file: config.regd
---
1|1705312200|set-num|Config|0|42|99|a3f2c1d8
2|1705312200|set-str|Config|0|/old/path|/new/path|b7d4e2f1
---
COMMIT|1705312205|d8e7f6a5
```

### Commands

```bash
# Check WAL status (shows uncommitted entries / crash state)
regedited wal document.md

# Show what would be replayed (dry run)
regedited wal-replay document.md

# Actually replay and clean up
regedited wal-replay document.md --apply
```

### WAL is Automatic

Every `set-num`, `set-str`, `set-zone`, `add`, `rm`, `zone-replace` automatically writes to WAL. You only need `wal-replay` after a crash.

### Recovery Flow

```
Crash during write → .wal file exists, no COMMIT marker
Next open → regedited detects uncommitted WAL
User runs: regedited wal-replay file.md --apply
  → Each entry verified by CRC32
  → Operations applied in sequence
  → WAL file removed
  → Document is consistent
```

---

## Transactions

Batch multiple operations into a single atomic unit. All-or-nothing semantics.

### Commands

```bash
# Begin a transaction (creates WAL if not exists)
regedited tx begin document.md

# Make changes (all staged, not yet applied)
regedited set-num document.md Config 0 42
regedited set-str document.md Config 0 "/new/path"
regedited set-zone document.md Config 0 10 50 --zone-type code

# Commit all at once (atomic)
regedited tx commit document.md

# Or discard everything
regedited tx rollback document.md

# Check what's staged
regedited tx status document.md
```

### Transaction States

| State | Meaning |
|-------|---------|
| `Started` | Transaction created, no ops staged |
| `Staging` | Ops staged, WAL logged, not applied |
| `Committed` | All ops applied, WAL cleaned |
| `RolledBack` | All ops discarded, WAL removed |

### Example: Safe Configuration Update

```bash
#!/bin/bash
set -e

FILE="config.regd"
regedited tx begin "$FILE"

# If any of these fail, the transaction aborts
regedited set-num "$FILE" "Server" 0 8080
regedited set-str "$FILE" "Server" 0 "production"
regedited set-zone "$FILE" "Server" 0 100 200 --zone-type code

# All or nothing
regedited tx commit "$FILE"
echo "Configuration updated atomically"
```

### Why This Matters

The Windows Registry cannot do transactional batch edits. If a Group Policy update sets 50 keys and fails on #47, the system is left inconsistent. Regedited guarantees all-or-nothing.
