# Regedited Quick Start

Regedited is a Rust CLI for treating one plaintext/markdown file as a fast indexed registry. It scans canonical `regedited open` trigger lines or compatible `## SECTION:` headers, then reads, writes, diffs, copies, and serves typed string, DB, and hex-word zone refs.

## 1. Install Rust

Windows:

```powershell
winget install --source winget --id Rustlang.Rustup
```

Close and reopen the shell, then verify:

```powershell
rustc --version
cargo --version
rustup --version
```

If Windows reports a linker error such as `link.exe` not found, install Visual Studio Build Tools with the `Desktop development with C++` workload.

Linux/macOS:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustc --version
cargo --version
```

## 2. Build and Test

```bash
cargo fmt --check
cargo test
cargo build --release
```

Windows binary:

```powershell
.\target\release\regedited.exe --help
```

Linux/macOS binary:

```bash
./target/release/regedited --help
```

## 3. Minimal Registry Fixture

```markdown
anything before regedited open anything after is ignored
index: 1
0x0000008 : 0x0000008 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000
1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
First string line
Second string line
Third string line
---
Hello from a zone.
```

The trigger line has no name. The address is the following numeric index:

```bash
regedited ref-get doc.md index:1:string:1
regedited ref-get doc.md index:1:db:7
regedited ref-get doc.md index:1:zone:1
regedited ref-get doc.md index:1:zone:1 --clip
```

## 4. Read More

- `docs/RUST_BEGINNER_SETUP.txt` - slower first-time Rust setup
- `docs/shell/POWERSHELL.txt` - PowerShell command cookbook
- `docs/shell/BASH.txt` - Bash command cookbook
- `docs/shell/PYTHON.txt` - Python subprocess cookbook
- `docs/ARCHITECTURE.md` - full internals and command reference
- `docs/FORMAT.md` - minimal document format

