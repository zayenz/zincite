use std::io::Write;
use std::process::{Command, Stdio};

fn run(arguments: &[&str], source: &str) -> std::process::Output {
    run_bytes(arguments, source.as_bytes())
}

fn run_bytes(arguments: &[&str], source: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zincite-fmt"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(source).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn opaque_comment_bytes_survive_cli_modes_while_errors_preserve_input() {
    let directory =
        std::env::temp_dir().join(format!("zincite-opaque-comments-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("values.dzn");
    let config = directory.join(".editorconfig");
    std::fs::write(
        &config,
        "root = true\n[*]\ncharset = utf-8-bom\nend_of_line = crlf\n",
    )
    .unwrap();
    let source = b"\xef\xbb\xbf/* \xe7\r\n  keep  \xff */\r\nvalue=1; /* \xe9 */";
    let expected = b"\xef\xbb\xbf/* \xe7\r\n  keep  \xff */\r\nvalue = 1; /* \xe9 */\r\n";
    let plain = run_bytes(&[], source);
    assert!(plain.status.success(), "{plain:?}");
    assert!(plain.stderr.is_empty());
    assert_eq!(
        plain.stdout,
        b"/* \xe7\r\n  keep  \xff */\nvalue = 1; /* \xe9 */\n"
    );
    let arguments = ["--stdin-filepath", path.to_str().unwrap()];
    let output = run_bytes(&arguments, source);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, expected);
    let check = ["--check", "--stdin-filepath", path.to_str().unwrap()];
    let output = run_bytes(&check, source);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let output = run_bytes(&check, expected);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty());

    std::fs::write(&path, source).unwrap();
    let output = run(&["--check", path.to_str().unwrap()], "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), source);
    let output = run(&["--write", path.to_str().unwrap()], "");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let output = run(&["--check", path.to_str().unwrap()], "");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty());

    // The body is already formatted: changing only the BOM policy must count.
    std::fs::write(
        &config,
        "root = true\n[*]\ncharset = utf-8\nend_of_line = crlf\n",
    )
    .unwrap();
    let output = run(&["--check", path.to_str().unwrap()], "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let output = run(&["--write", path.to_str().unwrap()], "");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), &expected[3..]);

    for (invalid, location, message) in [
        (
            b"\xef\xbb\xbf/* \xff \xc3\xa9 */ value=\xfe;".as_slice(),
            ":1:17: bytes 20..21:",
            "invalid UTF-8 is only supported inside comments",
        ),
        (
            b"\xef\xbb\xbf/* \xe7 */\r\n% zincite-fmt: skip\xff\r\nvalue=1;".as_slice(),
            ":2:1: bytes 12..32:",
            "unknown or malformed formatting directive",
        ),
    ] {
        let output = run_bytes(&arguments, invalid);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains(location), "{diagnostic}");
        assert!(diagnostic.contains(message), "{diagnostic}");
        std::fs::write(&path, invalid).unwrap();
        let output = run(&["--write", path.to_str().unwrap()], "");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains(location), "{diagnostic}");
        assert!(diagnostic.contains(message), "{diagnostic}");
        assert_eq!(std::fs::read(&path).unwrap(), invalid);
    }
    std::fs::remove_dir_all(directory).unwrap();
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
        let output = run(&arguments, "");
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
    assert!(diagnostic.contains("formatting on directive") && diagnostic.contains("invalid UTF-8"));
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

#[test]
fn editorconfig_parent_sections_unset_and_cli_overrides() {
    let directory = std::env::temp_dir().join(format!("zincite-config-{}", std::process::id()));
    let root = directory.join("project");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(directory.join(".editorconfig"), "[*]\ncharset = latin1\n").unwrap();
    std::fs::write(root.join(".editorconfig"), "root = true\n[*]\nindent_style = space\nindent_size = 2\nend_of_line = crlf\nmax_line_length = 20\nunknown_property = ignored\n").unwrap();
    std::fs::write(nested.join(".editorconfig"), "[*.mzn]\nindent_size = 3\nend_of_line = unset\nmax_line_length = off\n[model.mzn]\nindent_size = 6\n").unwrap();
    let path = nested.join("model.mzn");
    let source = "constraint if true then true else false endif;";
    std::fs::write(&path, source).unwrap();
    let expected = "constraint if true then\n      true\nelse\n      false\nendif;\n";
    let output = run(&[path.to_str().unwrap()], "");
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, expected.as_bytes());
    assert_eq!(
        run(&["--stdin-filepath", path.to_str().unwrap()], source).stdout,
        expected.as_bytes()
    );
    let overridden = run(
        &[
            "--indent-size",
            "2",
            "--indent-style",
            "tab",
            "--tab-width",
            "2",
            "--end-of-line",
            "cr",
            "--max-line-length",
            "off",
            path.to_str().unwrap(),
        ],
        "",
    );
    assert!(overridden.status.success(), "{:?}", overridden);
    assert_eq!(
        overridden.stdout,
        b"constraint if true then\r\ttrue\relse\r\tfalse\rendif;\r"
    );
    assert_eq!(
        run(&["--check", path.to_str().unwrap()], "").status.code(),
        Some(1)
    );
    assert!(
        run(&["--write", path.to_str().unwrap()], "")
            .status
            .success()
    );
    assert_eq!(std::fs::read(&path).unwrap(), expected.as_bytes());
    assert!(
        run(&["--check", path.to_str().unwrap()], "")
            .status
            .success()
    );
    // Plain stdin must not consult even an invalid config in its working directory.
    let mut child = Command::new(env!("CARGO_BIN_EXE_zincite-fmt"))
        .current_dir(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"int:x=1;").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"int: x = 1;\n");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn editorconfig_protected_bytes_bom_and_invalid_settings() {
    let directory =
        std::env::temp_dir().join(format!("zincite-config-bytes-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let config = directory.join(".editorconfig");
    let path = directory.join("model.mzn");
    std::fs::write(&config, "root = true\n[*]\nend_of_line = CRLF\nindent_style = SPACE\ninsert_final_newline = FALSE\ntrim_trailing_whitespace = TRUE\ncharset = UTF-8-BOM\n").unwrap();
    let source = "/* first  \nsecond\r\n */\nstring:s=\"line\\n  \";\n% zincite-fmt: skip\nint:  skipped=2;  \n";
    let output = run(&["--stdin-filepath", path.to_str().unwrap()], source);
    assert!(output.status.success(), "{:?}", output);
    let expected = "\u{feff}/* first  \nsecond\r\n */\r\nstring: s = \"line\\n  \";\n% zincite-fmt: skip\nint:  skipped=2;  \n";
    assert_eq!(output.stdout, expected.as_bytes());
    assert_eq!(
        run(&["--stdin-filepath", path.to_str().unwrap()], expected).stdout,
        expected.as_bytes()
    );
    assert_eq!(
        run(
            &["--stdin-filepath", path.to_str().unwrap()],
            "int:x=1;  \n"
        )
        .stdout,
        "\u{feff}int: x = 1;".as_bytes()
    );
    std::fs::write(&path, "int:x=1;").unwrap();
    for property in [
        "charset = latin1",
        "tab_width = 0",
        "insert_final_newline = perhaps",
        "indent_size =",
        "end_of_line =",
    ] {
        std::fs::write(&config, format!("root = true\n[*]\n{property}\n")).unwrap();
        let output = run(&["--write", path.to_str().unwrap()], "");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty() && !output.stderr.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), b"int:x=1;");
    }
    for arguments in [
        vec!["--indent-size", "0"],
        vec!["--end-of-line", "native"],
        vec!["--max-line-length"],
    ] {
        assert_eq!(run(&arguments, "").status.code(), Some(2));
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn directory_checks_discover_sources_deduplicate_and_preserve_every_input() {
    let root = std::env::temp_dir().join(format!("zincite-fmt-tree-{}", std::process::id()));
    let nested = root.join(".hidden");
    let git = root.join(".git");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(&git).unwrap();
    std::fs::write(root.join(".gitignore"), "ignored.mzn\n").unwrap();
    let hidden = nested.join("values.dzn");
    let ignored = root.join("ignored.mzn");
    std::fs::write(&hidden, "x=1;").unwrap();
    std::fs::write(&ignored, "int: x = 1;\n").unwrap();
    std::fs::write(git.join("invalid.mzn"), "int: x = ;").unwrap();
    std::fs::write(root.join("notes.txt"), "int: x = ;").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&root, nested.join("cycle")).unwrap();
    let check = run(
        &[
            "--check",
            root.to_str().unwrap(),
            nested.to_str().unwrap(),
            hidden.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(check.status.code(), Some(1));
    assert!(check.stdout.is_empty() && check.stderr.is_empty());
    for mode in [
        vec![root.to_str().unwrap()],
        vec!["--write", ignored.to_str().unwrap(), root.to_str().unwrap()],
    ] {
        let output = run(&mode, "");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("directory inputs require --check")
        );
    }
    let invalid = nested.join("invalid.mzn");
    std::fs::write(&invalid, "int: x = ;").unwrap();
    let missing = root.join("absent.mzn");
    let output = run(
        &[
            "--check",
            missing.to_str().unwrap(),
            root.to_str().unwrap(),
            invalid.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("absent.mzn"));
    assert_eq!(errors.matches("invalid.mzn:").count(), 1, "{errors}");
    assert_eq!(std::fs::read(&hidden).unwrap(), b"x=1;");
    assert_eq!(std::fs::read(&ignored).unwrap(), b"int: x = 1;\n");
    assert_eq!(std::fs::read(&invalid).unwrap(), b"int: x = ;");
    assert_eq!(
        std::fs::read(git.join("invalid.mzn")).unwrap(),
        b"int: x = ;"
    );
    std::fs::remove_dir_all(root).unwrap();
}
