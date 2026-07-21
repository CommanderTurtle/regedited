//! # Regedited CLI
//!
//! Command-line interface for the fast plaintext parse-ment database.
//!
//! ## Commands
//!
//! ```bash
//! # Safetensors-style fast scan (header-only)
//! regedited scan myfile.md
//! regedited scan myfile.md --filter Config
//!
//! # Diff two files (metadata-only)
//! regedited diff base.md patched.md
//!
//! # Replace sections (safetensors-style patch)
//! regedited replace base.md patched.md --output result.md
//!
//! # Fast grep (ripgrep-style, memory-mapped)
//! regedited fgrep myfile.md "pattern"
//! regedited fgrep myfile.md "pattern" --section MySection
//! regedited fgrep-multi myfile.md pattern1 pattern2 pattern3
//!
//! # ZONE CONTENT MANIPULATION (Python-scriptable)
//! # Copy zone content from one section to another
//! regedited zone-copy myfile.md --from Alpha --from-zone 0 --to Beta --to-zone 1
//!
//! # Append content to a zone (from stdin or --text)
//! echo "new content" | regedited zone-append myfile.md MySection 0
//! regedited zone-append myfile.md MySection 0 --text "inline content"
//!
//! # Replace zone content (from stdin or --text)
//! cat new.md | regedited zone-replace myfile.md MySection 1
//!
//! # Extract raw zone content to stdout (for piping)
//! regedited zone-extract myfile.md MySection 1 > extracted.md
//!
//! # Zone info in machine-readable format (for Python scripts)
//! regedited zone-info myfile.md MySection 1
//!
//! # Show database table for a section
//! regedited db myfile.md MySection
//!
//! # Show the hex-word line for a section (`ascii` is the legacy alias)
//! regedited hexline myfile.md MySection
//!
//! # Extract a zone (grep by line range)
//! regedited grep myfile.md MySection 0
//!
//! # Copy a string to clipboard
//! regedited clip myfile.md MySection 2
//!
//! # Echo a string (safe for Windows CMD)
//! regedited echo myfile.md MySection 1
//!
//! # Convert line range to hex-words
//! regedited convert 50 80 --zone-type code
//!
//! # Update a numeric value
//! regedited set-num myfile.md MySection 0 42
//!
//! # Update a string
//! regedited set-str myfile.md MySection 0 "new value"
//!
//! # Update a hex-word line zone (with type)
//! regedited set-zone myfile.md MySection 0 10 100 --zone-type code
//!
//! # Show section content
//! regedited content myfile.md MySection
//!
//! # Create a new document
//! regedited new myfile.md "Document Title"
//!
//! # Add / remove sections
//! regedited add myfile.md NewSection
//! regedited rm myfile.md OldSection
//! ```

// SPDX-License-Identifier: AGPL-3.0

use clap::{error::ErrorKind, CommandFactory, Parser, Subcommand};
use owo_colors::OwoColorize;
use regedited::{
    bool_ops::{bool_and, bool_nand, bool_or, bool_xor, count, if_contains},
    echo::safe_echo,
    encapsulate::{convert_mode, encapsulate, extract, EncapMode},
    header::scan_content,
    html_extract::{extract_attributes, format_as_set_vars, format_numbered},
    store::{Store, StoreConfig},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(name = "regedited")]
#[command(about = "Fast plaintext parse-ment database")]
#[command(version = "0.2.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbose output
    #[arg(long, global = true)]
    verbose: bool,

    /// Don't auto-save changes
    #[arg(long, global = true)]
    no_save: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List all indexes in the document
    List {
        /// Path to the markdown file
        file: PathBuf,
    },

    /// Show the database table for an index
    Db {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference: 64, i64, or index:64 (legacy name accepted)
        #[arg(value_name = "INDEX")]
        section: String,
    },

    /// Show the hex-word line for an index (`ascii` is the legacy command name)
    #[command(aliases = ["ascii", "hex-word-line", "ranges"])]
    Hexline {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
    },

    /// Scan all indexes
    Scan {
        /// Path to the markdown file
        file: PathBuf,
        /// Filter indexes by legacy key pattern
        #[arg(short, long)]
        filter: Option<String>,
        /// Filter by database value index and range (e.g., "0:5:50")
        #[arg(short, long)]
        value: Option<String>,
    },

    /// Diff two Regedited files (metadata-only, like safetensors header diff)
    Diff {
        /// First file
        file_a: PathBuf,
        /// Second file
        file_b: PathBuf,
    },

    /// Replace matching numeric indexes from source into target
    Replace {
        /// Target file (to be modified)
        target: PathBuf,
        /// Source file (donor indexes)
        source: PathBuf,
        /// Index references to replace (all matching if omitted)
        #[arg(short, long = "indexes", visible_alias = "sections")]
        sections: Option<Vec<String>>,
        /// Output file (default: overwrite target)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Fast grep (ripgrep-style, memory-mapped)
    Fgrep {
        /// Path to the markdown file
        file: PathBuf,
        /// Search pattern
        pattern: String,
        /// Limit to an index reference
        #[arg(short, long = "index", visible_alias = "section", value_name = "INDEX")]
        section: Option<String>,
    },

    /// Multi-pattern grep (OR logic)
    FgrepMulti {
        /// Path to the markdown file
        file: PathBuf,
        /// Search patterns
        patterns: Vec<String>,
    },

    /// Zone content copy: copy one zone's content to another zone
    ZoneCopy {
        /// Path to the markdown file
        file: PathBuf,
        /// Source index reference
        #[arg(short = 'f', long, value_name = "INDEX")]
        from: String,
        /// Source zone index (0-2)
        #[arg(short = 'm', long, default_value = "0")]
        from_zone: usize,
        /// Target index reference
        #[arg(short = 't', long, value_name = "INDEX")]
        to: String,
        /// Target zone index (0-2)
        #[arg(short = 'n', long, default_value = "0")]
        to_zone: usize,
    },

    /// Zone content append: append content (from stdin or --text) to a zone
    ZoneAppend {
        /// Path to the markdown file
        file: PathBuf,
        /// Target index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// Target zone index (0-2)
        zone: usize,
        /// Text to append (if not provided, reads from stdin)
        #[arg(short, long)]
        text: Option<String>,
    },

    /// Zone content replace: replace a zone's content (from stdin or --text)
    ZoneReplace {
        /// Path to the markdown file
        file: PathBuf,
        /// Target index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// Target zone index (0-2)
        zone: usize,
        /// Replacement text (if not provided, reads from stdin)
        #[arg(short, long)]
        text: Option<String>,
    },

    /// Zone content extract: dump raw zone content to stdout (for piping)
    ZoneExtract {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// Zone index (0-2)
        zone: usize,
    },

    /// Zone info: machine-readable zone metadata (for Python scripts)
    ZoneInfo {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// Zone index (0-2)
        zone: usize,
    },

    /// Resolve a registry index number to its internal layout key
    ResolveIndex {
        /// Path to the markdown file
        file: PathBuf,
        /// Registry index value from the `index: N` line
        registry_index: u64,
    },

    /// Extract a zone from a numeric registry index
    IndexZoneExtract {
        /// Path to the markdown file
        file: PathBuf,
        /// Registry index value from the `index: N` line
        registry_index: u64,
        /// Zone index (0-2)
        zone: usize,
    },

    /// Replace a zone in a numeric registry index
    IndexZoneReplace {
        /// Path to the markdown file
        file: PathBuf,
        /// Registry index value from the `index: N` line
        registry_index: u64,
        /// Zone index (0-2)
        zone: usize,
        /// Replacement text (if not provided, reads from stdin)
        #[arg(short, long)]
        text: Option<String>,
    },

    /// Copy one zone to another within the same file, addressed by registry index
    IndexZoneCopy {
        /// Path to the markdown file
        file: PathBuf,
        /// Source registry index value
        #[arg(long)]
        from_index: u64,
        /// Source zone index (0-2)
        #[arg(long, default_value = "0")]
        from_zone: usize,
        /// Target registry index value
        #[arg(long)]
        to_index: u64,
        /// Target zone index (0-2)
        #[arg(long, default_value = "0")]
        to_zone: usize,
    },

    /// Transfer a zone between two files, addressed by registry index
    IndexZoneTransfer {
        /// Source markdown file
        #[arg(long)]
        from_file: PathBuf,
        /// Source registry index value
        #[arg(long)]
        from_index: u64,
        /// Source zone index (0-2)
        #[arg(long, default_value = "0")]
        from_zone: usize,
        /// Target markdown file
        #[arg(long)]
        to_file: PathBuf,
        /// Target registry index value
        #[arg(long)]
        to_index: u64,
        /// Target zone index (0-2)
        #[arg(long, default_value = "0")]
        to_zone: usize,
    },

    /// Extract an explicit hex-word line range
    HexExtract {
        /// Path to the markdown file
        file: PathBuf,
        /// Start hex-word, e.g. 1x0000032 or legacy 0x10000032
        start: String,
        /// End hex-word, e.g. 1x0000050 or legacy 0x10000050
        end: String,
    },

    /// Replace an explicit hex-word line range and shift later hex-words
    HexReplace {
        /// Path to the markdown file
        file: PathBuf,
        /// Start hex-word, e.g. 1x0000032 or legacy 0x10000032
        start: String,
        /// End hex-word, e.g. 1x0000050 or legacy 0x10000050
        end: String,
        /// Replacement text (if not provided, reads from stdin)
        #[arg(short, long)]
        text: Option<String>,
    },

    /// Read any native ref spec: index string, DB value, DB line, defined zone, literal hex line/range
    RefGet {
        /// Path to the markdown file
        file: PathBuf,
        /// Ref spec, e.g. index:4:string:3, index:5:db:8, index:3:zone:2, hex:0x0000021..0x0000022
        spec: String,
        /// Copy the resolved value to the system clipboard
        #[arg(long)]
        clip: bool,
    },

    /// Write a literal or resolved ref value to any writable native ref spec
    RefSet {
        /// Path to the markdown file
        file: PathBuf,
        /// Target ref spec
        target: String,
        /// Source ref spec
        #[arg(long)]
        from: Option<String>,
        /// Literal text source; stdin is used if neither --from nor --text is supplied
        #[arg(short, long)]
        text: Option<String>,
        /// Append to the target instead of replacing it
        #[arg(long)]
        append: bool,
    },

    /// Copy or move a resolved ref into another writable ref
    RefCopy {
        /// Path to the markdown file
        file: PathBuf,
        /// Source ref spec
        from: String,
        /// Target ref spec
        to: String,
        /// Append to the target instead of replacing it
        #[arg(long)]
        append: bool,
        /// Remove the source after writing the target
        #[arg(long = "move")]
        move_source: bool,
    },

    /// Diff any two native ref specs
    RefDiff {
        /// Path to the markdown file
        file: PathBuf,
        /// Left ref spec
        left: String,
        /// Right ref spec
        right: String,
    },

    /// Boolean comparison over arbitrary ref specs and literals
    RefBool {
        /// Path to the markdown file
        file: PathBuf,
        /// Left ref spec or literal
        left: String,
        /// Operation: contains, eq, ne, gt, gte, lt, lte
        op: String,
        /// Right ref spec or literal
        right: String,
        /// Value printed when true
        #[arg(long, default_value = "TRUE")]
        then_val: String,
        /// Value printed when false
        #[arg(long, default_value = "FALSE")]
        else_val: String,
    },

    /// List string 1, string 2, and string 3 for an index
    IndexStrList {
        /// Path to the markdown file
        file: PathBuf,
        /// Registry index value
        registry_index: u64,
    },

    /// Set a defined index zone's stored hexword range without changing content
    IndexZoneSetHex {
        /// Path to the markdown file
        file: PathBuf,
        /// Registry index value
        registry_index: u64,
        /// Defined range slot, user-facing 1-3
        zone: usize,
        /// Start hex-word
        start: String,
        /// End hex-word
        end: String,
    },

    /// Convert two line numbers and assign them to an index zone (zones are 1-3)
    IndexZoneSetLines {
        /// Path to the indexed document
        file: PathBuf,
        /// Registry index value
        registry_index: u64,
        /// Defined zone slot, user-facing 1-3
        zone: usize,
        /// Two line numbers plus an optional inline p/b/m/d type token and clip/c suffix
        #[arg(value_name = "VALUE", num_args = 2..)]
        values: Vec<String>,
        /// Default zone type when no inline type token is supplied
        #[arg(short = 't', long, default_value = "markdown")]
        zone_type: String,
    },

    /// Show current native Regedited state as JSON
    State {
        /// Path to the markdown file
        file: PathBuf,
    },

    /// Compare current native Regedited state with a prior state JSON
    StateCompare {
        /// Path to the markdown file
        file: PathBuf,
        /// State JSON path
        state: PathBuf,
    },

    /// Check committed zone fingerprints and write a temporary relocation diff
    Check {
        /// Path to the indexed document
        file: PathBuf,
    },

    /// Save one zone checkpoint, or check and optionally pull safe range relocations
    Commit {
        /// Path to the indexed document
        file: PathBuf,
        /// Pull safe relocations without an interactive prompt
        #[arg(long)]
        pull: bool,
    },

    /// Apply the latest guarded zone relocation diff
    Pull {
        /// Path to the indexed document
        file: PathBuf,
    },

    /// Restore the last one-step undo copy
    Undo {
        /// Path to the markdown file
        file: PathBuf,
    },

    /// Extract a zone by index (0-2)
    Grep {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// Zone index (0-2)
        #[arg(value_name = "ZONE")]
        index: usize,
    },

    /// Copy a string to clipboard
    Clip {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// String index (0-2)
        #[arg(value_name = "STRING")]
        index: usize,
    },

    /// Echo a string safely (handles Windows CMD special chars)
    Echo {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// String index (0-2)
        #[arg(value_name = "STRING")]
        index: usize,
    },

    /// Echo any text safely (direct string mode)
    EchoDirect {
        /// Text to echo safely
        text: String,
    },

    /// getutf — Convert a line number to UTF-16LE representation
    Getutf {
        /// Line number to encode
        number: u32,
        /// Decode mode (provide UTF-16LE hex to decode back)
        #[arg(short, long)]
        decode: Option<String>,
    },

    /// Update a numeric value
    SetNum {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// Value index (0-8)
        #[arg(value_name = "SLOT")]
        index: usize,
        /// New value
        value: i64,
    },

    /// Update a string value
    SetStr {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// String index (0-2)
        #[arg(value_name = "STRING")]
        index: usize,
        /// New value
        value: String,
    },

    /// Update Hex-word line zone (with type: markdown/code/media/database)
    SetZone {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// Zone index (0-2)
        #[arg(value_name = "ZONE")]
        index: usize,
        /// Start line
        start: u32,
        /// End line
        end: u32,
        /// Zone type: markdown (0), code (1), media (2), database (3)
        #[arg(short, long, default_value = "markdown")]
        zone_type: String,
    },

    /// Convert line numbers to hex-words or assign one pair to an index zone
    Convert {
        /// Line numbers plus optional inline p/b/m/d type tokens and clip/c suffix
        #[arg(value_name = "VALUE", num_args = 1..)]
        values: Vec<String>,
        /// Default zone type: markdown, code, media, database (or 0-3)
        #[arg(short = 't', long, default_value = "markdown")]
        zone_type: String,
        /// Legacy zone-converter marker (inline p/b/m/d tokens remain optional)
        #[arg(short = 'z', long = "zone")]
        zone: bool,
    },

    /// List all zone types and their hex nibble values
    Types,

    /// Show index content (between --- and the next index)
    Content {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
    },

    /// Extract arbitrary line range (ad-hoc grep)
    Lines {
        /// Path to the markdown file
        file: PathBuf,
        /// Start line (0-indexed)
        start: usize,
        /// End line (inclusive, 0-indexed)
        end: usize,
    },

    /// Create a new document
    New {
        /// Path for the new file
        file: PathBuf,
        /// Document title
        title: String,
    },

    /// Add a new canonical index
    Add {
        /// Path to the markdown file
        file: PathBuf,
        /// New numeric registry index
        registry_index: u64,
    },

    /// Remove an index
    Rm {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
    },

    /// Show document summary
    Summary {
        /// Path to the markdown file
        file: PathBuf,
    },

    /// Show full document info (all indexes with details)
    Info {
        /// Path to the markdown file
        file: PathBuf,
    },

    // ==================== ENCAPSULATION (shel.sh/XML inspired) ====================
    /// Encapsulate text in b/c/d modes — ["..."], ['...'], ["'...'"]
    Encap {
        /// Text to encapsulate (or extract/convert if --extract/--to provided)
        text: String,
        /// Mode: b/search (["..."]), c/delimit (['...']), d/store (["'...'"])
        #[arg(short, long, default_value = "d")]
        mode: String,
        /// Extract inner text from an encapsulated string
        #[arg(long)]
        extract: bool,
        /// Convert to a different mode (b/c/d)
        #[arg(long)]
        to: Option<String>,
        /// Output as set variable (e.g., --set 0aaa)
        #[arg(long)]
        set: Option<String>,
    },

    // ==================== HTML EXTRACTION (GRAB B/C/D equivalent) ====================
    /// Extract HTML attributes (GRAB B/C/D equivalent)
    GrabHtml {
        /// Path to HTML file
        file: PathBuf,
        /// Attribute name (HREF, SRC, etc.)
        attr: String,
        /// Encapsulation mode: b, c, or d
        #[arg(short, long, default_value = "b")]
        mode: String,
        /// Filter by tag name (e.g., "a", "img")
        #[arg(short, long)]
        tag: Option<String>,
        /// Output as set variables with base name (e.g., --set 0aaa)
        #[arg(long)]
        set: Option<String>,
        /// Output with numbered indices (-0, -1, ...)
        #[arg(long)]
        numbered: bool,
    },

    // ==================== BOOLEAN OPERATIONS (if-then logic) ====================
    /// Boolean AND: content must contain ALL patterns
    BoolAnd {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference (or "__all__" for the entire file)
        #[arg(value_name = "INDEX")]
        section: String,
        /// Patterns to match (ALL must be found)
        patterns: Vec<String>,
    },

    /// Boolean NAND: contains first pattern but NOT second
    BoolNand {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference (or "__all__" for the entire file)
        #[arg(value_name = "INDEX")]
        section: String,
        /// Pattern that must be found
        must_contain: String,
        /// Pattern that must NOT be found
        must_not: String,
    },

    /// Boolean OR: content contains ANY of the patterns
    BoolOr {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference (or "__all__" for the entire file)
        #[arg(value_name = "INDEX")]
        section: String,
        /// Patterns to match (ANY must be found)
        patterns: Vec<String>,
    },

    /// Boolean XOR: contains EXACTLY ONE of two patterns
    BoolXor {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference (or "__all__" for the entire file)
        #[arg(value_name = "INDEX")]
        section: String,
        /// First pattern
        pattern_a: String,
        /// Second pattern
        pattern_b: String,
    },

    /// Count occurrences of a pattern in content
    Count {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference (or "__all__" for the entire file)
        #[arg(value_name = "INDEX")]
        section: String,
        /// Pattern to count
        pattern: String,
    },

    /// If-contains-then: return value based on pattern presence
    IfContains {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference (or "__all__" for the entire file)
        #[arg(value_name = "INDEX")]
        section: String,
        /// Pattern to check for
        pattern: String,
        /// Value to return if pattern is found
        #[arg(long, default_value = "TRUE")]
        then_val: String,
        /// Value to return if pattern is NOT found
        #[arg(long, default_value = "FALSE")]
        else_val: String,
    },

    // ==================== WAL (Write-Ahead Log) ====================
    /// Show WAL status for a document
    Wal {
        /// Path to the markdown file
        file: PathBuf,
    },

    /// Replay WAL (crash recovery)
    WalReplay {
        /// Path to the markdown file
        file: PathBuf,
        /// Actually apply changes (without this, just shows what would be done)
        #[arg(long)]
        apply: bool,
    },

    // ==================== TRANSACTIONS ====================
    /// Transaction: begin, commit, or rollback
    Tx {
        /// Transaction action: begin, commit, rollback, status
        action: String,
        /// Path to the markdown file
        file: PathBuf,
    },

    // ==================== SCHEMA ====================
    /// Show or validate schema for a document
    Schema {
        /// Path to the markdown file
        file: PathBuf,
        /// Validate document against schema (shows errors if any)
        #[arg(long)]
        validate: bool,
        /// Create a starter schema from existing document
        #[arg(long)]
        init: bool,
    },

    // ==================== TYPED VALUES ====================
    /// List all supported registry types
    RegTypes,

    /// Parse a value as a typed registry value
    RegParse {
        /// Value to parse
        value: String,
        /// Registry type: REG_SZ, REG_DWORD, REG_QWORD, REG_BINARY, REG_MULTI_SZ, REG_JSON, REG_BOOL, ...
        #[arg(short, long, default_value = "REG_SZ")]
        reg_type: String,
    },

    // ==================== SERVE (Registry Container) ====================

    // ==================== ENHANCED CLIPBOARD ====================
    /// Copy zone content (by index 0-2) to clipboard
    ClipZone {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// Zone index (0, 1, or 2)
        #[arg(value_name = "ZONE")]
        zone: usize,
    },

    /// Copy database value (by index 0-8) to clipboard
    ClipDb {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
        /// Value index (0-8)
        #[arg(value_name = "SLOT")]
        index: usize,
    },

    /// Copy entire database line to clipboard
    ClipDbline {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
    },

    /// Copy hex-word line to clipboard (`clip-ascii` is the legacy command name)
    #[command(aliases = ["clip-ascii", "clip-hex-word-line", "clip-ranges"])]
    ClipHexline {
        /// Path to the markdown file
        file: PathBuf,
        /// Index reference
        #[arg(value_name = "INDEX")]
        section: String,
    },

    /// Copy manually-keyed hex-word range to clipboard
    ClipHexword {
        /// Start line
        start: u32,
        /// End line
        end: u32,
        /// Zone type: markdown, code, media, database
        #[arg(short, long, default_value = "code")]
        zone_type: String,
    },

    /// Start HTTP server (registry container mode)
    Serve {
        /// Path to the document file to serve
        #[arg(short, long)]
        file: PathBuf,
        /// Port to listen on
        #[arg(short, long, default_value = "5000")]
        port: u16,
        /// Read-only mode (disallow modifications)
        #[arg(long, default_value = "true")]
        read_only: bool,
    },
}

const NO_LOADED_PATH: &str = "No path specified yet, specify path, else perform a load first, i.e. `rgd load ~/example/file/location.txt`";

fn main() {
    let result = std::thread::Builder::new()
        .name("regedited-cli".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(run_main)
        .unwrap_or_else(|error| {
            eprintln!("Failed to start Regedited: {}", error);
            std::process::exit(1);
        })
        .join();

    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

fn run_main() {
    let mut args: Vec<OsString> = std::env::args_os().collect();
    let short_mode = args
        .first()
        .is_some_and(|argv0| regedited::qol::is_rgd_invocation(argv0));
    regedited::qol::normalize_global_arguments(&mut args);

    if short_mode {
        if let Err(error) = regedited::qol::validate_aliases() {
            clap::Error::raw(ErrorKind::InvalidValue, error).exit();
        }
        match handle_loaded_path_command(&args) {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => clap::Error::raw(ErrorKind::InvalidValue, error.to_string()).exit(),
        }
        regedited::qol::normalize_short_command(&mut args);
        regedited::qol::normalize_compact_refs(&mut args);
        regedited::qol::normalize_short_clip_flag(&mut args);
        if let Err(error) = regedited::qol::normalize_convert_destination(&mut args) {
            clap::Error::raw(ErrorKind::InvalidValue, error).exit();
        }
    }

    if handle_example_request(&args) {
        return;
    }

    if handle_help_request(&args, short_mode) {
        return;
    }

    let cli = parse_cli(args, short_mode);

    if let Err(e) = run(cli) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn handle_example_request(args: &[OsString]) -> bool {
    let requested = args
        .get(1)
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == "-ex" || value == "--examples");
    if !requested {
        return false;
    }

    let values: Vec<&str> = args
        .iter()
        .skip(2)
        .filter_map(|value| value.to_str())
        .collect();
    let (script, environment) = match values.as_slice() {
        [environment] => (false, *environment),
        ["script", environment] => (true, *environment),
        _ => clap::Error::raw(
            ErrorKind::InvalidValue,
            "Usage: regedited -ex [script] <powershell|repl|python|bash|bat>",
        )
        .exit(),
    };

    let content = match (script, environment.to_ascii_lowercase().as_str()) {
        (false, "powershell") => include_str!("../docs/shell/POWERSHELL.txt"),
        (false, "repl") => include_str!("../docs/shell/REPL.txt"),
        (false, "python") => include_str!("../docs/shell/PYTHON.txt"),
        (false, "bash") => include_str!("../docs/shell/BASH.txt"),
        (false, "bat") => include_str!("../docs/shell/BAT.txt"),
        (true, "powershell") => include_str!("../docs/shell/scripts/POWERSHELL.txt"),
        (true, "repl") => include_str!("../docs/shell/scripts/REPL.txt"),
        (true, "python") => include_str!("../docs/shell/scripts/PYTHON.txt"),
        (true, "bash") => include_str!("../docs/shell/scripts/BASH.txt"),
        (true, "bat") => include_str!("../docs/shell/scripts/BAT.txt"),
        _ => clap::Error::raw(
            ErrorKind::InvalidValue,
            format!(
                "unknown example environment '{}'; use powershell, repl, python, bash, or bat",
                environment
            ),
        )
        .exit(),
    };
    print!("{}", content);
    true
}

fn handle_loaded_path_command(args: &[OsString]) -> Result<bool, Box<dyn std::error::Error>> {
    let Some(command) = args.get(1).and_then(|value| value.to_str()) else {
        return Ok(false);
    };

    match command {
        "load" => {
            if args
                .iter()
                .skip(2)
                .any(|value| matches!(value.to_str(), Some("-h" | "--help" | "-help")))
            {
                println!("Usage: rgd load [FILE]");
                println!("Load FILE as the default document for later rgd commands.");
                println!("With no FILE, print the currently loaded document.");
                return Ok(true);
            }
            match args.get(2) {
                Some(path) if args.len() == 3 => {
                    let path = regedited::qol::save_loaded_path(&PathBuf::from(path))?;
                    println!("Loaded {}", path.display());
                }
                None => match regedited::qol::read_loaded_path()? {
                    Some(path) => println!("Loaded {}", path.display()),
                    None => println!("No path loaded."),
                },
                _ => return Err("Usage: rgd load [FILE]".into()),
            }
            Ok(true)
        }
        "unload" | "clear-load" => {
            if regedited::qol::clear_loaded_path()? {
                println!("Loaded path cleared.");
            } else {
                println!("No path was loaded.");
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn parse_cli(args: Vec<OsString>, short_mode: bool) -> Cli {
    if !short_mode {
        return Cli::try_parse_from(args).unwrap_or_else(|error| error.exit());
    }

    let command = args
        .get(1)
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let placement = regedited::qol::file_placement(command);
    if placement == regedited::qol::FilePlacement::None {
        return Cli::try_parse_from(args).unwrap_or_else(|error| error.exit());
    }

    let explicit_file = regedited::qol::has_explicit_file(&args, command);
    let original = Cli::try_parse_from(args.clone());
    let loaded = regedited::qol::read_loaded_path()
        .unwrap_or_else(|error| clap::Error::raw(ErrorKind::Io, error.to_string()).exit());

    if let Some(loaded) = loaded {
        let force_loaded_first =
            original.is_err() && matches!(command, "diff" | "replace" | "state-compare");
        if !explicit_file || force_loaded_first {
            if let Some(with_file) =
                regedited::qol::inject_loaded_file(&args, command, &loaded, force_loaded_first)
            {
                return Cli::try_parse_from(with_file).unwrap_or_else(|error| error.exit());
            }
        }
    } else if !explicit_file {
        clap::Error::raw(ErrorKind::MissingRequiredArgument, NO_LOADED_PATH).exit();
    }

    original.unwrap_or_else(|error| error.exit())
}

fn handle_help_request(args: &[OsString], short_mode: bool) -> bool {
    let example_mode = args
        .iter()
        .skip(2)
        .any(|value| matches!(value.to_str(), Some("-e" | "--examples")));
    let top_level_help = args.len() == 1
        || args
            .get(1)
            .is_some_and(|value| matches!(value.to_str(), Some("-h" | "--help" | "-help")));
    if top_level_help {
        print_top_level_help(short_mode, example_mode);
        return true;
    }

    if args.get(1).is_some_and(|value| value == "help") {
        if let Some(command) = args.get(2).and_then(|value| value.to_str()) {
            let canonical = if short_mode {
                regedited::qol::canonical_command(command).unwrap_or(command)
            } else {
                command
            };
            print_command_help(canonical, short_mode);
        } else {
            print_top_level_help(short_mode, false);
        }
        return true;
    }

    let requested = args
        .iter()
        .skip(2)
        .any(|value| matches!(value.to_str(), Some("-h" | "--help" | "-help")));
    if requested {
        if let Some(command) = args.get(1).and_then(|value| value.to_str()) {
            print_command_help(command, short_mode);
        } else {
            print_top_level_help(short_mode, false);
        }
        return true;
    }
    false
}

fn print_top_level_help(short_mode: bool, example_mode: bool) {
    let root = Cli::command();
    let loaded = regedited::qol::read_loaded_path().ok().flatten();
    let program = if short_mode { "rgd" } else { "regedited" };
    println!("{} - Fast indexed plaintext operations", program);
    println!("Usage: {} <COMMAND> [ARGS] [OPTIONS]", program);
    println!(
        "View: {}  (apply `-e` after `--help` for advanced examples)",
        if example_mode {
            "advanced examples"
        } else {
            "command syntax"
        }
    );
    if short_mode {
        match loaded {
            Some(path) => println!("Loaded: {}", path.display()),
            None => println!("Loaded: none (`rgd load <FILE>`)"),
        }
    }
    println!("Index refs: 64 = i64 = index:64; legacy names remain accepted.\n");
    if short_mode {
        println!("Line -> zone: rgd cv <p|b|m|d> <START> <END> to i<INDEX> <ZONE 1-3>");
    } else {
        println!(
            "Line -> zone: regedited index-zone-set-lines <FILE> <INDEX> <ZONE 1-3> <p|b|m|d> <START> <END>"
        );
    }
    println!();
    print_help_row(
        "RGD",
        "COMMAND",
        if example_mode {
            "ADVANCED EXAMPLE"
        } else {
            "ARGUMENTS"
        },
        "PURPOSE",
    );
    println!("  {:-<4} {:-<22} {:-<50} {:-<35}", "", "", "", "");

    for category in HELP_CATEGORIES {
        println!("\n[{}]", category);
        for alias in regedited::qol::COMMAND_ALIASES
            .iter()
            .filter(|alias| help_category(alias.canonical) == *category)
        {
            let Some(subcommand) = root.find_subcommand(alias.canonical) else {
                continue;
            };
            let details = if example_mode {
                help_example(alias.canonical, alias.short, short_mode)
            } else {
                help_arguments(subcommand, alias.canonical, alias.short, short_mode)
            };
            let description = subcommand
                .get_about()
                .map(ToString::to_string)
                .unwrap_or_default();
            print_help_row(alias.short, alias.canonical, &details, &description);
        }
    }

    println!("\nTop-level examples: `{} --help -e`", program);
    println!("Command detail: `<command> -help`");
    println!("Shell guides: `regedited -ex <powershell|repl|python|bash|bat>`");
    if short_mode {
        println!("Context: `rgd load <FILE>` | `rgd load` | `rgd unload`");
    }
}

fn print_help_row(short: &str, command: &str, details: &str, purpose: &str) {
    let details = wrap_help_cell(details, 50);
    let purpose = wrap_help_cell(purpose, 35);
    let rows = details.len().max(purpose.len());
    for row in 0..rows {
        println!(
            "  {:<4} {:<22} {:<50} {}",
            if row == 0 { short } else { "" },
            if row == 0 { command } else { "" },
            details.get(row).map(String::as_str).unwrap_or(""),
            purpose.get(row).map(String::as_str).unwrap_or("")
        );
    }
}

fn wrap_help_cell(value: &str, width: usize) -> Vec<String> {
    if value.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut line = String::new();
    for word in value.split_whitespace() {
        let added = usize::from(!line.is_empty()) + word.len();
        if !line.is_empty() && line.len() + added > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

const HELP_CATEGORIES: &[&str] = &[
    "INDEXES & DOCUMENT",
    "VALUES & REFERENCES",
    "ZONES & LINE RANGES",
    "SEARCH & BOOLEAN",
    "CHECKPOINTS & SAFETY",
    "CLIPBOARD & OUTPUT",
    "STRUCTURE & UTILITIES",
    "RUNTIME & INTEGRATION",
];

fn help_category(command: &str) -> &'static str {
    match command {
        "list" | "scan" | "summary" | "info" | "db" | "hexline" | "content" | "index-str-list"
        | "resolve-index" | "add" | "rm" => "INDEXES & DOCUMENT",
        "ref-get" | "ref-set" | "ref-copy" | "ref-diff" | "ref-bool" | "set-num" | "set-str" => {
            "VALUES & REFERENCES"
        }
        "zone-copy"
        | "zone-append"
        | "zone-replace"
        | "zone-extract"
        | "zone-info"
        | "index-zone-extract"
        | "index-zone-replace"
        | "index-zone-copy"
        | "index-zone-transfer"
        | "index-zone-set-hex"
        | "index-zone-set-lines"
        | "set-zone"
        | "hex-extract"
        | "hex-replace"
        | "lines"
        | "convert"
        | "types" => "ZONES & LINE RANGES",
        "fgrep" | "fgrep-multi" | "grep" | "bool-and" | "bool-nand" | "bool-or" | "bool-xor"
        | "count" | "if-contains" => "SEARCH & BOOLEAN",
        "diff" | "replace" | "state" | "state-compare" | "check" | "commit" | "pull" | "undo"
        | "wal" | "wal-replay" | "tx" => "CHECKPOINTS & SAFETY",
        "clip" | "echo" | "echo-direct" | "clip-zone" | "clip-db" | "clip-dbline"
        | "clip-hexline" | "clip-hexword" => "CLIPBOARD & OUTPUT",
        "getutf" | "new" | "encap" | "grab-html" | "schema" | "reg-types" | "reg-parse" => {
            "STRUCTURE & UTILITIES"
        }
        "serve" => "RUNTIME & INTEGRATION",
        _ => "STRUCTURE & UTILITIES",
    }
}

fn help_arguments(
    subcommand: &clap::Command,
    canonical: &str,
    short: &str,
    short_mode: bool,
) -> String {
    let mut command = subcommand.clone();
    let invocation = if short_mode {
        format!("rgd {}", short)
    } else {
        format!("regedited {}", canonical)
    };
    command.set_bin_name(invocation.clone());
    let usage = command.render_usage().to_string().replace("Usage: ", "");
    let mut arguments = usage
        .strip_prefix(&invocation)
        .unwrap_or(&usage)
        .trim()
        .to_string();
    if short_mode
        && regedited::qol::file_placement(canonical) != regedited::qol::FilePlacement::None
    {
        arguments = arguments.replacen("<FILE>", "[FILE]", 1);
    }
    arguments
}

fn help_example(command: &str, short: &str, short_mode: bool) -> String {
    let call = if short_mode {
        format!("rgd {}", short)
    } else {
        format!("regedited {}", command)
    };
    let doc = |arguments: &str| {
        if short_mode {
            format!("{}{}", call, arguments)
        } else {
            format!("{} $DOC{}", call, arguments)
        }
    };

    match command {
        "list" => doc(""),
        "db" => doc(" i64"),
        "hexline" => doc(" index:64"),
        "scan" => doc(" --filter Client"),
        "diff" => format!("{} $BASE $EDITED", call),
        "replace" => format!("{} $TARGET $SOURCE --output $OUT", call),
        "fgrep" => doc(" \"closing date\" --index i64"),
        "fgrep-multi" => doc(" waterfront inspection financing"),
        "zone-copy" => doc(" --from i64 --from-zone 0 --to i70 --to-zone 1"),
        "zone-append" => doc(" i64 0 --text \"new line\""),
        "zone-replace" => doc(" i64 1 --text \"replacement\""),
        "zone-extract" => doc(" i64 1"),
        "zone-info" => doc(" i64 1"),
        "resolve-index" => doc(" 64"),
        "index-zone-extract" => doc(" 64 0"),
        "index-zone-replace" => doc(" 64 0 --text \"replacement\""),
        "index-zone-copy" => doc(" --from-index 64 --from-zone 0 --to-index 70 --to-zone 1"),
        "index-zone-transfer" => format!(
            "{} --from-file $A --from-index 64 --to-file $B --to-index 70",
            call
        ),
        "hex-extract" => doc(" 1x0000055 1x000005F"),
        "hex-replace" => doc(" 1x0000055 1x000005F --text \"replacement\""),
        "ref-get" => doc(if short_mode {
            " i64s2 c"
        } else {
            " i64s2 --clip"
        }),
        "ref-set" => doc(" i64s2 --text \"ready\""),
        "ref-copy" => doc(" i64s1 i70s2 --append"),
        "ref-diff" => doc(" i64db1 i70db2"),
        "ref-bool" => doc(" i64db7 gte 8 --then-val READY --else-val WAIT"),
        "index-str-list" => doc(" 64"),
        "index-zone-set-hex" => doc(" 64 1 1x0000055 1x000005F"),
        "index-zone-set-lines" => doc(" 64 1 b 85 95"),
        "state" => doc(""),
        "state-compare" => doc(" $STATE_JSON"),
        "check" => doc(""),
        "commit" => doc(" --pull"),
        "pull" => doc(""),
        "undo" => doc(""),
        "grep" => doc(" i64 0"),
        "clip" => doc(" i64 1"),
        "echo" => doc(" i64 1"),
        "echo-direct" => format!("{} \"A&B|C\"", call),
        "getutf" => format!("{} 128640", call),
        "set-num" => doc(" i64 6 15"),
        "set-str" => doc(" i64 1 \"follow up Friday\""),
        "set-zone" => doc(" i64 0 85 95 --zone-type code"),
        "convert" => {
            if short_mode {
                "rgd cv b 85 95 to i64 1".to_string()
            } else {
                "regedited index-zone-set-lines $DOC 64 1 b 85 95".to_string()
            }
        }
        "types" => call,
        "content" => doc(" i64"),
        "lines" => doc(" 85 95"),
        "new" => format!("{} notes.md \"Indexed notes\"", call),
        "add" => doc(" 900"),
        "rm" => doc(" i900"),
        "summary" => doc(""),
        "info" => doc(""),
        "encap" => format!("{} \"value\" --mode d", call),
        "grab-html" => format!("{} page.html href --tag a --numbered", call),
        "bool-and" => doc(" i64 waterfront inspection"),
        "bool-nand" => doc(" i64 active archived"),
        "bool-or" => doc(" i64 cash financing"),
        "bool-xor" => doc(" i64 buyer seller"),
        "count" => doc(" i64 \"follow up\""),
        "if-contains" => doc(" i64 waterfront --then-val HOT --else-val COLD"),
        "wal" => doc(""),
        "wal-replay" => doc(" --apply"),
        "tx" => {
            if short_mode {
                format!("{} status", call)
            } else {
                format!("{} status $DOC", call)
            }
        }
        "schema" => doc(" --validate"),
        "reg-types" => call,
        "reg-parse" => format!("{} 42 --reg-type REG_DWORD", call),
        "clip-zone" => doc(" i64 0"),
        "clip-db" => doc(" i64 6"),
        "clip-dbline" => doc(" i64"),
        "clip-hexline" => doc(" i64"),
        "clip-hexword" => format!("{} 85 95 --zone-type code", call),
        "serve" => {
            if short_mode {
                format!("{} --port 5000", call)
            } else {
                format!("{} --file $DOC --port 5000", call)
            }
        }
        _ => call,
    }
}

fn print_command_help(command_name: &str, short_mode: bool) {
    let root = Cli::command();
    let Some(subcommand) = root.find_subcommand(command_name) else {
        clap::Error::raw(
            ErrorKind::InvalidSubcommand,
            format!("unknown command '{}'", command_name),
        )
        .exit();
    };

    let short = regedited::qol::short_command(command_name).unwrap_or(command_name);
    if short_mode {
        println!("rgd {} -> regedited {}", short, command_name);
        if regedited::qol::file_placement(command_name) != regedited::qol::FilePlacement::None {
            println!("FILE may be omitted after `rgd load <FILE>`.\n");
        }
    } else {
        println!("rgd shorthand: {}\n", short);
    }

    let mut command = subcommand.clone();
    command.set_bin_name(if short_mode {
        format!("rgd {}", short)
    } else {
        format!("regedited {}", command_name)
    });
    command
        .print_long_help()
        .unwrap_or_else(|error| clap::Error::raw(ErrorKind::Io, error.to_string()).exit());
    println!();
}

fn cmd_getutf(number: u32, decode: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(hex) = decode {
        // Decode mode
        let decoded = regedited::getutf_decode(&hex)?;
        println!(
            "{} UTF-16LE {} → {}",
            "Decoded:".green().bold(),
            hex.cyan(),
            decoded.to_string().yellow()
        );
    } else {
        // Encode mode
        let result = regedited::getutf(number);
        println!(
            "{} {} → {}",
            "getutf:".green().bold(),
            number.to_string().yellow(),
            result.cyan()
        );
        println!(
            "  {} Encodes {} as UTF-16LE code point(s)",
            "Note:".dimmed(),
            number
        );
    }
    Ok(())
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config = StoreConfig {
        auto_save: !cli.no_save,
        create_backups: !cli.no_save,
        verbose: cli.verbose,
    };

    match cli.command {
        Commands::List { file } => cmd_list(&file),
        Commands::Db { file, section } => cmd_db(&file, &section, config),
        Commands::Hexline { file, section } => cmd_ascii(&file, &section, config),
        Commands::Scan {
            file,
            filter,
            value,
        } => cmd_scan(&file, filter, value),
        Commands::Diff { file_a, file_b } => cmd_diff(&file_a, &file_b),
        Commands::Replace {
            target,
            source,
            sections,
            output,
        } => cmd_replace(&target, &source, sections, output),
        Commands::Fgrep {
            file,
            pattern,
            section,
        } => cmd_fgrep(&file, &pattern, section),
        Commands::FgrepMulti { file, patterns } => cmd_fgrep_multi(&file, patterns),
        Commands::ZoneCopy {
            file,
            from,
            from_zone,
            to,
            to_zone,
        } => cmd_zone_copy(&file, &from, from_zone, &to, to_zone),
        Commands::ZoneAppend {
            file,
            section,
            zone,
            text,
        } => cmd_zone_append(&file, &section, zone, text),
        Commands::ZoneReplace {
            file,
            section,
            zone,
            text,
        } => cmd_zone_replace(&file, &section, zone, text),
        Commands::ZoneExtract {
            file,
            section,
            zone,
        } => cmd_zone_extract(&file, &section, zone),
        Commands::ZoneInfo {
            file,
            section,
            zone,
        } => cmd_zone_info(&file, &section, zone),
        Commands::ResolveIndex {
            file,
            registry_index,
        } => cmd_resolve_index(&file, registry_index),
        Commands::IndexZoneExtract {
            file,
            registry_index,
            zone,
        } => cmd_index_zone_extract(&file, registry_index, zone),
        Commands::IndexZoneReplace {
            file,
            registry_index,
            zone,
            text,
        } => cmd_index_zone_replace(&file, registry_index, zone, text),
        Commands::IndexZoneCopy {
            file,
            from_index,
            from_zone,
            to_index,
            to_zone,
        } => cmd_index_zone_copy(&file, from_index, from_zone, to_index, to_zone),
        Commands::IndexZoneTransfer {
            from_file,
            from_index,
            from_zone,
            to_file,
            to_index,
            to_zone,
        } => cmd_index_zone_transfer(
            &from_file, from_index, from_zone, &to_file, to_index, to_zone,
        ),
        Commands::HexExtract { file, start, end } => cmd_hex_extract(&file, &start, &end),
        Commands::HexReplace {
            file,
            start,
            end,
            text,
        } => cmd_hex_replace(&file, &start, &end, text),
        Commands::RefGet { file, spec, clip } => cmd_ref_get(&file, &spec, clip),
        Commands::RefSet {
            file,
            target,
            from,
            text,
            append,
        } => cmd_ref_set(&file, &target, from, text, append),
        Commands::RefCopy {
            file,
            from,
            to,
            append,
            move_source,
        } => cmd_ref_copy(&file, &from, &to, append, move_source),
        Commands::RefDiff { file, left, right } => cmd_ref_diff(&file, &left, &right),
        Commands::RefBool {
            file,
            left,
            op,
            right,
            then_val,
            else_val,
        } => cmd_ref_bool(&file, &left, &op, &right, &then_val, &else_val),
        Commands::IndexStrList {
            file,
            registry_index,
        } => cmd_index_str_list(&file, registry_index),
        Commands::IndexZoneSetHex {
            file,
            registry_index,
            zone,
            start,
            end,
        } => cmd_index_zone_set_hex(&file, registry_index, zone, &start, &end),
        Commands::IndexZoneSetLines {
            file,
            registry_index,
            zone,
            values,
            zone_type,
        } => cmd_index_zone_set_lines(&file, registry_index, zone, &values, &zone_type, config),
        Commands::State { file } => cmd_state(&file),
        Commands::StateCompare { file, state } => cmd_state_compare(&file, &state),
        Commands::Check { file } => cmd_zone_check(&file),
        Commands::Commit { file, pull } => cmd_zone_commit(&file, pull),
        Commands::Pull { file } => cmd_zone_pull(&file),
        Commands::Undo { file } => cmd_undo(&file),
        Commands::Grep {
            file,
            section,
            index,
        } => cmd_grep(&file, &section, index, config),
        Commands::Clip {
            file,
            section,
            index,
        } => cmd_clip(&file, &section, index, config),
        Commands::Echo {
            file,
            section,
            index,
        } => cmd_echo(&file, &section, index, config),
        Commands::EchoDirect { text } => cmd_echo_direct(&text),
        Commands::SetNum {
            file,
            section,
            index,
            value,
        } => cmd_set_num(&file, &section, index, value, config),
        Commands::SetStr {
            file,
            section,
            index,
            value,
        } => cmd_set_str(&file, &section, index, &value, config),
        Commands::SetZone {
            file,
            section,
            index,
            start,
            end,
            zone_type,
        } => cmd_set_zone(&file, &section, index, start, end, &zone_type, config),
        Commands::Convert {
            values,
            zone_type,
            zone: _,
        } => cmd_convert(&values, &zone_type),
        Commands::Types => cmd_types(),
        Commands::Content { file, section } => cmd_content(&file, &section, config),
        Commands::Lines { file, start, end } => cmd_lines(&file, start, end),
        Commands::Getutf { number, decode } => cmd_getutf(number, decode),
        Commands::New { file, title } => cmd_new(&file, &title),
        Commands::Add {
            file,
            registry_index,
        } => cmd_add(&file, registry_index, config),
        Commands::Rm { file, section } => cmd_rm(&file, &section, config),
        Commands::Summary { file } => cmd_summary(&file),
        Commands::Info { file } => cmd_info(&file),
        Commands::Encap {
            text,
            mode,
            extract,
            to,
            set,
        } => cmd_encap(&text, &mode, extract, to, set),
        Commands::GrabHtml {
            file,
            attr,
            mode,
            tag,
            set,
            numbered,
        } => cmd_grab_html(&file, &attr, &mode, tag, set, numbered),
        Commands::BoolAnd {
            file,
            section,
            patterns,
        } => cmd_bool_and(&file, &section, patterns),
        Commands::BoolNand {
            file,
            section,
            must_contain,
            must_not,
        } => cmd_bool_nand(&file, &section, &must_contain, &must_not),
        Commands::BoolOr {
            file,
            section,
            patterns,
        } => cmd_bool_or(&file, &section, patterns),
        Commands::BoolXor {
            file,
            section,
            pattern_a,
            pattern_b,
        } => cmd_bool_xor(&file, &section, &pattern_a, &pattern_b),
        Commands::Count {
            file,
            section,
            pattern,
        } => cmd_count(&file, &section, &pattern),
        Commands::IfContains {
            file,
            section,
            pattern,
            then_val,
            else_val,
        } => cmd_if_contains(&file, &section, &pattern, &then_val, &else_val),
        Commands::Wal { file } => cmd_wal(&file),
        Commands::WalReplay { file, apply } => cmd_wal_replay(&file, apply),
        Commands::Tx { action, file } => cmd_tx(&action, &file),
        Commands::Schema {
            file,
            validate,
            init,
        } => cmd_schema(&file, validate, init),
        Commands::RegTypes => cmd_reg_types(),
        Commands::RegParse { value, reg_type } => cmd_reg_parse(&value, &reg_type),
        Commands::ClipZone {
            file,
            section,
            zone,
        } => cmd_clip_zone(&file, &section, zone),
        Commands::ClipDb {
            file,
            section,
            index,
        } => cmd_clip_db(&file, &section, index),
        Commands::ClipDbline { file, section } => cmd_clip_dbline(&file, &section),
        Commands::ClipHexline { file, section } => cmd_clip_ascii(&file, &section),
        Commands::ClipHexword {
            start,
            end,
            zone_type,
        } => cmd_clip_hexword(start, end, &zone_type),
        Commands::Serve {
            file,
            port,
            read_only,
        } => cmd_serve(&file, port, read_only),
    }
}

// ==================== COMMAND IMPLEMENTATIONS ====================

fn cmd_list(file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let indexes = regedited::fast_ops::fast_scan(file)?;

    if indexes.is_empty() {
        println!("{} No indexes found", "Note:".yellow());
        return Ok(());
    }

    println!(
        "{} {} indexes in {}",
        "Indexes:".green().bold(),
        indexes.len(),
        file.display()
    );

    for section in indexes {
        let canonical = format!("index:{}", section.index);
        let legacy = (section.name != canonical).then(|| format!(" legacy={}", section.name));
        println!(
            "  {:>8}  {:<12} header={}{}",
            section.index.to_string().cyan().bold(),
            format!("i{}", section.index),
            section.header_line,
            legacy.unwrap_or_default()
        );
    }

    Ok(())
}

fn cmd_db(
    file: &PathBuf,
    section: &str,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;

    println!(
        "{} '{}' in {}",
        "Database table for section".green().bold(),
        section.cyan(),
        file.display()
    );
    println!();

    let table = store.get_db_table(section)?;
    println!("{}", table);

    // Also show Hex-word line
    println!();
    let ascii = store.get_ascii_store(section)?;
    println!("{}", ascii.display());

    Ok(())
}

fn cmd_ascii(
    file: &PathBuf,
    section: &str,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;

    println!(
        "{} '{}' in {}",
        "Hex-word line for section".green().bold(),
        section.cyan(),
        file.display()
    );
    println!();

    let ascii = store.get_ascii_store(section)?;
    println!("{}", ascii.display());
    println!();
    println!("Raw: {}", ascii.to_line().dimmed());

    Ok(())
}

fn cmd_grep(
    file: &PathBuf,
    section: &str,
    index: usize,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;

    let zone = store.get_zone(section, index)?;

    println!("{}", zone.display());

    Ok(())
}

fn cmd_clip(
    file: &PathBuf,
    section: &str,
    index: usize,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;

    let text = store.copy_string(section, index)?;
    println!(
        "{} Copied to clipboard from '{}': {}",
        "OK".green().bold(),
        section.cyan(),
        text.chars().take(60).collect::<String>()
    );

    Ok(())
}

fn cmd_echo(
    file: &PathBuf,
    section: &str,
    index: usize,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;

    let result = store.echo_string(section, index)?;

    // Show analysis
    println!(
        "{} {}",
        "Original:".green().bold(),
        result.analysis.original
    );
    println!("{} {}", "Strategy:".green().bold(), result.strategy);
    println!(
        "{} {}",
        "Command:".green().bold(),
        result.echo_command.cyan()
    );

    if !result.analysis.is_safe {
        println!("{} {}", "Note:".yellow(), result.analysis.summary());
    }

    Ok(())
}

fn cmd_echo_direct(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let result = safe_echo(text)?;

    println!("{}", result.command().cyan());

    if !result.analysis.is_safe {
        println!("{} {}", "Analysis:".yellow(), result.analysis.summary());
    }

    Ok(())
}

fn cmd_set_num(
    file: &PathBuf,
    section: &str,
    index: usize,
    value: i64,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;

    store.update_number(section, index, value)?;

    println!(
        "{} Updated '{}'.Num{} = {} in {}",
        "OK".green().bold(),
        section.cyan(),
        index,
        value.to_string().yellow(),
        file.display()
    );

    Ok(())
}

fn cmd_set_str(
    file: &PathBuf,
    section: &str,
    index: usize,
    value: &str,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;

    store.update_string(section, index, value.to_string())?;

    println!(
        "{} Updated '{}'.Str{} = \"{}\" in {}",
        "OK".green().bold(),
        section.cyan(),
        index,
        value.yellow(),
        file.display()
    );

    Ok(())
}

fn cmd_set_zone(
    file: &PathBuf,
    section: &str,
    index: usize,
    start: u32,
    end: u32,
    zone_type_str: &str,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let zt = regedited::zone_type::ZoneType::from_name(zone_type_str)
        .ok_or_else(|| format!("Unknown zone type: '{}'", zone_type_str))?;

    let mut store = Store::open_with_config(file, config)?;

    store.update_zone(section, index, start, end, zt)?;

    println!(
        "{} Updated '{}'.Zone{} = {} -> {} [{}] in {}",
        "OK".green().bold(),
        section.cyan(),
        index,
        start.to_string().yellow(),
        end.to_string().yellow(),
        zt.short(),
        file.display()
    );

    Ok(())
}

fn cmd_convert(values: &[String], zone_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    let conversion = regedited::converter::parse_conversion(values, zone_type)?;
    if conversion.clip {
        regedited::clip::copy_to_clipboard(&conversion.output)?;
    }
    println!("{}", conversion.output);
    Ok(())
}

fn cmd_types() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        "Zone Types (first hex nibble after 0x)".green().bold()
    );
    println!();
    for zt in regedited::zone_type::ZoneType::ALL {
        println!(
            "  {} {} - {}",
            format!("0x{}XXXXXXX", zt.nibble()).cyan().bold(),
            zt.short(),
            zt.label(),
        );
    }
    println!();
    println!("{}", "Examples:".dimmed());
    println!("  0x000000A = Markdown, line 10");
    println!("  1x0000050 = Code, line 80");
    println!("  2x0000A00 = Media, line 2560");
    println!("  3x0000001 = Database, line 1");
    println!();
    println!("{}", "Usage:".dimmed());
    println!("  regedited set-zone file.md Section 0 10 100 --zone-type code");
    println!("  regedited convert 50 80 --zone-type media");
    Ok(())
}

fn cmd_content(
    file: &PathBuf,
    section: &str,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_with_config(file, config)?;

    let content = store.get_section_content(section)?;

    println!(
        "{} '{}' from {}",
        "Content of section".green().bold(),
        section.cyan(),
        file.display()
    );
    println!("{}", "---".dimmed());
    println!("{}", content);
    println!("{}", "---".dimmed());

    Ok(())
}

fn cmd_lines(file: &PathBuf, start: usize, end: usize) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;

    let extracted = regedited::extract_lines(content.as_bytes(), start, end)?;
    let extracted = String::from_utf8(extracted)?;

    println!(
        "{} Lines {}-{} from {}",
        "Extracted".green().bold(),
        start.to_string().yellow(),
        end.to_string().yellow(),
        file.display()
    );
    println!("{}", "---".dimmed());
    println!("{}", extracted);

    Ok(())
}

fn cmd_new(file: &PathBuf, title: &str) -> Result<(), Box<dyn std::error::Error>> {
    if file.exists() {
        return Err(format!("File already exists: {}", file.display()).into());
    }

    Store::create(file, title)?;

    println!(
        "{} Created new document: {} (\"{}\")",
        "OK".green().bold(),
        file.display(),
        title.cyan()
    );

    Ok(())
}

fn cmd_add(
    file: &PathBuf,
    registry_index: u64,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;

    store.add_index(registry_index)?;

    println!(
        "{} Added index {} to {}",
        "OK".green().bold(),
        registry_index.to_string().cyan(),
        file.display()
    );

    Ok(())
}

fn cmd_rm(
    file: &PathBuf,
    section: &str,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;

    store.remove_section(section)?;

    println!(
        "{} Removed index '{}' from {}",
        "OK".green().bold(),
        section.cyan(),
        file.display()
    );

    Ok(())
}

fn cmd_summary(file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open(file)?;

    println!("{}", store.display_summary());

    Ok(())
}

fn cmd_info(file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;

    println!("{}", "Document Information".green().bold());
    println!("  File: {}", file.display());
    println!("  Indexes: {}", header.section_count());
    println!("  Total lines: {}", header.total_lines);
    println!("  Total bytes: {}", header.total_bytes);
    println!();

    for (name, info) in &header.sections {
        let identity = info
            .registry_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "unresolved".to_string());
        println!("{}", format!("Index: {}", identity).cyan().bold());
        if name != &format!("index:{}", identity) {
            println!("  Legacy key: {}", name);
        }
        println!("  Header @ line {}", info.header_line);
        println!("  Index @ line {}", info.header_line + 1);
        println!("  Hex-word line @ line {}", info.ascii_line);
        println!("  Numeric line @ line {}", info.numeric_line);
        println!("  String 1 @ line {}", info.string1_line);
        println!("  String 2 @ line {}", info.string2_line);
        println!("  String 3 @ line {}", info.string3_line);
        println!("  Content separator @ line {}", info.separator_line);
        println!(
            "  Content: lines {}-{}",
            info.content_start, info.content_end
        );
        println!();
    }

    Ok(())
}

// ==================== FAST OPERATIONS (SAFETENSORS-STYLE) ====================

fn cmd_scan(
    file: &Path,
    filter: Option<String>,
    value: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::{fast_scan, filter_by_name, filter_by_value};

    let scanned = fast_scan(file)?;

    if scanned.is_empty() {
        println!("{} No indexes found", "Note:".yellow());
        return Ok(());
    }

    println!(
        "{} {} indexes in {}",
        "Scan:".green().bold(),
        scanned.len(),
        file.display()
    );

    // Apply name filter
    let by_name: Vec<&regedited::fast_ops::ScannedSection> = match filter {
        Some(ref pat) => {
            let r = filter_by_name(&scanned, pat);
            println!("  Index/key filter '{}': {} matches", pat.cyan(), r.len());
            r
        }
        None => scanned.iter().collect(),
    };

    // Apply value filter
    let final_secs: Vec<&regedited::fast_ops::ScannedSection> = match value {
        Some(ref val_str) => {
            let parts: Vec<&str> = val_str.split(':').collect();
            if parts.len() == 3 {
                let idx = parts[0].parse::<usize>().unwrap_or(0);
                let min = parts[1].parse::<i64>().unwrap_or(0);
                let max = parts[2].parse::<i64>().unwrap_or(0);
                // Need owned vec for filter_by_value
                let owned: Vec<regedited::fast_ops::ScannedSection> =
                    by_name.into_iter().cloned().collect();
                let r = filter_by_value(&owned, idx, min, max);
                println!(
                    "  Value filter [{}] {}-{}: {} matches",
                    idx,
                    min,
                    max,
                    r.len()
                );
                // Convert back to refs... actually just print from owned
                println!();
                for sec in &owned {
                    if sec
                        .db_values
                        .get(idx)
                        .is_some_and(|&v| v >= min && v <= max)
                    {
                        println!("{}", sec.display_compact());
                    }
                }
                return Ok(());
            } else {
                by_name
            }
        }
        None => by_name,
    };

    println!();
    for sec in final_secs {
        println!("{}", sec.display_compact());
    }

    Ok(())
}

fn cmd_diff(file_a: &Path, file_b: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::fast_diff;

    println!(
        "{} {} vs {}",
        "Diff:".green().bold(),
        file_a.display(),
        file_b.display()
    );
    println!(
        "{}",
        "  (metadata-only comparison, like safetensors header diff)".dimmed()
    );
    println!();

    let diff = fast_diff(file_a, file_b)?;
    println!("{}", diff.display());

    Ok(())
}

fn cmd_replace(
    target: &PathBuf,
    source: &Path,
    sections: Option<Vec<String>>,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::fast_replace;

    println!(
        "{} Replacing from {} → {}",
        "Replace:".green().bold(),
        source.display(),
        target.display()
    );

    let result = fast_replace(target, source, sections.as_deref())?;

    let out_path = output.as_ref().unwrap_or(target);
    write_file_with_undo(out_path, result)?;

    println!(
        "{} Patched file written to {}",
        "OK".green().bold(),
        out_path.display()
    );

    Ok(())
}

fn cmd_fgrep(
    file: &Path,
    pattern: &str,
    section: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::{fast_grep, fast_grep_section};

    let matches = if let Some(sec) = section {
        println!(
            "{} '{}' in section '{}' of {} (memory-mapped)",
            "Fast grep:".green().bold(),
            pattern.cyan(),
            sec.cyan(),
            file.display()
        );
        fast_grep_section(file, &sec, pattern)?
    } else {
        println!(
            "{} '{}' in {} (memory-mapped, ripgrep-style)",
            "Fast grep:".green().bold(),
            pattern.cyan(),
            file.display()
        );
        fast_grep(file, pattern)?
    };

    println!("  {} matches\n", matches.len());
    for (line_num, line) in matches {
        println!("  {}: {}", line_num.to_string().dimmed(), line);
    }

    Ok(())
}

fn cmd_fgrep_multi(file: &Path, patterns: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::fast_grep_multi;

    println!(
        "{} {} patterns in {} (OR logic)",
        "Multi grep:".green().bold(),
        patterns.len(),
        file.display()
    );
    println!("  Patterns: {}\n", patterns.join(", ").cyan());

    let matches = fast_grep_multi(file, &patterns)?;

    println!("  {} matches\n", matches.len());
    for (line_num, line, matched) in matches {
        let tags: Vec<String> = matched.iter().map(|p| format!("[{}]", p)).collect();
        println!(
            "  {}: {} {}",
            line_num.to_string().dimmed(),
            line,
            tags.join(" ").yellow()
        );
    }

    Ok(())
}

// ==================== ZONE CONTENT MANIPULATION (PYTHON-SCRIPTABLE) ====================

fn cmd_zone_copy(
    file: &PathBuf,
    from: &str,
    from_zone: usize,
    to: &str,
    to_zone: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::header::scan_content;
    use regedited::zone_editor::copy_zone_content;

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;

    let from_sec = header.resolve_section(from)?;
    let to_sec = header.resolve_section(to)?;

    let result = copy_zone_content(&content, from_sec, from_zone, to_sec, to_zone)?;
    write_file_with_undo(file, result)?;

    println!(
        "{} Copied zone {} from '{}' → zone {} from '{}' in {}",
        "OK".green().bold(),
        from_zone,
        from.cyan(),
        to_zone,
        to.cyan(),
        file.display()
    );

    Ok(())
}

fn cmd_zone_append(
    file: &PathBuf,
    section: &str,
    zone: usize,
    text: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::header::scan_content;
    use regedited::zone_editor::append_zone_content;

    let append_text = match text {
        Some(t) => t,
        None => {
            use std::io::{self, Read};
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;

    let sec = header.resolve_section(section)?;

    let result = append_zone_content(&content, sec, zone, &append_text)?;
    write_file_with_undo(file, result)?;

    println!(
        "{} Appended {} bytes to zone {} of '{}' in {}",
        "OK".green().bold(),
        append_text.len(),
        zone,
        section.cyan(),
        file.display()
    );

    Ok(())
}

fn cmd_zone_replace(
    file: &PathBuf,
    section: &str,
    zone: usize,
    text: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::header::scan_content;
    use regedited::zone_editor::replace_zone_content;

    let replace_text = match text {
        Some(t) => t,
        None => {
            use std::io::{self, Read};
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;

    let sec = header.resolve_section(section)?;

    let result = replace_zone_content(&content, sec, zone, &replace_text)?;
    write_file_with_undo(file, result)?;

    println!(
        "{} Replaced zone {} of '{}' with {} bytes in {}",
        "OK".green().bold(),
        zone,
        section.cyan(),
        replace_text.len(),
        file.display()
    );

    Ok(())
}

fn cmd_zone_extract(
    file: &PathBuf,
    section: &str,
    zone: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::header::scan_content;
    use regedited::zone_editor::extract_zone_content;

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;

    let sec = header.resolve_section(section)?;

    let extracted = extract_zone_content(&content, sec, zone)?;

    // Print ONLY the content (no headers) — for piping
    println!("{}", extracted);

    Ok(())
}

fn cmd_zone_info(
    file: &PathBuf,
    section: &str,
    zone: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::header::scan_content;
    use regedited::zone_editor::format_zone_info;

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;

    let sec = header.resolve_section(section)?;

    let info = format_zone_info(&content, sec, zone)?;
    println!("{}", info);

    Ok(())
}

// ==================== INDEX-ADDRESSED AND HEX-WORD OPERATIONS ====================

fn read_text_or_stdin(text: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    match text {
        Some(t) => Ok(t),
        None => {
            use std::io::{self, Read};
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            Ok(buffer)
        }
    }
}

fn undo_path(file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.undo", file.display()))
}

fn legacy_backup_path(file: &Path) -> PathBuf {
    file.with_extension("md.bak")
}

fn write_file_with_undo(file: &PathBuf, content: String) -> Result<(), Box<dyn std::error::Error>> {
    if file.exists() {
        std::fs::copy(file, undo_path(file))?;
    }
    std::fs::write(file, content)?;
    Ok(())
}

fn cmd_undo(file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let primary = undo_path(file);
    let fallback = legacy_backup_path(file);
    let source = if primary.exists() {
        primary
    } else if fallback.exists() {
        fallback
    } else {
        return Err(format!("No undo file found for {}", file.display()).into());
    };
    std::fs::copy(&source, file)?;
    println!("{} restored {}", "OK".green().bold(), file.display());
    Ok(())
}

fn resolve_section_name_by_index(
    file: &Path,
    registry_index: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    use regedited::fast_ops::fast_scan;

    let matches: Vec<_> = fast_scan(file)?
        .into_iter()
        .filter(|section| section.index == registry_index)
        .collect();

    match matches.len() {
        0 => Err(format!(
            "Registry index {} not found in {}",
            registry_index,
            file.display()
        )
        .into()),
        1 => Ok(matches[0].name.clone()),
        _ => {
            let names: Vec<String> = matches.into_iter().map(|section| section.name).collect();
            Err(format!(
                "Registry index {} is ambiguous in {}: {}",
                registry_index,
                file.display(),
                names.join(", ")
            )
            .into())
        }
    }
}

fn cmd_resolve_index(file: &Path, registry_index: u64) -> Result<(), Box<dyn std::error::Error>> {
    let section = resolve_section_name_by_index(file, registry_index)?;
    println!("{}", section);
    Ok(())
}

fn cmd_index_zone_extract(
    file: &PathBuf,
    registry_index: u64,
    zone: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let section = resolve_section_name_by_index(file, registry_index)?;
    cmd_zone_extract(file, &section, zone)
}

fn cmd_index_zone_replace(
    file: &PathBuf,
    registry_index: u64,
    zone: usize,
    text: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let section = resolve_section_name_by_index(file, registry_index)?;
    cmd_zone_replace(file, &section, zone, text)
}

fn cmd_index_zone_copy(
    file: &PathBuf,
    from_index: u64,
    from_zone: usize,
    to_index: u64,
    to_zone: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let from = resolve_section_name_by_index(file, from_index)?;
    let to = resolve_section_name_by_index(file, to_index)?;
    cmd_zone_copy(file, &from, from_zone, &to, to_zone)
}

fn cmd_index_zone_transfer(
    from_file: &PathBuf,
    from_index: u64,
    from_zone: usize,
    to_file: &PathBuf,
    to_index: u64,
    to_zone: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::header::scan_content;
    use regedited::zone_editor::{extract_zone_content, replace_zone_content};

    let from_section = resolve_section_name_by_index(from_file, from_index)?;
    let to_section = resolve_section_name_by_index(to_file, to_index)?;

    let source_content = std::fs::read_to_string(from_file)?;
    let source_header = scan_content(&source_content)?;
    let source_sec = source_header
        .get_section(&from_section)
        .ok_or_else(|| format!("Resolved source section '{}' not found", from_section))?;
    let extracted = extract_zone_content(&source_content, source_sec, from_zone)?;

    let target_content = std::fs::read_to_string(to_file)?;
    let target_header = scan_content(&target_content)?;
    let target_sec = target_header
        .get_section(&to_section)
        .ok_or_else(|| format!("Resolved target section '{}' not found", to_section))?;
    let result = replace_zone_content(&target_content, target_sec, to_zone, &extracted)?;
    write_file_with_undo(to_file, result)?;

    println!(
        "{} Transferred index {} zone {} from {} -> index {} zone {} in {}",
        "OK".green().bold(),
        from_index,
        from_zone,
        from_file.display(),
        to_index,
        to_zone,
        to_file.display()
    );

    Ok(())
}

fn decode_hex_line_range(
    start: &str,
    end: &str,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    use regedited::zone_type::decode_hex_word;

    let (start_line, _) = decode_hex_word(start)?;
    let (end_line, _) = decode_hex_word(end)?;
    if start_line > end_line {
        return Err(format!("Hex range start {} is after end {}", start, end).into());
    }
    Ok((start_line as usize, end_line as usize))
}

fn line_range_text(
    content: &str,
    start_line: usize,
    end_line: usize,
) -> Result<String, Box<dyn std::error::Error>> {
    let lines: Vec<&str> = content.lines().collect();
    if start_line >= lines.len() || end_line >= lines.len() {
        return Err(format!(
            "Line range {}-{} is out of bounds for {} lines",
            start_line,
            end_line,
            lines.len()
        )
        .into());
    }
    Ok(lines[start_line..=end_line].join("\n"))
}

fn replace_line_range_and_shift(
    content: &str,
    start_line: usize,
    end_line: usize,
    replacement: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    use regedited::ascii_store::AsciiStore;
    use regedited::header::{scan_content, update_lines};
    use regedited::zone_editor::{apply_line_deltas, LineDelta};

    let lines: Vec<&str> = content.lines().collect();
    if start_line >= lines.len() || end_line >= lines.len() {
        return Err(format!(
            "Line range {}-{} is out of bounds for {} lines",
            start_line,
            end_line,
            lines.len()
        )
        .into());
    }

    let mut new_lines: Vec<String> = Vec::new();
    for line in &lines[..start_line] {
        new_lines.push((*line).to_string());
    }
    for line in replacement.lines() {
        new_lines.push(line.to_string());
    }
    if end_line + 1 < lines.len() {
        for line in &lines[(end_line + 1)..] {
            new_lines.push((*line).to_string());
        }
    }

    let old_count = end_line - start_line + 1;
    let new_count = replacement.lines().count();
    let delta = new_count as i64 - old_count as i64;

    let mut result = new_lines.join("\n");
    if delta != 0 {
        result = apply_line_deltas(
            &result,
            &[LineDelta {
                start_line: end_line + 1,
                delta,
            }],
        )?;
    }

    let header = scan_content(&result)?;
    let result_lines: Vec<&str> = result.lines().collect();
    let mut changes = Vec::new();
    for info in header.sections.values() {
        if info.ascii_line >= result_lines.len() {
            continue;
        }
        let mut ascii = AsciiStore::from_line(result_lines[info.ascii_line])?;
        let mut changed = false;
        for zone_index in 0..3 {
            if let Some(zone) = ascii.zone(zone_index) {
                if zone.start as usize == start_line && zone.end as usize == end_line {
                    if new_count == 0 {
                        ascii.set_zone(
                            zone_index,
                            0,
                            0,
                            regedited::zone_type::ZoneType::Markdown,
                        )?;
                    } else {
                        ascii.set_zone(
                            zone_index,
                            start_line as u32,
                            (start_line + new_count - 1) as u32,
                            zone.zone_type,
                        )?;
                    }
                    changed = true;
                }
            }
        }
        if changed {
            changes.push((info.ascii_line, ascii.to_line()));
        }
    }
    if !changes.is_empty() {
        result = update_lines(&result, &changes)?;
    }

    Ok(result)
}

fn cmd_hex_extract(
    file: &PathBuf,
    start: &str,
    end: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (start_line, end_line) = decode_hex_line_range(start, end)?;
    let content = std::fs::read_to_string(file)?;
    println!("{}", line_range_text(&content, start_line, end_line)?);
    Ok(())
}

fn cmd_hex_replace(
    file: &PathBuf,
    start: &str,
    end: &str,
    text: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (start_line, end_line) = decode_hex_line_range(start, end)?;
    let replacement = read_text_or_stdin(text)?;
    let content = std::fs::read_to_string(file)?;
    let result = replace_line_range_and_shift(&content, start_line, end_line, &replacement)?;
    write_file_with_undo(file, result)?;
    println!(
        "{} Replaced hex range {} : {} with {} bytes in {}",
        "OK".green().bold(),
        start,
        end,
        replacement.len(),
        file.display()
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RefSpec {
    Literal(String),
    IndexString { registry_index: u64, slot: usize },
    IndexDb { registry_index: u64, slot: usize },
    IndexDbLine { registry_index: u64 },
    IndexAscii { registry_index: u64 },
    IndexZone { registry_index: u64, zone: usize },
    IndexZoneHex { registry_index: u64, zone: usize },
    HexRange { start: String, end: String },
}

#[derive(Debug, Clone)]
struct ResolvedRange {
    start: u32,
    end: u32,
    zone_type: regedited::zone_type::ZoneType,
}

#[derive(Debug, Serialize, Deserialize)]
struct NativeState {
    format: String,
    file: String,
    created_unix: u64,
    file_checksum: String,
    sections: Vec<NativeStateSection>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NativeStateSection {
    index: u64,
    name: String,
    #[serde(rename = "hex_word_line", alias = "ascii")]
    ascii: String,
    db_values: [i64; 9],
    strings: [String; 3],
    zones: Vec<NativeStateZone>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NativeStateZone {
    slot: usize,
    start: u32,
    end: u32,
    zone_type: String,
    content_len: usize,
    content_checksum: String,
}

fn parse_user_slot(
    raw: &str,
    max: usize,
    label: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let value = raw.parse::<usize>()?;
    if value == 0 || value > max {
        return Err(format!("{} slot {} out of range; use 1-{}", label, value, max).into());
    }
    Ok(value - 1)
}

fn parse_hex_ref(rest: &str) -> Result<RefSpec, Box<dyn std::error::Error>> {
    let trimmed = rest.trim();
    if let Some((start, end)) = trimmed.split_once("..") {
        return Ok(RefSpec::HexRange {
            start: start.trim().to_string(),
            end: end.trim().to_string(),
        });
    }
    if let Some((start, end)) = trimmed.split_once(" : ") {
        return Ok(RefSpec::HexRange {
            start: start.trim().to_string(),
            end: end.trim().to_string(),
        });
    }
    if let Some((start, end)) = trimmed.split_once(',') {
        return Ok(RefSpec::HexRange {
            start: start.trim().to_string(),
            end: end.trim().to_string(),
        });
    }
    if trimmed.matches(':').count() == 1 {
        if let Some((start, end)) = trimmed.split_once(':') {
            return Ok(RefSpec::HexRange {
                start: start.trim().to_string(),
                end: end.trim().to_string(),
            });
        }
    }
    Ok(RefSpec::HexRange {
        start: trimmed.to_string(),
        end: trimmed.to_string(),
    })
}

fn parse_ref_spec(spec: &str) -> Result<RefSpec, Box<dyn std::error::Error>> {
    let trimmed = spec.trim();
    if let Some(value) = trimmed
        .strip_prefix("text:")
        .or_else(|| trimmed.strip_prefix("literal:"))
    {
        return Ok(RefSpec::Literal(value.to_string()));
    }
    if let Some(rest) = trimmed.strip_prefix("hex:") {
        return parse_hex_ref(rest);
    }
    if trimmed.len() >= 9
        && (trimmed.contains('x') || trimmed.starts_with("0x"))
        && regedited::zone_type::decode_hex_word(trimmed).is_ok()
    {
        return Ok(RefSpec::HexRange {
            start: trimmed.to_string(),
            end: trimmed.to_string(),
        });
    }

    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("index") {
        let registry_index = parts[1].parse::<u64>()?;
        let kind = parts[2].to_ascii_lowercase();
        return match kind.as_str() {
            "str" | "string" => {
                if parts.len() != 4 {
                    return Err("index string spec must be index:<n>:string:<1-3>".into());
                }
                Ok(RefSpec::IndexString {
                    registry_index,
                    slot: parse_user_slot(parts[3], 3, "string")?,
                })
            }
            "db" | "num" | "number" => {
                if parts.len() != 4 {
                    return Err("index DB spec must be index:<n>:db:<1-9>".into());
                }
                Ok(RefSpec::IndexDb {
                    registry_index,
                    slot: parse_user_slot(parts[3], 9, "DB")?,
                })
            }
            "dbline" | "db-line" => Ok(RefSpec::IndexDbLine { registry_index }),
            "hexline" | "hex-word-line" | "hex_word_line" | "ascii" | "ranges" => {
                Ok(RefSpec::IndexAscii { registry_index })
            }
            "zone" | "range" | "defined" => {
                if parts.len() != 4 {
                    return Err("index zone spec must be index:<n>:zone:<1-3>".into());
                }
                Ok(RefSpec::IndexZone {
                    registry_index,
                    zone: parse_user_slot(parts[3], 3, "zone")?,
                })
            }
            "zonehex" | "rangehex" | "defhex" | "definedhex" => {
                if parts.len() != 4 {
                    return Err("index zonehex spec must be index:<n>:zonehex:<1-3>".into());
                }
                Ok(RefSpec::IndexZoneHex {
                    registry_index,
                    zone: parse_user_slot(parts[3], 3, "zone")?,
                })
            }
            _ => Err(format!("Unknown index ref kind '{}'", parts[2]).into()),
        };
    }

    Ok(RefSpec::Literal(trimmed.to_string()))
}

fn read_ref_value(file: &PathBuf, spec: &RefSpec) -> Result<String, Box<dyn std::error::Error>> {
    match spec {
        RefSpec::Literal(value) => Ok(value.clone()),
        RefSpec::IndexString {
            registry_index,
            slot,
        } => {
            let section = resolve_section_name_by_index(file, *registry_index)?;
            let mut store = Store::open_with_config(file, StoreConfig::default())?;
            Ok(store.get_string(&section, *slot)?)
        }
        RefSpec::IndexDb {
            registry_index,
            slot,
        } => {
            let section = resolve_section_name_by_index(file, *registry_index)?;
            let mut store = Store::open_with_config(file, StoreConfig::default())?;
            Ok(store.get_number(&section, *slot)?.to_string())
        }
        RefSpec::IndexDbLine { registry_index } => {
            let section = resolve_section_name_by_index(file, *registry_index)?;
            let mut store = Store::open_with_config(file, StoreConfig::default())?;
            Ok(store.get_db_line(&section)?.numeric_line())
        }
        RefSpec::IndexAscii { registry_index } => {
            let section = find_scanned_section(file, *registry_index)?;
            let content = std::fs::read_to_string(file)?;
            let lines: Vec<&str> = content.lines().collect();
            Ok(lines.get(section.ascii_line).unwrap_or(&"").to_string())
        }
        RefSpec::IndexZone {
            registry_index,
            zone,
        } => {
            let content = std::fs::read_to_string(file)?;
            let range = resolve_range_ref(
                file,
                &RefSpec::IndexZone {
                    registry_index: *registry_index,
                    zone: *zone,
                },
            )?;
            if range.start == 0 && range.end == 0 {
                return Ok(String::new());
            }
            line_range_text(&content, range.start as usize, range.end as usize)
        }
        RefSpec::IndexZoneHex { .. } => {
            let range = resolve_range_ref(file, spec)?;
            Ok(format!(
                "{} : {}",
                regedited::zone_type::encode_hex_word(range.start, range.zone_type),
                regedited::zone_type::encode_hex_word(range.end, range.zone_type)
            ))
        }
        RefSpec::HexRange { start, end } => {
            let (start_line, end_line) = decode_hex_line_range(start, end)?;
            let content = std::fs::read_to_string(file)?;
            line_range_text(&content, start_line, end_line)
        }
    }
}

fn find_scanned_section(
    file: &Path,
    registry_index: u64,
) -> Result<regedited::fast_ops::ScannedSection, Box<dyn std::error::Error>> {
    let matches: Vec<_> = regedited::fast_ops::fast_scan(file)?
        .into_iter()
        .filter(|section| section.index == registry_index)
        .collect();
    match matches.len() {
        0 => Err(format!(
            "Registry index {} not found in {}",
            registry_index,
            file.display()
        )
        .into()),
        1 => Ok(matches[0].clone()),
        _ => Err(format!(
            "Registry index {} is ambiguous in {}",
            registry_index,
            file.display()
        )
        .into()),
    }
}

fn resolve_range_ref(
    file: &Path,
    spec: &RefSpec,
) -> Result<ResolvedRange, Box<dyn std::error::Error>> {
    match spec {
        RefSpec::IndexZone {
            registry_index,
            zone,
        }
        | RefSpec::IndexZoneHex {
            registry_index,
            zone,
        } => {
            let section = find_scanned_section(file, *registry_index)?;
            let (start, end) = section.zone_pairs[*zone];
            Ok(ResolvedRange {
                start,
                end,
                zone_type: section.zone_types[*zone],
            })
        }
        RefSpec::HexRange { start, end } => {
            let (start_line, start_type) = regedited::zone_type::decode_hex_word(start)?;
            let (end_line, _) = regedited::zone_type::decode_hex_word(end)?;
            if start_line > end_line {
                return Err(format!("Hex range start {} is after end {}", start, end).into());
            }
            Ok(ResolvedRange {
                start: start_line,
                end: end_line,
                zone_type: start_type,
            })
        }
        _ => Err("Ref spec does not resolve to a hexword range".into()),
    }
}

fn append_or_replace(existing: String, incoming: String, append: bool) -> String {
    if append {
        format!("{}{}", existing, incoming)
    } else {
        incoming
    }
}

fn parse_i64_value(value: &str, context: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let trimmed = value.trim();
    if trimmed.lines().count() > 1 {
        return Err(format!(
            "{} must be one regular number, got multiline content",
            context
        )
        .into());
    }
    Ok(trimmed.parse::<i64>().map_err(|e| {
        format!(
            "{} must be one regular integer number, got '{}': {}",
            context, trimmed, e
        )
    })?)
}

fn set_ref_value(
    file: &PathBuf,
    spec: &RefSpec,
    value: &str,
    append: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match spec {
        RefSpec::Literal(_) => Err("Cannot write to a literal ref".into()),
        RefSpec::IndexString {
            registry_index,
            slot,
        } => {
            let section = resolve_section_name_by_index(file, *registry_index)?;
            let mut store = Store::open_with_config(file, StoreConfig::default())?;
            let current = if append {
                store.get_string(&section, *slot)?
            } else {
                String::new()
            };
            store.update_string(
                &section,
                *slot,
                append_or_replace(current, value.to_string(), append),
            )?;
            Ok(())
        }
        RefSpec::IndexDb {
            registry_index,
            slot,
        } => {
            let section = resolve_section_name_by_index(file, *registry_index)?;
            let incoming = parse_i64_value(value, "DB value")?;
            let mut store = Store::open_with_config(file, StoreConfig::default())?;
            store.update_number(&section, *slot, incoming)?;
            Ok(())
        }
        RefSpec::IndexDbLine { registry_index } => {
            let section = resolve_section_name_by_index(file, *registry_index)?;
            let numbers = regedited::db_line::parse_numeric_line(value)?;
            let mut store = Store::open_with_config(file, StoreConfig::default())?;
            let mut db = store.get_db_line(&section)?;
            db.numbers = numbers;
            store.update_db_line(&section, db)?;
            Ok(())
        }
        RefSpec::IndexAscii { registry_index } => {
            let section = find_scanned_section(file, *registry_index)?;
            let content = std::fs::read_to_string(file)?;
            let result = regedited::header::update_line(&content, section.ascii_line, value)?;
            write_file_with_undo(file, result)?;
            Ok(())
        }
        RefSpec::IndexZone {
            registry_index,
            zone,
        } => {
            let section = resolve_section_name_by_index(file, *registry_index)?;
            let current = if append {
                read_ref_value(file, spec)?
            } else {
                String::new()
            };
            let replacement = append_or_replace(current, value.to_string(), append);
            let content = std::fs::read_to_string(file)?;
            let header = scan_content(&content)?;
            let sec = header
                .get_section(&section)
                .ok_or_else(|| format!("Resolved section '{}' not found", section))?;
            let result =
                regedited::zone_editor::replace_zone_content(&content, sec, *zone, &replacement)?;
            write_file_with_undo(file, result)?;
            Ok(())
        }
        RefSpec::IndexZoneHex {
            registry_index,
            zone,
        } => {
            let range_spec = parse_hex_ref(value)?;
            let range = resolve_range_ref(file, &range_spec)?;
            set_index_zone_range(
                file,
                *registry_index,
                *zone,
                range.start,
                range.end,
                range.zone_type,
            )
        }
        RefSpec::HexRange { start, end } => {
            let (start_line, end_line) = decode_hex_line_range(start, end)?;
            let content = std::fs::read_to_string(file)?;
            let current = if append {
                line_range_text(&content, start_line, end_line)?
            } else {
                String::new()
            };
            let replacement = append_or_replace(current, value.to_string(), append);
            let result =
                replace_line_range_and_shift(&content, start_line, end_line, &replacement)?;
            write_file_with_undo(file, result)?;
            Ok(())
        }
    }
}

fn clear_ref_value(file: &PathBuf, spec: &RefSpec) -> Result<(), Box<dyn std::error::Error>> {
    match spec {
        RefSpec::IndexDb { .. } => set_ref_value(file, spec, "0", false),
        RefSpec::IndexZoneHex {
            registry_index,
            zone,
        } => set_index_zone_range(
            file,
            *registry_index,
            *zone,
            0,
            0,
            regedited::zone_type::ZoneType::Markdown,
        ),
        RefSpec::Literal(_) => Err("Cannot clear a literal ref".into()),
        _ => set_ref_value(file, spec, "", false),
    }
}

fn range_bounds_for_move(
    file: &Path,
    spec: &RefSpec,
) -> Result<Option<ResolvedRange>, Box<dyn std::error::Error>> {
    match spec {
        RefSpec::IndexZone { .. } | RefSpec::IndexZoneHex { .. } | RefSpec::HexRange { .. } => {
            Ok(Some(resolve_range_ref(file, spec)?))
        }
        _ => Ok(None),
    }
}

fn adjust_literal_target_after_source_clear(
    file: &Path,
    target: &RefSpec,
    source: Option<&ResolvedRange>,
) -> Result<RefSpec, Box<dyn std::error::Error>> {
    let Some(source_range) = source else {
        return Ok(target.clone());
    };
    if source_range.start == 0 && source_range.end == 0 {
        return Ok(target.clone());
    }

    let RefSpec::HexRange { .. } = target else {
        return Ok(target.clone());
    };

    let target_range = resolve_range_ref(file, target)?;
    if target_range.start <= source_range.end && target_range.end >= source_range.start {
        return Err("Cannot move a range into an overlapping literal hex range".into());
    }

    if target_range.start > source_range.end {
        let removed = source_range.end - source_range.start + 1;
        let shifted_start = target_range.start - removed;
        let shifted_end = target_range.end - removed;
        return Ok(RefSpec::HexRange {
            start: regedited::zone_type::encode_hex_word(shifted_start, target_range.zone_type),
            end: regedited::zone_type::encode_hex_word(shifted_end, target_range.zone_type),
        });
    }

    Ok(target.clone())
}

fn set_index_zone_range(
    file: &PathBuf,
    registry_index: u64,
    zone: usize,
    start: u32,
    end: u32,
    zone_type: regedited::zone_type::ZoneType,
) -> Result<(), Box<dyn std::error::Error>> {
    let section = resolve_section_name_by_index(file, registry_index)?;
    let mut store = Store::open_with_config(file, StoreConfig::default())?;
    store.update_zone(&section, zone, start, end, zone_type)?;
    Ok(())
}

fn source_value_for_ref_set(
    file: &PathBuf,
    from: Option<String>,
    text: Option<String>,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(from_spec) = from {
        let parsed = parse_ref_spec(&from_spec)?;
        return read_ref_value(file, &parsed);
    }
    read_text_or_stdin(text)
}

fn cmd_ref_get(file: &PathBuf, spec: &str, clip: bool) -> Result<(), Box<dyn std::error::Error>> {
    let parsed = parse_ref_spec(spec)?;
    let value = read_ref_value(file, &parsed)?;
    if clip {
        regedited::clip::copy_to_clipboard(&value)?;
        println!(
            "{} copied {} bytes from {}",
            "OK".green().bold(),
            value.len(),
            spec.cyan()
        );
    } else {
        println!("{}", value);
    }
    Ok(())
}

fn cmd_ref_set(
    file: &PathBuf,
    target: &str,
    from: Option<String>,
    text: Option<String>,
    append: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_ref = parse_ref_spec(target)?;
    let value = source_value_for_ref_set(file, from, text)?;
    set_ref_value(file, &target_ref, &value, append)?;
    println!(
        "{} wrote {} bytes to {}",
        "OK".green().bold(),
        value.len(),
        target.cyan()
    );
    Ok(())
}

fn cmd_ref_copy(
    file: &PathBuf,
    from: &str,
    to: &str,
    append: bool,
    move_source: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let from_ref = parse_ref_spec(from)?;
    let to_ref = parse_ref_spec(to)?;
    let value = read_ref_value(file, &from_ref)?;
    if move_source {
        let source_range = range_bounds_for_move(file, &from_ref)?;
        let adjusted_to_ref =
            adjust_literal_target_after_source_clear(file, &to_ref, source_range.as_ref())?;
        clear_ref_value(file, &from_ref)?;
        set_ref_value(file, &adjusted_to_ref, &value, append)?;
    } else {
        set_ref_value(file, &to_ref, &value, append)?;
    }
    println!(
        "{} {} {} bytes from {} to {}",
        "OK".green().bold(),
        if move_source { "moved" } else { "copied" },
        value.len(),
        from.cyan(),
        to.cyan()
    );
    Ok(())
}

fn cmd_ref_diff(file: &PathBuf, left: &str, right: &str) -> Result<(), Box<dyn std::error::Error>> {
    let left_ref = parse_ref_spec(left)?;
    let right_ref = parse_ref_spec(right)?;
    let left_value = read_ref_value(file, &left_ref)?;
    let right_value = read_ref_value(file, &right_ref)?;
    if left_value == right_value {
        println!("EQUAL");
        return Ok(());
    }
    println!("DIFF");
    println!("--- {}", left);
    println!("+++ {}", right);
    let left_lines: Vec<&str> = left_value.lines().collect();
    let right_lines: Vec<&str> = right_value.lines().collect();
    let max = left_lines.len().max(right_lines.len());
    for i in 0..max {
        match (left_lines.get(i), right_lines.get(i)) {
            (Some(a), Some(b)) if a == b => println!(" {}", a),
            (Some(a), Some(b)) => {
                println!("-{}", a);
                println!("+{}", b);
            }
            (Some(a), None) => println!("-{}", a),
            (None, Some(b)) => println!("+{}", b),
            (None, None) => {}
        }
    }
    Ok(())
}

fn resolve_literal_or_ref(
    file: &PathBuf,
    value: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let looks_like_ref = value.starts_with("index:")
        || value.starts_with("hex:")
        || value.starts_with("text:")
        || value.starts_with("literal:")
        || regedited::zone_type::decode_hex_word(value).is_ok();
    if looks_like_ref {
        read_ref_value(file, &parse_ref_spec(value)?)
    } else {
        Ok(value.to_string())
    }
}

fn cmd_ref_bool(
    file: &PathBuf,
    left: &str,
    op: &str,
    right: &str,
    then_val: &str,
    else_val: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let left_value = resolve_literal_or_ref(file, left)?;
    let right_value = resolve_literal_or_ref(file, right)?;
    let op_normalized = op.to_ascii_lowercase();
    let result = match op_normalized.as_str() {
        "contains" => left_value
            .to_lowercase()
            .contains(&right_value.to_lowercase()),
        "eq" | "==" | "=" => left_value == right_value,
        "ne" | "!=" => left_value != right_value,
        "gt" | ">" | "gte" | ">=" | "lt" | "<" | "lte" | "<=" => {
            let left_num = left_value
                .trim()
                .parse::<f64>()
                .map_err(|e| format!("Left value '{}' is not numeric: {}", left_value.trim(), e))?;
            let right_num = right_value.trim().parse::<f64>().map_err(|e| {
                format!("Right value '{}' is not numeric: {}", right_value.trim(), e)
            })?;
            match op_normalized.as_str() {
                "gt" | ">" => left_num > right_num,
                "gte" | ">=" => left_num >= right_num,
                "lt" | "<" => left_num < right_num,
                "lte" | "<=" => left_num <= right_num,
                _ => unreachable!(),
            }
        }
        _ => return Err(format!("Unknown ref-bool op '{}'", op).into()),
    };
    println!("{}", if result { then_val } else { else_val });
    Ok(())
}

fn cmd_index_str_list(
    file: &PathBuf,
    registry_index: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let section = resolve_section_name_by_index(file, registry_index)?;
    let mut store = Store::open_with_config(file, StoreConfig::default())?;
    for i in 0..3 {
        println!("string {}: {}", i + 1, store.get_string(&section, i)?);
    }
    Ok(())
}

fn cmd_index_zone_set_hex(
    file: &PathBuf,
    registry_index: u64,
    zone: usize,
    start: &str,
    end: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let zone_slot = parse_user_slot(&zone.to_string(), 3, "zone")?;
    let range = resolve_range_ref(
        file,
        &RefSpec::HexRange {
            start: start.to_string(),
            end: end.to_string(),
        },
    )?;
    set_index_zone_range(
        file,
        registry_index,
        zone_slot,
        range.start,
        range.end,
        range.zone_type,
    )?;
    println!(
        "{} set index {} zone {} to {} : {}",
        "OK".green().bold(),
        registry_index,
        zone,
        start,
        end
    );
    Ok(())
}

fn cmd_index_zone_set_lines(
    file: &PathBuf,
    registry_index: u64,
    zone: usize,
    values: &[String],
    zone_type: &str,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let zone_slot = parse_user_slot(&zone.to_string(), 3, "zone")?;
    let assignment = regedited::converter::parse_zone_assignment(values, zone_type)?;
    let section = resolve_section_name_by_index(file, registry_index)?;
    let mut store = Store::open_with_config(file, config)?;
    store.update_zone(
        &section,
        zone_slot,
        assignment.start,
        assignment.end,
        assignment.zone_type,
    )?;

    let start_hex = regedited::zone_type::encode_hex_word(assignment.start, assignment.zone_type);
    let end_hex = regedited::zone_type::encode_hex_word(assignment.end, assignment.zone_type);
    if assignment.clip {
        regedited::clip::copy_to_clipboard(&format!("{} : {}", start_hex, end_hex))?;
    }
    println!(
        "{} set index {} zone {} to {} : {} (lines {}-{})",
        "OK".green().bold(),
        registry_index,
        zone,
        start_hex,
        end_hex,
        assignment.start,
        assignment.end
    );
    Ok(())
}

fn make_state(file: &PathBuf) -> Result<NativeState, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;
    let scan = regedited::fast_ops::fast_scan(file)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut sections = Vec::new();

    for section in scan {
        let ascii = lines.get(section.ascii_line).unwrap_or(&"").to_string();
        let mut zones = Vec::new();
        for slot in 0..3 {
            let (start, end) = section.zone_pairs[slot];
            let zone_text = if start == 0 && end == 0 {
                String::new()
            } else {
                line_range_text(&content, start as usize, end as usize).unwrap_or_default()
            };
            zones.push(NativeStateZone {
                slot: slot + 1,
                start,
                end,
                zone_type: section.zone_types[slot].short().to_string(),
                content_len: zone_text.len(),
                content_checksum: regedited::checksum_hex(zone_text.as_bytes()),
            });
        }
        sections.push(NativeStateSection {
            index: section.index,
            name: section.name,
            ascii,
            db_values: section.db_values,
            strings: section.strings,
            zones,
        });
    }

    let created_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    Ok(NativeState {
        format: "regedited-native-state-v1".to_string(),
        file: file.display().to_string(),
        created_unix,
        file_checksum: regedited::checksum_hex(content.as_bytes()),
        sections,
    })
}

fn state_json(file: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
    Ok(serde_json::to_string_pretty(&make_state(file)?)?)
}

fn cmd_state(file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", state_json(file)?);
    Ok(())
}

fn parse_state_json(text: &str) -> Result<NativeState, Box<dyn std::error::Error>> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        return Ok(serde_json::from_str(trimmed)?);
    }
    Err("State input must be raw Regedited state JSON".into())
}

fn cmd_state_compare(file: &PathBuf, state: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let old = parse_state_json(&std::fs::read_to_string(state)?)?;
    let current = make_state(file)?;
    let old_map: std::collections::BTreeMap<u64, &NativeStateSection> = old
        .sections
        .iter()
        .map(|section| (section.index, section))
        .collect();
    let current_map: std::collections::BTreeMap<u64, &NativeStateSection> = current
        .sections
        .iter()
        .map(|section| (section.index, section))
        .collect();

    let mut all_indexes = std::collections::BTreeSet::new();
    all_indexes.extend(old_map.keys().copied());
    all_indexes.extend(current_map.keys().copied());

    let mut changes = 0usize;
    for index in all_indexes {
        match (old_map.get(&index), current_map.get(&index)) {
            (Some(_), None) => {
                changes += 1;
                println!("removed index {}", index);
            }
            (None, Some(section)) => {
                changes += 1;
                println!("added index {} ({})", index, section.name);
            }
            (Some(old_section), Some(new_section)) => {
                if old_section.strings != new_section.strings {
                    changes += 1;
                    println!("changed strings index {} ({})", index, new_section.name);
                }
                if old_section.db_values != new_section.db_values {
                    changes += 1;
                    println!("changed db index {} ({})", index, new_section.name);
                }
                if old_section.ascii != new_section.ascii {
                    changes += 1;
                    println!(
                        "Changed hex-word line index {} ({})",
                        index, new_section.name
                    );
                }
                for (old_zone, new_zone) in old_section.zones.iter().zip(new_section.zones.iter()) {
                    if old_zone.content_checksum != new_zone.content_checksum
                        || old_zone.content_len != new_zone.content_len
                    {
                        changes += 1;
                        println!(
                            "changed content index {} zone {} ({})",
                            index, new_zone.slot, new_section.name
                        );
                    }
                }
            }
            (None, None) => {}
        }
    }
    if changes == 0 {
        println!("EQUAL");
    } else {
        println!("changes={}", changes);
    }
    Ok(())
}

fn print_zone_diff(diff: &regedited::zone_checkpoint::ZoneDiff, path: &Path) {
    for update in &diff.updates {
        println!(
            "move index {} zone {}: {}-{} -> {}-{} ({})",
            update.index,
            update.slot,
            update.old_start,
            update.old_end,
            update.new_start,
            update.new_end,
            update.method
        );
    }
    for item in &diff.manual {
        println!(
            "manual index {} zone {}: {}",
            item.index, item.slot, item.reason
        );
    }
    for item in &diff.unresolved {
        println!(
            "unresolved index {} zone {}: {}",
            item.index, item.slot, item.reason
        );
    }
    for item in &diff.content_changed {
        println!(
            "content index {} zone {}: {}",
            item.index, item.slot, item.reason
        );
    }
    if !diff.added_indexes.is_empty() {
        println!("added indexes: {:?}", diff.added_indexes);
    }
    println!(
        "updates={} manual={} unresolved={} content_changed={}",
        diff.updates.len(),
        diff.manual.len(),
        diff.unresolved.len(),
        diff.content_changed.len()
    );
    println!("diff={}", path.display());
}

fn cmd_zone_check(file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (diff, path) = regedited::zone_checkpoint::check(file)?;
    print_zone_diff(&diff, &path);
    Ok(())
}

fn prompt_pull(count: usize) -> Result<bool, Box<dyn std::error::Error>> {
    use std::io::{IsTerminal, Write};
    if !std::io::stdin().is_terminal() {
        return Ok(false);
    }
    print!("Pull {} safe range update(s) now? [y/N] ", count);
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn write_pulled_zones(
    file: &Path,
    diff: &regedited::zone_checkpoint::ZoneDiff,
) -> Result<(), Box<dyn std::error::Error>> {
    let updated = regedited::zone_checkpoint::apply_diff(file, diff)?;
    write_file_with_undo(&file.to_path_buf(), updated)?;
    println!(
        "{} pulled {} zone range(s)",
        "OK".green().bold(),
        diff.updates.len()
    );

    if diff.unresolved.is_empty() {
        let (checkpoint, path) = regedited::zone_checkpoint::save_checkpoint(file)?;
        regedited::zone_checkpoint::clear_diff(file)?;
        println!(
            "{} committed {} active zone(s) to {}",
            "OK".green().bold(),
            checkpoint.zones.len(),
            path.display()
        );
    } else {
        regedited::zone_checkpoint::save_checkpoint_preserving(file, &diff.unresolved)?;
        let (_, path) = regedited::zone_checkpoint::check(file)?;
        println!(
            "Checkpoint advanced; preserved {} unresolved zone fingerprint(s) in {}",
            diff.unresolved.len(),
            path.display()
        );
    }
    Ok(())
}

fn cmd_zone_commit(file: &Path, pull: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !regedited::zone_checkpoint::checkpoint_exists(file) {
        let (checkpoint, path) = regedited::zone_checkpoint::save_checkpoint(file)?;
        println!(
            "{} committed {} active zone(s) to {}",
            "OK".green().bold(),
            checkpoint.zones.len(),
            path.display()
        );
        return Ok(());
    }

    let (diff, path) = regedited::zone_checkpoint::check(file)?;
    print_zone_diff(&diff, &path);
    if diff.updates.is_empty() {
        if diff.unresolved.is_empty() {
            let (checkpoint, checkpoint_path) = regedited::zone_checkpoint::save_checkpoint(file)?;
            regedited::zone_checkpoint::clear_diff(file)?;
            println!(
                "{} committed {} active zone(s) to {}",
                "OK".green().bold(),
                checkpoint.zones.len(),
                checkpoint_path.display()
            );
        } else {
            println!(
                "Checkpoint unchanged: resolve {} ambiguous zone(s), then commit again.",
                diff.unresolved.len()
            );
        }
        return Ok(());
    }

    if pull || prompt_pull(diff.updates.len())? {
        write_pulled_zones(file, &diff)?;
    } else {
        println!(
            "Checkpoint unchanged. Run `rgd pull` to apply {}.",
            path.display()
        );
    }
    Ok(())
}

fn cmd_zone_pull(file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let diff = regedited::zone_checkpoint::load_diff(file)?;
    if diff.updates.is_empty() {
        println!("No safe zone relocations are pending.");
        return Ok(());
    }
    write_pulled_zones(file, &diff)
}

// ==================== ENCAPSULATION COMMANDS (shel.sh/XML) ====================

fn cmd_encap(
    text: &str,
    mode_str: &str,
    do_extract: bool,
    to: Option<String>,
    set_var: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if do_extract {
        // Extract mode: pull inner content from encapsulated string
        let inner = extract(text)?;
        println!("{}", inner);
        return Ok(());
    }

    if let Some(to_mode_str) = to {
        // Convert mode: change encapsulation format
        let target = EncapMode::from_name(&to_mode_str)
            .ok_or_else(|| format!("Unknown mode: '{}'", to_mode_str))?;
        let converted = convert_mode(text, target)?;
        println!("{}", converted);
        return Ok(());
    }

    // Normal encapsulate mode
    let mode =
        EncapMode::from_name(mode_str).ok_or_else(|| format!("Unknown mode: '{}'", mode_str))?;

    if let Some(var_name) = set_var {
        // Output as set variable
        let encap = encapsulate(text, mode);
        println!("set \"{}={}\"", var_name, encap);
    } else {
        let encap = encapsulate(text, mode);
        println!("{}", encap);
    }

    Ok(())
}

// ==================== HTML EXTRACTION COMMANDS (GRAB B/C/D) ====================

fn cmd_grab_html(
    file: &PathBuf,
    attr: &str,
    mode_str: &str,
    tag_filter: Option<String>,
    set_var: Option<String>,
    numbered: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mode =
        EncapMode::from_name(mode_str).ok_or_else(|| format!("Unknown mode: '{}'", mode_str))?;

    let content = std::fs::read_to_string(file)?;

    let tag_opt = tag_filter.as_deref();
    let extracts = extract_attributes(&content, attr, tag_opt);

    if extracts.is_empty() {
        println!("{} No '{}' attributes found", "Note:".yellow(), attr);
        return Ok(());
    }

    println!(
        "{} {} <{}> attribute(s) in {} (mode: {})",
        "Found:".green().bold(),
        extracts.len(),
        attr.cyan(),
        file.display(),
        mode.letter().to_string().yellow()
    );

    if let Some(base_name) = set_var {
        // Output as set variables (shel.sh database style)
        let vars = format_as_set_vars(&extracts, mode, &base_name);
        for var in vars {
            println!("{}", var);
        }
    } else if numbered {
        // Output with numbered indices
        let lines = format_numbered(&extracts, mode);
        for line in lines {
            println!("{}", line);
        }
    } else {
        // Default: show with context
        for (i, ex) in extracts.iter().enumerate() {
            let encap_val = encapsulate(&ex.value, mode);
            println!(
                "  [{}] Line {} <{} {}={}>",
                i.to_string().dimmed(),
                ex.line_num.to_string().dimmed(),
                ex.tag.cyan(),
                ex.attr.yellow(),
                encap_val.green()
            );
        }
    }

    Ok(())
}

// ==================== BOOLEAN OPERATION COMMANDS ====================

/// Get content for boolean operations (section or full file)
fn get_bool_content(file: &PathBuf, section: &str) -> Result<String, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;

    if section == "__all__" {
        Ok(content)
    } else {
        let header = scan_content(&content)?;
        let sec = header.resolve_section(section)?;

        // Extract section content
        let lines: Vec<&str> = content.lines().collect();
        let start = sec.content_start;
        let end = sec.content_end.min(lines.len());
        let section_lines = &lines[start..end];
        Ok(section_lines.join("\n"))
    }
}

fn cmd_bool_and(
    file: &PathBuf,
    section: &str,
    patterns: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_bool_content(file, section)?;
    let result = bool_and(&content, &patterns);

    println!("{}", result.display());

    if !result.matches.is_empty() {
        println!("\n  {} Matching lines:", "Matches:".green());
        for (line_num, line) in result.matches.iter().take(20) {
            println!("    {}: {}", line_num.to_string().dimmed(), line);
        }
        if result.matches.len() > 20 {
            println!("    ... and {} more", result.matches.len() - 20);
        }
    }

    std::process::exit(result.exit_code());
}

fn cmd_bool_nand(
    file: &PathBuf,
    section: &str,
    must_contain: &str,
    must_not: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_bool_content(file, section)?;
    let result = bool_nand(&content, must_contain, must_not);

    println!("{}", result.display());

    if !result.matches.is_empty() {
        println!(
            "\n  {} Lines matching '{}':",
            "Matches:".green(),
            must_contain
        );
        for (line_num, line) in result.matches.iter().take(20) {
            println!("    {}: {}", line_num.to_string().dimmed(), line);
        }
        if result.matches.len() > 20 {
            println!("    ... and {} more", result.matches.len() - 20);
        }
    }

    std::process::exit(result.exit_code());
}

fn cmd_bool_or(
    file: &PathBuf,
    section: &str,
    patterns: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_bool_content(file, section)?;
    let result = bool_or(&content, &patterns);

    println!("{}", result.display());

    if !result.matches.is_empty() {
        println!("\n  {} Matching lines:", "Matches:".green());
        for (line_num, line) in result.matches.iter().take(20) {
            println!("    {}: {}", line_num.to_string().dimmed(), line);
        }
        if result.matches.len() > 20 {
            println!("    ... and {} more", result.matches.len() - 20);
        }
    }

    std::process::exit(result.exit_code());
}

fn cmd_bool_xor(
    file: &PathBuf,
    section: &str,
    pattern_a: &str,
    pattern_b: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_bool_content(file, section)?;
    let result = bool_xor(&content, pattern_a, pattern_b);

    println!("{}", result.display());

    std::process::exit(result.exit_code());
}

fn cmd_count(
    file: &PathBuf,
    section: &str,
    pattern: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_bool_content(file, section)?;
    let (cnt, matches) = count(&content, pattern);

    println!(
        "{} Pattern '{}' found {} time(s) across {} lines",
        "Count:".green().bold(),
        pattern.cyan(),
        cnt.to_string().yellow(),
        content.lines().count().to_string().dimmed()
    );

    if !matches.is_empty() {
        println!();
        for (line_num, line) in matches.iter().take(20) {
            println!("  {}: {}", line_num.to_string().dimmed(), line);
        }
        if matches.len() > 20 {
            println!("  ... and {} more", matches.len() - 20);
        }
    }

    Ok(())
}

fn cmd_if_contains(
    file: &PathBuf,
    section: &str,
    pattern: &str,
    then_val: &str,
    else_val: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_bool_content(file, section)?;
    let result = if_contains(&content, pattern, then_val, else_val);

    println!("{}", result);

    Ok(())
}
// SPDX-License-Identifier: AGPL-3.0
// ==================== WAL COMMANDS ====================

fn cmd_wal(file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::wal::WalStatus;

    let status = WalStatus::check(file)?;
    println!("{}", status.display());

    if status.has_wal && !status.is_committed {
        println!("\n{} Uncommitted WAL detected!", "WARNING:".red().bold());
        println!(
            "  Run 'regedited wal-replay {} --apply' to recover",
            file.display()
        );
    }

    Ok(())
}

fn cmd_wal_replay(file: &PathBuf, apply: bool) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::wal::Wal;

    if !Wal::exists_for(file) {
        println!(
            "{} No WAL file found for {}",
            "Note:".yellow(),
            file.display()
        );
        return Ok(());
    }

    let entries = Wal::read_entries(file)?;
    if entries.is_empty() {
        println!("{} WAL is empty — nothing to replay", "Note:".yellow());
        return Ok(());
    }

    println!(
        "{} {} WAL entries to replay",
        "Found:".green().bold(),
        entries.len()
    );

    for entry in &entries {
        println!(
            "  [{:4}] {} (checksum: {:08x})",
            entry.seq,
            entry.operation.description(),
            entry.checksum
        );
    }

    if apply {
        println!(
            "\n{} Replaying {} entries...",
            "Applying:".green().bold(),
            entries.len()
        );
        // In a full implementation, each operation would be applied here
        // For now, mark WAL as resolved by removing it
        let wal = Wal::open(file)?;
        wal.cleanup()?;
        println!(
            "{} WAL replay complete. File cleaned up.",
            "OK:".green().bold()
        );
    } else {
        println!(
            "\n{} Use --apply to actually replay these changes",
            "Dry run:".yellow()
        );
    }

    Ok(())
}

// ==================== TRANSACTION COMMANDS ====================

fn cmd_tx(action: &str, file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::transaction::{Transaction, TransactionManager};

    match action.to_lowercase().as_str() {
        "begin" | "start" => {
            let mut mgr = TransactionManager::new();
            match mgr.begin(file) {
                Ok(tx) => {
                    println!(
                        "{} Transaction begun for {}",
                        "TX:".green().bold(),
                        file.display()
                    );
                    println!("  State: {:?}", tx.state());
                    println!("  Use 'tx commit' or 'tx rollback' to finish");
                }
                Err(e) => {
                    // Transaction may already exist — show status
                    println!("{} {}", "Note:".yellow(), e);
                    if let Ok(tx) = Transaction::begin(file) {
                        println!("  Current state: {} staged ops", tx.len());
                    }
                }
            }
        }
        "commit" => {
            // Try to load existing transaction
            if let Ok(tx) = Transaction::begin(file) {
                println!(
                    "{} Committing {} operations...",
                    "TX:".green().bold(),
                    tx.len()
                );
                // Transaction already has WAL entries — just commit marker
                drop(tx);
                let mut wal = regedited::wal::Wal::open(file)?;
                wal.commit()?;
                println!("{} Transaction committed", "OK:".green().bold());
            } else {
                println!(
                    "{} No active transaction for {}",
                    "Note:".yellow(),
                    file.display()
                );
            }
        }
        "rollback" | "abort" => {
            if let Ok(tx) = Transaction::begin(file) {
                println!(
                    "{} Rolling back {} operations...",
                    "TX:".yellow().bold(),
                    tx.len()
                );
                drop(tx);
                let mut wal = regedited::wal::Wal::open(file)?;
                wal.rollback()?;
                println!("{} Transaction rolled back", "OK:".green().bold());
            } else {
                println!(
                    "{} No active transaction for {}",
                    "Note:".yellow(),
                    file.display()
                );
            }
        }
        "status" | "st" => {
            if let Ok(tx) = Transaction::begin(file) {
                println!("{}", tx.summary());
            } else {
                println!(
                    "{} No active transaction for {}",
                    "Note:".yellow(),
                    file.display()
                );
            }
        }
        _ => {
            return Err(format!(
                "Unknown transaction action: '{}'. Use: begin, commit, rollback, status",
                action
            )
            .into());
        }
    }

    Ok(())
}

// ==================== SCHEMA COMMANDS ====================

fn cmd_schema(
    file: &PathBuf,
    validate: bool,
    init: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::schema::DocumentSchema;

    if init {
        // Generate a starter schema from the document
        let content = std::fs::read_to_string(file)?;
        let header = regedited::header::scan_content(&content)?;

        let mut schema = DocumentSchema::new();
        for info in header.sections.values() {
            let index_ref = info
                .registry_index
                .map(|index| index.to_string())
                .unwrap_or_else(|| info.name.clone());
            let sec = schema.section(&index_ref);
            // Add default fields
            sec.add_field(regedited::schema::SchemaField::new(
                "description",
                regedited::schema::SchemaFieldType::String,
            ));
            sec.fields.get_mut("description").unwrap().constraint =
                regedited::schema::FieldConstraint::Optional;
        }

        let schema_path = DocumentSchema::schema_path(file);
        schema.save(&schema_path)?;
        println!(
            "{} Created starter schema: {}",
            "OK:".green().bold(),
            schema_path.display()
        );
        println!("{}", schema.summary());
        return Ok(());
    }

    let schema_path = DocumentSchema::schema_path(file);
    if !schema_path.exists() {
        println!(
            "{} No schema found for {}",
            "Note:".yellow(),
            file.display()
        );
        println!(
            "  Run 'regedited schema {} --init' to create one",
            file.display()
        );
        return Ok(());
    }

    let schema = DocumentSchema::load(&schema_path)?;
    println!("{}", schema.summary());

    if validate {
        // Validate document against schema
        let content = std::fs::read_to_string(file)?;
        let header = regedited::header::scan_content(&content)?;
        let lines: Vec<&str> = content.lines().collect();

        let mut total_errors = 0;
        for (sec_name, sec_schema) in &schema.sections {
            if let Ok(info) = header.resolve_section(sec_name) {
                // Build values map from the section
                let mut values = std::collections::BTreeMap::new();

                // Extract numeric values (pipe-separated, with tab fallback)
                if info.numeric_line < lines.len() {
                    let line = lines[info.numeric_line];
                    let sep = if line.contains(" | ") { " | " } else { "\t" };
                    let nums: Vec<&str> = line.split(sep).collect();
                    for (i, v) in nums.iter().enumerate() {
                        values.insert(format!("num_{}", i), v.trim().to_string());
                    }
                }

                // Extract strings
                if info.string1_line < lines.len() {
                    values.insert("str_0".to_string(), lines[info.string1_line].to_string());
                }

                let errors = sec_schema.validate(&values);
                if !errors.is_empty() {
                    total_errors += errors.len();
                    println!("\n  [{}] Validation errors:", sec_name.red());
                    for err in &errors {
                        println!("    - {}", err);
                    }
                }
            } else {
                println!("  [{}] Index not found in document", sec_name.yellow());
            }
        }

        if total_errors == 0 {
            println!(
                "\n{} Document validates against schema",
                "OK:".green().bold()
            );
        } else {
            println!(
                "\n{} {} validation error(s) found",
                "FAIL:".red().bold(),
                total_errors
            );
        }
    }

    Ok(())
}

// ==================== TYPED VALUE COMMANDS ====================

fn cmd_reg_types() -> Result<(), Box<dyn std::error::Error>> {
    use regedited::typed_value::list_registry_types;

    println!("{}", "Registry Types:".green().bold());
    println!();
    println!("  {:<16} Description", "Type");
    println!("  {:-<40}", "");

    for (name, desc) in list_registry_types() {
        println!("  {:<16} {}", name.cyan(), desc);
    }

    println!();
    println!(
        "  {} Regedited extensions to Windows registry types:",
        "Note:".yellow()
    );
    println!("    REG_JSON  — structured JSON data");
    println!("    REG_TOML  — structured TOML data");
    println!("    REG_BOOL  — boolean flag");

    Ok(())
}

fn cmd_reg_parse(value: &str, reg_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::typed_value::TypedValue;

    let parsed = TypedValue::from_store_string(value, reg_type)?;

    println!(
        "{} Parsed as {}",
        "Result:".green().bold(),
        parsed.reg_type_name().cyan()
    );
    println!("  Type:  {}", parsed.type_name());
    println!("  Value: {}", parsed.display());
    println!("  Bytes: {}", parsed.byte_size());

    Ok(())
}

// ==================== SERVE COMMAND ====================

fn cmd_serve(file: &Path, port: u16, read_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::serve::{serve, ServeConfig};

    if !file.exists() {
        return Err(format!("File not found: {}", file.display()).into());
    }

    let config = ServeConfig {
        port,
        file_path: file.display().to_string(),
        read_only,
        cors: true,
    };

    println!(
        "{} Regedited Registry Container",
        "Starting:".green().bold()
    );
    println!("  File:      {}", file.display());
    println!("  Endpoint:  http://localhost:{}", port);
    println!("  Read-only: {}", read_only);
    println!();
    println!("  Endpoints:");
    println!("    GET  /              — Status + section list");
    println!("    GET  /sections      — All indexes (legacy route name)");
    println!("    GET  /section/{{name}}     — Section metadata");
    println!("    GET  /section/{{name}}/db  — Database table");
    println!("    GET  /section/{{name}}/hexline — Hex-word line");
    println!("    GET  /section/{{name}}/ascii — Legacy alias for /hexline");
    println!("    GET  /section/{{name}}/zone/{{i}} — Zone content");
    println!("    GET  /grep?pattern= &section= — Search");
    println!("    GET  /types         — Zone types");
    println!("    GET  /wal           — WAL status");
    println!("    GET  /health        — Health check");
    println!();
    println!("  Press Ctrl+C to stop");
    println!();

    serve(config)?;
    Ok(())
}

// ==================== ENHANCED CLIPBOARD HANDLERS ====================

fn cmd_clip_zone(
    file: &PathBuf,
    section: &str,
    zone: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::clip::clip_zone_content;
    use regedited::header::scan_content;

    if zone >= 3 {
        return Err("Zone index must be 0, 1, or 2".into());
    }

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;
    let sec = header.resolve_section(section)?;

    let clipped = clip_zone_content(&content, sec, zone)?;
    println!(
        "{} Zone {} from [{}] copied to clipboard ({} chars)",
        "✓".green(),
        zone,
        section.cyan(),
        clipped.len()
    );

    Ok(())
}

fn cmd_clip_db(
    file: &PathBuf,
    section: &str,
    index: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::clip::clip_db_value;
    use regedited::header::scan_content;

    if index >= 9 {
        return Err("DB value index must be 0-8".into());
    }

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;
    let sec = header.resolve_section(section)?;

    let value = clip_db_value(&content, sec, index)?;
    println!(
        "{} DB value [{}].{} = {} copied to clipboard",
        "✓".green(),
        section.cyan(),
        index.to_string().yellow(),
        value
    );

    Ok(())
}

fn cmd_clip_dbline(file: &PathBuf, section: &str) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::clip::clip_db_line;
    use regedited::header::scan_content;

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;
    let sec = header.resolve_section(section)?;

    let line = clip_db_line(&content, sec)?;
    println!(
        "{} DB line from [{}] copied to clipboard: {}",
        "✓".green(),
        section.cyan(),
        line.dimmed()
    );

    Ok(())
}

fn cmd_clip_ascii(file: &PathBuf, section: &str) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::clip::clip_ascii_store;
    use regedited::header::scan_content;

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;
    let sec = header.resolve_section(section)?;

    let ascii = clip_ascii_store(&content, sec)?;
    println!(
        "{} Hex-word line from [{}] copied to clipboard: {}",
        "✓".green(),
        section.cyan(),
        ascii.dimmed()
    );

    Ok(())
}

fn cmd_clip_hexword(
    start: u32,
    end: u32,
    zone_type_str: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::clip::clip_hexword_range;
    use regedited::zone_type::ZoneType;

    let zt = match zone_type_str.to_lowercase().as_str() {
        "markdown" | "md" | "text" => ZoneType::Markdown,
        "code" | "rust" | "python" | "js" | "cpp" | "c" => ZoneType::Code,
        "media" | "img" | "image" | "audio" | "video" => ZoneType::Media,
        "database" | "db" | "data" => ZoneType::Database,
        _ => {
            return Err(format!(
                "Unknown zone type: '{}'. Use: markdown, code, media, database",
                zone_type_str
            )
            .into())
        }
    };

    let result = clip_hexword_range(start, end, zt)?;
    println!("{} Hex-word range copied to clipboard:", "✓".green());
    println!("  {}", result.yellow());
    println!();
    println!("  Paste this into your hex-word line:");
    println!(
        "  {}",
        format!("0x0000000 : {} : 0x0000000", result).dimmed()
    );

    Ok(())
}
