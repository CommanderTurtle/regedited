# Schema & Typed Values Reference

## Schema Enforcement

Optional per-section schemas for type-safe configuration. The Windows Registry has never had this.

### Schema File

Stored alongside the document as `document.md.schema`:

```text
# Regedited Schema v1
---
section Config
  field version    : string    : required
  field max_size   : int       : range(1, 1000000)
  field enabled    : bool      : default(true)
  field mode       : string    : one_of("auto", "manual", "hybrid")
  field path       : path      : required
  field tags       : array     : optional
---
section Code
  field language   : string    : required
  field version    : string    : default("stable")
  field features   : array     : optional
---
```

### Field Types

| Type | Validates | Example |
|------|-----------|---------|
| `string` | Any non-empty string | `"hello"` |
| `int` | Integer value | `42`, `-7` |
| `bool` | true/false/1/0/yes/no | `true` |
| `path` | Non-empty path string | `/etc/config` |
| `enum` | One of allowed values | Requires `one_of(a, b, c)` |
| `array` | Comma-separated values | `["a", "b"]` |
| `hex` | Hex data (0xNN format) | `0x48 0x65` |

### Constraints

| Constraint | Format | Effect |
|------------|--------|--------|
| `required` | `: required` | Field must be present |
| `optional` | `: optional` | Field may be missing |
| `range` | `: range(min, max)` | Integer must be in range |
| `one_of` | `: one_of(a, b, c)` | String must match one |
| `default` | `: default(value)` | Default if missing |

### Commands

```bash
# Generate starter schema from existing document
regedited schema document.md --init
# → Creates document.md.schema

# Validate document against schema
regedited schema document.md --validate
# → Shows errors if any fields violate schema

# Show schema without validating
regedited schema document.md
```

### Python Integration

```python
import subprocess

# Validate before applying changes
result = subprocess.run(
    ["regedited", "schema", "config.regd", "--validate"],
    capture_output=True, text=True
)
if "validation error" in result.stdout.lower():
    print("Schema validation failed — not applying changes")
    exit(1)

# Safe to apply
subprocess.run(["regedited", "set-num", "config.regd", "Config", "0", "42"])
```

---

## Typed Registry Values

Rich data types beyond plain strings. Windows Registry types plus Regedited extensions.

### Registry Types

| Type | Name | Storage | Example |
|------|------|---------|---------|
| `REG_SZ` | String | Plain text | `"hello world"` |
| `REG_DWORD` | u32 | Integer | `42` |
| `REG_QWORD` | u64 | Large integer | `9007199254740992` |
| `REG_BINARY` | Binary | Hex block | `0x48 0x65 0x6C` |
| `REG_MULTI_SZ` | String array | Null-separated | `["a", "b", "c"]` |
| `REG_EXPAND_SZ` | Expandable | Path with env vars | `%SYSTEMROOT%\system32` |
| `REG_JSON` | JSON | Structured data | `{"name":"test"}` |
| `REG_TOML` | TOML | Structured data | `name = "test"` |
| `REG_BOOL` | Boolean | Flag | `true` / `false` |

### Commands

```bash
# List all types
regedited reg-types

# Parse value as DWORD
regedited reg-parse "42" --reg-type REG_DWORD
# → 0x0000002A (42)

# Parse value as QWORD
regedited reg-parse "9007199254740992" --reg-type REG_QWORD
# → 0x0020000000000000 (9007199254740992)

# Parse JSON
regedited reg-parse '{"enabled":true,"count":42}' --reg-type REG_JSON
# → JSON: {"enabled":true,"count":42}

# Parse boolean
regedited reg-parse "yes" --reg-type REG_BOOL
# → true

# Parse hex
regedited reg-parse "0x48 0x65 0x6C 0x6C 0x6F" --reg-type REG_BINARY
# → 5 bytes: 0x48 0x65 0x6C 0x6C 0x6F
```

### From Python

```python
import subprocess

# Parse typed values for validation
def parse_typed(value, reg_type):
    result = subprocess.run(
        ["regedited", "reg-parse", value, "--reg-type", reg_type],
        capture_output=True, text=True
    )
    return result.returncode == 0, result.stdout

# Validate a DWORD
ok, output = parse_typed("42", "REG_DWORD")
print(output)  # 0x0000002A (42)

# Invalid value
check, _ = parse_typed("not_a_number", "REG_DWORD")
print(check)  # False
```
