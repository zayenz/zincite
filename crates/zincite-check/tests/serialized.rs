use serde_json::{Value as Json, json};
use zincite_check::{Assignment, AssignmentEntry, AssignmentValue, PreparedModel, Status, Value};

// An executable can be loaded and checked even when syntax preparation is disabled.
fn executable() -> Json {
    let reference = json!({
        "name": "x", "position": null,
        "location": {"source": "large.fzn", "start": 0, "end": 10, "line": 1, "column": 1},
        "metadata": {"application": null, "label": null, "paths": [], "is_defined": false, "defines": null, "output": false},
        "aliases": []
    });
    json!({
        "format_version": 1,
        "model": {
            "slots": [{"kind": "integer", "default": null, "reference": reference}],
            "bindings": {"x": {"elements": [{"Variable": 0}], "is_array": false, "kind": "integer", "assignable": true}},
            "variables": [{"reference": reference, "slot": 0, "domains": [{"kind": "integer"}]}],
            "constraints": [{
                "reference": {
                    "ordinal": 1, "predicate": "int_lin_eq",
                    "location": {"source": "large.fzn", "start": 11, "end": 40, "line": 2, "column": 1},
                    "metadata": {"application": {"id": "large", "entities": ["9007199254740993"]}, "label": null, "paths": [], "is_defined": false, "defines": null, "output": false}
                },
                "predicate": {"Linear": {"coefficients": ["1"], "terms": [{"Variable": 0}], "bound": "9007199254740993", "equality": true}}
            }],
            "objective": ["minimize", {"Variable": 0}]
        }
    })
}

#[test]
fn parser_free_load_and_exact_integer_transport() {
    let loaded = PreparedModel::from_json(&executable().to_string()).unwrap();
    let exported: Json = serde_json::from_str(&loaded.to_json().unwrap()).unwrap();
    assert_eq!(exported, executable());
    let assignment = Assignment {
        entries: vec![AssignmentEntry {
            name: "x".into(),
            value: AssignmentValue::Scalar(Value::Integer(9_007_199_254_740_993)),
            location: None,
        }],
    };
    let report = loaded.check(&assignment).unwrap();
    assert_eq!(report.status, Status::Valid);
    assert_eq!(
        report.objective.unwrap().argument.value,
        Value::Integer(9_007_199_254_740_993)
    );
    let report = loaded
        .check(&Assignment {
            entries: vec![AssignmentEntry {
                name: "x".into(),
                value: AssignmentValue::Scalar(Value::Integer(0)),
                location: None,
            }],
        })
        .unwrap();
    let report = serde_json::to_value(report).unwrap();
    assert_eq!(
        report["violations"][0]["details"]["bound"],
        "9007199254740993"
    );
    assert_eq!(
        report["violations"][0]["check"]["constraint"]["metadata"]["application"]["entities"][0],
        "9007199254740993"
    );
}

#[test]
fn malformed_executables_fail_loading_before_checking() {
    let original = executable();
    for (pointer, replacement, message) in [
        ("/format_version", json!(2), "format version"),
        (
            "/model/constraints/0/predicate/Linear/terms/0/Variable",
            json!(7),
            "unknown slot",
        ),
        (
            "/model/constraints/0/predicate/Linear/terms/0",
            json!({"Literal": {"kind": "boolean", "value": true}}),
            "operand type",
        ),
        (
            "/model/bindings/x/elements/0",
            json!({"Literal": {"kind": "integer", "value": "1"}}),
            "variable bindings need slots",
        ),
        (
            "/model/constraints/0/predicate/Linear/coefficients",
            json!([]),
            "equal lengths",
        ),
        (
            "/model/constraints/0/reference/metadata/defines",
            json!("absent"),
            "unknown variable",
        ),
        (
            "/model/slots/0/default",
            json!({"kind": "integer", "value": "1"}),
            "slot default",
        ),
        (
            "/model/constraints/0/predicate/Linear/bound",
            json!(9007199254740993_i64),
            "string",
        ),
    ] {
        let mut malformed = original.clone();
        *malformed.pointer_mut(pointer).unwrap() = replacement;
        let error = PreparedModel::from_json(&malformed.to_string()).unwrap_err();
        assert!(
            error.message.contains(message),
            "{pointer}: {}",
            error.message
        );
    }
}

#[test]
fn parser_free_arithmetic_loading_checks_shapes_types_names_and_fixed_exponents() {
    let integer = |value: &str| json!({"Literal": {"kind": "integer", "value": value}});
    let mut original = executable();
    original["model"]["slots"][0]["reference"]["metadata"]["is_defined"] = json!(true);
    original["model"]["variables"][0]["reference"]["metadata"]["is_defined"] = json!(true);
    original["model"]["constraints"][0]["reference"]["metadata"]["defines"] = json!("x");
    original["model"]["constraints"][0]["reference"]["predicate"] = json!("fzn_int_plus");
    original["model"]["constraints"][0]["predicate"] = json!({"IntegerArithmetic": {
        "operation": "add", "inputs": [integer("9007199254740993"), integer("1")], "result": {"Variable": 0}
    }});
    let model = PreparedModel::from_json(&original.to_string()).unwrap();
    let report = model.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Valid);
    assert_eq!(
        report.objective.unwrap().argument.value,
        Value::Integer(9_007_199_254_740_994)
    );
    for (pointer, replacement, message) in [
        (
            "/model/constraints/0/predicate/IntegerArithmetic/inputs/0",
            json!({"Variable": 9}),
            "unknown slot",
        ),
        (
            "/model/constraints/0/predicate/IntegerArithmetic/inputs/0",
            json!({"Literal": {"kind": "boolean", "value": true}}),
            "operand type",
        ),
        (
            "/model/constraints/0/predicate/IntegerArithmetic/result",
            json!({"Literal": {"kind": "boolean", "value": false}}),
            "operand type",
        ),
        (
            "/model/constraints/0/predicate/IntegerArithmetic/result",
            json!({"Variable": 9}),
            "unknown slot",
        ),
        (
            "/model/constraints/0/predicate/IntegerArithmetic/inputs",
            json!([integer("1")]),
            "input count",
        ),
        (
            "/model/constraints/0/reference/predicate",
            json!("int_minus"),
            "report predicate",
        ),
        (
            "/model/constraints/0/reference/metadata/defines",
            json!("absent"),
            "unknown variable",
        ),
        (
            "/model/constraints/0/predicate/IntegerArithmetic/result",
            integer("1"),
            "does not occur",
        ),
    ] {
        let mut malformed = original.clone();
        *malformed.pointer_mut(pointer).unwrap() = replacement;
        let error = PreparedModel::from_json(&malformed.to_string()).unwrap_err();
        assert!(
            error.message.contains(message),
            "{pointer}: {}",
            error.message
        );
    }
    for (name, operation) in [
        ("int_abs", "absolute"),
        ("int_pow_fixed", "fixed_power"),
        ("gecode_int_pow", "gecode_power"),
    ] {
        let mut artifact = original.clone();
        artifact["model"]["constraints"][0]["reference"]["predicate"] = json!(name);
        artifact["model"]["constraints"][0]["predicate"]["IntegerArithmetic"]["operation"] =
            json!(operation);
        if name == "int_abs" {
            assert!(
                PreparedModel::from_json(&artifact.to_string())
                    .unwrap_err()
                    .message
                    .contains("input count")
            );
        } else {
            for exponent in [json!({"Variable": 0}), integer("-1"), integer("2147483647")] {
                artifact["model"]["constraints"][0]["predicate"]["IntegerArithmetic"]["inputs"]
                    [1] = exponent.clone();
                let loaded = PreparedModel::from_json(&artifact.to_string());
                if name == "int_pow_fixed" && exponent.get("Literal").is_some() {
                    assert!(loaded.is_ok());
                } else {
                    assert!(loaded.is_err(), "{name}: {exponent}");
                }
            }
        }
    }
}

#[cfg(feature = "prepare")]
#[test]
fn arithmetic_round_trips_preserve_typed_findings_text_and_completeness() {
    use zincite_check::{parse_assignment, prepare};
    let model = prepare("arithmetic.fzn", "var int:x;var 0..1:y::is_defined_var;constraint int_pow(x,3,y)::defines_var(y)::checker_constraint(\"cube\",[9007199254740993]);constraint int_div(y,2,0);solve minimize y;").unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for candidate in ["x=1;", "x=2;", "x=2;y=0;", "", "x=1;"] {
        let assignment = parse_assignment("candidate.dzn", candidate).unwrap();
        let original = model.check(&assignment).unwrap();
        let round_trip = loaded.check(&assignment).unwrap();
        assert_eq!(
            serde_json::to_value(&original).unwrap(),
            serde_json::to_value(&round_trip).unwrap()
        );
        assert_eq!(
            zincite_check::cli::render_text(&original),
            zincite_check::cli::render_text(&round_trip)
        );
        if candidate.contains("y=") {
            let text = zincite_check::cli::render_text(&original);
            assert!(text.contains("Power(x = 2, 3) = (y = 0)"), "{text}");
            let json = serde_json::to_value(&original).unwrap();
            assert_eq!(json["violations"][0]["details"]["operation"], "power");
            assert_eq!(
                json["violations"][0]["details"]["inputs"][0]["value"]["value"],
                "2"
            );
        }
    }
}

#[cfg(feature = "prepare")]
#[test]
fn round_trips_preserve_reports_metadata_and_incomplete_findings() {
    use zincite_check::{parse_assignment, prepare};
    let model = prepare(
        "model.fzn",
        include_str!("../../../examples/checker-compilation/model.fzn"),
    )
    .unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for candidate in [
        include_str!("../../../examples/checker-compilation/candidates/feasible.dzn"),
        include_str!("../../../examples/checker-compilation/candidates/infeasible.dzn"),
        include_str!("../../../examples/checker-compilation/candidates/incomplete.dzn"),
        include_str!("../../../examples/checker-compilation/candidates/incomplete-invalid.dzn"),
    ] {
        let assignment = parse_assignment("candidate.dzn", candidate).unwrap();
        let original = model.check(&assignment).unwrap();
        let round_trip = loaded.check(&assignment).unwrap();
        assert_eq!(
            serde_json::to_value(&original).unwrap(),
            serde_json::to_value(&round_trip).unwrap()
        );
        assert_eq!(
            zincite_check::cli::render_text(&original),
            zincite_check::cli::render_text(&round_trip)
        );
    }
    // Seed defaults can conflict with another alias domain and still produce a report.
    for source in [
        "var 0..3:x=5;solve satisfy;",
        "var int:c;var 3..3:b=c;var 2..2:a=b;solve minimize c;",
        "var int:x::is_defined_var;var 0..0:narrow=x;constraint int_lin_eq([1],[x],1)::defines_var(x);solve minimize x;",
        "var 0..0:x::is_defined_var;constraint int_lin_le([1],[x],0)::defines_var(x);solve satisfy;",
    ] {
        let model = prepare("defaults.fzn", source).unwrap();
        let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(model.check(&Assignment::default()).unwrap()).unwrap(),
            serde_json::to_value(loaded.check(&Assignment::default()).unwrap()).unwrap()
        );
    }
}

#[cfg(feature = "prepare")]
#[test]
fn set_global_round_trips_and_admission_validate_typed_compact_values() {
    use zincite_check::{parse_assignment, prepare};
    let model = prepare(
        "bench.fzn",
        include_str!("../../../examples/checker-compilation/shared-bench.fzn"),
    )
    .unwrap();
    let original: Json = serde_json::from_str(&model.to_json().unwrap()).unwrap();
    let loaded = PreparedModel::from_json(&original.to_string()).unwrap();
    for candidate in [
        include_str!("../../../examples/checker-compilation/candidates/shared-bench-feasible.dzn"),
        include_str!(
            "../../../examples/checker-compilation/candidates/shared-bench-infeasible.dzn"
        ),
        "start=[1,2];lead_tool=1;",
        "",
    ] {
        let assignment = parse_assignment("candidate.dzn", candidate).unwrap();
        let before = model.check(&assignment).unwrap();
        let after = loaded.check(&assignment).unwrap();
        assert_eq!(
            serde_json::to_value(&before).unwrap(),
            serde_json::to_value(&after).unwrap()
        );
        assert_eq!(
            zincite_check::cli::render_text(&before),
            zincite_check::cli::render_text(&after)
        );
    }
    for (pointer, replacement, message) in [
        (
            "/model/variables/2/domains/0/upper/ranges",
            json!([{"lower":"3","upper":"1"}]),
            "ranges",
        ),
        (
            "/model/variables/2/domains/0/upper/ranges",
            json!([{"lower":"1","upper":"2"},{"lower":"3","upper":"3"}]),
            "ranges",
        ),
        (
            "/model/variables/2/domains/0/upper/ranges/0/lower",
            json!(1),
            "string",
        ),
        (
            "/model/constraints/0/predicate/DisjunctiveStrict/durations/0",
            json!({"Variable": 0}),
            "fixed nonnegative",
        ),
        (
            "/model/constraints/0/predicate/DisjunctiveStrict/durations/0/Literal/value",
            json!("-1"),
            "fixed nonnegative",
        ),
        (
            "/model/constraints/0/predicate/DisjunctiveStrict/starts",
            json!([]),
            "equal lengths",
        ),
        (
            "/model/constraints/1/predicate/SetUnion/left",
            json!({"Literal":{"kind":"integer","value":"1"}}),
            "operand type",
        ),
        (
            "/model/constraints/1/reference/predicate",
            json!("set_intersect"),
            "report predicate",
        ),
        (
            "/model/slots/2/default",
            json!({"kind":"integer_set","value":{"ranges":[]}}),
            "slot default",
        ),
    ] {
        let mut malformed = original.clone();
        *malformed.pointer_mut(pointer).unwrap() = replacement;
        let error = PreparedModel::from_json(&malformed.to_string()).unwrap_err();
        assert!(
            error.message.contains(message),
            "{pointer}: {}",
            error.message
        );
    }
    // Assignment JSON uses the same validated representation as executable values.
    for ranges in [
        json!([{"lower":"2","upper":"1"}]),
        json!([{"lower":"2","upper":"2"},{"lower":"1","upper":"1"}]),
    ] {
        assert!(serde_json::from_value::<Assignment>(json!({"entries":[{"name":"s","value":{"kind":"scalar","value":{"kind":"integer_set","value":{"ranges":ranges}}}}]})).is_err());
    }
}
