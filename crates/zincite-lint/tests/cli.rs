use std::io::Write;
use std::process::{Command, Stdio};

fn run(arguments: &[&str], source: &str) -> std::process::Output {
    run_with_library(arguments, source, None)
}

fn run_with_library(
    arguments: &[&str],
    source: &str,
    library: Option<&str>,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zincite-lint"));
    if let Some(library) = library {
        command.env("MZN_STDLIB_DIR", library);
    }
    let mut child = command
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
fn catalogue_commands_inspect_metadata_without_reading_or_analyzing_inputs() {
    let list = run(&["--list-rules"], "not valid MiniZinc");
    assert!(list.status.success() && list.stderr.is_empty());
    let list = String::from_utf8(list.stdout).unwrap();
    assert_eq!(list.lines().count(), 17);
    assert!(list.contains("naming\tstyle\tavailable\tnone\tdefault"));
    assert!(list.contains("search-coverage\tmodelling\tavailable\tnone\tthesis"));
    let explanation = run_with_library(
        &["--explain", "element-predicate"],
        "not valid MiniZinc",
        Some("missing_library"),
    );
    assert!(explanation.status.success() && explanation.stderr.is_empty());
    let explanation = String::from_utf8(explanation.stdout).unwrap();
    for expected in [
        "Family: modelling",
        "Availability: available",
        "Requires model context: yes",
        "Required facts:",
        "Fix support: none",
        "Options: none",
        "Limitations: User lookalikes",
    ] {
        assert!(explanation.contains(expected), "{explanation}");
    }
    for args in [
        vec!["--explain", "unknown-rule"],
        vec!["--explain"],
        vec!["--list-rules", "missing.mzn"],
        vec!["--explain", "naming", "--rules", "all"],
    ] {
        let result = run(&args, "");
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty() && !result.stderr.is_empty());
    }
}

#[test]
fn model_configuration_keeps_syntax_defaults_independent_and_loads_semantic_inputs() {
    let source = "include \"missing_dependency.mzn\"; int: good_name = 1;";
    for arguments in [
        vec![],
        vec!["-I", "missing_first", "-I", "missing_second"],
        vec!["--stdlib-dir", "missing_explicit_library"],
    ] {
        let output = run_with_library(&arguments, source, Some("missing_environment_library"));
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    for selection in ["thesis", "all", "naming,search-coverage"] {
        let output = run_with_library(
            &[
                "--rules",
                selection,
                "--stdlib-dir",
                "missing_explicit_library",
                "missing_input.mzn",
            ],
            "",
            Some("missing_environment_library"),
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("missing_input") && !stderr.contains("unavailable rules"));
    }
    for arguments in [
        vec!["-I"],
        vec!["--stdlib-dir"],
        vec!["--stdlib-dir", "one", "--stdlib-dir", "two"],
    ] {
        assert_eq!(run(&arguments, source).status.code(), Some(2));
    }
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

#[test]
fn directory_inputs_include_hidden_ignored_files_and_continue_after_errors() {
    let root = std::env::temp_dir().join(format!("zincite-lint-tree-{}", std::process::id()));
    let nested = root.join(".hidden");
    let git = root.join(".git");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(&git).unwrap();
    std::fs::write(root.join(".gitignore"), "ignored.mzn\n").unwrap();
    let hidden = nested.join("advice.mzn");
    let ignored = root.join("ignored.mzn");
    let data = nested.join("data.dzn");
    std::fs::write(&hidden, "constraint true;").unwrap();
    std::fs::write(&ignored, "int: BadName = 1;").unwrap();
    std::fs::write(&data, "BAD_TARGET=1;").unwrap();
    std::fs::write(git.join("invalid.mzn"), "int: x = ;").unwrap();
    std::fs::write(root.join("notes.txt"), "int: x = ;").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&root, nested.join("cycle")).unwrap();
    let arguments = [
        root.to_str().unwrap(),
        nested.to_str().unwrap(),
        hidden.to_str().unwrap(),
    ];
    let output = run(&arguments, "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let warnings = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        warnings
            .matches("warning [missing-constraint-label]")
            .count(),
        1
    );
    assert_eq!(warnings.matches("warning [naming]").count(), 1);
    assert!(warnings.find("advice.mzn:").unwrap() < warnings.find("ignored.mzn:").unwrap());
    assert_eq!(run(&arguments, "").stderr, warnings.as_bytes());
    let invalid = nested.join("invalid.mzn");
    std::fs::write(&invalid, "int: x = ;").unwrap();
    let missing = root.join("absent.mzn");
    let output = run(
        &[
            missing.to_str().unwrap(),
            root.to_str().unwrap(),
            invalid.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostics.contains("absent.mzn") && diagnostics.contains("warning [naming]"));
    assert_eq!(
        diagnostics.matches("invalid.mzn:").count(),
        1,
        "{diagnostics}"
    );
    for (path, bytes) in [
        (&hidden, b"constraint true;".as_slice()),
        (&ignored, b"int: BadName = 1;".as_slice()),
        (&data, b"BAD_TARGET=1;".as_slice()),
        (&invalid, b"int: x = ;".as_slice()),
    ] {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rule_selection_preserves_defaults_and_runs_all_catalogue_rules() {
    let source = "int: BadName = 1; constraint true;";
    let default = run(&[], source);
    assert_eq!(default.status.code(), Some(1));
    for selection in ["default", "naming,missing-constraint-label"] {
        let output = run(&["--rules", selection], source);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stderr, default.stderr);
        assert!(output.stdout.is_empty());
    }
    let naming = run(&["--rules", "naming"], source);
    assert_eq!(naming.status.code(), Some(1));
    let diagnostic = String::from_utf8(naming.stderr).unwrap();
    assert!(
        diagnostic.contains("warning [naming]") && !diagnostic.contains("missing-constraint-label")
    );
    let suppressed = run(
        &["--rules", "naming"],
        "% zincite-lint: ignore compact-if\n% zincite-lint: ignore naming\nint: BadName = 1; constraint true;",
    );
    assert_eq!(suppressed.status.code(), Some(0));
    assert!(suppressed.stdout.is_empty() && suppressed.stderr.is_empty());
    for selection in ["thesis", "all", "search-coverage", "naming,search-coverage"] {
        let output = run(&["--rules", selection], "");
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("analysis limitation") && !stderr.contains("unavailable rules"));
    }
    for arguments in [
        vec!["--rules"],
        vec!["--rules", "unknown"],
        vec!["--rules", "naming,"],
        vec!["--rules", "naming", "--rules", "default"],
    ] {
        assert_eq!(run(&arguments, source).status.code(), Some(2));
    }
    let empty = std::env::temp_dir().join(format!("zincite-lint-empty-{}", std::process::id()));
    std::fs::create_dir(&empty).unwrap();
    assert_eq!(
        run(&["--rules", "thesis", empty.to_str().unwrap()], "")
            .status
            .code(),
        Some(0)
    );
    std::fs::remove_dir(empty).unwrap();
}

#[test]
fn capture_selection_loads_model_roots_and_keeps_stdin_data_and_error_precedence_honest() {
    let directory =
        std::env::temp_dir().join(format!("zincite-lint-captures-{}", std::process::id()));
    let library = directory.join("library");
    let includes = directory.join("includes");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::create_dir_all(&includes).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "").unwrap();
    let root = directory.join("root.mzn");
    let shared = includes.join("shared.mzn");
    let source = "include \"shared.mzn\"; var int: global; solve satisfy;";
    let included = "function var int: capture() = global;\n% zincite-lint: ignore global-variable-in-function\nfunction var int: suppressed() = global;";
    std::fs::write(&root, source).unwrap();
    std::fs::write(&shared, included).unwrap();
    let arguments = [
        "--rules",
        "global-variable-in-function",
        "-I",
        includes.to_str().unwrap(),
        "--stdlib-dir",
        library.to_str().unwrap(),
        root.to_str().unwrap(),
    ];
    let output = run_with_library(&arguments, "", Some("missing_environment_library"));
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(&format!(
            "{}:1:31: bytes 30..36: warning [global-variable-in-function]",
            shared.display()
        )),
        "{stderr}"
    );
    assert_eq!(
        stderr
            .matches("warning [global-variable-in-function]")
            .count(),
        1
    );
    assert!(!stderr.contains("analysis limitation"));
    let capture_stderr = stderr.clone();
    assert!(run(&[root.to_str().unwrap()], "").status.success());
    let failed = directory.join("failed.mzn");
    std::fs::write(&failed, "include \"missing.mzn\";").unwrap();
    let mut independent = arguments.to_vec();
    independent.push(failed.to_str().unwrap());
    let output = run(&independent, "");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("warning [global-variable-in-function]")
            && stderr.contains("cannot resolve include")
    );
    let stdin = run(
        &["--rules", "global-variable-in-function"],
        "var int: global; function var int: capture() = global;",
    );
    assert_eq!(stdin.status.code(), Some(0));
    assert!(stdin.stdout.is_empty());
    assert!(
        String::from_utf8(stdin.stderr)
            .unwrap()
            .contains("requires a ModelContext")
    );
    let data = run(
        &[
            "--rules",
            "global-variable-in-function",
            "--stdin-filepath",
            "data.dzn",
        ],
        "global=1;",
    );
    assert_eq!(data.status.code(), Some(0));
    assert!(data.stdout.is_empty() && data.stderr.is_empty());
    for rule in zincite_lint::Rule::THESIS
        .into_iter()
        .filter(|rule| !rule.is_available())
    {
        let output = run(&["--rules", rule.id(), "missing_input.mzn"], "");
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("unavailable rules") && !stderr.contains("missing_input"));
    }
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    assert_eq!(std::fs::read_to_string(&shared).unwrap(), included);
    // The selected root's settings govern its closure, even where an included
    // file has a nearer configuration of its own.
    std::fs::write(
        directory.join("zincite.toml"),
        "[lint]\nselect=['global-variable-in-function']",
    )
    .unwrap();
    std::fs::write(includes.join("zincite.toml"), "[lint]\nselect=[]").unwrap();
    let configured = run(&arguments[2..], "");
    assert_eq!(configured.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(configured.stderr).unwrap(),
        capture_stderr
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn element_selection_reports_advice_limits_and_independent_root_errors() {
    let directory =
        std::env::temp_dir().join(format!("zincite-lint-element-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "").unwrap();
    std::fs::write(
        library.join("std/element.mzn"),
        "predicate element(var $$E: i,array[$$E] of var $$T: x,var $$T: y)=y=x[i];",
    )
    .unwrap();
    let root = directory.join("root.mzn");
    let source = "include \"element.mzn\"; array[int] of var int: xs; var int: value; constraint element(1,xs,value);";
    std::fs::write(&root, source).unwrap();
    let arguments = [
        "--rules",
        "element-predicate",
        "--stdlib-dir",
        library.to_str().unwrap(),
        root.to_str().unwrap(),
    ];
    let warning = run(&arguments, "");
    assert_eq!(warning.status.code(), Some(1));
    assert!(warning.stdout.is_empty());
    let stderr = String::from_utf8(warning.stderr).unwrap();
    let start = source.find("element(1").unwrap();
    assert!(
        stderr.contains(&format!(
            "{}:1:{}: bytes {}..{}: warning [element-predicate]",
            root.display(),
            start + 1,
            start,
            start + 7
        )),
        "{stderr}"
    );
    assert!(!stderr.contains("analysis limitation"));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    let failed = directory.join("failed.mzn");
    std::fs::write(&failed, "include \"missing.mzn\";").unwrap();
    let mut independent = arguments.to_vec();
    independent.push(failed.to_str().unwrap());
    let failure = run(&independent, "");
    assert_eq!(failure.status.code(), Some(2));
    let stderr = String::from_utf8(failure.stderr).unwrap();
    assert!(
        stderr.contains("warning [element-predicate]") && stderr.contains("cannot resolve include")
    );
    std::fs::write(
        &root,
        "predicate element(int: x)=true; constraint element(1);",
    )
    .unwrap();
    let clean = run(&arguments, "");
    assert_eq!(clean.status.code(), Some(0));
    assert!(clean.stdout.is_empty() && clean.stderr.is_empty());
    std::fs::write(&root, "constraint missing(1);").unwrap();
    let limited = run(&arguments, "");
    assert_eq!(limited.status.code(), Some(0));
    assert!(
        String::from_utf8(limited.stderr)
            .unwrap()
            .contains("analysis limitation: element-predicate: callable name is unresolved")
    );
    let stdin = run(&["--rules", "element-predicate"], source);
    assert_eq!(stdin.status.code(), Some(0));
    assert!(
        String::from_utf8(stdin.stderr)
            .unwrap()
            .contains("requires a ModelContext")
    );
    let data = run(
        &[
            "--rules",
            "element-predicate",
            "--stdin-filepath",
            "data.dzn",
        ],
        "x=1;",
    );
    assert_eq!(data.status.code(), Some(0));
    assert!(data.stderr.is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn decision_use_selection_preserves_cli_status_context_and_independent_inputs() {
    let directory =
        std::env::temp_dir().join(format!("zincite-lint-decision-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "").unwrap();
    let root = directory.join("root.mzn");
    let source = "var bool: choice; var 1..3: value; array[int] of var opt int: xs=[i|i in 1..value where choice]; constraint if choice then value^2>0 else true endif; solve satisfy;";
    std::fs::write(&root, source).unwrap();
    let rules =
        "decision-variable-operator,decision-variable-generator,decision-variable-condition";
    let args = [
        "--rules",
        rules,
        "--stdlib-dir",
        library.to_str().unwrap(),
        root.to_str().unwrap(),
    ];
    let warning = run(&args, "");
    assert_eq!(warning.status.code(), Some(1));
    assert!(warning.stdout.is_empty());
    let stderr = String::from_utf8(warning.stderr).unwrap();
    for id in rules.split(',') {
        assert!(stderr.contains(&format!("warning [{id}]")), "{stderr}");
    }
    assert!(!stderr.contains("analysis limitation"));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    let failed = directory.join("failed.mzn");
    std::fs::write(&failed, "include \"missing.mzn\";").unwrap();
    let mut independent = args.to_vec();
    independent.push(failed.to_str().unwrap());
    let failed = run(&independent, "");
    assert_eq!(failed.status.code(), Some(2));
    let stderr = String::from_utf8(failed.stderr).unwrap();
    assert!(
        stderr.contains("warning [decision-variable-operator]")
            && stderr.contains("cannot resolve include")
    );
    std::fs::write(&root,"int: limit; array[int] of int: xs=[i|i in 1..limit where true]; constraint if true then limit^2>0 else false endif; solve satisfy;").unwrap();
    let quiet = run(&args, "");
    assert_eq!(quiet.status.code(), Some(0));
    assert!(quiet.stdout.is_empty() && quiet.stderr.is_empty());
    std::fs::write(&root, "constraint if missing() then true else false endif;").unwrap();
    let limited = run(&args, "");
    assert_eq!(limited.status.code(), Some(0));
    assert!(
        String::from_utf8(limited.stderr)
            .unwrap()
            .contains("analysis limitation: decision-variable-condition")
    );
    let stdin = run(&["--rules", rules], source);
    assert_eq!(stdin.status.code(), Some(0));
    assert!(
        String::from_utf8(stdin.stderr)
            .unwrap()
            .contains("requires a ModelContext")
    );
    let data = run(
        &["--rules", rules, "--stdin-filepath", "data.dzn"],
        "value=1;",
    );
    assert_eq!(data.status.code(), Some(0));
    assert!(data.stderr.is_empty());
    let parsed = zincite_syntax::parse(source);
    assert!(
        zincite_lint::lint_with_options(
            &parsed,
            &zincite_lint::LintOptions::from_selection(rules).unwrap()
        )
        .unwrap_err()[0]
            .message
            .contains("ModelContext")
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn numeric_index_advice_uses_model_selection_and_item_suppression() {
    let directory = std::env::temp_dir().join(format!("zincite-index-cli-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("std")).unwrap();
    std::fs::write(directory.join("std/stdlib.mzn"), "").unwrap();
    let root = directory.join("root.mzn");
    let args = [
        "--rules",
        "array-index-start",
        "--stdlib-dir",
        directory.to_str().unwrap(),
        root.to_str().unwrap(),
    ];
    std::fs::write(
        &root,
        "set of int: Index={2,4}; array[Index] of int: values; solve satisfy;",
    )
    .unwrap();
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("warning [array-index-start]")
    );
    std::fs::write(
        &root,
        "% zincite-lint: ignore array-index-start\narray[0..4] of int: values; solve satisfy;",
    )
    .unwrap();
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    std::fs::write(&root, "array[0..3 of int: values; solve satisfy;").unwrap();
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(2));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn constant_variable_selection_keeps_context_limits_and_status_precedence() {
    let directory =
        std::env::temp_dir().join(format!("zincite-constant-cli-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("library/std")).unwrap();
    std::fs::write(directory.join("library/std/stdlib.mzn"), "").unwrap();
    let model = directory.join("root.mzn");
    let bad = directory.join("bad.mzn");
    let data = directory.join("values.dzn");
    let library = directory.join("library");
    let original = "var int: value=2; solve satisfy;";
    std::fs::write(&model, original).unwrap();
    std::fs::write(&bad, "include \"missing.mzn\";").unwrap();
    std::fs::write(&data, "value=2;").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
        .args(["--rules", "constant-variable", "--stdlib-dir"])
        .arg(&library)
        .arg(&model)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("warning [constant-variable]"));
    assert!(output.stdout.is_empty());
    let failed = Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
        .args(["--rules", "constant-variable", "--stdlib-dir"])
        .arg(&library)
        .args([&bad, &model])
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("warning [constant-variable]"));
    let inapplicable = Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
        .args(["--rules", "constant-variable"])
        .arg(&data)
        .output()
        .unwrap();
    assert_eq!(inapplicable.status.code(), Some(0));
    assert!(inapplicable.stdout.is_empty() && inapplicable.stderr.is_empty());
    let stdin = run(&["--rules", "constant-variable"], original);
    assert_eq!(stdin.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&stdin.stderr).contains("requires a ModelContext"));
    std::fs::write(&model, "var opt int: value=<>; solve satisfy;").unwrap();
    let limited = Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
        .args(["--rules", "constant-variable", "--stdlib-dir"])
        .arg(&library)
        .arg(&model)
        .output()
        .unwrap();
    assert_eq!(limited.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&limited.stderr).contains("analysis limitation:"));
    assert_eq!(
        std::fs::read_to_string(&model).unwrap(),
        "var opt int: value=<>; solve satisfy;"
    );
    assert!(
        zincite_lint::lint_with_options(
            &zincite_syntax::parse(original),
            &zincite_lint::LintOptions::from_selection("constant-variable").unwrap()
        )
        .unwrap_err()[0]
            .message
            .contains("ModelContext")
    );
    assert_eq!(
        zincite_lint::Rule::THESIS
            .iter()
            .filter(|r| !r.is_available())
            .count(),
        0
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unbounded_selection_preserves_domains_definitions_suppression_and_errors() {
    let directory =
        std::env::temp_dir().join(format!("zincite-unbounded-cli-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "").unwrap();
    let root = directory.join("root.mzn");
    let original = "type Real=var float; Real: value; var 0..3: bounded; var int: defined; constraint defined=bounded; solve satisfy;";
    std::fs::write(&root, original).unwrap();
    let args = [
        "--rules",
        "unbounded-variable",
        "--stdlib-dir",
        library.to_str().unwrap(),
        root.to_str().unwrap(),
    ];
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        diagnostics.matches("warning [unbounded-variable]").count(),
        1
    );
    assert!(!diagnostics.contains("analysis limitation"));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), original);
    std::fs::write(&root,"% zincite-lint: ignore unbounded-variable\nvar int: suppressed; var float: initialized=2.0; solve satisfy;").unwrap();
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let failed = directory.join("failed.mzn");
    std::fs::write(&failed, "var int: broken = ;").unwrap();
    std::fs::write(&root, original).unwrap();
    let mut independent = args.to_vec();
    independent.push(failed.to_str().unwrap());
    let output = run(&independent, "");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("warning [unbounded-variable]")
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn compact_if_selection_keeps_advice_limits_and_suppression_separate() {
    let directory =
        std::env::temp_dir().join(format!("zincite-compact-cli-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "").unwrap();
    let root = directory.join("root.mzn");
    let source = "var bool: choice; var int: value; var int: first=if choice then value else 0 endif; var int: second=if choice then 0 else value endif; solve satisfy;";
    std::fs::write(&root, source).unwrap();
    let args = [
        "--rules",
        "compact-if",
        "--stdlib-dir",
        library.to_str().unwrap(),
        root.to_str().unwrap(),
    ];
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert_eq!(diagnostics.matches("warning [compact-if]").count(), 2);
    assert!(!diagnostics.contains("analysis limitation"));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    std::fs::write(&root,"var bool: choice; var int: value; % section\n% zincite-lint: ignore compact-if\nvar int: suppressed=if choice then value else 0 endif; solve satisfy;").unwrap();
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    std::fs::write(
        &root,
        "var bool: choice; var int: partial=if choice then 1 div 0 else 0 endif; solve satisfy;",
    )
    .unwrap();
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(0));
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostics.contains("analysis limitation: compact-if"));
    assert!(!diagnostics.contains("warning [compact-if]"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn effective_zero_one_selection_preserves_facts_limits_and_status_precedence() {
    let directory =
        std::env::temp_dir().join(format!("zincite-zero-one-cli-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"),"function var bool: '='(any $T: left,any $T: right); function var bool: '->'(var bool: left,var bool: right); function var int: sum(array[int] of var int: values);").unwrap();
    let root = directory.join("root.mzn");
    let source = "var 0..1: a; var 0..1: b; array[1..2] of var 0..1: flags; constraint a=1 -> b=1; constraint a=0 -> b=0; var int: total=sum(i in 1..2)(flags[i]=1); solve satisfy;";
    std::fs::write(&root, source).unwrap();
    let args = [
        "--rules",
        "effective-zero-one",
        "--stdlib-dir",
        library.to_str().unwrap(),
        root.to_str().unwrap(),
    ];
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.matches("warning [effective-zero-one]").count(), 3);
    assert!(!stderr.contains("analysis limitation"));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    let broken = directory.join("broken.mzn");
    std::fs::write(&broken, "var int: =;").unwrap();
    let output = run(
        &[
            "--rules",
            "effective-zero-one",
            "--stdlib-dir",
            library.to_str().unwrap(),
            broken.to_str().unwrap(),
            root.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("warning [effective-zero-one]")
    );
    std::fs::write(&root,"var 0..1: a; var 0..1: b; function int: opaque()=1; constraint a=opaque() -> b=1; solve satisfy;").unwrap();
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(0));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("analysis limitation: effective-zero-one")
    );
    let output = run(&["--rules", "effective-zero-one"], source);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("analysis limitation")
    );
    let data = directory.join("values.dzn");
    std::fs::write(&data, "value=1;").unwrap();
    let output = run(
        &["--rules", "effective-zero-one", data.to_str().unwrap()],
        "",
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unused_declaration_selection_respects_complete_roots_limits_and_status_precedence() {
    let dir = std::env::temp_dir().join(format!("zincite-unused-cli-{}", std::process::id()));
    let library = dir.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(
        library.join("std/stdlib.mzn"),
        "function string: show(any $T: value);",
    )
    .unwrap();
    let root = dir.join("root.mzn");
    let original = "int: kept=1; int: unused=2; solve satisfy; output [show(kept)];";
    std::fs::write(&root, original).unwrap();
    let args = [
        "--rules",
        "unused-declaration",
        "--stdlib-dir",
        library.to_str().unwrap(),
        root.to_str().unwrap(),
    ];
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.matches("warning [unused-declaration]").count(), 1);
    assert!(stderr.contains("'unused'") && !stderr.contains("analysis limitation"));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), original);
    assert!(
        zincite_lint::lint_with_options(
            &zincite_syntax::parse(original),
            &zincite_lint::LintOptions::from_selection("unused-declaration").unwrap()
        )
        .unwrap_err()[0]
            .message
            .contains("ModelContext")
    );
    std::fs::write(&root, "int: exported=2;").unwrap();
    let fragment = run(&args, "");
    assert_eq!(fragment.status.code(), Some(0));
    assert!(fragment.stderr.is_empty());
    std::fs::write(
        &root,
        "int: unused=2; constraint missing; solve satisfy; output [];",
    )
    .unwrap();
    let uncertain = run(&args, "");
    assert_eq!(uncertain.status.code(), Some(0));
    let stderr = String::from_utf8(uncertain.stderr).unwrap();
    assert!(
        stderr.contains("analysis limitation: unused-declaration")
            && !stderr.contains("warning [unused-declaration]")
    );
    std::fs::write(&root, original).unwrap();
    let broken = dir.join("broken.mzn");
    std::fs::write(&broken, "var int: =;").unwrap();
    let mixed = run(
        &[
            "--rules",
            "unused-declaration",
            "--stdlib-dir",
            library.to_str().unwrap(),
            broken.to_str().unwrap(),
            root.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(mixed.status.code(), Some(2));
    assert!(
        String::from_utf8(mixed.stderr)
            .unwrap()
            .contains("warning [unused-declaration]")
    );
    let stdin = run(&["--rules", "unused-declaration"], original);
    assert_eq!(stdin.status.code(), Some(0));
    assert!(
        String::from_utf8(stdin.stderr)
            .unwrap()
            .contains("analysis limitation")
    );
    let data = dir.join("data.dzn");
    std::fs::write(&data, "value=1;").unwrap();
    let data_output = run(
        &["--rules", "unused-declaration", data.to_str().unwrap()],
        "",
    );
    assert_eq!(data_output.status.code(), Some(0));
    assert!(data_output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn search_selection_keeps_completion_suppression_and_status_precedence() {
    let directory = std::env::temp_dir().join(format!("zincite-search-cli-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), concat!(
        "annotation input_order; annotation indomain_min; annotation complete;\n",
        "annotation int_search(array[int] of var int: x,ann: select,ann: choice,ann: explore);\n"
    )).unwrap();
    let root = directory.join("root.mzn");
    let source = "var 0..9: seed; var 0..9: missing; solve :: int_search([seed],input_order,indomain_min,complete) satisfy;";
    std::fs::write(&root, source).unwrap();
    let args = [
        "--rules",
        "search-coverage",
        "--stdlib-dir",
        library.to_str().unwrap(),
        root.to_str().unwrap(),
    ];
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert_eq!(diagnostics.matches("warning [search-coverage]").count(), 1);
    assert!(
        diagnostics.contains("bytes 26..33:") && diagnostics.contains("'missing'"),
        "{diagnostics}"
    );
    assert_eq!(diagnostics.matches("analysis limitation:").count(), 0);
    for selection in ["thesis", "all"] {
        let mut selected = args;
        selected[1] = selection;
        let output = run(&selected, "");
        assert_eq!(output.status.code(), Some(1));
        let diagnostics = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostics.contains("warning [search-coverage]"));
        assert!(
            !diagnostics.contains("direct definitions only")
                && !diagnostics.contains("unavailable rules")
        );
    }
    std::fs::write(&root, "var 0..9: seed;\n% zincite-lint: ignore search-coverage\nvar 0..9: missing; solve :: int_search([seed],input_order,indomain_min,complete) satisfy;").unwrap();
    let output = run(&args, "");
    assert_eq!(output.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("warning ["));
    std::fs::write(&root, source).unwrap();
    let broken = directory.join("broken.mzn");
    std::fs::write(&broken, "var int: broken=;").unwrap();
    let mut independent = args.to_vec();
    independent.push(broken.to_str().unwrap());
    let output = run(&independent, "");
    assert_eq!(output.status.code(), Some(2));
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostics.contains("warning [search-coverage]") && diagnostics.contains("error:"));
    let stdin = run(&["--rules", "search-coverage"], source);
    assert_eq!(stdin.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&stdin.stderr).contains("requires a ModelContext"));
    let data = run(
        &["--rules", "search-coverage", "--stdin-filepath", "data.dzn"],
        "seed=2;",
    );
    assert_eq!(data.status.code(), Some(0));
    assert!(data.stdout.is_empty() && data.stderr.is_empty());
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn nearest_settings_cli_replacement_and_stdin_anchors_share_one_preflight() {
    let dir = std::env::temp_dir().join(format!("zincite-settings-{}", std::process::id()));
    let nested = dir.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let parent_config = dir.join("zincite.toml");
    let nested_config = nested.join("zincite.toml");
    std::fs::write(&parent_config, "[lint]\nselect=['naming']").unwrap();
    std::fs::write(
        &nested_config,
        "[lint]\nselect=['missing-constraint-label']\nextend-select=['naming']\nignore=['naming']",
    )
    .unwrap();
    let source = "int: BadName=1; constraint true;";
    let parent = dir.join("parent.mzn");
    let child = nested.join("child.mzn");
    std::fs::write(&parent, source).unwrap();
    std::fs::write(&child, source).unwrap();
    let parent_path = parent.to_str().unwrap();
    let child_path = child.to_str().unwrap();
    let output = run(&[parent_path, child_path], "");
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(text.matches("warning [naming]").count(), 1);
    assert_eq!(
        text.matches("warning [missing-constraint-label]").count(),
        1
    );
    for arguments in [
        vec!["--rules", "naming", child_path],
        vec!["--config", parent_config.to_str().unwrap(), child_path],
    ] {
        let text = String::from_utf8(run(&arguments, "").stderr).unwrap();
        assert!(text.contains("warning [naming]"));
        assert!(!text.contains("missing-constraint-label"));
    }
    let isolated = String::from_utf8(run(&["--isolated", child_path], "").stderr).unwrap();
    assert!(
        isolated.contains("warning [naming]")
            && isolated.contains("warning [missing-constraint-label]")
    );
    let unconfigured = dir.join("unconfigured");
    std::fs::create_dir_all(unconfigured.join("unrelated")).unwrap();
    std::fs::write(
        unconfigured.join("unrelated/zincite.toml"),
        "[lint]\nselect=[]",
    )
    .unwrap();
    std::fs::write(unconfigured.join("model.mzn"), source).unwrap();
    let relative_parent = unconfigured.join("unrelated/../model.mzn");
    let output = run(&[relative_parent.to_str().unwrap()], "");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("warning [naming]")
    );
    let unsaved = nested.join("unsaved.mzn");
    let anchored = run(&["--stdin-filepath", unsaved.to_str().unwrap()], source);
    assert_eq!(anchored.status.code(), Some(1));
    let text = String::from_utf8(anchored.stderr).unwrap();
    assert!(
        text.contains("warning [missing-constraint-label]") && !text.contains("warning [naming]")
    );
    let plain = run(&[], source);
    assert!(
        String::from_utf8(plain.stderr)
            .unwrap()
            .contains("warning [naming]")
    );
    let explicit_stdin = run(&["--config", parent_config.to_str().unwrap()], source);
    assert!(
        String::from_utf8(explicit_stdin.stderr)
            .unwrap()
            .contains("warning [naming]")
    );
    let shown = run(
        &[
            "--show-settings",
            "--stdin-filepath",
            unsaved.to_str().unwrap(),
        ],
        "not valid MiniZinc",
    );
    assert!(shown.status.success() && shown.stderr.is_empty());
    assert_eq!(
        String::from_utf8(shown.stdout).unwrap(),
        format!(
            "Root: {}\nSettings: {}\nRules: missing-constraint-label\n",
            unsaved.display(),
            nested_config.canonicalize().unwrap().display()
        )
    );
    std::fs::write(&child, "include \"missing.mzn\"; not valid MiniZinc").unwrap();
    let shown = run(&["--show-settings", "--rules", "thesis", child_path], "");
    assert!(shown.status.success() && shown.stderr.is_empty());
    let shown = String::from_utf8(shown.stdout).unwrap();
    assert!(shown.contains("array-index-start") && shown.contains("decision-variable-condition"));
    // Plain library calls use explicit options regardless of nearby files.
    let parsed = zincite_syntax::parse(source);
    assert_eq!(zincite_lint::lint(&parsed).unwrap().len(), 2);
    assert_eq!(std::fs::read_to_string(&parent).unwrap(), source);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_configuration_preflights_all_roots_and_empty_input_directories() {
    let dir = std::env::temp_dir().join(format!("zincite-settings-errors-{}", std::process::id()));
    let nested = dir.join("nested");
    let empty = dir.join("empty");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir_all(&empty).unwrap();
    let config = nested.join("zincite.toml");
    let parent = dir.join("parent.mzn");
    let child = nested.join("child.mzn");
    std::fs::write(dir.join("zincite.toml"), "[lint]\nselect=['naming']").unwrap();
    std::fs::write(&parent, "int: BadName=1;").unwrap();
    std::fs::write(&child, "constraint true;").unwrap();
    for (settings, field) in [
        ("[other]", "other"),
        ("[lint]\nunknown=true", "lint.unknown"),
        ("[lint]\nselect=1", "lint.select"),
        ("[lint]\nselect=['unknown']", "lint.select"),
        ("[lint]\nignore=['family:unknown']", "lint.ignore"),
        ("[lint]\nselect=[", "TOML"),
    ] {
        std::fs::write(&config, settings).unwrap();
        for args in [
            vec![parent.to_str().unwrap(), child.to_str().unwrap()],
            vec![
                "--show-settings",
                parent.to_str().unwrap(),
                child.to_str().unwrap(),
            ],
            vec!["--rules", "naming", child.to_str().unwrap()],
            vec![
                "--config",
                config.to_str().unwrap(),
                empty.to_str().unwrap(),
            ],
        ] {
            let output = run(&args, "");
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            let error = String::from_utf8(output.stderr).unwrap();
            assert!(
                error.contains(config.to_str().unwrap()) && error.contains(field),
                "{error}"
            );
            assert!(!error.contains("warning ["));
        }
    }
    for args in [
        vec!["--config", config.to_str().unwrap(), "--isolated"],
        vec!["--isolated", "--config", config.to_str().unwrap()],
        vec!["--rules", "family:unknown", empty.to_str().unwrap()],
    ] {
        assert_eq!(run(&args, "").status.code(), Some(2));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
