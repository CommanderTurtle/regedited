#!/usr/bin/env python3
"""Example: Maintain a code snippets database with Regedited.

This demonstrates the complete Regedited Python workflow:
- Scanning sections
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


def get_zone_content(path: str, section: str, zone: int) -> str:
    """Extract raw zone content."""
    return run("zone-extract", path, section, str(zone))


def set_zone_content(path: str, section: str, zone: int, content: str):
    """Replace zone content entirely."""
    subprocess.run(
        [REGEDITED, "zone-replace", path, section, str(zone), "--text", content],
        check=True
    )


def append_zone_content(path: str, section: str, zone: int, content: str):
    """Append content to a zone."""
    subprocess.run(
        [REGEDITED, "zone-append", path, section, str(zone), "--text", content],
        check=True
    )


def copy_zone(path: str, from_sec: str, from_zone: int, to_sec: str, to_zone: int):
    """Copy zone content between sections."""
    subprocess.run(
        [REGEDITED, "zone-copy", path,
         "--from", from_sec, "--from-zone", str(from_zone),
         "--to", to_sec, "--to-zone", str(to_zone)],
        check=True
    )


def list_sections(path: str) -> str:
    """List all sections."""
    return run("list", path)


def scan_sections(path: str, name_filter: str = None) -> str:
    """Scan sections with optional name filter."""
    args = ["scan", path]
    if name_filter:
        args.extend(["--filter", name_filter])
    return run(*args)


def get_zone_info(path: str, section: str, zone: int) -> dict:
    """Get machine-readable zone metadata."""
    output = run("zone-info", path, section, str(zone))
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


def update_number(path: str, section: str, index: int, value: int):
    """Update a database value."""
    run("set-num", path, section, str(index), str(value))


def update_string(path: str, section: str, index: int, value: str):
    """Update a string value."""
    run("set-str", path, section, str(index), value)


def update_zone(path: str, section: str, zone: int, start: int, end: int, zone_type: str = "markdown"):
    """Update a zone range with type."""
    run("set-zone", path, section, str(zone), str(start), str(end),
        "--zone-type", zone_type)


def diff_files(path_a: str, path_b: str) -> str:
    """Diff two files."""
    return run("diff", path_a, path_b)


def replace_sections(target: str, source: str, output: str = None, sections: list = None):
    """Replace sections from source into target."""
    args = ["replace", target, source]
    if output:
        args.extend(["-o", output])
    if sections:
        args.append("-s")
        args.extend(sections)
    run(*args)


def fast_grep(path: str, pattern: str, section: str = None) -> str:
    """Fast grep."""
    args = ["fgrep", path, pattern]
    if section:
        args.extend(["--section", section])
    return run(*args)


# ============== EXAMPLE WORKFLOW ==============

if __name__ == "__main__":
    DOC = "example.md"

    print("=" * 60)
    print("1. LIST ALL SECTIONS")
    print("=" * 60)
    print(list_sections(DOC))

    print("=" * 60)
    print("2. SCAN WITH FILTER")
    print("=" * 60)
    print(scan_sections(DOC, "Code"))

    print("=" * 60)
    print("3. GET ZONE INFO")
    print("=" * 60)
    info = get_zone_info(DOC, "CodeSnippets", 1)
    print(f"Zone: {info['zone_index']}")
    print(f"Lines: {info['start_line']}-{info['end_line']}")
    print(f"Type: {info['zone_type']}")
    print(f"Type nibble: {info['type_nibble']}")
    print(f"Line count: {info['line_count']}")
    print(f"Byte size: {info['byte_size']}")

    print("=" * 60)
    print("4. EXTRACT ZONE CONTENT")
    print("=" * 60)
    code = get_zone_content(DOC, "CodeSnippets", 1)
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
    append_zone_content(DOC, "CodeSnippets", 1, new_func)
    print("Appended successfully!")

    print("=" * 60)
    print("6. VERIFY (extract again)")
    print("=" * 60)
    updated = get_zone_content(DOC, "CodeSnippets", 1)
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
