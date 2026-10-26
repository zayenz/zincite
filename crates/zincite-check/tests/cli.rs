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
