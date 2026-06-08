# Regedited Command Reference

## Document Inspection

| Command | Args | Description |
|---------|------|-------------|
| `list` | `<file>` | List all sections |
| `scan` | `<file> [--filter <pat>] [--value <i:min:max>]` | Header-only scan |
| `db` | `<file> <section>` | Show database table |
| `ascii` | `<file> <section>` | Show hex-word store |
| `info` | `<file>` | Full document info |
| `summary` | `<file>` | Document summary |
| `content` | `<file> <section>` | Section markdown content |
| `wal` | `<file>` | Show WAL status |

## Grep & Extract

| Command | Args | Description |
|---------|------|-------------|
| `fgrep` | `<file> <pattern> [-s <section>]` | Memory-mapped grep |
| `fgrep-multi` | `<file> <p1> <p2>...` | Multi-pattern OR grep |
| `grep` | `<file> <section> <zone>` | Extract zone by index |
| `zone-extract` | `<file> <section> <zone>` | Raw zone to stdout |
| `zone-info` | `<file> <section> <zone>` | Machine-readable zone meta |
| `lines` | `<file> <start> <end>` | Arbitrary line range |

## Zone Manipulation

| Command | Args | Description |
|---------|------|-------------|
| `zone-copy` | `<file> -f <S> -m <n> -t <T> -n <n>` | Copy zone content |
| `zone-append` | `<file> <S> <z> [--text <t>]` | Append to zone (or stdin) |
| `zone-replace` | `<file> <S> <z> [--text <t>]` | Replace zone (or stdin) |

## Boolean Operations

| Command | Args | Exit 0 when |
|---------|------|-------------|
| `bool-and` | `<file> <S> <p1> [p2]...` | ALL patterns found |
| `bool-nand` | `<file> <S> <must> <mustnot>` | Contains must, NOT mustnot |
| `bool-or` | `<file> <S> <p1> [p2]...` | ANY pattern found |
| `bool-xor` | `<file> <S> <a> <b>` | Exactly ONE found |
| `count` | `<file> <S> <pattern>` | Always 0 (shows count) |
| `if-contains` | `<file> <S> <p> [--then <v>] [--else <v>]` | Always 0 (prints value) |

## Write

| Command | Args | Description |
|---------|------|-------------|
| `set-num` | `<file> <S> <i> <v>` | Update numeric value (0-8) |
| `set-str` | `<file> <S> <i> <v>` | Update string (0-2) |
| `set-zone` | `<file> <S> <z> <s> <e> [-t <type>]` | Update zone range+type |
| `add` | `<file> <section>` | Add new section |
| `rm` | `<file> <section>` | Remove section |
| `new` | `<file> <title>` | Create new document |

## WAL (Crash Safety)

| Command | Args | Description |
|---------|------|-------------|
| `wal` | `<file>` | Show WAL status |
| `wal-replay` | `<file> [--apply]` | Replay uncommitted WAL |

## Transactions (Batch Atomicity)

| Command | Args | Description |
|---------|------|-------------|
| `tx` | `<begin\|commit\|rollback\|status> <file>` | Transaction control |

## Schema (Type Enforcement)

| Command | Args | Description |
|---------|------|-------------|
| `schema` | `<file> [--validate] [--init]` | Show/validate/create schema |

## Typed Values (Registry Types)

| Command | Args | Description |
|---------|------|-------------|
| `reg-types` | | List all registry types |
| `reg-parse` | `<value> --reg-type <type>` | Parse as typed value |

## Serve (Registry Container)

| Command | Args | Description |
|---------|------|-------------|
| `serve` | `--file <f> [--port <n>] [--read-only <b>]` | HTTP server |

## Encapsulation (shel.sh/XML)

| Command | Args | Description |
|---------|------|-------------|
| `encap` | `<text> [-m b/c/d] [--extract] [--to <m>] [--set <v>]` | Encapsulate/extract/convert |

## HTML Extraction

| Command | Args | Description |
|---------|------|-------------|
| `grab-html` | `<file> <attr> [-m b/c/d] [--tag <t>] [--set <b>] [-n]` | Extract HTML attrs |

## Utility

| Command | Args | Description |
|---------|------|-------------|
| `types` | | List zone types |
| `convert` | `<start> <end> [-t <type>]` | Range to hex-words |
| `getutf` | `<number> [--decode <hex>]` | DWORD encode/decode |
| `echo` | `<file> <S> <i>` | Safe echo string |
| `echo-direct` | `<text>` | Safe echo raw text |
| `clip` | `<file> <S> <i>` | Copy string to clipboard |

## Diff & Replace

| Command | Args | Description |
|---------|------|-------------|
| `diff` | `<a> <b>` | Metadata-only diff |
| `replace` | `<target> <source> [-o <out>] [-s <s1> <s2>]` | Patch sections |
