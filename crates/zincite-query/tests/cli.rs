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
    assert!(!help.contains("--model") && !help.contains("subtree"));
}
