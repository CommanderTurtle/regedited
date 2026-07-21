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

#[test]
fn canonical_and_linked_qol_surfaces_work_end_to_end() {
    let temporary = tempfile::tempdir().unwrap();
    let state = temporary.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let rgd = rgd_link(temporary.path());
    let canonical = PathBuf::from(env!("CARGO_BIN_EXE_regedited"));
    let document = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/example.md");
    let document_text = document.to_string_lossy().to_string();

    let converted = run(&canonical, &state, &["convert", "d", "58", "p", "59"]);
    assert!(converted.status.success(), "{}", text(&converted.stderr));
    assert_eq!(text(&converted.stdout).trim(), "3x000003A : 0x000003B");

    let converted_short = run(&rgd, &state, &["cv", "b", "10", "20"]);
    assert!(
        converted_short.status.success(),
        "{}",
        text(&converted_short.stderr)
    );
    assert_eq!(
        text(&converted_short.stdout).trim(),
        "1x000000A : 1x0000014"
    );

    let loaded = run(&rgd, &state, &["load", &document_text]);
    assert!(loaded.status.success(), "{}", text(&loaded.stderr));

    let listed = run(&rgd, &state, &["l"]);
    assert!(listed.status.success(), "{}", text(&listed.stderr));
    assert!(text(&listed.stdout).contains("ProjectConfig"));

    let peer = temporary.path().join("peer.md");
    std::fs::copy(&document, &peer).unwrap();
    let peer_text = peer.to_string_lossy().to_string();
    let compared = run(&rgd, &state, &["d", &peer_text]);
    assert!(compared.status.success(), "{}", text(&compared.stderr));
    assert!(text(&compared.stdout).contains("Identical: 4"));

    let snapshot = run(&rgd, &state, &["st"]);
    assert!(snapshot.status.success(), "{}", text(&snapshot.stderr));
    let snapshot_path = temporary.path().join("state.json");
    std::fs::write(&snapshot_path, &snapshot.stdout).unwrap();
    let snapshot_text = snapshot_path.to_string_lossy().to_string();
    let state_comparison = run(&rgd, &state, &["stc", &snapshot_text]);
    assert!(
        state_comparison.status.success(),
        "{}",
        text(&state_comparison.stderr)
    );
    assert_eq!(text(&state_comparison.stdout).trim(), "EQUAL");

    let reference = run(&rgd, &state, &["rg", "i100s1"]);
    assert!(reference.status.success(), "{}", text(&reference.stderr));
    assert_eq!(text(&reference.stdout).trim(), "project root path");

    let third_string = run(&rgd, &state, &["rg", "i100s3"]);
    assert!(
        third_string.status.success(),
        "{}",
        text(&third_string.stderr)
    );
    assert_eq!(
        text(&third_string.stdout).trim(),
        "https://github.com/user/project"
    );

    let ninth_number = run(&rgd, &state, &["rg", "i100db9"]);
    assert!(
        ninth_number.status.success(),
        "{}",
        text(&ninth_number.stderr)
    );
    assert_eq!(text(&ninth_number.stdout).trim(), "30");

    let transaction = run(&rgd, &state, &["tx", "status"]);
    assert!(
        transaction.status.success(),
        "{}",
        text(&transaction.stderr)
    );

    let help = run(&rgd, &state, &["rg", "-help"]);
    assert!(help.status.success(), "{}", text(&help.stderr));
    let help = text(&help.stdout);
    assert!(help.contains("rgd rg -> regedited ref-get"));
    assert!(help.contains("Usage: rgd rg"), "{help}");

    let examples = run(&canonical, &state, &["-ex", "script", "python"]);
    assert!(examples.status.success(), "{}", text(&examples.stderr));
    assert!(text(&examples.stdout).contains("subprocess.run"));

    let unloaded = run(&rgd, &state, &["unload"]);
    assert!(unloaded.status.success(), "{}", text(&unloaded.stderr));
    let missing = run(&rgd, &state, &["l"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(text(&missing.stderr).contains(
        "No path specified yet, specify path, else perform a load first, i.e. `rgd load ~/example/file/location.txt`"
    ));
}
