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

#[test]
fn parser_free_relations_validate_names_conditions_and_linear_shapes() {
    let integer = |value: &str| json!({"Literal":{"kind":"integer","value":value}});
    let boolean = |value| json!({"Literal":{"kind":"boolean","value":value}});
    let mut original = executable();
    original["model"]["constraints"][0]["reference"]["predicate"] = json!("fzn_int_gt_imp");
    original["model"]["constraints"][0]["predicate"] = json!({"IntegerRelation":{
        "relation":"greater", "left":{"Variable":0}, "right":integer("9007199254740993"),
        "condition":{"Implied":{"guard":boolean(true)}}
    }});
    let candidate = Assignment {
        entries: vec![AssignmentEntry {
            name: "x".into(),
            value: AssignmentValue::Scalar(Value::Integer(9_007_199_254_740_993)),
            location: None,
        }],
    };
    for (name, predicate) in [
        (
            "fzn_int_gt_imp",
            original["model"]["constraints"][0]["predicate"].clone(),
        ),
        (
            "gecode_int_lin_ne_reif",
            json!({"LinearRelation":{
                "relation":"not_equal", "coefficients":["1"], "terms":[{"Variable":0}], "bound":"0",
                "condition":{"Reified":{"result":boolean(false)}}
            }}),
        ),
    ] {
        original["model"]["constraints"][0]["reference"]["predicate"] = json!(name);
        original["model"]["constraints"][0]["predicate"] = predicate;
        let loaded = PreparedModel::from_json(&original.to_string()).unwrap();
        let exported = loaded.to_json().unwrap();
        assert_eq!(serde_json::from_str::<Json>(&exported).unwrap(), original);
        let report = loaded.check(&candidate).unwrap();
        assert_eq!(report.status, Status::Invalid);
        let reloaded = PreparedModel::from_json(&exported).unwrap();
        assert_eq!(
            serde_json::to_value(report).unwrap(),
            serde_json::to_value(reloaded.check(&candidate).unwrap()).unwrap()
        );
    }
    for (pointer, replacement, message) in [
        (
            "/model/constraints/0/reference/predicate",
            json!("int_lin_ne_imp"),
            "report predicate",
        ),
        (
            "/model/constraints/0/reference/predicate",
            json!("gecode_gecode_int_lin_ne_reif"),
            "report predicate",
        ),
        (
            "/model/constraints/0/predicate/LinearRelation/relation",
            json!("less_equal"),
            "report predicate",
        ),
        (
            "/model/constraints/0/predicate/LinearRelation/relation",
            json!("invalid"),
            "unknown variant",
        ),
        (
            "/model/constraints/0/predicate/LinearRelation/coefficients",
            json!([]),
            "equal lengths",
        ),
        (
            "/model/constraints/0/predicate/LinearRelation/terms/0",
            boolean(true),
            "operand type",
        ),
        (
            "/model/constraints/0/predicate/LinearRelation/terms/0",
            json!({"Variable":20}),
            "unknown slot",
        ),
        (
            "/model/constraints/0/predicate/LinearRelation/condition/Reified/result",
            integer("1"),
            "operand type",
        ),
        (
            "/model/constraints/0/predicate/LinearRelation/condition",
            json!("Plain"),
            "report predicate",
        ),
        (
            "/model/constraints/0/predicate/LinearRelation/bound",
            json!(1),
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
    let mut scalar = executable();
    scalar["model"]["constraints"][0]["reference"]["predicate"] = json!("int_eq");
    scalar["model"]["constraints"][0]["predicate"] = json!({"IntegerRelation":{
        "relation":"equal", "left":{"Variable":0}, "right":integer("1"), "condition":"Plain"
    }});
    for (field, operand) in [("left", boolean(true)), ("right", json!({"Variable":30}))] {
        let mut malformed = scalar.clone();
        malformed["model"]["constraints"][0]["predicate"]["IntegerRelation"][field] = operand;
        assert!(PreparedModel::from_json(&malformed.to_string()).is_err());
    }
}

#[cfg(feature = "prepare")]
#[test]
fn completed_relation_reports_survive_serialization_with_legacy_shapes() {
    use zincite_check::{parse_assignment, prepare};
    let model = prepare("relations.fzn", "var int:x;var int:y::is_defined_var;var bool:b::is_defined_var;constraint int_eq(x,y)::defines_var(y);constraint int_lin_eq([1],[y],3);constraint int_le_reif(x,3,true);constraint int_lin_gt_reif([1],[y],2,b)::defines_var(b);constraint int_lt_imp(x,2,b);solve minimize y;").unwrap();
    let original: Json = serde_json::from_str(&model.to_json().unwrap()).unwrap();
    assert!(original["model"]["constraints"][1]["predicate"]["Linear"].is_object());
    assert!(original["model"]["constraints"][2]["predicate"]["ReifiedLessEqual"].is_object());
    let loaded = PreparedModel::from_json(&original.to_string()).unwrap();
    for candidate in ["x=1;", "x=3;", "x=3;y=1;b=true;", "", "x=1;"] {
        let candidate = parse_assignment("candidate.dzn", candidate).unwrap();
        let before = model.check(&candidate).unwrap();
        let after = loaded.check(&candidate).unwrap();
        assert_eq!(
            serde_json::to_value(&before).unwrap(),
            serde_json::to_value(&after).unwrap()
        );
        assert_eq!(
            zincite_check::cli::render_text(&before),
            zincite_check::cli::render_text(&after)
        );
    }
}

#[test]
fn parser_free_boolean_loading_rebuilds_definitions_and_validates_typed_contracts() {
    let boolean = |value| json!({"Literal":{"kind":"boolean","value":value}});
    let mut original = executable();
    let mut reference = original["model"]["slots"][0]["reference"].clone();
    reference["name"] = json!("b");
    reference["metadata"]["is_defined"] = json!(true);
    original["model"]["slots"]
        .as_array_mut()
        .unwrap()
        .push(json!({"kind":"boolean","default":null,"reference":reference}));
    original["model"]["variables"]
        .as_array_mut()
        .unwrap()
        .push(json!({"reference":reference,"slot":1,"domains":[{"kind":"boolean"}]}));
    original["model"]["bindings"]["b"] =
        json!({"elements":[{"Variable":1}],"is_array":false,"kind":"boolean","assignable":true});
    original["model"]["slots"][0]["reference"]["metadata"]["is_defined"] = json!(true);
    original["model"]["variables"][0]["reference"]["metadata"]["is_defined"] = json!(true);
    let reference = original["model"]["constraints"][0]["reference"].clone();
    let constraints = [
        ("gecode_bool_clause_imp", Some("b"), json!({"BooleanClause":{"positive":[],"negative":[],"condition":{"Reified":{"result":{"Variable":1}}}}})),
        ("bool_lin_eq", Some("x"), json!({"BooleanLinear":{"relation":"equal","coefficients":["9007199254740993"],"terms":[{"Variable":1}],"bound":{"Variable":0},"condition":"Plain"}})),
        ("fzn_bool_eq", None, json!({"BooleanRelation":{"relation":"equal","left":{"Variable":1},"right":boolean(false),"condition":"Plain"}})),
        ("bool_not", None, json!({"BooleanOperation":{"operation":"not","inputs":[{"Variable":1}],"condition":{"Reified":{"result":boolean(true)}}}})),
        ("fzn_bool2int", None, json!({"BooleanToInteger":{"boolean":{"Variable":1},"integer":{"Variable":0}}})),
    ].into_iter().enumerate().map(|(index,(name,defines,predicate))| {
        let mut reference = reference.clone();
        reference["ordinal"] = json!(index+1);
        reference["predicate"] = json!(name);
        reference["metadata"]["defines"] = json!(defines);
        json!({"reference":reference,"predicate":predicate})
    }).collect::<Vec<_>>();
    original["model"]["constraints"] = json!(constraints);
    let loaded = PreparedModel::from_json(&original.to_string()).unwrap();
    assert_eq!(
        loaded.check(&Assignment::default()).unwrap().status,
        Status::Valid
    );
    let candidate = Assignment {
        entries: vec![
            AssignmentEntry {
                name: "b".into(),
                value: AssignmentValue::Scalar(Value::Boolean(true)),
                location: None,
            },
            AssignmentEntry {
                name: "x".into(),
                value: AssignmentValue::Scalar(Value::Integer(2)),
                location: None,
            },
        ],
    };
    let report = loaded.check(&candidate).unwrap();
    assert_eq!(report.violations.len(), 5);
    let report = serde_json::to_value(report).unwrap();
    assert_eq!(
        report["violations"][1]["details"]["sum"],
        "9007199254740993"
    );
    assert_eq!(
        report["violations"][1]["details"]["bound"]["variable"]["name"],
        "x"
    );
    assert_eq!(
        serde_json::from_str::<Json>(&loaded.to_json().unwrap()).unwrap(),
        original
    );
    for (pointer, replacement, message) in [
        (
            "/model/constraints/0/reference/predicate",
            json!("bool_clause_imp_imp"),
            "report predicate",
        ),
        (
            "/model/constraints/0/predicate/BooleanClause/condition",
            json!({"Implied":{"guard":boolean(false)}}),
            "report predicate",
        ),
        (
            "/model/constraints/1/predicate/BooleanLinear/coefficients",
            json!([]),
            "equal lengths",
        ),
        (
            "/model/constraints/1/predicate/BooleanLinear/coefficients/0",
            json!(1),
            "string",
        ),
        (
            "/model/constraints/1/predicate/BooleanLinear/terms/0",
            json!({"Variable":0}),
            "operand type",
        ),
        (
            "/model/constraints/1/predicate/BooleanLinear/bound",
            boolean(true),
            "operand type",
        ),
        (
            "/model/constraints/1/predicate/BooleanLinear/condition",
            json!({"Reified":{"result":boolean(false)}}),
            "report predicate",
        ),
        (
            "/model/constraints/2/predicate/BooleanRelation/left",
            json!({"Variable":20}),
            "unknown slot",
        ),
        (
            "/model/constraints/2/predicate/BooleanRelation/relation",
            json!("not_equal"),
            "report predicate",
        ),
        (
            "/model/constraints/3/predicate/BooleanOperation/inputs",
            json!([]),
            "input count",
        ),
        (
            "/model/constraints/3/predicate/BooleanOperation/condition/Reified/result",
            json!({"Variable":0}),
            "operand type",
        ),
        (
            "/model/constraints/3/predicate/BooleanOperation/condition",
            json!("Plain"),
            "report predicate",
        ),
        (
            "/model/constraints/4/predicate/BooleanToInteger/boolean",
            json!({"Variable":0}),
            "operand type",
        ),
        (
            "/model/constraints/4/predicate/BooleanToInteger/integer",
            boolean(false),
            "operand type",
        ),
        (
            "/model/constraints/0/reference/metadata/defines",
            json!("x"),
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
    // A prefixed scalar XOR cannot use the bare predicate's asserted arity.
    let mut malformed = original.clone();
    malformed["model"]["constraints"][3]["reference"]["predicate"] = json!("fzn_bool_xor");
    malformed["model"]["constraints"][3]["predicate"] = json!({"BooleanOperation":{"operation":"xor","inputs":[boolean(false),boolean(true)],"condition":"Plain"}});
    assert!(PreparedModel::from_json(&malformed.to_string()).is_err());
}

#[cfg(feature = "prepare")]
#[test]
fn boolean_round_trips_preserve_reports_exact_values_and_text() {
    use zincite_check::{parse_assignment, prepare};
    let model = prepare("boolean.fzn", r#"
        var bool:b::is_defined_var;
        var int:x::is_defined_var;
        constraint bool_clause_imp([],[],b)::defines_var(b);
        constraint bool_lin_eq([9007199254740993],[b],x)::defines_var(x)::checker_constraint("weighted",[9007199254740993]);
        constraint bool_eq(b,false);
        constraint bool_not(b,true);
        constraint bool2int(b,x);
        solve minimize x;
    "#).unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for candidate in ["", "b=true;x=2;", "b=false;", "x=1;", ""] {
        let assignment = parse_assignment("candidate.dzn", candidate).unwrap();
        let before = model.check(&assignment).unwrap();
        let after = loaded.check(&assignment).unwrap();
        assert_eq!(
            serde_json::to_value(&before).unwrap(),
            serde_json::to_value(&after).unwrap()
        );
        let text = zincite_check::cli::render_text(&before);
        assert_eq!(text, zincite_check::cli::render_text(&after));
        if candidate == "b=true;x=2;" {
            assert_eq!(before.violations.len(), 5);
            for expected in [
                "clause positive = []",
                "negative = []",
                "sum = 9007199254740993",
                "x = 2",
                "Not([b = true])",
                "bool2int(b = true)",
                "entities=[9007199254740993]",
            ] {
                assert!(text.contains(expected), "{expected}: {text}");
            }
            let json = serde_json::to_value(before).unwrap();
            assert_eq!(
                json["violations"][0]["details"]["condition"]["kind"],
                "reified"
            );
            assert_eq!(
                json["violations"][1]["details"]["coefficients"][0],
                "9007199254740993"
            );
        }
    }
}
