#!/bin/bash
# Boolean check with human-readable output
# Usage: regedited_bool_check.sh <file> <section> <operation> [args...]
# Operations: and, nand, or, xor, count, if

set -e

FILE="${1:?Usage: regedited_bool_check.sh <file> <section> <operation> [args...]}"
SECTION="${2:?Usage: regedited_bool_check.sh <file> <section> <operation> [args...]}"
OP="${3:?Usage: regedited_bool_check.sh <file> <section> <operation> [args...]}"

case "$OP" in
    and)
        shift 3
        regedited bool-and "$FILE" "$SECTION" "$@"
        ;;
    nand)
        MUST="${4:?nand requires <must_contain> <must_not>}"
        MUSTNOT="${5:?nand requires <must_contain> <must_not>}"
        regedited bool-nand "$FILE" "$SECTION" "$MUST" "$MUSTNOT"
        ;;
    or)
        shift 3
        regedited bool-or "$FILE" "$SECTION" "$@"
        ;;
    xor)
        A="${4:?xor requires <pattern_a> <pattern_b>}"
        B="${5:?xor requires <pattern_a> <pattern_b>}"
        regedited bool-xor "$FILE" "$SECTION" "$A" "$B"
        ;;
    count)
        PATTERN="${4:?count requires <pattern>}"
        regedited count "$FILE" "$SECTION" "$PATTERN"
        ;;
    if)
        PATTERN="${4:?if requires <pattern> <then> <else>}"
        THEN="${5:-TRUE}"
        ELSE="${6:-FALSE}"
        regedited if-contains "$FILE" "$SECTION" "$PATTERN" --then-val "$THEN" --else-val "$ELSE"
        ;;
    *)
        echo "Unknown operation: $OP"
        echo "Valid: and, nand, or, xor, count, if"
        exit 1
        ;;
esac
