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

fn assert_true(rgd: &Path, state: &Path, args: &[&str]) {
    let output = run(rgd, state, args);
    assert!(
        output.status.success(),
        "command {:?} failed\nstdout: {}\nstderr: {}",
        args,
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn boolean_commands_honor_every_exact_reference_scope() {
    let temporary = tempfile::tempdir().unwrap();
    let state = temporary.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let rgd = rgd_link(temporary.path());
    let document = temporary.path().join("scopes.txt");
    std::fs::write(
        &document,
        "zone alpha\nzone beta\nforeign payload\nprefix-regedited open-suffix\nindex: 8\n1x0000000 : 1x0000001 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000\n0.125 | -1.5 | 2 | 3 | 4 | 5 | 6 | 7 | 9007199254740993\nfirst alpha string\nsecond audit string\nthird string\n",
    )
    .unwrap();
    let document = document.to_string_lossy().to_string();
    assert_true(&rgd, &state, &["load", &document]);

    assert_true(&rgd, &state, &["ba", "i8s1", "first", "alpha"]);
    assert_true(&rgd, &state, &["bx", "i8s2", "audit", "missing"]);
    assert_true(&rgd, &state, &["ba", "i8db1", "0.125"]);
    assert_true(&rgd, &state, &["ba", "i8db2", "-1.5"]);
    assert_true(&rgd, &state, &["ba", "i8dbl", "0.125", "9007199254740993"]);
    assert_true(&rgd, &state, &["ba", "i8z1", "zone alpha", "zone beta"]);
    assert_true(
        &rgd,
        &state,
        &["ba", "i8", "first alpha", "0.125", "zone beta"],
    );
    assert_true(&rgd, &state, &["ba", "i8hl", "1x0000000"]);

    let selected = run(
        &rgd,
        &state,
        &[
            "if",
            "i8dbl",
            "9007199254740993",
            "--then-val",
            "EXACT",
            "--else-val",
            "MISSING",
        ],
    );
    assert!(selected.status.success(), "{}", text(&selected.stderr));
    assert_eq!(text(&selected.stdout).trim(), "EXACT");

    let isolated = run(&rgd, &state, &["ba", "i8s1", "foreign payload"]);
    assert_eq!(isolated.status.code(), Some(1));
    assert!(text(&isolated.stdout).starts_with("FALSE"));

    let invalid = run(&rgd, &state, &["ba", "not-a-ref", "first"]);
    assert!(!invalid.status.success());
    assert!(text(&invalid.stderr).contains("Boolean scope 'not-a-ref' is not a reference"));
}
