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

fn base_document() -> String {
    [
        "# document",
        "regedited open",
        "index: 1",
        "0x0000009 : 0x000000B : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
        "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9",
        "summary one",
        "summary two",
        "summary three",
        "---",
        "alpha",
        "beta",
        "gamma",
        "tail",
    ]
    .join("\n")
}

fn shifted_document() -> String {
    [
        "# document",
        "regedited open",
        "index: 2",
        "0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
        "0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0",
        "new one",
        "new two",
        "new three",
        "---",
        "new body",
        "regedited open",
        "index: 1",
        "0x0000009 : 0x000000B : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
        "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9",
        "summary one",
        "summary two",
        "summary three",
        "---",
        "alpha",
        "beta",
        "gamma",
        "tail",
    ]
    .join("\n")
}

#[test]
fn loaded_path_commit_check_pull_and_undo_work_end_to_end() {
    let temporary = tempfile::tempdir().unwrap();
    let state = temporary.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let rgd = rgd_link(temporary.path());
    let document = temporary.path().join("document.md");
    std::fs::write(&document, base_document()).unwrap();
    let document_text = document.to_string_lossy().to_string();

    let loaded = run(&rgd, &state, &["load", &document_text]);
    assert!(loaded.status.success(), "{}", text(&loaded.stderr));

    let committed = run(&rgd, &state, &["cm"]);
    assert!(committed.status.success(), "{}", text(&committed.stderr));
    assert!(text(&committed.stdout).contains("committed 1 active zone(s)"));
    assert!(PathBuf::from(format!("{}.rgd-state.json", document.display())).is_file());

    std::fs::write(&document, shifted_document()).unwrap();
    let checked = run(&rgd, &state, &["ck"]);
    assert!(checked.status.success(), "{}", text(&checked.stderr));
    let checked_stdout = text(&checked.stdout);
    assert!(
        checked_stdout.contains("move index 1 zone 1: 9-11 -> 18-20 (exact-content)"),
        "{checked_stdout}"
    );
    assert!(checked_stdout.contains("added indexes: [2]"));
    let diff_path = checked_stdout
        .lines()
        .find_map(|line| line.strip_prefix("diff="))
        .map(PathBuf::from)
        .unwrap();
    assert!(diff_path.is_file());

    let pulled = run(&rgd, &state, &["pl"]);
    assert!(pulled.status.success(), "{}", text(&pulled.stderr));
    assert!(text(&pulled.stdout).contains("pulled 1 zone range(s)"));
    let updated = std::fs::read_to_string(&document).unwrap();
    assert!(
        updated.contains("0x0000012 : 0x0000014 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000")
    );
    assert!(!diff_path.exists());

    let undo_path = PathBuf::from(format!("{}.undo", document.display()));
    assert!(undo_path.is_file());
    let undo = run(&rgd, &state, &["u"]);
    assert!(undo.status.success(), "{}", text(&undo.stderr));
    let restored = std::fs::read_to_string(&document).unwrap();
    assert!(
        restored.contains("0x0000009 : 0x000000B : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000")
    );
}
