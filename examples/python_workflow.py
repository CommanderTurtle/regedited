#!/usr/bin/env python3
"""Example: Maintain absolute code ranges with Regedited.

This demonstrates the complete Regedited Python workflow:
- Scanning indexes
- Extracting zone content
- Appending new code
- Copying between zones
- Getting machine-readable metadata
"""

import subprocess
import shutil

REGEDITED = shutil.which("regedited") or "../target/release/regedited"


def run(*args, **kwargs):
    """Run regedited command, return stdout."""
    result = subprocess.run(
        [REGEDITED, *args],
        capture_output=True, text=True,
        **kwargs
    )
    if result.returncode != 0:
        print(f"Error running regedited {' '.join(args)}:")
        print(f"  stderr: {result.stderr}")
        raise RuntimeError(f"regedited {' '.join(args)} failed")
    return result.stdout


def get_zone_content(path: str, index: str, zone: int) -> str:
    """Extract raw zone content."""
    return run("zone-extract", path, index, str(zone))


def set_zone_content(path: str, index: str, zone: int, content: str):
    """Replace zone content entirely."""
    subprocess.run(
        [REGEDITED, "zone-replace", path, index, str(zone), "--text", content],
        check=True
    )


def append_zone_content(path: str, index: str, zone: int, content: str):
    """Append content to a zone."""
    subprocess.run(
        [REGEDITED, "zone-append", path, index, str(zone), "--text", content],
        check=True
    )


def copy_zone(path: str, from_index: str, from_zone: int, to_index: str, to_zone: int):
    """Copy zone content between indexes."""
    subprocess.run(
        [REGEDITED, "zone-copy", path,
         "--from", from_index, "--from-zone", str(from_zone),
         "--to", to_index, "--to-zone", str(to_zone)],
        check=True
    )


def list_indexes(path: str) -> str:
    """List all indexes."""
    return run("list", path)


def scan_indexes(path: str, index_filter: str = None) -> str:
    """Scan indexes with an optional numeric filter."""
    args = ["scan", path]
    if index_filter:
        args.extend(["--filter", index_filter])
    return run(*args)


def get_zone_info(path: str, index: str, zone: int) -> dict:
    """Get machine-readable zone metadata."""
    output = run("zone-info", path, index, str(zone))
    info = {}
    content_lines = []
    in_content = False
    for line in output.strip().split("\n"):
        if line == "---CONTENT---":
            in_content = True
            continue
        if in_content:
            content_lines.append(line)
        elif "=" in line:
            key, value = line.split("=", 1)
            info[key] = value
    info["content"] = "\n".join(content_lines)
    return info


def update_number(path: str, registry_index: str, slot: int, value: str):
    """Update a database value."""
    run("set-num", path, registry_index, str(slot), value)


def update_string(path: str, registry_index: str, slot: int, value: str):
    """Update a string value."""
    run("set-str", path, registry_index, str(slot), value)


def update_zone(path: str, registry_index: str, zone: int, start: int, end: int, zone_type: str = "markdown"):
    """Update a zone range with type."""
    run("set-zone", path, registry_index, str(zone), str(start), str(end),
        "--zone-type", zone_type)


def diff_files(path_a: str, path_b: str) -> str:
    """Diff two files."""
    return run("diff", path_a, path_b)


def replace_indexes(target: str, source: str, output: str = None, indexes: list = None):
    """Replace fixed index records from source into target."""
    args = ["replace", target, source]
    if output:
        args.extend(["-o", output])
    if indexes:
        args.append("--indexes")
        args.extend(indexes)
    run(*args)


def fast_grep(path: str, pattern: str, index: str = None) -> str:
    """Fast grep."""
    args = ["fgrep", path, pattern]
    if index:
        args.extend(["--index", index])
    return run(*args)


# ============== EXAMPLE WORKFLOW ==============

if __name__ == "__main__":
    DOC = "example.md"

    print("=" * 60)
    print("1. LIST ALL INDEXES")
    print("=" * 60)
    print(list_indexes(DOC))

    print("=" * 60)
    print("2. SCAN WITH FILTER")
    print("=" * 60)
    print(scan_indexes(DOC, "200"))

    print("=" * 60)
    print("3. GET ZONE INFO")
    print("=" * 60)
    info = get_zone_info(DOC, "i200", 1)
    print(f"Zone: {info['zone_index']}")
    print(f"Lines: {info['start_line']}-{info['end_line']}")
    print(f"Type: {info['zone_type']}")
    print(f"Type nibble: {info['type_nibble']}")
    print(f"Line count: {info['line_count']}")
    print(f"Byte size: {info['byte_size']}")

    print("=" * 60)
    print("4. EXTRACT ZONE CONTENT")
    print("=" * 60)
    code = get_zone_content(DOC, "i200", 1)
    print(f"Extracted {len(code)} bytes, {code.count(chr(10))} lines")
    print(code[:500])

    print("=" * 60)
    print("5. APPEND NEW FUNCTION")
    print("=" * 60)
    new_func = '''

## New Function

```rust
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
```
'''
    append_zone_content(DOC, "i200", 1, new_func)
    print("Appended successfully!")

    print("=" * 60)
    print("6. VERIFY (extract again)")
    print("=" * 60)
    updated = get_zone_content(DOC, "i200", 1)
    print(f"Now {len(updated)} bytes, {updated.count(chr(10))} lines")
    assert "greet" in updated, "New function should be in content"
    print("Verified!")

    print("=" * 60)
    print("7. FAST GREP")
    print("=" * 60)
    print(fast_grep(DOC, "fn "))

    print("=" * 60)
    print("All operations completed successfully!")
    print("=" * 60)
