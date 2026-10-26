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
    std::fs::write(&model, "var float:x;solve satisfy;").unwrap();
    let error = run("text");
    assert_eq!(error.status.code(), Some(2));
    assert!(error.stdout.is_empty());
    assert!(String::from_utf8_lossy(&error.stderr).contains("unsupported type"));
    std::fs::remove_dir_all(directory).unwrap();
}
