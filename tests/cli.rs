use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(arguments: &[&str], source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zincite"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn formatter_stdout_and_check_keep_standalone_defaults() {
    let source = "int: value=1;";
    let formatted = "int: value = 1;\n";
    let output = run(&["fmt"], source);
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, formatted.as_bytes());
    for (source, status) in [(source, 1), (formatted, 0)] {
        let output = run(&["fmt", "--check"], source);
        assert_eq!(output.status.code(), Some(status));
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
}

#[test]
fn lint_selection_and_settings_keep_standalone_behavior() {
    let source = "int: BadName=1; constraint true;";
    let output = run(&["lint", "--isolated", "--rules", "naming"], source);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostics.contains("warning [naming]"));
    assert!(!diagnostics.contains("missing-constraint-label"));

    let directory =
        std::env::temp_dir().join(format!("zincite-root-settings-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let config = directory.join("zincite.toml");
    std::fs::write(&config, "[lint]\nselect=['missing-constraint-label']\n").unwrap();
    let path = directory.join("unsaved.mzn");
    let arguments = ["lint", "--stdin-filepath", path.to_str().unwrap()];
    let output = run(&arguments, source);
    assert_eq!(output.status.code(), Some(1));
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert!(!diagnostics.contains("warning [naming]"));
    assert!(diagnostics.contains("warning [missing-constraint-label]"));
    let shown = run(
        &[
            "lint",
            "--show-settings",
            "--stdin-filepath",
            path.to_str().unwrap(),
        ],
        "invalid MiniZinc",
    );
    assert!(shown.status.success() && shown.stderr.is_empty());
    let settings = String::from_utf8(shown.stdout).unwrap();
    assert!(settings.contains(&format!("Root: {}\n", path.display())));
    assert!(settings.contains("Rules: missing-constraint-label\n"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn help_uses_the_invocation_and_usage_errors_exit_two() {
    for arguments in [&[][..], &["--help"][..], &["-h"][..]] {
        let output = run(arguments, "");
        assert!(output.status.success() && output.stderr.is_empty());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.starts_with("Usage: zincite <COMMAND>"));
        assert!(help.contains("fmt") && help.contains("lint"));
        assert!(!help.contains("query"));
    }
    for command in ["fmt", "lint"] {
        let output = run(&[command, "--help"], "");
        assert!(output.status.success() && output.stderr.is_empty());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.starts_with(&format!("Usage: zincite {command} ")));
        if command == "lint" {
            assert!(help.contains("% zincite-lint: ignore naming"));
            assert!(!help.contains("       zincite-lint ["));
        }
        let invalid = run(&[command, "--unsupported"], "");
        assert_eq!(invalid.status.code(), Some(2));
        assert!(invalid.stdout.is_empty() && !invalid.stderr.is_empty());
    }
    let unknown = run(&["query"], "");
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    assert!(
        String::from_utf8(unknown.stderr)
            .unwrap()
            .contains("unknown command 'query'")
    );
}
