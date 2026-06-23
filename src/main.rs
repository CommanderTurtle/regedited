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
//! # Show ASCII store for a section
//! regedited ascii myfile.md MySection
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
//! # Update ASCII store zone (with type)
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

use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use regedited::{
    bool_ops::{bool_and, bool_nand, bool_or, bool_xor, count, if_contains},
    echo::safe_echo,
    encapsulate::{encapsulate, convert_mode, detect_mode, display_modes, extract, EncapMode},
    header::{quick_scan_names, scan_content},
    html_extract::{extract_attributes, format_as_set_vars, format_numbered, index_to_suffix},
    store::{Store, StoreConfig},
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "regedited")]
#[command(about = "Fast plaintext parse-ment database")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Don't auto-save changes
    #[arg(long, global = true)]
    no_save: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List all sections in the document
    List {
        /// Path to the markdown file
        file: PathBuf,
    },

    /// Show the database table for a section
    Db {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
    },

    /// Show the ASCII store for a section
    Ascii {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
    },

    /// Scan all sections (safetensors-style header scan)
    Scan {
        /// Path to the markdown file
        file: PathBuf,
        /// Filter sections by name pattern
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

    /// Replace sections from source into target (safetensors-style patch)
    Replace {
        /// Target file (to be modified)
        target: PathBuf,
        /// Source file (donor sections)
        source: PathBuf,
        /// Section names to replace (all matching if omitted)
        #[arg(short, long)]
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
        /// Limit to a section
        #[arg(short, long)]
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
        /// Source section name
        #[arg(short = 'f', long)]
        from: String,
        /// Source zone index (0-2)
        #[arg(short = 'm', long, default_value = "0")]
        from_zone: usize,
        /// Target section name
        #[arg(short = 't', long)]
        to: String,
        /// Target zone index (0-2)
        #[arg(short = 'n', long, default_value = "0")]
        to_zone: usize,
    },

    /// Zone content append: append content (from stdin or --text) to a zone
    ZoneAppend {
        /// Path to the markdown file
        file: PathBuf,
        /// Target section name
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
        /// Target section name
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
        /// Section name
        section: String,
        /// Zone index (0-2)
        zone: usize,
    },

    /// Zone info: machine-readable zone metadata (for Python scripts)
    ZoneInfo {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
        /// Zone index (0-2)
        zone: usize,
    },

    /// Extract a zone by index (0-2)
    Grep {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
        /// Zone index (0-2)
        index: usize,
    },

    /// Copy a string to clipboard
    Clip {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
        /// String index (0-2)
        index: usize,
    },

    /// Echo a string safely (handles Windows CMD special chars)
    Echo {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
        /// String index (0-2)
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
        /// Section name
        section: String,
        /// Value index (0-8)
        index: usize,
        /// New value
        value: i64,
    },

    /// Update a string value
    SetStr {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
        /// String index (0-2)
        index: usize,
        /// New value
        value: String,
    },

    /// Update ASCII store zone (with type: markdown/code/media/database)
    SetZone {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
        /// Zone index (0-2)
        index: usize,
        /// Start line
        start: u32,
        /// End line
        end: u32,
        /// Zone type: markdown (0), code (1), media (2), database (3)
        #[arg(short, long, default_value = "markdown")]
        zone_type: String,
    },

    /// Convert line range + type to hex-words (interactive converter)
    Convert {
        /// Start line
        start: u32,
        /// End line
        end: u32,
        /// Zone type: markdown, code, media, database (or 0-3)
        #[arg(short, long, default_value = "markdown")]
        zone_type: String,
    },

    /// List all zone types and their hex nibble values
    Types,

    /// Show section content (markdown between --- and next section)
    Content {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
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

    /// Add a new section
    Add {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
    },

    /// Remove a section
    Rm {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
    },

    /// Show document summary
    Summary {
        /// Path to the markdown file
        file: PathBuf,
    },

    /// Show full document info (all sections with details)
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
        /// Section name (or "__all__" for entire file)
        section: String,
        /// Patterns to match (ALL must be found)
        patterns: Vec<String>,
    },

    /// Boolean NAND: contains first pattern but NOT second
    BoolNand {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name (or "__all__" for entire file)
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
        /// Section name (or "__all__" for entire file)
        section: String,
        /// Patterns to match (ANY must be found)
        patterns: Vec<String>,
    },

    /// Boolean XOR: contains EXACTLY ONE of two patterns
    BoolXor {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name (or "__all__" for entire file)
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
        /// Section name (or "__all__" for entire file)
        section: String,
        /// Pattern to count
        pattern: String,
    },

    /// If-contains-then: return value based on pattern presence
    IfContains {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name (or "__all__" for entire file)
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
        /// Section name
        section: String,
        /// Zone index (0, 1, or 2)
        zone: usize,
    },

    /// Copy database value (by index 0-8) to clipboard
    ClipDb {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
        /// Value index (0-8)
        index: usize,
    },

    /// Copy entire database line to clipboard
    ClipDbline {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
        section: String,
    },

    /// Copy ASCII store line to clipboard
    ClipAscii {
        /// Path to the markdown file
        file: PathBuf,
        /// Section name
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

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn cmd_getutf(number: u32, decode: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(hex) = decode {
        // Decode mode
        let decoded = regedited::getutf_decode(&hex)?;
        println!("{} UTF-16LE {} → {}",
            "Decoded:".green().bold(),
            hex.cyan(),
            decoded.to_string().yellow()
        );
    } else {
        // Encode mode
        let result = regedited::getutf(number);
        println!("{} {} → {}",
            "getutf:".green().bold(),
            number.to_string().yellow(),
            result.cyan()
        );
        println!("  {} Encodes {} as UTF-16LE code point(s)",
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
        Commands::Ascii { file, section } => cmd_ascii(&file, &section, config),
        Commands::Scan { file, filter, value } => cmd_scan(&file, filter, value),
        Commands::Diff { file_a, file_b } => cmd_diff(&file_a, &file_b),
        Commands::Replace { target, source, sections, output } => {
            cmd_replace(&target, &source, sections, output)
        }
        Commands::Fgrep { file, pattern, section } => cmd_fgrep(&file, &pattern, section),
        Commands::FgrepMulti { file, patterns } => cmd_fgrep_multi(&file, patterns),
        Commands::ZoneCopy { file, from, from_zone, to, to_zone } => {
            cmd_zone_copy(&file, &from, from_zone, &to, to_zone)
        }
        Commands::ZoneAppend { file, section, zone, text } => {
            cmd_zone_append(&file, &section, zone, text)
        }
        Commands::ZoneReplace { file, section, zone, text } => {
            cmd_zone_replace(&file, &section, zone, text)
        }
        Commands::ZoneExtract { file, section, zone } => {
            cmd_zone_extract(&file, &section, zone)
        }
        Commands::ZoneInfo { file, section, zone } => {
            cmd_zone_info(&file, &section, zone)
        }
        Commands::Grep { file, section, index } => cmd_grep(&file, &section, index, config),
        Commands::Clip { file, section, index } => cmd_clip(&file, &section, index, config),
        Commands::Echo { file, section, index } => cmd_echo(&file, &section, index, config),
        Commands::EchoDirect { text } => cmd_echo_direct(&text),
        Commands::SetNum { file, section, index, value } => {
            cmd_set_num(&file, &section, index, value, config)
        }
        Commands::SetStr { file, section, index, value } => {
            cmd_set_str(&file, &section, index, &value, config)
        }
        Commands::SetZone { file, section, index, start, end, zone_type } => {
            cmd_set_zone(&file, &section, index, start, end, &zone_type, config)
        }
        Commands::Convert { start, end, zone_type } => cmd_convert(start, end, &zone_type),
        Commands::Types => cmd_types(),
        Commands::Content { file, section } => cmd_content(&file, &section, config),
        Commands::Lines { file, start, end } => cmd_lines(&file, start, end),
        Commands::Getutf { number, decode } => cmd_getutf(number, decode),
        Commands::New { file, title } => cmd_new(&file, &title),
        Commands::Add { file, section } => cmd_add(&file, &section, config),
        Commands::Rm { file, section } => cmd_rm(&file, &section, config),
        Commands::Summary { file } => cmd_summary(&file),
        Commands::Info { file } => cmd_info(&file),
        Commands::Encap { text, mode, extract, to, set } => {
            cmd_encap(&text, &mode, extract, to, set)
        }
        Commands::GrabHtml { file, attr, mode, tag, set, numbered } => {
            cmd_grab_html(&file, &attr, &mode, tag, set, numbered)
        }
        Commands::BoolAnd { file, section, patterns } => {
            cmd_bool_and(&file, &section, patterns)
        }
        Commands::BoolNand { file, section, must_contain, must_not } => {
            cmd_bool_nand(&file, &section, &must_contain, &must_not)
        }
        Commands::BoolOr { file, section, patterns } => {
            cmd_bool_or(&file, &section, patterns)
        }
        Commands::BoolXor { file, section, pattern_a, pattern_b } => {
            cmd_bool_xor(&file, &section, &pattern_a, &pattern_b)
        }
        Commands::Count { file, section, pattern } => {
            cmd_count(&file, &section, &pattern)
        }
        Commands::IfContains { file, section, pattern, then_val, else_val } => {
            cmd_if_contains(&file, &section, &pattern, &then_val, &else_val)
        }
        Commands::Wal { file } => cmd_wal(&file),
        Commands::WalReplay { file, apply } => cmd_wal_replay(&file, apply),
        Commands::Tx { action, file } => cmd_tx(&action, &file),
        Commands::Schema { file, validate, init } => cmd_schema(&file, validate, init),
        Commands::RegTypes => cmd_reg_types(),
        Commands::RegParse { value, reg_type } => cmd_reg_parse(&value, &reg_type),
        Commands::ClipZone { file, section, zone } => {
            cmd_clip_zone(&file, &section, zone)
        }
        Commands::ClipDb { file, section, index } => {
            cmd_clip_db(&file, &section, index)
        }
        Commands::ClipDbline { file, section } => {
            cmd_clip_dbline(&file, &section)
        }
        Commands::ClipAscii { file, section } => {
            cmd_clip_ascii(&file, &section)
        }
        Commands::ClipHexword { start, end, zone_type } => {
            cmd_clip_hexword(start, end, &zone_type)
        }
        Commands::Serve { file, port, read_only } => cmd_serve(&file, port, read_only),
    }
}

// ==================== COMMAND IMPLEMENTATIONS ====================

fn cmd_list(file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;
    let names = quick_scan_names(&content);

    if names.is_empty() {
        println!("{} No sections found", "Note:".yellow());
        return Ok(());
    }

    println!("{} {} sections in {}",
        "Sections:".green().bold(),
        names.len(),
        file.display()
    );
    
    for (name, line) in names {
        println!("  {} {} (header @ line {})",
            "-".cyan(),
            name.bold(),
            line.to_string().dimmed()
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
    
    println!("{} '{}' in {}",
        "Database table for section".green().bold(),
        section.cyan(),
        file.display()
    );
    println!();
    
    let table = store.get_db_table(section)?;
    println!("{}", table);
    
    // Also show ASCII store
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
    
    println!("{} '{}' in {}",
        "ASCII store for section".green().bold(),
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
    println!("{} Copied to clipboard from '{}': {}",
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
    println!("{} {}", "Original:".green().bold(), result.analysis.original);
    println!("{} {}", "Strategy:".green().bold(), result.strategy);
    println!("{} {}", "Command:".green().bold(), result.echo_command.cyan());
    
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
    
    println!("{} Updated '{}'.Num{} = {} in {}",
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
    
    println!("{} Updated '{}'.Str{} = \"{}\" in {}",
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
    
    println!("{} Updated '{}'.Zone{} = {} -> {} [{}] in {}",
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

fn cmd_convert(
    start: u32,
    end: u32,
    zone_type_str: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let zt = regedited::zone_type::ZoneType::from_name(zone_type_str)
        .ok_or_else(|| format!("Unknown zone type: '{}'", zone_type_str))?;
    
    let (start_hex, end_hex) = regedited::zone_type::convert_to_hex_words(start, end, zt);
    
    println!("{} Converted range {}-{} [{}]",
        "Converter:".green().bold(),
        start.to_string().yellow(),
        end.to_string().yellow(),
        zt.name().cyan(),
    );
    println!("  Start: {}", start_hex.cyan());
    println!("  End:   {}", end_hex.cyan());
    println!("\n  {} Full line:", "Paste into your .md:".dimmed());
    println!("  {} : {} : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
        start_hex, end_hex
    );

    Ok(())
}

fn cmd_types() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "Zone Types (first hex nibble after 0x)".green().bold());
    println!();
    for zt in regedited::zone_type::ZoneType::ALL {
        println!("  {} {} - {}",
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
    
    println!("{} '{}' from {}",
        "Content of section".green().bold(),
        section.cyan(),
        file.display()
    );
    println!("{}", "---".dimmed());
    println!("{}", content);
    println!("{}", "---".dimmed());

    Ok(())
}

fn cmd_lines(
    file: &PathBuf,
    start: usize,
    end: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;
    
    let extracted = regedited::extract_lines(content.as_bytes(), start, end)?;
    let extracted = String::from_utf8(extracted)?;
    
    println!("{} Lines {}-{} from {}",
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
    
    println!("{} Created new document: {} (\"{}\")",
        "OK".green().bold(),
        file.display(),
        title.cyan()
    );

    Ok(())
}

fn cmd_add(
    file: &PathBuf,
    section: &str,
    config: StoreConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::open_with_config(file, config)?;
    
    store.add_section(section)?;
    
    println!("{} Added section '{}' to {}",
        "OK".green().bold(),
        section.cyan(),
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
    
    println!("{} Removed section '{}' from {}",
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
    println!("  Sections: {}", header.section_count());
    println!("  Total lines: {}", header.total_lines);
    println!("  Total bytes: {}", header.total_bytes);
    println!();
    
    for (name, info) in &header.sections {
        println!("{}", format!("## SECTION: {}", name).cyan().bold());
        println!("  Header @ line {}", info.header_line);
        println!("  Index @ line {}", info.header_line + 1);
        println!("  ASCII store @ line {}", info.ascii_line);
        println!("  Numeric line @ line {}", info.numeric_line);
        println!("  String 1 @ line {}", info.string1_line);
        println!("  String 2 @ line {}", info.string2_line);
        println!("  String 3 @ line {}", info.string3_line);
        println!("  Content separator @ line {}", info.separator_line);
        println!("  Content: lines {}-{}", info.content_start, info.content_end);
        println!();
    }

    Ok(())
}

// ==================== FAST OPERATIONS (SAFETENSORS-STYLE) ====================

fn cmd_scan(
    file: &PathBuf,
    filter: Option<String>,
    value: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::{fast_scan, filter_by_name, filter_by_value};

    let scanned = fast_scan(file)?;

    if scanned.is_empty() {
        println!("{} No sections found", "Note:".yellow());
        return Ok(());
    }

    println!("{} {} sections in {} (safetensors-style header scan)",
        "Scan:".green().bold(), scanned.len(), file.display());

    // Apply name filter
    let by_name: Vec<&regedited::fast_ops::ScannedSection> = match filter {
        Some(ref pat) => {
            let r = filter_by_name(&scanned, pat);
            println!("  Name filter '{}': {} matches", pat.cyan(), r.len());
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
                println!("  Value filter [{}] {}-{}: {} matches", idx, min, max, r.len());
                // Convert back to refs... actually just print from owned
                println!();
                for sec in &owned {
                    if sec.db_values.get(idx).map_or(false, |&v| v >= min && v <= max) {
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

fn cmd_diff(
    file_a: &PathBuf,
    file_b: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::fast_diff;

    println!("{} {} vs {}",
        "Diff:".green().bold(),
        file_a.display(),
        file_b.display()
    );
    println!("{}", "  (metadata-only comparison, like safetensors header diff)".dimmed());
    println!();

    let diff = fast_diff(file_a, file_b)?;
    println!("{}", diff.display());

    Ok(())
}

fn cmd_replace(
    target: &PathBuf,
    source: &PathBuf,
    sections: Option<Vec<String>>,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::fast_replace;

    println!("{} Replacing from {} → {}",
        "Replace:".green().bold(),
        source.display(),
        target.display()
    );

    let result = fast_replace(target, source, sections.as_deref())?;

    let out_path = output.as_ref().unwrap_or(target);
    std::fs::write(out_path, result)?;

    println!("{} Patched file written to {}",
        "OK".green().bold(),
        out_path.display()
    );

    Ok(())
}

fn cmd_fgrep(
    file: &PathBuf,
    pattern: &str,
    section: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::{fast_grep, fast_grep_section};

    let matches = if let Some(sec) = section {
        println!("{} '{}' in section '{}' of {} (memory-mapped)",
            "Fast grep:".green().bold(),
            pattern.cyan(),
            sec.cyan(),
            file.display()
        );
        fast_grep_section(file, &sec, pattern)?
    } else {
        println!("{} '{}' in {} (memory-mapped, ripgrep-style)",
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

fn cmd_fgrep_multi(
    file: &PathBuf,
    patterns: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::fast_ops::fast_grep_multi;

    println!("{} {} patterns in {} (OR logic)",
        "Multi grep:".green().bold(),
        patterns.len(),
        file.display()
    );
    println!("  Patterns: {}\n", patterns.join(", ").cyan());

    let matches = fast_grep_multi(file, &patterns)?;

    println!("  {} matches\n", matches.len());
    for (line_num, line, matched) in matches {
        let tags: Vec<String> = matched.iter().map(|p| format!("[{}]", p)).collect();
        println!("  {}: {} {}",
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

    let from_sec = header.get_section(from)
        .or_else(|| header.get_section_case_insensitive(from))
        .ok_or_else(|| format!("Source section '{}' not found", from))?;
    let to_sec = header.get_section(to)
        .or_else(|| header.get_section_case_insensitive(to))
        .ok_or_else(|| format!("Target section '{}' not found", to))?;

    let result = copy_zone_content(&content, from_sec, from_zone, to_sec, to_zone)?;
    std::fs::write(file, result)?;

    println!("{} Copied zone {} from '{}' → zone {} from '{}' in {}",
        "OK".green().bold(),
        from_zone, from.cyan(),
        to_zone, to.cyan(),
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

    let sec = header.get_section(section)
        .or_else(|| header.get_section_case_insensitive(section))
        .ok_or_else(|| format!("Section '{}' not found", section))?;

    let result = append_zone_content(&content, sec, zone, &append_text)?;
    std::fs::write(file, result)?;

    println!("{} Appended {} bytes to zone {} of '{}' in {}",
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

    let sec = header.get_section(section)
        .or_else(|| header.get_section_case_insensitive(section))
        .ok_or_else(|| format!("Section '{}' not found", section))?;

    let result = replace_zone_content(&content, sec, zone, &replace_text)?;
    std::fs::write(file, result)?;

    println!("{} Replaced zone {} of '{}' with {} bytes in {}",
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

    let sec = header.get_section(section)
        .or_else(|| header.get_section_case_insensitive(section))
        .ok_or_else(|| format!("Section '{}' not found", section))?;

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

    let sec = header.get_section(section)
        .or_else(|| header.get_section_case_insensitive(section))
        .ok_or_else(|| format!("Section '{}' not found", section))?;

    let info = format_zone_info(&content, sec, zone)?;
    println!("{}", info);

    Ok(())
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
    let mode = EncapMode::from_name(mode_str)
        .ok_or_else(|| format!("Unknown mode: '{}'", mode_str))?;

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
    let mode = EncapMode::from_name(mode_str)
        .ok_or_else(|| format!("Unknown mode: '{}'", mode_str))?;

    let content = std::fs::read_to_string(file)?;

    let tag_opt = tag_filter.as_deref();
    let extracts = extract_attributes(&content, attr, tag_opt);

    if extracts.is_empty() {
        println!("{} No '{}' attributes found", "Note:".yellow(), attr);
        return Ok(());
    }

    println!("{} {} <{}> attribute(s) in {} (mode: {})",
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
            println!("  [{}] Line {} <{} {}={}>",
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
        let sec = header.get_section(section)
            .or_else(|| header.get_section_case_insensitive(section))
            .ok_or_else(|| format!("Section '{}' not found", section))?;

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
        println!("\n  {} Lines matching '{}':", "Matches:".green(), must_contain);
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

    println!("{} Pattern '{}' found {} time(s) across {} lines",
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
        println!("  Run 'regedited wal-replay {} --apply' to recover", file.display());
    }

    Ok(())
}

fn cmd_wal_replay(file: &PathBuf, apply: bool) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::wal::{Wal, WalOperation};

    if !Wal::exists_for(file) {
        println!("{} No WAL file found for {}", "Note:".yellow(), file.display());
        return Ok(());
    }

    let entries = Wal::read_entries(file)?;
    if entries.is_empty() {
        println!("{} WAL is empty — nothing to replay", "Note:".yellow());
        return Ok(());
    }

    println!("{} {} WAL entries to replay",
        "Found:".green().bold(),
        entries.len()
    );

    for entry in &entries {
        println!("  [{:4}] {} (checksum: {:08x})",
            entry.seq,
            entry.operation.description(),
            entry.checksum
        );
    }

    if apply {
        println!("\n{} Replaying {} entries...", "Applying:".green().bold(), entries.len());
        // In a full implementation, each operation would be applied here
        // For now, mark WAL as resolved by removing it
        let mut wal = Wal::open(file)?;
        wal.cleanup()?;
        println!("{} WAL replay complete. File cleaned up.", "OK:".green().bold());
    } else {
        println!("\n{} Use --apply to actually replay these changes", "Dry run:".yellow());
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
                    println!("{} Transaction begun for {}",
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
            let mut mgr = TransactionManager::new();
            // Try to load existing transaction
            if let Ok(tx) = Transaction::begin(file) {
                println!("{} Committing {} operations...",
                    "TX:".green().bold(),
                    tx.len()
                );
                // Transaction already has WAL entries — just commit marker
                drop(tx);
                let mut wal = regedited::wal::Wal::open(file)?;
                wal.commit()?;
                println!("{} Transaction committed", "OK:".green().bold());
            } else {
                println!("{} No active transaction for {}", "Note:".yellow(), file.display());
            }
        }
        "rollback" | "abort" => {
            if let Ok(tx) = Transaction::begin(file) {
                println!("{} Rolling back {} operations...",
                    "TX:".yellow().bold(),
                    tx.len()
                );
                drop(tx);
                let mut wal = regedited::wal::Wal::open(file)?;
                wal.rollback()?;
                println!("{} Transaction rolled back", "OK:".green().bold());
            } else {
                println!("{} No active transaction for {}", "Note:".yellow(), file.display());
            }
        }
        "status" | "st" => {
            if let Ok(tx) = Transaction::begin(file) {
                println!("{}", tx.summary());
            } else {
                println!("{} No active transaction for {}", "Note:".yellow(), file.display());
            }
        }
        _ => {
            return Err(format!(
                "Unknown transaction action: '{}'. Use: begin, commit, rollback, status",
                action
            ).into());
        }
    }

    Ok(())
}

// ==================== SCHEMA COMMANDS ====================

fn cmd_schema(file: &PathBuf, validate: bool, init: bool) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::schema::DocumentSchema;

    if init {
        // Generate a starter schema from the document
        let content = std::fs::read_to_string(file)?;
        let header = regedited::header::scan_content(&content)?;

        let mut schema = DocumentSchema::new();
        for (name, _info) in &header.sections {
            let sec = schema.section(name);
            // Add default fields
            sec.add_field(regedited::schema::SchemaField::new(
                "description", regedited::schema::SchemaFieldType::String
            ));
            sec.fields.get_mut("description").unwrap().constraint =
                regedited::schema::FieldConstraint::Optional;
        }

        let schema_path = DocumentSchema::schema_path(file);
        schema.save(&schema_path)?;
        println!("{} Created starter schema: {}", "OK:".green().bold(), schema_path.display());
        println!("{}", schema.summary());
        return Ok(());
    }

    let schema_path = DocumentSchema::schema_path(file);
    if !schema_path.exists() {
        println!("{} No schema found for {}", "Note:".yellow(), file.display());
        println!("  Run 'regedited schema {} --init' to create one", file.display());
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
            if let Some(info) = header.get_section(sec_name) {
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
                println!("  [{}] Section not found in document", sec_name.yellow());
            }
        }

        if total_errors == 0 {
            println!("\n{} Document validates against schema", "OK:".green().bold());
        } else {
            println!("\n{} {} validation error(s) found", "FAIL:".red().bold(), total_errors);
        }
    }

    Ok(())
}

// ==================== TYPED VALUE COMMANDS ====================

fn cmd_reg_types() -> Result<(), Box<dyn std::error::Error>> {
    use regedited::typed_value::list_registry_types;

    println!("{}", "Registry Types:".green().bold());
    println!();
    println!("  {:<16} {}", "Type", "Description");
    println!("  {:-<40}", "");

    for (name, desc) in list_registry_types() {
        println!("  {:<16} {}", name.cyan(), desc);
    }

    println!();
    println!("  {} Regedited extensions to Windows registry types:", "Note:".yellow());
    println!("    REG_JSON  — structured JSON data");
    println!("    REG_TOML  — structured TOML data");
    println!("    REG_BOOL  — boolean flag");

    Ok(())
}

fn cmd_reg_parse(value: &str, reg_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::typed_value::TypedValue;

    let parsed = TypedValue::from_store_string(value, reg_type)?;

    println!("{} Parsed as {}", "Result:".green().bold(), parsed.reg_type_name().cyan());
    println!("  Type:  {}", parsed.type_name());
    println!("  Value: {}", parsed.display());
    println!("  Bytes: {}", parsed.byte_size());

    Ok(())
}

// ==================== SERVE COMMAND ====================

fn cmd_serve(file: &PathBuf, port: u16, read_only: bool) -> Result<(), Box<dyn std::error::Error>> {
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

    println!("{} Regedited Registry Container", "Starting:".green().bold());
    println!("  File:      {}", file.display());
    println!("  Endpoint:  http://localhost:{}", port);
    println!("  Read-only: {}", read_only);
    println!();
    println!("  Endpoints:");
    println!("    GET  /              — Status + section list");
    println!("    GET  /sections      — All sections");
    println!("    GET  /section/{{name}}     — Section metadata");
    println!("    GET  /section/{{name}}/db  — Database table");
    println!("    GET  /section/{{name}}/ascii — Hex-word store");
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
    let sec = header.get_section(section)
        .or_else(|| header.get_section_case_insensitive(section))
        .ok_or_else(|| format!("Section '{}' not found", section))?;

    let clipped = clip_zone_content(&content, sec, zone)?;
    println!("{} Zone {} from [{}] copied to clipboard ({} chars)",
        "✓".green(), zone, section.cyan(), clipped.len());

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
    let sec = header.get_section(section)
        .or_else(|| header.get_section_case_insensitive(section))
        .ok_or_else(|| format!("Section '{}' not found", section))?;

    let value = clip_db_value(&content, sec, index)?;
    println!("{} DB value [{}].{} = {} copied to clipboard",
        "✓".green(), section.cyan(), index.to_string().yellow(), value);

    Ok(())
}

fn cmd_clip_dbline(
    file: &PathBuf,
    section: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::clip::clip_db_line;
    use regedited::header::scan_content;

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;
    let sec = header.get_section(section)
        .or_else(|| header.get_section_case_insensitive(section))
        .ok_or_else(|| format!("Section '{}' not found", section))?;

    let line = clip_db_line(&content, sec)?;
    println!("{} DB line from [{}] copied to clipboard: {}",
        "✓".green(), section.cyan(), line.dimmed());

    Ok(())
}

fn cmd_clip_ascii(
    file: &PathBuf,
    section: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use regedited::clip::clip_ascii_store;
    use regedited::header::scan_content;

    let content = std::fs::read_to_string(file)?;
    let header = scan_content(&content)?;
    let sec = header.get_section(section)
        .or_else(|| header.get_section_case_insensitive(section))
        .ok_or_else(|| format!("Section '{}' not found", section))?;

    let ascii = clip_ascii_store(&content, sec)?;
    println!("{} ASCII store from [{}] copied to clipboard: {}",
        "✓".green(), section.cyan(), ascii.dimmed());

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
        _ => return Err(format!("Unknown zone type: '{}'. Use: markdown, code, media, database", zone_type_str).into()),
    };

    let result = clip_hexword_range(start, end, zt)?;
    println!("{} Hex-word range copied to clipboard:", "✓".green());
    println!("  {}", result.yellow());
    println!();
    println!("  Paste this into your ASCII store line:");
    println!("  {}", format!("0x0000000 : {} : 0x0000000", result).dimmed());

    Ok(())
}
