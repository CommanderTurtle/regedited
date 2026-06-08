# Python Scripting Guide

See [ARCHITECTURE.md](ARCHITECTURE.md#python-integration) for the complete Python integration guide with examples.

This file is kept for backward compatibility. All documentation has been consolidated into ARCHITECTURE.md.
 Python calls. This mirrors how a safetensors workflow works — Python orchestrates, Rust performs.

## Installation

```bash
# Clone and build
git clone https://github.com/CommanderTurtle/regedited.git
cd regedited
cargo build --release

# Add to PATH or use absolute path
cp target/release/regedited ~/.local/bin/
```

## Basic Usage

```python
import subprocess
import shutil

REGEDITED = shutil.which("regedited") or "./target/release/regedited"

def regedited(*args):
    """Call regedited with arguments, return stdout."""
    result = subprocess.run(
        [REGEDITED, *args],
        capture_output=True,
        text=True,
        check=True
    )
    return result.stdout
```

## Section Scanning

```python
# Fast scan — reads only metadata, not content
output = regedited("scan", "document.md")
print(output)
# Scan: 4 sections in document.md (safetensors-style header scan)
#   [ 100] ProjectConfig     DB:[1 100 50 200 25 75 10 20 30] Zones:[none] Lines:21
#   [ 200] CodeSnippets      DB:[42 7 3 256 1024 4096 100 200 300] Zones:[Z1:60..66[CODE]] Lines:35

# Filtered scan
output = regedited("scan", "document.md", "--filter", "Code")

# Scan with value filter
output = regedited("scan", "document.md", "--value", "0:10:100")
```

## Zone Content Extraction

```python
# Extract zone content to a variable
result = subprocess.run(
    [REGEDITED, "zone-extract", "document.md", "CodeSnippets", "1"],
    capture_output=True, text=True, check=True
)
code_block = result.stdout
print(f"Extracted {len(code_block)} bytes")

# Extract zone info (machine-readable)
result = subprocess.run(
    [REGEDITED, "zone-info", "document.md", "CodeSnippets", "1"],
    capture_output=True, text=True, check=True
)

# Parse the output
info = {}
for line in result.stdout.strip().split('\n'):
    if line == '---CONTENT---':
        break
    if '=' in line:
        key, value = line.split('=', 1)
        info[key] = value

print(f"Zone: {info['zone_index']}")
print(f"Lines: {info['start_line']}-{info['end_line']}")
print(f"Type: {info['zone_type']}")  # CODE
print(f"Line count: {info['line_count']}")
```

## Zone Content Manipulation

### Copy Between Sections

```python
# Copy zone 1 from CodeSnippets to zone 0 of MySection
subprocess.run(
    [REGEDITED, "zone-copy", "document.md",
     "--from", "CodeSnippets", "--from-zone", "1",
     "--to", "MySection", "--to-zone", "0"],
    check=True
)
```

### Append Content

```python
# Append from a Python string
subprocess.run(
    [REGEDITED, "zone-append", "document.md", "CodeSnippets", "1",
     "--text", "\n## New Section\n\nNew content here."],
    check=True
)

# Append from a file
with open("new_functions.md", "r") as f:
    subprocess.run(
        [REGEDITED, "zone-append", "document.md", "CodeSnippets", "1"],
        stdin=f, check=True
    )

# Append from a Python variable via stdin
content = """
## Utility Functions

```rust
pub fn new_util() -> String {
    "utility".to_string()
}
```
"""
subprocess.run(
    [REGEDITED, "zone-append", "document.md", "CodeSnippets", "1"],
    input=content, text=True, check=True
)
```

### Replace Content

```python
# Replace zone content entirely
new_content = """```rust
fn main() {
    let x = 42;
    println!("{}", x);
}
```
"""
subprocess.run(
    [REGEDITED, "zone-replace", "document.md", "CodeSnippets", "1",
     "--text", new_content],
    check=True
)
```

### Pipe Between Zones

```python
# Extract from one zone, transform in Python, append to another
result = subprocess.run(
    [REGEDITED, "zone-extract", "document.md", "CodeSnippets", "1"],
    capture_output=True, text=True, check=True
)
extracted = result.stdout

# Transform in Python
transformed = extracted.replace("fn main", "pub fn main")

# Write to another zone
subprocess.run(
    [REGEDITED, "zone-replace", "document.md", "ExportedCode", "0",
     "--text", transformed],
    check=True
)
```

## Database Value Updates

```python
# Update specific values
regedited("set-num", "document.md", "ProjectConfig", "0", "999")
regedited("set-num", "document.md", "ProjectConfig", "5", "42")

# Update strings
regedited("set-str", "document.md", "ProjectConfig", "0", "/home/user/project")
regedited("set-str", "document.md", "ProjectConfig", "2", "https://github.com/user/repo")

# Update zone range with type
regedited("set-zone", "document.md", "CodeSnippets", "1", "60", "66",
       "--zone-type", "code")
```

## Diff and Replace (Safetensors-Style)

```python
# Diff two files
output = regedited("diff", "base.md", "patched.md")
print(output)

# Replace all matching sections from source into target
subprocess.run(
    [REGEDITED, "replace", "base.md", "patched.md", "-o", "result.md"],
    check=True
)

# Replace specific sections only
subprocess.run(
    [REGEDITED, "replace", "base.md", "patched.md",
     "-s", "CodeSnippets", "Documentation",
     "-o", "result.md"],
    check=True
)
```

## Fast Grep

```python
# Grep entire file
result = subprocess.run(
    [REGEDITED, "fgrep", "document.md", "fn "],
    capture_output=True, text=True, check=True
)
for line in result.stdout.strip().split('\n')[2:]:  # Skip header
    if line.startswith('  '):
        print(line.strip())

# Grep within a section
result = subprocess.run(
    [REGEDITED, "fgrep", "document.md", "fn ", "--section", "CodeSnippets"],
    capture_output=True, text=True, check=True
)

# Multi-pattern grep
result = subprocess.run(
    [REGEDITED, "fgrep-multi", "document.md", "rust", "fn", "struct"],
    capture_output=True, text=True, check=True
)
```

## Complete Workflow Example

```python
#!/usr/bin/env python3
"""Example: Maintain a code snippets database with Regedited."""

import subprocess
import shutil

REGEDITED = shutil.which("regedited") or "./target/release/regedited"

def run(*args, **kwargs):
    """Run regedited command."""
    result = subprocess.run(
        [REGEDITED, *args],
        capture_output=True, text=True,
        **kwargs
    )
    if result.returncode != 0:
        print(f"Error: {result.stderr}")
        raise RuntimeError(f"regedited {' '.join(args)} failed")
    return result.stdout

def get_zone_content(path: str, section: str, zone: int) -> str:
    """Extract zone content."""
    return run("zone-extract", path, section, str(zone))

def set_zone_content(path: str, section: str, zone: int, content: str):
    """Replace zone content."""
    subprocess.run(
        [REGEDITED, "zone-replace", path, section, str(zone), "--text", content],
        check=True
    )

def append_zone_content(path: str, section: str, zone: int, content: str):
    """Append to zone content."""
    subprocess.run(
        [REGEDITED, "zone-append", path, section, str(zone), "--text", content],
        check=True
    )

def list_sections(path: str):
    """List all sections."""
    return run("list", path)

def scan_sections(path: str, name_filter: str = None):
    """Scan sections with optional name filter."""
    args = ["scan", path]
    if name_filter:
        args.extend(["--filter", name_filter])
    return run(*args)

# Example workflow
if __name__ == "__main__":
    DOC = "snippets.md"
    
    # List all sections
    print("=== Sections ===")
    print(list_sections(DOC))
    
    # Get the main code block
    print("=== Main Code ===")
    code = get_zone_content(DOC, "CodeSnippets", 1)
    print(code[:500])
    
    # Append a new function
    print("\n=== Appending new function ===")
    new_func = '''

pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
'''
    append_zone_content(DOC, "CodeSnippets", 1, new_func)
    
    # Verify
    print("=== Updated code ===")
    updated = get_zone_content(DOC, "CodeSnippets", 1)
    print(f"Now {len(updated)} bytes")
```

## Error Handling

All commands exit with non-zero status on error:

```python
try:
    subprocess.run([REGEDITED, "grep", "doc.md", "Nonexistent", "0"], check=True)
except subprocess.CalledProcessError as e:
    print(f"Error: {e.stderr}")
    # Error: Section 'Nonexistent' not found
```

Common errors:
- `Section 'X' not found` — section name doesn't exist
- `Zone index N out of range (0-2)` — zone index must be 0, 1, or 2
- `Zone N is empty (0x00000000 : 0x00000000)` — zone has no content to extract
- `Hex-word line must have 6 values separated by ' : '` — malformed hex-word store
