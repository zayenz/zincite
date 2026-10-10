use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(arguments: &[&str], source: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zincite-query"))
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
fn stdin_and_files_emit_original_source_or_counts() {
    let source = b"% attached\r\n'capacity' = 2; % tail\r\na = 1;\r\n";
    let output = run(
        &["--stdin-filepath", "data.dzn", "filter(name(\"capacity\"))"],
        source,
    );
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, b"% attached\r\n'capacity' = 2; % tail\r\n");
    let path = std::env::temp_dir().join(format!("zincite-query-{}.dzn", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let output = run(&["items | head(1) | count", path.to_str().unwrap()], b"");
    std::fs::remove_file(path).unwrap();
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, b"1\n");
}

#[test]
fn query_source_and_usage_errors_exit_two_with_empty_stdout() {
    for (arguments, source, expected) in [
        (vec!["count | emit"], "x = 1;", "<query>:1:"),
        (
            vec!["count", "--stdin-filepath", "data.dzn"],
            "constraint true;",
            "data.dzn:1:1:",
        ),
        (vec!["emit"], "int: x = ;", "<stdin>:1:"),
        (vec!["count", "a.dzn", "b.dzn"], "", "at most one input"),
        (
            vec!["count", "--model", "model.mzn"],
            "",
            "unsupported option",
        ),
        (
            vec!["count", "--stdin-filepath", "data.dzn", "model.mzn"],
            "",
            "requires stdin",
        ),
    ] {
        let output = run(&arguments, source.as_bytes());
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains(expected));
    }
    let output = run(&["--help"], b"");
    assert!(output.status.success() && output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.starts_with("Usage: zincite-query "));
    assert!(help.contains("filter") && help.contains("count"));
    assert!(!help.contains("--model") && help.contains("subtree"));
}

#[test]
fn assignment_previews_and_explicit_write_share_complete_validated_bytes() {
    let source = b"\xef\xbb\xbf% zincite-lint: ignore made-up-rule\r\nx = 1;\r\ny = 2;\r\n";
    let candidate = b"\xef\xbb\xbf% zincite-lint: ignore made-up-rule\r\nx = 3;\r\ny = 2;\r\n";
    let path = std::env::temp_dir().join(format!("zincite-query-edit-{}.dzn", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let expression = "filter(name(\"x\")) | set_value(\"3\")";
    let output = run(&[expression, path.to_str().unwrap()], b"");
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, candidate);
    assert_eq!(std::fs::read(&path).unwrap(), source);
    let output = run(&["--diff", expression, path.to_str().unwrap()], b"");
    assert!(output.status.success() && output.stderr.is_empty());
    assert!(output.stdout.starts_with(b"--- a/"));
    assert!(
        output
            .stdout
            .windows(b"+x = 3;\r\n".len())
            .any(|bytes| bytes == b"+x = 3;\r\n")
    );
    assert_eq!(std::fs::read(&path).unwrap(), source);
    for invalid in ["set_value(\"[1,\")", "head(1) | set_value(\"3 % comment\")"] {
        let output = run(&["--write", invalid, path.to_str().unwrap()], b"");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), source);
    }
    let output = run(&["--write", expression, path.to_str().unwrap()], b"");
    assert!(output.status.success() && output.stderr.is_empty() && output.stdout.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), candidate);
    std::fs::remove_file(path).unwrap();

    let output = run(
        &["--diff", "--stdin-filepath", "data.dzn", expression],
        source,
    );
    assert!(output.status.success() && output.stderr.is_empty());
    assert!(output.stdout.starts_with(b"--- a/data.dzn\n"));
}

#[test]
fn edit_modes_reject_invalid_usage_and_symlinks_without_partial_output() {
    for arguments in [
        vec!["--write", "--stdin-filepath", "data.dzn", "remove"],
        vec!["--write", "--diff", "remove"],
        vec!["--diff", "count"],
    ] {
        let output = run(&arguments, b"x = 1;\n");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    #[cfg(unix)]
    {
        let path =
            std::env::temp_dir().join(format!("zincite-query-target-{}.dzn", std::process::id()));
        let alias = path.with_extension("link.dzn");
        std::fs::write(&path, b"x = 1;\n").unwrap();
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        let output = run(&["--write", "remove", alias.to_str().unwrap()], b"");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), b"x = 1;\n");
        std::fs::remove_file(alias).unwrap();
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn json_uses_stdin_identity_and_errors_leave_stdout_empty() {
    let output = run(
        &[
            "--stdin-filepath",
            "model.mzn",
            "expressions | call_names | tally | json",
        ],
        b"constraint f(g(1), g(2));",
    );
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, b"{\"f\":1,\"g\":2}\n");
    let output = run(
        &["--stdin-filepath", "model.mzn", "json"],
        b"constraint true;",
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["file"], "model.mzn");
    for expression in [
        "expressions | range(0,999)",
        "names | children",
        "count | subtree",
    ] {
        let output = run(&[expression], b"constraint true;");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn collection_filters_inspect_preview_and_write_only_complete_candidates() {
    let source = b"x = [A: 1, B: -1, C: 2];\n";
    let candidate = b"x = [A: 1,  C: 2];\n";
    let path = std::env::temp_dir().join(format!(
        "zincite-query-collection-{}.dzn",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let expression = "filter_elements(gt(0))";
    let output = run(&[expression, path.to_str().unwrap()], b"");
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, candidate);
    assert_eq!(std::fs::read(&path).unwrap(), source);
    let output = run(&["--diff", expression, path.to_str().unwrap()], b"");
    assert!(output.status.success() && output.stderr.is_empty());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("+x = [A: 1,  C: 2];")
    );
    assert_eq!(std::fs::read(&path).unwrap(), source);
    let output = run(&["keys | names | json", path.to_str().unwrap()], b"");
    assert_eq!(output.stdout, b"[\"A\",\"B\",\"C\"]\n");
    let output = run(&["--write", expression, path.to_str().unwrap()], b"");
    assert!(output.status.success() && output.stderr.is_empty() && output.stdout.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), candidate);
    for source in [
        b"x = [1]; y = [f(2)];\n".as_slice(),
        b"x = [| 1, -1 | -1, 1 |];\n".as_slice(),
    ] {
        std::fs::write(&path, source).unwrap();
        let output = run(&["--write", expression, path.to_str().unwrap()], b"");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains(path.to_str().unwrap())
        );
        assert_eq!(std::fs::read(&path).unwrap(), source);
    }
    std::fs::remove_file(path).unwrap();
}
