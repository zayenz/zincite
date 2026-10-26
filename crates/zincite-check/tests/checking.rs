#![cfg(feature = "prepare")]

use zincite_check::{
    Assignment, AssignmentEntry, AssignmentValue, CheckRef, Domain, ErrorKind, Status, Value,
    ViolationKind, parse_assignment, prepare,
};

const SPECIMEN: &str = include_str!("../../../examples/checker-compilation/model.fzn");

fn assignment(text: &str) -> Assignment {
    parse_assignment("candidate.dzn", text).unwrap()
}

#[test]
fn compiled_model_checks_repeated_complete_candidates_and_all_failures() {
    let model = prepare("model.fzn", SPECIMEN).unwrap();
    let feasible = assignment("start=[1,4]; finish=[4,6]; starts_early=true;");
    assert_eq!(model.check(&feasible).unwrap().status, Status::Valid);
    let infeasible = assignment("start=[8,5]; finish=[11,7]; starts_early=false;");
    let report = model.check(&infeasible).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(report.completeness.complete);
    assert_eq!(report.violations.len(), 3);
    assert!(
        report
            .violations
            .windows(2)
            .all(|pair| pair[0].check.location().start < pair[1].check.location().start)
    );
    let CheckRef::Domain { variable, domain } = &report.violations[0].check else {
        panic!("expected finish domain")
    };
    assert_eq!(variable.name, "X_INTRODUCED_19_");
    assert_eq!(*domain, Domain::Interval { lower: 3, upper: 9 });
    assert!(
        variable
            .aliases
            .iter()
            .any(|alias| alias.name == "finish" && alias.position == Some(1))
    );
    assert!(SPECIMEN[variable.location.start..variable.location.end].starts_with("var 3..9"));
    assert_eq!(variable.location.line, 9);
    assert_eq!(variable.metadata.application.as_ref().unwrap().id, "finish");
    assert_eq!(
        variable.metadata.application.as_ref().unwrap().entities,
        [1, 2]
    );
    assert!(!variable.metadata.paths.is_empty());
    for (violation, ordinal, id) in [
        (&report.violations[1], 3, "precedence"),
        (&report.violations[2], 4, "start_budget"),
    ] {
        let CheckRef::Constraint { constraint } = &violation.check else {
            panic!("expected constraint")
        };
        assert_eq!(constraint.ordinal, ordinal);
        assert_eq!(constraint.metadata.application.as_ref().unwrap().id, id);
        assert_eq!(
            constraint.metadata.application.as_ref().unwrap().entities,
            [1, 2]
        );
        assert!(constraint.metadata.label.is_some());
        assert!(
            SPECIMEN[constraint.location.start..constraint.location.end].starts_with("constraint")
        );
        let ViolationKind::Linear { terms, .. } = &violation.details else {
            panic!("expected typed operands")
        };
        assert!(
            terms
                .iter()
                .all(|term| matches!(term.value, Value::Integer(_)))
        );
    }
    let rendered = zincite_check::cli::render_text(&report);
    assert!(rendered.contains("finish[1]") && rendered.contains("entities=[1, 2]"));
    assert!(rendered.contains("Job 1 precedes job 2") && rendered.contains("mzn_path="));
    assert_eq!(model.check(&feasible).unwrap().status, Status::Valid);

    // A supplied defined output is checked by its relation, including full reification.
    let wrong = model
        .check(&assignment(
            "start=[8,5]; finish=[10,7]; starts_early=true;",
        ))
        .unwrap();
    assert_eq!(wrong.violations.len(), 5);
    assert!(matches!(
        wrong.violations.last().unwrap().details,
        ViolationKind::ReifiedLessEqual { .. }
    ));
}

#[test]
fn insufficient_input_is_independent_of_known_violations() {
    let model = prepare("model.fzn", SPECIMEN).unwrap();
    for (start, finish, early, status, violations) in [
        (1, 4, true, Status::Incomplete, 0),
        (8, 11, false, Status::Invalid, 1),
    ] {
        let report = model
            .check(&assignment(&format!(
                "X_INTRODUCED_17_={start}; X_INTRODUCED_19_={finish}; starts_early={early};"
            )))
            .unwrap();
        assert_eq!(report.status, status);
        assert_eq!(report.violations.len(), violations);
        assert!(!report.completeness.complete);
        assert!(
            report
                .completeness
                .unresolved_variables
                .iter()
                .any(|variable| variable.name == "start" && variable.position == Some(2))
        );
        let ordinals: Vec<_> = report
            .completeness
            .blocked_checks
            .iter()
            .filter_map(|blocked| match &blocked.check {
                CheckRef::Constraint { constraint } => Some(constraint.ordinal),
                _ => None,
            })
            .collect();
        assert_eq!(ordinals, [2, 3, 4]);
        assert!(
            report
                .completeness
                .blocked_checks
                .iter()
                .any(|blocked| matches!(&blocked.check, CheckRef::Domain { .. }))
        );
    }
    let primary_only = model.check(&assignment("start=[1,4];")).unwrap();
    assert_eq!(primary_only.status, Status::Incomplete);
    assert!(
        primary_only
            .completeness
            .unresolved_variables
            .iter()
            .any(|variable| variable.name == "starts_early")
    );
}

#[test]
fn aliases_domains_fixed_values_and_input_validation_work_together() {
    let model = prepare("aliases.fzn", "var int: x; var 1..1: narrow=x; var int: fixed=2; var {0,2}: choice; array[1..2] of var 0..3: a=[x,choice]; array[1..2] of var int: b=a; solve minimize x;").unwrap();
    let report = model.check(&assignment("choice=2;")).unwrap();
    assert_eq!(report.status, Status::Valid);
    assert_eq!(report.objective.unwrap().argument.value, Value::Integer(1));
    let supplied = Assignment {
        entries: vec![AssignmentEntry {
            name: "b".into(),
            value: AssignmentValue::Array(vec![Value::Integer(9), Value::Integer(1)]),
            location: None,
        }],
    };
    let report = model.check(&supplied).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(report.completeness.complete);
    assert_eq!(report.violations.len(), 3); // narrow, choice enumeration, array element domain
    assert_eq!(
        model
            .check(&assignment("choice=2; fixed=5;"))
            .unwrap()
            .violations
            .len(),
        1
    );
    for text in [
        "x=1;x=1;",
        "unknown=1;",
        "x=true;",
        "a=[1];",
        "a=1;",
        "x=1; b=[2,2];",
        "a=[1,2];b=[1,0];",
    ] {
        let error = model.check(&assignment(text)).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Assignment, "{text}");
        assert!(error.diagnostics[0].location.is_some());
    }
    let singleton = prepare("singleton.fzn", "var 4..4: x; solve satisfy;").unwrap();
    let report = singleton.check(&assignment("x=9;")).unwrap();
    assert_eq!(report.violations.len(), 1);
    assert!(matches!(
        report.violations[0].details,
        ViolationKind::Domain {
            value: Value::Integer(9)
        }
    ));
}

#[test]
fn unsupported_forms_fail_preparation_and_arithmetic_is_checked() {
    for text in [
        "var float: x; solve satisfy;",
        "var set of int: x; solve satisfy;",
        "var int:x; constraint int_plus(x,1,x); solve satisfy;",
        "var int:x; constraint int_lin_eq^-1([1],[x],0); solve satisfy;",
        "var int:x; constraint int_lin_eq⁻¹([1],[x],0); solve satisfy;",
        "var int:x; constraint int_lin_eq([1],[x],0)::checker_constraint^-1(\"id\",[1]); solve satisfy;",
        "var int:x; constraint int_lin_eq([1],[x],0)::checker_constraint⁻¹(\"id\",[1]); solve satisfy;",
        "var int:x; solve ::int_search^-1([x],input_order,indomain_min,complete) satisfy;",
        "var int:x; solve ::int_search⁻¹([x],input_order,indomain_min,complete) satisfy;",
        "var int:x; solve ::seq_search([int_search^-1([x],input_order,indomain_min,complete)]) satisfy;",
        "var int:x; solve ::seq_search([int_search⁻¹([x],input_order,indomain_min,complete)]) satisfy;",
        "var int:x; constraint int_le_reif(x,1,2); solve satisfy;",
        "var int:x; constraint int_lin_eq([1,2],[x],0); solve satisfy;",
        "var int:x; constraint int_lin_eq([x],[x],0); solve satisfy;",
        "var int:x; constraint int_lin_eq([1],[missing],0); solve satisfy;",
        "array[1..2] of int:x=[1]; solve satisfy;",
        "var int:x::mystery; solve satisfy;",
        "var int:x; constraint int_lin_eq([1],[x],0)::defines_var(x); solve satisfy;",
        "var int:x::is_defined_var; constraint int_lin_le([1],[x],0)::defines_var(x); solve satisfy;",
        "var int:x; var int:y=x+1; solve satisfy;",
        "var int:x; solve minimize x+1;",
        "var int:x; constraint int_lin_eq([1],[x],0)::checker_constraint(1,[2]); solve satisfy;",
        "var int:x; constraint int_lin_eq([1],[x],0)::defines_var([x]); solve satisfy;",
        "var int:x=y;var int:y=x;solve satisfy;",
    ] {
        let errors = prepare("bad.fzn", text).unwrap_err();
        assert!(errors[0].location.is_some(), "{text}");
    }
    for text in [
        "x=1+1;",
        "x=[1,true+false];",
        "x=-9223372036854775809;",
        "x=9223372036854775808;",
        "x=[[1]];",
    ] {
        assert!(parse_assignment("bad.dzn", text).is_err(), "{text}");
    }
    let model = prepare(
        "arithmetic.fzn",
        "var int:x; constraint int_lin_eq([1],[x],9223372036854775807); solve satisfy;",
    )
    .unwrap();
    let report = model.check(&assignment("x=9223372036854775807;")).unwrap();
    assert_eq!(report.status, Status::Valid);
    let model = prepare(
        "overflow.fzn",
        "var int:x; constraint int_lin_eq([2],[x],0); solve satisfy;",
    )
    .unwrap();
    let error = model
        .check(&assignment("x=9223372036854775807;"))
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Execution);
    assert_eq!(
        error.diagnostics[0].location.as_ref().unwrap().source,
        "overflow.fzn"
    );
    let min = prepare(
        "min.fzn",
        "var int:x; constraint int_lin_eq([1],[x],-9223372036854775808); solve satisfy;",
    )
    .unwrap();
    assert_eq!(
        min.check(&assignment("x=-9223372036854775808;"))
            .unwrap()
            .status,
        Status::Valid
    );
}
