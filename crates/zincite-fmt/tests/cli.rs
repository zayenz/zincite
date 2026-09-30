use std::io::Write;
use std::process::{Command, Stdio};

fn run(arguments: &[&str], source: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zincite-fmt"))
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
fn paths_and_stdin_paths_select_data_syntax_and_reject_mixed_inputs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    for file in ["data-model.mzn", "data.dzn"] {
        let output = run(&[root.join(file).to_str().unwrap()], "");
        assert!(output.status.success(), "{:?}", output);
    }
    for arguments in [
        vec!["--stdin-filepath", "input.dzn"],
        vec!["-", "--stdin-filepath", "input.dzn"],
    ] {
        let output = run(&arguments, "constraint true;");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(
            diagnostic.contains("input.dzn:1:1: bytes 0..10"),
            "{diagnostic}"
        );
        assert!(diagnostic.contains("only assignments"));
        assert!(run(&arguments, "value=1;").status.success());
    }
    for arguments in [vec![], vec!["--stdin-filepath", "input.mzn"]] {
        assert!(run(&arguments, "constraint true;").status.success());
    }
    for arguments in [
        vec!["--stdin-filepath"],
        vec![
            "--stdin-filepath",
            "input.dzn",
            "--stdin-filepath",
            "other.dzn",
        ],
        vec!["--stdin-filepath", "input.dzn", "model.mzn"],
        vec!["-", "model.mzn"],
        vec!["--check", "--write", "model.mzn"],
        vec!["--write"],
        vec!["--write", "-"],
        vec!["--check", "-", "model.mzn"],
        vec!["--write", "--stdin-filepath", "input.mzn"],
        vec!["first.mzn", "second.mzn"],
    ] {
        let output = run(&arguments, "value=1;");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    for (source, status) in [("int:x=1;", 1), ("int: x = 1;\n", 0), ("int:x=;", 2)] {
        let output = run(&["--check"], source);
        assert_eq!(output.status.code(), Some(status));
        assert!(output.stdout.is_empty());
    }
    let output = run(
        &["--check", "--stdin-filepath", "input.dzn", "-"],
        "value=1;",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let help = run(&["--help"], "");
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("--check") && help.contains("--write") && help.contains("Exit codes:"));
    // Use the same model bytes under a data path to check file-mode enforcement.
    let path = std::env::temp_dir().join(format!("zincite-data-mode-{}.dzn", std::process::id()));
    std::fs::write(&path, "constraint true;").unwrap();
    let output = run(&[path.to_str().unwrap()], "");
    std::fs::remove_file(path).unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("only assignments")
    );
}

#[test]
fn check_and_write_modes_preserve_failures_and_process_independent_files() {
    let directory = std::env::temp_dir().join(format!("zincite-file-modes-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let clean = directory.join("clean.mzn");
    let changed = directory.join("changed.mzn");
    let syntax = directory.join("syntax.mzn");
    let directive = directory.join("directive.mzn");
    let utf8 = directory.join("utf8.mzn");
    let missing = directory.join("missing.mzn");
    let clean_source = "int: clean = 1;\n";
    let changed_source = "int:changed=2;";
    let syntax_source = "int: bad = ; int: after = 7;";
    let directive_source = "% zincite-fmt: on\nint:x=1;";
    std::fs::write(&clean, clean_source).unwrap();
    std::fs::write(&changed, changed_source).unwrap();
    std::fs::write(&syntax, syntax_source).unwrap();
    std::fs::write(&directive, directive_source).unwrap();
    std::fs::write(&utf8, b"int:x=\xff;").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&changed, std::fs::Permissions::from_mode(0o640)).unwrap();
    }
    let output = run(&["--check", clean.to_str().unwrap()], "");
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let output = run(
        &[
            "--check",
            clean.to_str().unwrap(),
            changed.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert_eq!(std::fs::read(&changed).unwrap(), changed_source.as_bytes());
    let output = run(
        &[
            "--check",
            changed.to_str().unwrap(),
            syntax.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(std::fs::read(&changed).unwrap(), changed_source.as_bytes());

    // Errors precede the valid file, so its replacement proves continued work.
    let output = run(
        &[
            "--write",
            syntax.to_str().unwrap(),
            directive.to_str().unwrap(),
            utf8.to_str().unwrap(),
            missing.to_str().unwrap(),
            changed.to_str().unwrap(),
            clean.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    for path in [&syntax, &directive, &utf8, &missing] {
        assert!(diagnostic.contains(path.to_str().unwrap()), "{diagnostic}");
    }
    assert!(diagnostic.contains("formatting on directive") && diagnostic.contains("not UTF-8"));
    assert_eq!(std::fs::read(&syntax).unwrap(), syntax_source.as_bytes());
    assert_eq!(
        std::fs::read(&directive).unwrap(),
        directive_source.as_bytes()
    );
    assert_eq!(std::fs::read(&utf8).unwrap(), b"int:x=\xff;");
    assert_eq!(std::fs::read(&changed).unwrap(), b"int: changed = 2;\n");
    assert_eq!(std::fs::read(&clean).unwrap(), clean_source.as_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&changed).unwrap().permissions().mode() & 0o7777,
            0o640
        );
    }
    let output = run(
        &[
            "--write",
            changed.to_str().unwrap(),
            clean.to_str().unwrap(),
        ],
        "",
    );
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert!(
        run(
            &[
                "--check",
                changed.to_str().unwrap(),
                clean.to_str().unwrap()
            ],
            ""
        )
        .status
        .success()
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_inputs_can_be_read_but_never_written() {
    let directory = std::env::temp_dir().join(format!("zincite-symlinks-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let target = directory.join("target.mzn");
    let link = directory.join("link.mzn");
    let independent = directory.join("independent.mzn");
    let source = "int:target=1;";
    std::fs::write(&target, source).unwrap();
    std::fs::write(&independent, "int:other=2;").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let output = run(&[link.to_str().unwrap()], "");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"int: target = 1;\n");
    let output = run(&["--check", link.to_str().unwrap()], "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let output = run(
        &[
            "--write",
            link.to_str().unwrap(),
            independent.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("refusing --write through a symlink")
    );
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&target).unwrap(), source.as_bytes());
    assert_eq!(std::fs::read(&independent).unwrap(), b"int: other = 2;\n");
    std::fs::remove_dir_all(directory).unwrap();
}
