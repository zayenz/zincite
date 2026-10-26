#![cfg(feature = "prepare")]

use zincite_check::{
    Assignment, AssignmentEntry, AssignmentValue, CheckRef, Domain, ErrorKind, PreparedModel,
    Status, Value, ViolationKind, parse_assignment, prepare,
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
            let ViolationKind::Domain { value } = &violation.details else {
                panic!("expected a domain value")
            };
            Some((position, application.entities[position - 1], value.clone()))
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
        "var int:x; constraint int_pow_unknown(x,1,x); solve satisfy;",
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
        "var int:x; var bool:b::is_defined_var; constraint int_between(x,1,b)::defines_var(b); solve satisfy;",
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

const BENCH: &str = include_str!("../../../examples/checker-compilation/shared-bench.fzn");
const BENCH_FEASIBLE: &str =
    include_str!("../../../examples/checker-compilation/candidates/shared-bench-feasible.dzn");
const BENCH_INFEASIBLE: &str =
    include_str!("../../../examples/checker-compilation/candidates/shared-bench-infeasible.dzn");

#[test]
fn workshop_sets_complete_and_report_independent_resource_and_domain_failures() {
    let model = prepare("bench.fzn", BENCH).unwrap();
    assert_eq!(
        model.check(&assignment(BENCH_FEASIBLE)).unwrap().status,
        Status::Valid
    );
    let report = model.check(&assignment(BENCH_INFEASIBLE)).unwrap();
    assert!(report.completeness.complete);
    assert_eq!(report.violations.len(), 5);
    assert!(matches!(
        report.violations[2].details,
        ViolationKind::Domain {
            value: Value::Integer(5)
        }
    ));
    let ViolationKind::DisjunctiveStrict { tasks, conflicts } = &report.violations[3].details
    else {
        panic!("expected the bench finding")
    };
    assert_eq!(
        conflicts,
        &[zincite_check::ConflictingPair {
            first: 1,
            second: 2
        }]
    );
    assert_eq!(tasks[0].start.value, Value::Integer(1));
    assert_eq!(tasks[1].duration.value, Value::Integer(2));
    let CheckRef::Constraint { constraint } = &report.violations[3].check else {
        panic!("expected constraint metadata")
    };
    assert_eq!(
        constraint.metadata.application.as_ref().unwrap().id,
        "shared_bench"
    );
    assert_eq!(
        constraint.metadata.application.as_ref().unwrap().entities,
        [1, 2]
    );
    assert!(constraint.metadata.label.is_some() && !constraint.metadata.paths.is_empty());
    assert!(matches!(
        report.violations[4].details,
        ViolationKind::SetIn { .. }
    ));
    let wrong = model
        .check(&assignment(&format!(
            "{BENCH_FEASIBLE} all_tools={{1}}; shared_tools={{}}; n_tools=2;"
        )))
        .unwrap();
    assert!(wrong.completeness.complete);
    assert!(
        wrong
            .violations
            .iter()
            .any(|finding| matches!(finding.details, ViolationKind::SetUnion { .. }))
    );
    assert!(
        wrong
            .violations
            .iter()
            .any(|finding| matches!(finding.details, ViolationKind::SetIntersection { .. }))
    );
    assert!(
        wrong
            .violations
            .iter()
            .any(|finding| matches!(finding.details, ViolationKind::SetCardinality { .. }))
    );
    let missing = model
        .check(&assignment("start=[1,2];lead_tool=1;"))
        .unwrap();
    assert_eq!(missing.status, Status::Invalid);
    assert!(!missing.completeness.complete);
    assert_eq!(missing.violations.len(), 1);
    assert!(
        missing
            .completeness
            .unresolved_variables
            .iter()
            .any(|variable| variable.name == "n_tools")
    );
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
    assert_eq!(
        model.check(&assignment(BENCH_FEASIBLE)).unwrap().status,
        Status::Valid
    );
}

#[test]
fn set_aliases_defaults_and_definition_directions_do_not_choose_members() {
    let model = prepare(
        "sets.fzn",
        "
        var set of 1..5:a; var set of 1..5:b::is_defined_var;
        var set of int:c::is_defined_var; var set of {}:narrow=c;
        var set of 1..1:choice; var set of {}:empty;
        var set of int:fixed={3,2,2};
        array[1..2] of var set of int:sets=[a,b];
        array[1..2] of var set of int:alias=sets;
        constraint set_eq(a,b)::defines_var(b);
        constraint set_eq(c,b)::defines_var(c);
        constraint set_subset(c,{1,2});
        constraint set_card(fixed,2);
        solve satisfy;",
    )
    .unwrap();
    let missing = model.check(&assignment("a={2,3};")).unwrap();
    assert_eq!(missing.status, Status::Invalid);
    assert!(!missing.completeness.complete);
    assert_eq!(missing.completeness.unresolved_variables.len(), 1);
    assert_eq!(missing.completeness.unresolved_variables[0].name, "choice");
    assert_eq!(missing.violations.len(), 2); // Empty alias upper bound and subset both check the derived set.
    assert!(matches!(
        missing.violations[1].details,
        ViolationKind::SetSubset { .. }
    ));
    let supplied = model
        .check(&assignment("alias=[{2,3},{1}];choice={};"))
        .unwrap();
    assert!(supplied.completeness.complete);
    assert!(
        supplied
            .violations
            .iter()
            .any(|finding| matches!(finding.details, ViolationKind::SetEqual { .. }))
    );
    assert_eq!(
        model.check(&assignment("a={};choice={1};")).unwrap().status,
        Status::Valid
    );
    assert!(model.check(&assignment("a={};alias=[{1},{}];")).is_err());
    assert!(model.check(&assignment("a=1;")).is_err());

    // Repeated output occurrences are checkable but cannot establish a value.
    let cyclic = prepare("self.fzn", "var set of {}:s::is_defined_var;constraint set_union(s,{},s)::defines_var(s);solve satisfy;").unwrap();
    assert_eq!(
        cyclic.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
    assert_eq!(
        cyclic.check(&assignment("s={};")).unwrap().status,
        Status::Valid
    );
}

#[test]
fn compact_intervals_and_cardinality_use_target_independent_checked_integers() {
    use zincite_check::IntegerSet;
    let max_cardinality = IntegerSet::interval(1, i64::MAX);
    assert_eq!(max_cardinality.cardinality(), Some(i64::MAX));
    let whole = IntegerSet::interval(i64::MIN, i64::MAX);
    assert_eq!(whole.cardinality(), None);
    assert!(whole.contains(i64::MIN) && whole.contains(i64::MAX));
    assert_eq!(whole.intersection(&max_cardinality), max_cardinality);
    assert_eq!(whole.union(&max_cardinality), whole);
    let encoded = serde_json::to_string(&whole).unwrap();
    assert!(encoded.len() < 100);
    assert_eq!(serde_json::from_str::<IntegerSet>(&encoded).unwrap(), whole);
    assert!(IntegerSet::interval(2, 1).is_empty());
    assert_eq!(
        IntegerSet::from_values([i64::MAX, i64::MAX - 1, i64::MIN, i64::MIN]),
        IntegerSet::interval(i64::MIN, i64::MIN)
            .union(&IntegerSet::interval(i64::MAX - 1, i64::MAX))
    );
    for (bounds, valid) in [
        ("1..9223372036854775807", true),
        ("0..9223372036854775807", false),
    ] {
        let source = format!(
            "set of int:s={bounds};var int:n::is_defined_var;constraint set_card(s,n)::defines_var(n);solve minimize n;"
        );
        let model = prepare("cardinality.fzn", &source).unwrap();
        if valid {
            assert_eq!(
                model
                    .check(&Assignment::default())
                    .unwrap()
                    .objective
                    .unwrap()
                    .argument
                    .value,
                Value::Integer(i64::MAX)
            );
        } else {
            for candidate in ["", "n=0;"] {
                let error = model.check(&assignment(candidate)).unwrap_err();
                assert_eq!(error.kind, ErrorKind::Execution);
                let location = error.diagnostics[0].location.as_ref().unwrap();
                assert!(source[location.start..location.end].starts_with("constraint"));
            }
        }
    }
}

const STRICT_DECLARATION: &str =
    "predicate fzn_disjunctive_strict(array[int] of var int:start,array[int] of var int:duration);";

#[test]
fn strict_disjunctive_handles_empty_zero_length_and_all_conflicting_pairs() {
    for (starts, durations, conflicts) in [
        ("[]", "[]", 0),
        ("[0,0,0]", "[2,2,2]", 3),
        ("[0,1]", "[3,0]", 1),
        ("[0,0]", "[3,0]", 0),
        ("[0,3]", "[3,0]", 0),
        ("[0,0]", "[0,0]", 0),
    ] {
        let source = format!(
            "{STRICT_DECLARATION} constraint fzn_disjunctive_strict({starts},{durations});solve satisfy;"
        );
        let report = prepare("strict.fzn", &source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap();
        assert!(report.completeness.complete);
        if conflicts == 0 {
            assert_eq!(report.status, Status::Valid, "{starts},{durations}");
        } else {
            let ViolationKind::DisjunctiveStrict {
                conflicts: pairs, ..
            } = &report.violations[0].details
            else {
                panic!("expected strict disjunctive finding")
            };
            assert_eq!(pairs.len(), conflicts);
        }
    }
    let source = format!(
        "{STRICT_DECLARATION}constraint fzn_disjunctive_strict([9223372036854775807],[1]);solve satisfy;"
    );
    let error = prepare("end.fzn", &source)
        .unwrap()
        .check(&Assignment::default())
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Execution);
    assert_eq!(
        error.diagnostics[0].location.as_ref().unwrap().source,
        "end.fzn"
    );
}

#[test]
fn malformed_set_calls_and_global_declarations_are_located() {
    for source in [
        "var set of int:s;constraint set_diff(s,{});solve satisfy;".into(),
        "var set of int:s;constraint set_in_reif(true,s,true);solve satisfy;".into(),
        "var set of bool:s;solve satisfy;".into(),
        "var set of int:s;constraint set_in(s,1);solve satisfy;".into(),
        "constraint set_union({},{});solve satisfy;".into(),
        "constraint fzn_disjunctive_strict([],[]);solve satisfy;".into(),
        format!("{STRICT_DECLARATION}{STRICT_DECLARATION}solve satisfy;"),
        format!("{}=true;solve satisfy;", STRICT_DECLARATION.trim_end_matches(';')),
        STRICT_DECLARATION.replace("var int:duration", "int:duration") + "solve satisfy;",
        "predicate mystery(array[int] of var int:start,array[int] of var int:duration);solve satisfy;".into(),
        format!("{STRICT_DECLARATION}constraint fzn_disjunctive_strict([1],[]);solve satisfy;"),
        format!("{STRICT_DECLARATION}constraint fzn_disjunctive_strict([1],[-1]);solve satisfy;"),
        format!("{STRICT_DECLARATION}var 1..1:d;constraint fzn_disjunctive_strict([1],[d]);solve satisfy;"),
    ] {
        let errors = prepare("bad-sets.fzn", &source).unwrap_err();
        assert!(errors[0].location.is_some(), "{source}");
    }
    for candidate in ["s={1+1};", "s=1..(1+1);", "s={true};"] {
        assert!(parse_assignment("bad.dzn", candidate).is_err());
    }
}

#[test]
fn arithmetic_spellings_check_supplied_outputs_and_complete_the_final_argument() {
    use zincite_check::PreparedModel;
    for (names, inputs, expected) in [
        ("int_abs gecode_int_abs fzn_int_abs", "-7", 7),
        ("int_negate gecode_int_negate fzn_int_negate", "7", -7),
        ("int_plus gecode_int_plus fzn_int_plus", "-7,3", -4),
        ("int_minus gecode_int_minus fzn_int_minus", "-7,3", -10),
        ("int_times gecode_int_times fzn_int_times", "-7,3", -21),
        ("int_div gecode_int_div fzn_int_div", "-7,3", -2),
        ("int_mod gecode_int_mod fzn_int_mod", "-7,3", -1),
        ("int_min gecode_int_min fzn_int_min", "-7,3", -7),
        ("int_max gecode_int_max fzn_int_max", "-7,3", 3),
        ("int_pow", "-2,3", -8),
        ("int_pow_fixed", "-2,-3", 0),
        (
            "gecode_int_pow gecode_gecode_int_pow fzn_gecode_int_pow",
            "-2,3",
            -8,
        ),
    ] {
        for name in names.split_whitespace() {
            let fixed = name.contains("pow") && name != "int_pow";
            let parameters = if !inputs.contains(',') {
                "var int: operand, var int: answer"
            } else if fixed {
                "var int: base, int: exponent, var int: answer"
            } else {
                "var int: first, var int: second, var int: answer"
            };
            let source = format!(
                "predicate {name}({parameters});var int:answer::is_defined_var;constraint {name}({inputs},answer)::defines_var(answer);solve minimize answer;"
            );
            let model = prepare("arithmetic.fzn", &source).unwrap();
            let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
            for model in [&model, &loaded] {
                let report = model.check(&Assignment::default()).unwrap();
                assert_eq!(report.status, Status::Valid, "{name}");
                assert_eq!(
                    report.objective.unwrap().argument.value,
                    Value::Integer(expected),
                    "{name}"
                );
                let report = model
                    .check(&assignment(&format!("answer={};", expected + 1)))
                    .unwrap();
                assert_eq!(report.status, Status::Invalid, "{name}");
                let ViolationKind::IntegerArithmetic { inputs, result, .. } =
                    &report.violations[0].details
                else {
                    panic!("typed arithmetic finding expected")
                };
                assert!(
                    inputs
                        .iter()
                        .all(|input| matches!(input.value, Value::Integer(_)))
                );
                assert_eq!(result.variable.as_ref().unwrap().name, "answer");
                assert_eq!(result.value, Value::Integer(expected + 1));
            }
        }
    }
}

#[test]
fn integer_arithmetic_boundaries_use_checked_signed_operations() {
    for (name, inputs, expected) in [
        ("int_div", "7,3", 2),
        ("int_div", "7,-3", -2),
        ("int_div", "-7,3", -2),
        ("int_div", "-7,-3", 2),
        ("int_mod", "7,3", 1),
        ("int_mod", "7,-3", 1),
        ("int_mod", "-7,3", -1),
        ("int_mod", "-7,-3", -1),
        ("int_mod", "-9223372036854775808,-1", 0),
        ("int_pow", "0,0", 1),
        ("int_pow", "0,9223372036854775807", 0),
        ("int_pow", "1,9223372036854775807", 1),
        ("int_pow", "-1,9223372036854775807", -1),
        ("int_pow", "-1,4294967296", 1),
        ("int_pow", "-2,63", i64::MIN),
        ("int_pow_fixed", "-2,63", i64::MIN),
        ("int_pow", "-9223372036854775808,1", i64::MIN),
        ("int_pow", "-2,-63", 0),
        ("int_pow", "-1,-9223372036854775807", -1),
        ("int_pow_fixed", "1,-4294967296", 1),
        ("gecode_int_pow", "-1,2147483646", 1),
    ] {
        let source = format!(
            "var int:r::is_defined_var;constraint {name}({inputs},r)::defines_var(r);solve minimize r;"
        );
        let model = prepare("limits.fzn", &source).unwrap();
        for candidate in [String::new(), format!("r={expected};")] {
            let report = model.check(&assignment(&candidate)).unwrap();
            assert_eq!(report.status, Status::Valid, "{name}({inputs})");
            assert_eq!(
                report.objective.unwrap().argument.value,
                Value::Integer(expected)
            );
        }
    }
    for (name, inputs, message) in [
        ("int_abs", "-9223372036854775808", "overflow"),
        ("int_negate", "-9223372036854775808", "overflow"),
        ("int_plus", "9223372036854775807,1", "overflow"),
        ("int_minus", "-9223372036854775808,1", "overflow"),
        ("int_times", "9223372036854775807,2", "overflow"),
        ("int_div", "1,0", "division by zero"),
        ("int_mod", "1,0", "division by zero"),
        ("int_div", "-9223372036854775808,-1", "overflow"),
        ("int_pow", "2,63", "overflow"),
        ("int_pow", "2,-63", "overflow"),
        ("int_pow_fixed", "2,-64", "overflow"),
        ("int_pow", "1,-9223372036854775808", "exponent magnitude"),
        ("int_pow", "0,-1", "zero base"),
    ] {
        let source = format!(
            "var int:r::is_defined_var;constraint {name}({inputs},r)::defines_var(r);solve satisfy;"
        );
        let model = prepare("overflow.fzn", &source).unwrap();
        for candidate in ["", "r=0;"] {
            let error = model.check(&assignment(candidate)).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Execution);
            assert!(
                error.diagnostics[0].message.contains(message),
                "{name}({inputs}): {error:?}"
            );
            assert_eq!(
                error.diagnostics[0].location.as_ref().unwrap().source,
                "overflow.fzn"
            );
        }
    }
}

#[test]
fn arithmetic_completion_feeds_downstream_checks_and_starts_fresh() {
    let source = "var int:x;var 0..1:y::is_defined_var;var int:z::is_defined_var;constraint int_times(y,3,z)::defines_var(z);constraint int_plus(x,2,y)::defines_var(y);constraint int_min(z,10,9);constraint int_plus(1,1,3);solve minimize z;";
    let model = prepare("chain.fzn", source).unwrap();
    for candidate in ["x=2;", "x=0;", "", "x=2;y=0;z=0;", "x=2;"] {
        let report = model.check(&assignment(candidate)).unwrap();
        assert_eq!(report.status, Status::Invalid);
        assert_eq!(report.completeness.complete, !candidate.is_empty());
        if candidate == "x=2;" {
            assert_eq!(report.violations.len(), 3);
            assert_eq!(report.objective.unwrap().argument.value, Value::Integer(12));
            assert!(matches!(
                report.violations[0].details,
                ViolationKind::Domain {
                    value: Value::Integer(4)
                }
            ));
        } else if candidate.is_empty() {
            assert_eq!(report.violations.len(), 1);
            assert_eq!(report.completeness.unresolved_variables.len(), 3);
        } else if candidate.contains("y=") {
            assert_eq!(report.violations.len(), 3);
            assert_eq!(report.objective.unwrap().argument.value, Value::Integer(0));
        }
    }
    for source in [
        "var int:x::is_defined_var;var int:alias=x;constraint int_plus(alias,1,x)::defines_var(x);solve satisfy;",
        "var int:x::is_defined_var;constraint int_minus(x,1,3)::defines_var(x);solve satisfy;",
        "var int:x::is_defined_var;var int:y::is_defined_var;constraint int_negate(y,x)::defines_var(x);constraint int_negate(x,y)::defines_var(y);solve satisfy;",
    ] {
        let model = prepare("direction.fzn", source).unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status, Status::Incomplete);
        assert!(!report.completeness.blocked_checks.is_empty());
        let supplied = model.check(&assignment("x=4;")).unwrap();
        assert!(supplied.completeness.complete);
    }
    let model = prepare("computed.fzn", "var 0..0:x::computed_domain;var int:y::is_defined_var;constraint int_plus(x,2,y)::defines_var(y);solve minimize y;").unwrap();
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
    assert_eq!(
        model
            .check(&assignment("x=4;"))
            .unwrap()
            .objective
            .unwrap()
            .argument
            .value,
        Value::Integer(6)
    );
}

#[test]
fn integer_relations_check_all_exact_names_and_reification_truth_tables() {
    for (relation, compare) in [
        ("eq", i64::eq as fn(&i64, &i64) -> bool),
        ("ne", i64::ne),
        ("lt", i64::lt),
        ("le", i64::le),
        ("gt", i64::gt),
        ("ge", i64::ge),
    ] {
        for linear in [false, true] {
            for suffix in ["", "_reif", "_imp"] {
                for prefix in ["", "gecode_", "fzn_"] {
                    let name = format!(
                        "{prefix}int_{}{relation}{suffix}",
                        if linear { "lin_" } else { "" }
                    );
                    let arguments = if linear { "[1],[x],0" } else { "x,0" };
                    let (boolean_decl, boolean_arg) = if suffix.is_empty() {
                        ("", "")
                    } else {
                        (",var bool: flag", ",b")
                    };
                    let signature = if linear {
                        "array[int] of int: cs,array[int] of var int: xs,int: bound"
                    } else {
                        "var int: lhs,var int: rhs"
                    };
                    let source = format!(
                        "predicate {name}({signature}{boolean_decl});var int:x;var bool:b;constraint {name}({arguments}{boolean_arg});solve satisfy;"
                    );
                    let model = prepare("relations.fzn", &source).unwrap();
                    for x in [-1, 0, 1] {
                        for b in [false, true] {
                            let truth = compare(&x, &0);
                            let satisfied = match suffix {
                                "_reif" => b == truth,
                                "_imp" => !b || truth,
                                _ => truth,
                            };
                            let report =
                                model.check(&assignment(&format!("x={x};b={b};"))).unwrap();
                            assert_eq!(
                                report.status,
                                if satisfied {
                                    Status::Valid
                                } else {
                                    Status::Invalid
                                },
                                "{name}: {x}, {b}"
                            );
                            assert!(report.completeness.complete);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn relation_completion_uses_only_unique_equalities_and_boolean_equivalence() {
    let model = prepare("completion.fzn", "var int:x;var 0..1:y::is_defined_var;var int:z::is_defined_var;var bool:b::is_defined_var;constraint int_eq(z,y)::defines_var(z);constraint fzn_int_lin_eq([2,-2],[y,x],4)::defines_var(y);constraint gecode_int_lin_gt_reif([1],[z],3,b)::defines_var(b);constraint int_ne(z,4);solve minimize z;").unwrap();
    for candidate in ["x=2;", "x=-1;", "", "x=2;y=1;z=1;b=true;", "x=2;"] {
        let report = model.check(&assignment(candidate)).unwrap();
        assert_eq!(report.completeness.complete, !candidate.is_empty());
        if candidate == "x=2;" {
            assert_eq!(report.status, Status::Invalid);
            assert_eq!(report.violations.len(), 2);
            assert!(matches!(
                report.violations[0].details,
                ViolationKind::Domain {
                    value: Value::Integer(4)
                }
            ));
            assert_eq!(report.objective.unwrap().argument.value, Value::Integer(4));
        } else if candidate == "x=-1;" {
            assert_eq!(report.status, Status::Valid);
            assert_eq!(report.objective.unwrap().argument.value, Value::Integer(1));
        } else if !candidate.is_empty() {
            assert_eq!(report.violations.len(), 2);
        }
    }
    for relation in [
        "int_eq(x,alias)",
        "int_lin_eq([1,1],[x,alias],4)",
        "int_lin_eq([0],[x],0)",
        "int_lin_eq([2],[x],3)",
        "int_ge(x,4)",
        "int_eq_reif(x,4,true)",
        "int_lin_ge_reif([1],[x],4,true)",
    ] {
        let source = format!(
            "var int:x::is_defined_var;var 0..0:alias=x;constraint {relation}::defines_var(x);solve satisfy;"
        );
        let model = prepare("direction.fzn", &source).unwrap();
        assert_eq!(
            model.check(&Assignment::default()).unwrap().status,
            Status::Incomplete,
            "{relation}"
        );
        assert!(
            model
                .check(&assignment("x=4;"))
                .unwrap()
                .completeness
                .complete
        );
    }
    for relation in ["int_ge_imp(4,5,b)", "int_lin_eq_imp([1],[4],5,b)"] {
        let source = format!(
            "var bool:b::is_defined_var;constraint {relation}::defines_var(b);solve satisfy;"
        );
        let model = prepare("guard.fzn", &source).unwrap();
        assert_eq!(
            model.check(&Assignment::default()).unwrap().status,
            Status::Incomplete
        );
        assert_eq!(
            model.check(&assignment("b=false;")).unwrap().status,
            Status::Valid
        );
        assert_eq!(
            model.check(&assignment("b=true;")).unwrap().status,
            Status::Invalid
        );
    }
    for relation in ["int_eq(x,7)", "int_eq(7,x)"] {
        let source = format!(
            "var int:x::is_defined_var;constraint {relation}::defines_var(x);solve minimize x;"
        );
        assert_eq!(
            prepare("copy.fzn", &source)
                .unwrap()
                .check(&Assignment::default())
                .unwrap()
                .objective
                .unwrap()
                .argument
                .value,
            Value::Integer(7)
        );
    }
}

#[test]
fn implication_short_circuits_and_linear_checks_use_source_order_and_i64_bounds() {
    let model = prepare("inactive.fzn", "var int:x;constraint int_lt_imp(x,0,false);constraint int_lin_eq_imp([2,1],[9223372036854775807,x],0,false);constraint int_lin_ne([],[],0);solve satisfy;").unwrap();
    let report = model.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.completeness.blocked_checks.len(), 1);
    assert!(matches!(
        report.completeness.blocked_checks[0].check,
        CheckRef::Domain { .. }
    ));
    assert!(
        model
            .check(&assignment("x=0;"))
            .unwrap()
            .completeness
            .complete
    );
    for relation in [
        "int_lt(-9223372036854775808,9223372036854775807)",
        "int_gt(9223372036854775807,-9223372036854775808)",
        "int_lin_lt([1],[-9223372036854775808],9223372036854775807)",
        "int_lin_gt([1],[9223372036854775807],-9223372036854775808)",
        "int_lin_eq_reif([],[],0,true)",
        "int_lin_lt_reif([],[],0,false)",
        "int_lin_eq_imp([2],[9223372036854775807],0,false)",
    ] {
        let source = format!("constraint {relation};solve satisfy;");
        assert_eq!(
            prepare("bounds.fzn", &source)
                .unwrap()
                .check(&Assignment::default())
                .unwrap()
                .status,
            Status::Valid,
            "{relation}"
        );
    }
    for relation in [
        "int_lin_eq_imp([2],[9223372036854775807],0,true)",
        "int_lin_ne_reif([2],[9223372036854775807],0,false)",
        "int_lin_lt([1,1,-1],[9223372036854775807,1,1],0)",
    ] {
        let source = format!("constraint {relation};solve satisfy;");
        let error = prepare("overflow.fzn", &source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution);
        assert!(error.diagnostics[0].message.contains("overflow"));
    }
}

#[test]
fn relation_source_admission_rejects_wrong_types_shapes_roles_and_names() {
    for source in [
        "predicate int_ge(var bool:a,var int:b);solve satisfy;",
        "predicate int_lin_lt_reif(array[int] of var int:a,array[int] of var int:b,int:c,var bool:d);solve satisfy;",
        "predicate int_eq_imp(var int:a,var int:b,bool:c);solve satisfy;",
        "constraint int_eq(1,2,true);solve satisfy;",
        "constraint int_ge_reif(1,2,1);solve satisfy;",
        "constraint int_ne([1],2);solve satisfy;",
        "constraint int_lin_lt_reif([1,2],[1],0,true);solve satisfy;",
        "var int:x;constraint int_lin_ge([1],[1],x);solve satisfy;",
        "var int:x;constraint int_lin_ne_imp([x],[1],0,false);solve satisfy;",
        "constraint gecode_gecode_int_eq(1,1);solve satisfy;",
        "constraint int_lte(1,2);solve satisfy;",
    ] {
        let errors = prepare("bad-relations.fzn", source).unwrap_err();
        assert!(errors[0].location.is_some(), "{source}");
    }
}

#[test]
fn boolean_exact_names_check_truth_tables_and_argument_conventions() {
    let mut names = std::collections::HashSet::new();
    let rows = [(false, false), (false, true), (true, false), (true, true)];
    for prefix in ["", "gecode_", "fzn_"] {
        for (relation, truth) in [
            ("eq", [true, false, false, true]),
            ("ne", [false, true, true, false]),
            ("lt", [false, true, false, false]),
            ("le", [true, true, false, true]),
            ("gt", [false, false, true, false]),
            ("ge", [true, false, true, true]),
        ] {
            for shape in ["", "lin_"] {
                for suffix in ["", "_reif", "_imp"] {
                    let name = format!("{prefix}bool_{shape}{relation}{suffix}");
                    names.insert(name.clone());
                    for ((left, right), holds) in rows.into_iter().zip(truth) {
                        for condition in [false, true] {
                            let inputs = if shape.is_empty() {
                                format!("{left},{right}")
                            } else {
                                // One Boolean term and an integer bound compare the same 0/1 values.
                                format!("[1],[{left}],{}", i64::from(right))
                            };
                            let args = if suffix.is_empty() {
                                inputs
                            } else {
                                format!("{inputs},{condition}")
                            };
                            let source = format!("constraint {name}({args});solve satisfy;");
                            let report = prepare("truth.fzn", &source)
                                .unwrap()
                                .check(&Assignment::default())
                                .unwrap();
                            let valid = match suffix {
                                "_reif" => holds == condition,
                                "_imp" => !condition || holds,
                                _ => holds,
                            };
                            assert_eq!(
                                report.status,
                                if valid {
                                    Status::Valid
                                } else {
                                    Status::Invalid
                                },
                                "{source}"
                            );
                        }
                    }
                }
            }
        }
        for (operation, truth) in [
            ("and", [false, false, false, true]),
            ("or", [false, true, true, true]),
            ("xor", [false, true, true, false]),
            ("left_imp", [true, false, true, true]),
            ("right_imp", [true, true, false, true]),
        ] {
            for suffix in if operation.ends_with("imp") {
                &[""][..]
            } else {
                &["", "_imp"][..]
            } {
                let name = format!("{prefix}bool_{operation}{suffix}");
                names.insert(name.clone());
                for ((left, right), holds) in rows.into_iter().zip(truth) {
                    for result in [false, true] {
                        let source =
                            format!("constraint {name}({left},{right},{result});solve satisfy;");
                        let report = prepare("operation.fzn", &source)
                            .unwrap()
                            .check(&Assignment::default())
                            .unwrap();
                        let valid = if *suffix == "_imp" {
                            !result || holds
                        } else {
                            result == holds
                        };
                        assert_eq!(
                            report.status,
                            if valid {
                                Status::Valid
                            } else {
                                Status::Invalid
                            },
                            "{source}"
                        );
                    }
                }
            }
        }
        for (name, args) in [
            ("bool_not", "false,true"),
            ("bool2int", "true,1"),
            ("bool_clause", "[false],[false]"),
            ("bool_clause_reif", "[false],[true],false"),
            ("bool_clause_imp", "[true],[],true"),
        ] {
            let name = format!("{prefix}{name}");
            names.insert(name.clone());
            let source = format!("constraint {name}({args});solve satisfy;");
            assert_eq!(
                prepare("extra.fzn", &source)
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                Status::Valid,
                "{source}"
            );
        }
        for (operation, empty) in [("and", true), ("or", false), ("xor", false)] {
            for suffix in ["", "_imp"] {
                let name = format!("{prefix}array_bool_{operation}{suffix}");
                names.insert(name.clone());
                for (inputs, holds) in [("[]", empty), ("[true,false,true]", operation == "or")] {
                    for result in [false, true] {
                        let source = format!("constraint {name}({inputs},{result});solve satisfy;");
                        let valid = if suffix.is_empty() {
                            result == holds
                        } else {
                            !result || holds
                        };
                        assert_eq!(
                            prepare("array.fzn", &source)
                                .unwrap()
                                .check(&Assignment::default())
                                .unwrap()
                                .status,
                            if valid {
                                Status::Valid
                            } else {
                                Status::Invalid
                            },
                            "{source}"
                        );
                    }
                    if suffix.is_empty() {
                        let source = format!("constraint {name}({inputs});solve satisfy;");
                        assert_eq!(
                            prepare("assertion.fzn", &source)
                                .unwrap()
                                .check(&Assignment::default())
                                .unwrap()
                                .status,
                            if holds {
                                Status::Valid
                            } else {
                                Status::Invalid
                            },
                            "{source}"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(names.len(), 165);
    for (call, valid) in [
        ("bool_xor(false,true)", true),
        ("bool_xor(true,true)", false),
        ("bool_clause([],[])", false),
        ("bool_clause([false],[true])", false),
        ("bool_clause_reif([],[],false)", true),
        ("bool_clause_imp([true],[],false)", false),
        ("gecode_bool_clause_imp([],[],false)", true),
        ("fzn_bool_clause_imp([],[false],false)", false),
        ("bool2int(false,2)", false),
    ] {
        let source = format!("constraint {call};solve satisfy;");
        assert_eq!(
            prepare("convention.fzn", &source)
                .unwrap()
                .check(&Assignment::default())
                .unwrap()
                .status,
            if valid {
                Status::Valid
            } else {
                Status::Invalid
            },
            "{call}"
        );
    }
}

#[test]
fn boolean_completion_checks_supplied_outputs_domains_downstream_and_fresh_calls() {
    let model = prepare(
        "completion.fzn",
        r"
        var bool: x;
        var bool: copy ::is_defined_var;
        var bool: complement ::is_defined_var;
        var bool: via_int ::is_defined_var;
        var bool: term ::is_defined_var;
        var bool: truth ::is_defined_var;
        var bool: clause_truth ::is_defined_var;
        var int: as_int ::is_defined_var;
        var -1..0: bound ::is_defined_var;
        constraint bool_eq(copy,x)::defines_var(copy);
        constraint bool_not(complement,copy)::defines_var(complement);
        constraint bool2int(complement,as_int)::defines_var(as_int);
        constraint bool2int(via_int,as_int)::defines_var(via_int);
        constraint bool_lin_eq([2,-1],[copy,via_int],bound)::defines_var(bound);
        constraint bool_lin_eq([2,-1],[term,via_int],bound)::defines_var(term);
        constraint bool_le_reif(term,via_int,truth)::defines_var(truth);
        constraint bool_clause_imp([term],[via_int],clause_truth)::defines_var(clause_truth);
        constraint bool_right_imp(term,via_int,true);
        constraint int_le(bound,0);
        solve minimize bound;",
    )
    .unwrap();
    for (candidate, status, complete, objective) in [
        ("x=false;", Status::Valid, true, Some(-1)),
        ("x=true;", Status::Invalid, true, Some(2)),
        ("", Status::Incomplete, false, None),
        ("x=false;complement=false;", Status::Invalid, true, Some(0)),
        ("x=false;", Status::Valid, true, Some(-1)),
    ] {
        let report = model.check(&assignment(candidate)).unwrap();
        assert_eq!(report.status, status, "{candidate}");
        assert_eq!(report.completeness.complete, complete, "{candidate}");
        assert_eq!(
            report.objective.map(|objective| objective.argument.value),
            objective.map(Value::Integer)
        );
        if candidate == "x=true;" {
            assert_eq!(report.violations.len(), 3);
            assert!(matches!(
                report.violations[0].details,
                ViolationKind::Domain {
                    value: Value::Integer(2)
                }
            ));
        }
        if candidate.contains("complement") {
            assert!(matches!(
                report.violations[0].details,
                ViolationKind::BooleanOperation { .. }
            ));
        }
    }
    for (call, expected) in [
        ("bool_eq(x,true)", 1),
        ("bool_eq(false,x)", 0),
        ("bool_ne(x,true)", 0),
        ("bool_ne(false,x)", 1),
        ("bool_not(x,false)", 1),
        ("bool_not(true,x)", 0),
        ("bool_xor(x,true)", 0),
        ("bool_xor(false,x)", 1),
        ("bool_and(true,false,x)", 0),
        ("bool_or(true,false,x)", 1),
        ("bool_xor(true,true,x)", 0),
        ("bool_left_imp(false,true,x)", 0),
        ("bool_right_imp(false,true,x)", 1),
        ("array_bool_and([],x)", 1),
        ("array_bool_or([],x)", 0),
        ("array_bool_xor([],x)", 0),
        ("bool_clause_reif([],[],x)", 0),
        ("bool_clause_imp([],[false],x)", 1),
        ("bool_lt_reif(true,false,x)", 0),
        ("bool_lin_ne_reif([2],[true],1,x)", 1),
        (
            "bool_lin_eq([-9223372036854775808],[x],-9223372036854775808)",
            1,
        ),
    ] {
        let source = format!(
            "var bool:x::is_defined_var;var int:n::is_defined_var;constraint {call}::defines_var(x);constraint bool2int(x,n)::defines_var(n);solve minimize n;"
        );
        let report = prepare("direction.fzn", &source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap();
        assert_eq!(report.status, Status::Valid, "{call}");
        assert_eq!(
            report.objective.unwrap().argument.value,
            Value::Integer(expected),
            "{call}"
        );
    }
}

#[test]
fn boolean_ambiguous_or_unavailable_outputs_produce_ordinary_incompleteness() {
    for call in [
        "bool_eq(x,alias)",
        "bool_not(x,alias)",
        "bool_xor(x,alias)",
        "bool_xor(x,false,alias)",
        "bool_and(x,true,true)",
        "bool_eq_reif(x,true,false)",
        "bool_le(x,true)",
        "bool_eq_imp(false,true,x)",
        "array_bool_or_imp([true],x)",
        "bool_lin_eq([0],[x],0)",
        "bool_lin_eq([1,1],[x,alias],0)",
        "bool_lin_eq([2],[x],1)",
        "bool_lin_eq([1],[x],2)",
        "bool_lin_eq_reif([1],[x],0,alias)",
        "bool_clause_imp([x],[],alias)",
        "bool2int(x,2)",
    ] {
        let source = format!(
            "var bool:x::is_defined_var;var bool:alias=x;constraint {call}::defines_var(x);solve satisfy;"
        );
        let model = prepare("unresolved.fzn", &source).unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status, Status::Incomplete, "{call}");
        assert!(!report.completeness.complete, "{call}");
        assert!(
            model
                .check(&assignment("x=false;"))
                .unwrap()
                .completeness
                .complete,
            "{call}"
        );
    }
    let model = prepare("missing.fzn", "var bool:x;var int:y;constraint bool_and_imp(x,true,false);constraint array_bool_xor_imp([x],false);constraint bool_lin_gt_imp([9223372036854775807,1],[true,x],y,false);constraint bool_clause_reif([],[],true);constraint bool_eq_reif(x,true,false);solve satisfy;").unwrap();
    let report = model.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    assert_eq!(report.violations.len(), 1);
    // Only the two domains and the full reification await inputs.
    assert_eq!(report.completeness.blocked_checks.len(), 3);
    for call in [
        "bool_lin_gt([],[], -9223372036854775808)",
        "bool_lin_lt([],[],9223372036854775807)",
        "bool_lin_eq_imp([9223372036854775807,1],[true,true],0,false)",
        "bool_lin_ne_reif([],[],0,false)",
    ] {
        let source = format!("constraint {call};solve satisfy;");
        assert_eq!(
            prepare("bounds.fzn", &source)
                .unwrap()
                .check(&Assignment::default())
                .unwrap()
                .status,
            Status::Valid,
            "{call}"
        );
    }
    for call in [
        "bool_lin_eq_imp([9223372036854775807,1],[true,true],0,true)",
        "bool_lin_eq_reif([9223372036854775807,1],[true,true],0,false)",
        "bool_lin_eq([9223372036854775807,1,-1],[true,true,true],9223372036854775807)",
    ] {
        let source = format!("constraint {call};solve satisfy;");
        let error = prepare("overflow.fzn", &source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution, "{call}");
        assert!(error.diagnostics[0].message.contains("overflow"));
    }
}

#[test]
fn boolean_source_admission_checks_declarations_arity_shape_and_roles() {
    for source in [
        "predicate bool_xor(var bool:a,var bool:b);constraint bool_xor(false,true);solve satisfy;",
        "predicate bool_xor(var bool:a,var bool:b,var bool:c);constraint bool_xor(false,true,true);solve satisfy;",
        "predicate array_bool_and(array[int] of var bool:a);constraint array_bool_and([]);solve satisfy;",
        "predicate array_bool_or(array[int] of var bool:a,var bool:b);constraint array_bool_or([],false);solve satisfy;",
        "predicate bool_lin_eq(array[int] of int:a,array[int] of var bool:b,var int:c);var int:n;constraint bool_lin_eq([1],[true],n);solve satisfy;",
        "predicate bool_lin_le(array[int] of int:a,array[int] of var bool:b,int:c);constraint bool_lin_le([1],[true],2);solve satisfy;",
        "predicate bool_lin_le(array[int] of int:a,array[int] of var bool:b,var int:c);var int:n;constraint bool_lin_le([1],[true],n);solve satisfy;",
    ] {
        prepare("declaration.fzn", source).unwrap();
    }
    for source in [
        "predicate bool_xor(var bool:a,var bool:b);constraint bool_xor(false,true,true);solve satisfy;",
        "predicate bool_xor(var bool:a,var bool:b,var bool:c);constraint bool_xor(false,true);solve satisfy;",
        "predicate array_bool_and(array[int] of var bool:a);constraint array_bool_and([],true);solve satisfy;",
        "predicate array_bool_or(array[int] of var bool:a,var bool:b);constraint array_bool_or([]);solve satisfy;",
        "predicate bool_not(var bool:a,bool:b);solve satisfy;",
        "predicate bool_lin_eq(array[int] of int:a,array[int] of var bool:b,int:c);solve satisfy;",
        "predicate bool_lin_le(array[int] of int:a,array[int] of var bool:b,int:c);var int:n;constraint bool_lin_le([1],[true],n);solve satisfy;",
        "predicate bool_lin_le(array[int] of var int:a,array[int] of var bool:b,int:c);solve satisfy;",
        "predicate bool_eq(var bool:a,var bool:b);predicate bool_eq(var bool:x,var bool:y);solve satisfy;",
        "predicate bool_clause(array[int] of var bool:a,array[int] of var int:b);solve satisfy;",
        "constraint bool_eq(1,true);solve satisfy;",
        "constraint bool_not([true],false);solve satisfy;",
        "constraint bool_lin_eq([1,2],[true],0);solve satisfy;",
        "constraint bool_lin_eq([1],[1],0);solve satisfy;",
        "var int:x;constraint bool_lin_eq([x],[true],1);solve satisfy;",
        "constraint bool_lin_eq([1],[true],false);solve satisfy;",
        "constraint bool2int(true,false);solve satisfy;",
        "constraint array_bool_xor_imp([]);solve satisfy;",
        "constraint fzn_bool_xor(false,true);solve satisfy;",
        "constraint bool_xor_reif(false,true,true);solve satisfy;",
        "constraint gecode_gecode_bool_eq(false,false);solve satisfy;",
        "var bool:x::is_defined_var;constraint bool_not(true,false)::defines_var(x);solve satisfy;",
    ] {
        let errors = prepare("bad-boolean.fzn", source).unwrap_err();
        assert!(errors[0].location.is_some(), "{source}");
    }
}

#[test]
fn floats_share_validated_default_custom_and_exact_comparisons() {
    use zincite_check::{FiniteFloat, FloatTolerance};
    let f = |value| FiniteFloat::new(value).unwrap();
    let default = FloatTolerance::default();
    assert_eq!((default.absolute(), default.relative()), (1e-9, 1e-9));
    assert!(default.equal(f(1.0), f(1.0 + 5e-10)));
    assert!(!default.equal(f(1.0), f(1.0 + 2e-9)));
    assert!(
        !FloatTolerance::new(0.0, 1.5)
            .unwrap()
            .equal(f(f64::MAX), f(-f64::MAX))
    );
    assert!(
        FloatTolerance::new(0.0, 2.0)
            .unwrap()
            .equal(f(f64::MAX), f(-f64::MAX))
    );
    let tiny = f64::from_bits(1);
    assert!(
        !FloatTolerance::new(0.0, 0.5)
            .unwrap()
            .equal(f(tiny), f(0.0))
    );
    assert!(
        FloatTolerance::new(0.0, 1.0)
            .unwrap()
            .equal(f(tiny), f(0.0))
    );
    assert_eq!(f(-0.0).get().to_bits(), 0);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(FiniteFloat::new(value).is_err());
        assert!(FloatTolerance::new(value, 0.0).is_err());
        assert!(FloatTolerance::new(0.0, value).is_err());
    }
    assert!(FloatTolerance::new(-1.0, 0.0).is_err());
    let mut model = prepare(
        "comparisons.fzn",
        r#"
        var float:x;
        var bool:eq::is_defined_var;
        var bool:ne::is_defined_var;
        constraint float_eq_reif(x,1.0,eq)::defines_var(eq);
        constraint float_ne_reif(x,1.0,ne)::defines_var(ne);
        constraint float_le(x,1.0);
        constraint float_lt(x,1.0);
        constraint bool_eq(eq,true);
        constraint bool_eq(ne,false);
        solve minimize x;
    "#,
    )
    .unwrap();
    let candidate = assignment("x=0.9999999995;");
    let report = model.check(&candidate).unwrap();
    assert_eq!(report.violations.len(), 1);
    assert!(matches!(
        report.violations[0].details,
        ViolationKind::FloatRelation { .. }
    ));
    model.set_float_tolerance(FloatTolerance::new(0.0, 0.0).unwrap());
    assert_eq!(model.check(&candidate).unwrap().violations.len(), 2);
    model.set_float_tolerance(FloatTolerance::new(0.01, 0.0).unwrap());
    assert_eq!(
        model
            .check(&assignment("x=1.005;"))
            .unwrap()
            .violations
            .len(),
        1
    );
}

#[test]
fn float_completion_respects_alias_identity_domains_and_fresh_inputs() {
    let model = prepare(
        "completion.fzn",
        r#"
        array[1..2] of float:coefficients=[2.0,1.0];
        var float:x;
        var float:alias=x;
        var 0.0..1.0:y::is_defined_var;
        var float:z::is_defined_var;
        var float:copied::is_defined_var;
        var float:repeated::is_defined_var;
        var float:repeat_alias=repeated;
        array[1..2] of var float:values=[y,z];
        constraint float_plus(x,0.5,y)::defines_var(y);
        constraint float_lin_eq(coefficients,[y,z],5.0)::defines_var(z);
        constraint float_eq(z,copied)::defines_var(copied);
        constraint float_eq(repeated,repeat_alias)::defines_var(repeated);
        constraint float_le(y,1.0);
        solve minimize copied;
    "#,
    )
    .unwrap();
    let report = model.check(&assignment("x=1.0;")).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    assert_eq!(report.violations.len(), 2);
    assert!(matches!(
        report.violations[0].details,
        ViolationKind::Domain { .. }
    ));
    assert_eq!(
        report.objective.unwrap().argument.value,
        Value::Float(zincite_check::FiniteFloat::new(2.0).unwrap())
    );
    assert!(
        model
            .check(&assignment("x=1.0;alias=1.0000000001;"))
            .is_err()
    );
    assert_eq!(
        model
            .check(&assignment("x=0.0;y=7.0;"))
            .unwrap()
            .violations
            .len(),
        3
    );
    let fresh = model.check(&Assignment::default()).unwrap();
    assert_eq!(fresh.status, Status::Incomplete);
    assert!(fresh.objective.is_none());
    let near = prepare("identity.fzn","var 1.0..1.0000000001:x; var 1.0..1.0:y; var 3.0..2.0:empty; var 2.0..2.0:computed::computed_domain; solve satisfy;").unwrap();
    assert_eq!(
        near.check(&Assignment::default())
            .unwrap()
            .completeness
            .unresolved_variables
            .len(),
        3
    );
    assert_eq!(
        near.check(&assignment("x=1.0;empty=2.5;computed=100.0;"))
            .unwrap()
            .violations
            .len(),
        1
    );
}

#[test]
fn float_arithmetic_conversion_and_linear_failures_are_located() {
    for (call, expected) in [
        ("float_abs(-2.5,y)", "2.5"),
        ("float_plus(1.25,2.5,y)", "3.75"),
        ("float_times(1e-300,1e-300,y)", "0.0"),
        ("float_div(3.0,2.0,y)", "1.5"),
        ("float_min(3.0,2.0,y)", "2.0"),
        ("float_max(-3.0,-2.0,y)", "-2.0"),
        ("int2float(9007199254740993,y)", "9007199254740992.0"),
    ] {
        let model = prepare(
            "arithmetic.fzn",
            &format!(
                "var float:y::is_defined_var;constraint {call}::defines_var(y);solve minimize y;"
            ),
        )
        .unwrap();
        let expected = assignment(&format!("y={expected};"));
        assert_eq!(
            model.check(&expected).unwrap().status,
            Status::Valid,
            "{call}"
        );
        assert_eq!(
            serde_json::to_value(model.check(&expected).unwrap().objective).unwrap(),
            serde_json::to_value(model.check(&Assignment::default()).unwrap().objective).unwrap()
        );
    }
    for (operation, input, expected) in [
        ("float_ceil", "-1.25", "-1"),
        ("float_floor", "-1.25", "-2"),
        ("float_round", "1.5", "2"),
        ("float_round", "-1.5", "-2"),
        ("float_round", "0.49999999999999994", "0"),
        (
            "float_floor",
            "-9223372036854775808.0",
            "-9223372036854775808",
        ),
        ("float_ceil", "9223372036854774784.0", "9223372036854774784"),
    ] {
        let model=prepare("conversion.fzn",&format!("var int:y::is_defined_var;constraint {operation}({input},y)::defines_var(y);solve minimize y;")).unwrap();
        assert_eq!(
            model
                .check(&assignment(&format!("y={expected};")))
                .unwrap()
                .status,
            Status::Valid
        );
        assert_eq!(
            model
                .check(&Assignment::default())
                .unwrap()
                .objective
                .unwrap()
                .argument
                .value,
            Value::Integer(expected.parse().unwrap())
        );
    }
    for call in [
        "float_plus(1e308,1e308,0.0)",
        "float_times(1e308,2.0,0.0)",
        "float_div(1.0,-0.0,0.0)",
        "float_lin_eq([1e308,1e308],[2.0,-2.0],0.0)",
        "float_floor(9223372036854775808.0,0)",
        "float_ceil(-9223372036854777856.0,0)",
    ] {
        let model = prepare("failure.fzn", &format!("constraint {call};solve satisfy;")).unwrap();
        let error = model.check(&Assignment::default()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution, "{call}");
        assert!(error.diagnostics[0].location.is_some());
    }
    let model=prepare("exact.fzn","var int:x;var float:y::is_defined_var;constraint int2float(x,y)::defines_var(y);constraint int_eq(x,9007199254740993);constraint float_eq(y,9007199254740992.0);solve satisfy;").unwrap();
    assert_eq!(
        model
            .check(&assignment("x=9007199254740993;"))
            .unwrap()
            .status,
        Status::Valid
    );
    assert_eq!(
        model
            .check(&assignment("x=9007199254740992;"))
            .unwrap()
            .violations
            .len(),
        1
    );
}

#[test]
fn float_membership_uses_complete_pairs_and_exact_interval_emptiness() {
    let model = prepare(
        "membership.fzn",
        r#"
        set of float:allowed={1.0,2.0..3.0,2.5..4.0};
        array[1..3] of float:endpoints=[1.0,2.0,99.0];
        var float:x;
        constraint float_in(x,1.0,4.0);
        constraint float_dom(x,endpoints);
        constraint float_set_in(x,allowed);
        constraint float_dom(x,[]);
        constraint float_dom(x,[99.0]);
        constraint float_in(x,1.0000000001,1.0);
        solve satisfy;
    "#,
    )
    .unwrap();
    assert_eq!(
        model
            .check(&assignment("x=1.0000000001;"))
            .unwrap()
            .violations
            .len(),
        3
    );
    assert_eq!(
        model
            .check(&assignment("x=99.0;"))
            .unwrap()
            .violations
            .len(),
        6
    );
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
}

#[test]
fn float_names_conditions_and_source_roles_follow_pinned_signatures() {
    for prefix in ["", "gecode_", "fzn_"] {
        for (relation, left, right, truth) in [
            ("eq", "1.0", "1.0", true),
            ("ne", "1.0", "2.0", true),
            ("le", "1.0", "2.0", true),
            ("lt", "1.0", "1.0", false),
        ] {
            let model = prepare(
                "name.fzn",
                &format!("constraint {prefix}float_{relation}({left},{right});solve satisfy;"),
            )
            .unwrap();
            assert_eq!(
                model
                    .check(&Assignment::default())
                    .unwrap()
                    .violations
                    .is_empty(),
                truth
            );
            if relation != "ne" {
                let model=prepare("name.fzn",&format!("constraint {prefix}float_{relation}_reif({left},{right},{truth});constraint {prefix}float_lin_{relation}_reif([1.0],[{left}],{right},{truth});solve satisfy;")).unwrap();
                assert_eq!(
                    model.check(&Assignment::default()).unwrap().status,
                    Status::Valid
                );
            }
        }
    }
    for prefix in ["", "gecode_", "fzn_"] {
        for (code, truth) in [
            (1, false),
            (2, true),
            (3, true),
            (4, true),
            (5, false),
            (6, false),
        ] {
            let model=prepare("coded.fzn",&format!("var float:x;var bool:guard;constraint {prefix}gecode_float_rel_imp_fzn(x,{code},2.0,guard);solve satisfy;")).unwrap();
            assert_eq!(
                model
                    .check(&assignment("guard=false;"))
                    .unwrap()
                    .violations
                    .len(),
                0
            );
            assert_eq!(
                model
                    .check(&assignment("x=1.0;guard=true;"))
                    .unwrap()
                    .violations
                    .is_empty(),
                truth
            );
        }
    }
    let inactive=prepare("inactive.fzn","var float:x;constraint float_lin_eq_imp([1e308,1.0],[1e308,x],0.0,false);constraint float_lin_ne_imp([1.0],[x],2.0,false);constraint float_ne_reif(1.0,2.0,false);solve satisfy;").unwrap();
    let report = inactive.check(&Assignment::default()).unwrap();
    assert_eq!(report.violations.len(), 1);
    assert!(!report.completeness.complete);
    for text in [
        "constraint float_eq(1,1.0);solve satisfy;",
        "constraint float_eq(1.0e999,1.0);solve satisfy;",
        "var set of float:x;solve satisfy;",
        "constraint float_ge(1.0,2.0);solve satisfy;",
        "constraint gecode_float_ne_reif(1.0,2.0,true);solve satisfy;",
        "constraint fzn_float_lin_ne([1.0],[2.0],0.0);solve satisfy;",
        "constraint gecode_float_rel_imp_fzn(1.0,0,2.0,false);solve satisfy;",
        "var int:c;constraint gecode_float_rel_imp_fzn(1.0,c,2.0,false);solve satisfy;",
        "var float:c;constraint float_in(1.0,c,2.0);solve satisfy;",
        "var float:c;constraint float_lin_eq([c],[1.0],2.0);solve satisfy;",
        "constraint float_lin_eq([1.0,2.0],[1.0],2.0);solve satisfy;",
        "predicate float_in(var float:x,var float:l,float:u);solve satisfy;",
        "predicate float_ceil(var int:x,var float:y);solve satisfy;",
        "constraint float_sqrt(1.0,1.0);solve satisfy;",
    ] {
        assert!(prepare("bad.fzn", text).is_err(), "{text}");
    }
}

#[test]
fn selection_spellings_and_declared_roles_match_upstream_signatures() {
    let mut count = 0;
    for (kind, value) in [
        ("bool", "true"),
        ("int", "7"),
        ("float", "1.25"),
        ("set of int", "{2,7}"),
    ] {
        let stem = if kind == "set of int" { "set" } else { kind };
        for variable_array in [false, true] {
            let var = if variable_array { "var_" } else { "" };
            let role = if variable_array { "var " } else { "" };
            let prefixes: &[&str] = if stem == "float" {
                &[""]
            } else {
                &["", "gecode_", "fzn_"]
            };
            for prefix in prefixes {
                let name = format!("{prefix}array_{var}{stem}_element");
                let source = format!(
                    "predicate {name}(var int: i, array[int] of {role}{kind}: a, var {kind}: y); var {kind}: y ::is_defined_var; constraint {name}(1,[{value}],y)::defines_var(y); solve satisfy;"
                );
                assert_eq!(
                    prepare("selection.fzn", &source)
                        .unwrap()
                        .check(&assignment(""))
                        .unwrap()
                        .status,
                    Status::Valid,
                    "{name}"
                );
                count += 1;
            }
        }
        let name = format!("array_var_{stem}_element_nonshifted");
        let source = format!("constraint {name}(1,[{value}],{value}); solve satisfy;");
        assert_eq!(
            prepare("selection.fzn", &source)
                .unwrap()
                .check(&assignment(""))
                .unwrap()
                .status,
            Status::Valid
        );
        count += 1;
        for prefix in ["", "gecode_", "fzn_"] {
            let name = format!("{prefix}gecode_{stem}_ite");
            let source = format!(
                "predicate {name}(var bool: b,var {kind}: x,var {kind}: y,var {kind}: z); var {kind}: z ::is_defined_var; constraint {name}(true,{value},z,z)::defines_var(z); solve satisfy;"
            );
            assert_eq!(
                prepare("selection.fzn", &source)
                    .unwrap()
                    .check(&assignment(""))
                    .unwrap()
                    .status,
                Status::Valid
            );
            count += 1;
        }
        if matches!(stem, "int" | "bool") {
            for prefix in ["", "gecode_", "fzn_"] {
                for var in ["", "var_"] {
                    let name = format!("{prefix}gecode_{var}{stem}_element");
                    let source = format!(
                        "predicate {name}(var int: i,int: offset,array[int] of var {kind}: a,var {kind}: y); var {kind}: x; var {kind}: y ::is_defined_var; constraint {name}(-3,-3,[x],y)::defines_var(y); solve satisfy;"
                    );
                    assert_eq!(
                        prepare("selection.fzn", &source)
                            .unwrap()
                            .check(&assignment(&format!("x={value};")))
                            .unwrap()
                            .status,
                        Status::Valid
                    );
                    count += 1;
                }
                let name = format!("{prefix}gecode_{stem}_element2d");
                let source = format!(
                    "predicate {name}(var int: r,var int: c,array[int] of var {kind}: a,set of int: rows,set of int: cols,var {kind}: y); constraint {name}(-1,5,[{value},{value}],-1..-1,4..5,{value}); solve satisfy;"
                );
                assert_eq!(
                    prepare("selection.fzn", &source)
                        .unwrap()
                        .check(&assignment(""))
                        .unwrap()
                        .status,
                    Status::Valid
                );
                count += 1;
            }
        }
    }
    for prefix in ["", "gecode_", "fzn_"] {
        let name = format!("{prefix}gecode_set_element2d");
        let source = format!(
            "predicate {name}(array[int] of var set of int: a,var int: c,int: w,var int: r,int: h,var set of int: y); constraint {name}([{{1}},{{2}},{{3}},{{4}},{{5}},{{6}}],2,3,1,2,{{6}}); solve satisfy;"
        );
        assert_eq!(
            prepare("selection.fzn", &source)
                .unwrap()
                .check(&assignment(""))
                .unwrap()
                .status,
            Status::Valid
        );
        count += 1;
    }
    assert_eq!(count, 57);

    for source in [
        "var int: x; constraint array_int_element(1,[x],1); solve satisfy;",
        "var int: o; constraint gecode_int_element(1,o,[1],1); solve satisfy;",
        "var set of int: rows; constraint gecode_int_element2d(1,1,[1],rows,1..1,1); solve satisfy;",
        "var int: w; constraint gecode_set_element2d([{1}],0,w,0,1,{1}); solve satisfy;",
        "predicate array_int_element(var int: i,array[int] of var int: a,var int: y); solve satisfy;",
        "predicate gecode_int_element(var int: i,var int: o,array[int] of var int: a,var int: y); solve satisfy;",
        "predicate gecode_bool_ite(bool: b,var bool: x,var bool: y,var bool: z); solve satisfy;",
        "constraint gecode_array_float_element(1,[1.0],1.0); solve satisfy;",
        "constraint array_var_int_element2d_nonshifted(1,1,[1],1); solve satisfy;",
    ] {
        assert!(prepare("roles.fzn", source).is_err(), "{source}");
    }
}

#[test]
fn selections_require_only_current_selected_values_and_keep_conflicts_and_domain_failures() {
    let model = prepare("selection.fzn", r#"
var int: i;
var bool: condition;
var int: missing;
var 0..5: selected ::is_defined_var;
var int: branch ::is_defined_var;
var int: next ::is_defined_var;
constraint array_var_int_element(i,[missing,9,selected],selected)::defines_var(selected)::checker_constraint("pick",[42])::mzn_constraint_name("Choose value");
constraint gecode_int_ite(condition,selected,branch,branch)::defines_var(branch);
constraint int_plus(branch,1,next)::defines_var(next);
solve minimize next;
"#).unwrap();
    let report = model.check(&assignment("i=2;condition=true;")).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete); // The independent missing variable still has a domain check.
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.objective.unwrap().argument.value, Value::Integer(10));
    assert!(
        !report
            .completeness
            .blocked_checks
            .iter()
            .any(|blocked| matches!(blocked.check, CheckRef::Constraint { .. }))
    );

    let report = model
        .check(&assignment("i=2;condition=true;selected=3;branch=4;"))
        .unwrap();
    assert_eq!(report.violations.len(), 2);
    let ViolationKind::ElementMismatch {
        indices,
        selected,
        result,
        ..
    } = &report.violations[0].details
    else {
        panic!("element mismatch")
    };
    assert_eq!(indices[0].value, Value::Integer(2));
    assert_eq!(selected.value, Value::Integer(9));
    assert_eq!(result.variable.as_ref().unwrap().name, "selected");
    let CheckRef::Constraint { constraint } = &report.violations[0].check else {
        panic!("constraint")
    };
    assert_eq!(constraint.metadata.application.as_ref().unwrap().id, "pick");
    assert_eq!(
        constraint.metadata.application.as_ref().unwrap().entities,
        [42]
    );
    assert_eq!(constraint.metadata.label.as_deref(), Some("Choose value"));
    assert!(matches!(
        report.violations[1].details,
        ViolationKind::ConditionalMismatch { .. }
    ));

    for candidate in [
        "i=1;condition=true;",
        "i=3;condition=true;",
        "i=2;condition=false;",
    ] {
        let report = model.check(&assignment(candidate)).unwrap();
        assert!(report.objective.is_none(), "{candidate}");
        assert!(!report.completeness.complete);
    }
    let report = model.check(&assignment("condition=true;")).unwrap();
    let missing = report
        .completeness
        .blocked_checks
        .iter()
        .find_map(|blocked| match &blocked.check {
            CheckRef::Constraint { constraint } if constraint.ordinal == 1 => {
                Some(&blocked.missing)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        missing.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
        ["i"]
    );
    let report = model.check(&assignment("i=-9223372036854775808;")).unwrap();
    assert!(matches!(
        report.violations[0].details,
        ViolationKind::ElementRange { result: None, .. }
    ));
    assert!(
        model
            .check(&assignment("i=2;condition=true;"))
            .unwrap()
            .objective
            .is_some()
    );
}

#[test]
fn element_layouts_check_axes_before_row_major_selection_and_use_exact_indices() {
    for (call, good, bad) in [
        (
            "gecode_int_element(i,-9223372036854775808,[3,7],y)",
            "i=-9223372036854775807;y=7;",
            "i=9223372036854775807;",
        ),
        (
            "gecode_int_element2d(i,j,[1,2,3,4,5,6],-2..-1,4..6,y)",
            "i=-1;j=5;y=5;",
            "i=-2;j=7;",
        ),
        (
            "gecode_bool_element2d(i,j,[true,false,false,false,true,false],-2..-1,4..6,y)",
            "i=-1;j=5;y=true;",
            "i=-3;",
        ),
        (
            "gecode_set_element2d([{1},{2},{3},{4},{5},{6}],j,3,i,2,y)",
            "i=1;j=2;y={6};",
            "i=0;j=3;",
        ),
    ] {
        let kind = if call.starts_with("gecode_bool") {
            "bool"
        } else if call.starts_with("gecode_set") {
            "set of int"
        } else {
            "int"
        };
        let source =
            format!("var int: i; var int: j; var {kind}: y; constraint {call}; solve satisfy;");
        let model = prepare("layout.fzn", &source).unwrap();
        let report = model.check(&assignment(good)).unwrap();
        assert!(report.violations.is_empty(), "{call}");
        let report = model.check(&assignment(bad)).unwrap();
        assert!(
            matches!(
                report.violations[0].details,
                ViolationKind::ElementRange { result: None, .. }
            ),
            "{call}"
        );
    }
    for call in [
        "gecode_int_element2d(1,1,[1],{},1..1,1)",
        "gecode_int_element2d(1,1,[1,2],{1,3},1..1,1)",
        "gecode_int_element2d(1,1,[1],1..2,1..3,1)",
        "gecode_int_element2d(1,1,[],1..9223372036854775807,1..9223372036854775807,1)",
        "gecode_set_element2d([],0,0,0,1,{})",
        "gecode_set_element2d([],0,1,0,-1,{})",
        "gecode_set_element2d([{1}],0,2,0,1,{1})",
    ] {
        assert!(
            prepare("shape.fzn", &format!("constraint {call}; solve satisfy;")).is_err(),
            "{call}"
        );
    }
    let model = prepare(
        "empty.fzn",
        "var int: i; var int: y; constraint array_var_int_element(i,[],y); solve satisfy;",
    )
    .unwrap();
    assert_eq!(
        model.check(&assignment("")).unwrap().status,
        Status::Incomplete
    );
    assert!(matches!(
        model.check(&assignment("i=1;")).unwrap().violations[0].details,
        ViolationKind::ElementRange { length: 0, .. }
    ));
}

#[test]
fn selected_floats_use_the_saved_policy_without_changing_the_selected_value() {
    let mut model = prepare("float-selection.fzn", "var float: chosen ::is_defined_var; constraint array_var_float_element_nonshifted(2,[9.0,1.25],chosen)::defines_var(chosen); constraint gecode_float_ite(false,9.0,chosen,1.2500000001); solve minimize chosen;").unwrap();
    let report = model.check(&assignment("")).unwrap();
    assert_eq!(report.status, Status::Valid);
    assert_eq!(
        report.objective.unwrap().argument.value,
        Value::Float(zincite_check::FiniteFloat::new(1.25).unwrap())
    );
    model.set_float_tolerance(zincite_check::FloatTolerance::new(0.0, 0.0).unwrap());
    assert!(matches!(
        model.check(&assignment("")).unwrap().violations[0].details,
        ViolationKind::ConditionalMismatch { .. }
    ));
}

#[test]
fn selection_reports_and_text_survive_validated_round_trips() {
    let mut model = prepare(
        "selection.fzn",
        include_str!("../../../examples/checker-compilation/selection.fzn"),
    )
    .unwrap();
    model.set_float_tolerance(zincite_check::FloatTolerance::new(2e-6, 3e-6).unwrap());
    let loaded = zincite_check::PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for candidate in [
        "index=0;condition=true;input_float=1.2500000001;input_set={6};",
        "index=0;condition=true;input_float=1.25;input_set={6};picked=8;chosen_bool=false;chosen_set={5};chosen_float=2.0;",
        "index=9223372036854775807;condition=true;input_set={6};",
        "",
    ] {
        let original = model.check(&assignment(candidate)).unwrap();
        let round_trip = loaded.check(&assignment(candidate)).unwrap();
        assert_eq!(
            serde_json::to_value(&original).unwrap(),
            serde_json::to_value(&round_trip).unwrap()
        );
        assert_eq!(
            zincite_check::cli::render_text(&original),
            zincite_check::cli::render_text(&round_trip)
        );
    }
}

#[test]
fn set_names_declarations_and_aliases_check_the_same_relations() {
    for (name, parameters, arguments) in [
        (
            "set_eq",
            "var set of int:a,var set of int:b",
            "{2,1,1},1..2",
        ),
        ("equal", "var set of int:a,var set of int:b", "{2,1,1},1..2"),
        ("set_ne", "var set of int:a,var set of int:b", "{1},{2}"),
        ("set_le", "var set of int:a,var set of int:b", "{1},{2}"),
        ("set_lt", "var set of int:a,var set of int:b", "{1,3},{2}"),
        (
            "set_subset",
            "var set of int:a,var set of int:b",
            "{1},{1,2}",
        ),
        (
            "set_superset",
            "var set of int:a,var set of int:b",
            "{1,2},{1}",
        ),
        (
            "set_eq_reif",
            "var set of int:a,var set of int:b,var bool:c",
            "{1},{2},false",
        ),
        (
            "equal_reif",
            "var set of int:a,var set of int:b,var bool:c",
            "{1},{2},false",
        ),
        (
            "set_ne_reif",
            "var set of int:a,var set of int:b,var bool:c",
            "{1},{1},false",
        ),
        (
            "set_le_reif",
            "var set of int:a,var set of int:b,var bool:c",
            "{2},{1},false",
        ),
        (
            "set_lt_reif",
            "var set of int:a,var set of int:b,var bool:c",
            "{1},{1},false",
        ),
        (
            "set_subset_reif",
            "var set of int:a,var set of int:b,var bool:c",
            "{1,2},{1},false",
        ),
        (
            "set_superset_reif",
            "var set of int:a,var set of int:b,var bool:c",
            "{1},{1,2},false",
        ),
        ("set_in", "var int:a,var set of int:b", "2,1..3"),
        (
            "set_in_reif",
            "var int:a,var set of int:b,var bool:c",
            "2,{1},false",
        ),
        (
            "set_in_imp",
            "var int:a,var set of int:b,var bool:c",
            "2,{1},false",
        ),
        ("int_in", "var bool:a,set of int:b", "true,{1}"),
        (
            "int_in_reif",
            "var bool:a,set of int:b,var bool:c",
            "false,{1},false",
        ),
        (
            "int_in_imp",
            "var int:a,set of int:b,var bool:c",
            "2,{1},false",
        ),
        ("set_card", "var set of int:a,var int:b", "{1,3},2"),
        (
            "set_union",
            "var set of int:a,var set of int:b,var set of int:c",
            "{1},{2},1..2",
        ),
        (
            "set_intersect",
            "var set of int:a,var set of int:b,var set of int:c",
            "1..3,2..4,2..3",
        ),
        (
            "set_diff",
            "var set of int:a,var set of int:b,var set of int:c",
            "1..3,2..4,{1}",
        ),
        (
            "set_symdiff",
            "var set of int:a,var set of int:b,var set of int:c",
            "1..3,2..4,{1,4}",
        ),
        (
            "gecode_set_rel_imp_fzn",
            "var set of int:a,int:b,var set of int:c,var bool:d",
            "{2},8,{1},true",
        ),
    ] {
        for prefix in ["", "gecode_", "fzn_"] {
            let name = format!("{prefix}{name}");
            let source = format!(
                "predicate {name}({parameters}); constraint {name}({arguments}); solve satisfy;"
            );
            let model = prepare("aliases.fzn", &source).unwrap();
            let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
            assert_eq!(
                loaded.check(&Assignment::default()).unwrap().status,
                Status::Valid,
                "{name}"
            );
        }
    }
    for name in ["eq", "ne", "le", "lt", "subset", "superset"] {
        let source = format!(
            "predicate set_{name}_imp(var set of int:a,var set of int:b,var bool:c); constraint set_{name}_imp({{}},{{}},false); solve satisfy;"
        );
        let model = prepare("imp.fzn", &source).unwrap();
        assert_eq!(
            PreparedModel::from_json(&model.to_json().unwrap())
                .unwrap()
                .check(&Assignment::default())
                .unwrap()
                .status,
            Status::Valid
        );
    }
    // Fixed-set declarations also apply to the set_in compatibility signature.
    let model = prepare("bool.fzn", "predicate set_in(var bool:x,set of int:s);var bool:b;constraint set_in(b,{0});solve satisfy;").unwrap();
    assert_eq!(
        model.check(&assignment("b=false;")).unwrap().status,
        Status::Valid
    );
    assert_eq!(
        model.check(&assignment("b=true;")).unwrap().status,
        Status::Invalid
    );
}

#[test]
fn sorted_element_order_and_native_characteristic_order_are_distinct() {
    for (left, right, less, less_equal) in [
        ("{}", "{}", false, true),
        ("{}", "{1,3}", true, true),
        ("{1,3}", "{2}", true, true),
        ("{1}", "{1,2}", true, true),
        ("{1,2}", "{1}", false, false),
        ("{1}", "{2}", true, true),
        ("{2}", "{1}", false, false),
        ("{1,2}", "{1,2}", false, true),
        ("{1,3}", "{}", false, false),
    ] {
        let source = format!(
            "constraint set_lt_reif({left},{right},{less});constraint fzn_set_le_reif({left},{right},{less_equal});solve satisfy;"
        );
        assert_eq!(
            prepare("sorted.fzn", &source)
                .unwrap()
                .check(&Assignment::default())
                .unwrap()
                .status,
            Status::Valid,
            "{source}"
        );
    }
    for (code, left, right, expected) in [
        (1, "{1}", "{1}", true),
        (1, "{1}", "{2}", false),
        (2, "{1}", "{2}", true),
        (2, "{1}", "{1}", false),
        (3, "{1}", "1..2", true),
        (3, "1..2", "{1}", false),
        (4, "1..2", "{1}", true),
        (4, "{1}", "1..2", false),
        (5, "{}", "{1}", true),
        (5, "{1}", "{1}", false),
        (6, "-1073741822..-1", "0..1073741822", true),
        (6, "{}", "-1073741822..1073741822", true),
        (6, "-1073741821..-1", "0..1073741822", false),
        (6, "{-1073741823}", "-1073741822..1073741822", false),
        (7, "{2}", "{1}", true),
        (7, "{1}", "{2}", false),
        (7, "{}", "{1}", true),
        (7, "{}", "{}", true),
        (8, "{2}", "{1}", true),
        (8, "{1}", "{1}", false),
        (8, "{}", "{1}", true),
        (8, "{1}", "{}", false),
        (9, "{1}", "{2}", true),
        (9, "{2}", "{1}", false),
        (9, "{}", "{}", true),
        (9, "{}", "{1}", false),
        (10, "{1}", "{2}", true),
        (10, "{1}", "{1}", false),
        (10, "{1}", "{}", true),
        (10, "{}", "{1}", false),
    ] {
        let source =
            format!("constraint gecode_set_rel_imp_fzn({left},{code},{right},true);solve satisfy;");
        let model = prepare("native.fzn", &source).unwrap();
        assert_eq!(
            model.check(&Assignment::default()).unwrap().status == Status::Valid,
            expected,
            "{source}"
        );
    }
    // A shared outside element cannot cancel into an apparent complement.
    let model = prepare("outside.fzn", "constraint gecode_set_rel_imp_fzn({-1073741823},6,-1073741823..1073741822,true);solve satisfy;").unwrap();
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Invalid
    );
}

#[test]
fn set_completion_uses_known_inputs_and_fresh_values_without_inverse_choices() {
    let model = prepare(
        "definitions.fzn",
        r#"
        var set of int:input;
        var set of int:other;
        var set of {}:difference::is_defined_var;
        var set of int:result::is_defined_var;
        var int:size::is_defined_var;
        var bool:member::is_defined_var;
        var bool:ordered::is_defined_var;
        var bool:guard::is_defined_var;
        constraint fzn_set_diff(input,other,difference)::defines_var(difference);
        constraint gecode_set_symdiff(difference,{3},result)::defines_var(result);
        constraint fzn_set_card(result,size)::defines_var(size);
        constraint set_in_reif(3,result,member)::defines_var(member);
        constraint set_lt_reif(result,{4},ordered)::defines_var(ordered);
        constraint bool_eq(member,true);
        constraint set_in_imp(3,other,guard)::defines_var(guard);
        solve minimize size;
    "#,
    )
    .unwrap();
    let report = model
        .check(&assignment("input=1..3;other={2};guard=false;"))
        .unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(report.completeness.complete);
    assert_eq!(report.objective.unwrap().argument.value, Value::Integer(1));
    assert_eq!(report.violations.len(), 2); // Derived {1,3} violates its domain; 3 is absent from {1}.
    let conflict = model
        .check(&assignment(
            "input={1};other={};difference={};member=false;guard=false;",
        ))
        .unwrap();
    assert!(conflict.completeness.complete);
    assert!(
        conflict
            .violations
            .iter()
            .any(|finding| matches!(finding.details, ViolationKind::SetOperation { .. }))
    );
    let fresh = model.check(&assignment("guard=false;")).unwrap();
    assert_eq!(fresh.status, Status::Incomplete);
    assert!(fresh.objective.is_none());
    assert!(!fresh.completeness.blocked_checks.iter().any(|check| matches!(&check.check, CheckRef::Constraint { constraint } if constraint.predicate == "set_in_imp")));
    let active = model.check(&assignment("input={};guard=true;")).unwrap();
    assert!(!active.completeness.complete);
    assert!(active.completeness.blocked_checks.iter().any(|check| matches!(&check.check, CheckRef::Constraint { constraint } if constraint.predicate == "set_in_imp")));
    let disabled = prepare("disabled.fzn", "var set of int:s;var bool:b::is_defined_var;constraint set_subset_imp(s,{},false);constraint gecode_set_rel_imp_fzn(s,8,{},false);constraint set_in_imp(1,s,b)::defines_var(b);solve satisfy;").unwrap();
    let report = disabled.check(&Assignment::default()).unwrap();
    assert_eq!(
        report
            .completeness
            .blocked_checks
            .iter()
            .filter(|check| matches!(check.check, CheckRef::Constraint { .. }))
            .count(),
        1
    );
    assert!(
        report
            .completeness
            .unresolved_variables
            .iter()
            .any(|v| v.name == "b")
    );
    for call in [
        "set_diff(s,{},alias)",
        "set_symdiff(s,{},alias)",
        "set_eq_reif({}, {}, b)",
        "set_in_reif(b,{0},b)",
    ] {
        let target = if call.contains('b') { "b" } else { "s" };
        let set_annotation = if target == "s" {
            "::is_defined_var"
        } else {
            ""
        };
        let bool_annotation = if target == "b" {
            "::is_defined_var"
        } else {
            ""
        };
        let source = format!(
            "var set of {{}}:s{set_annotation};var set of int:alias=s;var bool:b{bool_annotation};constraint {call}::defines_var({target});solve satisfy;"
        );
        let model = prepare("repeated.fzn", &source).unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        if call == "set_eq_reif({}, {}, b)" {
            assert!(
                !report
                    .completeness
                    .unresolved_variables
                    .iter()
                    .any(|v| v.name == "b")
            );
        } else {
            assert_eq!(report.status, Status::Incomplete, "{call}");
        }
    }
    let variable_member = prepare("member.fzn", "var int:x;var set of int:s;constraint set_in(x,s);constraint set_in_reif(x,s,false);constraint set_ne({},{});solve satisfy;").unwrap();
    let report = variable_member.check(&assignment("x=2;")).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    assert_eq!(report.violations.len(), 1);
    let report = variable_member.check(&assignment("x=2;s={1};")).unwrap();
    assert!(report.completeness.complete);
    assert_eq!(report.violations.len(), 2);
}

#[test]
fn compact_set_algebra_handles_extreme_endpoints_and_checked_cardinality() {
    for (operation, left, right, expected) in [
        (
            "diff",
            "-9223372036854775808..9223372036854775807",
            "-9223372036854775807..9223372036854775806",
            "{-9223372036854775808,9223372036854775807}",
        ),
        (
            "symdiff",
            "-9223372036854775808..9223372036854775806",
            "-9223372036854775807..9223372036854775807",
            "{-9223372036854775808,9223372036854775807}",
        ),
        (
            "diff",
            "-9223372036854775808..9223372036854775807",
            "-9223372036854775808..9223372036854775807",
            "{}",
        ),
        (
            "symdiff",
            "-9223372036854775808..9223372036854775807",
            "-9223372036854775808..9223372036854775807",
            "{}",
        ),
    ] {
        let source = format!(
            "var set of int:r::is_defined_var;constraint set_{operation}({left},{right},r)::defines_var(r);constraint set_eq(r,{expected});solve satisfy;"
        );
        let model = prepare("large.fzn", &source).unwrap();
        assert_eq!(
            model.check(&Assignment::default()).unwrap().status,
            Status::Valid
        );
        assert_eq!(
            model
                .check(&assignment(&format!("r={expected};")))
                .unwrap()
                .status,
            Status::Valid
        );
        assert!(model.to_json().unwrap().len() < 10000);
    }
    let model = prepare("large-order.fzn", "constraint set_lt(-9223372036854775808..9223372036854775806,-9223372036854775808..9223372036854775807);constraint gecode_set_rel_imp_fzn(-9223372036854775808..9223372036854775806,8,-9223372036854775808..9223372036854775807,true);solve satisfy;").unwrap();
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Valid
    );
    let model = prepare("cardinality.fzn", "var int:n::is_defined_var;constraint set_card(1..9223372036854775807,n)::defines_var(n);solve minimize n;").unwrap();
    assert_eq!(
        model
            .check(&Assignment::default())
            .unwrap()
            .objective
            .unwrap()
            .argument
            .value,
        Value::Integer(i64::MAX)
    );
    for source in [
        "constraint set_card(0..9223372036854775807,0);solve satisfy;",
        "var int:n::is_defined_var;constraint set_card(0..9223372036854775807,n)::defines_var(n);solve satisfy;",
        "var int:n::is_defined_var;var int:m::is_defined_var;constraint set_card(1..9223372036854775807,n)::defines_var(n);constraint int_plus(n,1,m)::defines_var(m);solve satisfy;",
    ] {
        let error = prepare("overflow.fzn", source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution);
        assert!(error.diagnostics[0].message.contains("overflow"));
        assert_eq!(
            error.diagnostics[0].location.as_ref().unwrap().source,
            "overflow.fzn"
        );
    }
}

#[test]
fn set_source_rejects_invalid_roles_codes_and_invented_aliases() {
    for statement in [
        "constraint set_diff({},{});",
        "constraint set_card({},false);",
        "constraint set_in(true,s);",
        "constraint int_in(1,s);",
        "constraint int_in_reif(1,s,false);",
        "constraint fzn_int_in_imp(1,s,false);",
        "constraint gecode_set_rel_imp_fzn({},0,{},false);",
        "constraint gecode_set_rel_imp_fzn({},11,{},false);",
        "constraint gecode_set_rel_imp_fzn({},code,{},false);",
        "constraint gecode_set_rel_imp_fzn({},1,{},0);",
        "constraint fzn_set_eq_imp({},{},false);",
        "constraint gecode_set_lt_imp({},{},false);",
        "constraint equal_imp({},{},false);",
        "constraint set_union_reif({},{},{},true);",
        "constraint set_card_reif({},0,true);",
        "predicate set_eq(var set of int:a,set of int:b);",
        "predicate set_diff(var set of int:a,var int:b,var set of int:c);",
        "predicate int_in(var int:a,var set of int:b);",
        "predicate gecode_set_rel_imp_fzn(var set of int:a,var int:code,var set of int:b,var bool:c);",
        "predicate set_in(var int:x,set of int:t);constraint set_in(1,s);",
        "predicate set_in(var bool:x,var set of int:t);",
    ] {
        let source = format!("var set of int:s;var int:code;{statement}solve satisfy;");
        let error = prepare("bad-set.fzn", &source).unwrap_err();
        assert!(!error.is_empty(), "{statement}");
        assert!(
            error.iter().all(|d| d
                .location
                .as_ref()
                .is_some_and(|l| l.source == "bad-set.fzn")),
            "{statement}"
        );
    }
}
