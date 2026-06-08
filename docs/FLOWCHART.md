# Regedited Architecture Flowcharts

Comprehensive mermaid diagrams showing all modules, call paths, and abilities.

---

## Diagram 1: Module Dependency Graph

```mermaid
flowchart TB
    subgraph ENTRY["Entry Points"]
        CLI["main.rs<br/>30+ CLI Commands"]
        PY["Python subprocess"]
        EVR["evcxr REPL (Jupyter)"]
    end

    subgraph CORE["Core Types (lib.rs)"]
        ERR["RegeditedError"]
        MMF["MmapFile"]
        BS["ByteScanner"]
        RSLT["Result&lt;T&gt;"]
    end

    subgraph INDEX["Document Index"]
        HDR["header.rs<br/>scan_content()<br/>DocumentHeader<br/>SectionInfo"]
        HSC["hex-word codec"]
    end

    subgraph DATA["Data Parsers"]
        AS["ascii_store.rs<br/>AsciiStore<br/>ZonePair"]
        ZT["zone_type.rs<br/>ZoneType enum<br/>encode/decode_hex_word"]
        DL["db_line.rs<br/>DbLine<br/>SectionData<br/>9 values + 3 strings"]
    end

    subgraph OPS["Operations"]
        FO["fast_ops.rs<br/>fast_scan<br/>fast_diff<br/>fast_replace<br/>fast_grep"]
        ZE["zone_editor.rs<br/>extract/replace/append<br/>copy/swap zone<br/>apply_line_deltas"]
        ZN["zone.rs<br/>Zone struct<br/>extract_zone"]
    end

    subgraph API["High-Level API"]
        ST["store.rs<br/>Store struct<br/>caching + CRUD"]
    end

    subgraph UTIL["Utilities"]
        EC["echo.rs<br/>safe_echo<br/>5 strategies"]
        CL["clip.rs<br/>clipboard<br/>cross-platform"]
        UT["utf16.rs<br/>getutf()<br/>DWORD encode"]
    end

    subgraph SHEL["shel.sh/XML Integration"]
        EN["encapsulate.rs<br/>EncapMode<br/>b/c/d modes"]
        HE["html_extract.rs<br/>extract_attributes<br/>GRAB B/C/D equiv"]
        BO["bool_ops.rs<br/>AND/NAND/OR/XOR<br/>count/if_contains"]
    end

    CLI --> ST
    CLI --> FO
    CLI --> ZE
    CLI --> EC
    CLI --> CL
    CLI --> UT
    CLI --> EN
    CLI --> HE
    CLI --> BO

    PY -->|"subprocess.run([RE, ...])"| CLI
    EVR -->|"use regedited::*"| CORE
    EVR --> HDR
    EVR --> ZE
    EVR --> EN
    EVR --> HE
    EVR --> BO

    ST --> HDR
    ST --> AS
    ST --> DL
    ST --> ZN

    FO --> HDR
    FO --> AS
    FO --> DL
    FO --> MMF

    ZE --> HDR
    ZE --> AS
    ZE --> ZT

    ZN --> HDR
    ZN --> AS

    AS --> ZT
    DL --> HDR

    HE --> EN
    BO -->|"content analysis"| ZE

    HDR --> CORE
    BS --> MMF
```

---

## Diagram 2: CLI Command Router

```mermaid
flowchart LR
    subgraph PARSER["main.rs - clap Parser"]
        C[Commands enum]
    end

    subgraph SCAN["Scan Commands"]
        c_list["list &lt;file&gt;"]
        c_scan["scan [--filter] [--value]"]
        c_db["db &lt;file&gt; &lt;section&gt;"]
        c_ascii["ascii &lt;file&gt; &lt;section&gt;"]
        c_info["info &lt;file&gt;"]
        c_content["content &lt;file&gt; &lt;section&gt;"]
        c_summary["summary &lt;file&gt;"]
    end

    subgraph GREP["Grep Commands"]
        c_fgrep["fgrep &lt;file&gt; &lt;pattern&gt; [-s]"]
        c_fgm["fgrep-multi &lt;file&gt; &lt;p1&gt; &lt;p2&gt;..."]
        c_grep["grep &lt;file&gt; &lt;section&gt; &lt;zone&gt;"]
        c_lines["lines &lt;file&gt; &lt;start&gt; &lt;end&gt;"]
    end

    subgraph ZONE["Zone Manipulation"]
        c_zcopy["zone-copy -f S -m n -t T -n n"]
        c_zapp["zone-append &lt;file&gt; S z [--text]"]
        c_zrep["zone-replace &lt;file&gt; S z [--text]"]
        c_zext["zone-extract &lt;file&gt; S z"]
        c_zinf["zone-info &lt;file&gt; S z"]
    end

    subgraph WRITE["Write Commands"]
        c_sn["set-num &lt;file&gt; S i v"]
        c_ss["set-str &lt;file&gt; S i v"]
        c_sz["set-zone &lt;file&gt; S z s e [-t type]"]
        c_add["add &lt;file&gt; &lt;section&gt;"]
        c_rm["rm &lt;file&gt; &lt;section&gt;"]
        c_new["new &lt;file&gt; &lt;title&gt;"]
    end

    subgraph DIFF["Diff & Replace"]
        c_diff["diff &lt;a&gt; &lt;b&gt;"]
        c_replace["replace &lt;target&gt; &lt;source&gt; [-o] [-s]"]
    end

    subgraph SHEL_CMD["shel.sh/XML Commands"]
        c_encap["encap &lt;text&gt; [-m b/c/d]<br>[--extract] [--to] [--set]"]
        c_gh["grab-html &lt;file&gt; &lt;attr&gt;<br>[-m] [--tag] [--set] [-n]"]
        c_band["bool-and &lt;file&gt; S p1 [p2]..."]
        c_bnand["bool-nand &lt;file&gt; S &lt;must&gt; &lt;not&gt;"]
        c_bor["bool-or &lt;file&gt; S p1 [p2]..."]
        c_bxor["bool-xor &lt;file&gt; S a b"]
        c_count["count &lt;file&gt; S &lt;pattern&gt;"]
        c_if["if-contains &lt;file&gt; S p<br>[--then] [--else]"]
    end

    subgraph UTIL_CMD["Utility Commands"]
        c_types["types"]
        c_conv["convert &lt;start&gt; &lt;end&gt; [-t type]"]
        c_getutf["getutf &lt;n&gt; [--decode]"]
        c_echo["echo &lt;file&gt; S i"]
        c_echod["echo-direct &lt;text&gt;"]
        c_clip["clip &lt;file&gt; S i"]
    end

    C --> SCAN
    C --> GREP
    C --> ZONE
    C --> WRITE
    C --> DIFF
    C --> SHEL_CMD
    C --> UTIL_CMD
```

---

## Diagram 3: Python Integration Paths

```mermaid
flowchart TD
    subgraph PY_IN["Python Input"]
        py_sub["subprocess.run([RE, ...])"]
        py_cap["capture_output=True"]
    end

    subgraph PY_SCAN["Scanning"]
        ps1["re('scan', file)"]
        ps2["re('scan', file, '--filter', 'Code')"]
        ps3["re('scan', file, '--value', '0:10:100')"]
    end

    subgraph PY_ZONE["Zone Operations"]
        pz1["re('zone-extract', file, section, zone)"]
        pz2["re('zone-copy', file, '--from', A, '-m', 0, '--to', B, '-n', 1)"]
        pz3["re('zone-append', file, S, z, '--text', 'new')"]
        pz4["re('zone-replace', file, S, z, '--text', content)"]
    end

    subgraph PY_BOOL["Boolean Checks"]
        pb1["subprocess.run([RE, 'bool-and', f, S, p1, p2])"]
        pb2["result.returncode == 0 # TRUE"]
        pb3["result.returncode == 1 # FALSE"]
        pb4["re('if-contains', f, S, p, '--then-val', YES, '--else-val', NO)"]
    end

    subgraph PY_HTML["HTML Extraction"]
        ph1["re('grab-html', 'page.html', 'HREF', '--tag', 'a', '--mode', 'd', '--set', '0')"]
        ph2["Output: set “0aaa=[“'url'”]”"]
    end

    subgraph PY_ENC["Encapsulation"]
        pe1["re('encap', text, '--mode', 'd')"]
        pe2["re('encap', text, '--set', '0aaa', '--mode', 'd')"]
    end

    py_sub --> ps1
    py_sub --> pz1
    py_sub --> pb1
    py_sub --> ph1
    py_sub --> pe1

    pb1 --> pb2
    pb1 --> pb3
    ph1 --> ph2
```

---

## Diagram 4: evcxr REPL Integration

```mermaid
flowchart TD
    subgraph EVR_SETUP["Setup"]
        e_dep[":dep regedited = { path = ... }"]
        e_use["use regedited::*"]
    end

    subgraph EVR_CORE["Core Operations"]
        e1["scan_content(&content) -> DocumentHeader"]
        e2["header.get_section('Name') -> &SectionInfo"]
        e3["header.section_names() -> Vec<&str>"]
    end

    subgraph EVR_HEX["Hex-Word Operations"]
        e4["encode_hex_word(line, ZoneType::Code) -> String"]
        e5["decode_hex_word('0x10000032') -> (u32, ZoneType)"]
        e6["ZoneType::from_name('code') -> Option<ZoneType>"]
    end

    subgraph EVR_ZONE["Zone Manipulation"]
        e7["extract_zone_content(content, section, zone) -> String"]
        e8["replace_zone_content(content, section, zone, new) -> String"]
        e9["append_zone_content(content, section, zone, append) -> String"]
    end

    subgraph EVR_ENCAP["Encapsulation"]
        e10["encapsulate(text, EncapMode::Search) -> [“...”]"]
        e11["encapsulate(text, EncapMode::Delimit) -> ['...']"]
        e12["encapsulate(text, EncapMode::Store) -> [“'...'”]"]
    end

    subgraph EVR_BOOL["Boolean Operations"]
        e13["bool_and(content, &[p1, p2]) -> BoolResult"]
        e14["bool_nand(content, must, must_not) -> BoolResult"]
        e15["count(content, pattern) -> (usize, Vec)"]
    end

    e_dep --> e_use
    e_use --> e1
    e_use --> e4
    e_use --> e7
    e_use --> e10
    e_use --> e13

    e1 --> e2
    e2 --> e7
```

---

## Diagram 5: Function Abilities Map

```mermaid
mindmap
  root((Regedited<br/>Abilities))
    Scan
      list :: list all sections
      scan :: header-only metadata scan
      db :: show database table
      ascii :: show hex-word store
      info :: full document info
    Grep
      fgrep :: memory-mapped file grep
      fgrep_multi :: multi-pattern OR grep
      grep :: extract zone by index
      lines :: arbitrary line range
    Zone[Zone Manipulation]
      zone_extract :: extract zone content
      zone_replace :: replace zone content
      zone_append :: append to zone content
      zone_copy :: copy zone A to zone B
      zone_info :: machine-readable zone meta
    Write
      set_num :: update numeric value 0-8
      set_str :: update string 0-2
      set_zone :: update zone range + type
      add :: add new section
      rm :: remove section
      new :: create new document
    Diff
      diff :: metadata-only comparison
      replace :: patch sections from source
    Boolean
      bool_and :: ALL patterns must match
      bool_nand :: contains A NOT B
      bool_or :: ANY pattern matches
      bool_xor :: exactly ONE matches
      count :: count occurrences
      if_contains :: conditional output
    HTML
      grab_html :: extract attributes
      format_as_set_vars :: output as set variables
      format_numbered :: numbered index output
    Encapsulation
      encap :: wrap in b/c/d mode
      extract :: unwrap encapsulated text
      convert_mode :: convert between modes
      format_set_command :: output as set var
    Utility
      types :: list zone types
      convert :: range to hex-words
      getutf :: DWORD encode/decode
      echo :: safe echo for CMD
      echo_direct :: safe echo raw text
      clip :: copy to clipboard
```

---

## Diagram 6: Data Flow — Read Path

```mermaid
sequenceDiagram
    participant User
    participant CLI as main.rs
    participant Store as store.rs
    participant Header as header.rs
    participant Mmap as MmapFile
    participant Zone as zone.rs
    participant Ascii as ascii_store.rs

    User->>CLI: regedited grep doc.md Section 1
    CLI->>Store: Store::open()
    Store->>Mmap: MmapFile::open()
    Mmap-->>Store: &str (zero-copy mmap)
    Store->>Header: scan_content()
    Header->>Header: find_line_offsets()
    Header->>Header: parse_section_header() x N
    Header-->>Store: DocumentHeader (BTreeMap)
    Store->>Ascii: AsciiStore::from_line()
    Ascii-->>Store: ZonePair {start: 60, end: 66}
    Store->>Zone: extract_zone()
    Zone->>Mmap: byte offset jump
    Mmap-->>Zone: lines 60-66
    Zone-->>CLI: Zone content
    CLI-->>User: Display content
```

---

## Diagram 7: Data Flow — Write Path

```mermaid
sequenceDiagram
    participant User
    participant CLI as main.rs
    participant ZE as zone_editor.rs
    participant AS as ascii_store.rs
    participant HDR as header.rs
    participant ZT as zone_type.rs

    User->>CLI: regedited zone-replace doc.md Section 1 --text "new"
    CLI->>ZE: replace_zone_content()
    ZE->>AS: AsciiStore::from_line()
    AS-->>ZE: ZonePair {start: 60, end: 66}
    ZE->>ZE: Calculate delta<br/>(new_lines - old_lines)
    ZE->>ZE: Splice new content
    ZE->>ZT: shift_hex_word_line()
    ZT->>ZT: decode_hex_word() x 6
    ZT->>ZT: Shift lines >= threshold
    ZT->>ZT: encode_hex_word() x 6
    ZT-->>ZE: Updated hex-word line
    ZE->>HDR: update_lines()
    HDR-->>ZE: New document content
    ZE-->>CLI: Updated document
    CLI->>CLI: fs::write() (atomic)
    CLI-->>User: OK message
```
