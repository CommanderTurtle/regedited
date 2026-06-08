# Boolean Operations Reference

Exit-code friendly boolean logic for scripting. All patterns are case-insensitive.

## Truth Table

| Command | Patterns Found | Exit Code | Use Case |
|---------|---------------|-----------|----------|
| `bool-and A B C` | ALL of A,B,C | 0 | Validate all requirements met |
| `bool-and A B C` | Missing any | 1 | Gate: fail if incomplete |
| `bool-nand A B` | A yes, B no | 0 | "Contains Rust, NOT Python" |
| `bool-nand A B` | B found | 1 | Gate: fail if forbidden present |
| `bool-or A B C` | ANY found | 0 | "Has at least one language" |
| `bool-or A B C` | None found | 1 | Gate: fail if nothing matches |
| `bool-xor A B` | Exactly one | 0 | "Has Rust XOR Python" |
| `bool-xor A B` | Both/neither | 1 | Gate: fail if ambiguous |

## Scripting Patterns

### Gate Pattern
```bash
# Only proceed if section contains ALL required patterns
regedited bool-and doc.md MySection "license" "copyright" "MIT" \
    && echo "Compliant" \
    || echo "NON-COMPLIANT"
```

### Guard Pattern
```bash
# Abort if forbidden pattern found
regedited bool-nand src.md Code "rust" "unsafe" \
    || { echo "Found unsafe! Aborting."; exit 1; }
```

### Switch Pattern
```bash
# Branch based on content
case $(regedited if-contains doc.md __all__ "TODO" --then-val "dirty" --else-val "clean") in
    dirty) echo "Found TODOs — review needed" ;;
    clean) echo "Clean — ready to ship" ;;
esac
```

### Checklist Pattern
```bash
# Multi-step validation
regedited bool-and doc.md README "install" "usage" \
&& regedited bool-or doc.md README "license" "LICENSE" \
&& regedited bool-nand doc.md README "TODO" "FIXME" \
&& echo "README passes all checks" \
|| echo "README has issues"
```

### Count Threshold
```bash
# Fail if too many TODOs
count=$(regedited count doc.md __all__ "TODO" | head -1 | grep -oP '^\d+')
[ "$count" -lt 5 ] && echo "OK ($count TODOs)" || echo "FAIL ($count TODOs)"
```

## `__all__` Section

Use `__all__` as the section name to check the entire file instead of one section.

```bash
# Check entire document for patterns
regedited bool-and doc.md __all__ "API" "docs" "examples"
regedited count doc.md __all__ "deprecated"
```
