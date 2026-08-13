// SPDX-License-Identifier: AGPL-3.0

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(executable: &Path, state: &Path, args: &[&str]) -> Output {
    Command::new(executable)
        .args(args)
        .env("REGEDITED_STATE_HOME", state)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace('\r', "")
}

fn rgd_link(directory: &Path) -> PathBuf {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_regedited"));
    let link = directory.join(format!("rgd{}", std::env::consts::EXE_SUFFIX));
    std::fs::hard_link(executable, &link).unwrap();
    link
}

fn canonical_document() -> String {
    let mut lines = vec![
        "# Numeric identity fixture".to_string(),
        "prefix-noise regedited open suffix-noise".to_string(),
        "index: 64".to_string(),
        "0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000".to_string(),
        "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9".to_string(),
        "first string".to_string(),
        "second string".to_string(),
        "third string".to_string(),
    ];
    for number in 8..=110 {
        lines.push(format!("line {}", number));
    }
    lines.join("\n")
}

#[test]
fn numeric_refs_help_and_line_assignment_work_end_to_end() {
    let temporary = tempfile::tempdir().unwrap();
    let state = temporary.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let rgd = rgd_link(temporary.path());
    let document = temporary.path().join("legacy.md");
    std::fs::write(&document, canonical_document()).unwrap();
    let document_text = document.to_string_lossy().to_string();

    let loaded = run(&rgd, &state, &["load", &document_text]);
    assert!(loaded.status.success(), "{}", text(&loaded.stderr));

    for reference in ["64", "i64", "index:64"] {
        let output = run(&rgd, &state, &["db", reference]);
        assert!(output.status.success(), "{}", text(&output.stderr));
        assert!(text(&output.stdout).contains("1"));
    }

    let assigned = run(&rgd, &state, &["cv", "b", "85", "95", "to", "i64", "1"]);
    assert!(assigned.status.success(), "{}", text(&assigned.stderr));
    let content = std::fs::read_to_string(&document).unwrap();
    assert!(
        content.contains("1x0000055 : 1x000005F : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000")
    );

    let direct = run(&rgd, &state, &["izl", "64", "2", "d", "20", "30"]);
    assert!(direct.status.success(), "{}", text(&direct.stderr));
    let content = std::fs::read_to_string(&document).unwrap();
    assert!(
        content.contains("1x0000055 : 1x000005F : 3x0000014 : 3x000001E : 0x0000000 : 0x0000000")
    );

    let bad_zone = run(&rgd, &state, &["cv", "b", "1", "2", "to", "i64", "4"]);
    assert!(!bad_zone.status.success());
    assert!(text(&bad_zone.stderr).contains("destination zone must be 1, 2, or 3"));

    let help = run(&rgd, &state, &["--help"]);
    assert!(help.status.success());
    let help_text = text(&help.stdout);
    assert!(help_text.contains("[INDEXES & DOCUMENT]"));
    assert!(help_text.contains("apply `--ex [1|2|3|4|5|6|7|8]` after `--help`"));

    let examples = run(&rgd, &state, &["--help", "--ex", "1"]);
    assert!(examples.status.success());
    let examples_text = text(&examples.stdout);
    assert!(examples_text.contains("rgd cv b 85 95 to i64 1"));
    assert!(examples_text.contains("[ZONES & LINE RANGES]"));

    for selector in 5..=8 {
        let advanced = run(
            &rgd,
            &state,
            &["ba", "--help", "--ex", &selector.to_string()],
        );
        assert!(advanced.status.success(), "{}", text(&advanced.stderr));
        let advanced_text = text(&advanced.stdout);
        assert!(advanced_text.contains("(advanced)"));
        let shell = match selector {
            5 => "PowerShell (pwsh)",
            6 => "Bash",
            7 => "Python",
            8 => "Command Prompt (cmd)",
            _ => unreachable!(),
        };
        assert!(advanced_text.contains(shell));
        assert!(advanced_text.contains("i8z1"));
        assert!(advanced_text.contains("i8db3"));
        assert!(advanced_text.contains("i90db4"));
    }

    let invalid_selector = run(&rgd, &state, &["ba", "--help", "--ex", "9"]);
    assert!(!invalid_selector.status.success());
    assert!(text(&invalid_selector.stderr)
        .contains("use 1-4 for standard PowerShell/Bash/Python/CMD or 5-8"));
}

#[test]
fn add_creates_canonical_numeric_indexes_and_rejects_duplicates() {
    let temporary = tempfile::tempdir().unwrap();
    let state = temporary.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let rgd = rgd_link(temporary.path());
    let document = temporary.path().join("document.md");
    std::fs::write(&document, canonical_document()).unwrap();
    let document_text = document.to_string_lossy().to_string();

    let loaded = run(&rgd, &state, &["load", &document_text]);
    assert!(loaded.status.success(), "{}", text(&loaded.stderr));

    let added = run(&rgd, &state, &["add", "900"]);
    assert!(added.status.success(), "{}", text(&added.stderr));
    let content = std::fs::read_to_string(&document).unwrap();
    assert!(content.contains("regedited open\nindex: 900"));

    let duplicate = run(&rgd, &state, &["add", "900"]);
    assert!(!duplicate.status.success());
    assert!(text(&duplicate.stderr).contains("already exists"));
}
