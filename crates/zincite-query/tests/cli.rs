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
        (vec!["count", "--model"], "", "requires a path"),
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
    assert!(help.contains("--model") && help.contains("reduce_enum") && help.contains("subtree"));
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

#[test]
fn enum_reduction_previews_complete_and_incomplete_candidates_and_uses_read_only_model() {
    let directory =
        std::env::temp_dir().join(format!("zincite-query-reduce-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let includes = directory.join("includes");
    std::fs::create_dir(&includes).unwrap();
    let model = directory.join("model.mzn");
    let declarations = includes.join("declarations.mzn");
    let data = directory.join("instance.dzn");
    let model_bytes = b"include \"declarations.mzn\"; solve satisfy;\n";
    let declaration_bytes = b"enum Guests; array[Guests] of int: scores; array[int] of set of Guests: same_table; set of Guests: chosen;\nconstraint forall(g in Guests)(scores[g] >= 0);\n";
    let source = b"Guests = {A,B,C}; chosen = same_table[3]; scores = [10,20,30]; same_table = [{A,B},{B},{C}];\n";
    let expression = "reduce_enum(\"Guests\", keep(\"C\", \"A\"))";
    std::fs::write(&model, model_bytes).unwrap();
    std::fs::write(&declarations, declaration_bytes).unwrap();
    std::fs::write(&data, source).unwrap();
    let options = [
        "--model",
        model.to_str().unwrap(),
        "-I",
        includes.to_str().unwrap(),
    ];
    let mut arguments = options.to_vec();
    arguments.extend([expression, data.to_str().unwrap()]);
    let output = run(&arguments, b"");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"Guests = {A,C}; chosen = same_table[2]; scores = [10,30]; same_table = [{A},{C}];\n"
    );
    let candidate = output.stdout;
    for mode in ["--diff", "--write"] {
        let mut arguments = options.to_vec();
        arguments.extend([mode, expression, data.to_str().unwrap()]);
        let output = run(&arguments, b"");
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        if mode == "--diff" {
            assert!(output.stdout.starts_with(b"--- a/"));
        }
    }
    assert_eq!(std::fs::read(&data).unwrap(), candidate);
    assert_eq!(std::fs::read(&model).unwrap(), model_bytes);
    assert_eq!(std::fs::read(&declarations).unwrap(), declaration_bytes);

    let incomplete = b"Guests = {A,B,C}; scores = [10,20,30]; same_table = [{A,B},{B},{C}]; chosen = same_table[2];\n";
    std::fs::write(&data, incomplete).unwrap();
    for mode in [None, Some("--diff"), Some("--write")] {
        let mut arguments = options.to_vec();
        arguments.extend(mode);
        arguments.extend([expression, data.to_str().unwrap()]);
        let output = run(&arguments, b"");
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("removed or out-of-coverage group")
        );
        if mode == Some("--write") {
            assert!(output.stdout.is_empty());
        } else {
            assert!(!output.stdout.is_empty());
        }
        assert_eq!(std::fs::read(&data).unwrap(), incomplete);
    }
    let computed_model = b"\xef\xbb\xbfenum Guests; array[Guests] of int: scores; array[int] of set of Guests: same_table;\nint: ordinal = scores[to_enum(Guests, 2)];\nset of Guests: chosen;\n";
    std::fs::write(&declarations, computed_model).unwrap();
    std::fs::write(&data, source).unwrap();
    for mode in [None, Some("--diff"), Some("--write")] {
        let mut arguments = options.to_vec();
        arguments.extend(mode);
        arguments.extend([expression, data.to_str().unwrap()]);
        let output = run(&arguments, b"");
        assert_eq!(output.status.code(), Some(1));
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostics.contains("declarations.mzn:2:16:"),
            "{diagnostics}"
        );
        assert!(diagnostics.contains("computed enum-indexed access"));
        assert_eq!(std::fs::read(&data).unwrap(), source);
        assert_eq!(std::fs::read(&declarations).unwrap(), computed_model);
        if mode == Some("--write") {
            assert!(output.stdout.is_empty());
        } else {
            assert!(!output.stdout.is_empty());
        }
    }
    let output = run(
        &[
            "--model",
            model.to_str().unwrap(),
            expression,
            data.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("model.mzn:1:"));
    let output = run(
        &[
            "--model",
            "unavailable.mzn",
            "count",
            data.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"4\n");
    let output = run(
        &[
            "reduce_enum(\"Guests\", keep(\"unknown\"))",
            data.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn semantic_cli_loads_only_requested_models_and_keeps_incomplete_and_error_statuses() {
    let directory =
        std::env::temp_dir().join(format!("zincite-query-semantic-cli-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("library/std")).unwrap();
    let root = directory.join("root.mzn");
    let library = directory.join("library");
    let source =
        b"\xef\xbb\xbfinclude \"included.mzn\";\r\nint: value = imported;\r\nsolve satisfy;\r\n";
    std::fs::write(&root, source).unwrap();
    std::fs::write(
        directory.join("included.mzn"),
        b"\xef\xbb\xbfint: imported = 2;\r\n",
    )
    .unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "").unwrap();
    let expression = "filter(name(\"value\")) | references | declarations | types | json";
    let output = run(
        &[
            "--stdlib-dir",
            library.to_str().unwrap(),
            expression,
            root.to_str().unwrap(),
        ],
        b"",
    );
    assert!(output.status.success(), "{:?}", output.stderr);
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["result"][0]["text"], "int: imported = 2;");
    assert_eq!(json["result"][0]["type"]["kind"], "int");
    assert_eq!(json["result"][0]["source_kind"], "user");
    let arguments = [
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
    let mismatch = run(&arguments, &source[3..]);
    assert_eq!(mismatch.status.code(), Some(2));
    assert!(mismatch.stdout.is_empty());
    assert!(
        String::from_utf8(mismatch.stderr)
            .unwrap()
            .contains("exactly match")
    );
    let label = run(
        &["--stdin-filepath", root.to_str().unwrap(), "references"],
        source,
    );
    assert_eq!(label.status.code(), Some(2));
    assert!(label.stdout.is_empty());

    let unknown = directory.join("unknown.mzn");
    std::fs::write(&unknown, "int: value = absent; solve satisfy;").unwrap();
    let output = run(
        &[
            "--stdlib-dir",
            library.to_str().unwrap(),
            "references | declarations | names | count | json",
            unknown.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["complete"], false);
    assert_eq!(json["result"], 0);
    assert!(!json["limitations"].as_array().unwrap().is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("incomplete")
    );
    let missing = directory.join("missing.mzn");
    std::fs::write(&missing, "include \"absent.mzn\"; solve satisfy;").unwrap();
    let output = run(&["references | json", missing.to_str().unwrap()], b"");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("absent.mzn")
    );
    let output = run(
        &[
            "--model",
            directory.join("not-present.mzn").to_str().unwrap(),
            "count",
            root.to_str().unwrap(),
        ],
        b"",
    );
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(output.stdout, b"3\n");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn program_files_report_native_results_and_buffer_later_failures() {
    let path =
        std::env::temp_dir().join(format!("zincite-query-program-{}.zq", std::process::id()));
    std::fs::write(&path, r#"source selected = query(""); source n = selected | query("count"); source h = selected | query("names | tally"); report (count: n, histogram: h, selected: selected);"#).unwrap();
    let output = run(
        &["--stdin-filepath", "data.dzn", "-f", path.to_str().unwrap()],
        b"a = 1; b = 2;",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(report["result"]["count"], 2);
    assert_eq!(report["result"]["histogram"]["a"], 1);
    assert_eq!(report["result"]["selected"][0]["range"]["start"], 0);
    std::fs::write(&path, r#"query("count | json"); query("values");"#).unwrap();
    let output = run(
        &[
            "--stdin-filepath",
            "data.dzn",
            "--program-file",
            path.to_str().unwrap(),
        ],
        b"a = 1 + 2;",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("literal"), "{diagnostic}");
    assert!(diagnostic.contains("data.dzn"), "{diagnostic}");
    std::fs::write(&path, r#"query("count | json"); query("count | items");"#).unwrap();
    let output = run(
        &["--stdin-filepath", "data.dzn", "-f", path.to_str().unwrap()],
        b"a = 1; b = 2;",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains(path.to_str().unwrap()));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn positional_program_files_share_order_and_buffer_value_failures() {
    let path = std::env::temp_dir().join(format!(
        "zincite-query-positional-{}.zq",
        std::process::id()
    ));
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
    let args = ["--stdin-filepath", "data.dzn", "-f", path.to_str().unwrap()];
    let output = run(&args, b"zeta = [3, 5]; alpha = 2;");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["result"]["entries"][0]["value"]["fields"][0]["value"]["value"],
        "zeta"
    );
    assert_eq!(
        json["result"]["entries"][2]["value"]["fields"][2]["value"]["value"],
        9
    );
    for source in [
        "report 7; report 1 div 0;",
        "report if true then 1 else max([1]) endif;",
    ] {
        std::fs::write(&path, source).unwrap();
        let output = run(&args, b"a = 1;");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(path.to_str().unwrap()));
    }
    std::fs::write(&path, "report [i | i in 1..1000 where false];").unwrap();
    let output = run(
        &[
            "--stdin-filepath",
            "data.dzn",
            "--max-results",
            "50",
            "-f",
            path.to_str().unwrap(),
        ],
        b"a = 1;",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    std::fs::write(&path, "report [i | i in 1..20];").unwrap();
    let output = run(
        &[
            "--stdin-filepath",
            "data.dzn",
            "--max-results",
            "200",
            "-f",
            path.to_str().unwrap(),
        ],
        b"a = 1;",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn keyed_program_files_keep_nominal_axes_and_buffer_later_errors() {
    let dir = std::env::temp_dir().join(format!("zincite-query-keyed-{}", std::process::id()));
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
    let report = r#"source table=literal("table"); let members={enum_member(Guest,"Alpha"),enum_member(Guest,"Zulu")};
        let cells=[(i,e):table[i,e] | i in 2..3,e in members];
        let pairs=zip_exact(cells,cells); report [e.key:e.value.1 | e in entries(pairs)];"#;
    std::fs::write(&program, report).unwrap();
    let args = [
        "--stdin-filepath",
        "report.dzn",
        "--model",
        model.to_str().unwrap(),
        "--stdlib-dir",
        library.to_str().unwrap(),
        "-f",
        program.to_str().unwrap(),
    ];
    let data = b"\xef\xbb\xbftable=[(3,Alpha):32,(2,Zulu):21,(3,Zulu):31,(2,Alpha):22];";
    let output = run(&args, data);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["result"]["indexing"], "explicit");
    assert_eq!(json["result"]["axes"][1][0]["name"], "Zulu");
    assert_eq!(json["result"]["entries"][0]["value"]["value"], 21);
    assert_eq!(json["result"]["entries"][3]["value"]["value"], 32);
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
        "report [v|v in [(1,1):1,(1,2):2,(2,1):3]];",
        "let bad=[(1,1):1,(1,2):2,(2,1):3]; report [x|x in bad];",
    ] {
        std::fs::write(&program, format!("{report} {later}")).unwrap();
        let output = run(&args, data);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(program.to_str().unwrap()));
    }
    std::fs::write(&program, r#"report [(i,e):1 | i in 1..0,e in {enum_member(Guest,"Zulu"),enum_member(Guest,"Alpha")}];"#).unwrap();
    let output = run(&args, data);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["result"]["axes"][0], serde_json::json!([]));
    assert_eq!(json["result"]["axes"][1].as_array().unwrap().len(), 2);
    assert_eq!(json["result"]["entries"], serde_json::json!([]));
    std::fs::write(&program, r#"source t=literal("table"); report t;"#).unwrap();
    let output = run(
        &args,
        b"\xef\xbb\xbftable=[(2,Zulu):21,(3,Zulu):31,(2,Alpha):22];",
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("report.dzn:1:8:"));
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
        std::env::temp_dir().join(format!("zincite-query-data-program-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let model = dir.join("model.mzn");
    let program = dir.join("double.zq");
    let data = dir.join("input.dzn");
    std::fs::write(&model, include_bytes!("fixtures/doubled.mzn")).unwrap();
    std::fs::write(&program, include_bytes!("fixtures/double.zq")).unwrap();
    let original = include_bytes!("fixtures/doubled.dzn");
    std::fs::write(&data, original).unwrap();
    let library = dir.join("library");
    let args = vec![
        "--model",
        model.to_str().unwrap(),
        "--stdlib-dir",
        library.to_str().unwrap(),
        "-f",
        program.to_str().unwrap(),
        data.to_str().unwrap(),
    ];
    let output = run(&args, b"");
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
    diff_args.insert(0, "--diff");
    let output = run(&diff_args, b"");
    assert!(output.status.success());
    assert!(output.stdout.starts_with(b"--- a/"));
    assert_eq!(std::fs::read(&data).unwrap(), original);
    let mut write_args = args.clone();
    write_args.insert(0, "--write");
    let output = run(&write_args, b"");
    assert!(output.status.success() && output.stdout.is_empty());
    assert_eq!(std::fs::read(&data).unwrap(), candidate);
    assert_eq!(
        std::fs::read(&model).unwrap(),
        include_bytes!("fixtures/doubled.mzn")
    );
    std::fs::write(&data, original).unwrap();
    std::fs::write(&program,"param field:string; param value:int; source old=literal(field); replace group_count=value; emit;").unwrap();
    let mut parameter_args = args.clone();
    parameter_args.extend(["--param", "field=group_count", "--param", "value=1"]);
    let parameter_preview = run(&parameter_args, b"");
    assert!(
        parameter_preview.status.success(),
        "{:?}",
        parameter_preview
    );
    assert_eq!(std::fs::read(&data).unwrap(), original);
    for supplied in [
        vec![],
        vec!["field=group_count", "value=1", "value=1"],
        vec!["field=group_count", "value=wrong"],
    ] {
        let mut refused_args = write_args.clone();
        for value in supplied {
            refused_args.extend(["--param", value]);
        }
        let refused = run(&refused_args, b"");
        assert!(!refused.status.success() && refused.stdout.is_empty());
        assert_eq!(std::fs::read(&data).unwrap(), original);
    }

    for failing in [
        "replace entity_count=4; emit;",
        "replace entity_count=4; replace group_count=1 div 0; emit;",
        "replace entity_count=4; replace 'entity_count'=5; emit;",
    ] {
        std::fs::write(&program, failing).unwrap();
        let output = run(&write_args, b"");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(std::fs::read(&data).unwrap(), original);
    }
    std::fs::write(&program, "create entity_count=2; create group_count=1; create entities=[(weight:7,peer:2,group:1),(weight:11,peer:1,group:1)]; create capacities=[(i,j):i+j | i in 1..2,j in 1..2]; create groups=[{1,2}]; emit;").unwrap();
    std::fs::write(&data, "").unwrap();
    let output = run(&args, b"");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let created = output.stdout;
    let output = run(&write_args, b"");
    assert!(output.status.success());
    assert_eq!(std::fs::read(&data).unwrap(), created);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn inline_programs_share_file_library_results_and_buffer_errors() {
    use zincite_query::{Input, Program, ProgramLimits};
    use zincite_syntax::FileMode;
    let source = r#"source d = literal("demand"); report sum(d);"#;
    let input = "demand=[2,3,4]; capacity=[6,8,5];";
    let path = std::env::temp_dir().join(format!("zincite-query-inline-{}.zq", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let native = Input::parse(input.as_bytes().to_vec(), FileMode::Data).unwrap();
    let expected = Program::parse(source, ProgramLimits::default())
        .unwrap()
        .evaluate(&native, ProgramLimits::default())
        .unwrap()
        .render()
        .unwrap();
    let inline = run(
        &["--stdin-filepath", "data.dzn", "--program", source],
        input.as_bytes(),
    );
    let file = run(
        &[
            "--stdin-filepath",
            "data.dzn",
            "--program-file",
            path.to_str().unwrap(),
        ],
        input.as_bytes(),
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
    let capacity = include_str!("fixtures/capacity.zq");
    let expected = Program::parse(capacity, ProgramLimits::default())
        .unwrap()
        .evaluate(&native, ProgramLimits::default())
        .unwrap()
        .render()
        .unwrap();
    let report = run(
        &["--stdin-filepath", "data.dzn", "--program", capacity, "-"],
        input.as_bytes(),
    );
    assert!(
        report.status.success() && report.stderr.is_empty(),
        "{:?}",
        report
    );
    assert_eq!(report.stdout, expected);
    let parameter_program = include_str!("fixtures/capacity-parameters.zq");
    let parameter_inline = run(
        &[
            "--stdin-filepath",
            "data.dzn",
            "--param",
            "factor=2",
            "--program",
            parameter_program,
        ],
        input.as_bytes(),
    );
    assert!(parameter_inline.status.success(), "{:?}", parameter_inline);
    assert_eq!(parameter_inline.stdout, report.stdout);
    std::fs::write(&path, parameter_program).unwrap();
    let parameter_file = run(
        &[
            "--stdin-filepath",
            "data.dzn",
            "--param",
            "factor=+2",
            "-f",
            path.to_str().unwrap(),
        ],
        input.as_bytes(),
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
            "--stdin-filepath",
            "data.dzn",
            "--program",
            parameter_program,
        ];
        for value in supplied {
            arguments.extend(["--param", value]);
        }
        let refused = run(&arguments, input.as_bytes());
        assert!(
            !refused.status.success() && refused.stdout.is_empty(),
            "{:?}",
            refused
        );
    }

    for arguments in [
        vec![
            "--program",
            source,
            "--program-file",
            path.to_str().unwrap(),
        ],
        vec!["--program", source, "--program", source],
        vec!["--program", source, "count", "data.dzn"],
        vec!["--program", source, "--max-work", "0"],
        vec!["--program", source, "--diff"],
    ] {
        let output = run(&arguments, input.as_bytes());
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let output = run(
        &[
            "--stdin-filepath",
            "data.dzn",
            "--program",
            "report 7; report require(false,1 div 0,\"capacity refused\");",
        ],
        input.as_bytes(),
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
        &["--program", "let unused=sum([1]); replace a=[2]; emit;"],
        input.as_bytes(),
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("report-only"));
    let groups = include_str!("fixtures/machine-groups.zq");
    let data = include_bytes!("fixtures/machine-groups.dzn");
    std::fs::write(&path, groups).unwrap();
    let data_path = path.with_extension("dzn");
    std::fs::write(&data_path, data).unwrap();
    let native = Input::parse(data.to_vec(), FileMode::Data).unwrap();
    let expected = Program::parse(groups, ProgramLimits::default())
        .unwrap()
        .evaluate(&native, ProgramLimits::default())
        .unwrap()
        .render()
        .unwrap();
    for args in [
        vec!["--program", groups, data_path.to_str().unwrap()],
        vec![
            "--program-file",
            path.to_str().unwrap(),
            data_path.to_str().unwrap(),
        ],
    ] {
        let output = run(&args, b"");
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{output:?}"
        );
        assert_eq!(output.stdout, expected);
    }
    let failure = run(
        &[
            "--program",
            r#"report 7; report group_by([(machine: "b")], "absent");"#,
            data_path.to_str().unwrap(),
        ],
        b"",
    );
    assert!(!failure.status.success() && failure.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&failure.stderr);
    assert!(
        diagnostic.contains("missing record field: absent") && diagnostic.contains("bytes 44..52"),
        "{diagnostic}"
    );
    assert_eq!(std::fs::read(&data_path).unwrap(), data);
    std::fs::remove_file(data_path).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_composition_inline_and_file_keep_results_status_and_buffering() {
    let dir = std::env::temp_dir().join(format!("zincite-query-native-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let model_path = dir.join("model.mzn");
    let program_path = dir.join("report.zq");
    let source = include_bytes!("fixtures/native-composition.mzn");
    let program_source = include_str!("fixtures/native-composition.zq");
    std::fs::write(&model_path, source).unwrap();
    std::fs::write(&program_path, program_source).unwrap();
    let library = dir.join("library");
    let common = ["--stdlib-dir", library.to_str().unwrap()];
    let inline = run(
        &[
            common[0],
            common[1],
            "--program",
            program_source,
            model_path.to_str().unwrap(),
        ],
        b"",
    );
    let file = run(
        &[
            common[0],
            common[1],
            "--program-file",
            program_path.to_str().unwrap(),
            model_path.to_str().unwrap(),
        ],
        b"",
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

    let model = zincite_lint::load_model(
        &model_path,
        &zincite_lint::ModelOptions {
            include_dirs: vec![],
            stdlib_dir: Some(dir.join("library")),
        },
    );
    let input =
        zincite_query::Input::parse(source.to_vec(), zincite_syntax::FileMode::Model).unwrap();
    let program =
        zincite_query::Program::parse(program_source, zincite_query::ProgramLimits::default())
            .unwrap();
    let expected = program
        .evaluate_with_model(
            &input,
            Some(&model),
            zincite_query::ProgramLimits::default(),
        )
        .unwrap()
        .render()
        .unwrap();
    assert_eq!(inline.stdout, expected);

    let outputs = inline
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 3);
    let rows = outputs[0]["result"]["entries"].as_array().unwrap();
    for (row, (name, uses)) in rows.iter().zip([
        ("capacity", 2),
        ("reads", 0),
        ("shadow", 0),
        ("capacity", 1),
    ]) {
        assert_eq!(row["value"]["fields"][0]["value"]["value"], name);
        assert_eq!(row["value"]["fields"][1]["value"]["value"], uses);
    }
    assert_eq!(outputs[1]["result"].as_array().unwrap().len(), 3);
    assert_eq!(std::fs::read(&model_path).unwrap(), source);
    let incomplete = b"int: value = missing; solve satisfy;";
    std::fs::write(&model_path, incomplete).unwrap();
    let known = r#"source unresolved = query("references | declarations | names | count"); report match { from d in (subtree | declarations | unique); };"#;
    for (program_source, expected_status) in [
        (known.to_owned(), 1),
        (format!("{known} report 1 div 0;"), 2),
    ] {
        std::fs::write(&program_path, &program_source).unwrap();
        for arguments in [
            vec![
                common[0],
                common[1],
                "--program",
                &program_source,
                model_path.to_str().unwrap(),
            ],
            vec![
                common[0],
                common[1],
                "--program-file",
                program_path.to_str().unwrap(),
                model_path.to_str().unwrap(),
            ],
        ] {
            let output = run(&arguments, b"");
            assert_eq!(
                output.status.code(),
                Some(expected_status),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if expected_status == 1 {
                let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(value["complete"], false);
                assert_eq!(value["result"].as_array().unwrap().len(), 1);
            } else {
                assert!(output.stdout.is_empty());
            }
            assert_eq!(std::fs::read(&model_path).unwrap(), incomplete);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn bounded_paths_compact_inline_and_file_keep_native_json_and_buffer_failures() {
    let dir = std::env::temp_dir().join(format!("zincite-query-paths-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let model = dir.join("model.mzn");
    let file = dir.join("paths.zq");
    let source = include_bytes!("fixtures/bounded-paths.mzn");
    let program = include_str!("fixtures/bounded-paths.zq");
    std::fs::write(&model, source).unwrap();
    std::fs::write(&file, program).unwrap();
    let library = dir.join("library");
    let inline = run(
        &[
            "--stdlib-dir",
            library.to_str().unwrap(),
            "--program",
            program.trim(),
            model.to_str().unwrap(),
        ],
        b"",
    );
    let saved = run(
        &[
            "--stdlib-dir",
            library.to_str().unwrap(),
            "-f",
            file.to_str().unwrap(),
            model.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(
        inline.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&inline.stderr)
    );
    assert_eq!(saved.status.code(), Some(0));
    assert_eq!(inline.stdout, saved.stdout);
    let value: serde_json::Value = serde_json::from_slice(&inline.stdout).unwrap();
    assert_eq!(
        value["result"][0]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    let one = run(
        &[
            "--stdlib-dir",
            library.to_str().unwrap(),
            "--program",
            &program.replace("max_depth=2", "max_depth=1"),
            model.to_str().unwrap(),
        ],
        b"",
    );
    let value: serde_json::Value = serde_json::from_slice(&one.stdout).unwrap();
    assert_eq!(value["result"], serde_json::json!([]));
    let later = run(
        &[
            "--stdlib-dir",
            library.to_str().unwrap(),
            "--program",
            &format!("{}; report 1 div 0;", program.trim()),
            model.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(later.status.code(), Some(2));
    assert!(later.stdout.is_empty());
    let limited = run(
        &[
            "--stdlib-dir",
            library.to_str().unwrap(),
            "--max-results",
            "20",
            "--program",
            program,
            model.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(limited.status.code(), Some(2));
    assert!(limited.stdout.is_empty());
    assert_eq!(std::fs::read(&model).unwrap(), source);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn keyed_comparison_inline_and_file_match_library_and_buffer_alignment_errors() {
    use zincite_query::{Input, Program, ProgramLimits};
    let directory =
        std::env::temp_dir().join(format!("zincite-query-comparison-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let data_path = directory.join("comparison.dzn");
    let program_path = directory.join("comparison.zq");
    let data = include_bytes!("fixtures/keyed-comparison.dzn");
    std::fs::write(&data_path, data).unwrap();
    let input = Input::parse(data.to_vec(), zincite_syntax::FileMode::Data).unwrap();
    for source in [
        include_str!("fixtures/keyed-comparison.zq"),
        "report entries([|3,4|5,6|]);",
    ] {
        std::fs::write(&program_path, source).unwrap();
        let expected = Program::parse(source, ProgramLimits::default())
            .unwrap()
            .evaluate(&input, ProgramLimits::default())
            .unwrap()
            .render()
            .unwrap();
        for args in [
            vec!["--program", source, data_path.to_str().unwrap()],
            vec![
                "--program-file",
                program_path.to_str().unwrap(),
                data_path.to_str().unwrap(),
            ],
        ] {
            let output = run(&args, b"");
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
            assert_eq!(output.stdout, expected);
        }
    }
    let source = "report 7; report zip_exact([1:3,2:4], [2:3,3:4]);";
    let output = run(&["--program", source, data_path.to_str().unwrap()], b"");
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
