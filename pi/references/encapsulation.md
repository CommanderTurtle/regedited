# Three-Mode Encapsulation System

Inspired by shel.sh/XML. Three modes for different contexts — eliminates escape-hell.

## Modes

| Mode | Letter | Format | Use Case | Safe for |
|------|--------|--------|----------|----------|
| Search | **b** | `["..."]` | String matching, echoing | `findstr`, `echo`, `if` |
| Delimit | **c** | `['...']` | Piping, literal content | `clip`, file writes |
| Store | **d** | `["'...'"]` | Universal storage | `set` variables, databases |

## Pattern Reference

### CMD Search Mode (b)
```cmd
:: Finding text that contains quotes
findstr /C:"["hello world"]" file.txt

:: Echo with special chars
echo ["https://site.com?a=1&b=2"]
```

### CMD Delimit Mode (c)
```cmd
:: Pipe without breaking on special chars
echo ['https://site.com?a=1&b=2'] | clip

:: File append
echo ['content with <tags> & more'] >> file.txt
```

### CMD Store Mode (d)
```cmd
:: Database variable (the universal format)
set "0aaa=["'https://example.com'"]"
set "0aab=["'https://another.com'"]"

:: Safe for ALL contexts — search, pipe, and storage
```

## Regedited Commands

```bash
# Encapsulate
regedited encap "text" --mode b     # ["text"]
regedited encap "text" --mode c     # ['text']
regedited encap "text" --mode d     # ["'text'"]

# Extract
regedited encap "['text']" --extract    # text
regedited encap "["'text'"]" --extract  # text

# Convert
regedited encap "['text']" --to d   # ["'text'"]

# Set variable
regedited encap "value" --set 0aaa --mode d
# → set "0aaa=["'value'"]"
```

## Suffix Naming Convention

Variables auto-increment: `0aaa`, `0aab`, `0aac`, `0aad`... `0aaz`, `0aba`, `0abb`...

```bash
# Extract multiple attributes as numbered variables
regedited grab-html page.html HREF --tag a --mode d --set 0
# → set "0aaa=["'https://example.com'"]"
# → set "0aab=["'https://another.com'"]"
# → set "0aac=["'https://third.com'"]"
```
