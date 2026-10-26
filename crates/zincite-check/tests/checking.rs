#![cfg(feature = "prepare")]

use zincite_check::{
    Assignment, AssignmentEntry, AssignmentValue, CheckRef, Domain, ErrorKind, Status, Value,
    ViolationKind, parse_assignment, prepare,
};

const SPECIMEN: &str = include_str!("../../../examples/checker-compilation/model.fzn");
const FEASIBLE: &str =
    include_str!("../../../examples/checker-compilation/candidates/feasible.dzn");
const INFEASIBLE: &str =
    include_str!("../../../examples/checker-compilation/candidates/infeasible.dzn");
const INCOMPLETE: &str =
    include_str!("../../../examples/checker-compilation/candidates/incomplete.dzn");
const INCOMPLETE_INVALID: &str =
    include_str!("../../../examples/checker-compilation/candidates/incomplete-invalid.dzn");

fn assignment(text: &str) -> Assignment {
    parse_assignment("candidate.dzn", text).unwrap()
}

#[test]
fn compiled_model_checks_repeated_complete_candidates_and_all_failures() {
    let model = prepare("model.fzn", SPECIMEN).unwrap();
    let feasible = assignment(FEASIBLE);
    assert_eq!(model.check(&feasible).unwrap().status, Status::Valid);
    let infeasible = assignment(INFEASIBLE);
    let report = model.check(&infeasible).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(report.completeness.complete);
    assert_eq!(report.violations.len(), 3);
    assert!(matches!(
        report.violations[0].details,
        ViolationKind::Domain {
            value: Value::Integer(11)
        }
    ));
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
    assert_eq!(
        model.check(&assignment(FEASIBLE)).unwrap().status,
        Status::Valid
    );
    for (source, status, violations) in [
        (INCOMPLETE, Status::Incomplete, 0),
        (INCOMPLETE_INVALID, Status::Invalid, 1),
    ] {
        let report = model.check(&assignment(source)).unwrap();
        assert_eq!(report.status, status);
        assert_eq!(report.violations.len(), violations);
        assert!(!report.completeness.complete);
        for name in ["start", "finish"] {
            assert!(
                report
                    .completeness
                    .unresolved_variables
                    .iter()
                    .any(|variable| { variable.name == name && variable.position == Some(2) })
            );
        }
        assert!(
            !report
                .completeness
                .unresolved_variables
                .iter()
                .any(|variable| {
                    variable.name == "starts_early"
                        || (matches!(variable.name.as_str(), "start" | "finish")
                            && variable.position == Some(1))
                })
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
    let empty = model.check(&Assignment::default()).unwrap();
    assert_eq!(empty.status, Status::Incomplete);
    assert_eq!(
        empty
            .completeness
            .blocked_checks
            .iter()
            .filter(|blocked| { matches!(blocked.check, CheckRef::Constraint { .. }) })
            .count(),
        5
    );
}

#[test]
fn shared_finish_metadata_identifies_both_job_positions() {
    let model = prepare("model.fzn", SPECIMEN).unwrap();
    let report = model.check(&assignment("start=[8,8];")).unwrap();
    assert!(report.completeness.complete);
    assert_eq!(report.violations.len(), 4);
    let jobs: Vec<_> = report
        .violations
        .iter()
        .filter_map(|violation| {
            let CheckRef::Domain { variable, .. } = &violation.check else {
                return None;
            };
            let application = variable.metadata.application.as_ref().unwrap();
            assert_eq!(application.id, "finish");
            assert_eq!(application.entities, [1, 2]);
            let position = variable
                .aliases
                .iter()
                .find(|alias| alias.name == "finish")
                .unwrap()
                .position
                .unwrap();
            let ViolationKind::Domain { value } = violation.details else {
                panic!("expected a domain value")
            };
            Some((position, application.entities[position - 1], value))
        })
        .collect();
    assert_eq!(
        jobs,
        [(1, 1, Value::Integer(11)), (2, 2, Value::Integer(10))]
    );
}

#[test]
fn definition_chains_use_computed_values_despite_singleton_aliases() {
    let first = "constraint int_lin_eq([1,-1],[middle,input],1)::defines_var(middle);";
    let second = "constraint int_lin_eq([1,-1],[total,narrow],1)::defines_var(total);";
    for definitions in [format!("{first}{second}"), format!("{second}{first}")] {
        let source = format!(
            "var int:input; var int:middle::is_defined_var; var 0..0:narrow=middle;
             var int:total::is_defined_var; {definitions}
             constraint int_lin_le([1],[total],1); solve minimize total;"
        );
        let model = prepare("chain.fzn", &source).unwrap();
        let report = model.check(&assignment("input=1;")).unwrap();
        assert_eq!(report.status, Status::Invalid);
        assert!(report.completeness.complete);
        assert_eq!(report.objective.unwrap().argument.value, Value::Integer(3));
        assert_eq!(report.violations.len(), 2);
        assert!(matches!(
            report.violations[0].details,
            ViolationKind::Domain {
                value: Value::Integer(2)
            }
        ));
        assert!(matches!(
            report.violations[1].details,
            ViolationKind::Linear { sum: 3, .. }
        ));

        let supplied = model.check(&assignment("input=1;narrow=0;")).unwrap();
        assert!(supplied.completeness.complete);
        assert_eq!(
            supplied.objective.unwrap().argument.value,
            Value::Integer(1)
        );
        assert_eq!(supplied.violations.len(), 1);
        let CheckRef::Constraint { constraint } = &supplied.violations[0].check else {
            panic!("expected the supplied output's defining relation")
        };
        assert_eq!(constraint.metadata.defines.as_deref(), Some("middle"));

        let missing = model.check(&Assignment::default()).unwrap();
        assert_eq!(missing.status, Status::Incomplete);
        assert!(missing.objective.is_none());
        assert!(
            missing
                .completeness
                .unresolved_variables
                .iter()
                .any(|variable| variable.name == "middle")
        );
    }
}

#[test]
fn cycles_need_supplied_values_and_do_not_hide_known_violations() {
    let model = prepare(
        "cycle.fzn",
        "var int:x::is_defined_var; var int:y::is_defined_var; var 0..1:flag;
        constraint int_lin_eq([1,-1],[x,y],0)::defines_var(x);
        constraint int_lin_eq([1,-1],[y,x],0)::defines_var(y);
        constraint int_lin_le([1],[flag],0); solve minimize y;",
    )
    .unwrap();
    let partial = model.check(&assignment("flag=1;")).unwrap();
    assert_eq!(partial.status, Status::Invalid);
    assert!(!partial.completeness.complete);
    assert_eq!(partial.violations.len(), 1);
    assert_eq!(partial.completeness.unresolved_variables.len(), 2);
    let supplied = model.check(&assignment("x=2;y=2;flag=0;")).unwrap();
    assert_eq!(supplied.status, Status::Valid);
    let changed = model.check(&assignment("x=3;flag=0;")).unwrap();
    assert_eq!(changed.status, Status::Valid);
    assert_eq!(changed.objective.unwrap().argument.value, Value::Integer(3));
    let removed = model.check(&assignment("flag=0;")).unwrap();
    assert_eq!(removed.status, Status::Incomplete);
    assert_eq!(removed.completeness.unresolved_variables.len(), 2);
    assert!(removed.objective.is_none());
}

#[test]
fn linear_completion_requires_a_supported_exact_output() {
    let model = prepare(
        "signed.fzn",
        "var int:input; var int:derived::is_defined_var;
        constraint int_lin_eq([-2,3],[derived,input],3)::defines_var(derived);
        solve minimize derived;",
    )
    .unwrap();
    let exact = model.check(&assignment("input=-1;")).unwrap();
    assert_eq!(exact.status, Status::Valid);
    assert_eq!(exact.objective.unwrap().argument.value, Value::Integer(-3));
    let nonexact = model.check(&assignment("input=0;")).unwrap();
    assert_eq!(nonexact.status, Status::Incomplete);
    assert_eq!(
        nonexact.completeness.unresolved_variables[0].name,
        "derived"
    );
    assert!(nonexact.objective.is_none());
    let supplied = model.check(&assignment("input=0;derived=-1;")).unwrap();
    assert_eq!(supplied.status, Status::Invalid);
    assert!(supplied.completeness.complete);

    for relation in [
        "int_lin_le([1],[derived],0)",
        "int_lin_eq([0],[derived],0)",
        "int_lin_eq([1,1],[derived,derived],0)",
        "int_le_reif(derived,0,true)",
    ] {
        let source = format!(
            "var 0..0:derived::is_defined_var; constraint {relation}::defines_var(derived); solve satisfy;"
        );
        let model = prepare("unsupported-direction.fzn", &source).unwrap();
        let missing = model.check(&Assignment::default()).unwrap();
        assert_eq!(missing.status, Status::Incomplete, "{relation}");
        assert_eq!(missing.completeness.unresolved_variables[0].name, "derived");
        assert_eq!(
            model.check(&assignment("derived=0;")).unwrap().status,
            Status::Valid,
            "{relation}"
        );
    }
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
        "var int:x::is_defined_var; solve satisfy;",
        "var int:x::is_defined_var; constraint int_lin_eq([1],[x],0)::defines_var(missing); solve satisfy;",
        "var int:x::is_defined_var; var int:y; constraint int_lin_eq([1],[y],0)::defines_var(x); solve satisfy;",
        "int:p=0; constraint int_lin_eq([1],[p],0)::defines_var(p); solve satisfy;",
        "var int:x::is_defined_var; var int:y::is_defined_var=x; constraint int_lin_eq([1],[x],0)::defines_var(x); constraint int_lin_eq([1],[y],0)::defines_var(y); solve satisfy;",
        "var int:x; var bool:b::is_defined_var; constraint int_le_imp(x,1,b)::defines_var(b); solve satisfy;",
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
    for bound in [i64::MIN, i64::MAX] {
        let source = format!(
            "var int:x::is_defined_var; constraint int_lin_eq([1],[x],{bound})::defines_var(x); solve minimize x;"
        );
        let model = prepare("boundary.fzn", &source).unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status, Status::Valid);
        assert_eq!(
            report.objective.unwrap().argument.value,
            Value::Integer(bound)
        );
    }
    for relation in [
        "int_lin_eq([1,2],[x,9223372036854775807],0)",
        "int_lin_eq([1,1,1],[x,9223372036854775807,1],0)",
        "int_lin_eq([1,-1],[x,1],9223372036854775807)",
        "int_lin_eq([-1],[x],-9223372036854775808)",
    ] {
        let source = format!(
            "var int:x::is_defined_var; constraint {relation}::defines_var(x); solve satisfy;"
        );
        let model = prepare("definition-overflow.fzn", &source).unwrap();
        let error = model.check(&Assignment::default()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution, "{relation}");
        let location = error.diagnostics[0].location.as_ref().unwrap();
        assert_eq!(location.source, "definition-overflow.fzn");
        assert!(source[location.start..location.end].starts_with("constraint"));
    }
}
