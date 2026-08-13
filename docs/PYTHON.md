# Python Scripting Guide

Python should orchestrate the compiled Regedited process. Regedited keeps the
file grammar, exact-decimal handling, zone relocation, and crash-safety logic;
Python supplies ordinary control flow.

The executable is also its own example catalog:

```powershell
rgd --help --ex 3
rgd rg --help --ex 3
rgd rb --help --ex 3
```

Shell number `3` always means Python. See
[`shell/PYTHON.txt`](shell/PYTHON.txt) for the complete subprocess cookbook.

## Process Wrapper

```python
from pathlib import Path
import shutil
import subprocess

RGD = shutil.which("rgd") or shutil.which("regedited")
if RGD is None:
    raise RuntimeError("Build Regedited and place rgd or regedited on PATH")

DOC = Path("document.md")

def run(*args: object, input_text: str | None = None) -> str:
    completed = subprocess.run(
        [RGD, *(str(arg) for arg in args)],
        input=input_text,
        capture_output=True,
        text=True,
        check=True,
    )
    return completed.stdout
```

## Canonical Index Reads

The exact lowercase substring `regedited open` identifies a record marker.
Its following six lines are the complete record. Numeric `index: N` is the
identity; surrounding document text is shared and only absolute zones bound
content.

```python
print(run("list", DOC))
print(run("scan", DOC))
print(run("db", DOC, "i64"))
print(run("hexline", DOC, "i64"))
print(run("index-str-list", DOC, 64))

second_string = run("ref-get", DOC, "index:64:string:2").rstrip("\n")
seventh_decimal = run("ref-get", DOC, "index:64:db:7").rstrip("\n")
```

`content` validates the selected index, then returns the complete shared
document. It does not infer an index-owned body.

```python
shared_document = run("content", DOC, "i64")
```

## Exact Decimals and Boolean Routing

DB values are exact fixed-point decimals. They are not coerced through a
binary floating-point value.

```python
run("set-num", DOC, "i64", 0, "0.102000000")

decision = run(
    "ref-bool", DOC,
    "i64db1", "lte", "i70db4",
    "--then-val", "1",
    "--else-val", "0",
).strip()

if decision == "1":
    pair = run("convert", "b", 58, 59).strip()
    run("zone-append", DOC, "i64", 1, "--text", pair)
```

## Zone Reads and Writes

The older direct zone commands use zero-based slots `0..2`. Native compact
references such as `i64z1` use human-facing zones `1..3`.

```python
raw = run("zone-extract", DOC, "i64", 0)
metadata = run("zone-info", DOC, "i64", 0)

run("zone-replace", DOC, "i64", 0, input_text="replacement\ntext\n")
run("zone-append", DOC, "i64", 0, input_text="appended line\n")

# Copy absolute zone content between numeric indexes.
run(
    "zone-copy", DOC,
    "--from", "i64", "--from-zone", 0,
    "--to", "i70", "--to-zone", 1,
)

# The native-ref equivalent uses one-based zones.
run("ref-copy", DOC, "i64z1", "i70z2")
```

Any line range can be addressed independently of an index:

```python
print(run("hex-extract", DOC, "1x000003A", "1x0000040"))
print(run("lines", DOC, 58, 64))
```

## Search

An index-scoped `fgrep` validates the numeric index and searches the shared
document. The legacy `--section` option spelling remains an alias for
`--index`; it does not create a content section.

```python
print(run("fgrep", DOC, "TODO"))
print(run("fgrep", DOC, "TODO", "--index", "i64"))
print(run("fgrep-multi", DOC, "TODO", "FIXME", "SAFETY"))
```

## Guarded Relocation

```python
run("commit", DOC)       # save the current compact checkpoint
# An external editor moves lines here.
print(run("check", DOC)) # calculate the guarded relocation proposal
print(run("pull", DOC))  # apply only the safe proposal
```

`commit --pull` combines checking and safe application. Checkpoints store
zone fingerprints and anchors, not document history.

## Metadata Diff, Replacement, and State

```python
print(run("diff", "base.md", "donor.md"))
run(
    "replace", "base.md", "donor.md",
    "--indexes", "i64", "i70",
    "--output", "patched.md",
)

Path("state.json").write_text(run("state", DOC), encoding="utf-8")
print(run("state-compare", DOC, "state.json"))
```

Replacement patches only the marker and six fixed metadata lines for matching
numeric indexes. It never invents or transfers an implicit body.

## Error Handling

```python
try:
    run("ref-get", DOC, "index:999:db:1")
except subprocess.CalledProcessError as error:
    print(error.stderr)
```

All command failures use a nonzero exit status. Typical causes are a missing
numeric index, an undefined zone pair, an out-of-range slot, malformed
hex-words, or an invalid exact-decimal literal.
