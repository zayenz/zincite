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
    ] {
        let output = run(&arguments, "value=1;");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
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
