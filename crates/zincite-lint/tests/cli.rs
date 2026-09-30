use std::io::Write;
use std::process::{Command, Stdio};

fn run(arguments: &[&str], source: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
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
fn cli_reports_located_advice_errors_and_processes_independent_files_without_writes() {
    let warning = run(
        &["--stdin-filepath", "model.mzn", "-"],
        "% é\r\nconstraint true;",
    );
    assert_eq!(warning.status.code(), Some(1));
    assert!(warning.stdout.is_empty());
    let diagnostic = String::from_utf8(warning.stderr).unwrap();
    assert!(
        diagnostic.contains("model.mzn:2:1: bytes 6..16: warning [missing-constraint-label]"),
        "{diagnostic}"
    );
    for source in [
        "constraint :: \"label\" true;",
        "% zincite-lint: ignore missing-constraint-label\nconstraint true;",
    ] {
        let output = run(&[], source);
        assert!(output.status.success());
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    let data = run(&["--stdin-filepath", "data.dzn"], "BAD_TARGET=1;");
    assert!(data.status.success());
    for source in [
        "constraint true; int:x=;",
        "% zincite-lint: ignore unknown\nconstraint true;",
    ] {
        let output = run(&[], source);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("error:") && !diagnostic.contains("warning ["));
    }
    for arguments in [
        vec!["-", "model.mzn"],
        vec!["--stdin-filepath"],
        vec!["--stdin-filepath", "a.mzn", "b.mzn"],
        vec!["--write"],
    ] {
        assert_eq!(run(&arguments, "").status.code(), Some(2));
    }
    let naming = run(&[], "int: BadName = 1;");
    assert_eq!(naming.status.code(), Some(1));
    assert!(naming.stdout.is_empty());
    assert!(
        String::from_utf8(naming.stderr)
            .unwrap()
            .contains("bytes 5..12: warning [naming]")
    );
    let naming = run(&[], "% zincite-lint: ignore naming\nint: BadName = 1;");
    assert!(naming.status.success());
    assert!(naming.stdout.is_empty() && naming.stderr.is_empty());
    let directory = std::env::temp_dir().join(format!("zincite-lint-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let bad = directory.join("bad.mzn");
    let good = directory.join("good.mzn");
    std::fs::write(&bad, b"\xff").unwrap();
    std::fs::write(&good, b"constraint true;").unwrap();
    let output = run(&[bad.to_str().unwrap(), good.to_str().unwrap()], "");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(
        diagnostic.contains("not UTF-8")
            && diagnostic.contains("warning [missing-constraint-label]")
    );
    assert_eq!(std::fs::read(&bad).unwrap(), b"\xff");
    assert_eq!(std::fs::read(&good).unwrap(), b"constraint true;");
    std::fs::remove_dir_all(directory).unwrap();
    let help = run(&["--help"], "");
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(
        help.contains("ignore naming")
            && help.contains("Exit codes")
            && help.contains("variant_record")
    );
}
