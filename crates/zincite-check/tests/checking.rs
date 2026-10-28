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
        assert_eq!(
            constraint.metadata.defines.as_ref(),
            Some(&zincite_check::DefinitionTarget::Name("middle".into()))
        );

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
    for prefix in ["", "gecode_", "fzn_"] {
        for (role, declaration, bound, candidates) in [
            (
                "int",
                "",
                "0",
                [("x=false;", Status::Valid), ("x=true;", Status::Invalid)],
            ),
            (
                "var int",
                "var int:n;",
                "n",
                [
                    ("x=true;n=1;", Status::Valid),
                    ("x=true;n=0;", Status::Invalid),
                ],
            ),
        ] {
            let source = format!(
                "predicate {prefix}bool_lin_le(array[int] of int:a,array[int] of var bool:b,{role}:c);{declaration}var bool:x;constraint {prefix}bool_lin_le([1],[x],{bound});solve satisfy;"
            );
            let model = prepare("bound-role.fzn", &source).unwrap();
            for (candidate, status) in candidates {
                assert_eq!(
                    model.check(&assignment(candidate)).unwrap().status,
                    status,
                    "{source}: {candidate}"
                );
            }
        }
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
        for shape in ["", "lin_"] {
            for (relation, truth) in [("eq", false), ("ne", true), ("le", true), ("lt", true)] {
                for suffix in ["", "_reif", "_imp"] {
                    if !prefix.is_empty()
                        && (suffix == "_imp"
                            || (relation == "ne" && (shape == "lin_" || suffix == "_reif")))
                    {
                        continue;
                    }
                    for flag in [false, true] {
                        let operands = if shape.is_empty() {
                            "1.0,2.0"
                        } else {
                            "[1.0],[1.0],2.0"
                        };
                        let condition = if suffix.is_empty() {
                            String::new()
                        } else {
                            format!(",{flag}")
                        };
                        let source = format!(
                            "constraint {prefix}float_{shape}{relation}{suffix}({operands}{condition});solve satisfy;"
                        );
                        let valid = match suffix {
                            "_reif" => flag == truth,
                            "_imp" => !flag || truth,
                            _ => truth,
                        };
                        assert_eq!(
                            prepare("float-name.fzn", &source)
                                .unwrap()
                                .check(&Assignment::default())
                                .unwrap()
                                .status
                                == Status::Valid,
                            valid,
                            "{source}"
                        );
                    }
                }
            }
        }
        for (name, args) in [
            ("float_abs", "-2.0,2.0"),
            ("float_plus", "1.0,2.0,3.0"),
            ("float_times", "2.0,3.0,6.0"),
            ("float_div", "6.0,2.0,3.0"),
            ("float_min", "1.0,2.0,1.0"),
            ("float_max", "1.0,2.0,2.0"),
            ("int2float", "9007199254740993,9007199254740992.0"),
        ] {
            let source = format!("constraint {prefix}{name}({args});solve satisfy;");
            assert_eq!(
                prepare("float-alias.fzn", &source)
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                Status::Valid,
                "{source}"
            );
        }
    }
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
        "constraint gecode_float_acosh(1.0,1.0);solve satisfy;",
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
                let entries = if variable_array { "x" } else { value };
                let source = format!(
                    "predicate {name}(var int: i, array[int] of {role}{kind}: a, var {kind}: y); var {kind}: x; var {kind}: y ::is_defined_var; constraint {name}(1,[{entries}],y)::defines_var(y); solve satisfy;"
                );
                assert_eq!(
                    prepare("selection.fzn", &source)
                        .unwrap()
                        .check(&assignment(&format!("x={value};")))
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
    for (name, truth) in [
        ("eq", true),
        ("ne", false),
        ("le", true),
        ("lt", false),
        ("subset", true),
        ("superset", true),
    ] {
        for guard in [false, true] {
            let source = format!(
                "predicate set_{name}_imp(var set of int:a,var set of int:b,var bool:c); constraint set_{name}_imp({{}},{{}},{guard}); solve satisfy;"
            );
            let model = prepare("imp.fzn", &source).unwrap();
            assert_eq!(
                PreparedModel::from_json(&model.to_json().unwrap())
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                if !guard || truth {
                    Status::Valid
                } else {
                    Status::Invalid
                }
            );
        }
    }
    for prefix in ["", "gecode_", "fzn_"] {
        for name in ["set_in", "int_in"] {
            for (kind, value) in [("int", "1"), ("bool", "true")] {
                for suffix in ["", "_reif", "_imp"] {
                    for member in [false, true] {
                        let set = if member { "{1}" } else { "{}" };
                        let (parameter, flag) = if suffix.is_empty() {
                            ("", "")
                        } else {
                            (",var bool:b", ",true")
                        };
                        let source = format!(
                            "predicate {prefix}{name}{suffix}(var {kind}:x,set of int:s{parameter});constraint {prefix}{name}{suffix}({value},{set}{flag});solve satisfy;"
                        );
                        assert_eq!(
                            prepare("member-role.fzn", &source)
                                .unwrap()
                                .check(&Assignment::default())
                                .unwrap()
                                .status
                                == Status::Valid,
                            member,
                            "{source}"
                        );
                    }
                }
            }
        }
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

#[test]
fn float_functions_calculate_forward_and_check_supplied_outputs() {
    for (call, expected) in [
        ("float_sqrt(4.0,y)", "2.0"),
        ("float_exp(1.0,y)", "2.718281828459045"),
        ("float_ln(2.0,y)", "0.6931471805599453"),
        ("float_log10(100.0,y)", "2.0"),
        ("float_log2(8.0,y)", "3.0"),
        ("float_sin(0.5,y)", "0.479425538604203"),
        ("float_cos(0.5,y)", "0.8775825618903728"),
        ("float_tan(0.5,y)", "0.5463024898437905"),
        ("float_asin(0.5,y)", "0.5235987755982989"),
        ("float_acos(0.5,y)", "1.0471975511965979"),
        ("float_atan(1.0,y)", "0.7853981633974483"),
        ("float_sinh(1.0,y)", "1.1752011936438014"),
        ("float_cosh(1.0,y)", "1.5430806348152437"),
        ("float_tanh(1.0,y)", "0.7615941559557649"),
        ("float_asinh(1.0,y)", "0.881373587019543"),
        ("float_acosh(2.0,y)", "1.3169578969248166"),
        ("float_atanh(0.5,y)", "0.5493061443340548"),
        ("float_pow(-2.0,3.0,y)", "-8.0"),
        ("float_pow(-2.0,-3.0,y)", "-0.125"),
        ("float_pow(1e308,-2.0,y)", "0.0"),
        ("float_pow(0.0,0.0,y)", "1.0"),
        ("float_pow(4.0,0.5,y)", "2.0"),
        ("gecode_float_pow_base(1.0,-1e308,y)", "1.0"),
        ("gecode_float_pow_base(4.0,-0.5,y)", "0.5"),
        ("gecode_float_pow(-2.0,3,y)", "-8.0"),
        ("gecode_float_pow(-1.0,2147483646,y)", "1.0"),
        ("array_float_minimum(y,[2.0,1.0,1.0000000001])", "1.0"),
        ("array_float_maximum(y,[-2.0,-1.0])", "-1.0"),
        ("gecode_float_array_min([2.0,1.0],y)", "1.0"),
        ("gecode_float_array_max([-2.0,-1.0],y)", "-1.0"),
    ] {
        let source = format!(
            "var float:y::is_defined_var;constraint {call}::defines_var(y);constraint float_eq(y,{expected});solve minimize y;"
        );
        let model = prepare("functions.fzn", &source).unwrap();
        let derived = model.check(&Assignment::default()).unwrap();
        assert_eq!(derived.status, Status::Valid, "{call}");
        assert!(derived.completeness.complete, "{call}");
        let wrong = expected.parse::<f64>().unwrap() + 1.0;
        let supplied = model
            .check(&assignment(&format!("y={wrong:.17};")))
            .unwrap();
        assert_eq!(supplied.status, Status::Invalid, "{call}");
        assert!(matches!(
            supplied.violations[0].details,
            ViolationKind::FloatArithmetic { .. }
                | ViolationKind::FloatIntegerPower { .. }
                | ViolationKind::FloatArrayExtremum { .. }
        ));
        assert_eq!(
            model.check(&Assignment::default()).unwrap().status,
            Status::Valid
        );
    }
}

#[test]
fn float_function_domains_are_exact_and_failures_are_located() {
    for call in [
        "float_sqrt(-0.0000000001,0.0)",
        "float_ln(0.0,0.0)",
        "float_log2(-1.0,0.0)",
        "float_log10(0.0,0.0)",
        "float_asin(1.0000000000000002,0.0)",
        "float_acos(-1.0000000000000002,0.0)",
        "float_acosh(0.9999999999999999,0.0)",
        "float_atanh(1.0,0.0)",
        "float_atanh(-1.0,0.0)",
        "float_pow(-2.0,0.5,0.0)",
        "float_pow(0.0,-1.0,0.0)",
        "gecode_float_pow(0.0,0,1.0)",
        "gecode_float_pow(1e308,2,0.0)",
        "float_exp(1000.0,0.0)",
        "float_sinh(1000.0,0.0)",
        "float_cosh(-1000.0,0.0)",
        "float_pow(1e308,2.0,0.0)",
    ] {
        let model = prepare("domain.fzn", &format!("constraint {call};solve satisfy;")).unwrap();
        let error = model.check(&Assignment::default()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution, "{call}");
        assert_eq!(
            error.diagnostics[0].location.as_ref().unwrap().source,
            "domain.fzn"
        );
    }
    for call in [
        "float_sqrt(-0.0,y)",
        "float_asin(1.0,y)",
        "float_acos(-1.0,y)",
        "float_acosh(1.0,y)",
        "float_tan(1.5707963267948966,y)",
        "float_tan(4.71238898038469,y)",
        "float_asinh(1e308,y)",
        "float_tanh(-1e308,y)",
        "float_exp(-1000.0,y)",
    ] {
        let mut model = prepare(
            "boundary.fzn",
            &format!(
                "var float:y::is_defined_var;constraint {call}::defines_var(y);solve minimize y;"
            ),
        )
        .unwrap();
        model.set_float_tolerance(zincite_check::FloatTolerance::new(0.0, 0.0).unwrap());
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status, Status::Valid, "{call}");
        if call == "float_sqrt(-0.0,y)" {
            let Value::Float(value) = report.objective.unwrap().argument.value else {
                panic!()
            };
            assert_eq!(value.get().to_bits(), 0.0_f64.to_bits());
        }
    }
    for source in [
        "var float:y::is_defined_var;constraint float_exp(50.0,y)::defines_var(y);constraint float_floor(y,0);solve satisfy;",
        "var float:y::is_defined_var;constraint float_exp(-1000.0,y)::defines_var(y);constraint float_div(1.0,y,0.0);solve satisfy;",
    ] {
        assert_eq!(
            prepare("composed.fzn", source)
                .unwrap()
                .check(&Assignment::default())
                .unwrap_err()
                .kind,
            ErrorKind::Execution
        );
    }
}

#[test]
fn float_function_names_signatures_and_fixed_roles_follow_upstream() {
    for name in [
        "sqrt", "acos", "asin", "atan", "cos", "exp", "ln", "log10", "log2", "sin", "tan",
    ] {
        let input = if ["sqrt", "acos", "asin", "ln", "log10", "log2"].contains(&name) {
            "1.0"
        } else {
            "0.0"
        };
        for prefix in ["", "gecode_", "fzn_"] {
            let source = format!(
                "predicate {prefix}float_{name}(var float:source_value, var float:target_value); var float:y::is_defined_var;constraint {prefix}float_{name}({input},y)::defines_var(y);solve satisfy;"
            );
            assert_eq!(
                prepare("alias.fzn", &source)
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                Status::Valid
            );
        }
    }
    for prefix in ["", "gecode_", "fzn_"] {
        for (name, signature, call) in [
            (
                "gecode_float_pow",
                "var float:a,int:b,var float:c",
                "2.0,3,y",
            ),
            (
                "gecode_float_pow_base",
                "float:a,var float:b,var float:c",
                "2.0,3.0,y",
            ),
            (
                "gecode_float_array_min",
                "array[int] of var float:a,var float:b",
                "[1.0,2.0],y",
            ),
            (
                "gecode_float_array_max",
                "array[1..2] of var float:a,var float:b",
                "[1.0,2.0],y",
            ),
        ] {
            let source = format!(
                "predicate {prefix}{name}({signature});var float:y::is_defined_var;constraint {prefix}{name}({call})::defines_var(y);solve satisfy;"
            );
            assert_eq!(
                prepare("alias.fzn", &source)
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                Status::Valid
            );
        }
    }
    for source in [
        "constraint gecode_float_asinh(1.0,1.0);",
        "constraint fzn_float_pow(2.0,3.0,8.0);",
        "var float:b;constraint gecode_float_pow_base(b,2.0,4.0);",
        "constraint gecode_float_pow_base(0.0,2.0,0.0);",
        "constraint gecode_float_pow_base(-2.0,2.0,4.0);",
        "var int:e;constraint gecode_float_pow(2.0,e,4.0);",
        "constraint gecode_float_pow(2.0,2.0,4.0);",
        "constraint gecode_float_pow(2.0,-1,0.5);",
        "constraint gecode_float_pow(2.0,2147483647,1.0);",
        "constraint array_float_minimum(0.0,[]);",
        "constraint gecode_float_array_max([],0.0);",
        "constraint array_float_maximum(0.0,[1]);",
        "constraint float_exp([1.0],2.0);",
        "predicate gecode_float_pow(var float:a,var int:b,var float:c);",
        "predicate gecode_float_pow_base(var float:a,var float:b,var float:c);",
        "predicate array_float_minimum(array[int] of var float:a,var float:b);",
    ] {
        assert!(
            prepare("bad.fzn", &format!("{source}solve satisfy;")).is_err(),
            "{source}"
        );
    }
}

#[test]
fn float_functions_compose_with_aliases_domains_and_fresh_candidate_values() {
    let source = r#"
        var float:x;
        var 0.0..1.0:root::is_defined_var;
        var float:root_alias=root;
        var float:cube::is_defined_var;
        var float:smallest::is_defined_var;
        var float:repeated::is_defined_var;
        var float:repeat_alias=repeated;
        constraint float_sqrt(x,root)::defines_var(root);
        constraint gecode_float_pow(root_alias,3,cube)::defines_var(cube);
        constraint array_float_minimum(smallest,[1.0000000001,1.0])::defines_var(smallest);
        constraint float_sin(repeat_alias,repeated)::defines_var(repeated);
        constraint float_eq(smallest,1.0000000001);
        constraint float_le(cube,1.0);
        solve minimize smallest;
    "#;
    let mut model = prepare("composed.fzn", source).unwrap();
    let report = model.check(&assignment("x=4.0;")).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    assert_eq!(report.violations.len(), 2);
    assert!(matches!(
        report.violations[0].details,
        ViolationKind::Domain { .. }
    ));
    assert!(matches!(
        report.violations[1].details,
        ViolationKind::FloatRelation { .. }
    ));
    let Value::Float(minimum) = report.objective.unwrap().argument.value else {
        panic!()
    };
    assert_eq!(minimum.get(), 1.0);
    let report = model
        .check(&assignment("x=1.0;repeated=0.0;root=0.0;cube=99.0;"))
        .unwrap();
    assert!(report.completeness.complete);
    assert!(
        report
            .violations
            .iter()
            .any(|v| matches!(v.details, ViolationKind::FloatArithmetic { .. }))
    );
    assert!(report.violations.iter().any(|v| matches!(
        v.details,
        ViolationKind::FloatIntegerPower { exponent: 3, .. }
    )));
    assert_eq!(
        model
            .check(&assignment("x=1.0;repeated=0.0;"))
            .unwrap()
            .status,
        Status::Valid
    );
    let missing = model.check(&assignment("repeated=0.0;")).unwrap();
    assert_eq!(missing.status, Status::Incomplete);
    model.set_float_tolerance(zincite_check::FloatTolerance::new(0.0, 0.0).unwrap());
    let strict = model.check(&assignment("x=1.0;repeated=0.0;")).unwrap();
    assert_eq!(strict.status, Status::Invalid);
    assert_eq!(strict.violations.len(), 1);
    assert_eq!(
        strict.objective.unwrap().argument.value,
        Value::Float(minimum)
    );
}

#[test]
fn counting_names_and_declarations_follow_registry_literals() {
    let cases = [
        (
            "count",
            "array[int] of var int: a, var int: b, var int: c",
            "[1],1,1",
        ),
        (
            "count_eq",
            "array[int] of var int: a, var int: b, var int: c",
            "[1],1,1",
        ),
        (
            "count_reif",
            "array[int] of var int: a, var int: b, var int: c, var bool: d",
            "[1],1,1,true",
        ),
        (
            "count_eq_reif",
            "array[int] of var int: a, var int: b, var int: c, var bool: d",
            "[1],1,1,true",
        ),
        (
            "count_imp",
            "array[int] of var int: a, var int: b, var int: c, var bool: d",
            "[1],1,1,true",
        ),
        (
            "count_eq_imp",
            "array[int] of var int: a, var int: b, var int: c, var bool: d",
            "[1],1,1,true",
        ),
        (
            "at_least_int",
            "int: a, array[int] of var int: b, int: c",
            "1,[1],1",
        ),
        (
            "at_most_int",
            "int: a, array[int] of var int: b, int: c",
            "1,[1],1",
        ),
        (
            "among",
            "var int: a, array[int] of var int: b, set of int: c",
            "1,[1],{1}",
        ),
        ("nvalue", "var int: a, array[int] of var int: b", "1,[1]"),
        (
            "gecode_global_cardinality",
            "array[int] of var int: a, array[int] of int: b, array[int] of var int: c",
            "[1],[1],[1]",
        ),
        (
            "gecode_global_cardinality_closed",
            "array[int] of var int: a, array[int] of int: b, array[int] of var int: c",
            "[1],[1],[1]",
        ),
        (
            "global_cardinality_low_up",
            "array[int] of var int: a, array[int] of int: b, array[int] of int: c, array[int] of int: d",
            "[1],[1],[1],[1]",
        ),
        (
            "global_cardinality_low_up_closed",
            "array[int] of var int: a, array[int] of int: b, array[int] of int: c, array[int] of int: d",
            "[1],[1],[1],[1]",
        ),
        (
            "gecode_among_seq_int",
            "array[int] of var int: a, set of int: b, int: c, int: d, int: e",
            "[1],{1},1,1,1",
        ),
        (
            "gecode_among_seq_bool",
            "array[int] of var bool: a, bool: b, int: c, int: d, int: e",
            "[false],false,1,1,1",
        ),
        (
            "gecode_count_int_fzn",
            "array[int] of var int: a, var int: b, int: c, var int: d",
            "[1],1,1,1",
        ),
        (
            "gecode_count_matches_fzn",
            "array[int] of var int: a, array[int] of int: b, int: c, var int: d",
            "[1],[1],1,1",
        ),
    ];
    for (base, parameters, arguments) in cases {
        for prefix in ["", "gecode_", "fzn_"] {
            let name = format!("{prefix}{base}");
            let source = format!(
                "predicate {name}({parameters}); constraint {name}({arguments}); solve satisfy;"
            );
            let model =
                prepare("names.fzn", &source).unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert_eq!(
                model.check(&Assignment::default()).unwrap().status,
                Status::Valid,
                "{name}"
            );
            let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
            assert_eq!(
                loaded.check(&Assignment::default()).unwrap().status,
                Status::Valid
            );
            let wrong = source.replacen("array[int] of var", "array[int] of", 1);
            assert!(prepare("wrong-declaration.fzn", &wrong).is_err(), "{name}");
        }
    }
    for call in [
        "global_cardinality([1],[1],[1])",
        "fzn_global_cardinality([1],[1],[1])",
        "fzn_count_geq(1,[1],1)",
        "among_reif(1,[1],{1},true)",
        "count([true],1,1)",
        "count([1],1)",
        "gecode_count_int_fzn([],1,0,0)",
        "gecode_count_matches_fzn([],[1],1,0)",
        "gecode_global_cardinality([],[1],[])",
        "global_cardinality_low_up([],[1],[],[])",
        "gecode_among_seq_int([],{1},1,0,1)",
        "gecode_among_seq_int([1],{1},0,0,1)",
        "gecode_among_seq_bool([true],true,2,0,1)",
    ] {
        assert!(
            prepare("bad.fzn", &format!("constraint {call}; solve satisfy;")).is_err(),
            "{call}"
        );
    }
    for call in [
        "at_least_int(n,[],1)",
        "at_most_int(0,[],n)",
        "among(0,[],s)",
        "gecode_count_int_fzn([],1,n,0)",
        "gecode_global_cardinality([],[n],[0])",
    ] {
        assert!(
            prepare(
                "fixed.fzn",
                &format!("var int: n; var set of int: s; constraint {call}; solve satisfy;")
            )
            .is_err(),
            "{call}"
        );
    }
}

#[test]
fn counts_use_positions_compact_membership_and_native_code_direction() {
    let source = r#"
        var int: x;
        var int: total ::is_defined_var;
        var int: members ::is_defined_var;
        var int: distinct_values ::is_defined_var;
        var int: matches ::is_defined_var;
        var bool: same ::is_defined_var;
        constraint count([x,x,-9223372036854775808],x,total)::defines_var(total);
        constraint among(members,[x,x,-9223372036854775808],-9223372036854775808..9223372036854775807)::defines_var(members);
        constraint nvalue(distinct_values,[x,x,-9223372036854775808])::defines_var(distinct_values);
        constraint gecode_count_matches_fzn([x,9223372036854775807,-9223372036854775808],[7,-9223372036854775808,-9223372036854775808],1,matches)::defines_var(matches);
        constraint count_reif([x,x],x,2,same)::defines_var(same);
        constraint int_eq(total,2);
        constraint int_eq(members,3);
        constraint int_eq(distinct_values,2);
        constraint int_eq(matches,2);
        constraint bool_eq(same,true);
        solve satisfy;
    "#;
    let model = prepare("counts.fzn", source).unwrap();
    assert_eq!(
        model.check(&assignment("x=7;")).unwrap().status,
        Status::Valid
    );
    assert_eq!(
        model.check(&assignment("x=8;")).unwrap().status,
        Status::Invalid
    );
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
    assert_eq!(
        model.check(&assignment("x=7;")).unwrap().status,
        Status::Valid
    );
    for code in 1..=6 {
        for supplied in [0, 1, 2] {
            let holds = match code {
                1 => 1 == supplied,
                2 => 1 != supplied,
                3 => 1 <= supplied,
                4 => 1 < supplied,
                5 => 1 >= supplied,
                _ => 1 > supplied,
            };
            let model = prepare(
                "codes.fzn",
                &format!(
                    "constraint gecode_count_int_fzn([7],7,{code},{supplied}); solve satisfy;"
                ),
            )
            .unwrap();
            assert_eq!(
                model.check(&Assignment::default()).unwrap().status,
                if holds {
                    Status::Valid
                } else {
                    Status::Invalid
                }
            );
        }
    }
    let model = prepare(
        "empty.fzn",
        r#"
        var int: needle;
        var int: total ::is_defined_var;
        var bool: truth ::is_defined_var;
        var int: unresolved;
        constraint count([],needle,total)::defines_var(total);
        constraint count_reif([],needle,1,truth)::defines_var(truth);
        constraint count([],needle,1);
        constraint count_imp([unresolved],needle,unresolved,false);
        constraint bool_eq(truth,false);
        solve minimize total;
    "#,
    )
    .unwrap();
    let report = model.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    assert_eq!(report.objective.unwrap().argument.value, Value::Integer(0));
    assert_eq!(report.violations.len(), 1);
    assert!(
        report
            .completeness
            .blocked_checks
            .iter()
            .all(|blocked| matches!(blocked.check, CheckRef::Domain { .. }))
    );
    let ViolationKind::Count {
        calculated,
        selector:
            zincite_check::CountSelectorValue::Needle {
                value,
                irrelevant,
                unresolved,
            },
        ..
    } = &report.violations[0].details
    else {
        panic!("count finding")
    };
    assert_eq!(*calculated, 0);
    assert!(value.is_none() && *irrelevant);
    assert_eq!(unresolved.as_ref().unwrap().name, "needle");
    let model = prepare("no-witness.fzn", "var int: n ::is_defined_var; constraint gecode_count_int_fzn([1],1,3,n)::defines_var(n); solve satisfy;").unwrap();
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
}

#[test]
fn cardinality_and_windows_check_every_offending_position() {
    let source = r#"
        constraint gecode_global_cardinality([1,1,9],[1,1],[2,2]);
        constraint gecode_global_cardinality_closed([1,1,9,9],[1,1],[1,3]);
        constraint global_cardinality_low_up([1,1,9],[1,1],[0,2],[3,2]);
        constraint global_cardinality_low_up_closed([1,1,9],[1,1],[3,-1],[1,-2]);
        constraint gecode_global_cardinality([],[],[]);
        constraint gecode_global_cardinality_closed([1],[],[]);
        constraint gecode_global_cardinality_closed([],[1],[0]);
        constraint gecode_among_seq_int([1,1,1,0],{1},2,0,1);
        constraint gecode_among_seq_bool([false,false,true,false],false,2,2,2);
        constraint gecode_among_seq_bool([false,false],false,1,-2,8);
        constraint gecode_among_seq_int([0,1,2],{1},3,2,1);
        constraint at_least_int(-1,[],0);
        constraint at_most_int(-1,[],0);
        solve satisfy;
    "#;
    let model = prepare("distributions.fzn", source).unwrap();
    let report = model.check(&Assignment::default()).unwrap();
    assert_eq!(report.violations.len(), 7);
    let ViolationKind::Cardinality {
        counts,
        outside_cover,
        ..
    } = &report.violations[0].details
    else {
        panic!()
    };
    assert_eq!(
        counts.iter().map(|c| c.position).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(outside_cover, &[3, 4]);
    let ViolationKind::CountWindows { windows, .. } = &report.violations[3].details else {
        panic!()
    };
    assert_eq!(
        windows.iter().map(|w| w.position).collect::<Vec<_>>(),
        [1, 2]
    );
    let text = zincite_check::cli::render_text(&report);
    assert!(text.contains("outside cover") && text.contains("window[2]"));
}

#[test]
fn cardinality_outputs_complete_canonical_slots_and_check_supplied_conflicts() {
    for target in [
        "counts",
        "[first,second,first]",
        "[second,first,first]",
        "marked_scalars",
    ] {
        let array_marker = if target == "counts" {
            "::is_defined_var"
        } else {
            ""
        };
        let scalar_marker = if target == "counts" {
            ""
        } else {
            "::is_defined_var"
        };
        let annotation_target = if target == "marked_scalars" {
            "counts"
        } else {
            target
        };
        let source = format!(
            r#"
            var int: x;
            var 0..1: first {scalar_marker};
            var int: second {scalar_marker};
            array[1..3] of var int: counts {array_marker} = [first,second,first];
            var int: next ::is_defined_var;
            constraint gecode_global_cardinality([x,x,2],[1,2,1],counts)::defines_var({annotation_target})::checker_constraint("histogram",[9007199254740993]);
            constraint int_plus(first,1,next)::defines_var(next);
            constraint int_eq(next,2);
            solve minimize second;
        "#
        );
        let model = prepare("outputs.fzn", &source).unwrap();
        let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
        for candidate in ["x=1;", "x=1;second=1;", "x=1;first=7;", "x=2;", ""] {
            let report = model.check(&assignment(candidate)).unwrap();
            assert_eq!(
                serde_json::to_value(&report).unwrap(),
                serde_json::to_value(loaded.check(&assignment(candidate)).unwrap()).unwrap()
            );
        }
        let report = model.check(&assignment("x=1;second=1;")).unwrap();
        assert!(report.completeness.complete);
        assert_eq!(report.objective.unwrap().argument.value, Value::Integer(1));
        assert_eq!(report.violations.len(), 2); // Out-of-domain first still supplies next=3.
        let report = model.check(&assignment("x=1;first=7;")).unwrap();
        assert!(
            report
                .violations
                .iter()
                .any(|v| matches!(v.details, ViolationKind::Cardinality { .. }))
        );
        assert_eq!(
            model.check(&Assignment::default()).unwrap().status,
            Status::Incomplete
        );
    }
    let conflict = "var int: n ::is_defined_var; constraint gecode_global_cardinality([1],[1,2],[n,n])::defines_var([n,n]); solve satisfy;";
    let model = prepare("duplicate-output.fzn", conflict).unwrap();
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
    assert_eq!(
        model.check(&assignment("n=1;")).unwrap().status,
        Status::Invalid
    );
    let cycle = prepare("cycle.fzn", "var int: n ::is_defined_var; constraint gecode_global_cardinality([n],[1],[n])::defines_var([n]); solve satisfy;").unwrap();
    assert_eq!(
        cycle.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
    assert_eq!(
        cycle.check(&assignment("n=1;")).unwrap().status,
        Status::Valid
    );
    for source in [
        "var int: n ::is_defined_var; var int: m ::is_defined_var; constraint gecode_global_cardinality([],[1],[n])::defines_var([m]);",
        "var int: n ::is_defined_var; constraint count([],1,n)::defines_var([n]);",
        "var int: n ::is_defined_var; constraint count([],1,n)::defines_var(n); constraint count([],2,n)::defines_var(n);",
        "int: n=0; constraint gecode_global_cardinality([],[1],[n])::defines_var([n]);",
        "array[1..0] of var bool: flags ::is_defined_var=[]; constraint gecode_global_cardinality([],[],[])::defines_var(flags);",
    ] {
        assert!(
            prepare("target.fzn", &format!("{source} solve satisfy;")).is_err(),
            "{source}"
        );
    }
}

#[test]
fn ordering_exact_registry_names_and_signatures() {
    // Literal registry prefixes also apply to names already beginning with gecode_.
    let cases = [
        ("all_different_int", "array[int] of var int: a", "[1,2]"),
        (
            "all_different_offset",
            "array[int] of int: a,array[int] of var int: b",
            "[0,0],[1,2]",
        ),
        (
            "gecode_all_different_offset_fzn",
            "array[int] of int: a,array[int] of var int: b",
            "[0,0],[1,2]",
        ),
        ("all_equal_int", "array[int] of var int: a", "[1,1]"),
        (
            "array_int_lt",
            "array[int] of var int: a,array[int] of var int: b",
            "[1],[2]",
        ),
        (
            "array_int_lq",
            "array[int] of var int: a,array[int] of var int: b",
            "[1],[1]",
        ),
        (
            "array_bool_lt",
            "array[int] of var bool: a,array[int] of var bool: b",
            "[false],[true]",
        ),
        (
            "array_bool_lq",
            "array[int] of var bool: a,array[int] of var bool: b",
            "[true],[true]",
        ),
        (
            "array_int_minimum",
            "var int: a,array[int] of var int: b",
            "1,[2,1]",
        ),
        (
            "array_int_maximum",
            "var int: a,array[int] of var int: b",
            "2,[2,1]",
        ),
        (
            "gecode_minimum_arg_int_offset",
            "array[int] of var int: a,int: b,var int: c",
            "[1,2],5,5",
        ),
        (
            "gecode_maximum_arg_int_offset",
            "array[int] of var int: a,int: b,var int: c",
            "[1,2],5,6",
        ),
        (
            "gecode_minimum_arg_bool_offset",
            "array[int] of var bool: a,int: b,var int: c",
            "[false,true],5,5",
        ),
        (
            "gecode_maximum_arg_bool_offset",
            "array[int] of var bool: a,int: b,var int: c",
            "[false,true],5,6",
        ),
        (
            "sort",
            "array[int] of var int: a,array[int] of var int: b",
            "[2,1],[1,2]",
        ),
        (
            "inverse_offsets",
            "array[int] of var int: a,int: b,array[int] of var int: c,int: d",
            "[6,5],5,[11,10],10",
        ),
        ("increasing_int", "array[int] of var int: a", "[1,1,2]"),
        (
            "increasing_bool",
            "array[int] of var bool: a",
            "[false,true,true]",
        ),
        ("decreasing_int", "array[int] of var int: a", "[2,1,1]"),
        (
            "decreasing_bool",
            "array[int] of var bool: a",
            "[true,true,false]",
        ),
        (
            "gecode_precede",
            "array[int] of var int: a,int: b,int: c",
            "[1,2,2],1,2",
        ),
        (
            "member_int",
            "array[int] of var int: a,var int: b",
            "[1,2],2",
        ),
        (
            "gecode_member_int_reif",
            "array[int] of var int: a,var int: b,var bool: c",
            "[1,2],3,false",
        ),
        (
            "member_bool",
            "array[int] of var bool: a,var bool: b",
            "[false],false",
        ),
        (
            "gecode_member_bool_reif",
            "array[int] of var bool: a,var bool: b,var bool: c",
            "[false],true,false",
        ),
        (
            "gecode_array_int_ne",
            "array[int] of var int: a,array[int] of var int: b",
            "[],[1]",
        ),
        (
            "gecode_not_all_equal_int",
            "array[int] of var int: a",
            "[1,2]",
        ),
        (
            "gecode_array_bool_ne",
            "array[int] of var bool: a,array[int] of var bool: b",
            "[false],[true]",
        ),
        (
            "gecode_bool_int_channel",
            "array[int] of var bool: a,var int: b,int: c",
            "[false,true],-1,-2",
        ),
        (
            "gecode_sort_permutation",
            "array[int] of var int: a,array[int] of var int: b,array[int] of var int: c,int: d",
            "[3,1,2],[1,2,3],[0,-2,-1],-2",
        ),
        (
            "gecode_all_different_optional",
            "array[int] of var int: a,array[int] of var bool: b",
            "[1,1],[true,false]",
        ),
        (
            "gecode_all_different_except",
            "array[int] of var int: a,int: b",
            "[1,1],1",
        ),
        (
            "gecode_arg_min_int",
            "array[int] of var int: a,int: b,var int: c,bool: d",
            "[1,1],5,6,false",
        ),
        (
            "gecode_arg_max_int",
            "array[int] of var int: a,int: b,var int: c,bool: d",
            "[1,1],5,5,true",
        ),
        (
            "gecode_arg_min_bool",
            "array[int] of var bool: a,int: b,var int: c,bool: d",
            "[false,false],5,6,false",
        ),
        (
            "gecode_arg_max_bool",
            "array[int] of var bool: a,int: b,var int: c,bool: d",
            "[true,true],5,5,true",
        ),
    ];
    assert_eq!(cases.len(), 36);
    for (literal, signature, args) in cases {
        for prefix in ["", "gecode_", "fzn_"] {
            let name = format!("{prefix}{literal}");
            let source =
                format!("predicate {name}({signature}); constraint {name}({args}); solve satisfy;");
            let model = prepare(&name, &source).unwrap_or_else(|error| panic!("{name}: {error:?}"));
            let report = model.check(&Assignment::default()).unwrap();
            assert_eq!(report.status, Status::Valid, "{name}");
            let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
            assert_eq!(
                serde_json::to_value(report).unwrap(),
                serde_json::to_value(loaded.check(&Assignment::default()).unwrap()).unwrap()
            );
        }
    }
    for source in [
        "constraint fzn_all_different_int_reif([1],true);",
        "constraint fzn_lex_less_int([1],[2]);",
        "constraint member_int_reif([1],1,true);",
        "constraint fzn_inverse([1],[1]);",
        "constraint fzn_decreasing_float([1.0]);",
        "constraint fzn_decreasing_set([{1}]);",
        "constraint all_different_offset([0],[1,2]);",
        "constraint gecode_all_different_optional([1],[]);",
        "constraint array_int_minimum(1,[]);",
        "constraint gecode_arg_min_int([],0,0,true);",
        "constraint gecode_arg_min_int([1],-1,0,true);",
        "constraint inverse_offsets([],-1,[],0);",
        "constraint sort([1],[]);",
        "constraint gecode_sort_permutation([1],[1],[0,1],0);",
        "constraint gecode_arg_min_int([1,2],9223372036854775807,0,true);",
        "constraint gecode_bool_int_channel([false,true],0,9223372036854775807);",
        "var bool: flag; constraint gecode_arg_min_int([1],0,0,flag);",
        "var int: offset; constraint inverse_offsets([],offset,[],0);",
        "constraint increasing_bool([1]);",
        "predicate sort(array[int] of int: a, array[int] of var int: b);",
        "constraint gecode_arg_min_int([1],0,0,1);",
    ] {
        assert!(
            prepare("bad-ordering", &format!("{source} solve satisfy;")).is_err(),
            "{source}"
        );
    }
}

#[test]
fn ordering_empty_optional_order_and_exact_arithmetic() {
    let model = prepare(
        "ordering",
        r#"
        var int: missing; var bool: needle;
        constraint all_different_int([missing]);
        constraint all_equal_int([missing]);
        constraint increasing_int([missing]);
        constraint decreasing_bool([needle]);
        constraint gecode_all_different_optional([missing,1,1],[false,true,false]);
        constraint gecode_all_different_optional([missing],[needle]);
        constraint gecode_all_different_optional([missing,missing],[false,needle]);
        constraint gecode_all_different_except([4,4,1,2],4);
        constraint gecode_array_int_ne([missing],[]);
        constraint array_int_lt([],[1]);
        constraint array_int_lq([],[]);
        constraint array_bool_lt([false],[false,true]);
        constraint gecode_precede([1,1],2,3);
        constraint inverse_offsets([],0,[],3);
        constraint gecode_sort_permutation([],[],[],-9223372036854775808);
        constraint gecode_not_all_equal_int([]);
        constraint gecode_not_all_equal_int([missing]);
        constraint gecode_array_bool_ne([],[]);
        constraint member_bool([],needle);
        constraint gecode_bool_int_channel([],missing,-1);
        constraint all_different_int([3,3,3]);
        constraint all_equal_int([1,2,3]);
        constraint decreasing_int([1,3,2,4]);
        constraint increasing_bool([true,false,true,false]);
        constraint array_int_lt([1,2],[1]);
        constraint array_int_lt([2],[1,4]);
        constraint gecode_precede([2,1,2],1,2);
        constraint gecode_precede([3,1,3],3,3);
        solve satisfy;
    "#,
    )
    .unwrap();
    let report = model.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    assert!(
        report
            .completeness
            .blocked_checks
            .iter()
            .all(|check| !matches!(check.check, CheckRef::Constraint { .. })),
        "{report:?}"
    );
    assert_eq!(report.violations.len(), 13, "{report:?}");
    let findings = serde_json::to_value(&report).unwrap();
    assert_eq!(
        findings["violations"][5]["details"]["finding"]["conflicts"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(zincite_check::cli::render_text(&report).contains("(1, 2), (1, 3), (2, 3)"));
    for call in [
        "all_different_int([x,x])",
        "all_different_offset([0,1],[2,1])",
        "gecode_all_different_optional([1,1],[true,true])",
        "gecode_all_different_except([1,1],2)",
        "gecode_minimum_arg_int_offset([1,1],5,6)",
        "gecode_arg_max_bool([true,true],5,6,true)",
    ] {
        let declarations = if call.contains('x') {
            "var int: x=1;"
        } else {
            ""
        };
        let result = prepare(
            "conflict",
            &format!("{declarations} constraint {call}; solve satisfy;"),
        )
        .unwrap()
        .check(&Assignment::default())
        .unwrap();
        assert_eq!(result.status, Status::Invalid, "{call}");
    }
    let overflow = prepare(
        "overflow",
        "constraint all_different_offset([1],[9223372036854775807]); solve satisfy;",
    )
    .unwrap();
    let error = overflow.check(&Assignment::default()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Execution);
    assert_eq!(error.diagnostics[0].location.as_ref().unwrap().line, 1);
    let boundary = prepare("bounds", "constraint array_int_minimum(-9223372036854775808,[-9223372036854775808,9223372036854775807]); constraint array_int_maximum(9223372036854775807,[-9223372036854775808,9223372036854775807]); solve satisfy;").unwrap();
    assert_eq!(
        boundary.check(&Assignment::default()).unwrap().status,
        Status::Valid
    );
}

#[test]
fn ordering_sort_inverse_channels_and_unique_completion() {
    let source = include_str!("../../../examples/checker-compilation/ordering.fzn");
    let model = prepare("completion", source).unwrap();
    let candidate = assignment("input=[3,1,2]; selected=9007199254740994; forward=[7,5,6];");
    let report = model.check(&candidate).unwrap();
    assert_eq!(report.status, Status::Valid);
    assert_eq!(
        serde_json::to_value(&report).unwrap()["objective"]["argument"]["value"]["value"],
        "9007199254740994"
    );
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        serde_json::to_value(loaded.check(&candidate).unwrap()).unwrap()
    );
    let tied = model
        .check(&assignment(
            "input=[1,1,2]; selected=9007199254740993; forward=[7,5,6];",
        ))
        .unwrap();
    assert_eq!(tied.status, Status::Incomplete);
    assert!(
        tied.completeness
            .unresolved_variables
            .iter()
            .any(|variable| variable.name == "any_min")
    );
    assert!(
        tied.completeness
            .unresolved_variables
            .iter()
            .any(|variable| variable.name == "permutation")
    );
    let supplied_ties = model.check(&assignment("input=[1,1,2]; selected=9007199254740993; forward=[7,5,6]; permutation=[-1,-2,0]; any_min=9007199254740994;")).unwrap();
    assert_eq!(supplied_ties.status, Status::Valid);
    let conflict = model.check(&assignment("input=[3,1,2]; selected=9007199254740994; forward=[7,5,6]; sorted=[1,1,3]; permutation=[-2,-2,9223372036854775807]; backward=[10,11,12]; flags=[true,true,false]; first_min=9007199254740995; member=false;")).unwrap();
    assert_eq!(conflict.status, Status::Invalid);
    assert!(conflict.completeness.complete);
    assert!(conflict.violations.len() >= 6);
    let text = zincite_check::cli::render_text(&conflict);
    assert!(text.contains("permutation faults") && text.contains("one-hot channel"));
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
    assert_eq!(model.check(&candidate).unwrap().status, Status::Valid);
}

#[test]
fn ordering_bidirectional_outputs_aliases_cycles_and_partial_faults() {
    for (relation, declarations, candidate) in [
        (
            "inverse_offsets(f,5,g,10)::defines_var(f)",
            "var int: a ::is_defined_var; var int: b ::is_defined_var; var int: c ::is_defined_var; var int: d; var int: e; var int: h; array[1..3] of var int: f = [a,b,c]; array[1..3] of var int: g = [d,e,h];",
            "g=[12,10,11];",
        ),
        (
            "gecode_bool_int_channel(flags,index,-2)::defines_var(index)",
            "var bool: a; var bool: b; var bool: c; array[1..3] of var bool: flags = [a,b,c]; var -2..0: index ::is_defined_var;",
            "flags=[false,true,false];",
        ),
        (
            "sort([2,2],[a,a])::defines_var([a,a])",
            "var int: a ::is_defined_var;",
            "",
        ),
        (
            "inverse_offsets([5,5],5,[a,a],10)::defines_var([a,a])",
            "var int: a ::is_defined_var;",
            "a=10;",
        ),
    ] {
        let model = prepare(
            "directions",
            &format!("{declarations} constraint {relation}; solve satisfy;"),
        )
        .unwrap();
        let report = model.check(&assignment(candidate)).unwrap();
        assert_eq!(
            report.status,
            if relation.starts_with("inverse_offsets([5,5]") {
                Status::Invalid
            } else {
                Status::Valid
            },
            "{relation}"
        );
    }
    for source in [
        "var int: a ::is_defined_var; constraint sort([1,2],[a,a])::defines_var([a,a]);",
        "var bool: a ::is_defined_var; constraint gecode_bool_int_channel([a,a],1,0)::defines_var([a,a]);",
        "var int: a ::is_defined_var; constraint sort([a,2],[a,2])::defines_var(a);",
        "var int: a ::is_defined_var; constraint inverse_offsets([a],0,[a],0)::defines_var(a);",
        "var int: a ::is_defined_var; constraint member_int([1],a)::defines_var(a);",
        "var int: a ::is_defined_var; constraint gecode_arg_max_int([1,1],0,a,false)::defines_var(a);",
    ] {
        let model = prepare("ambiguous", &format!("{source} solve satisfy;")).unwrap();
        assert_eq!(
            model.check(&Assignment::default()).unwrap().status,
            Status::Incomplete,
            "{source}"
        );
    }
    let partial = prepare("partial", "var int: missing; var bool: b; constraint inverse_offsets([5,5],5,[missing,missing],10); constraint gecode_bool_int_channel([false,false],missing,0); constraint gecode_bool_int_channel([b],-9223372036854775808,9223372036854775807); constraint gecode_arg_min_int([missing],0,9223372036854775807,false); solve satisfy;").unwrap();
    let report = partial.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert_eq!(report.violations.len(), 4);
    assert!(!report.completeness.complete);
    let domain = prepare("domain", "var 0..1: result ::is_defined_var; constraint array_int_maximum(result,[2,3])::defines_var(result); constraint int_le(result,0); solve satisfy;").unwrap().check(&Assignment::default()).unwrap();
    assert_eq!(domain.violations.len(), 2);
    let empty = prepare("empty-member", "var bool: missing; var bool: result ::is_defined_var; constraint gecode_member_bool_reif([],missing,result)::defines_var(result); constraint bool_eq(result,false); solve satisfy;").unwrap().check(&Assignment::default()).unwrap();
    assert_eq!(empty.status, Status::Incomplete);
    assert!(empty.violations.is_empty());
    for source in [
        "var int: a ::is_defined_var; var int: b ::is_defined_var; array[1..2] of var int: x=[a,b]; constraint sort([1,2],x)::defines_var(x); constraint sort([1,2],x)::defines_var(x);",
        "var bool: a ::is_defined_var; var bool: b ::is_defined_var; array[1..2] of var bool: x=[a,b]; constraint increasing_bool(x)::defines_var(x);",
        "var int: a ::is_defined_var; var int: b ::is_defined_var; constraint sort([1,2],[a,b])::defines_var([a]);",
        "array[1..0] of var bool: flags=[]; constraint sort([],[])::defines_var(flags);",
        "array[1..0] of var int: indexes=[]; var int: index; constraint gecode_bool_int_channel([],index,0)::defines_var(indexes);",
    ] {
        assert!(
            prepare("bad-target", &format!("{source} solve satisfy;")).is_err(),
            "{source}"
        );
    }
}

#[test]
fn legacy_float_and_set_decreasing_use_adjacent_relations_and_boolean_composition() {
    let source = r#"
        var float: left; var float: right;
        var bool: float_order ::is_defined_var; var bool: set_order ::is_defined_var;
        var bool: is_subset ::is_defined_var; var bool: decreasing ::is_defined_var;
        constraint float_le_reif(right,left,float_order)::defines_var(float_order);
        constraint set_le_reif({1,2,100},{1,9},set_order)::defines_var(set_order);
        constraint set_subset_reif({1,2,100},{1,9},is_subset)::defines_var(is_subset);
        constraint array_bool_and([float_order,set_order],decreasing)::defines_var(decreasing);
        constraint bool_eq(decreasing,true);
        constraint bool_eq(is_subset,false);
        constraint float_le_imp(100.0,0.0,false);
        constraint set_le_imp({9},{1},false);
        solve satisfy;
    "#;
    let mut model = prepare("legacy-decreasing", source).unwrap();
    let candidate = assignment("left=1.0;right=1.0000000001;");
    assert_eq!(model.check(&candidate).unwrap().status, Status::Valid);
    model.set_float_tolerance(zincite_check::FloatTolerance::new(2e-6, 0.0).unwrap());
    assert_eq!(
        model
            .check(&assignment("left=1.0;right=1.000001;"))
            .unwrap()
            .status,
        Status::Valid
    );
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    assert_eq!(loaded.float_tolerance(), model.float_tolerance());
    model.set_float_tolerance(zincite_check::FloatTolerance::new(0.0, 0.0).unwrap());
    let strict = model.check(&candidate).unwrap();
    assert_eq!(strict.status, Status::Invalid);
    assert_eq!(strict.violations.len(), 1);
}

#[test]
fn table_names_signatures_and_malformed_source() {
    for prefix in ["", "gecode_", "fzn_"] {
        for (kind, tuple, table) in [
            ("int", "1,2", "1,2,1,2"),
            ("bool", "false,true", "false,true,false,true"),
        ] {
            for suffix in ["", "_reif", "_imp"] {
                let name = format!("{prefix}gecode_table_{kind}{suffix}");
                let extra = if suffix.is_empty() {
                    ""
                } else {
                    ",var bool: answer"
                };
                let argument = if suffix.is_empty() { "" } else { ",true" };
                let source = format!(
                    "predicate {name}(array[int] of var {kind}: arbitrary,array[int] of {kind}: rows{extra}); constraint {name}([{tuple}],[{table}]{argument}); solve satisfy;"
                );
                let model = prepare("table-names", &source).unwrap();
                assert_eq!(
                    model.check(&Assignment::default()).unwrap().status,
                    Status::Valid,
                    "{name}"
                );
                let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
                assert_eq!(
                    loaded.check(&Assignment::default()).unwrap().status,
                    Status::Valid
                );
            }
        }
    }
    for source in [
        "constraint table_int([1],[1]);",
        "constraint fzn_table_int([1],[1]);",
        "constraint gecode_gecode_gecode_table_int([1],[1]);",
        "constraint gecode_table_int([1],[1],true);",
        "constraint gecode_table_bool([1],[true]);",
        "constraint gecode_table_int([1],[true]);",
        "constraint gecode_table_bool_reif([true],[true],1);",
        "var int: x; constraint gecode_table_int([1],[x]);",
        "var bool: x; constraint gecode_table_bool([true],[x]);",
        "constraint gecode_table_int_imp([1,2],[1],false);",
        "constraint gecode_table_bool_imp([],[false],false);",
        "array[1..2] of int: rows=[1]; constraint gecode_table_int([1],rows);",
        "predicate gecode_table_int(array[int] of int: a,array[int] of int: b); constraint gecode_table_int([1],[1]);",
        "predicate gecode_table_bool(array[int] of var bool: a,array[int] of var bool: b); constraint gecode_table_bool([true],[true]);",
        "predicate gecode_table_int(array[int] of var int: a,array[int] of int: b,var bool: c);",
    ] {
        assert!(
            prepare("bad-table", &format!("{source} solve satisfy;")).is_err(),
            "{source}"
        );
    }
}

#[test]
fn tables_check_rows_empty_guards_reification_and_completion() {
    for (call, status) in [
        ("gecode_table_int([1,2],[1,2,3,4])", Status::Valid),
        ("gecode_table_int([1,3],[1,2,3,4])", Status::Invalid),
        ("gecode_table_int([1,2],[1,2,1,2])", Status::Valid),
        (
            "gecode_table_int([-9223372036854775808,9223372036854775807],[-9223372036854775808,9223372036854775807])",
            Status::Valid,
        ),
        (
            "gecode_table_bool([true,false],[false,true,true,false])",
            Status::Valid,
        ),
        (
            "gecode_table_bool_reif([true,false],[false,true],false)",
            Status::Valid,
        ),
        ("gecode_table_int_reif([1],[1],false)", Status::Invalid),
        ("gecode_table_int_reif([1],[],false)", Status::Valid),
        ("gecode_table_int([],[])", Status::Invalid),
        ("gecode_table_bool_reif([],[],false)", Status::Valid),
        ("gecode_table_bool_imp([],[],true)", Status::Invalid),
        (
            "gecode_table_int_imp([missing],[],false)",
            Status::Incomplete,
        ),
        (
            "gecode_table_int_reif([missing],[],false)",
            Status::Incomplete,
        ),
    ] {
        let declaration = if call.contains("missing") {
            "var int: missing;"
        } else {
            ""
        };
        let model = prepare(
            "table-rows",
            &format!("{declaration} constraint {call}; solve satisfy;"),
        )
        .unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status, status, "{call}");
        if call.contains("missing") && call.contains("_imp") {
            assert!(
                report
                    .completeness
                    .blocked_checks
                    .iter()
                    .all(|blocked| matches!(blocked.check, CheckRef::Domain { .. }))
            );
        }
    }
    for source in [
        "var int: x; var bool: r ::is_defined_var; constraint gecode_table_int_reif([x],[],r)::defines_var(r);",
        "var bool: r ::is_defined_var; constraint gecode_table_bool_reif([r],[true],r)::defines_var(r);",
        "var bool: a ::is_defined_var; var bool: b ::is_defined_var; constraint gecode_table_bool_reif([a],[true],b)::defines_var(b); constraint gecode_table_bool_reif([b],[true],a)::defines_var(a);",
        "var int: x ::is_defined_var; constraint gecode_table_int([x],[1])::defines_var(x);",
        "var bool: guard ::is_defined_var; constraint gecode_table_int_imp([1],[1],guard)::defines_var(guard);",
    ] {
        let model = prepare("table-no-choice", &format!("{source} solve satisfy;")).unwrap();
        if source.contains("var bool: a") {
            assert_eq!(
                model.check(&assignment("a=true;b=true;")).unwrap().status,
                Status::Valid
            );
        }
        assert_eq!(
            model.check(&Assignment::default()).unwrap().status,
            Status::Incomplete,
            "{source}"
        );
    }
    let empty = prepare("zero-width", "var bool: r ::is_defined_var; constraint gecode_table_int_reif([],[],r)::defines_var(r); constraint bool_eq(r,false); solve satisfy;").unwrap();
    assert_eq!(
        empty.check(&Assignment::default()).unwrap().status,
        Status::Valid
    );
    let domain = prepare("table-domain", "var bool: r ::is_defined_var = false; constraint gecode_table_int_reif([1],[1],r)::defines_var(r); constraint bool_eq(r,false); solve satisfy;").unwrap();
    assert_eq!(
        domain
            .check(&Assignment::default())
            .unwrap()
            .violations
            .len(),
        2
    );
}

#[test]
fn table_reports_round_trip_with_metadata_and_fresh_assignments() {
    let source = include_str!("../../../examples/checker-compilation/tables.fzn");
    let model = prepare("tables.fzn", source).unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for (candidate, status, complete) in [
        ("x=9007199254740993;flag=true;", Status::Valid, true),
        ("x=3;flag=false;", Status::Invalid, true),
        (
            "x=9007199254740993;flag=true;member=false;",
            Status::Invalid,
            true,
        ),
        ("", Status::Incomplete, false),
        ("flag=false;bool_member=true;", Status::Invalid, false),
    ] {
        let input = assignment(candidate);
        let report = model.check(&input).unwrap();
        assert_eq!(report.status, status);
        assert_eq!(report.completeness.complete, complete);
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::to_value(loaded.check(&input).unwrap()).unwrap()
        );
        if candidate.contains("member=false") {
            let report = serde_json::to_value(report).unwrap();
            let finding = &report["violations"][0];
            assert_eq!(finding["details"]["kind"], "table");
            assert_eq!(finding["details"]["table"]["values"][0], "9007199254740993");
            assert_eq!(finding["details"]["width"], 2);
            assert_eq!(finding["details"]["membership"], true);
            assert_eq!(finding["details"]["tuple"][0]["variable"]["name"], "x");
            assert_eq!(
                finding["details"]["condition"]["result"]["value"]["value"],
                false
            );
            assert_eq!(
                finding["check"]["constraint"]["metadata"]["application"]["id"],
                "integer_table"
            );
        }
    }
}

#[test]
fn automaton_names_signatures_and_fixed_validation() {
    let forms = [
        (
            "gecode_regular",
            "array[int] of var int: a,int: b,int: c,array[int] of int: d,int: e,set of int: f",
            "[],1,1,[1],1,{1}",
        ),
        (
            "gecode_regular_set",
            "array[int] of var int: a,int: b,int: c,int: d,array[int] of int: e,int: f,set of int: g",
            "[],1,-1,0,[1,1],1,{1}",
        ),
        (
            "gecode_regular_nfa_int_fzn",
            "array[int] of var int: a,int: b,int: c,int: d,array[int] of set of int: e,int: f,set of int: g",
            "[],1,-1,2,[{1},{1}],1,{1}",
        ),
    ];
    for (base, signature, args) in forms {
        for prefix in ["", "gecode_", "fzn_"] {
            let name = format!("{prefix}{base}");
            let source =
                format!("predicate {name}({signature}); constraint {name}({args}); solve satisfy;");
            assert_eq!(
                prepare(&name, &source)
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                Status::Valid
            );
            let wrong = source.replace("array[int] of var int: a", "array[int] of var bool: a");
            assert!(prepare("wrong-signature", &wrong).is_err(), "{wrong}");
        }
    }
    for call in [
        "regular([],1,1,[1],1,{1})",
        "regular_set([],1,1,1,[1],1,{1})",
        "regular_nfa([],1,1,1,[{1}],1,{1})",
        "fzn_regular([],1,1,[1],1,{1})",
        "gecode_regular_reif([],1,1,[1],1,{1},true)",
        "fzn_fzn_gecode_regular([],1,1,[1],1,{1})",
        "gecode_regular([],1,1,[1],1)",
        "gecode_regular([true],1,1,[1],1,{1})",
        "gecode_regular([],q,1,[1],1,{1})",
        "gecode_regular([],1,q,[1],1,{1})",
        "gecode_regular([],1,1,[q],1,{1})",
        "gecode_regular([],1,1,[1],q,{1})",
        "gecode_regular([],1,1,[1],1,finals)",
        "gecode_regular([],0,1,[],1,{})",
        "gecode_regular([],1,0,[],1,{})",
        "gecode_regular([],1,1,[],1,{})",
        "gecode_regular([],1,1,[1,1],1,{})",
        "gecode_regular([],1,1,[-1],1,{})",
        "gecode_regular([],1,1,[2],1,{})",
        "gecode_regular([],1,1,[0],0,{})",
        "gecode_regular([],1,1,[0],1,{0})",
        "gecode_regular([],1,1,[0],1,{2})",
        "gecode_regular([],9223372036854775807,9223372036854775807,[],1,{})",
        "gecode_regular_set([],1,1,0,[],1,{})",
        "gecode_regular_set([],1,-9223372036854775808,9223372036854775807,[],1,{})",
        "gecode_regular_nfa_int_fzn([],1,1,0,[],1,{})",
        "gecode_regular_nfa_int_fzn([],1,1,1,[{0}],1,{})",
        "gecode_regular_nfa_int_fzn([],1,1,1,[{2}],1,{})",
        "gecode_regular_nfa_int_fzn([],1,1,1,[targets],1,{})",
        "gecode_regular_nfa_int_fzn([],1,-2147483647,1,[{}],1,{})",
        "gecode_regular_nfa_int_fzn([],1,2147483646,2,[{},{}],1,{})",
        "gecode_regular_nfa_int_fzn([],1,9223372036854775807,2,[],1,{})",
    ] {
        let source = format!(
            "var int: q;var set of int: finals;var set of int: targets;constraint {call};solve satisfy;"
        );
        let errors = prepare("bad-automaton", &source).unwrap_err();
        assert!(
            errors.iter().all(|error| error.location.is_some()),
            "{call}"
        );
    }
}

#[test]
fn concrete_automata_distinguish_layout_alphabets_and_empty_words() {
    for (call, status, rejection) in [
        (
            "gecode_regular([1,2],2,2,[2,0,2,1],1,{1})",
            Status::Valid,
            "",
        ),
        (
            "gecode_regular([1,1],2,2,[2,0,2,1],1,{1})",
            Status::Invalid,
            "nonaccepting_end",
        ),
        (
            "gecode_regular([2],2,2,[2,0,2,1],1,{1})",
            Status::Invalid,
            "dead_path",
        ),
        (
            "gecode_regular([0],1,1,[1],1,{1})",
            Status::Invalid,
            "out_of_alphabet",
        ),
        (
            "gecode_regular_set([-2,-1],2,-2,-1,[2,0,0,1],1,{1})",
            Status::Valid,
            "",
        ),
        ("gecode_regular_set([5],1,5,5,[1],1,{1})", Status::Valid, ""),
        (
            "gecode_regular_set([6],1,5,5,[1],1,{1})",
            Status::Invalid,
            "out_of_alphabet",
        ),
        (
            "gecode_regular_nfa_int_fzn([-1,0],3,-1,2,[{1,2},{},{},{3},{},{3}],1,{3})",
            Status::Valid,
            "",
        ),
        (
            "gecode_regular_nfa_int_fzn([-1,-1],3,-1,2,[{1,2},{},{},{3},{},{3}],1,{3})",
            Status::Invalid,
            "nonaccepting_end",
        ),
        (
            "gecode_regular_nfa_int_fzn([0],1,0,1,[{}],1,{1})",
            Status::Invalid,
            "dead_path",
        ),
        (
            "gecode_regular_nfa_int_fzn([-2147483646],1,-2147483646,1,[{1}],1,{1})",
            Status::Valid,
            "",
        ),
        (
            "gecode_regular_nfa_int_fzn([2147483646],1,2147483646,1,[{1}],1,{1})",
            Status::Valid,
            "",
        ),
        ("gecode_regular([],1,1,[0],1,{1})", Status::Valid, ""),
        (
            "gecode_regular([],1,1,[0],1,{})",
            Status::Invalid,
            "nonaccepting_end",
        ),
        (
            "gecode_regular_nfa_int_fzn([],1,0,1,[{}],1,{1})",
            Status::Valid,
            "",
        ),
        (
            "gecode_regular_nfa_int_fzn([],1,0,1,[{}],1,{})",
            Status::Invalid,
            "nonaccepting_end",
        ),
    ] {
        let model = prepare("word", &format!("constraint {call};solve satisfy;")).unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status, status, "{call}");
        if status == Status::Invalid {
            assert_eq!(
                serde_json::to_value(report).unwrap()["violations"][0]["details"]["finding"]["rejection"]
                    ["kind"],
                rejection,
                "{call}"
            );
        }
    }
    let alias = prepare("alias", "var int: x;var int: alias=x;constraint gecode_regular([x,alias],1,1,[1],1,{1});solve satisfy;").unwrap();
    assert_eq!(
        alias.check(&assignment("x=1;")).unwrap().status,
        Status::Valid
    );
    for call in [
        "gecode_regular([x],1,1,[0],1,{})",
        "gecode_regular_nfa_int_fzn([x],1,1,1,[{}],1,{})",
    ] {
        let model = prepare(
            "missing",
            &format!("var int:x::is_defined_var;constraint {call}::defines_var(x);solve satisfy;"),
        )
        .unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status, Status::Incomplete);
        assert!(report.violations.is_empty());
        assert!(
            report
                .completeness
                .blocked_checks
                .iter()
                .any(|blocked| matches!(blocked.check, CheckRef::Constraint { .. }))
        );
    }
}

#[test]
fn automaton_reports_round_trip_and_start_fresh() {
    let model = prepare(
        "automata.fzn",
        include_str!("../../../examples/checker-compilation/automata.fzn"),
    )
    .unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for (candidate, status, complete) in [
        ("x=9007199254740993;y=1;a=-1;b=0;", Status::Valid, true),
        ("x=9007199254740992;y=2;a=-1;b=-1;", Status::Invalid, true),
        ("", Status::Incomplete, false),
        ("x=9007199254740993;y=1;a=0;b=0;", Status::Invalid, true),
        ("y=2;", Status::Invalid, false),
        ("x=9007199254740993;y=1;a=-1;b=0;", Status::Valid, true),
    ] {
        let input = assignment(candidate);
        let report = model.check(&input).unwrap();
        assert_eq!(report.status, status);
        assert_eq!(report.completeness.complete, complete);
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json,
            serde_json::to_value(loaded.check(&input).unwrap()).unwrap()
        );
        if candidate.contains("9007199254740992") {
            assert_eq!(report.violations.len(), 3);
            let finding = &json["violations"][0];
            assert_eq!(
                finding["details"]["finding"]["minimum_symbol"],
                "9007199254740993"
            );
            assert_eq!(
                finding["details"]["finding"]["word"][0]["variable"]["name"],
                "x"
            );
            assert_eq!(finding["details"]["finding"]["rejection"]["position"], 1);
            assert_eq!(
                finding["check"]["constraint"]["metadata"]["application"]["id"],
                "exact_word"
            );
            assert_eq!(
                finding["check"]["constraint"]["metadata"]["application"]["entities"][0],
                "9007199254740993"
            );
            assert_eq!(
                finding["check"]["constraint"]["metadata"]["label"],
                "Exact symbol"
            );
        }
    }
}

#[test]
fn circuit_names_signatures_and_fixed_shapes_are_exact() {
    for prefix in ["", "gecode_", "fzn_"] {
        for (name, signature, args) in [
            (
                "gecode_circuit",
                "int: banana,array[int] of var int: pear",
                "0,[0]",
            ),
            (
                "gecode_circuit_cost",
                "array[int] of int: banana,array[int] of var int: pear,var int: kiwi",
                "[-7],[0],-7",
            ),
            (
                "gecode_circuit_cost_array",
                "array[int] of int: banana,array[int] of var int: pear,array[int] of var int: kiwi,var int: melon",
                "[-7],[0],[-7],-7",
            ),
        ] {
            let source = format!(
                "predicate {prefix}{name}({signature});constraint {prefix}{name}({args});solve satisfy;"
            );
            assert_eq!(
                prepare("names", &source)
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                Status::Valid,
                "{source}"
            );
        }
    }
    for call in [
        "circuit([0])",
        "fzn_circuit([0])",
        "circuit_cost([0],[0],0)",
        "circuit_cost_array([0],[0],[0],0)",
        "gecode_circuit_reif(0,[0],true)",
        "fzn_fzn_gecode_circuit(0,[0])",
        "gecode_circuit([0],0)",
        "gecode_circuit(0,[true])",
        "gecode_circuit(v,[0])",
        "gecode_circuit(0,[])",
        "gecode_circuit(-1,[0])",
        "gecode_circuit(9223372036854775807,[0,1])",
        "gecode_circuit(0,[0],0)",
        "gecode_circuit_cost([],[],0)",
        "gecode_circuit_cost([0],[0,1],0)",
        "gecode_circuit_cost([v],[0],0)",
        "gecode_circuit_cost([0],[0],true)",
        "gecode_circuit_cost_array([0],[0],[],0)",
        "gecode_circuit_cost_array([0],[0],[true],0)",
        "gecode_circuit_cost_array([0],[0],[0],0,0)",
    ] {
        let errors = prepare(
            "bad",
            &format!("var int:v;constraint {call};solve satisfy;"),
        )
        .unwrap_err();
        assert!(
            errors.iter().all(|error| error.location.is_some()),
            "{call}"
        );
    }
    for signature in [
        "var int:o,array[int] of var int:x",
        "int:o,array[int] of int:x",
        "int:o,var int:x",
    ] {
        assert!(
            prepare(
                "bad-declaration",
                &format!("predicate gecode_circuit({signature});solve satisfy;")
            )
            .is_err()
        );
    }
}

#[test]
fn circuits_check_topology_offsets_layout_and_actual_cost_sums() {
    for (call, status, kinds) in [
        ("gecode_circuit(0,[0])", Status::Valid, vec![]),
        (
            "gecode_circuit(9223372036854775807,[9223372036854775807])",
            Status::Valid,
            vec![],
        ),
        (
            "gecode_circuit(9007199254740993,[9007199254740994,9007199254740993])",
            Status::Valid,
            vec![],
        ),
        (
            "gecode_circuit(0,[1,0,3,2])",
            Status::Invalid,
            vec!["subcycle", "subcycle"],
        ),
        (
            "gecode_circuit(0,[1,1])",
            Status::Invalid,
            vec!["repeated_successor", "subcycle"],
        ),
        (
            "gecode_circuit(9007199254740993,[-9223372036854775808,9223372036854775807])",
            Status::Invalid,
            vec!["out_of_range", "out_of_range"],
        ),
        (
            "gecode_circuit_cost([1,7,-4,2],[1,0],3)",
            Status::Valid,
            vec![],
        ),
        (
            "gecode_circuit_cost_array([1,7,-4,2],[1,0],[8,-4],4)",
            Status::Invalid,
            vec!["edge_cost"],
        ),
        (
            "gecode_circuit_cost_array([1,7,-4,2],[1,0],[8,-4],3)",
            Status::Invalid,
            vec!["edge_cost", "total_cost"],
        ),
        (
            "gecode_circuit_cost([1,7,-4,2],[1,0],4)",
            Status::Invalid,
            vec!["total_cost"],
        ),
        (
            "gecode_circuit_cost([0,9007199254740993,-9007199254740992,0],[1,0],1)",
            Status::Valid,
            vec![],
        ),
    ] {
        let model = prepare("topology", &format!("constraint {call};solve satisfy;")).unwrap();
        let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status, status, "{call}");
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json,
            serde_json::to_value(loaded.check(&Assignment::default()).unwrap()).unwrap()
        );
        if status == Status::Invalid {
            let actual: Vec<_> = json["violations"][0]["details"]["finding"]["faults"]
                .as_array()
                .unwrap()
                .iter()
                .map(|fault| fault["kind"].as_str().unwrap())
                .collect();
            assert_eq!(actual, kinds, "{call}");
        }
    }
    let alias = prepare(
        "aliases",
        "var int:x;var int:y=x;constraint gecode_circuit(0,[x,y]);solve satisfy;",
    )
    .unwrap();
    assert_eq!(
        alias.check(&assignment("x=1;")).unwrap().status,
        Status::Invalid
    );
    for call in [
        "gecode_circuit_cost([0,9223372036854775807,0,0,0,1,-1,0,0],[1,2,0],9223372036854775807)",
        "gecode_circuit_cost_array([0,0,0,0,0,0,0,0,0],[1,2,0],[9223372036854775807,1,-1],9223372036854775807)",
    ] {
        let model = prepare("overflow", &format!("constraint {call};solve satisfy;")).unwrap();
        let error = model.check(&Assignment::default()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution);
        assert!(error.diagnostics[0].location.is_some());
    }
}

#[test]
fn circuit_cost_definitions_use_marked_slots_and_known_edges() {
    for target in ["costs", "[y2,y1]", "[z,y2,y1]"] {
        let define_total = if target.contains('z') {
            ""
        } else {
            "constraint int_plus(y1,y2,z)::defines_var(z);"
        };
        let source = format!(
            "var int:a;var int:b;var 0..5:y1::is_defined_var;var -4..0:y2::is_defined_var;var 0..5:z::is_defined_var;array[1..2] of var int:costs=[y1,y2];constraint gecode_circuit_cost_array([1,7,-4,2],[a,b],costs,z)::defines_var({target});{define_total}constraint int_le(y1,6);solve satisfy;"
        );
        let model = prepare("completion", &source).unwrap();
        let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
        for candidate in ["a=1;b=0;", "a=1;b=0;y1=8;", "a=0;", "", "a=1;b=0;"] {
            let report = model.check(&assignment(candidate)).unwrap();
            assert_eq!(
                serde_json::to_value(&report).unwrap(),
                serde_json::to_value(loaded.check(&assignment(candidate)).unwrap()).unwrap()
            );
            if candidate == "a=1;b=0;" {
                assert!(report.completeness.complete);
                assert_eq!(report.violations.len(), 2); // Derived domain and downstream bound both fail.
                let json = serde_json::to_value(&report).unwrap();
                assert!(json.to_string().contains("\"7\""));
            }
            if candidate == "a=0;" {
                assert_eq!(report.status, Status::Invalid);
                assert!(!report.completeness.complete);
            }
            if candidate.is_empty() {
                assert_eq!(report.status, Status::Incomplete);
            }
        }
    }
    for (matrix, status) in [
        ("[0,7,7,0]", Status::Valid),
        ("[0,7,8,0]", Status::Incomplete),
    ] {
        let source = format!(
            "var int:y::is_defined_var;constraint gecode_circuit_cost_array({matrix},[1,0],[y,y],14)::defines_var([y,y]);solve satisfy;"
        );
        let model = prepare("repeated", &source).unwrap();
        assert_eq!(model.check(&Assignment::default()).unwrap().status, status);
    }
    for source in [
        "var 0..0:x::is_defined_var;constraint gecode_circuit_cost([0],[x],x)::defines_var(x);solve satisfy;",
        "var 0..0:x::is_defined_var;constraint gecode_circuit_cost_array([0],[0],[x],x)::defines_var([x,x]);solve satisfy;",
    ] {
        let model = prepare("self", source).unwrap();
        let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
        assert_eq!(
            loaded.check(&Assignment::default()).unwrap().status,
            Status::Incomplete
        );
        assert_eq!(
            loaded.check(&assignment("x=0;")).unwrap().status,
            Status::Valid
        );
    }
    let scalar=prepare("scalar","var int:z::is_defined_var;constraint gecode_circuit_cost([0,7,-4,0],[1,0],z)::defines_var(z);solve satisfy;").unwrap();
    assert_eq!(
        scalar.check(&Assignment::default()).unwrap().status,
        Status::Valid
    );
    let unmarked=prepare("unmarked","var int:y;var int:z::is_defined_var;constraint gecode_circuit_cost_array([7],[0],[y],z)::defines_var(z);solve satisfy;").unwrap();
    assert_eq!(
        unmarked.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
    assert_eq!(
        unmarked.check(&assignment("y=8;")).unwrap().status,
        Status::Invalid
    );
    for target in ["[y1]", "[y1,z]", "[y1,y1]"] {
        assert!(prepare("bad-target",&format!("var int:y1::is_defined_var;var int:y2::is_defined_var;var int:z::is_defined_var;constraint gecode_circuit_cost_array([0,7,-4,0],[1,0],[y1,y2],z)::defines_var({target});solve satisfy;")).is_err());
    }
    assert!(prepare("competing","var int:z::is_defined_var;constraint gecode_circuit_cost([7],[0],z)::defines_var(z);constraint int_eq(z,7)::defines_var(z);solve satisfy;").is_err());
}

#[test]
fn circuit_partial_findings_and_round_trips_keep_metadata_and_completeness() {
    let partial=prepare("partial","var int:a;var int:b;var int:c;var int:d;var int:z;constraint gecode_circuit_cost_array([0,7,-4,0],[a,b],[c,d],z);solve satisfy;").unwrap();
    for (candidate, status, blocked) in [
        ("c=7;d=-4;z=3;", Status::Incomplete, true),
        ("c=7;d=-4;z=4;", Status::Invalid, false),
        ("a=0;", Status::Invalid, false),
        ("a=1;c=8;", Status::Invalid, false),
        ("a=-9223372036854775808;", Status::Invalid, false),
    ] {
        let report = partial.check(&assignment(candidate)).unwrap();
        assert_eq!(report.status, status, "{candidate}");
        assert!(!report.completeness.complete);
        assert_eq!(
            report
                .completeness
                .blocked_checks
                .iter()
                .any(|check| matches!(check.check, CheckRef::Constraint { .. })),
            blocked,
            "{candidate}"
        );
    }
    let model = prepare(
        "circuits.fzn",
        include_str!("../../../examples/checker-compilation/circuits.fzn"),
    )
    .unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for (candidate, status, complete) in [
        (
            "a=1;b=0;p=9007199254740994;q=9007199254740993;",
            Status::Valid,
            true,
        ),
        (
            "a=0;b=1;p=9007199254740993;q=9007199254740994;costs=[8,9];total=17;scalar_total=4;",
            Status::Invalid,
            true,
        ),
        ("a=0;", Status::Invalid, false),
        ("", Status::Incomplete, false),
        (
            "a=1;b=0;p=9007199254740994;q=9007199254740993;",
            Status::Valid,
            true,
        ),
    ] {
        let input = assignment(candidate);
        let report = model.check(&input).unwrap();
        assert_eq!(report.status, status);
        assert_eq!(report.completeness.complete, complete);
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json,
            serde_json::to_value(loaded.check(&input).unwrap()).unwrap()
        );
        if candidate.starts_with("a=0;b=1") {
            let violation = &json["violations"][0];
            assert_eq!(
                violation["details"]["finding"]["successors"][0]["value"]["variable"]["name"],
                "p"
            );
            assert_eq!(
                violation["details"]["finding"]["offset"],
                "9007199254740993"
            );
            assert_eq!(
                violation["details"]["finding"]["faults"][0]["positions"][0],
                1
            );
            assert_eq!(
                violation["check"]["constraint"]["metadata"]["application"]["id"],
                "wide_cycle"
            );
            assert_eq!(
                violation["check"]["constraint"]["metadata"]["application"]["entities"][0],
                "9007199254740993"
            );
            assert_eq!(
                violation["check"]["constraint"]["metadata"]["label"],
                "Wide circuit"
            );
        }
    }
}

#[test]
fn legacy_scheduling_checks_zero_optional_signed_and_idle_resources() {
    for (call, valid) in [
        ("gecode_schedule_unary([0,1],[3,0])", false),
        ("gecode_schedule_unary([0,3],[3,0])", true),
        (
            "gecode_schedule_unary_optional([0,1],[3,0],[true,false])",
            true,
        ),
        (
            "gecode_schedule_unary_optional([0,1],[3,0],[true,true])",
            false,
        ),
        ("gecode_schedule_cumulative_optional([],[],[],[],-1)", false),
        (
            "gecode_schedule_cumulative_optional([0],[1],[9],[false],-1)",
            false,
        ),
        (
            "gecode_schedule_cumulative_optional([0,1],[3,0],[2,1],[true,true],2)",
            false,
        ),
        (
            "gecode_schedule_cumulative_optional([0,0,3],[3,0,0],[2,2,2],[true,true,true],2)",
            true,
        ),
        (
            "gecode_schedule_cumulative_optional([1,1],[0,0],[2,2],[true,true],2)",
            true,
        ),
        ("cumulatives([],[],[],-1)", true),
        ("cumulatives([0],[0],[9],-1)", true),
        ("cumulatives([0],[-1],[9],-1)", true),
        ("cumulatives([0],[1],[-2],-1)", true),
        ("cumulatives([0],[1],[0],-1)", false),
        ("cumulatives([0,0],[0,0],[1,1],-1)", true),
        ("cumulatives([0,2],[1,1],[-1,-1],-1)", false),
        ("cumulatives([0,0],[2,2],[5,-3],2)", true),
        ("cumulatives([0,0],[2,2],[5,-2],2)", false),
        ("cumulatives([0,2],[1,1],[-1,-1],[0,0],[-1],true)", true),
        ("cumulatives([0,0],[1,1],[5,-3],[0,0],[2],false)", true),
        ("cumulatives([0,0],[1,1],[5,-4],[0,0],[2],false)", false),
        ("cumulatives([0],[0],[99],[0],[-1],true)", true),
        ("cumulatives([0],[0],[99],[1],[2],true)", false),
        ("cumulatives([0],[-1],[1],[0],[2],true)", false),
        ("cumulatives([0,0],[1,1],[2,2],[0,1],[2,2],true)", true),
        (
            "cumulatives([-9223372036854775808,9223372036854775806],[1,1],[1,1],1)",
            true,
        ),
    ] {
        let source = format!("constraint {call};solve satisfy;");
        let model = prepare("legacy.fzn", &source).unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status == Status::Valid, valid, "{call}");
        assert!(report.completeness.complete, "{call}");
    }
    for domain in ["0..0", "0..1"] {
        let source = format!(
            "var {domain}:a;var {domain}:b;constraint cumulatives([a,b],[0,0],[1,1],-1);solve satisfy;"
        );
        let model = prepare("concrete.fzn", &source).unwrap();
        assert_eq!(
            model.check(&assignment("a=0;b=0;")).unwrap().status,
            Status::Valid
        );
    }
    let singleton = prepare("singleton.fzn", &format!("{STRICT_DECLARATION}var 1..1:d;constraint fzn_disjunctive_strict([1],[d]);solve satisfy;")).unwrap();
    assert_eq!(
        singleton.check(&Assignment::default()).unwrap().status,
        Status::Valid
    );
}

#[test]
fn legacy_scheduling_admits_exact_names_roles_and_checked_arithmetic() {
    for prefix in ["", "gecode_", "fzn_"] {
        for (base, parameters, arguments) in [
            (
                "gecode_schedule_unary",
                "array[int] of var int:a,array[int] of int:b",
                "[0],[1]",
            ),
            (
                "gecode_schedule_unary_optional",
                "array[int] of var int:a,array[int] of int:b,array[int] of var bool:c",
                "[0],[1],[true]",
            ),
            (
                "gecode_schedule_cumulative_optional",
                "array[int] of var int:a,array[int] of int:b,array[int] of int:c,array[int] of var bool:d,var int:e",
                "[0],[1],[1],[true],1",
            ),
            (
                "cumulatives",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,var int:d",
                "[0],[1],[1],1",
            ),
            (
                "cumulatives",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of var int:d,array[int] of int:e,bool:f",
                "[0],[1],[1],[0],[1],true",
            ),
        ] {
            let name = format!("{prefix}{base}");
            let model = prepare(
                "names.fzn",
                &format!(
                    "predicate {name}({parameters});constraint {name}({arguments});solve satisfy;"
                ),
            )
            .unwrap();
            assert_eq!(
                model.check(&Assignment::default()).unwrap().status,
                Status::Valid,
                "{name}"
            );
        }
    }
    for source in [
        "constraint schedule_unary([0],[1]);solve satisfy;",
        "constraint gecode_schedule_unary([0],[-1]);solve satisfy;",
        "constraint gecode_schedule_unary_optional([0],[-1],[false]);solve satisfy;",
        "var 1..1:d;constraint gecode_schedule_unary([0],[d]);solve satisfy;",
        "constraint gecode_schedule_cumulative_optional([0],[1],[-1],[false],1);solve satisfy;",
        "constraint gecode_schedule_unary_optional([0],[1],[]);solve satisfy;",
        "var bool:b;constraint cumulatives([],[],[],[],[],b);solve satisfy;",
        "var 1..1:b;constraint cumulatives([0],[1],[1],[0],[b],true);solve satisfy;",
        "predicate gecode_schedule_unary(array[int] of var int:a,array[int] of var int:b);solve satisfy;",
    ] {
        assert!(prepare("bad-schedule.fzn", source).is_err(), "{source}");
    }
    for call in [
        "gecode_schedule_unary([9223372036854775807],[1])",
        "cumulatives([9223372036854775807,0],[1,1],[1,1],1)",
        "cumulatives([0,0],[1,1],[9223372036854775807,1],9223372036854775807)",
        "cumulatives([0,0],[1,1],[-9223372036854775808,-1],[0,0],[0],false)",
    ] {
        let model = prepare("overflow.fzn", &format!("constraint {call};solve satisfy;")).unwrap();
        let error = model.check(&Assignment::default()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution, "{call}");
        assert!(error.diagnostics[0].location.is_some());
    }
}

#[test]
fn legacy_scheduling_round_trips_findings_completion_and_removed_assignments() {
    let model = prepare(
        "legacy-scheduling.fzn",
        include_str!("../../../examples/checker-compilation/legacy-scheduling.fzn"),
    )
    .unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for (candidate, status, complete) in [
        (
            "s=9007199254740995;length=2;present=true;",
            Status::Valid,
            true,
        ),
        (
            "s=9007199254740994;length=2;present=true;",
            Status::Invalid,
            true,
        ),
        ("length=4;", Status::Invalid, false),
        ("", Status::Incomplete, false),
        (
            "s=9007199254740995;length=2;present=false;",
            Status::Valid,
            true,
        ),
    ] {
        let report = model.check(&assignment(candidate)).unwrap();
        assert_eq!(report.status, status, "{candidate}");
        assert_eq!(report.completeness.complete, complete, "{candidate}");
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json,
            serde_json::to_value(loaded.check(&assignment(candidate)).unwrap()).unwrap()
        );
        if candidate.starts_with("s=9007199254740994") {
            assert_eq!(report.violations.len(), 3);
            assert_eq!(json["violations"][0]["details"]["kind"], "scheduling");
            assert_eq!(
                json["violations"][0]["details"]["finding"]["tasks"][0]["start"]["value"]["value"],
                "9007199254740993"
            );
            assert_eq!(
                json["violations"][0]["check"]["constraint"]["metadata"]["application"]["id"],
                "strict"
            );
            assert_eq!(
                json["violations"][0]["check"]["constraint"]["metadata"]["label"],
                "Strict schedule"
            );
            assert_eq!(
                json["violations"][2]["details"]["finding"]["faults"][0]["usage"],
                "3"
            );
        }
    }
    let absent = prepare("absent.fzn", "var int:s;constraint gecode_schedule_unary_optional([s],[1],[false]);constraint gecode_schedule_cumulative_optional([s],[1],[1],[false],0);solve satisfy;").unwrap();
    let report = absent.check(&Assignment::default()).unwrap();
    assert!(
        report
            .completeness
            .blocked_checks
            .iter()
            .all(|blocked| !matches!(blocked.check, CheckRef::Constraint { .. }))
    );
    assert!(!report.completeness.complete);
}

#[test]
fn scheduling_known_preconditions_report_without_unrelated_values() {
    let model = prepare("partial-schedule.fzn", "var int:s;var int:d;var bool:p;var int:m;constraint gecode_schedule_cumulative_optional([s],[1],[1],[p],-1);constraint cumulatives([s],[d],[1],[m],[2],true);solve satisfy;").unwrap();
    let report = model.check(&assignment("d=-1;m=2;")).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    assert_eq!(report.violations.len(), 2);
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(
        json["violations"][0]["details"]["finding"]["faults"][0]["kind"],
        "capacity"
    );
    assert_eq!(
        json["violations"][1]["details"]["finding"]["faults"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let model = prepare("no-guesses.fzn", "var int:s::is_defined_var;constraint gecode_schedule_unary([s],[1])::defines_var(s);solve satisfy;").unwrap();
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
}

#[test]
fn explicit_scheduling_checks_endpoints_signs_and_active_machine_loads() {
    for (call, valid) in [
        ("gecode_unary_flex([],[],[])", true),
        ("gecode_unary_flex([0,1],[3,0],[3,1])", false),
        ("gecode_unary_flex([0,3],[3,0],[3,3])", true),
        ("gecode_unary_flex([0,0],[0,0],[0,0])", true),
        // Supplied endpoints, independently of duration and posting-time specialization.
        ("gecode_unary_flex([0,2],[1,1],[4,4])", false),
        ("gecode_unary_flex([0,2],[4,4],[1,3])", true),
        ("gecode_unary_flex([3,2],[1,1],[1,4])", true),
        ("gecode_unary_flex([3,0],[1,1],[1,4])", false),
        (
            "gecode_unary_flex_optional([0,1],[3,0],[3,1],[true,false])",
            true,
        ),
        ("gecode_unary_flex_optional([0],[-1],[9],[false])", false),
        ("gecode_cumulative_flex([],[],[],[],-1)", false),
        ("gecode_cumulative_flex([0,1],[3,0],[3,1],[2,1],2)", false),
        (
            "gecode_cumulative_flex([0,0,3],[3,0,0],[3,0,3],[2,2,2],2)",
            true,
        ),
        ("gecode_cumulative_flex([1,1],[0,0],[1,1],[2,2],2)", true),
        ("gecode_cumulative_flex([0,2],[1,1],[4,4],[1,1],1)", false),
        ("gecode_cumulative_flex([0,2],[4,4],[1,3],[1,1],1)", true),
        ("gecode_cumulative_flex([3],[1],[1],[2],1)", false),
        (
            "gecode_cumulative_flex_optional([0,1],[3,0],[3,1],[2,9],2,[true,false])",
            true,
        ),
        (
            "gecode_cumulative_flex_optional([0],[-1],[0],[1],1,[false])",
            false,
        ),
        (
            "gecode_cumulative_flex_optional([0],[1],[1],[1],-1,[false])",
            false,
        ),
        (
            "gecode_cumulatives_flex([0,0],[1,1],[2,2],[5,-3],[0,0],[2],true)",
            true,
        ),
        (
            "gecode_cumulatives_flex([0,0],[1,1],[2,2],[5,-2],[0,0],[2],true)",
            false,
        ),
        (
            "gecode_cumulatives_flex([0,0],[1,1],[2,2],[5,-3],[0,0],[2],false)",
            true,
        ),
        (
            "gecode_cumulatives_flex([0,0],[1,1],[2,2],[5,-4],[0,0],[2],false)",
            false,
        ),
        (
            "gecode_cumulatives_flex([0,2],[1,1],[1,3],[-1,-1],[0,0],[-1],true)",
            true,
        ),
        (
            "gecode_cumulatives_flex([1],[0],[1],[99],[0],[-1],true)",
            true,
        ),
        (
            "gecode_cumulatives_flex([3],[-2],[1],[99],[0],[-1],true)",
            true,
        ),
        (
            "gecode_cumulatives_flex([0],[-1],[1],[1],[0],[1],true)",
            true,
        ),
        (
            "gecode_cumulatives_flex([0],[1],[1],[99],[-1],[1],true)",
            true,
        ),
        (
            "gecode_cumulatives_flex([0],[1],[1],[99],[1],[1],true)",
            true,
        ),
        // No start+duration addition belongs to any explicit-end resource relation.
        (
            "gecode_unary_flex([9223372036854775807],[1],[9223372036854775807])",
            true,
        ),
        (
            "gecode_cumulative_flex([9223372036854775807],[1],[9223372036854775807],[1],1)",
            true,
        ),
    ] {
        let model = prepare("flex.fzn", &format!("constraint {call};solve satisfy;")).unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status == Status::Valid, valid, "{call}");
        assert!(report.completeness.complete, "{call}");
    }
}

#[test]
fn explicit_scheduling_admits_exact_names_and_signature_roles() {
    for prefix in ["", "gecode_", "fzn_"] {
        for (base, parameters, arguments) in [
            (
                "gecode_unary_flex",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c",
                "[0],[1],[1]",
            ),
            (
                "gecode_unary_flex_optional",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of var bool:d",
                "[0],[1],[1],[true]",
            ),
            (
                "gecode_cumulative_flex",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of int:d,var int:e",
                "[0],[1],[1],[1],1",
            ),
            (
                "gecode_cumulative_flex_optional",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of int:d,var int:e,array[int] of var bool:f",
                "[0],[1],[1],[1],1,[true]",
            ),
            (
                "gecode_cumulatives_flex",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of var int:d,array[int] of var int:e,array[int] of int:f,bool:g",
                "[0],[1],[1],[1],[0],[1],true",
            ),
            (
                "cumulatives",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of var int:d,array[int] of var int:e,array[int] of int:f,bool:g",
                "[0],[1],[1],[1],[0],[1],true",
            ),
        ] {
            let name = format!("{prefix}{base}");
            let model = prepare(
                "names.fzn",
                &format!(
                    "predicate {name}({parameters});constraint {name}({arguments});solve satisfy;"
                ),
            )
            .unwrap();
            assert_eq!(
                model.check(&Assignment::default()).unwrap().status,
                Status::Valid,
                "{name}"
            );
        }
    }
    for source in [
        "constraint unary_flex([],[],[]);solve satisfy;",
        "constraint gecode_unary_flex([0],[1],[]);solve satisfy;",
        "constraint gecode_unary_flex([0],[true],[1]);solve satisfy;",
        "constraint gecode_unary_flex_optional([0],[1],[1],[1]);solve satisfy;",
        "constraint gecode_cumulative_flex([0],[1],[1],[-1],1);solve satisfy;",
        "var int:u;constraint gecode_cumulative_flex([0],[1],[1],[u],1);solve satisfy;",
        "constraint gecode_cumulative_flex_optional([0],[1],[1],[1],[true],1);solve satisfy;",
        "var int:b;constraint gecode_cumulatives_flex([0],[1],[1],[1],[0],[b],true);solve satisfy;",
        "var bool:b;constraint gecode_cumulatives_flex([],[],[],[],[],[],b);solve satisfy;",
        "predicate fzn_cumulatives(array[int] of var int:s,array[int] of var int:d,array[int] of var int:r,array[int] of var int:m,array[int] of var int:b,bool:upper,int:min_machine);solve satisfy;",
    ] {
        assert!(prepare("bad-flex.fzn", source).is_err(), "{source}");
    }
    let declarations = "predicate cumulatives(array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,var int:d);predicate cumulatives(array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of var int:d,array[int] of int:e,bool:f);predicate cumulatives(array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of var int:d,array[int] of var int:e,array[int] of int:f,bool:g);";
    prepare("overloads.fzn", &format!("{declarations}constraint cumulatives([],[],[],0);constraint cumulatives([],[],[],[],[],true);constraint cumulatives([],[],[],[],[],[],true);solve satisfy;")).unwrap();
    let overflow = prepare("overflow.fzn", "constraint gecode_cumulatives_flex([0,0],[0,0],[1,1],[9223372036854775807,1],[0,0],[0],true);solve satisfy;").unwrap();
    assert_eq!(
        overflow.check(&Assignment::default()).unwrap_err().kind,
        ErrorKind::Execution
    );
}

#[test]
fn explicit_scheduling_round_trips_completion_and_partial_findings() {
    let model = prepare(
        "explicit-end-scheduling.fzn",
        include_str!("../../../examples/checker-compilation/explicit-end-scheduling.fzn"),
    )
    .unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for (candidate, status, complete) in [
        (
            "s=9007199254740995;length=2;present=true;",
            Status::Valid,
            true,
        ),
        (
            "s=9007199254740994;length=2;present=true;",
            Status::Invalid,
            true,
        ),
        (
            "s=9007199254740995;length=2;present=false;",
            Status::Valid,
            true,
        ),
        (
            "s=9007199254740994;length=2;present=true;finish=9007199254740993;",
            Status::Invalid,
            true,
        ),
        ("length=-1;", Status::Invalid, false),
        ("", Status::Incomplete, false),
    ] {
        let input = assignment(candidate);
        let report = model.check(&input).unwrap();
        assert_eq!(report.status, status, "{candidate}");
        assert_eq!(report.completeness.complete, complete, "{candidate}");
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json,
            serde_json::to_value(loaded.check(&input).unwrap()).unwrap()
        );
        if candidate == "s=9007199254740994;length=2;present=true;" {
            assert_eq!(report.violations.len(), 4);
            let unary = &json["violations"][0];
            assert_eq!(
                unary["details"]["finding"]["tasks"][0]["end"]["value"]["value"],
                "9007199254740995"
            );
            assert_eq!(
                unary["check"]["constraint"]["metadata"]["application"]["id"],
                "unary_flex"
            );
            assert_eq!(
                json["violations"][2]["details"]["finding"]["faults"][0]["usage"],
                "3"
            );
        }
        if candidate.ends_with("finish=9007199254740993;") {
            // The end equation fails; scheduling still uses the supplied empty interval.
            assert_eq!(report.violations.len(), 1);
        }
        if candidate == "length=-1;" {
            assert_eq!(report.violations.len(), 5);
        }
    }
    let absent = prepare("absent-flex.fzn", "var int:s;var int:e;var int:d;constraint gecode_unary_flex_optional([s],[d],[e],[false]);constraint gecode_cumulative_flex_optional([s],[d],[e],[1],1,[false]);solve satisfy;").unwrap();
    let report = absent.check(&assignment("d=1;")).unwrap();
    assert_eq!(report.status, Status::Incomplete);
    assert!(
        report
            .completeness
            .blocked_checks
            .iter()
            .all(|check| !matches!(check.check, CheckRef::Constraint { .. }))
    );
    let report = absent.check(&assignment("d=-1;")).unwrap();
    assert_eq!(report.violations.len(), 2);
    let report = absent.check(&Assignment::default()).unwrap();
    assert!(
        report
            .completeness
            .blocked_checks
            .iter()
            .any(|check| matches!(check.check, CheckRef::Constraint { .. }))
    );
    let caller_duration = prepare(
        "machine-duration.fzn",
        "var int:d;constraint gecode_cumulatives_flex([0],[d],[1],[1],[0],[1],true);solve satisfy;",
    )
    .unwrap();
    let report = caller_duration.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Incomplete);
    assert!(
        report
            .completeness
            .blocked_checks
            .iter()
            .all(|check| !matches!(check.check, CheckRef::Constraint { .. }))
    );
    let empty_machine_task = prepare("empty-machine-task.fzn", "var int:d;var int:u;constraint gecode_cumulatives_flex([0],[d],[0],[u],[0],[0],true);solve satisfy;").unwrap();
    let report = empty_machine_task.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Incomplete);
    assert!(
        report
            .completeness
            .blocked_checks
            .iter()
            .all(|check| !matches!(check.check, CheckRef::Constraint { .. }))
    );
    let no_guesses = prepare("no-flex-guesses.fzn", "var int:e::is_defined_var;constraint gecode_unary_flex([0],[1],[e])::defines_var(e);solve satisfy;").unwrap();
    assert_eq!(
        no_guesses.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
}

#[test]
fn packing_aliases_offsets_shapes_and_completion() {
    for prefix in ["", "gecode_", "fzn_"] {
        for minimum in [-2, 0, 3] {
            let name = format!("{prefix}gecode_bin_packing_load");
            let source = format!(
                "predicate {name}(array[int] of var int: a,array[int] of var int: b,array[int] of int: c,int: d);var int:load_a::is_defined_var;var int:load_b::is_defined_var;array[1..2] of var int: loads=[load_a,load_b];var int:b;constraint {name}(loads,[b],[3],{minimum})::defines_var(loads);solve minimize load_a;"
            );
            let model = prepare("packing.fzn", &source).unwrap();
            let report = model.check(&assignment(&format!("b={minimum};"))).unwrap();
            assert_eq!(report.status, Status::Valid);
            assert_eq!(report.objective.unwrap().argument.value, Value::Integer(3));
            let report = model
                .check(&assignment(&format!("b={};loads=[9,8];", minimum + 1)))
                .unwrap();
            let json = serde_json::to_value(report).unwrap();
            assert_eq!(
                json["violations"][0]["details"]["finding"]["faults"],
                serde_json::json!([{ "kind":"load", "position":1 },{ "kind":"load", "position":2 }])
            );
            assert_eq!(
                model.check(&Assignment::default()).unwrap().status,
                Status::Incomplete
            );
        }
    }
    for source in [
        "constraint gecode_bin_packing_load([0],[1],[1,2],0);solve satisfy;",
        "constraint gecode_bin_packing_load([0],[0],[-1],0);solve satisfy;",
        "var int:w;constraint gecode_bin_packing_load([0],[0],[w],0);solve satisfy;",
        "var int:m;constraint gecode_bin_packing_load([0],[0],[1],m);solve satisfy;",
        "constraint gecode_bin_packing_load([0,0],[],[],9223372036854775807);solve satisfy;",
        "predicate gecode_bin_packing_load(array[int] of var int:a,array[int] of int:b,array[int] of int:c,int:d);solve satisfy;",
        "var int:a::is_defined_var;var int:b::is_defined_var;array[1..2] of var int:l=[a,b];constraint gecode_bin_packing_load([a],[0],[1],0)::defines_var(l);solve satisfy;",
    ] {
        assert!(prepare("bad.fzn", source).is_err(), "{source}");
    }
    for (source, status) in [
        (
            "constraint gecode_bin_packing_load([0,0],[],[],-2);solve satisfy;",
            Status::Valid,
        ),
        (
            "constraint gecode_bin_packing_load([],[],[],9223372036854775807);solve satisfy;",
            Status::Valid,
        ),
        (
            "constraint gecode_bin_packing_load([],[0,1],[0,0],0);solve satisfy;",
            Status::Invalid,
        ),
        (
            "var int:l::is_defined_var;constraint gecode_bin_packing_load([l,l],[0,1],[3,3],0)::defines_var(l);solve minimize l;",
            Status::Valid,
        ),
        (
            "var int:l::is_defined_var;constraint gecode_bin_packing_load([l,l],[0],[3],0)::defines_var(l);solve satisfy;",
            Status::Incomplete,
        ),
        (
            "var int:l::is_defined_var;constraint gecode_bin_packing_load([l],[l],[0],0)::defines_var(l);solve satisfy;",
            Status::Incomplete,
        ),
        (
            "var 0..1:l::is_defined_var;constraint gecode_bin_packing_load([l],[0],[3],0)::defines_var(l);constraint int_le(l,2);solve satisfy;",
            Status::Invalid,
        ),
    ] {
        assert_eq!(
            prepare("loads.fzn", source)
                .unwrap()
                .check(&Assignment::default())
                .unwrap()
                .status,
            status,
            "{source}"
        );
    }
    let invalid = prepare("bad-bin.fzn", "var int:missing;constraint gecode_bin_packing_load([0,0],[0,2,missing],[3,0,1],0);solve satisfy;").unwrap();
    let report = invalid.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    let report = invalid.check(&assignment("missing=0;")).unwrap();
    let json = serde_json::to_value(report).unwrap();
    assert_eq!(
        json["violations"][0]["details"]["finding"]["faults"],
        serde_json::json!([{ "kind":"bin", "position":2,"value":"2" },{ "kind":"load", "position":1 }])
    );
    assert_eq!(
        json["violations"][0]["details"]["finding"]["loads"][0]["items"],
        serde_json::json!([1, 3])
    );
    let downstream = prepare("downstream.fzn", "var 0..1:l::is_defined_var;constraint gecode_bin_packing_load([l],[0],[3],0)::defines_var(l);constraint int_le(l,2);solve satisfy;").unwrap().check(&Assignment::default()).unwrap();
    assert_eq!(downstream.violations.len(), 2);
    assert!(downstream.completeness.complete);
    let overflow = prepare(
        "overflow.fzn",
        "constraint gecode_bin_packing_load([0],[0,0],[9223372036854775807,1],0);solve satisfy;",
    )
    .unwrap()
    .check(&Assignment::default())
    .unwrap_err();
    assert_eq!(overflow.kind, ErrorKind::Execution);
    assert!(overflow.diagnostics[0].location.is_some());
}

#[test]
fn rectangle_aliases_strict_zero_sizes_presence_and_explicit_ends() {
    for prefix in ["", "gecode_", "fzn_"] {
        for (base, signature, args) in [
            (
                "gecode_nooverlap",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of var int:d",
                "[0,1],[2,0],[0,1],[2,0]",
            ),
            (
                "gecode_nooverlap_fzn",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of var int:d",
                "[0,1],[2,0],[0,1],[2,0]",
            ),
            (
                "gecode_nooverlap_optional",
                "array[int] of var int:a,array[int] of int:b,array[int] of var int:c,array[int] of int:d,array[int] of var bool:e",
                "[0,1],[2,0],[0,1],[2,0],[true,true]",
            ),
            (
                "gecode_nooverlap_flex_optional",
                "array[int] of var int:a,array[int] of var int:b,array[int] of var int:c,array[int] of var int:d,array[int] of var int:e,array[int] of var int:f,array[int] of var bool:g",
                "[0,1],[2,0],[2,1],[0,1],[2,0],[2,1],[true,true]",
            ),
        ] {
            let name = format!("{prefix}{base}");
            let source =
                format!("predicate {name}({signature});constraint {name}({args});solve satisfy;");
            let report = prepare("rectangles.fzn", &source)
                .unwrap()
                .check(&Assignment::default())
                .unwrap();
            assert_eq!(report.status, Status::Invalid, "{name}");
            let json = serde_json::to_value(report).unwrap();
            assert_eq!(
                json["violations"][0]["details"]["finding"]["faults"],
                serde_json::json!([{ "kind":"conflict", "first":1,"second":2 }])
            );
        }
    }
    for (args, status) in [
        ("[0,2],[2,0],[0,1],[2,0]", Status::Valid),
        ("[0,1],[2,0],[0,1],[2,1]", Status::Invalid),
        ("[0,0],[0,0],[0,0],[0,0]", Status::Valid),
        ("[0,1,1],[2,1,1],[0,1,1],[2,1,1]", Status::Invalid),
        ("[],[],[],[]", Status::Valid),
    ] {
        let source = format!("constraint gecode_nooverlap_fzn({args});solve satisfy;");
        let report = prepare("r.fzn", &source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap();
        assert_eq!(report.status, status);
        if args.starts_with("[0,1,1]") {
            let json = serde_json::to_value(report).unwrap();
            assert_eq!(
                json["violations"][0]["details"]["finding"]["faults"]
                    .as_array()
                    .unwrap()
                    .len(),
                3
            );
        }
    }
    let absent = prepare("absent.fzn","var int:x;var int:y;var int:r;var int:t;var int:w;constraint gecode_nooverlap_flex_optional([x],[w],[r],[y],[0],[t],[false]);solve satisfy;").unwrap();
    let report = absent.check(&assignment("w=0;")).unwrap();
    assert!(
        report
            .completeness
            .blocked_checks
            .iter()
            .all(|check| !matches!(check.check, CheckRef::Constraint { .. }))
    );
    let json = serde_json::to_value(absent.check(&assignment("w=-1;")).unwrap()).unwrap();
    assert_eq!(
        json["violations"][0]["details"]["finding"]["faults"][0],
        serde_json::json!({"kind":"dimension","position":1,"dimension":"width","value":"-1"})
    );
    for source in [
        "constraint gecode_nooverlap_fzn([0],[],[0],[1]);solve satisfy;",
        "constraint gecode_nooverlap_optional([0],[-1],[0],[1],[false]);solve satisfy;",
        "var int:w;constraint gecode_nooverlap_optional([0],[w],[0],[1],[true]);solve satisfy;",
        "predicate gecode_nooverlap(array[int] of var opt int:x,array[int] of var int:w,array[int] of var opt int:y,array[int] of var int:h);solve satisfy;",
    ] {
        assert!(prepare("bad.fzn", source).is_err(), "{source}");
    }
    let explicit=prepare("explicit.fzn","constraint gecode_nooverlap_flex_optional([0,1],[2,0],[-1,1],[0,1],[2,0],[2,1],[true,true]);constraint int_plus(0,2,-1);solve satisfy;").unwrap();
    let report = explicit.check(&Assignment::default()).unwrap();
    assert_eq!(report.violations.len(), 1);
    assert!(matches!(
        report.violations[0].details,
        ViolationKind::IntegerArithmetic { .. }
    ));
    let overflow = prepare(
        "overflow.fzn",
        "constraint gecode_nooverlap_fzn([9223372036854775807],[1],[0],[0]);solve satisfy;",
    )
    .unwrap()
    .check(&Assignment::default())
    .unwrap_err();
    assert_eq!(overflow.kind, ErrorKind::Execution);
    assert!(overflow.diagnostics[0].location.is_some());
}

#[test]
fn packing_and_rectangle_reports_round_trip_and_recompute_edits() {
    let model = prepare(
        "packing.fzn",
        include_str!("../../../examples/checker-compilation/packing.fzn"),
    )
    .unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for (candidate, status, complete) in [
        (
            "bin=9007199254740993;other=9007199254740994;x=9007199254740995;width=2;present=true;",
            Status::Valid,
            true,
        ),
        (
            "bin=9007199254740993;other=9007199254740994;x=9007199254740994;width=2;present=true;loads=[9,8];",
            Status::Invalid,
            true,
        ),
        (
            "bin=9007199254740993;other=9007199254740994;x=9007199254740994;width=2;present=false;right=9007199254740993;",
            Status::Invalid,
            true,
        ),
        ("other=0;width=-1;", Status::Invalid, false),
        ("", Status::Incomplete, false),
    ] {
        let input = assignment(candidate);
        let report = model.check(&input).unwrap();
        assert_eq!(report.status, status, "{candidate}");
        assert_eq!(report.completeness.complete, complete);
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json,
            serde_json::to_value(loaded.check(&input).unwrap()).unwrap()
        );
        if candidate.contains("loads=") {
            let packing = &json["violations"][0];
            assert_eq!(packing["details"]["finding"]["minimum"], "9007199254740993");
            assert_eq!(
                packing["details"]["finding"]["items"][0]["bin"]["variable"]["name"],
                "bin"
            );
            assert_eq!(
                packing["check"]["constraint"]["metadata"]["application"]["id"],
                "loads"
            );
            assert_eq!(report.violations.len(), 4);
        }
    }
}

#[test]
fn set_global_aliases_signatures_and_fixed_roles() {
    for prefix in ["", "gecode_", "fzn_"] {
        for (root, signature, args) in [
            ("disjoint", "var set of int:a,var set of int:b", "{1},{2}"),
            (
                "array_set_union",
                "array[int] of var set of int:a,var set of int:b",
                "[{1},{2}],1..2",
            ),
            (
                "array_set_partition",
                "array[int] of var set of int:a,set of int:b",
                "[{1},{2}],1..2",
            ),
            ("set_convex", "var set of int:a", "1..2"),
            (
                "array_set_seq",
                "array[int] of var set of int:a",
                "[{1},{},{2}]",
            ),
            (
                "array_set_seq_union",
                "array[int] of var set of int:a,var set of int:b",
                "[{1},{2}],1..2",
            ),
            (
                "gecode_set_weights",
                "array[int] of int:a,array[int] of int:b,var set of int:c,var int:d",
                "[2,1,2],[-3,5,9],1..2,2",
            ),
            (
                "gecode_set_convex_hull",
                "var set of int:a,var set of int:b",
                "{1,3},1..3",
            ),
            (
                "gecode_set_atmost_one",
                "array[int] of var set of int:a,int:b",
                "[{1},{2}],1",
            ),
            (
                "gecode_array_set_intersect",
                "array[int] of var set of int:a,var set of int:b",
                "[1..3,2..4],2..3",
            ),
            ("gecode_set_min", "var set of int:a,var int:b", "{1,3},1"),
            (
                "gecode_set_min_reif",
                "var set of int:a,var int:b,var bool:c",
                "{},1,false",
            ),
            (
                "gecode_set_min_imp",
                "var set of int:a,var int:b,var bool:c",
                "{1,3},1,true",
            ),
            ("gecode_set_max", "var set of int:a,var int:b", "{1,3},3"),
            (
                "gecode_set_max_reif",
                "var set of int:a,var int:b,var bool:c",
                "{1,3},1,false",
            ),
            (
                "gecode_set_max_imp",
                "var set of int:a,var int:b,var bool:c",
                "{1,3},3,true",
            ),
        ] {
            let name = format!("{prefix}{root}");
            let source =
                format!("predicate {name}({signature});constraint {name}({args});solve satisfy;");
            let model = prepare("aliases.fzn", &source).unwrap();
            let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
            assert_eq!(
                loaded.check(&Assignment::default()).unwrap().status,
                Status::Valid,
                "{name}"
            );
        }
    }
    for source in [
        "predicate array_set_partition(array[int] of var set of int:x,var set of int:y);constraint array_set_partition([],{});solve satisfy;",
        "var set of int:s;constraint array_set_partition([],s);solve satisfy;",
    ] {
        assert!(prepare("partition.fzn", source).is_ok());
    }
    for source in [
        "constraint array_set_union([]);solve satisfy;",
        "constraint array_set_union([1],{});solve satisfy;",
        "constraint gecode_set_atmost_one([], -1);solve satisfy;",
        "var 1..1:c;constraint gecode_set_atmost_one([],c);solve satisfy;",
        "var 1..1:w;constraint gecode_set_weights([1],[w],{},0);solve satisfy;",
        "constraint gecode_set_weights([1],[],{},0);solve satisfy;",
        "predicate array_set_union(array[int] of set of int:a,var set of int:b);solve satisfy;",
        "predicate gecode_set_weights(array[int] of var int:a,array[int] of int:b,var set of int:c,var int:d);solve satisfy;",
        "predicate gecode_set_atmost_one(array[int] of var set of int:a,var int:b);solve satisfy;",
        "predicate set_convex(set of int:a);solve satisfy;",
        "constraint set_min({1},1);solve satisfy;",
        "constraint gecode_gecode_array_set_union([],{});solve satisfy;",
        "constraint array_set_union_reif([],{},true);solve satisfy;",
    ] {
        assert!(prepare("bad.fzn", source).is_err(), "{source}");
    }
}

#[test]
fn set_global_empty_wide_order_and_cardinality_contracts() {
    for (call, status) in [
        ("array_set_union([],{})", Status::Valid),
        ("array_set_partition([],{})", Status::Valid),
        ("array_set_seq_union([],{})", Status::Valid),
        (
            "gecode_array_set_intersect([],-1073741822..1073741822)",
            Status::Valid,
        ),
        (
            "gecode_array_set_intersect([9007199254740993..9223372036854775807,9007199254740994..9223372036854775807],9007199254740994..9223372036854775807)",
            Status::Valid,
        ),
        (
            "gecode_set_convex_hull({-9223372036854775808,9223372036854775807},-9223372036854775808..9223372036854775807)",
            Status::Valid,
        ),
        ("gecode_set_convex_hull({},{})", Status::Valid),
        ("set_convex({})", Status::Valid),
        (
            "set_convex(-9223372036854775808..9223372036854775807)",
            Status::Valid,
        ),
        ("set_convex({1,3})", Status::Invalid),
        ("array_set_seq([])", Status::Valid),
        ("array_set_seq([{3},{},{2}])", Status::Invalid),
        ("array_set_seq_union([{1},{},{2}],1..2)", Status::Valid),
        ("array_set_seq_union([{2},{},{1}],1..2)", Status::Invalid),
        ("array_set_partition([{1},{1}],{1})", Status::Invalid),
        ("gecode_set_atmost_one([],3)", Status::Valid),
        ("gecode_set_atmost_one([{}],0)", Status::Valid),
        ("gecode_set_atmost_one([{1}],0)", Status::Invalid),
        ("gecode_set_atmost_one([{}],1)", Status::Invalid),
        ("gecode_set_atmost_one([{1}],2)", Status::Invalid),
        ("gecode_set_atmost_one([1..2,3..4],2)", Status::Valid),
        ("gecode_set_atmost_one([1..2,2..3],2)", Status::Valid),
        ("gecode_set_atmost_one([1..2,1..2],2)", Status::Invalid),
        ("gecode_set_min({},1)", Status::Invalid),
        ("gecode_set_max({},1)", Status::Invalid),
        ("gecode_set_min_reif({},1,false)", Status::Valid),
        ("gecode_set_max_reif({},1,true)", Status::Invalid),
        ("gecode_set_min_imp({},1,true)", Status::Invalid),
        ("gecode_set_max_imp({},1,false)", Status::Valid),
    ] {
        let source = format!("constraint {call};solve satisfy;");
        assert_eq!(
            prepare("sets.fzn", &source)
                .unwrap()
                .check(&Assignment::default())
                .unwrap()
                .status,
            status,
            "{call}"
        );
    }
    let identity = prepare("identity.fzn", "var set of int:s::is_defined_var;var int:c::is_defined_var;constraint gecode_array_set_intersect([],s)::defines_var(s);constraint set_card(s,c)::defines_var(c);solve minimize c;").unwrap();
    assert_eq!(
        identity
            .check(&Assignment::default())
            .unwrap()
            .objective
            .unwrap()
            .argument
            .value,
        Value::Integer(zincite_check::GECODE_SET_CARDINALITY)
    );
    let partial=prepare("partial.fzn","var set of int:missing;var int:n;var bool:b::is_defined_var;constraint gecode_set_min_reif({},n,b)::defines_var(b);constraint gecode_set_max_imp(missing,n,false);constraint array_set_seq([{3},missing,{2}]);constraint array_set_partition([{1},missing,{1}],{});constraint gecode_set_atmost_one([{},missing],1);constraint gecode_set_max({},n);solve satisfy;").unwrap();
    let report = partial.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    assert_eq!(report.violations.len(), 4);
    assert!(
        !report
            .completeness
            .unresolved_variables
            .iter()
            .any(|v| v.name == "b")
    );
    let json = serde_json::to_value(&report).unwrap();
    let finding = &json["violations"][0]["details"]["finding"];
    assert_eq!(
        finding["conflicts"],
        serde_json::json!([{"first":1,"second":3}])
    );
    assert_eq!(finding["entries"][1]["unresolved"]["name"], "missing");
    assert_eq!(finding["entries"][0]["position"], 1);
    for call in [
        "gecode_set_atmost_one([-9223372036854775808..9223372036854775807],0)",
        "gecode_set_atmost_one([-9223372036854775808..9223372036854775807,-9223372036854775808..9223372036854775807],1)",
    ] {
        let error = prepare("overflow.fzn", &format!("constraint {call};solve satisfy;"))
            .unwrap()
            .check(&Assignment::default())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution);
        assert!(error.diagnostics[0].location.is_some());
    }
}

#[test]
fn weighted_sets_use_first_rows_coverage_and_checked_key_order() {
    for (args, status) in [
        ("[],[],{},0", Status::Valid),
        (
            "[3,1,2],[-5,9007199254740993,0],1..3,9007199254740988",
            Status::Valid,
        ),
        ("[2,1,2],[3,5,3],1..2,8", Status::Valid),
        ("[2,1,2],[3,5,9],1..2,8", Status::Valid),
        ("[2,1,2],[3,5,9],1..2,14", Status::Invalid),
        ("[1],[0],{2},0", Status::Invalid),
    ] {
        assert_eq!(
            prepare(
                "weights.fzn",
                &format!("constraint gecode_set_weights({args});solve satisfy;")
            )
            .unwrap()
            .check(&Assignment::default())
            .unwrap()
            .status,
            status,
            "{args}"
        );
    }
    let missing=prepare("coverage.fzn","var int:total::is_defined_var;constraint gecode_set_weights([1,3],[0,-1],-9223372036854775808..9223372036854775807,total)::defines_var(total);solve satisfy;").unwrap().check(&Assignment::default()).unwrap();
    assert_eq!(missing.status, Status::Invalid);
    assert!(!missing.completeness.complete);
    let json = serde_json::to_value(missing).unwrap();
    let finding = &json["violations"][0]["details"]["finding"];
    assert_eq!(finding["expected"], serde_json::Value::Null);
    assert_eq!(finding["result"]["unresolved"]["name"], "total");
    assert_eq!(finding["uncovered"]["ranges"].as_array().unwrap().len(), 3);
    assert_eq!(
        finding["rows"][1],
        serde_json::json!({"position":2,"element":"3","weight":"-1"})
    );
    // The final mathematical total fits, but the ascending-key intermediate does not.
    let error=prepare("overflow.fzn","constraint gecode_set_weights([3,1,2],[-1,9223372036854775807,1],1..3,9223372036854775807);solve satisfy;").unwrap().check(&Assignment::default()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Execution);
    assert!(error.diagnostics[0].location.is_some());
}

#[test]
fn set_global_completion_edits_conflicts_and_round_trips() {
    let model = prepare(
        "set-globals.fzn",
        include_str!("../../../examples/checker-compilation/set-globals.fzn"),
    )
    .unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for (candidate, status, complete) in [
        (
            "left={1};right={3};selected={9007199254740993};probe=9007199254740993;",
            Status::Valid,
            true,
        ),
        (
            "left={1};right={1};selected={9007199254740993};probe=9007199254740993;total=7;combined={3};",
            Status::Invalid,
            true,
        ),
        (
            "left={3};right={1};selected={1,3};probe=3;",
            Status::Invalid,
            true,
        ),
        ("left={1};right={1};", Status::Invalid, false),
        ("", Status::Incomplete, false),
    ] {
        let input = assignment(candidate);
        let report = model.check(&input).unwrap();
        assert_eq!(report.status, status, "{candidate}");
        assert_eq!(report.completeness.complete, complete);
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json,
            serde_json::to_value(loaded.check(&input).unwrap()).unwrap()
        );
        if candidate.contains("total=7") {
            let finding = json["violations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["details"]["finding"]["operation"] == "weights")
                .unwrap();
            assert_eq!(
                finding["details"]["finding"]["expected"],
                "9007199254740993"
            );
            assert_eq!(
                finding["check"]["constraint"]["metadata"]["application"]["entities"][0],
                "9007199254740993"
            );
            assert!(
                zincite_check::cli::render_text(&report)
                    .contains("set weighted sum: expected 9007199254740993")
            );
        }
    }
    for (source, status) in [
        (
            "var set of int:s::is_defined_var;constraint array_set_union([s],s)::defines_var(s);solve satisfy;",
            Status::Incomplete,
        ),
        (
            "var set of int:s::is_defined_var;constraint array_set_union([s],{})::defines_var(s);solve satisfy;",
            Status::Incomplete,
        ),
        (
            "var int:n::is_defined_var;constraint gecode_set_min({},n)::defines_var(n);solve satisfy;",
            Status::Invalid,
        ),
        (
            "var bool:b::is_defined_var;var int:n;constraint gecode_set_min_imp({},n,b)::defines_var(b);solve satisfy;",
            Status::Incomplete,
        ),
        (
            "var set of int:s::is_defined_var;constraint array_set_partition([{1},{1}],s)::defines_var(s);constraint set_eq(s,{});solve satisfy;",
            Status::Invalid,
        ),
        (
            "var set of int:s::is_defined_var;constraint array_set_seq_union([{2},{1}],s)::defines_var(s);constraint set_eq(s,{});solve satisfy;",
            Status::Invalid,
        ),
        (
            "var set of 1..1:s::is_defined_var;constraint array_set_union([{2}],s)::defines_var(s);constraint set_eq(s,{});solve satisfy;",
            Status::Invalid,
        ),
        (
            "var 0..0:n::is_defined_var;constraint gecode_set_weights([1],[2],{1},n)::defines_var(n);constraint int_le(n,1);solve satisfy;",
            Status::Invalid,
        ),
    ] {
        let report = prepare("definition.fzn", source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap();
        assert_eq!(report.status, status, "{source}");
        if source.contains("constraint set_eq(s,{})") || source.contains("constraint int_le(n,1)") {
            assert_eq!(report.violations.len(), 2);
            assert!(report.completeness.complete);
        }
    }
    let source = "var set of int:s::is_defined_var;constraint array_set_union([],s)::defines_var(s);constraint gecode_set_convex_hull({},s)::defines_var(s);solve satisfy;";
    assert!(prepare("markers.fzn", source).is_err());
}

#[test]
fn set_channel_aliases_validate_positional_contracts() {
    let contracts = [
        (
            "gecode_link_set_to_booleans",
            "var set of int:aa,array[int] of var bool:bb,int:cc",
            "{},[],0",
        ),
        (
            "gecode_int_set_channel",
            "array[int] of var int:aa,int:bb,array[int] of var set of int:cc,int:dd",
            "[],0,[],0",
        ),
        (
            "gecode_inverse_set",
            "array[int] of var set of int:aa,array[int] of var set of int:bb,int:cc,int:dd",
            "[],[],0,0",
        ),
        (
            "gecode_range",
            "array[int] of var int:aa,int:bb,var set of int:cc,var set of int:dd",
            "[],0,{},{}",
        ),
        (
            "gecode_precede_set",
            "array[int] of var set of int:aa,int:bb,int:cc",
            "[],1,2",
        ),
        (
            "gecode_set_channel_sorted",
            "array[int] of var int:aa,var set of int:bb",
            "[],{}",
        ),
        (
            "gecode_array_set_element_union",
            "var set of int:aa,array[int] of var set of int:bb,var set of int:cc",
            "{},[],{}",
        ),
        (
            "gecode_array_set_element_intersect",
            "var set of int:aa,array[int] of var set of int:bb,var set of int:cc",
            "{},[],-1073741822..1073741822",
        ),
        (
            "gecode_array_set_element_partition",
            "var set of int:aa,array[int] of var set of int:bb,var set of int:cc",
            "{},[],{}",
        ),
        (
            "gecode_array_set_element_intersect_in",
            "var set of int:aa,array[int] of var set of int:bb,var set of int:cc,set of int:dd",
            "{},[],{3},{3}",
        ),
    ];
    for (root, signature, args) in contracts {
        for prefix in ["", "gecode_", "fzn_"] {
            let name = format!("{prefix}{root}");
            let source =
                format!("predicate {name}({signature});constraint {name}({args});solve satisfy;");
            assert_eq!(
                prepare("aliases.fzn", &source)
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                Status::Valid,
                "{name}"
            );
            for signature in [
                signature.replacen("var", "", 1),
                format!("{signature},int:extra"),
            ] {
                assert!(
                    prepare(
                        "signature.fzn",
                        &format!("predicate {name}({signature});solve satisfy;")
                    )
                    .is_err(),
                    "{name}"
                );
            }
        }
    }
    for call in [
        "link_set_to_booleans({},[],0)",
        "gecode_gecode_gecode_range([],0,{},{})",
        "gecode_range_reif([],0,{},{},true)",
        "gecode_link_set_to_booleans({},[],-1)",
        "gecode_link_set_to_booleans({},[],1073741824)",
        "gecode_range([],9223372036854775807,{},{})",
        "gecode_range([1],2147483647,{},{})",
        "gecode_inverse_set([],[],-1,0)",
        "gecode_int_set_channel([],1073741823,[{}],0)",
        "gecode_int_set_channel([],1073741824,[],1)",
        "gecode_precede_set([],1073741823,0)",
        "gecode_precede_set([],-1073741823,0)",
        "gecode_array_set_element_intersect_in({},[],{},2147483648..2147483649)",
        "gecode_range([true],0,{},{})",
        "gecode_link_set_to_booleans({},[1],0)",
        "gecode_array_set_element_union({},[],{},0)",
    ] {
        assert!(
            prepare(
                "bad-channel.fzn",
                &format!("constraint {call};solve satisfy;")
            )
            .is_err(),
            "{call}"
        );
    }
    assert!(
        prepare(
            "fixed.fzn",
            "var int:k;constraint gecode_range([],k,{},{});solve satisfy;"
        )
        .is_err()
    );
    assert!(prepare("fixed.fzn", "var set of int:u;constraint gecode_array_set_element_intersect_in({},[],{},u);solve satisfy;").is_err());
}

#[test]
fn set_channels_follow_selection_padding_and_membership_contracts() {
    for (call, status) in [
        ("gecode_array_set_element_union({},[],{})", Status::Valid),
        (
            "gecode_array_set_element_partition({},[],{})",
            Status::Valid,
        ),
        (
            "gecode_array_set_element_intersect({},[],-1073741822..1073741822)",
            Status::Valid,
        ),
        (
            "gecode_array_set_element_intersect_in({},[],{7},{7})",
            Status::Valid,
        ),
        (
            "gecode_array_set_element_intersect_in({1},[{1,7}],{1,7},{1})",
            Status::Invalid,
        ),
        (
            "gecode_array_set_element_intersect_in({1},[{1,7}],{1},{1})",
            Status::Invalid,
        ),
        (
            "gecode_array_set_element_union({0},[{}],{})",
            Status::Invalid,
        ),
        (
            "gecode_array_set_element_union(-9223372036854775808..9223372036854775807,[{}],{})",
            Status::Invalid,
        ),
        (
            "gecode_array_set_element_partition({1,2},[{7},{7}],{7})",
            Status::Invalid,
        ),
        (
            "gecode_link_set_to_booleans({4},[false,true],3)",
            Status::Valid,
        ),
        (
            "gecode_link_set_to_booleans({0},[false,true],3)",
            Status::Invalid,
        ),
        (
            "gecode_link_set_to_booleans({},[],1000000000)",
            Status::Valid,
        ),
        ("gecode_link_set_to_booleans({3},[],3)", Status::Invalid),
        (
            "gecode_range([9007199254740993,7],1000000000,{1,1000000000},{0,9007199254740993})",
            Status::Valid,
        ),
        ("gecode_range([],5,{}, {})", Status::Valid),
        ("gecode_range([],5,{0},{0})", Status::Invalid),
        ("gecode_int_set_channel([1,2],1,[{1},{2}],1)", Status::Valid),
        ("gecode_int_set_channel([0],0,[{0}],0)", Status::Valid),
        ("gecode_int_set_channel([1],1,[{}, {1}],0)", Status::Invalid),
        ("gecode_int_set_channel([],3,[],1)", Status::Invalid),
        ("gecode_int_set_channel([],3,[{}],0)", Status::Valid),
        ("gecode_int_set_channel([1],0,[{0}],1)", Status::Valid),
        ("gecode_int_set_channel([1],0,[{}],1)", Status::Invalid),
        ("gecode_inverse_set([{4},{3}],[{2},{1}],1,3)", Status::Valid),
        (
            "gecode_inverse_set([], [{}],1000000000,1000000000)",
            Status::Valid,
        ),
        ("gecode_inverse_set([], [{0}],2,3)", Status::Invalid),
        ("gecode_inverse_set([{0}],[{}],1,3)", Status::Invalid),
        ("gecode_precede_set([{1,2},{2}],1,2)", Status::Invalid),
        ("gecode_precede_set([{1},{1,2},{2}],1,2)", Status::Valid),
        ("gecode_precede_set([{2},{1}],1,2)", Status::Invalid),
        ("gecode_precede_set([{1,2}],1,1)", Status::Valid),
        ("gecode_precede_set([{}],1,2)", Status::Valid),
        ("gecode_set_channel_sorted([1,3],{1,3})", Status::Valid),
        ("gecode_set_channel_sorted([3,1],{1,3})", Status::Invalid),
        ("gecode_set_channel_sorted([1,1],{1})", Status::Invalid),
        ("gecode_set_channel_sorted([],{})", Status::Valid),
        (
            "gecode_set_channel_sorted([],-1073741822..1073741822)",
            Status::Invalid,
        ),
    ] {
        let source = format!("constraint {call};solve satisfy;");
        let model = prepare("channel.fzn", &source).unwrap();
        let report = model.check(&Assignment::default()).unwrap();
        assert_eq!(report.status, status, "{call}");
        assert!(report.completeness.complete, "{call}");
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::to_value(
                PreparedModel::from_json(&model.to_json().unwrap())
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
            )
            .unwrap()
        );
    }
    let alias_model = prepare("repeated.fzn", "var set of int:shared;var set of int:result::is_defined_var;constraint gecode_array_set_element_partition({1,2},[shared,shared],result)::defines_var(result);constraint set_eq(result,{7});solve satisfy;").unwrap();
    let alias_report = alias_model.check(&assignment("shared={7};")).unwrap();
    assert_eq!(alias_report.status, Status::Invalid);
    assert!(alias_report.completeness.complete);
    assert_eq!(alias_report.violations.len(), 1);
    for source in [
        "var set of int:missing;constraint gecode_array_set_element_union({1},[{7},missing],{7});solve satisfy;",
        "var int:missing;constraint gecode_range([7,missing],3,{0,3},{0,7});solve satisfy;",
    ] {
        let report = prepare("selected.fzn", source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap();
        assert_eq!(report.status, Status::Incomplete);
        assert!(
            report
                .completeness
                .blocked_checks
                .iter()
                .all(|blocked| matches!(blocked.check, CheckRef::Domain { .. }))
        );
    }
    for source in [
        "var set of int:s;constraint gecode_int_set_channel([],3,[s],1);solve satisfy;",
        "var int:x;constraint gecode_int_set_channel([x],0,[],0);solve satisfy;",
        "var int:x;var set of int:s;constraint gecode_int_set_channel([x],1,[s],0);solve satisfy;",
        "var set of int:s;constraint gecode_array_set_element_partition({1,2,3},[{1},{1},s],{});solve satisfy;",
        "var int:x;constraint gecode_set_channel_sorted([x],{});solve satisfy;",
    ] {
        let report = prepare("partial.fzn", source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap();
        assert_eq!(report.status, Status::Invalid, "{source}");
        assert!(!report.completeness.complete);
        assert!(
            report
                .completeness
                .blocked_checks
                .iter()
                .all(|blocked| matches!(blocked.check, CheckRef::Domain { .. }))
        );
    }
}

#[test]
fn set_channel_definitions_complete_only_determined_outputs() {
    for (declarations, call, check, status, complete) in [
        (
            "var set of int:s::is_defined_var;",
            "gecode_array_set_element_union({1},[{7},s],s)::defines_var(s)",
            "set_eq(s,{7})",
            Status::Valid,
            true,
        ),
        (
            "var set of int:s::is_defined_var;",
            "gecode_array_set_element_union({1},[s],s)::defines_var(s)",
            "set_eq(s,{7})",
            Status::Incomplete,
            false,
        ),
        (
            "var set of int:s::is_defined_var;",
            "gecode_array_set_element_union({0},[{7}],s)::defines_var(s)",
            "set_eq(s,{7})",
            Status::Invalid,
            false,
        ),
        (
            "var set of int:s::is_defined_var;",
            "gecode_array_set_element_intersect_in({1},[{7}],s,{1})::defines_var(s)",
            "set_eq(s,{7})",
            Status::Invalid,
            true,
        ),
        (
            "var set of int:s::is_defined_var;",
            "gecode_array_set_element_partition({1,2},[{7},{7}],s)::defines_var(s)",
            "set_eq(s,{7})",
            Status::Invalid,
            true,
        ),
        (
            "var set of int:s::is_defined_var;",
            "gecode_link_set_to_booleans(s,[true,false],4)::defines_var(s)",
            "set_eq(s,{4})",
            Status::Valid,
            true,
        ),
        (
            "var bool:b::is_defined_var;",
            "gecode_link_set_to_booleans({},[b,b],4)::defines_var(b)",
            "bool_eq(b,false)",
            Status::Valid,
            true,
        ),
        (
            "var bool:b::is_defined_var;",
            "gecode_link_set_to_booleans({4},[b,b],4)::defines_var(b)",
            "bool_eq(b,false)",
            Status::Incomplete,
            false,
        ),
        (
            "var int:x::is_defined_var;",
            "gecode_int_set_channel([x],1,[{1},{}],3)::defines_var(x)",
            "int_eq(x,3)",
            Status::Valid,
            true,
        ),
        (
            "var int:x::is_defined_var;",
            "gecode_int_set_channel([x],1,[{},{}],3)::defines_var(x)",
            "int_eq(x,3)",
            Status::Invalid,
            false,
        ),
        (
            "var int:x::is_defined_var;",
            "gecode_int_set_channel([x],1,[{1},{1}],3)::defines_var(x)",
            "int_eq(x,3)",
            Status::Invalid,
            false,
        ),
        (
            "var set of int:s::is_defined_var;",
            "gecode_inverse_set([s],[{2}],2,3)::defines_var(s)",
            "set_eq(s,{3})",
            Status::Valid,
            true,
        ),
        (
            "var set of int:s::is_defined_var;",
            "gecode_inverse_set([{3}],[s],2,3)::defines_var(s)",
            "set_eq(s,{2})",
            Status::Valid,
            true,
        ),
        (
            "var set of int:s::is_defined_var;",
            "gecode_set_channel_sorted([1,3],s)::defines_var(s)",
            "set_eq(s,{1,3})",
            Status::Valid,
            true,
        ),
        (
            "var int:x::is_defined_var;",
            "gecode_set_channel_sorted([x,x],{1,3})::defines_var(x)",
            "int_eq(x,1)",
            Status::Incomplete,
            false,
        ),
        (
            "var int:x::is_defined_var;",
            "gecode_set_channel_sorted([x],1..1000000000)::defines_var(x)",
            "int_eq(x,1)",
            Status::Invalid,
            false,
        ),
        (
            "var set of 1..1:s::is_defined_var;",
            "gecode_range([7],0,{0},s)::defines_var(s)",
            "set_eq(s,{})",
            Status::Invalid,
            true,
        ),
    ] {
        let source = format!("{declarations}constraint {call};constraint {check};solve satisfy;");
        let report = prepare("definitions.fzn", &source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap();
        assert_eq!(
            (report.status, report.completeness.complete),
            (status, complete),
            "{source}"
        );
        if declarations.contains("set of 1..1") {
            assert_eq!(report.violations.len(), 2);
        }
    }
    let model = prepare("prefix.fzn", "var set of int:s::is_defined_var;constraint gecode_int_set_channel([1],1,[s,{1}],0)::defines_var(s);constraint set_eq(s,{0});solve satisfy;").unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    let report = model.check(&Assignment::default()).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(report.completeness.complete);
    assert_eq!(report.violations.len(), 1);
    assert!(
        matches!(&report.violations[0].check, CheckRef::Constraint { constraint } if constraint.predicate == "gecode_int_set_channel")
    );
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(
        json["violations"][0]["details"]["finding"]["arguments"][1]["value"]["value"],
        serde_json::json!({"kind":"integer_set","value":{"ranges":[{"lower":"0","upper":"0"}]}})
    );
    assert_eq!(
        json,
        serde_json::to_value(loaded.check(&Assignment::default()).unwrap()).unwrap()
    );
    for source in [
        "var int:x::is_defined_var;var set of int:s;constraint gecode_range([x],0,s,{})::defines_var(x);solve satisfy;",
        "var set of int:s::is_defined_var;constraint gecode_link_set_to_booleans(s,[],0)::defines_var(s);constraint gecode_set_channel_sorted([],s)::defines_var(s);solve satisfy;",
        "array[1..2] of var int:x::is_defined_var;constraint gecode_set_channel_sorted([x[1]],{})::defines_var(x);solve satisfy;",
    ] {
        // Unsupported definition directions stay unresolved; malformed targets/competing definitions fail.
        let result = prepare("targets.fzn", source);
        if source.contains("gecode_range") {
            assert_eq!(
                result
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                Status::Incomplete
            );
        } else {
            assert!(result.is_err());
        }
    }
}

#[test]
fn set_channel_reports_round_trip_after_edits_and_removed_inputs() {
    let model = prepare(
        "set-channels.fzn",
        include_str!("../../../examples/checker-compilation/set-channels.fzn"),
    )
    .unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for (candidate, status, complete) in [
        (
            "left={1};right={3};selected={1,2};value=9007199254740993;",
            Status::Valid,
            true,
        ),
        (
            "left={1};right={1};selected={1,2};value=9007199254740993;ordered=[1,1];",
            Status::Invalid,
            true,
        ),
        (
            "left={1};right={3};selected={0};value=7;",
            Status::Invalid,
            false,
        ),
        (
            "left={1};right={3};selected={1,2};value=7;combined={2,3};",
            Status::Invalid,
            true,
        ),
        ("left={1};right={1};selected={1,2};", Status::Invalid, false),
        ("", Status::Incomplete, false),
    ] {
        let report = model.check(&assignment(candidate)).unwrap();
        assert_eq!(
            (report.status, report.completeness.complete),
            (status, complete),
            "{candidate}"
        );
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json,
            serde_json::to_value(loaded.check(&assignment(candidate)).unwrap()).unwrap()
        );
        if candidate.contains("selected={0}") {
            let finding = &json["violations"][0];
            assert_eq!(finding["details"]["finding"]["arguments"][1]["index"], "1");
            assert_eq!(
                finding["details"]["finding"]["faults"][0]["members"]["ranges"][0]["lower"],
                "0"
            );
            assert_eq!(
                finding["details"]["finding"]["arguments"][3]["unresolved"]["name"],
                "combined"
            );
            assert_eq!(
                finding["check"]["constraint"]["metadata"]["application"]["entities"][0],
                "9007199254740993"
            );
            let text = zincite_check::cli::render_text(&report);
            assert!(
                text.contains("selected sets")
                    && text.contains("outside")
                    && text.contains("combined unresolved")
            );
        }
    }
}

#[test]
fn number_theory_and_distance_signatures_and_mathematical_cases() {
    // All 60 registry spellings use positional types, independent of formal names.
    let roots = [
        (
            "gecode_inter_distance",
            "array[int] of var int:a,var int:b",
            "[1,4],3",
        ),
        (
            "gecode_inter_distance_fzn",
            "array[int] of var int:a,var int:b",
            "[],0",
        ),
        (
            "gecode_gcd_int",
            "var int:a,var int:b,var int:c",
            "-12,18,6",
        ),
        (
            "gecode_gcd_int_reif",
            "var int:a,var int:b,var int:c,var bool:d",
            "0,0,0,true",
        ),
        (
            "gecode_gcd_int_imp",
            "var int:a,var int:b,var int:c,var bool:d",
            "0,2,1,false",
        ),
        ("gecode_divides_int", "var int:a,var int:b", "0,0"),
        (
            "gecode_divides_int_reif",
            "var int:a,var int:b,var bool:c",
            "0,2,false",
        ),
        (
            "gecode_divides_int_imp",
            "var int:a,var int:b,var bool:c",
            "0,2,false",
        ),
        (
            "gecode_product_int",
            "array[int] of var int:a,var int:b",
            "[],1",
        ),
        (
            "gecode_product_int_reif",
            "array[int] of var int:a,var int:b,var bool:c",
            "[-2,3],-6,true",
        ),
        (
            "gecode_product_int_imp",
            "array[int] of var int:a,var int:b,var bool:c",
            "[9223372036854775807,2],0,false",
        ),
        (
            "gecode_product_mod_fixed_fzn",
            "array[int] of var int:a,int:b,var int:c",
            "[-2,3],5,4",
        ),
        (
            "gecode_product_mod_fixed_fzn_reif",
            "array[int] of var int:a,int:b,var int:c,var bool:d",
            "[],1,0,true",
        ),
        (
            "gecode_product_mod_fixed_fzn_imp",
            "array[int] of var int:a,int:b,var int:c,var bool:d",
            "[],5,0,false",
        ),
        (
            "gecode_product_mod_var_fzn",
            "array[int] of var int:a,var int:b,var int:c",
            "[-2,3],5,4",
        ),
        (
            "gecode_product_mod_var_fzn_reif",
            "array[int] of var int:a,var int:b,var int:c,var bool:d",
            "[],0,1,false",
        ),
        (
            "gecode_product_mod_var_fzn_imp",
            "array[int] of var int:a,var int:b,var int:c,var bool:d",
            "[],-1,0,false",
        ),
        (
            "gecode_minimum_distance_fzn",
            "array[int] of var int:a,int:b,array[int] of int:c,var int:d",
            "[0,1],2,[0,7,7,0],7",
        ),
        (
            "gecode_minimum_distance_required_fzn",
            "array[int] of var int:a,int:b,array[int] of int:c,var int:d,array[int] of int:e",
            "[1,0],2,[0,7,7,0],7,[0,7,7,0]",
        ),
        ("gecode_int_nroot", "var int:a,int:b,var int:c", "-9,3,-3"),
    ];
    for prefix in ["", "gecode_", "fzn_"] {
        for (root, parameters, args) in roots {
            let source = format!(
                "predicate {prefix}{root}({parameters});constraint {prefix}{root}({args});solve satisfy;"
            );
            let model = prepare("registered.fzn", &source).unwrap();
            assert_eq!(
                model.check(&Assignment::default()).unwrap().status,
                Status::Valid,
                "{source}"
            );
        }
        for (root, args) in [
            ("gecode_gcd_int_imp", "12,18,6,true"),
            ("gecode_divides_int_imp", "3,12,true"),
            ("gecode_product_int_imp", "[-2,3],-6,true"),
            ("gecode_product_mod_fixed_fzn_imp", "[-2,3],5,4,true"),
            ("gecode_product_mod_var_fzn_imp", "[-2,3],5,4,true"),
        ] {
            let source = format!("constraint {prefix}{root}({args});solve satisfy;");
            assert_eq!(
                prepare("active-guard.fzn", &source)
                    .unwrap()
                    .check(&Assignment::default())
                    .unwrap()
                    .status,
                Status::Valid,
                "{source}"
            );
        }
    }
    for (call, valid) in [
        ("gecode_gcd_int(-9223372036854775808,2,2)", true),
        ("gecode_gcd_int(0,-8,-8)", false),
        ("gecode_divides_int(-1,-9223372036854775808)", true),
        ("gecode_divides_int(-3,12)", true),
        ("gecode_divides_int(0,1)", false),
        ("gecode_product_int([-2,-3],6)", true),
        ("gecode_product_int([9223372036854775807,2,0],0)", true),
        (
            "gecode_product_mod_var_fzn([9223372036854775807,9223372036854775807],9223372036854775807,0)",
            true,
        ),
        ("gecode_product_mod_var_fzn_reif([-2,3],5,-1,false)", true),
        ("gecode_product_mod_var_fzn_reif([],0,0,true)", false),
        ("gecode_int_nroot(9,2,3)", true),
        ("gecode_int_nroot(8,3,2)", true),
        ("gecode_int_nroot(9,3,2)", true),
        ("gecode_int_nroot(-8,3,-2)", true),
        ("gecode_int_nroot(-9,3,-2)", false),
        (
            "gecode_int_nroot(-9223372036854775808,1,-9223372036854775808)",
            true,
        ),
        ("gecode_int_nroot(-9223372036854775808,3,-2097152)", true),
        ("gecode_int_nroot(9223372036854775807,2,3037000499)", true),
        ("gecode_int_nroot(9223372036854775807,2147483646,1)", true),
        ("gecode_int_nroot(-2,2147483645,-2)", true),
        ("gecode_inter_distance([], -1)", false),
        ("gecode_inter_distance([7], 0)", true),
        ("gecode_inter_distance([7,7], 1)", false),
        ("gecode_inter_distance([7,7], 0)", true),
        ("gecode_minimum_distance_fzn([0,0],1,[0],0)", true),
        (
            "gecode_minimum_distance_fzn([0,1,2],3,[0,5,1,5,0,8,1,8,0],1)",
            true,
        ),
        (
            "gecode_minimum_distance_required_fzn([2,0,1],3,[0,5,1,5,0,8,1,8,0],1,[0,1,8,1,0,5,8,5,0])",
            true,
        ),
        (
            "gecode_minimum_distance_required_fzn([2,0,1],3,[0,5,1,5,0,8,1,8,0],1,[0,2,8,2,0,5,8,5,0])",
            false,
        ),
    ] {
        let source = format!("constraint {call};solve satisfy;");
        let report = prepare("math.fzn", &source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap();
        assert_eq!(
            report.status,
            if valid {
                Status::Valid
            } else {
                Status::Invalid
            },
            "{call}"
        );
        assert!(report.completeness.complete);
    }
    for call in [
        "gecode_gcd_int(-9223372036854775808,0,0)",
        "gecode_product_int([9223372036854775807,2],0)",
        "gecode_int_nroot(-1,2,0)",
        "gecode_inter_distance([-9223372036854775808,9223372036854775807],0)",
    ] {
        let model = prepare("overflow.fzn", &format!("constraint {call};solve satisfy;")).unwrap();
        let error = model.check(&Assignment::default()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Execution);
        assert_eq!(
            error.diagnostics[0].location.as_ref().unwrap().source,
            "overflow.fzn"
        );
    }
}

#[test]
fn arithmetic_extension_completion_reports_conflicts_and_restarts_after_edits() {
    let source = include_str!("../../../examples/checker-compilation/arithmetic-extensions.fzn");
    let model = prepare("extensions.fzn", source).unwrap();
    let valid = assignment("x=-9;y=6;modulus=5;sites=[0,1];");
    assert_eq!(model.check(&valid).unwrap().status, Status::Valid);
    let report = model
        .check(&assignment(
            "x=9007199254740993;y=1;modulus=5;sites=[0,1];product=0;root=0;minimum=8;truth=false;",
        ))
        .unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(report.completeness.complete);
    assert!(
        report
            .violations
            .iter()
            .any(|v| matches!(v.details, ViolationKind::NumberTheory { .. }))
    );
    assert!(
        report
            .violations
            .iter()
            .any(|v| matches!(v.details, ViolationKind::Distance { .. }))
    );
    let text = zincite_check::cli::render_text(&report);
    assert!(text.contains("9007199254740993") && text.contains("minimum distance is 7"));
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for candidate in [
        &valid,
        &assignment("x=-9;y=6;modulus=0;sites=[1,1];"),
        &Assignment::default(),
    ] {
        let before = model.check(candidate).unwrap();
        let after = loaded.check(candidate).unwrap();
        assert_eq!(
            serde_json::to_value(&before).unwrap(),
            serde_json::to_value(&after).unwrap()
        );
        assert_eq!(
            zincite_check::cli::render_text(&before),
            zincite_check::cli::render_text(&after)
        );
    }
    assert_eq!(model.check(&valid).unwrap().status, Status::Valid);

    // Pair requirements can fail while the exact minimum supplies downstream values.
    let source = "var int:a;var int:b;var 2..2:m::is_defined_var;var int:next::is_defined_var;constraint gecode_minimum_distance_required_fzn([a,b],2,[0,7,7,0],m,[0,8,8,0])::defines_var(m);constraint int_plus(m,1,next)::defines_var(next);constraint int_eq(next,0);solve satisfy;";
    let model = prepare("distance.fzn", source).unwrap();
    let report = model.check(&assignment("a=0;b=1;")).unwrap();
    assert!(report.completeness.complete);
    assert_eq!(report.violations.len(), 3);
    let ViolationKind::Distance {
        finding:
            zincite_check::DistanceFinding::Minimum {
                expected,
                result,
                faults,
                ..
            },
    } = &report.violations[1].details
    else {
        panic!("distance finding")
    };
    assert_eq!(*expected, Some(7));
    assert_eq!(result.value.as_ref().unwrap().value, Value::Integer(7));
    assert!(matches!(
        faults[0],
        zincite_check::DistanceFault::PairSeparation { required: 8, .. }
    ));
    let report = model.check(&assignment("a=2;")).unwrap();
    assert_eq!(report.status, Status::Invalid);
    assert!(!report.completeness.complete);
    let finding = serde_json::to_value(&report.violations[0]).unwrap();
    assert_eq!(
        finding["details"]["finding"]["sites"][1]["unresolved"]["name"],
        "b"
    );

    // Marking an inverse direction never chooses a factor, divisor, root input or site.
    for call in [
        "gecode_product_int([x,2],6)",
        "gecode_gcd_int(x,6,3)",
        "gecode_int_nroot(x,3,2)",
        "gecode_minimum_distance_fzn([x,0],2,[0,7,7,0],7)",
        "gecode_divides_int(x,6)",
        "gecode_inter_distance([x,0],2)",
        "gecode_product_int_imp([2],x,true)",
    ] {
        let source =
            format!("var int:x::is_defined_var;constraint {call}::defines_var(x);solve satisfy;");
        let report = prepare("inverse.fzn", &source)
            .unwrap()
            .check(&Assignment::default())
            .unwrap();
        assert_eq!(report.status, Status::Incomplete, "{call}");
    }
}

#[test]
fn arithmetic_extension_source_rejects_malformed_contracts() {
    for call in [
        "gecode_gcd_int(1,2)",
        "gcd_int(1,2,1)",
        "gecode_gecode_gecode_gcd_int(1,2,1)",
        "gecode_product_int([true],1)",
        "gecode_product_mod_fixed_fzn_imp([],0,0,false)",
        "gecode_product_mod_fixed_fzn([],2147483647,0)",
        "gecode_product_mod_fixed_fzn([],x,0)",
        "gecode_int_nroot(1,0,1)",
        "gecode_int_nroot(1,2147483647,1)",
        "gecode_int_nroot(1,x,1)",
        "gecode_minimum_distance_fzn([],1,[0],0)",
        "gecode_minimum_distance_fzn([0,0],0,[],0)",
        "gecode_minimum_distance_fzn([0,0],2,[0],0)",
        "gecode_minimum_distance_fzn([0,0],2,[0,1,2,0],0)",
        "gecode_minimum_distance_fzn([0,0],1,[1],0)",
        "gecode_minimum_distance_fzn([0,0],2,[0,-1,-1,0],0)",
        "gecode_minimum_distance_fzn([0,0],2,[0,2147483647,2147483647,0],0)",
        "gecode_minimum_distance_required_fzn([0,0],1,[0],0,[0])",
        "gecode_minimum_distance_required_fzn([0,0],1,[0],0,[0,1,2,0])",
        "gecode_minimum_distance_required_fzn([0,0],1,[0],0,[1,0,0,0])",
        "gecode_minimum_distance_required_fzn([0,0],1,[0],0,[0,-1,-1,0])",
    ] {
        let error = prepare(
            "bad.fzn",
            &format!("var int:x;\nconstraint {call};solve satisfy;"),
        )
        .unwrap_err();
        assert_eq!(
            error[0].location.as_ref().unwrap().source,
            "bad.fzn",
            "{call}"
        );
    }
}

#[test]
fn pinned_gecode_artifact_completes_edits_and_preserves_feedback() {
    let source = include_str!("../../../examples/checker-compilation/gecode-controls.fzn");
    let model = prepare("gecode.fzn", source).unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for (candidate, status, complete, findings) in [
        (
            include_str!("../../../examples/checker-compilation/candidates/gecode-valid.dzn"),
            Status::Valid,
            true,
            0,
        ),
        (
            include_str!("../../../examples/checker-compilation/candidates/gecode-invalid.dzn"),
            Status::Invalid,
            true,
            4,
        ),
        (
            include_str!("../../../examples/checker-compilation/candidates/gecode-conflict.dzn"),
            Status::Invalid,
            true,
            1,
        ),
        (
            include_str!("../../../examples/checker-compilation/candidates/gecode-partial.dzn"),
            Status::Incomplete,
            false,
            0,
        ),
        ("budget_offset=-3;", Status::Invalid, false, 2),
    ] {
        let candidate = assignment(candidate);
        let report = model.check(&candidate).unwrap();
        assert_eq!(report.status, status);
        assert_eq!(report.completeness.complete, complete);
        assert_eq!(report.violations.len(), findings);
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::to_value(loaded.check(&candidate).unwrap()).unwrap()
        );
    }
    let conflict = model
        .check(&assignment(include_str!(
            "../../../examples/checker-compilation/candidates/gecode-conflict.dzn"
        )))
        .unwrap();
    assert_eq!(
        conflict.objective.unwrap().argument.value,
        Value::Integer(0)
    );
    let CheckRef::Constraint { constraint } = &conflict.violations[0].check else {
        panic!("definition constraint expected")
    };
    assert_eq!(
        constraint.metadata.application.as_ref().unwrap().id,
        "offset"
    );
    assert_eq!(
        constraint.metadata.value_application.as_ref().unwrap().id,
        "tool_capacity"
    );
    assert_eq!(
        constraint.metadata.application.as_ref().unwrap().entities,
        [9_007_199_254_740_993]
    );
    let recovered = model
        .check(&assignment(include_str!(
            "../../../examples/checker-compilation/candidates/gecode-valid.dzn"
        )))
        .unwrap();
    assert_eq!(recovered.status, Status::Valid);
}
