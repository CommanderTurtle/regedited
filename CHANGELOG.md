# Regedited Changelog

# 2026-07-18 - QoL Update Part 2

### Added

- **Compact Command Aliases**: Complete shorthand alias table for all commands (e.g., `scan` → `s`, `clip` → `c`, `zone-append` → `za`).
- **rgd Invocation Mode**: Hard link/symlink to `regedited` executable enabling shorthand preprocessing and loaded-document context.
- **Loaded Document Context**: `rgd load <path>`, `rgd load`, and `rgd unload` commands for persistent user-level file path state.
- **Compact Reference Grammar**: Short-form index references (e.g., `i38` for `index:38`, `i38s1` for `index:38:string:1`).
- **Enhanced Convert Grammar**: Flexible conversion syntax accepting one to six values with inline type tokens (`p`/`b`/`m`/`d`) and trailing `clip`/`c` shortcuts.
- **Contextual Help System**: `-help` and `--help` flags after commands, aliases, and compact references with Clap-rendered usage.
- **Shell Script Exports**: `regedited -ex` command providing categorized references for PowerShell, REPL, Python, Bash, BAT, and custom scripts.
- **WebAssembly Browser Runner**: Read-only in-browser execution of core regedited functionality via `wasm-pack` and `runner.js`.
- **Cross-Platform Installation Scripts**: `pathadd.ps1` and `pathadd.sh` for idempotent PATH configuration and `rgd` link creation.
- **Web Build Automation**: `webbuild.ps1` and `webbuild.sh` scripts for Wasm package generation with prerequisite checks.

### Changed

- **Executable Structure**: Maintained single-target build (`regedited`) with `rgd` as hard link/symlink (Windows) or symlink (Unix).
- **Help Output**: `regedited --help` and `rgd --help` now provide differentiated documentation surfaces.
- **Documentation**: Extended help system now renders command-local arguments from Clap definitions, preventing drift from runtime behavior.

### Fixed

- **Canonical Command Preservation**: All existing canonical command spellings and accepted legacy forms remain fully functional.
- **Path Handling**: Correct treatment of non-leading file arguments in commands like `tx <action> <file>` and `serve --file`.

### Security

- **Read-Only Browser Mode**: Web runner intentionally restricted to in-memory operations; no file modification, transactions, or host clipboard access.
- **Installation Safety**: Build and install scripts validate prerequisites, request consent before installing tools, and fail fast on errors.

---

# 2026-07-18 QoL Update Part 1

### Added

- **Core QoL Feature Set**: Complete implementation of quality-of-life improvements based on upstream baseline commit `093c5e07fc365aec592aa1c3bfe103353ab5bc42`.

#### Architecture

- `regedited` builds exactly one executable target.
- `rgd` invocation detected via `argv[0]` enables shorthand mode without duplicating operational logic.
- Native clipboard support remains optional cargo feature for WebAssembly compatibility.

#### User Interface

- **Loaded State Management**: Persists user file path in `%LOCALAPPDATA%\regedited\loaded-path.txt` (Windows) or XDG state (Unix).
- **Incomplete Command Handling**: Automatically fills missing file argument with persisted path when appropriate.
- **Missing Path Error**: Clear message when no file specified and no path loaded.

#### Command Shortcuts

Complete alias table implemented in single runtime data structure (`src/qol.rs:14-284`):

```
list=l         scan=s         summary=sm
info=i         state=st       state-compare=stc
index-str-list=ist  index-zone-set-hex=izs  index-zone-extract=ize
index-zone-replace=izr  index-zone-copy=izc  index-zone-transfer=izt
db=db          hexline=hl     content=co
zone-info=zi   count=ct       clip=c
clip-zone=cz   clip-db=cdb    clip-dbline=cdbl
clip-hexline=chl  clip-hexword=chw  set-num=sn
set-str=ss     set-zone=sz    lines=ln
if-contains=if ref-get=rg     ref-set=rs
ref-copy=rc    ref-diff=rd    ref-bool=rb
resolve-index=ri  zone-append=za   diff=d
replace=r      fgrep=f        fgrep-multi=fm
zone-copy=zc   zone-replace=zr  zone-extract=ze
hex-extract=he  hex-replace=hr  undo=u
grep=g         echo=e         echo-direct=ed
getutf=gu      convert=cv     types=t
new=n          add=a          rm=rm
encap=en       grab-html=gh   bool-and=ba
bool-nand=bn   bool-or=bo     bool-xor=bx
wal=w          wal-replay=wr  tx=tx
schema=sc      reg-types=rt   reg-parse=rp
serve=sv
```

#### Reference System

Compact forms for all canonical reference types:

```
i38       → index:38
i38s1     → index:38:string:1
i38db9    → index:38:db:9
i38dbl    → index:38:dbline
i38hl     → index:38:hexline
i38hwl    → index:38:hex-word-line
i38rs     → index:38:ranges
i38r2     → index:38:range:2
i38z3     → index:38:zone:3
i38zh1    → index:38:zonehex:1
i38rh2    → index:38:rangehex:2
```

#### Converter Enhancements

- Accepts 1–6 line values without zero padding
- Inline type tokens (`p`/`b`/`m`/`d`) persist until changed
- Trailing `clip` or `c` copies exact output to clipboard
- Maintains backward compatibility with `-t/--zone-type` and `-z/--zone`

#### Documentation

- Embedded shell examples via `include_str!` in `src/main.rs`
- Verbose categorized references in `docs/shell/` covers:
  - PowerShell (`docs/shell/POWERSHELL.txt`)
  - Python (`docs/shell/PYTHON.txt`)
  - Bash (`docs/shell/BASH.txt`)
  - REPL (`docs/shell/REPL.txt`)
  - BAT (`docs/shell/BAT.txt`)
  - Custom scripts (`docs/shell/scripts/*.txt`)
- Browser documentation in `docs/web/JAVASCRIPT.txt` and `docs/web/STANDALONE_HTML.txt`

#### Build System

- `cargo build --release` builds optimized binary
- `pathadd.ps1` / `pathadd.sh` create `rgd` link and configure PATH
- `webbuild.ps1` / `webbuild.sh` build Wasm package with `wasm-pack`

---

## Testing & Validation

### Upstream Baseline

- 169 tests passed on clean baseline

### Post-Implementation

- `cargo fmt --check`: **passed**
- `cargo clippy --all-targets --all-features -- -D warnings`: **passed**
- `cargo test --all-targets`: **180 unit tests + 1 CLI integration test passed**
- `cargo build --release`: **passed**
- `wasm32-unknown-unknown` parent library (no default features): **passed**
- Web crate and optimized Wasm build: **passed without warnings**

### Integration Tests

- `tests/cli_qol.rs:26-123`: End-to-end testing of `rgd` hard link, load/unload, omitted paths, refs, and error messages
- `src/qol.rs:601-734`: Alias, compact-ref, global-flag, clipboard-suffix, and file-placement tests
- `src/converter.rs:81-152`: Conversion behavior and bounds testing
- Browser smoke test (`web/example/index.html`): Verified read-only operations over supplied text and page source

### Cross-Platform Validations

- PowerShell, CMD/BAT, Python, and Git Bash produced identical conversion output
- All ten `-ex` streams rendered successfully
- PowerShell launchers passed parser validation
- Bash launchers passed `bash -n` syntax check
- Windows hard link idempotence, PowerShell/CMD/Python lookup verified
- Unix symlink behavior with sudo and `.bashrc` handling confirmed
- Source-only ZIP extraction and build from short path verified

---

## Known Limitations

- **Loaded Path**: Inference never guesses over a complete canonical invocation; compact refs expand only unambiguous canonical forms; browser runner is read-only.
- **Installation**: Full native Linux installation not tested on Windows host; Windows clones should use ordinary short checkout path to avoid `LNK1104` path-length issues.

---

## Future Work

### README Updates (Pending Approval)

1. Add `rgd` section documenting hard link/symlink, `rgd load`/`unload`, and complete alias table
2. Replace fixed converter examples with 1–6 value grammar, inline type tokens, and `clip/c` suffix
3. Include build/install commands for `pathadd` and `webbuild` scripts
4. Link to verbose shell references in `docs/shell` and browser references in `docs/web`
5. Add browser-runner example with explicit read-only label

---

## Migration Notes

### Canonical Command Retention

All existing `regedited` commands remain fully functional. Users may adopt shorthand aliases incrementally without breaking existing workflows.

### Path Configuration

To install `regedited` and `rgd` in PATH:

```bash
cargo build --release
# Windows
.\pathadd.ps1
# Unix
./pathadd.sh
```

### Web Usage

To build the Wasm package:

```bash
# Windows
.\webbuild.ps1
# Unix
./webbuild.sh
```

See `docs/web/STANDALONE_HTML.txt` for standalone HTML usage.

---