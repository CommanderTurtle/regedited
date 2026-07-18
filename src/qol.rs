// SPDX-License-Identifier: AGPL-3.0

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandAlias {
    pub canonical: &'static str,
    pub short: &'static str,
}

pub const COMMAND_ALIASES: &[CommandAlias] = &[
    CommandAlias {
        canonical: "list",
        short: "l",
    },
    CommandAlias {
        canonical: "db",
        short: "db",
    },
    CommandAlias {
        canonical: "hexline",
        short: "hl",
    },
    CommandAlias {
        canonical: "scan",
        short: "s",
    },
    CommandAlias {
        canonical: "diff",
        short: "d",
    },
    CommandAlias {
        canonical: "replace",
        short: "r",
    },
    CommandAlias {
        canonical: "fgrep",
        short: "f",
    },
    CommandAlias {
        canonical: "fgrep-multi",
        short: "fm",
    },
    CommandAlias {
        canonical: "zone-copy",
        short: "zc",
    },
    CommandAlias {
        canonical: "zone-append",
        short: "za",
    },
    CommandAlias {
        canonical: "zone-replace",
        short: "zr",
    },
    CommandAlias {
        canonical: "zone-extract",
        short: "ze",
    },
    CommandAlias {
        canonical: "zone-info",
        short: "zi",
    },
    CommandAlias {
        canonical: "resolve-index",
        short: "ri",
    },
    CommandAlias {
        canonical: "index-zone-extract",
        short: "ize",
    },
    CommandAlias {
        canonical: "index-zone-replace",
        short: "izr",
    },
    CommandAlias {
        canonical: "index-zone-copy",
        short: "izc",
    },
    CommandAlias {
        canonical: "index-zone-transfer",
        short: "izt",
    },
    CommandAlias {
        canonical: "hex-extract",
        short: "he",
    },
    CommandAlias {
        canonical: "hex-replace",
        short: "hr",
    },
    CommandAlias {
        canonical: "ref-get",
        short: "rg",
    },
    CommandAlias {
        canonical: "ref-set",
        short: "rs",
    },
    CommandAlias {
        canonical: "ref-copy",
        short: "rc",
    },
    CommandAlias {
        canonical: "ref-diff",
        short: "rd",
    },
    CommandAlias {
        canonical: "ref-bool",
        short: "rb",
    },
    CommandAlias {
        canonical: "index-str-list",
        short: "ist",
    },
    CommandAlias {
        canonical: "index-zone-set-hex",
        short: "izs",
    },
    CommandAlias {
        canonical: "state",
        short: "st",
    },
    CommandAlias {
        canonical: "state-compare",
        short: "stc",
    },
    CommandAlias {
        canonical: "undo",
        short: "u",
    },
    CommandAlias {
        canonical: "grep",
        short: "g",
    },
    CommandAlias {
        canonical: "clip",
        short: "c",
    },
    CommandAlias {
        canonical: "echo",
        short: "e",
    },
    CommandAlias {
        canonical: "echo-direct",
        short: "ed",
    },
    CommandAlias {
        canonical: "getutf",
        short: "gu",
    },
    CommandAlias {
        canonical: "set-num",
        short: "sn",
    },
    CommandAlias {
        canonical: "set-str",
        short: "ss",
    },
    CommandAlias {
        canonical: "set-zone",
        short: "sz",
    },
    CommandAlias {
        canonical: "convert",
        short: "cv",
    },
    CommandAlias {
        canonical: "types",
        short: "t",
    },
    CommandAlias {
        canonical: "content",
        short: "co",
    },
    CommandAlias {
        canonical: "lines",
        short: "ln",
    },
    CommandAlias {
        canonical: "new",
        short: "n",
    },
    CommandAlias {
        canonical: "add",
        short: "a",
    },
    CommandAlias {
        canonical: "rm",
        short: "rm",
    },
    CommandAlias {
        canonical: "summary",
        short: "sm",
    },
    CommandAlias {
        canonical: "info",
        short: "i",
    },
    CommandAlias {
        canonical: "encap",
        short: "en",
    },
    CommandAlias {
        canonical: "grab-html",
        short: "gh",
    },
    CommandAlias {
        canonical: "bool-and",
        short: "ba",
    },
    CommandAlias {
        canonical: "bool-nand",
        short: "bn",
    },
    CommandAlias {
        canonical: "bool-or",
        short: "bo",
    },
    CommandAlias {
        canonical: "bool-xor",
        short: "bx",
    },
    CommandAlias {
        canonical: "count",
        short: "ct",
    },
    CommandAlias {
        canonical: "if-contains",
        short: "if",
    },
    CommandAlias {
        canonical: "wal",
        short: "w",
    },
    CommandAlias {
        canonical: "wal-replay",
        short: "wr",
    },
    CommandAlias {
        canonical: "tx",
        short: "tx",
    },
    CommandAlias {
        canonical: "schema",
        short: "sc",
    },
    CommandAlias {
        canonical: "reg-types",
        short: "rt",
    },
    CommandAlias {
        canonical: "reg-parse",
        short: "rp",
    },
    CommandAlias {
        canonical: "clip-zone",
        short: "cz",
    },
    CommandAlias {
        canonical: "clip-db",
        short: "cdb",
    },
    CommandAlias {
        canonical: "clip-dbline",
        short: "cdbl",
    },
    CommandAlias {
        canonical: "clip-hexline",
        short: "chl",
    },
    CommandAlias {
        canonical: "clip-hexword",
        short: "chw",
    },
    CommandAlias {
        canonical: "serve",
        short: "sv",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePlacement {
    None,
    First,
    AfterAction,
    FileFlag,
    FromFileFlag,
}

pub fn canonical_command(name: &str) -> Option<&'static str> {
    COMMAND_ALIASES
        .iter()
        .find(|entry| entry.short == name || entry.canonical == name)
        .map(|entry| entry.canonical)
}

pub fn short_command(name: &str) -> Option<&'static str> {
    COMMAND_ALIASES
        .iter()
        .find(|entry| entry.canonical == name)
        .map(|entry| entry.short)
}

pub fn validate_aliases() -> Result<(), String> {
    for (index, alias) in COMMAND_ALIASES.iter().enumerate() {
        if alias.canonical.is_empty() || alias.short.is_empty() {
            return Err("command aliases cannot be empty".to_string());
        }
        if COMMAND_ALIASES[index + 1..]
            .iter()
            .any(|other| other.short == alias.short)
        {
            return Err(format!("duplicate rgd shorthand '{}'", alias.short));
        }
        if COMMAND_ALIASES[index + 1..]
            .iter()
            .any(|other| other.canonical == alias.canonical)
        {
            return Err(format!("duplicate canonical command '{}'", alias.canonical));
        }
    }
    Ok(())
}

pub fn is_rgd_invocation(argv0: &OsStr) -> bool {
    let stem = Path::new(argv0)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default();
    stem.eq_ignore_ascii_case("rgd")
        || std::env::var_os("REGEDITED_RGD").is_some_and(|value| value != OsStr::new("0"))
}

pub fn normalize_short_command(args: &mut [OsString]) -> Option<&'static str> {
    let command = args.get(1)?.to_string_lossy();
    let canonical = canonical_command(&command)?;
    args[1] = OsString::from(canonical);
    Some(canonical)
}

pub fn normalize_global_arguments(args: &mut Vec<OsString>) {
    let command_index = args.iter().enumerate().skip(1).find_map(|(index, value)| {
        value
            .to_str()
            .filter(|value| canonical_command(value).is_some())
            .map(|_| index)
    });
    let scan_command = command_index
        .and_then(|index| args.get(index))
        .and_then(|value| value.to_str())
        .and_then(canonical_command)
        == Some("scan");

    let mut globals = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let value = args[index].to_string_lossy();
        let scan_value = value == "-v"
            && scan_command
            && command_index.is_some_and(|command| index > command)
            && args
                .get(index + 1)
                .is_some_and(|next| !next.to_string_lossy().starts_with('-'));
        if scan_value {
            index += 1;
            continue;
        }

        match value.as_ref() {
            "-v" | "--verbose" => {
                args.remove(index);
                if !globals.iter().any(|global| global == "--verbose") {
                    globals.push(OsString::from("--verbose"));
                }
            }
            "--no-save" => {
                args.remove(index);
                if !globals.iter().any(|global| global == "--no-save") {
                    globals.push(OsString::from("--no-save"));
                }
            }
            _ => index += 1,
        }
    }
    args.extend(globals);
}

pub fn file_placement(command: &str) -> FilePlacement {
    match command {
        "list" | "db" | "hexline" | "scan" | "diff" | "replace" | "fgrep" | "fgrep-multi"
        | "zone-copy" | "zone-append" | "zone-replace" | "zone-extract" | "zone-info"
        | "resolve-index" | "index-zone-extract" | "index-zone-replace" | "index-zone-copy"
        | "hex-extract" | "hex-replace" | "ref-get" | "ref-set" | "ref-copy" | "ref-diff"
        | "ref-bool" | "index-str-list" | "index-zone-set-hex" | "state" | "state-compare"
        | "undo" | "grep" | "clip" | "echo" | "set-num" | "set-str" | "set-zone" | "content"
        | "lines" | "add" | "rm" | "summary" | "info" | "grab-html" | "bool-and" | "bool-nand"
        | "bool-or" | "bool-xor" | "count" | "if-contains" | "wal" | "wal-replay" | "schema"
        | "clip-zone" | "clip-db" | "clip-dbline" | "clip-hexline" => FilePlacement::First,
        "tx" => FilePlacement::AfterAction,
        "serve" => FilePlacement::FileFlag,
        "index-zone-transfer" => FilePlacement::FromFileFlag,
        _ => FilePlacement::None,
    }
}

fn positional_token(args: &[OsString], index: usize) -> Option<&OsStr> {
    args.iter()
        .skip(2)
        .filter(|value| !value.to_string_lossy().starts_with('-'))
        .nth(index)
        .map(OsString::as_os_str)
}

fn looks_like_path(value: &OsStr) -> bool {
    let path = Path::new(value);
    if path.exists() || path.is_absolute() || path.components().count() > 1 {
        return true;
    }
    path.extension().is_some()
}

pub fn has_explicit_file(args: &[OsString], command: &str) -> bool {
    match file_placement(command) {
        FilePlacement::None => true,
        FilePlacement::First => positional_token(args, 0).is_some_and(looks_like_path),
        FilePlacement::AfterAction => positional_token(args, 1).is_some_and(looks_like_path),
        FilePlacement::FileFlag => args.iter().skip(2).any(|value| {
            let value = value.to_string_lossy();
            value == "--file" || value == "-f" || value.starts_with("--file=")
        }),
        FilePlacement::FromFileFlag => args.iter().skip(2).any(|value| {
            let value = value.to_string_lossy();
            value == "--from-file" || value.starts_with("--from-file=")
        }),
    }
}

pub fn inject_loaded_file(
    args: &[OsString],
    command: &str,
    loaded: &Path,
    force: bool,
) -> Option<Vec<OsString>> {
    if !force && has_explicit_file(args, command) {
        return None;
    }

    let mut normalized = args.to_vec();
    match file_placement(command) {
        FilePlacement::None => return None,
        FilePlacement::First => normalized.insert(2, loaded.as_os_str().to_os_string()),
        FilePlacement::AfterAction => normalized.insert(3, loaded.as_os_str().to_os_string()),
        FilePlacement::FileFlag => {
            normalized.insert(2, OsString::from("--file"));
            normalized.insert(3, loaded.as_os_str().to_os_string());
        }
        FilePlacement::FromFileFlag => {
            normalized.insert(2, OsString::from("--from-file"));
            normalized.insert(3, loaded.as_os_str().to_os_string());
        }
    }
    Some(normalized)
}

pub fn compact_ref(value: &str) -> Option<String> {
    let rest = value.strip_prefix('i')?;
    let digit_count = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digit_count == 0 {
        return None;
    }

    let (index, suffix) = rest.split_at(digit_count);
    let prefix = format!("index:{}", index);
    if suffix.is_empty() {
        return Some(prefix);
    }

    let with_slot = |marker: &str, canonical: &str, max: usize| {
        suffix.strip_prefix(marker).and_then(|slot| {
            slot.parse::<usize>()
                .ok()
                .filter(|slot| (1..=max).contains(slot))
                .map(|slot| format!("{}:{}:{}", prefix, canonical, slot))
        })
    };

    with_slot("s", "string", 3)
        .or_else(|| with_slot("db", "db", 9))
        .or_else(|| with_slot("zh", "zonehex", 3))
        .or_else(|| with_slot("rh", "rangehex", 3))
        .or_else(|| with_slot("z", "zone", 3))
        .or_else(|| with_slot("r", "range", 3))
        .or_else(|| match suffix {
            "dbl" => Some(format!("{}:dbline", prefix)),
            "hl" => Some(format!("{}:hexline", prefix)),
            "hwl" => Some(format!("{}:hex-word-line", prefix)),
            "rs" => Some(format!("{}:ranges", prefix)),
            _ => None,
        })
}

pub fn normalize_compact_refs(args: &mut [OsString]) {
    for value in args.iter_mut().skip(2) {
        if let Some(raw) = value.to_str() {
            if let Some(expanded) = compact_ref(raw) {
                *value = OsString::from(expanded);
            }
        }
    }
}

pub fn normalize_short_clip_flag(args: &mut [OsString]) {
    if args.get(1).and_then(|value| value.to_str()) != Some("ref-get") {
        return;
    }
    if let Some(value) = args.last_mut() {
        if value == "c" {
            *value = OsString::from("--clip");
        }
    }
}

pub fn loaded_path_state_file() -> io::Result<PathBuf> {
    if let Some(root) = std::env::var_os("REGEDITED_STATE_HOME") {
        return Ok(PathBuf::from(root).join("loaded-path.txt"));
    }

    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))?;

    #[cfg(not(windows))]
    let root = if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(state_home)
    } else {
        PathBuf::from(
            std::env::var_os("HOME")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?,
        )
        .join(".local")
        .join("state")
    };

    Ok(root.join("regedited").join("loaded-path.txt"))
}

pub fn save_loaded_path(path: &Path) -> io::Result<PathBuf> {
    let normalized = path.canonicalize()?;
    if !normalized.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("loaded path is not a file: {}", normalized.display()),
        ));
    }
    let state_file = loaded_path_state_file()?;
    if let Some(parent) = state_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&state_file, normalized.to_string_lossy().as_bytes())?;
    Ok(normalized)
}

pub fn read_loaded_path() -> io::Result<Option<PathBuf>> {
    let state_file = loaded_path_state_file()?;
    let value = match fs::read_to_string(state_file) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let path = PathBuf::from(value.trim());
    if path.as_os_str().is_empty() {
        return Ok(None);
    }
    Ok(Some(path))
}

pub fn clear_loaded_path() -> io::Result<bool> {
    let state_file = loaded_path_state_file()?;
    match fs::remove_file(state_file) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn os_args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn aliases_are_unique_and_resolve_both_spellings() {
        validate_aliases().unwrap();
        let mut seen = HashSet::new();
        for alias in COMMAND_ALIASES {
            assert!(seen.insert(alias.short));
            assert_eq!(canonical_command(alias.short), Some(alias.canonical));
            assert_eq!(canonical_command(alias.canonical), Some(alias.canonical));
            assert_eq!(short_command(alias.canonical), Some(alias.short));
        }
    }

    #[test]
    fn explicit_aliases_match_the_specification() {
        let expected = [
            ("list", "l"),
            ("scan", "s"),
            ("summary", "sm"),
            ("info", "i"),
            ("state", "st"),
            ("state-compare", "stc"),
            ("index-str-list", "ist"),
            ("index-zone-set-hex", "izs"),
            ("index-zone-extract", "ize"),
            ("index-zone-replace", "izr"),
            ("index-zone-copy", "izc"),
            ("index-zone-transfer", "izt"),
            ("db", "db"),
            ("hexline", "hl"),
            ("content", "co"),
            ("zone-info", "zi"),
            ("count", "ct"),
            ("clip", "c"),
            ("clip-zone", "cz"),
            ("clip-db", "cdb"),
            ("clip-dbline", "cdbl"),
            ("clip-hexline", "chl"),
            ("clip-hexword", "chw"),
            ("set-num", "sn"),
            ("set-str", "ss"),
            ("set-zone", "sz"),
            ("lines", "ln"),
            ("if-contains", "if"),
            ("ref-get", "rg"),
            ("ref-set", "rs"),
            ("ref-copy", "rc"),
            ("ref-diff", "rd"),
            ("ref-bool", "rb"),
            ("resolve-index", "ri"),
            ("zone-append", "za"),
        ];
        for (canonical, short) in expected {
            assert_eq!(short_command(canonical), Some(short));
        }
    }

    #[test]
    fn compact_refs_expand_to_canonical_forms() {
        let cases = [
            ("i38", "index:38"),
            ("i38s1", "index:38:string:1"),
            ("i38db9", "index:38:db:9"),
            ("i38dbl", "index:38:dbline"),
            ("i38hl", "index:38:hexline"),
            ("i38hwl", "index:38:hex-word-line"),
            ("i38rs", "index:38:ranges"),
            ("i38r2", "index:38:range:2"),
            ("i38z3", "index:38:zone:3"),
            ("i38zh1", "index:38:zonehex:1"),
            ("i38rh2", "index:38:rangehex:2"),
        ];
        for (compact, canonical) in cases {
            assert_eq!(compact_ref(compact).as_deref(), Some(canonical));
        }
        assert_eq!(compact_ref("index:38:string:1"), None);
        assert_eq!(compact_ref("inside"), None);
        assert_eq!(compact_ref("i38s4"), None);
    }

    #[test]
    fn ref_get_accepts_the_short_clip_suffix() {
        let mut args = os_args(&["rgd", "ref-get", "i38s1", "c"]);
        normalize_short_clip_flag(&mut args);
        assert_eq!(args, os_args(&["rgd", "ref-get", "i38s1", "--clip"]));

        let mut other = os_args(&["rgd", "ref-set", "i38s1", "c"]);
        normalize_short_clip_flag(&mut other);
        assert_eq!(other, os_args(&["rgd", "ref-set", "i38s1", "c"]));
    }

    #[test]
    fn global_flags_move_after_the_command_without_stealing_scan_value() {
        let mut ordinary = os_args(&["rgd", "-v", "l", "--no-save"]);
        normalize_global_arguments(&mut ordinary);
        assert_eq!(ordinary, os_args(&["rgd", "l", "--verbose", "--no-save"]));

        let mut scan = os_args(&["rgd", "s", "file.md", "-v", "0:5:50", "--verbose"]);
        normalize_global_arguments(&mut scan);
        assert_eq!(
            scan,
            os_args(&["rgd", "s", "file.md", "-v", "0:5:50", "--verbose"])
        );
    }

    #[test]
    fn loaded_file_is_inserted_at_command_specific_location() {
        let loaded = Path::new("C:/docs/example.md");
        assert_eq!(
            inject_loaded_file(
                &os_args(&["rgd", "index-str-list", "38"]),
                "index-str-list",
                loaded,
                false,
            )
            .unwrap(),
            os_args(&["rgd", "index-str-list", "C:/docs/example.md", "38"])
        );
        assert_eq!(
            inject_loaded_file(&os_args(&["rgd", "tx", "begin"]), "tx", loaded, false).unwrap(),
            os_args(&["rgd", "tx", "begin", "C:/docs/example.md"])
        );
        assert_eq!(
            inject_loaded_file(&os_args(&["rgd", "serve"]), "serve", loaded, false).unwrap(),
            os_args(&["rgd", "serve", "--file", "C:/docs/example.md"])
        );
        assert_eq!(
            inject_loaded_file(
                &os_args(&["rgd", "index-zone-transfer", "--from-index", "1"]),
                "index-zone-transfer",
                loaded,
                false,
            )
            .unwrap(),
            os_args(&[
                "rgd",
                "index-zone-transfer",
                "--from-file",
                "C:/docs/example.md",
                "--from-index",
                "1",
            ])
        );
    }
}
