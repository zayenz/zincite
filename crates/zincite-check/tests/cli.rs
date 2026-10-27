#![cfg(feature = "prepare")]

use std::process::Command;

#[test]
fn text_and_json_reports_cover_completeness_statuses_metadata_and_errors() {
    let directory = std::env::temp_dir().join(format!("zincite-check-cli-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = directory.join("model.fzn");
    let data = directory.join("candidate.dzn");
    std::fs::write(
        &model,
        include_str!("../../../examples/checker-compilation/model.fzn"),
    )
    .unwrap();
    let run = |format: &str| {
        Command::new(env!("CARGO_BIN_EXE_zincite-check"))
            .arg("--format")
            .arg(format)
            .arg(&model)
            .arg(&data)
            .output()
            .unwrap()
    };
    for (source, status, complete, exit) in [
        (
            include_str!("../../../examples/checker-compilation/candidates/feasible.dzn"),
            "valid",
            true,
            0,
        ),
        (
            include_str!("../../../examples/checker-compilation/candidates/infeasible.dzn"),
            "invalid",
            true,
            1,
        ),
        (
            include_str!("../../../examples/checker-compilation/candidates/incomplete-invalid.dzn"),
            "invalid",
            false,
            1,
        ),
        (
            include_str!("../../../examples/checker-compilation/candidates/incomplete.dzn"),
            "incomplete",
            false,
            1,
        ),
    ] {
        std::fs::write(&data, source).unwrap();
        let json = run("json");
        assert_eq!(json.status.code(), Some(exit));
        assert!(json.stderr.is_empty());
        let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
        assert_eq!(report["status"], status);
        assert_eq!(report["completeness"]["complete"], complete);
        let text = run("text");
        assert_eq!(text.status.code(), Some(exit));
        assert!(text.stderr.is_empty());
        let text = String::from_utf8(text.stdout).unwrap();
        assert!(text.starts_with(status));
        if status == "invalid" {
            assert_eq!(
                report["violations"][0]["details"]["value"]["kind"],
                "integer"
            );
            assert_eq!(report["violations"][0]["details"]["value"]["value"], "11");
            assert_eq!(
                report["violations"][0]["check"]["variable"]["metadata"]["application"]["id"],
                "finish"
            );
            assert!(text.contains("finish[1]") && text.contains("entities=[1, 2]"));
        }
        if !complete {
            assert!(
                !report["completeness"]["unresolved_variables"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                !report["completeness"]["blocked_checks"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(text.contains("unresolved:") && text.contains("blocked:"));
        }
    }
    std::fs::write(
        &model,
        include_str!("../../../examples/checker-compilation/shared-bench.fzn"),
    )
    .unwrap();
    std::fs::write(
        &data,
        include_str!(
            "../../../examples/checker-compilation/candidates/shared-bench-infeasible.dzn"
        ),
    )
    .unwrap();
    let output = run("json");
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["violations"].as_array().unwrap().len(), 5);
    assert_eq!(
        report["violations"][0]["details"]["value"]["kind"],
        "integer_set"
    );
    assert_eq!(
        report["violations"][0]["details"]["value"]["value"]["ranges"][1]["lower"],
        "5"
    );
    assert_eq!(
        report["violations"][3]["details"]["conflicts"],
        serde_json::json!([{"first":1,"second":2}])
    );
    let output = run("text");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("conflicting positions: 1, 2") && text.contains("value = {1..5}"));
    assert!(text.contains("id=\"shared_bench\""));
    std::fs::write(&model, "constraint int_gt_reif(2,1,false);constraint int_lin_ge_imp([1],[2],3,true);solve satisfy;").unwrap();
    std::fs::write(&data, "").unwrap();
    let output = run("text");
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("(2) > (1) <-> (false)"));
    assert!(text.contains("2 >= 3 when (true)"));
    let output = run("json");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["violations"][0]["details"]["condition"]["kind"],
        "reified"
    );
    assert_eq!(
        report["violations"][1]["details"]["condition"]["kind"],
        "implied"
    );
    std::fs::write(&model, "var int: y; constraint array_int_element(0,[],y); constraint array_int_element(1,[3],4); constraint gecode_bool_ite(false,true,false,true); solve satisfy;").unwrap();
    let output = run("text");
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("element indices [0] outside one-based positions (length 0)"));
    assert!(text.contains("selected 3 differs from result 4"));
    assert!(text.contains("condition false selects false, which differs from result true"));
    std::fs::write(&data, "missing=1;").unwrap();
    let error = run("json");
    assert_eq!(error.status.code(), Some(2));
    assert!(error.stdout.is_empty());
    assert!(String::from_utf8_lossy(&error.stderr).contains("candidate.dzn:1:1"));
    std::fs::write(
        &model,
        "var int:x;constraint int_lin_eq([2],[x],0);solve satisfy;",
    )
    .unwrap();
    std::fs::write(&data, "x=9223372036854775807;").unwrap();
    let error = run("json");
    assert_eq!(error.status.code(), Some(2));
    assert!(error.stdout.is_empty());
    assert!(String::from_utf8_lossy(&error.stderr).contains("overflow"));
    std::fs::write(&model, "var int:x;constraint int_lin_eq([1],[x],9223372036854775807)::checker_constraint(\"large\",[9007199254740993]);solve minimize x;").unwrap();
    let json = run("json");
    assert!(json.status.success());
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(
        report["objective"]["argument"]["value"]["value"],
        "9223372036854775807"
    );
    std::fs::write(&data, "x=0;").unwrap();
    let json = run("json");
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(
        report["violations"][0]["check"]["constraint"]["metadata"]["application"]["entities"][0],
        "9007199254740993"
    );
    std::fs::write(&model, "var string:x;solve satisfy;").unwrap();
    let error = run("text");
    assert_eq!(error.status.code(), Some(2));
    assert!(error.stdout.is_empty());
    assert!(String::from_utf8_lossy(&error.stderr).contains("unsupported type"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn export_and_load_check_the_same_candidates_and_refuse_overwrites() {
    let directory =
        std::env::temp_dir().join(format!("zincite-check-export-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = directory.join("model.fzn");
    let artifact = directory.join("model.json");
    let data = directory.join("candidate.dzn");
    let source = include_str!("../../../examples/checker-compilation/model.fzn");
    std::fs::write(&model, source).unwrap();
    let export = |output: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_zincite-check"))
            .arg("--export")
            .arg(output)
            .arg(&model)
            .output()
            .unwrap()
    };
    let output = export(&artifact);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let exported = std::fs::read(&artifact).unwrap();
    assert_eq!(export(&artifact).status.code(), Some(2));
    assert_eq!(std::fs::read(&artifact).unwrap(), exported);
    assert_eq!(export(&model).status.code(), Some(2));
    assert_eq!(std::fs::read_to_string(&model).unwrap(), source);
    for (source, exit) in [
        (
            include_str!("../../../examples/checker-compilation/candidates/feasible.dzn"),
            0,
        ),
        (
            include_str!("../../../examples/checker-compilation/candidates/infeasible.dzn"),
            1,
        ),
        (
            include_str!("../../../examples/checker-compilation/candidates/incomplete.dzn"),
            1,
        ),
        (
            include_str!("../../../examples/checker-compilation/candidates/incomplete-invalid.dzn"),
            1,
        ),
    ] {
        std::fs::write(&data, source).unwrap();
        let check = |load| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_zincite-check"));
            if load {
                command.arg("--load");
            }
            command
                .arg("--format")
                .arg("json")
                .arg(if load { &artifact } else { &model })
                .arg(&data)
                .output()
                .unwrap()
        };
        let original = check(false);
        let loaded = check(true);
        assert_eq!(original.status.code(), Some(exit));
        assert_eq!(loaded.status.code(), Some(exit), "{loaded:?}");
        assert!(loaded.stderr.is_empty());
        assert_eq!(loaded.stdout, original.stdout);
    }
    // Loaded checking no longer needs the source file.
    std::fs::remove_file(&model).unwrap();
    std::fs::write(&data, "start=[1,4];").unwrap();
    let loaded = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
        .arg("--load")
        .arg(&artifact)
        .arg(&data)
        .output()
        .unwrap();
    assert!(loaded.status.success(), "{loaded:?}");
    let mut malformed: serde_json::Value = serde_json::from_slice(&exported).unwrap();
    malformed["format_version"] = 42.into();
    std::fs::write(&artifact, malformed.to_string()).unwrap();
    let error = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
        .arg("--load")
        .arg(&artifact)
        .arg(&data)
        .output()
        .unwrap();
    assert_eq!(error.status.code(), Some(2));
    assert!(error.stdout.is_empty());
    assert!(String::from_utf8_lossy(&error.stderr).contains("format version"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn float_cli_flags_save_settings_and_override_only_the_named_component() {
    let directory =
        std::env::temp_dir().join(format!("zincite-check-floats-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("floats.fzn");
    let candidate = directory.join("candidate.dzn");
    let artifact = directory.join("floats.json");
    std::fs::write(
        &source,
        "var float:x;constraint float_eq(x,1.0);solve minimize x;",
    )
    .unwrap();
    std::fs::write(&candidate, "x=1.001;").unwrap();
    let run = |args: &[&std::ffi::OsStr]| {
        Command::new(env!("CARGO_BIN_EXE_zincite-check"))
            .args(args)
            .output()
            .unwrap()
    };
    let os = std::ffi::OsStr::new;
    assert_eq!(
        run(&[source.as_os_str(), candidate.as_os_str()])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        run(&[
            os("--float-absolute-tolerance"),
            os("0.01"),
            source.as_os_str(),
            candidate.as_os_str()
        ])
        .status
        .code(),
        Some(0)
    );
    assert_eq!(
        run(&[
            os("--float-absolute-tolerance"),
            os("0.01"),
            os("--float-relative-tolerance"),
            os("0"),
            os("--export"),
            artifact.as_os_str(),
            source.as_os_str()
        ])
        .status
        .code(),
        Some(0)
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&artifact).unwrap()).unwrap();
    assert_eq!(
        json["model"]["float_tolerance"],
        serde_json::json!({"absolute":0.01,"relative":0.0})
    );
    assert_eq!(
        run(&[os("--load"), artifact.as_os_str(), candidate.as_os_str()])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(
        run(&[
            os("--load"),
            os("--float-relative-tolerance"),
            os("0"),
            artifact.as_os_str(),
            candidate.as_os_str()
        ])
        .status
        .code(),
        Some(0)
    );
    assert_eq!(
        run(&[
            os("--load"),
            os("--float-absolute-tolerance"),
            os("0"),
            artifact.as_os_str(),
            candidate.as_os_str()
        ])
        .status
        .code(),
        Some(1)
    );
    for invalid in ["-1", "NaN", "inf", "1e999", "missing"] {
        assert_eq!(
            run(&[
                os("--float-relative-tolerance"),
                os(invalid),
                source.as_os_str(),
                candidate.as_os_str()
            ])
            .status
            .code(),
            Some(2)
        );
    }
    assert_eq!(
        run(&[os("--float-absolute-tolerance")]).status.code(),
        Some(2)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn float_function_cli_reports_typed_findings_and_located_domain_errors() {
    let directory =
        std::env::temp_dir().join(format!("zincite-check-functions-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = directory.join("functions.fzn");
    let data = directory.join("candidate.dzn");
    let artifact = directory.join("functions.json");
    std::fs::write(
        &model,
        include_str!("../../../examples/checker-compilation/float-functions.fzn"),
    )
    .unwrap();
    let exported = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
        .arg("--export")
        .arg(&artifact)
        .arg(&model)
        .output()
        .unwrap();
    assert!(exported.status.success());
    std::fs::write(&data, "x=4.0;cube=3.0;smallest=7.0;").unwrap();
    for loaded in [false, true] {
        for format in ["json", "text"] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_zincite-check"));
            if loaded {
                command.arg("--load");
            }
            let result = command
                .args(["--format", format])
                .arg(if loaded { &artifact } else { &model })
                .arg(&data)
                .output()
                .unwrap();
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stderr.is_empty());
            if format == "json" {
                let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
                assert_eq!(
                    report["violations"][0]["details"]["kind"],
                    "float_integer_power"
                );
                assert_eq!(report["violations"][0]["details"]["exponent"], "3");
                assert_eq!(
                    report["violations"][1]["details"]["kind"],
                    "float_array_extremum"
                );
            } else {
                let text = String::from_utf8(result.stdout).unwrap();
                assert!(text.contains("integer power") && text.contains("array Minimum"));
            }
        }
    }
    std::fs::write(&data, "x=-1.0;").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
        .args(["--load", "--format", "json"])
        .arg(&artifact)
        .arg(&data)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    let error = String::from_utf8(result.stderr).unwrap();
    assert!(error.contains("functions.fzn:8:1: sqrt requires a nonnegative argument"));
    assert!(result.stdout.is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn counting_cli_preserves_typed_conflicts_and_array_definitions_after_loading() {
    let directory =
        std::env::temp_dir().join(format!("zincite-check-counts-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("counting.fzn");
    let artifact = directory.join("counting.json");
    let data = directory.join("candidate.dzn");
    std::fs::write(
        &source,
        include_str!("../../../examples/checker-compilation/counting.fzn"),
    )
    .unwrap();
    let executable = env!("CARGO_BIN_EXE_zincite-check");
    assert!(
        Command::new(executable)
            .arg("--export")
            .arg(&artifact)
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    for candidate in ["x=1;", "x=1;ones=9;", ""] {
        std::fs::write(&data, candidate).unwrap();
        let run = |model: &std::path::Path, load: bool, format: &str| {
            let mut command = Command::new(executable);
            if load {
                command.arg("--load");
            }
            command
                .args(["--format", format])
                .arg(model)
                .arg(&data)
                .output()
                .unwrap()
        };
        let source_report = run(&source, false, "json");
        let loaded_report = run(&artifact, true, "json");
        assert_eq!(source_report.status.code(), loaded_report.status.code());
        assert_eq!(source_report.stdout, loaded_report.stdout);
        if candidate.contains("ones") {
            let report: serde_json::Value = serde_json::from_slice(&source_report.stdout).unwrap();
            assert_eq!(report["violations"][0]["details"]["kind"], "cardinality");
            assert_eq!(
                report["violations"][0]["details"]["counts"][0]["calculated"],
                "2"
            );
            let text = String::from_utf8(run(&artifact, true, "text").stdout).unwrap();
            assert!(
                text.contains("count[1] for 1 calculated 2")
                    && text.contains("supplied ones")
                    && text.contains("= 9")
            );
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn ordering_cli_preserves_completion_exact_indexes_and_typed_faults() {
    let directory =
        std::env::temp_dir().join(format!("zincite-check-ordering-cli-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = directory.join("ordering.fzn");
    let data = directory.join("candidate.dzn");
    let artifact = directory.join("ordering.json");
    std::fs::write(
        &model,
        include_str!("../../../examples/checker-compilation/ordering.fzn"),
    )
    .unwrap();
    let export = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
        .args(["--export"])
        .arg(&artifact)
        .arg(&model)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    for (candidate, status, complete) in [
        (
            "input=[3,1,2];selected=9007199254740994;forward=[7,5,6];",
            "valid",
            true,
        ),
        (
            "input=[3,1,2];selected=9007199254740994;forward=[7,5,6];permutation=[-2,-2,9223372036854775807];flags=[true,true,false];",
            "invalid",
            true,
        ),
        ("flags=[false,false,false];", "invalid", false),
        ("", "incomplete", false),
    ] {
        std::fs::write(&data, candidate).unwrap();
        let source = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
            .args(["--format", "json"])
            .arg(&model)
            .arg(&data)
            .output()
            .unwrap();
        let loaded = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
            .args(["--load", "--format", "json"])
            .arg(&artifact)
            .arg(&data)
            .output()
            .unwrap();
        assert_eq!(source.stdout, loaded.stdout);
        assert!(
            source.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&source.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&source.stdout).unwrap();
        assert_eq!(report["status"], status);
        assert_eq!(report["completeness"]["complete"], complete);
        if status == "valid" {
            assert_eq!(
                report["objective"]["argument"]["value"]["value"],
                "9007199254740994"
            );
        }
        if status == "invalid" {
            let text = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
                .args(["--load"])
                .arg(&artifact)
                .arg(&data)
                .output()
                .unwrap();
            assert!(
                String::from_utf8(text.stdout)
                    .unwrap()
                    .contains("one-hot channel")
            );
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn table_text_and_json_use_typed_membership_and_conditions() {
    let directory = std::env::temp_dir().join(format!("zincite-table-cli-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = directory.join("tables.fzn");
    let data = directory.join("candidate.dzn");
    std::fs::write(
        &model,
        include_str!("../../../examples/checker-compilation/tables.fzn"),
    )
    .unwrap();
    std::fs::write(&data, "x=9007199254740993;flag=true;member=false;").unwrap();
    for format in ["text", "json"] {
        let output = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
            .args(["--format", format])
            .arg(&model)
            .arg(&data)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        if format == "text" {
            assert!(text.contains("table membership true"));
            assert!(text.contains("rows of width 2"));
            assert!(text.contains("9007199254740993"));
            assert!(text.contains("<-> (member = false)"));
        } else {
            let report: serde_json::Value = serde_json::from_str(&text).unwrap();
            let finding = &report["violations"][0];
            assert_eq!(finding["details"]["kind"], "table");
            assert_eq!(finding["details"]["table"]["values"][0], "9007199254740993");
            assert_eq!(finding["details"]["width"], 2);
            assert_eq!(
                finding["check"]["constraint"]["metadata"]["application"]["id"],
                "integer_table"
            );
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn automaton_cli_source_export_load_and_typed_text_agree() {
    let directory =
        std::env::temp_dir().join(format!("zincite-automaton-cli-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("automata.fzn");
    let saved = directory.join("automata.json");
    let candidate = directory.join("candidate.dzn");
    std::fs::write(
        &source,
        include_str!("../../../examples/checker-compilation/automata.fzn"),
    )
    .unwrap();
    let export = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
        .arg("--export")
        .arg(&saved)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    for (input, status) in [
        ("x=9007199254740993;y=1;a=-1;b=0;", "valid"),
        ("x=9007199254740992;y=2;a=-1;b=-1;", "invalid"),
        ("y=2;", "invalid"),
        ("", "incomplete"),
    ] {
        std::fs::write(&candidate, input).unwrap();
        let run = |path: &std::path::Path, load: bool, format: &str| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_zincite-check"));
            if load {
                command.arg("--load");
            }
            command
                .args(["--format", format])
                .arg(path)
                .arg(&candidate)
                .output()
                .unwrap()
        };
        let native = run(&source, false, "json");
        let loaded = run(&saved, true, "json");
        assert_eq!(native.status.code(), loaded.status.code());
        let report: serde_json::Value = serde_json::from_slice(&native.stdout).unwrap();
        assert_eq!(report["status"], status);
        assert_eq!(
            report,
            serde_json::from_slice::<serde_json::Value>(&loaded.stdout).unwrap()
        );
        if input.contains("9007199254740992") {
            let text = String::from_utf8(run(&saved, true, "text").stdout).unwrap();
            for expected in [
                "symbol 9007199254740992 at position 1",
                "alphabet 9007199254740993..9007199254740993",
                "dead path at position 1 on symbol 2",
                "nonaccepting end",
                "accepting {3}",
                "exact_word",
                "Exact symbol",
            ] {
                assert!(text.contains(expected), "{text}");
            }
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn circuit_cli_source_export_load_and_typed_text_agree() {
    let directory =
        std::env::temp_dir().join(format!("zincite-circuit-cli-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("circuits.fzn");
    let saved = directory.join("circuits.json");
    let candidate = directory.join("candidate.dzn");
    std::fs::write(
        &source,
        include_str!("../../../examples/checker-compilation/circuits.fzn"),
    )
    .unwrap();
    let export = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
        .arg("--export")
        .arg(&saved)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    for (input, status) in [
        ("a=1;b=0;p=9007199254740994;q=9007199254740993;", "valid"),
        (
            "a=0;b=1;p=9007199254740993;q=9007199254740994;costs=[8,9];total=16;scalar_total=4;",
            "invalid",
        ),
        ("a=0;", "invalid"),
        ("", "incomplete"),
    ] {
        std::fs::write(&candidate, input).unwrap();
        let run = |path: &std::path::Path, load: bool, format: &str| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_zincite-check"));
            if load {
                command.arg("--load");
            }
            command
                .args(["--format", format])
                .arg(path)
                .arg(&candidate)
                .output()
                .unwrap()
        };
        let native = run(&source, false, "json");
        let loaded = run(&saved, true, "json");
        assert_eq!(native.status.code(), loaded.status.code());
        let report: serde_json::Value = serde_json::from_slice(&native.stdout).unwrap();
        assert_eq!(report["status"], status);
        assert_eq!(
            report,
            serde_json::from_slice::<serde_json::Value>(&loaded.stdout).unwrap()
        );
        if input.contains("costs=") {
            let text = String::from_utf8(run(&saved, true, "text").stdout).unwrap();
            for expected in [
                "proper cycle at positions [1]",
                "edge 1 -> 1 costs 0, supplied 8",
                "total cost is 17, supplied 16",
                "wide_cycle",
                "Wide circuit",
            ] {
                assert!(text.contains(expected), "{text}");
            }
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn packing_cli_source_load_and_typed_text_agree() {
    let directory =
        std::env::temp_dir().join(format!("zincite-packing-cli-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("packing.fzn");
    let saved = directory.join("packing.json");
    let candidate = directory.join("candidate.dzn");
    std::fs::write(
        &source,
        include_str!("../../../examples/checker-compilation/packing.fzn"),
    )
    .unwrap();
    let export = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
        .arg("--export")
        .arg(&saved)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    std::fs::write(
        &candidate,
        "bin=9007199254740993;other=0;x=9007199254740994;width=2;present=true;loads=[9,8];",
    )
    .unwrap();
    let run = |path: &std::path::Path, load: bool, format: &str| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_zincite-check"));
        if load {
            command.arg("--load");
        }
        command
            .args(["--format", format])
            .arg(path)
            .arg(&candidate)
            .output()
            .unwrap()
    };
    let report = run(&source, false, "json");
    let loaded = run(&saved, true, "json");
    assert_eq!(report.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&report.stdout).unwrap(),
        serde_json::from_slice::<serde_json::Value>(&loaded.stdout).unwrap()
    );
    let text = String::from_utf8(run(&saved, true, "text").stdout).unwrap();
    for expected in [
        "item 2 has invalid bin 0",
        "load 1 for bin 9007199254740993",
        "expected 3 from items [1]",
        "rectangles 1 and 2 overlap",
        "Exact bin loads",
        "loads",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn set_global_text_and_loaded_reports_preserve_exact_findings() {
    let directory =
        std::env::temp_dir().join(format!("zincite-set-global-cli-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("sets.fzn");
    let artifact = directory.join("sets.json");
    let candidate = directory.join("candidate.dzn");
    std::fs::write(
        &source,
        include_str!("../../../examples/checker-compilation/set-globals.fzn"),
    )
    .unwrap();
    std::fs::write(
        &candidate,
        "left={1};right={1};selected={9007199254740993};probe=9007199254740993;total=7;",
    )
    .unwrap();
    let export = Command::new(env!("CARGO_BIN_EXE_zincite-check"))
        .arg("--export")
        .arg(&artifact)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    for format in ["json", "text"] {
        let run = |load: bool| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_zincite-check"));
            if load {
                command.arg("--load");
            }
            command
                .args(["--format", format])
                .arg(if load { &artifact } else { &source })
                .arg(&candidate)
                .output()
                .unwrap()
        };
        let direct = run(false);
        let loaded = run(true);
        assert_eq!(direct.status.code(), Some(1));
        assert_eq!(loaded.status.code(), Some(1));
        assert!(direct.stderr.is_empty());
        assert!(loaded.stderr.is_empty());
        assert_eq!(direct.stdout, loaded.stdout);
        let text = String::from_utf8(direct.stdout).unwrap();
        if format == "text" {
            assert!(text.contains("set weighted sum: expected 9007199254740993"));
            assert!(text.contains("conflicting positions 1 and 2"));
        } else {
            let report: serde_json::Value = serde_json::from_str(&text).unwrap();
            let weights = report["violations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["details"]["finding"]["operation"] == "weights")
                .unwrap();
            assert_eq!(
                weights["details"]["finding"]["expected"],
                "9007199254740993"
            );
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}
