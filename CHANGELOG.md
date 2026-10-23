# Regedited Changelog

---

# 2026-10-23 - Zone Templates & Optional DOM Bindings

- The big browser push: the Wasm package can now render registry data directly into a page. A new optional `dom.js` module binds marked elements (`data-rgd-ref`, `data-rgd-template`) to live registry fields and zones, with a small template language (`%s`, `{{iN...}}`, `{{$var}}`) for zone-backed content.

- `refText` (`rgd.refText()`) returns exact decimal text with no `Number` rounding, and zone refs (`iNz1`, `iNzh1`) are now readable from the browser.

- `rgd serve --cors` plus a new `GET /document` endpoint. `createLocalRunner()` in the browser fetches a one-shot snapshot of the served registry and runs the same in-memory Wasm API against it. No polling, no native execution.

- Linux clipboard now uses session-native helpers (`wl-copy` / `wl-paste`, `xclip`) so clipboard ownership survives after the CLI exits. Wayland supported.

- Linux install/build scripts hardened and made executable (smaller 2026-10-20 commit).

- Also, since all of PHP collapsed this week (PHP2Shell RCE, and a third unpatched flaw disclosed on top of it): "switch to Next.js," they said. Well, now you can switch to Regedited — the whole web package runs client-side in Wasm, and there's no server to shell into.

---

## Testing/Advanced Info:

This pass adds the optional DOM binding layer and the pieces that feed it:
exact-text refs, a served-document snapshot endpoint, and a one-shot local
runner. The core runner API is unchanged, everything is read-only, and the
module is dependency-free.

### DOM Bindings (`web/dom.js`, new)

- Optional module shipped as `pkg/dom.js` + `dom.d.ts`. Works with any runner
  source (supplied text, File/Blob, page source, local snapshot); existing
  runner methods are unchanged.
- `bindRegistry(runner, root = document, { variables, sanitizeHTML })`
  returns `{ render(variables?), dispose() }`:
  - `render()` re-reads the current runner snapshot, merges supplied
    variables, and returns the number of bound elements.
  - All refs are resolved and every binding validated before the live DOM is
    touched; one per-render reference cache.
  - Only marked elements are modified; the outermost binding owns its whole
    subtree.
  - `dispose()` releases the binding, leaves rendered DOM intact, and does
    not dispose the runner.
  - Document, Element (including itself), and DocumentFragment roots
    supported. No timers, observers, background fetches, or automatic
    refresh.
- Element markers:
  - `data-rgd-ref="i38s1"` — literal field, template tokens not interpreted.
  - `data-rgd-template="i38z1"` — opts into substitution.
  - `data-rgd-mode` — `text` (default, `textContent`), `value` (form
    control), `html`.
  - `data-rgd-args='["i38s1","i38db9"]'` — JSON array consumed by `%s` in
    order.
- Template grammar (zone content is data, not code):
  - `%s` consumes one ref from the argument array; `%%` prints a literal
    percent sign.
  - Named tokens: `{{i38s1}}`, `{{index:38:db:9}}`, and `{{$variable}}`
    (variables accept strings, numbers, bigints, booleans, and null).
  - No eval, no expressions, no recursive expansion; unrelated `{{...}}`
    tokens are left alone.
  - Substitutions are HTML-escaped first. `html` mode requires an
    application-supplied synchronous `sanitizeHTML` callback (may return a
    string or native TrustedHTML); none is bundled.
  - Active targets (SCRIPT, STYLE, IFRAME, OBJECT, EMBED, LINK, META, BASE)
    are rejected; file inputs cannot be value-bound.
- `formatTemplate(runner, template, { args, variables, escapeHTML })` is the
  pure formatter: no DOM required, worker-safe.

### Exact-Text Refs and Zone Reads (Wasm)

- New `refText(reference)` / `ref_text`: exact text without building the
  full index JSON or rounding DB values. Accepts `string`, `db`, `dbline`,
  `hexline`, `hex-word-line`, `zone`, and `zonehex` refs.
- Zone reads validate bounds: empty `0:0` zones return empty text; reversed
  or out-of-document bounds throw.
- `refGet` now also accepts `iNz1` (zone content as text) and `iNzh1`
  (zonehex → `"start : end"`); existing numeric/object results unchanged.
- `refText` on `iNdbM` returns the original literal decimal (e.g.
  `9007199254740993`), bypassing `Number` entirely.

### Local Registry Loading (browser)

- `rgd serve --cors`: anonymous CORS is now opt-in (the server previously
  always sent CORS headers). `OPTIONS` is answered with `204`. A unit test
  pins header presence to the flag.
- New `GET /document` on the served instance: exact plaintext snapshot as
  `text/plain; charset=utf-8`.
- `createLocalRunner(baseUrl = "http://localhost:5000", { wasm, signal,
  headers })`: one `GET /document` (cors mode, credentials omitted,
  no-store), validates HTTP(S) without embedded credentials and a
  `text/plain` content type, then hands off to the same in-memory Wasm API.
  No polling, no native execution.
- `createPageRunner` accepts `wasm` + `signal`; browser-only paths guard
  against worker use.
- `web/package.json` added: `regedited-web` v0.2.0 (ESM,
  `sideEffects: false`, explicit file list) so the built `pkg/` directory is
  an installable package.
- `runner.d.ts` now types the full surface; `runner.js` wraps Wasm calls in
  clean errors, guards post-dispose use, and `initRegedited` dedupes
  concurrent loads.

### Linux Clipboard (`src/clip_linux.rs`, new)

- Linux now delegates to session-native helpers instead of arboard's
  in-process owner, which died when the CLI exited:
  - Wayland: `wl-copy --type text/plain;charset=utf-8` /
    `wl-paste --no-newline --type text`
  - X11: `xclip -selection clipboard`, with a fallback helper chain
- Ownership survives after the command exits; non-Linux platforms keep
  arboard.

### Scripts, Docs, and Tests

- `scripts/pathadd.sh` / `scripts/webbuild.sh`: made executable, line
  endings normalized, small robustness tweaks.
- `scripts/webbuild.*` now copy `dom.js` / `dom.d.ts` into `pkg/` and
  verify the artifacts.
- Docs: new `docs/web/DOM.txt`; `JAVASCRIPT.txt` and `STANDALONE_HTML.txt`
  extended for local loading and templates.
- Examples: new `web/example/dom.html` (zone-backed card using `%s`,
  `{{$viewer}}`, and html mode), linked from the playground page.
- Tests: `web/tests/dom.test.mjs` (node --test), `web/tests/dom.html`
  (real-browser checks), `web/tests/runner.test.mjs` and
  `web/tests/local.test.mjs` (real Wasm against `pkg/`, including the exact
  number `9007199254740993`), and an extended `types.test.ts`.

### Included Commits

- `f91137b` — Add zone templates and optional DOM bindings
- `f78c7d4` — Polish browser API and add local registry loading
- `5a53982` — Fix Linux clipboard ownership and Wayland support
- `0c363d7` — Update pathadders on Linux
- `12c771a` — Documentation Update

### Validation

- `node --test web/tests/*.test.mjs`: passed (DOM formatter/binding,
  runner, and local-loading tests)
- `web/tests/dom.html` in a real browser: passed
- `cargo test`: passed, including Linux clipboard routing and the
  serve/CORS changes
---

# 2026-08-13 - Parsing Bugfixes & Example Improvements

- `rgd <command> --help --ex` now supports `--ex <1/2/3/4/5/6/7/8>` pertaining to standard and advanced `pwsh`,`bash`,`python`,`cmd` examples for all subcommands.

- Bug fixes to decimal values

- Hardening of examples

Basically my edits over a month of using regedited in the real world. Adjusted for many things centered around QoL for parsing.

Tip - Easy commands to get started:

- `rgd rg <i#s# or i#z#>` (native reference retrieval for index strings or zones)
- `rgd ist <index number>` (String Summary)
- `rgd convert 58 72 | Set-Clipboard` (Example finding hexword for later. Append to index manually. Zero-based lines 58-72.)
- `rgd <commit/check/pull>` (Example using like git when things scoot downwards after editing)

---

## Testing/Advanced Info:

This pass consolidates the parsing and QoL adjustments discovered through a
month of using Regedited against real files. It makes the six-line index record
the sole structural contract, preserves decimal data exactly, and turns every
command's help examples into executable shell-specific documentation.

### Format and Parsing

- A record begins when a line contains the exact lowercase marker phrase
  `regedited open`. The marker may have arbitrary text before or after it.
- Exactly six lines after the marker define the complete record: `index: N`,
  one six-word hex line, one nine-value DB line, and three string lines.
- Removed the former implied divider, Markdown-section, and record-body
  semantics. Zones remain absolute line ranges in the shared document.
- Replaced integer-only DB parsing with exact fixed-point `DecimalValue`
  handling. Signed decimals, arbitrarily long fractional values, and integers
  beyond JavaScript's safe-number range retain their original literals.
- Browser/Wasm callers can use `dbExact(index)` / `db-exact` for exact decimal
  strings while the existing numeric `db(index)` interface remains available.
- Corrected line-range fixtures and assertions to match physical zero-based,
  inclusive zone coordinates.

### Native References and Boolean Scopes

- `iN` / `index:N` now dereferences to a read-only whole-index aggregate:
  identity, hex line, DB line, all three strings, and every defined zone.
- Content Boolean commands now operate on the exact selected scope:
  - `iNsM` — one string
  - `iNdbM` — one DB value
  - `iNdbl` — the complete DB line
  - `iNhl` — the complete hex line
  - `iNzM` — one defined zone
  - `iN` — the whole-index aggregate
  - `__all__` — the complete file, explicitly
- `bool-and`, `bool-nand`, `bool-or`, `bool-xor`, `count`, and `if-contains`
  no longer validate an index and then silently widen the search to the whole
  file. Invalid or ambiguous scope strings are rejected.
- Native HTTP `/query` uses the same exact-scope rules, and `/ref?spec=index:N`
  exposes the same whole-index aggregate as the CLI.

### Help and Examples

- Every command accepts eight command-local example selectors:
  - `--ex 1` / `2` / `3` / `4`: standard PowerShell, Bash, Python, and CMD
  - `--ex 5` / `6` / `7` / `8`: advanced counterparts in the same order
- Every canonical command and `rgd` alias has five runnable examples per
  selector. Stateful examples establish their own checkpoint, transaction, or
  source document rather than relying on hidden tutorial state.
- Advanced examples combine exact Boolean gates, cross-index decimal
  comparisons, zone diffs, conversions, and `commit` / `check` / `pull`
  lifecycles into deliberately dense one-liners.
- All 2,840 rendered examples were executed in their real shell interpreters:
  71 command surfaces × 5 examples × 8 selectors.

### Fixed and Hardened

- Removed the unused `fast_replace_str` implementation and restored a clean
  warning-free native build.
- Kept canonical and compact command spellings aligned across CLI help,
  shell references, HTTP behavior, and browser documentation.
- Hardened quoted phrases, negative decimal arguments, PowerShell statement
  chaining, CMD CRLF gates, Python subprocess rendering, and stateful examples.
- Preserved the canonical `iN` display form in list and integration output.

### Quick ways to get started

```powershell
# Reference for retrieval
rgd rg i38s1
rgd rg i38z1
rgd rg i38

# Show the three-string summary for one numeric index.
rgd ist 38

# Convert zero-based inclusive physical lines 58-72 to markdown hex-words.
rgd cv 58 72 | Set-Clipboard

# The built-in clipboard suffix is equivalent.
rgd cv 58 72 c

# Checkpoint, inspect external line movement, and apply safe relocations.
rgd cm
rgd ck
rgd pl

# Standard and advanced PowerShell examples for one command.
rgd rb --help --ex 1
rgd rb --help --ex 5
```

For actual text search, use `rgd f` / `fgrep`, `rgd fm` / `fgrep-multi`, or
`rgd g` / `grep` rather than `rgd rg`, which means `ref-get`.

### Included Commits

- `7314817` — Canonicalize Regedited index records and command behavior
- `a17c25c` — Document every Regedited command across interactive shells
- `2991d55` — Fix canonical record build and range assertions
- `36a2960` — Validate command examples across interactive shells
- `55286f8` — Preserve exact decimals in browser bindings
- `e861058` — Scope boolean operations and add advanced help lanes
- `7e1946e` — Document exact boolean scopes and advanced examples

### Validation

- `cargo check`: passed without warnings
- `cargo test --release`: passed, including CLI and exact-scope integration
  tests plus doctests
- `cargo build --release`: passed
- PowerShell, Bash, Python, and CMD standard examples: 1,420 / 1,420 passed
- PowerShell, Bash, Python, and CMD advanced examples: 1,420 / 1,420 passed
- Live HTTP exact-string, whole-index, aggregate-ref, and invalid-scope checks:
  passed
- Optimized Wasm package via `scripts/webbuild.ps1`: passed

---

# 2026-07-20 - QoL Update Part 3

- Added diffing through the `commit` command. Set a whole file of registry indices, with maintained hexwords (1,2,3) and instantly reload after line numbers change rather than relying on the previous method of accomplishing this only via served instances.

- Hardened `rgd` alias and .ps1 + .sh snippets for adding symlinks to the command properly.

- Updated and sorted `help` menu to be much more QoL.

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
