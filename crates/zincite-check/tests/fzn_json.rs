#![cfg(feature = "prepare")]

use serde_json::{Value as Json, json};
use zincite_check::{
    Assignment, PreparedModel, Status, Value, parse_assignment, prepare, prepare_json,
};

fn without_locations(value: Json) -> Json {
    match value {
        Json::Object(object) => Json::Object(
            object
                .into_iter()
                .filter(|(key, _)| key != "location")
                .map(|(key, value)| (key, without_locations(value)))
                .collect(),
        ),
        Json::Array(values) => Json::Array(values.into_iter().map(without_locations).collect()),
        value => value,
    }
}

#[test]
fn compiler_json_and_text_share_findings_completion_and_application_payloads() {
    let text = prepare(
        "model.fzn",
        include_str!("../../../examples/checker-compilation/model.fzn"),
    )
    .unwrap();
    let source = include_str!("../../../examples/checker-compilation/model.fzn.json");
    let json = prepare_json("model.fzn.json", source).unwrap();
    let loaded = PreparedModel::from_json(&json.to_json().unwrap()).unwrap();
    for candidate in [
        "start=[1,4];",
        "start=[8,5];",
        "X_INTRODUCED_17_=1;",
        "X_INTRODUCED_17_=8;",
        "start=[1,4];finish=[8,6];",
    ] {
        let assignment = parse_assignment("candidate.dzn", candidate).unwrap();
        let expected =
            without_locations(serde_json::to_value(text.check(&assignment).unwrap()).unwrap());
        for model in [&json, &loaded] {
            let report = model.check(&assignment).unwrap();
            assert_eq!(
                without_locations(serde_json::to_value(report).unwrap()),
                expected,
                "{candidate}"
            );
        }
    }
    let report = json
        .check(&parse_assignment("candidate", "start=[8,5];").unwrap())
        .unwrap();
    for violation in report.violations {
        let location = violation.check.location();
        assert_eq!(location.source, "model.fzn.json");
        assert_eq!(
            source[..location.start]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1,
            location.line
        );
        assert!(source[location.start..location.end].starts_with('{'));
    }
}

#[test]
fn json_fixed_initializers_and_propagation_annotations_match_text() {
    let text = prepare("fixed.fzn", "var int:x::is_defined_var=4;var int:y;constraint int_plus(y,1,x)::defines_var(x)::boundsZ::boundsR::boundsD::priority(2);constraint int_ne(x,4);solve minimize x;").unwrap();
    let source = r#"{
        "version": "1.0",
        "variables": {"x": {"type": "int", "rhs": 4, "defined": true}, "y": {"type": "int"}},
        "arrays": {},
        "constraints": [
            {"id": "int_plus", "args": ["y", 1, "x"], "defines": "x", "ann": ["boundsZ", "boundsR", "boundsD", {"id": "priority", "args": [2]}]},
            {"id": "int_ne", "args": ["x", 4]}
        ],
        "output": [],
        "solve": {"method": "minimize", "objective": "x"}
    }"#;
    let json = prepare_json("fixed.json", source).unwrap();
    let loaded = PreparedModel::from_json(&json.to_json().unwrap()).unwrap();
    for candidate in ["", "x=5;y=3;"] {
        let assignment = parse_assignment("candidate", candidate).unwrap();
        let expected =
            without_locations(serde_json::to_value(text.check(&assignment).unwrap()).unwrap());
        for model in [&json, &loaded] {
            assert_eq!(
                without_locations(serde_json::to_value(model.check(&assignment).unwrap()).unwrap()),
                expected
            );
        }
    }
}

#[test]
fn compiled_profiles_agree_on_float_set_objective_and_conflicting_results() {
    for (name, text, source, candidates) in [
        (
            "standard",
            include_str!("../../../examples/checker-compilation/standard-controls.fzn"),
            include_str!("../../../examples/checker-compilation/standard-controls.fzn.json"),
            [
                include_str!("../../../examples/checker-compilation/candidates/standard-valid.dzn"),
                include_str!(
                    "../../../examples/checker-compilation/candidates/standard-invalid.dzn"
                ),
                include_str!(
                    "../../../examples/checker-compilation/candidates/standard-conflict.dzn"
                ),
                include_str!(
                    "../../../examples/checker-compilation/candidates/standard-partial.dzn"
                ),
            ],
        ),
        (
            "gecode",
            include_str!("../../../examples/checker-compilation/gecode-controls.fzn"),
            include_str!("../../../examples/checker-compilation/gecode-controls.fzn.json"),
            [
                include_str!("../../../examples/checker-compilation/candidates/gecode-valid.dzn"),
                include_str!("../../../examples/checker-compilation/candidates/gecode-invalid.dzn"),
                include_str!(
                    "../../../examples/checker-compilation/candidates/gecode-conflict.dzn"
                ),
                include_str!("../../../examples/checker-compilation/candidates/gecode-partial.dzn"),
            ],
        ),
    ] {
        let text = prepare(name, text).unwrap();
        let json = prepare_json(name, source).unwrap();
        let loaded = PreparedModel::from_json(&json.to_json().unwrap()).unwrap();
        for candidate in candidates {
            let assignment = parse_assignment("candidate", candidate).unwrap();
            let expected =
                without_locations(serde_json::to_value(text.check(&assignment).unwrap()).unwrap());
            for model in [&json, &loaded] {
                assert_eq!(
                    without_locations(
                        serde_json::to_value(model.check(&assignment).unwrap()).unwrap()
                    ),
                    expected,
                    "{name}: {candidate}"
                );
            }
        }
    }
}

#[test]
fn exact_values_contextual_arrays_aliases_and_range_domains_survive_export() {
    let source = include_str!("../../../examples/checker-compilation/json-values.fzn.json");
    let model = prepare_json("values.json", source).unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    let assignment = parse_assignment("candidate", "x=9007199254740993;empty=[];").unwrap();
    for model in [&model, &loaded] {
        let report = model.check(&assignment).unwrap();
        assert_eq!(report.status, Status::Valid);
        assert!(report.completeness.complete);
        assert_eq!(
            report.objective.unwrap().argument.value,
            Value::Integer(9007199254740993)
        );
        let invalid = model
            .check(&parse_assignment("candidate", "x=0;result=5.0;").unwrap())
            .unwrap();
        assert_eq!(invalid.status, Status::Invalid);
        assert!(invalid.violations.len() >= 3);
    }
    let domains = r#"{"version":"1.0","variables":{"b":{"type":"bool","domain":[]},"f":{"type":"float","domain":[[0,1],[2,3]]},"i":{"type":"int","domain":[]}},"arrays":{},"constraints":[],"output":[],"solve":{"method":"satisfy"}}"#;
    let model = prepare_json("domains.json", domains).unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    let report = loaded
        .check(&parse_assignment("candidate", "b=false;f=1.5;i=0;").unwrap())
        .unwrap();
    assert_eq!(report.violations.len(), 3);
    assert_eq!(
        loaded.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
}

#[test]
fn malformed_compiler_json_rejects_shapes_types_references_and_operations() {
    let minimal = json!({"version":"1.0","variables":{},"arrays":{},"constraints":[],"output":[],"solve":{"method":"satisfy"}});
    let cases = [
        ("version", json!("2.0")),
        ("variables", json!({"x":{"type":"string"}})),
        (
            "variables",
            json!({"x":{"type":"int","rhs":9007199254740993.0}}),
        ),
        ("variables", json!({"x":{"type":"int","rhs":"absent"}})),
        (
            "variables",
            json!({"x":{"type":"int","rhs":"y"},"y":{"type":"int","rhs":"x"}}),
        ),
        (
            "variables",
            json!({"x":{"type":"int","domain":[[1,2],[2,3]]}}),
        ),
        ("variables", json!({"x":{"type":"float","domain":[[1,0]]}})),
        ("arrays", json!({"x":{"a":[true,1]}})),
        ("arrays", json!({"x":{"a":["absent"]}})),
        ("arrays", json!({"x":{"a":"y"},"y":{"a":"x"}})),
        ("constraints", json!([{"id":"unknown","args":[]}])),
        (
            "constraints",
            json!([{"id":"int_eq","args":[1,{"string":"1"}]}]),
        ),
        (
            "constraints",
            json!([{"id":"int_eq","args":[{"set":[[2,1]]},1]}]),
        ),
        (
            "constraints",
            json!([{"id":"float_eq","args":[1,1],"ann":["unknown"]}]),
        ),
        ("output", json!(["absent"])),
        ("solve", json!({"method":"maximize"})),
        ("solve", json!({"method":"satisfy","objective":1})),
    ];
    for (field, value) in cases {
        let mut input = minimal.clone();
        input[field] = value;
        let source = input.to_string();
        let errors = prepare_json("bad.json", &source).expect_err(&source);
        let location = errors[0].location.as_ref().unwrap();
        assert!(
            location.start <= location.end && location.end <= source.len(),
            "{source}"
        );
        assert_eq!(location.source, "bad.json");
    }
    let source = "{\r\"version\":\"1.0\",\"variables\":{},\"arrays\":{},\"constraints\":[{\"id\":\"unknown\",\"args\":[]}],\"output\":[],\"solve\":{\"method\":\"satisfy\"}}";
    let errors = prepare_json("cr.json", source).unwrap_err();
    let location = errors[0].location.as_ref().unwrap();
    assert_eq!(location.line, 2);
    assert!(source[location.start..location.end].starts_with('{'));
    for source in [
        r#"{"version":"1.0","version":"1.0"}"#,
        r#"{"version":"1.0","variables":{"x":{"type":"float","rhs":1e400}},"arrays":{},"constraints":[],"output":[],"solve":{"method":"satisfy"}}"#,
        r#"{"version":"1.0","variables":{"x":{"type":"int","rhs":9223372036854775808}},"arrays":{},"constraints":[],"output":[],"solve":{"method":"satisfy"}}"#,
    ] {
        assert!(prepare_json("bad.json", source).is_err());
    }
}
