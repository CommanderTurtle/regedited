//! Use session-native helpers whose clipboard ownership survives CLI exit.
use std::io::{self, Write};
use std::process::{Command, Stdio};

fn run(program: &str, args: &[&str], text: Option<&str>) -> io::Result<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(if text.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(if text.is_some() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        // Clipboard owners fork; inherited capture pipes would keep callers waiting.
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(text) = text {
        let written = child.stdin.take().unwrap().write_all(text.as_bytes());
        let status = child.wait()?;
        if !status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("{program} failed ({status}); check the graphical clipboard session"),
            ));
        }
        written?;
        Ok(Vec::new())
    } else {
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "{program} failed ({}); clipboard may be empty or unavailable",
                    output.status
                ),
            ));
        }
        Ok(output.stdout)
    }
}

fn transfer(
    wayland: bool,
    x11: bool,
    text: Option<&str>,
    mut invoke: impl FnMut(&str, &[&str], Option<&str>) -> io::Result<Vec<u8>>,
) -> io::Result<Vec<u8>> {
    let mut helpers: Vec<(&str, &[&str])> = Vec::new();
    if wayland {
        helpers.push(if text.is_some() {
            ("wl-copy", &["--type", "text/plain;charset=utf-8"])
        } else {
            ("wl-paste", &["--no-newline", "--type", "text"])
        });
    }
    if x11 {
        helpers.push(if text.is_some() {
            ("xclip", &["-selection", "clipboard", "-in"])
        } else {
            ("xclip", &["-selection", "clipboard", "-out"])
        });
        helpers.push(if text.is_some() {
            ("xsel", &["--clipboard", "--input"])
        } else {
            ("xsel", &["--clipboard", "--output"])
        });
    }
    if helpers.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "No graphical clipboard session: WAYLAND_DISPLAY and DISPLAY are unset",
        ));
    }
    for (program, args) in helpers {
        match invoke(program, args, text) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            // Do not replace a real helper failure/empty clipboard with stale fallback data.
            result => return result,
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "No clipboard helper found: Wayland needs wl-copy/wl-paste; X11 needs xclip or xsel on PATH"))
}

fn session() -> (bool, bool) {
    let present = |key| std::env::var_os(key).is_some_and(|v| !v.is_empty());
    (present("WAYLAND_DISPLAY"), present("DISPLAY"))
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(super) fn copy(text: &str) -> io::Result<()> {
    let (wayland, x11) = session();
    transfer(wayland, x11, Some(text), run).map(|_| ())
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(super) fn paste() -> io::Result<String> {
    let (wayland, x11) = session();
    String::from_utf8(transfer(wayland, x11, None, run)?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Clipboard text is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_copy_preserves_unicode_newlines_and_literal_shell_text() {
        for text in [
            "",
            "世界 🦀\n{\"json\":true}\n\n",
            "$(exit); --clear | & <>",
        ] {
            transfer(true, true, Some(text), |program, args, input| {
                assert_eq!(program, "wl-copy");
                assert_eq!(args, ["--type", "text/plain;charset=utf-8"]);
                assert_eq!(input, Some(text));
                Ok(Vec::new())
            })
            .unwrap();
        }
    }

    #[test]
    fn paste_preserves_exact_bytes_without_added_or_trimmed_newlines() {
        let expected = "hello 世界\n\n".as_bytes();
        let result = transfer(true, true, None, |program, args, input| {
            assert_eq!(program, "wl-paste");
            assert_eq!(args, ["--no-newline", "--type", "text"]);
            assert_eq!(input, None);
            Ok(expected.to_vec())
        })
        .unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn missing_helpers_fall_back_to_the_x11_clipboard_not_primary() {
        for text in [Some("value"), None] {
            let mut programs = Vec::new();
            transfer(true, true, text, |program, args, _| {
                programs.push(program.to_owned());
                if program == "xsel" {
                    assert_eq!(
                        args,
                        [
                            "--clipboard",
                            if text.is_some() {
                                "--input"
                            } else {
                                "--output"
                            }
                        ]
                    );
                    Ok(Vec::new())
                } else {
                    if program == "xclip" {
                        assert_eq!(args[0..2], ["-selection", "clipboard"]);
                    }
                    Err(io::Error::from(io::ErrorKind::NotFound))
                }
            })
            .unwrap();
            assert_eq!(
                programs,
                [
                    if text.is_some() {
                        "wl-copy"
                    } else {
                        "wl-paste"
                    },
                    "xclip",
                    "xsel"
                ]
            );
        }
    }

    #[test]
    fn real_failures_are_reported_without_claiming_success_or_reading_stale_x11_data() {
        let mut calls = 0;
        assert!(transfer(true, true, None, |_, _, _| {
            calls += 1;
            Err(io::Error::new(io::ErrorKind::Other, "empty clipboard"))
        })
        .is_err());
        assert_eq!(calls, 1);
    }

    #[test]
    fn headless_or_missing_tools_fail_cleanly() {
        let result = transfer(false, false, Some("value"), |_, _, _| {
            panic!("No session must not spawn a helper")
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotConnected);
        let result = transfer(true, false, None, |_, _, _| {
            Err(io::Error::from(io::ErrorKind::NotFound))
        });
        assert!(result.unwrap_err().to_string().contains("wl-copy/wl-paste"));
    }

    #[test]
    fn process_spawn_failure_is_returned() {
        assert_eq!(
            run(
                "regedited-nonexistent-clipboard-test-helper",
                &[],
                Some("value")
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::NotFound
        );
    }

    #[cfg(unix)]
    #[test]
    fn helper_processes_preserve_output_and_report_nonzero_exit() {
        assert_eq!(
            run("sh", &["-c", "printf 'one\\n\\n'"], None).unwrap(),
            b"one\n\n"
        );
        assert!(run("sh", &["-c", "cat > /dev/null"], Some("世界\n\n")).is_ok());
        assert!(run("sh", &["-c", "exit 7"], Some("value"))
            .unwrap_err()
            .to_string()
            .contains("7"));
    }
}
