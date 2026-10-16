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

    std::fs::write(&config, "[lint]\nselect=['preset:review']\nignore=['naming']\n[lint.presets.review]\nselect=['preset:conventions','family:correctness']\n").unwrap();
    let base = [
        "lint",
        "--config",
        config.to_str().unwrap(),
        "--show-settings",
    ];
    let shown = run(&base, "invalid input");
    assert!(shown.status.success() && shown.stderr.is_empty());
    assert!(
        String::from_utf8(shown.stdout)
            .unwrap()
            .contains("Rules: missing-constraint-label,array-index-start,index-set-mismatch\n")
    );
    let selected = run(
        &[
            base[0],
            base[1],
            base[2],
            base[3],
            "--rules",
            "preset:arrays,hidden-optionality",
        ],
        "invalid input",
    );
    assert!(selected.status.success() && selected.stderr.is_empty());
    assert!(
        String::from_utf8(selected.stdout)
            .unwrap()
            .contains("Rules: index-set-mismatch,hidden-optionality,prefer-array-comprehension\n")
    );
    let listed = run(&["lint", "--list-rules"], "invalid input");
    assert!(listed.status.success() && listed.stderr.is_empty());
    assert!(
        String::from_utf8(listed.stdout)
            .unwrap()
            .contains("unused-generator-binding\tsuspicious\tavailable\tsometimes\tscope,all")
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn default_lint_checks_included_shadowing_without_writing() {
    let directory =
        std::env::temp_dir().join(format!("zincite-root-default-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("library/std")).unwrap();
    std::fs::write(directory.join("library/std/stdlib.mzn"), "").unwrap();
    let shared = directory.join("shared.mzn");
    let root = directory.join("root.mzn");
    let library = directory.join("library");
    let outer = "int: value = 1;\r\n";
    std::fs::write(&shared, outer).unwrap();
    for (parameter, status) in [("value", 1), ("item", 0)] {
        let source = format!(
            "include \"shared.mzn\";\r\nfunction int: identity(int: {parameter}) = {parameter};\r\nsolve satisfy;\r\n"
        );
        std::fs::write(&root, &source).unwrap();
        for selection in [None, Some("default"), Some("preset:default")] {
            let mut arguments = vec![
                "lint",
                "--isolated",
                "--stdlib-dir",
                library.to_str().unwrap(),
            ];
            if let Some(selection) = selection {
                arguments.extend(["--rules", selection]);
            }
            arguments.push(root.to_str().unwrap());
            let output = run(&arguments, "");
            let diagnostics = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(status), "{diagnostics}");
            assert!(output.stdout.is_empty());
            assert_eq!(
                diagnostics.contains("warning [suspicious-shadowing]"),
                status == 1
            );
            assert_eq!(std::fs::read(&root).unwrap(), source.as_bytes());
            assert_eq!(std::fs::read(&shared).unwrap(), outer.as_bytes());
        }
    }
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
fn bounded_paths_run_through_the_root_query_command() {
    let dir = std::env::temp_dir().join(format!("zincite-root-paths-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let model = dir.join("model.mzn");
    let source = include_bytes!("../crates/zincite-query/tests/fixtures/bounded-paths.mzn");
    std::fs::write(&model, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_zincite"))
        .args([
            "query",
            "--stdlib-dir",
            dir.join("library").to_str().unwrap(),
            "--program",
            include_str!("../crates/zincite-query/tests/fixtures/bounded-paths.zq").trim(),
            model.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rendered = String::from_utf8(output.stdout).unwrap();
    let steps = rendered
        .split_once("\"steps\":[")
        .unwrap()
        .1
        .split_once(']')
        .unwrap()
        .0;
    let a = steps.find("\"name\":\"a\"").unwrap();
    let b = steps.find("\"name\":\"b\"").unwrap();
    let c = steps.find("\"name\":\"c\"").unwrap();
    assert!(a < b && b < c);
    assert_eq!(std::fs::read(model).unwrap(), source);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn program_files_report_native_results_and_buffer_later_failures() {
    let path = std::env::temp_dir().join(format!("zincite-root-program-{}.zq", std::process::id()));
    std::fs::write(&path, r#"let selected = items; let n = selected | count; let h = selected | names | tally; report (count: n, histogram: h, selected: selected);"#).unwrap();
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
    std::fs::write(&path, r#"count | json; values;"#).unwrap();
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
    std::fs::write(&path, r#"count | json; count | items;"#).unwrap();
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
        r#"let labels = names;
let numbers = items | filter(name("zeta")) | values | one;
let n = count;
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
        "report if true then 1 else unsupported_summary([1]) endif;",
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

#[test]
fn keyed_program_files_keep_nominal_axes_and_buffer_later_errors() {
    let dir = std::env::temp_dir().join(format!("zincite-root-keyed-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let library = dir.join("library");
    let model = dir.join("root.mzn");
    let program = dir.join("report.zq");
    std::fs::write(
        &model,
        "\u{feff}enum Guest={Zulu,Alpha}; array[2..3,Guest] of int: table; solve satisfy;",
    )
    .unwrap();
    let report = r#"let table=items | filter(name("table")) | values | one; let members={enum_member(Guest,"Alpha"),enum_member(Guest,"Zulu")};
        let cells=[(i,e):table[i,e] | i in 2..3,e in members];
        let pairs=zip_exact(cells,cells); report [e.key:e.value.1 | e in entries(pairs)];"#;
    std::fs::write(&program, report).unwrap();
    let args = [
        "query",
        "--stdin-filepath",
        "report.dzn",
        "--model",
        model.to_str().unwrap(),
        "--stdlib-dir",
        library.to_str().unwrap(),
        "--program-file",
        program.to_str().unwrap(),
    ];
    let data = "\u{feff}table=[(3,Alpha):32,(2,Zulu):21,(3,Zulu):31,(2,Alpha):22];";
    let output = run(&args, data);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut inline = args[..args.len() - 2].to_vec();
    inline.extend(["--program", report]);
    let direct = run(&inline, data);
    assert!(direct.status.success(), "{direct:?}");
    assert_eq!(direct.stdout, output.stdout);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"complete\":true"));
    assert!(text.contains("\"indexing\":\"explicit\""));
    assert!(text.find("Zulu").unwrap() < text.find("Alpha").unwrap());
    assert!(text.find("\"value\":21").unwrap() < text.find("\"value\":32").unwrap());
    // An unconfigured model remains an incomplete report with useful rows.
    let without_library: Vec<_> = args
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(i, arg)| {
            let library_option = args.iter().position(|arg| *arg == "--stdlib-dir").unwrap();
            (!matches!(i, n if n == library_option || n == library_option + 1)).then_some(arg)
        })
        .collect();
    let incomplete = run(&without_library, data);
    assert_eq!(incomplete.status.code(), Some(1));
    let incomplete_text = String::from_utf8_lossy(&incomplete.stdout);
    assert!(
        incomplete_text.contains("\"complete\":false")
            && incomplete_text.contains("standard library is not configured")
    );
    for later in [
        "report 1 div 0;",
        "report [(1,1):1,(1,2):2,(2,1):3][2,2];",
        "let bad=[(1,1):1,(1,2):2,(2,1):3]; report lookup(bad,1);",
    ] {
        std::fs::write(&program, format!("{report} {later}")).unwrap();
        let output = run(&args, data);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(program.to_str().unwrap()));
    }
    let zero = r#"report [(i,e):1 | i in 1..0,e in {enum_member(Guest,"Zulu"),enum_member(Guest,"Alpha")}];"#;
    std::fs::write(&program, zero).unwrap();
    let output = run(&args, data);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    *inline.last_mut().unwrap() = zero;
    let direct = run(&inline, data);
    assert!(direct.status.success(), "{direct:?}");
    assert_eq!(direct.stdout, output.stdout);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"axes\":[[],[{"));
    assert!(text.contains("\"entries\":[]"));
    std::fs::write(
        &program,
        r#"let t=items | filter(name("table")) | values | one; report t;"#,
    )
    .unwrap();
    let output = run(
        &args,
        "\u{feff}table=[(2,Zulu):21,(3,Zulu):31,(2,Alpha):22];",
    );
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .matches("\"kind\":\"integer\",\"value\":21")
            .count(),
        1
    );
    std::fs::write(
        &model,
        "\u{feff}enum Guest={Zulu} ++ anon_enum(2); solve satisfy;",
    )
    .unwrap();
    std::fs::write(&program, "report enum_member(Guest,\"Zulu\");").unwrap();
    let output = run(&args, data);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains(model.to_str().unwrap()) && error.contains("computed"),
        "{error}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn data_program_files_preview_diff_and_write_one_coherent_candidate() {
    let dir =
        std::env::temp_dir().join(format!("zincite-root-data-program-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let model = dir.join("model.mzn");
    let program = dir.join("double.zq");
    let data = dir.join("input.dzn");
    std::fs::write(
        &model,
        include_bytes!("../crates/zincite-query/tests/fixtures/doubled.mzn"),
    )
    .unwrap();
    std::fs::write(
        &program,
        include_bytes!("../crates/zincite-query/tests/fixtures/double.zq"),
    )
    .unwrap();
    let original = include_bytes!("../crates/zincite-query/tests/fixtures/doubled.dzn");
    std::fs::write(&data, original).unwrap();
    let library = dir.join("library");
    let args = vec![
        "query",
        "--model",
        model.to_str().unwrap(),
        "--stdlib-dir",
        library.to_str().unwrap(),
        "-f",
        program.to_str().unwrap(),
        data.to_str().unwrap(),
    ];
    let output = run(&args, "");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let candidate = output.stdout;
    assert!(
        candidate
            .windows(b"peer: 4, group: 2".len())
            .any(|bytes| bytes == b"peer: 4, group: 2")
    );
    assert!(
        candidate
            .windows(b"capacities = [| 3, 4 | 5, 6 | 3, 4 | 5, 6 |]".len())
            .any(|bytes| bytes == b"capacities = [| 3, 4 | 5, 6 | 3, 4 | 5, 6 |]")
    );
    assert!(
        candidate
            .windows(b"groups = [1: {1, 2}, 2: {3, 4}]".len())
            .any(|bytes| bytes == b"groups = [1: {1, 2}, 2: {3, 4}]")
    );
    assert_eq!(std::fs::read(&data).unwrap(), original);
    let mut diff_args = args.clone();
    diff_args.insert(1, "--diff");
    let output = run(&diff_args, "");
    assert!(output.status.success());
    assert!(output.stdout.starts_with(b"--- a/"));
    assert_eq!(std::fs::read(&data).unwrap(), original);
    let mut write_args = args.clone();
    write_args.insert(1, "--write");
    let output = run(&write_args, "");
    assert!(output.status.success() && output.stdout.is_empty());
    assert_eq!(std::fs::read(&data).unwrap(), candidate);
    assert_eq!(
        std::fs::read(&model).unwrap(),
        include_bytes!("../crates/zincite-query/tests/fixtures/doubled.mzn")
    );
    std::fs::write(&data, original).unwrap();
    for failing in [
        "replace entity_count=4; emit;",
        "replace entity_count=4; replace group_count=1 div 0; emit;",
        "replace entity_count=4; replace 'entity_count'=5; emit;",
        r#"let n=2; let old=items | filter(name("missing")) | values | one; replace entity_count=n; emit;"#,
        r#"let old=items | filter(name(if false then require(true,"entities","hidden") else "entities" endif)) | values | one; replace entity_count=2; emit;"#,
    ] {
        std::fs::write(&program, failing).unwrap();
        let output = run(&write_args, "");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(std::fs::read(&data).unwrap(), original);
    }
    std::fs::write(&program, "create entity_count=2; create group_count=1; create entities=[(weight:7,peer:2,group:1),(weight:11,peer:1,group:1)]; create capacities=[(i,j):i+j | i in 1..2,j in 1..2]; create groups=[{1,2}]; emit;").unwrap();
    std::fs::write(&data, "").unwrap();
    let output = run(&args, "");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let created = output.stdout;
    let output = run(&write_args, "");
    assert!(output.status.success());
    assert_eq!(std::fs::read(&data).unwrap(), created);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn inline_programs_share_file_results_and_buffer_errors() {
    let source =
        r#"let d = items | filter(name("demand")) | values | one; let total=sum(d); total"#;
    let input = "demand=[2,3,4]; capacity=[6,8,5];";
    let path = std::env::temp_dir().join(format!("zincite-query-inline-{}.zq", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let expected =
        b"{\"complete\":true,\"limitations\":[],\"result\":{\"kind\":\"integer\",\"value\":9}}\n";
    let inline = run(
        &["query", "--stdin-filepath", "data.dzn", "--program", source],
        input,
    );
    let file = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "--program-file",
            path.to_str().unwrap(),
        ],
        input,
    );
    assert!(
        inline.status.success() && inline.stderr.is_empty(),
        "{:?}",
        inline
    );
    assert!(
        file.status.success() && file.stderr.is_empty(),
        "{:?}",
        file
    );
    assert_eq!(inline.stdout, expected);
    assert_eq!(file.stdout, expected);
    let capacity = include_str!("../crates/zincite-query/tests/fixtures/capacity.zq");
    std::fs::write(&path, capacity).unwrap();
    let file = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "-f",
            path.to_str().unwrap(),
        ],
        input,
    );
    assert!(
        file.status.success() && file.stderr.is_empty(),
        "{:?}",
        file
    );
    let report = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "--program",
            capacity,
            "-",
        ],
        input,
    );
    assert!(
        report.status.success() && report.stderr.is_empty(),
        "{:?}",
        report
    );
    assert_eq!(report.stdout, file.stdout);
    let parameter_program =
        include_str!("../crates/zincite-query/tests/fixtures/capacity-parameters.zq");
    let parameter_inline = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "--param",
            "factor=2",
            "--program",
            parameter_program,
        ],
        input,
    );
    assert!(parameter_inline.status.success(), "{:?}", parameter_inline);
    assert_eq!(parameter_inline.stdout, report.stdout);
    std::fs::write(&path, parameter_program).unwrap();
    let parameter_file = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "--param",
            "factor=+2",
            "-f",
            path.to_str().unwrap(),
        ],
        input,
    );
    assert!(parameter_file.status.success(), "{:?}", parameter_file);
    assert_eq!(parameter_file.stdout, report.stdout);
    for supplied in [
        vec![],
        vec!["factor=2", "factor=3"],
        vec!["factor=2", "extra=1"],
        vec!["factor=2.0"],
    ] {
        let mut arguments = vec![
            "query",
            "--stdin-filepath",
            "data.dzn",
            "--program",
            parameter_program,
        ];
        for value in supplied {
            arguments.extend(["--param", value]);
        }
        let refused = run(&arguments, input);
        assert!(
            !refused.status.success() && refused.stdout.is_empty(),
            "{:?}",
            refused
        );
    }

    for arguments in [
        vec![
            "query",
            "--program",
            source,
            "--program-file",
            path.to_str().unwrap(),
        ],
        vec!["query", "--program", source, "--program", source],
        vec!["query", "--program", source, "count", "data.dzn"],
        vec!["query", "--program", source, "--max-work", "0"],
        vec!["query", "--program", source, "--diff"],
    ] {
        let output = run(&arguments, input);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let output = run(
        &[
            "query",
            "--stdin-filepath",
            "data.dzn",
            "--program",
            "report 7; report require(false,1 div 0,\"capacity refused\");",
        ],
        input,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("<program>:1:")
            && diagnostic.contains("capacity refused")
            && diagnostic.contains("bytes 17.."),
        "{diagnostic}"
    );
    let output = run(
        &[
            "query",
            "--program",
            "let unused=sum([1]); replace a=[2]; emit;",
        ],
        input,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("report-only"));
    let groups = include_str!("../crates/zincite-query/tests/fixtures/machine-groups.zq");
    let data = include_str!("../crates/zincite-query/tests/fixtures/machine-groups.dzn");
    std::fs::write(&path, groups).unwrap();
    let data_path = path.with_extension("dzn");
    std::fs::write(&data_path, data).unwrap();
    let inline = run(
        &["query", "--program", groups, data_path.to_str().unwrap()],
        "",
    );
    let file = run(
        &[
            "query",
            "--program-file",
            path.to_str().unwrap(),
            data_path.to_str().unwrap(),
        ],
        "",
    );
    assert!(
        inline.status.success() && inline.stderr.is_empty(),
        "{inline:?}"
    );
    assert!(file.status.success() && file.stderr.is_empty(), "{file:?}");
    assert_eq!(inline.stdout, file.stdout);
    let value = String::from_utf8(inline.stdout).unwrap();
    assert!(value.find("\"value\":\"b\"").unwrap() < value.find("\"value\":\"a\"").unwrap());
    assert!(value.contains("\"value\":13"));
    let failure = run(
        &[
            "query",
            "--program",
            r#"report 7; report group_by([(machine: "b")], "absent");"#,
            data_path.to_str().unwrap(),
        ],
        "",
    );
    assert!(!failure.status.success() && failure.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("missing record field: absent"));
    assert_eq!(std::fs::read_to_string(&data_path).unwrap(), data);
    std::fs::remove_file(data_path).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_composition_inline_and_file_keep_results_status_and_buffering() {
    let dir = std::env::temp_dir().join(format!("zincite-root-native-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let model_path = dir.join("model.mzn");
    let program_path = dir.join("report.zq");
    let source = include_bytes!("../crates/zincite-query/tests/fixtures/native-composition.mzn");
    let program_source =
        include_str!("../crates/zincite-query/tests/fixtures/native-composition.zq");
    std::fs::write(&model_path, source).unwrap();
    std::fs::write(&program_path, program_source).unwrap();
    let library = dir.join("library");
    let common = ["--stdlib-dir", library.to_str().unwrap()];
    let inline = run(
        &[
            "query",
            common[0],
            common[1],
            "--program",
            program_source,
            model_path.to_str().unwrap(),
        ],
        "",
    );
    let file = run(
        &[
            "query",
            common[0],
            common[1],
            "--program-file",
            program_path.to_str().unwrap(),
            model_path.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(
        inline.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&inline.stderr)
    );
    assert_eq!(
        file.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&file.stderr)
    );
    assert!(inline.stderr.is_empty() && file.stderr.is_empty());
    assert_eq!(inline.stdout, file.stdout);

    let output = String::from_utf8(inline.stdout).unwrap();
    assert_eq!(output.lines().count(), 4);
    assert!(output.contains(r#""matching_count":{"kind":"integer","value":2}"#));
    assert!(output.contains("\"value\":\"capacity\""));
    assert!(output.contains("\"value\":2"));
    assert_eq!(std::fs::read(&model_path).unwrap(), source);
    let expected_output = output.as_bytes();
    let paired = format!("fn unused(n:int)->int {{ n }} {program_source}");
    std::fs::write(&program_path, &paired).unwrap();
    for arguments in [
        vec![
            "query",
            common[0],
            common[1],
            "--program",
            &paired,
            model_path.to_str().unwrap(),
        ],
        vec![
            "query",
            common[0],
            common[1],
            "--program-file",
            program_path.to_str().unwrap(),
            model_path.to_str().unwrap(),
        ],
    ] {
        let output = run(&arguments, "");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected_output);
    }
    let failure = format!("{program_source} [1] | tally;");
    std::fs::write(&program_path, &failure).unwrap();
    for arguments in [
        vec![
            "query",
            common[0],
            common[1],
            "--program",
            &failure,
            model_path.to_str().unwrap(),
        ],
        vec![
            "query",
            common[0],
            common[1],
            "--program-file",
            program_path.to_str().unwrap(),
            model_path.to_str().unwrap(),
        ],
    ] {
        let output = run(&arguments, "");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("tally") && (error.contains("<program>") || error.contains("report.zq")),
            "{error}"
        );
    }
    let incomplete = b"int: value = missing; solve satisfy;";
    std::fs::write(&model_path, incomplete).unwrap();
    let known = r#"let unresolved = references | declarations | names | count; report match { from d in (subtree | declarations | unique); };"#;
    for (program_source, expected_status) in [
        (known.to_owned(), 1),
        (format!("{known} report 1 div 0;"), 2),
    ] {
        std::fs::write(&program_path, &program_source).unwrap();
        for arguments in [
            vec![
                "query",
                common[0],
                common[1],
                "--program",
                &program_source,
                model_path.to_str().unwrap(),
            ],
            vec![
                "query",
                common[0],
                common[1],
                "--program-file",
                program_path.to_str().unwrap(),
                model_path.to_str().unwrap(),
            ],
        ] {
            let output = run(&arguments, "");
            assert_eq!(
                output.status.code(),
                Some(expected_status),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if expected_status == 1 {
                let output = String::from_utf8(output.stdout).unwrap();
                assert!(output.contains("\"complete\":false"));
                assert!(output.contains("\"name\":\"value\""));
            } else {
                assert!(output.stdout.is_empty());
            }
            assert_eq!(std::fs::read(&model_path).unwrap(), incomplete);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn keyed_comparison_inline_and_file_agree_and_buffer_alignment_errors() {
    let directory =
        std::env::temp_dir().join(format!("zincite-root-comparison-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let data_path = directory.join("comparison.dzn");
    let program_path = directory.join("comparison.zq");
    let data = include_bytes!("../crates/zincite-query/tests/fixtures/keyed-comparison.dzn");
    std::fs::write(&data_path, data).unwrap();
    for source in [
        include_str!("../crates/zincite-query/tests/fixtures/keyed-comparison.zq"),
        include_str!("../crates/zincite-query/tests/fixtures/indexed-arrays.zq"),
        "[(5,8):58, (2,1):21] | count",
        "report entries([|3,4|5,6|]);",
    ] {
        std::fs::write(&program_path, source).unwrap();
        let mut expected = None;
        for args in [
            vec!["query", "--program", source, data_path.to_str().unwrap()],
            vec![
                "query",
                "--program-file",
                program_path.to_str().unwrap(),
                data_path.to_str().unwrap(),
            ],
        ] {
            let output = run(&args, "");
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
            if let Some(expected) = &expected {
                assert_eq!(&output.stdout, expected);
            } else {
                assert!(String::from_utf8_lossy(&output.stdout).contains("\"complete\":true"));
                expected = Some(output.stdout);
            }
        }
    }
    let source = "report 7; report zip_exact([1:3,2:4], [2:3,3:4]);";
    let output = run(
        &["query", "--program", source, data_path.to_str().unwrap()],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    let call_start = source.find("zip_exact").unwrap();
    assert!(
        diagnostic.contains("<program>:1:")
            && diagnostic.contains("identical ordered axes")
            && diagnostic.contains(&format!("bytes {call_start}..{}", source.len() - 1)),
        "{diagnostic}"
    );
    assert_eq!(std::fs::read(&data_path).unwrap(), data);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn string_matching_agrees_inline_file_library_and_buffers_later_errors() {
    let source = include_str!("../crates/zincite-query/tests/fixtures/string-matching.zq");
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/zincite-query/tests/fixtures/string-matching.zq"
    );
    let model = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/zincite-query/tests/fixtures/string-matching.mzn"
    );
    let mut expected = None;
    for arguments in [
        vec!["query", "--program", source, model],
        vec!["query", "--program-file", file, model],
    ] {
        let output = run(&arguments, "");
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("work_load"));
        if let Some(expected) = &expected {
            assert_eq!(&output.stdout, expected);
        } else {
            expected = Some(output.stdout);
        }
    }
    let failed = run(
        &[
            "query",
            "--program",
            &format!("{source} report contains(\"x\", [\"z\"][2]);"),
            model,
        ],
        "",
    );
    assert_eq!(failed.status.code(), Some(2));
    assert!(failed.stdout.is_empty());
}

#[test]
fn report_ordering_inline_file_and_later_failure_keep_output_buffering() {
    let source = include_str!("../crates/zincite-query/tests/fixtures/report-ordering.zq");
    let path =
        std::env::temp_dir().join(format!("zincite-ordering-root-{}.zq", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let common = ["query", "--stdin-filepath", "input.dzn"];
    let mut inline = common.to_vec();
    inline.extend(["--program", source]);
    let mut file = common.to_vec();
    file.extend(["--program-file", path.to_str().unwrap()]);
    let input = "a=1;";
    let inline = run(&inline, input);
    let file = run(&file, input);
    assert!(
        inline.status.success() && inline.stderr.is_empty(),
        "{inline:?}"
    );
    assert!(file.status.success() && file.stderr.is_empty(), "{file:?}");
    assert_eq!(inline.stdout, file.stdout);
    let mut failure = common.to_vec();
    failure.extend([
        "--program",
        r#"report 7; report min_by([(score:1), (other:2)], "score");"#,
    ]);
    let failure = run(&failure, input);
    assert!(!failure.status.success() && failure.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("missing ordering field"));
    for source in [
        "report 7; items | sort;",
        "report 7; (items | values) union (items | values);",
    ] {
        let failure = run(
            &[
                "query",
                "--stdin-filepath",
                "input.dzn",
                "--program",
                source,
            ],
            "a=1;",
        );
        assert_eq!(failure.status.code(), Some(2), "{failure:?}");
        assert!(failure.stdout.is_empty());
        assert!(!failure.stderr.is_empty());
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn query_patterns_and_control_values_replay_inline_file_and_buffer_later_errors() {
    let model = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/zincite-query/tests/fixtures/recursive-functions.mzn"
    );
    for (source, file) in [
        (
            include_str!("../crates/zincite-query/tests/fixtures/destructured-report.zq"),
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/crates/zincite-query/tests/fixtures/destructured-report.zq"
            ),
        ),
        (
            include_str!("../crates/zincite-query/tests/fixtures/optional-match.zq"),
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/crates/zincite-query/tests/fixtures/optional-match.zq"
            ),
        ),
    ] {
        let mut expected = None;
        for arguments in [
            vec!["query", "--program", source, model],
            vec!["query", "--program-file", file, model],
        ] {
            let output = run(&arguments, "");
            assert!(output.status.success(), "{output:?}");
            if let Some(expected) = &expected {
                assert_eq!(&output.stdout, expected);
            } else {
                expected = Some(output.stdout);
            }
        }
        let failed = run(
            &[
                "query",
                "--program",
                &format!("{source} report match(1){{ _ => 1 div 0 }};"),
                model,
            ],
            "",
        );
        assert_eq!(failed.status.code(), Some(2));
        assert!(failed.stdout.is_empty());
    }
}

#[test]
fn native_stream_controls_share_inline_file_and_positional_execution() {
    let source = r#"let saved=items; let n=saved | count; let message="rows";
        saved | tail(n-1) | reverse | check(message) | check(2,message) | names"#;
    let path = std::env::temp_dir().join(format!(
        "zincite-stream-controls-root-{}.zq",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let common = ["query", "--stdin-filepath", "input.dzn"];
    let mut inline = common.to_vec();
    inline.extend(["--program", source]);
    let mut file = common.to_vec();
    file.extend(["--program-file", path.to_str().unwrap()]);
    let input = "a=1; b=2; c=3;";
    let inline = run(&inline, input);
    let file = run(&file, input);
    assert!(
        inline.status.success() && inline.stderr.is_empty(),
        "{inline:?}"
    );
    assert!(file.status.success() && file.stderr.is_empty(), "{file:?}");
    assert_eq!(inline.stdout, b"c\nb\n");
    assert_eq!(inline.stdout, file.stdout);
    let mut positional = common.to_vec();
    positional.push("tail(2) | reverse | check(2, \"rows\") | names");
    let positional = run(&positional, input);
    assert!(
        positional.status.success() && positional.stderr.is_empty(),
        "{positional:?}"
    );
    assert_eq!(inline.stdout, positional.stdout);
    let failed = r#"report 7; items | tail(2) | check(1,"need one")"#;
    std::fs::write(&path, failed).unwrap();
    let mut failure = common.to_vec();
    failure.extend(["--program-file", path.to_str().unwrap()]);
    let failure = run(&failure, input);
    assert_eq!(failure.status.code(), Some(2));
    assert!(failure.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("expected 1 items, got 2: need one"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_dictionaries_replay_inline_files_and_buffer_duplicate_values() {
    let dir = std::env::temp_dir().join(format!("zincite-dictionary-root-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let source = include_str!("../crates/zincite-query/tests/fixtures/native-dictionaries.zq");
    let input = include_bytes!("../crates/zincite-query/tests/fixtures/native-dictionaries.mzn");
    let model_path = dir.join("model.mzn");
    let query_path = dir.join("report.zq");
    std::fs::write(&model_path, input).unwrap();
    std::fs::write(&query_path, source).unwrap();
    let library = dir.join("library");
    let common = [
        "query",
        "--stdlib-dir",
        library.to_str().unwrap(),
        model_path.to_str().unwrap(),
    ];
    let mut inline = common.to_vec();
    inline.extend(["--program", source]);
    let mut file = common.to_vec();
    file.extend(["--program-file", query_path.to_str().unwrap()]);
    let inline = run(&inline, "");
    let file = run(&file, "");
    assert!(
        inline.status.success() && inline.stderr.is_empty(),
        "{inline:?}"
    );
    assert!(file.status.success() && file.stderr.is_empty(), "{file:?}");
    assert_eq!(inline.stdout, file.stdout);

    let output = String::from_utf8(inline.stdout).unwrap();
    assert!(output.contains(r#""ranked":{"B":2,"A":1}"#), "{output}");
    assert!(
        output.contains(r#""empty":{},"nested":{"A":{"A":1,"B":2,"C":0}}"#),
        "{output}"
    );
    assert!(output.contains(r#""result":{"z":{"inner":{"kind":"integer","value":3}},"a":{"kind":"integer","value":1}}"#),"{output}");
    let mut failure = common.to_vec();
    failure.extend(["--program",r#"report 7; fn payload(n:Node)->int { if (n | names | one) = "B" { 1 div 0 } else { 1 } } let key="same"; items | filter(name("A") or name("B")) | map_by(key,payload(input));"#]);
    let failure = run(&failure, "");
    assert_eq!(failure.status.code(), Some(2), "{failure:?}");
    assert!(failure.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&failure.stderr).contains("duplicate key `same`"),
        "{failure:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_existence_and_fixed_points_replay_files_and_buffer_nested_failures() {
    let dir = std::env::temp_dir().join(format!("zincite-native-root-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let library = dir.join("library");
    let input = include_bytes!("../crates/zincite-query/tests/fixtures/worklist-reachability.mzn");
    let source = include_str!("../crates/zincite-query/tests/fixtures/native-fixed-points.zq");
    let model = dir.join("model.mzn");
    let file = dir.join("report.zq");
    std::fs::write(&model, input).unwrap();
    std::fs::write(&file, source).unwrap();
    let common = vec![
        "query",
        "--stdlib-dir",
        library.to_str().unwrap(),
        model.to_str().unwrap(),
    ];
    let mut args = common.clone();
    args.extend(["--program", source]);
    let inline = run(&args, "");
    assert!(inline.status.success(), "{inline:?}");
    let mut args = common.clone();
    args.extend(["--program-file", file.to_str().unwrap()]);
    let saved = run(&args, "");
    assert!(saved.status.success(), "{saved:?}");
    assert_eq!(inline.stdout, saved.stdout);

    for source in [
        r#"report 7; declarations | filter(not name("E")) | reverse | where_exists(referenced_declarations | check(1));"#,
        "report 7; declarations | fixpoint(reverse);",
    ] {
        std::fs::write(&file, source).unwrap();
        for (flag, value) in [
            ("--program", source),
            ("--program-file", file.to_str().unwrap()),
        ] {
            let mut args = common.clone();
            args.extend([flag, value]);
            let output = run(&args, "");
            assert_eq!(output.status.code(), Some(2), "{output:?}");
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn prepared_projections_keep_scope_and_buffer_failures_inline_and_in_files() {
    let dir = std::env::temp_dir().join(format!("zincite-projection-scope-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("report.zq");
    let input = "int: a=1; int: b=2;";
    for prefix in ["", "fn n()->int {1} "] {
        let source = format!(
            r#"{prefix}let original=items; report items | project(d,{{"n":count,"original":original | count}});"#
        );
        std::fs::write(&file, &source).unwrap();
        let inline = run(&["query", "--program", &source], input);
        let saved = run(&["query", "--program-file", file.to_str().unwrap()], input);
        assert!(inline.status.success(), "{inline:?}");
        assert!(saved.status.success(), "{saved:?}");
        assert_eq!(inline.stdout, saved.stdout);
        let output = String::from_utf8(inline.stdout).unwrap();
        assert_eq!(
            output
                .matches(
                    r#""n":{"kind":"integer","value":1},"original":{"kind":"integer","value":2}"#
                )
                .count(),
            2
        );
        let source = format!(r#"{prefix}report 7; report items | head(0) | project(d,{{"x":d}});"#);
        std::fs::write(&file, &source).unwrap();
        for (flag, value) in [
            ("--program", source.as_str()),
            ("--program-file", file.to_str().unwrap()),
        ] {
            let output = run(&["query", flag, value], input);
            assert_eq!(output.status.code(), Some(2), "{output:?}");
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("explicit scalar projection"));
        }
    }
    let source = r#"fn fail(n:int)->int {1 div (n-1)}
        report 7; report items | project(d,{"x":fail(d | count)});"#;
    std::fs::write(&file, source).unwrap();
    for (flag, value) in [
        ("--program", source),
        ("--program-file", file.to_str().unwrap()),
    ] {
        let output = run(&["query", flag, value], input);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("fail") && error.contains("bytes"), "{error}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}
