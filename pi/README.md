# Regedited Pi Skill

Pi/OMP skill package for the [Regedited](https://github.com/yourusername/regedited) fast plaintext parse-ment database.

## Install

### Global (recommended)
```bash
cd /path/to/regedited/pi
./install.sh
```

### For Oh My Pi (OMP)
```bash
./install.sh --omp
```

### Project-local
```bash
./install.sh --local   # Installs to ./.pi/skills/
```

### Manual
```bash
mkdir -p ~/.pi/agent/skills/
cp -r /path/to/regedited/pi ~/.pi/agent/skills/regedited
```

Then reload Pi:
```
/reload
```

## What Pi Gets

- **Structured document inspection** — fast header scans on multi-GB files
- **Zone-based content extraction** — O(1) section jumps via hex-word metadata
- **Boolean content analysis** — `bool-and`, `bool-nand`, `bool-or`, `bool-xor` with exit codes
- **HTML attribute extraction** — `grab-html` as a `grep -oP` replacement for attrs
- **Three-mode encapsulation** — Windows CMD-safe `["..."]`, `['...']`, `["'...'"]`
- **Content-aware zone manipulation** — copy/append/replace with automatic line recalculation

## Structure

```
pi/
├── SKILL.md                 # Skill definition (read by Pi)
├── scripts/                 # Helper scripts
│   ├── regedited_init.sh       # Initialize new document
│   ├── regedited_add_section.sh    # Add section
│   ├── regedited_quick_scan.sh     # Quick scan + summary
│   ├── regedited_extract_all.sh    # Extract all zones
│   ├── regedited_html_to_sets.sh   # HTML → set variables
│   └── regedited_bool_check.sh     # Boolean check wrapper
├── references/              # On-demand docs
│   ├── commands.md          # Full command reference
│   ├── encapsulation.md     # Three-mode system guide
│   └── boolean_ops.md       # Boolean operations patterns
├── assets/                  # Templates
│   └── template.md          # Empty Regedited document
├── install.sh               # Install script
└── README.md                # This file
```

## Usage in Pi

Once installed, just mention "regedited" or describe the operation:

> "Use regedited to scan this large markdown file and list all sections"

> "Check if this document contains both 'API' and 'examples'"

> "Extract all HREF attributes from this HTML as set variables"

> "Encapsulate this URL in store mode for a CMD script"

Pi loads the SKILL.md automatically and knows the commands.
