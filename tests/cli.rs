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
fn version_reports_the_package_and_available_build_revision() {
    let output = run(&["--version"], "");
    assert!(output.status.success() && output.stderr.is_empty());
    let expected = match option_env!("ZINCITE_BUILD_REVISION") {
        Some(revision) => format!("zincite {} ({revision})\n", env!("CARGO_PKG_VERSION")),
        None => format!("zincite {}\n", env!("CARGO_PKG_VERSION")),
    };
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn missing_library_dependencies_suggest_explicit_setup() {
    let directory =
        std::env::temp_dir().join(format!("zincite-root-library-hint-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let model = directory.join("model.mzn");
    let library = directory.join("library");
    std::fs::write(&model, "include \"globals.mzn\"; solve satisfy;\n").unwrap();

    for configured in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_zincite"));
        command
            .args(["lint", "--isolated"])
            .env_remove("MZN_STDLIB_DIR");
        if configured {
            command.arg("--stdlib-dir").arg(&library);
        }
        let output = command.arg(&model).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let diagnostics = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostics.contains("cannot resolve include 'globals.mzn'"));
        assert!(diagnostics.contains("--stdlib-dir DIR or MZN_STDLIB_DIR"));
        assert!(diagnostics.contains("containing std/stdlib.mzn"));
        if cfg!(target_os = "macos") {
            assert!(
                diagnostics
                    .contains("/Applications/MiniZincIDE.app/Contents/Resources/share/minizinc")
            );
        }
    }

    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "").unwrap();
    std::fs::write(library.join("std/globals.mzn"), "").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_zincite"))
        .args(["lint", "--isolated", "--stdlib-dir"])
        .arg(&library)
        .arg(&model)
        .env("MZN_STDLIB_DIR", directory.join("wrong-library"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        !String::from_utf8(output.stderr)
            .unwrap()
            .contains("--stdlib-dir")
    );
    std::fs::remove_dir_all(directory).unwrap();
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
        assert!(help.contains("fmt") && help.contains("lint") && help.contains("query"));
    }
    for command in ["fmt", "lint", "query"] {
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
    let unknown = run(&["unknown"], "");
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    assert!(
        String::from_utf8(unknown.stderr)
            .unwrap()
            .contains("unknown command 'unknown'")
    );
}

#[test]
fn query_selects_data_and_reports_errors_without_partial_stdout() {
    let source = "% attached\r\n'capacity' = 2; % tail\r\na = 1;\r\n";
    let output = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "filter(name(\"capacity\"))",
        ],
        source,
    );
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, b"% attached\r\n'capacity' = 2; % tail\r\n");
    let output = run(&["query", "items | head(1) | count", "-"], source);
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, b"1\n");
    let output = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "filter(name(\"capacity\")) | set_value(\"4\")",
        ],
        source,
    );
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        b"% attached\r\n'capacity' = 4; % tail\r\na = 1;\r\n"
    );
    let output = run(
        &[
            "query",
            "--diff",
            "--stdin-filepath",
            "data.dzn",
            "head(1) | remove",
        ],
        source,
    );
    assert!(output.status.success() && output.stderr.is_empty());
    assert!(output.stdout.starts_with(b"--- a/data.dzn\n"));
    let output = run(&["query", "count | emit"], source);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("<query>:1:")
    );
}

#[test]
fn query_inspection_reports_and_errors_share_root_command_behavior() {
    let output = run(
        &["query", "expressions | call_names | tally | json"],
        "constraint f(g(1), g(2));",
    );
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, b"{\"f\":1,\"g\":2}\n");
    let output = run(&["query", "names | children"], "constraint true;");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn query_subcommand_shares_semantic_context_stdin_and_status_contracts() {
    let directory =
        std::env::temp_dir().join(format!("zincite-root-semantic-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "").unwrap();
    let root = directory.join("root.mzn");
    let source =
        "\u{feff}include \"included.mzn\";\r\nint: value = imported;\r\nsolve satisfy;\r\n";
    std::fs::write(&root, source).unwrap();
    std::fs::write(
        directory.join("included.mzn"),
        "\u{feff}int: imported=2;\r\n",
    )
    .unwrap();
    let expression = "filter(name(\"value\")) | references | declarations | instantiations | json";
    let output = run(
        &[
            "query",
            "--stdlib-dir",
            library.to_str().unwrap(),
            expression,
            root.to_str().unwrap(),
        ],
        "",
    );
    assert!(output.status.success() && output.stderr.is_empty());
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(text.contains("\"complete\":true") && text.contains("\"instantiation\":\"par\""));
    let arguments = [
        "query",
        "--model",
        root.to_str().unwrap(),
        "--stdlib-dir",
        library.to_str().unwrap(),
        "--stdin-filepath",
        "label-only.mzn",
        expression,
    ];
    let stdin = run(&arguments, source);
    assert!(stdin.status.success() && stdin.stderr.is_empty());
    assert_eq!(stdin.stdout, output.stdout);
    let mismatch = run(&arguments, source.trim_start_matches('\u{feff}'));
    assert_eq!(mismatch.status.code(), Some(2));
    assert!(mismatch.stdout.is_empty());
    let unknown = directory.join("unknown.mzn");
    std::fs::write(&unknown, "function $T: identity($T:x)=x; solve satisfy;").unwrap();
    let incomplete = run(
        &[
            "query",
            "--stdlib-dir",
            library.to_str().unwrap(),
            "declarations | filter(not type(\"int\")) | names | count | json",
            unknown.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(incomplete.status.code(), Some(1));
    let text = String::from_utf8(incomplete.stdout).unwrap();
    assert!(text.contains("\"complete\":false") && text.contains("\"result\":0"));
    assert!(
        String::from_utf8(incomplete.stderr)
            .unwrap()
            .contains("incomplete")
    );
    let missing = directory.join("missing.mzn");
    std::fs::write(&missing, "include \"absent.mzn\"; solve satisfy;").unwrap();
    let error = run(
        &[
            "query",
            "references | count | json",
            missing.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(error.status.code(), Some(2));
    assert!(error.stdout.is_empty());
    let syntax = run(
        &[
            "query",
            "--model",
            directory.join("not-present.mzn").to_str().unwrap(),
            "count",
            root.to_str().unwrap(),
        ],
        "",
    );
    assert!(syntax.status.success() && syntax.stderr.is_empty());
    assert_eq!(syntax.stdout, b"3\n");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn default_lint_previews_and_applies_direct_comprehensions() {
    let dir =
        std::env::temp_dir().join(format!("zincite-root-comprehension-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "function set of int: '..'(int:a,int:b); function array[int] of int: array1d(set of int:s,array[int] of int:a);").unwrap();
    let source = "array[int] of int:values=array1d(1..3,[i | i in 1..3]); solve satisfy;\n";
    let root = dir.join("root.mzn");
    let library = dir.join("library");
    std::fs::write(&root, source).unwrap();
    let args = [
        "lint",
        "--isolated",
        "--stdlib-dir",
        library.to_str().unwrap(),
    ];
    let preview = run(
        &[args.as_slice(), &["--diff", root.to_str().unwrap()]].concat(),
        "",
    );
    assert_eq!(preview.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&preview.stdout).contains("values=[i | i in 1..3]"));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    let fixed = run(
        &[args.as_slice(), &["--fix", root.to_str().unwrap()]].concat(),
        "",
    );
    assert_eq!(
        fixed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&fixed.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&root).unwrap(),
        source.replace("array1d(1..3,[i | i in 1..3])", "[i | i in 1..3]")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn prefer_enum_is_advisory_through_the_root_command_and_never_edits() {
    let dir = std::env::temp_dir().join(format!("zincite-root-prefer-enum-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        "function set of int:'..'(int:a,int:b); function var bool:'='(var $T:a,var $T:b);",
    )
    .unwrap();
    let source =
        "set of int: People=1..3; var People:p; constraint p=p; solve satisfy; output [];\r\n";
    let path = dir.join("model.mzn");
    std::fs::write(&path, source).unwrap();
    for selection in ["prefer-enum", "family:style", "all"] {
        for mode in ["--fix", "--diff"] {
            let output = Command::new(env!("CARGO_BIN_EXE_zincite"))
                .args([
                    "lint",
                    "--isolated",
                    "--stdlib-dir",
                    dir.join("library").to_str().unwrap(),
                    "--rules",
                    selection,
                    mode,
                ])
                .arg(&path)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(1),
                "{:?}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stderr).contains("[prefer-enum]"));
            assert!(output.stdout.is_empty());
            assert_eq!(std::fs::read(&path).unwrap(), source.as_bytes());
        }
    }
    let explanation = run(&["lint", "--explain", "prefer-enum"], "invalid");
    assert!(explanation.status.success() && explanation.stderr.is_empty());
    assert!(String::from_utf8_lossy(&explanation.stdout).contains("Fix support: none"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn program_files_report_native_results_and_buffer_later_failures() {
    let path = std::env::temp_dir().join(format!("zincite-root-program-{}.zq", std::process::id()));
    std::fs::write(&path, r#"source selected = query(""); source n = selected | query("count"); source h = selected | query("names | tally"); report (count: n, histogram: h, selected: selected);"#).unwrap();
    let output = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "-f",
            path.to_str().unwrap(),
        ],
        "a = 1; b = 2;",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("\"complete\":true"));
    assert!(report.contains("\"count\":2"));
    assert!(report.contains("\"histogram\":{\"a\":1,\"b\":1}"));
    assert!(report.contains("\"selected\":["));
    std::fs::write(&path, r#"query("count | json"); query("values");"#).unwrap();
    let output = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "--program-file",
            path.to_str().unwrap(),
        ],
        "a = 1 + 2;",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("literal"), "{diagnostic}");
    assert!(diagnostic.contains("data.dzn"), "{diagnostic}");
    std::fs::write(&path, r#"query("count | json"); query("count | items");"#).unwrap();
    let output = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "-f",
            path.to_str().unwrap(),
        ],
        "a = 1; b = 2;",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains(path.to_str().unwrap()));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn positional_program_files_share_order_and_buffer_value_failures() {
    let path =
        std::env::temp_dir().join(format!("zincite-root-positional-{}.zq", std::process::id()));
    std::fs::write(
        &path,
        r#"source labels = query("names");
source numbers = literal("zeta");
source n = query("count");
let rows = [(name: label, pair: (i,j), value: numbers[i] + k, total: n)
  | i,j in index_set(numbers), label in [labels[i]], k = i+(i,j).2,
    _ in 1..1 where j <= i];
report rows;"#,
    )
    .unwrap();
    let args = [
        "query",
        "--stdin-filepath",
        "data.dzn",
        "-f",
        path.to_str().unwrap(),
    ];
    let output = run(&args, "zeta = [3, 5]; alpha = 2;");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("\"kind\":\"string\",\"value\":\"zeta\""));
    assert!(report.contains("\"kind\":\"integer\",\"value\":9"));
    assert!(report.find("zeta").unwrap() < report.find("alpha").unwrap());
    for source in [
        "report 7; report 1 div 0;",
        "report if true then 1 else sum([1]) endif;",
    ] {
        std::fs::write(&path, source).unwrap();
        let output = run(&args, "a = 1;");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(path.to_str().unwrap()));
    }
    std::fs::write(&path, "report [i | i in 1..1000 where false];").unwrap();
    let output = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "--max-results",
            "50",
            "-f",
            path.to_str().unwrap(),
        ],
        "a = 1;",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    std::fs::write(&path, "report [i | i in 1..20];").unwrap();
    let output = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "--max-results",
            "200",
            "-f",
            path.to_str().unwrap(),
        ],
        "a = 1;",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    std::fs::remove_file(path).unwrap();
}
