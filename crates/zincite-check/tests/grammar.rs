#![cfg(feature = "prepare")]

use zincite_check::{
    Assignment, CheckRef, PreparedModel, Status, Value, parse_assignment, prepare,
};

fn check(model: &PreparedModel, assignment: &str) -> zincite_check::CheckReport {
    model
        .check(&parse_assignment("candidate.dzn", assignment).unwrap())
        .unwrap()
}

#[test]
fn text_forms_validate_signatures_parameters_aliases_and_exact_literals() {
    // These signatures follow flatzinc_builtins.mzn; formal spelling is immaterial.
    let source = r#"
        predicate int_lin_eq(array [int] of int: coefficients, array [int] of var int: terms, int: bound);
        predicate int_lin_le(array[int] of int: as, array[int] of var int: xs, int: limit);
        predicate set_in(var int: x, set of int: s);
        predicate set_eq(var set of int: s, var set of int: t);
        predicate set_subset(var set of int: s, var set of int: t);
        predicate set_card(var set of int: s, var int: x);
        predicate set_union(var set of int: s, var set of int: t, var set of int: r);
        predicate set_intersect(var set of int: s, var set of int: t, var set of int: r);
        predicate int_le_reif(var int: first, var int: second, var bool: result);
        predicate fzn_disjunctive_strict(array[int] of var int: begins, array[int] of var int: lengths);
        int: _1 = -0x8000000000000000;
        bool: enabled = true;
        set of int: allowed = {-0o10,0x10};
        array[1..2] of int: coefficients = [1,0];
        array[1..0] of set of int: empty = [];
        var int: __x = _1;
        var int: alias = __x;
        var bool: ok = enabled;
        var set of int: selection = allowed;
        array[1..2] of var int: pair = [alias, __x];
        array[1..2] of var int: other = pair;
        var int: indexed = other[1];
        constraint int_lin_eq(coefficients,other,_1);
        constraint int_le_reif(indexed,0,ok);
        constraint int_lin_le([1],[indexed],0);
        constraint set_in(-8,allowed);
        constraint set_eq(selection,allowed);
        constraint set_subset(selection,allowed);
        constraint set_card(selection,2);
        constraint set_union(allowed,selection,selection);
        constraint set_intersect(selection,allowed,selection);
        constraint fzn_disjunctive_strict([],[]);
        solve minimize indexed;
    "#;
    let model = prepare("forms.fzn", source).unwrap();
    let report = check(&model, "__x=-0x8000000000000000; selection={-0o10,0x10};");
    assert_eq!(report.status, Status::Valid);
    assert_eq!(
        report.objective.unwrap().argument.value,
        Value::Integer(i64::MIN)
    );
    let assignment = parse_assignment(
        "limits.dzn",
        "a=0x7fffffffffffffff;b=-0o1000000000000000000000;",
    )
    .unwrap();
    assert_eq!(assignment.entries.len(), 2);

    // Formal domains and fixed array lengths constrain calls as well as declarations.
    let restricted = "predicate int_lin_le(array[1..1] of 1..2:a,array[int] of var 0..5:b,0..9:c);var 1..4:x;constraint int_lin_le([1],[x],3);solve satisfy;";
    assert!(prepare("restricted.fzn", restricted).is_ok());
    assert!(
        prepare(
            "restricted.fzn",
            &restricted.replace("var 1..4", "var 1..6")
        )
        .is_err()
    );
    assert!(prepare("restricted.fzn", &restricted.replace("([1]", "([3]")).is_err());
}

#[test]
fn arithmetic_admission_validates_types_shapes_domains_and_exponent_roles() {
    for source in [
        "predicate int_plus(var -2..2:a,var int:b,var int:c);var -1..1:x;constraint int_plus(x,1,2);solve satisfy;",
        "int:e=-1;var int:x;constraint int_pow_fixed(x,e,0);solve satisfy;",
        "var int:e;constraint int_pow(1,e,1);solve satisfy;",
        "constraint gecode_int_pow(1,0,1);solve satisfy;",
    ] {
        assert!(prepare("admission.fzn", source).is_ok(), "{source}");
    }
    for source in [
        "constraint int_plus(1,2);solve satisfy;",
        "constraint int_abs(1,2,3);solve satisfy;",
        "constraint int_plus(true,1,2);solve satisfy;",
        "constraint int_plus(1,2,false);solve satisfy;",
        "constraint int_abs([1],1);solve satisfy;",
        "constraint int_min(1,{},1);solve satisfy;",
        "constraint int_times(1,2,[2]);solve satisfy;",
        "predicate int_abs(int:a,var int:b);solve satisfy;",
        "predicate int_plus(var int:a,var bool:b,var int:c);solve satisfy;",
        "predicate int_pow(var int:a,int:b,var int:c);solve satisfy;",
        "predicate int_pow_fixed(var int:a,var int:b,var int:c);solve satisfy;",
        "predicate gecode_int_pow(var int:a,var int:b,var int:c);solve satisfy;",
        "predicate int_plus(var -2..2:a,var int:b,var int:c);var -3..3:x;constraint int_plus(x,1,2);solve satisfy;",
        "var int:e=2;constraint int_pow_fixed(1,e,1);solve satisfy;",
        "var 2..2:e;constraint gecode_int_pow(1,e,1);solve satisfy;",
        "constraint gecode_int_pow(1,-1,1);solve satisfy;",
        "constraint gecode_gecode_int_pow(1,2147483647,1);solve satisfy;",
        "constraint fzn_gecode_int_pow(1,-1,1);solve satisfy;",
        "constraint fzn_int_pow(1,2,1);solve satisfy;",
        "constraint gecode_int_pow_fixed(1,2,1);solve satisfy;",
        "predicate fzn_int_pow_fixed(var int:a,int:b,var int:c);solve satisfy;",
    ] {
        let errors = prepare("bad-arithmetic.fzn", source).unwrap_err();
        assert!(errors[0].location.is_some(), "{source}");
    }
}

#[test]
fn fixed_boolean_linear_bounds_work_for_each_literal_alias() {
    for name in ["bool_lin_le", "gecode_bool_lin_le", "fzn_bool_lin_le"] {
        for bound_type in ["int", "var int"] {
            let source = format!(
                "predicate {name}(array[int] of int:a,array[int] of var bool:x,{bound_type}:c);\
                 var bool:x;constraint {name}([2],[x],1);solve satisfy;"
            );
            let model = prepare("bounds.fzn", &source).unwrap();
            assert_eq!(check(&model, "x=false;").status, Status::Valid);
            assert_eq!(check(&model, "x=true;").status, Status::Invalid);
        }
    }
    assert!(prepare("fixed-elements.fzn", "predicate array_int_element(var int:i,array[int] of var int:a,var int:r);solve satisfy;").is_err());
}

#[test]
fn external_execution_and_restart_history_have_located_reasons() {
    for (source, reason) in [
        (
            "predicate gecode_blackbox(array[int] of var int:a,array[int] of var float:b,array[int] of var int:c,array[int] of var float:d);solve satisfy;",
            "external executable or DLL execution",
        ),
        (
            "constraint fzn_gecode_blackbox_bounds([],[],[]);solve satisfy;",
            "external executable or DLL execution",
        ),
        (
            "constraint int_eq(1,1)::blackbox_dll(\"library\");solve satisfy;",
            "external executable or DLL execution",
        ),
        (
            "constraint fzn_on_restart_sol_int(1,1);solve satisfy;",
            "restart status/history",
        ),
        (
            "solve::seq_search([on_restart(true)]) satisfy;",
            "restart status/history",
        ),
    ] {
        let errors = prepare("hook.fzn", source).unwrap_err();
        assert!(errors[0].location.is_some());
        assert!(errors[0].message.contains(reason), "{errors:?}");
    }
    // These seven upstream EOF prototypes omit semicolons; valid declarations still reject history.
    for signature in [
        "gecode_on_restart_last_val_bool(var bool:input,var bool:out)",
        "gecode_on_restart_last_val_float(var float:input,var float:out)",
        "gecode_on_restart_last_val_int(var int:input,var int:out)",
        "gecode_on_restart_last_val_set(var set of int:input,var set of int:out)",
        "gecode_on_restart_sol_float(var float:input,var float:out)",
        "gecode_on_restart_status(var int:s)",
        "gecode_on_restart_uniform_float(float:low,float:high,var float:out)",
    ] {
        let source = format!("predicate {signature};solve satisfy;");
        let errors = prepare("history.fzn", &source).unwrap_err();
        assert!(errors[0].location.is_some());
        assert!(errors[0].message.contains("restart status/history"));
    }
}

#[test]
fn standard_and_gecode_metadata_validate_nested_roles_without_changing_checks() {
    // Search and propagation names/signatures come from stdlib_ann.mzn and gecode.mzn.
    let source = r#"
        array[1..2] of int: initial=[1,2];
        array[1..2] of set of int: dimensions=[{-2,4},7..7];
        var 0..3: x ::var_is_introduced ::var_is_introduced ::output_var
            ::checker_value("caf\xc3\xa9\t\101\'\"\\",[9007199254740993]);
        var int: y ::is_defined_var ::mzn_path("model.mzn|2|1") ::mzn_path(23);
        var bool: b;
        var set of 1..3: s;
        array[1..2] of var int: xs ::output_array(dimensions)=[x,y];
        array[1..0] of var bool: none ::output_array([{-8},{}])=[];
        constraint int_lin_eq([1,-1],[y,x],1)::defines_var(y)::domain::bounds_propagation
            ::gecode_advanced_propagation::gecode_full_propagation::gecode_decomposed_propagation
            ::checker_constraint("offset",[9007199254740993])::mzn_constraint_name("Offset")::mzn_path("source.mzn|3|1");
        constraint int_lin_le([1],[y],1)::value_propagation::gecode_basic_propagation;
        solve ::seq_search([int_search(xs,afc_size_max,indomain_split_random,complete),
                           seq_search([bool_search([b],random,indomain_min,complete),
                                       set_search([s],action_max,outdomain_min,complete)])])
              ::warm_start_array([warm_start(xs,initial),warm_start([b],[true]),warm_start([s],[{1}])])
              ::int_assign(xs,indomain_random)::int_default_search(first_fail,indomain_min)
              ::bool_default_search(input_order,indomain_max)::set_default_search(afc_min,outdomain_max)
              ::float_default_search(first_fail,indomain_split)
              ::restart_luby(10)::restart_linear(2)::restart_constant(5)::restart_none
              ::restart_geometric(1.5e0,2)::relax_and_reconstruct(xs,50,initial) minimize y;
    "#;
    let model = prepare("annotations.fzn", source).unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for assignment in ["x=0;b=true;s={1};", "x=3;b=false;s={};", "b=true;s={1};"] {
        let report = check(&model, assignment);
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::to_value(check(&loaded, assignment)).unwrap()
        );
    }
    let report = check(&model, "x=3;b=false;s={};");
    assert_eq!(report.status, Status::Invalid);
    assert_eq!(report.violations.len(), 1);
    let CheckRef::Constraint { constraint } = &report.violations[0].check else {
        panic!("constraint expected")
    };
    assert_eq!(constraint.ordinal, 2);
    let missing = check(&model, "");
    assert_eq!(missing.status, Status::Incomplete);
    let x = missing
        .completeness
        .unresolved_variables
        .iter()
        .find(|variable| variable.name == "x")
        .unwrap();
    assert_eq!(x.metadata.application.as_ref().unwrap().id, "café\tA'\"\\");
    assert_eq!(
        x.metadata.application.as_ref().unwrap().entities,
        [9_007_199_254_740_993]
    );
    assert!(x.metadata.output);

    let plain = prepare(
        "plain.fzn",
        "var int:x;constraint int_lin_le([1],[x],0);solve satisfy;",
    )
    .unwrap();
    let annotated = prepare("plain.fzn", "var int:x::var_is_introduced;constraint int_lin_le([1],[x],0)::domain;solve::int_search([x],first_fail,indomain_min,complete) satisfy;").unwrap();
    for assignment in ["x=0;", "x=1;", ""] {
        let a = check(&plain, assignment);
        let b = check(&annotated, assignment);
        assert_eq!(
            (a.status, a.completeness.complete, a.violations.len()),
            (b.status, b.completeness.complete, b.violations.len())
        );
    }
}

#[test]
fn computed_domains_supply_neither_constraints_nor_singleton_seeds() {
    let source = r#"
        var 1..1: convenience ::computed_domain;
        var 2..2: logical = convenience;
        var 0..0: unseeded ::computed_domain;
        var 0..0: fixed ::computed_domain = 7;
        var set of {}: tools ::computed_domain;
        array[1..2] of var 0..0: both ::computed_domain = [convenience,unseeded];
        constraint set_in(4,tools);
        solve minimize fixed;
    "#;
    let model = prepare("computed.fzn", source).unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    let missing = check(&model, "");
    assert_eq!(missing.status, Status::Incomplete);
    assert!(missing.violations.is_empty());
    assert!(
        missing
            .completeness
            .unresolved_variables
            .iter()
            .any(|variable| variable.name == "unseeded")
    );
    assert!(
        !missing
            .completeness
            .unresolved_variables
            .iter()
            .any(|variable| variable.name == "convenience")
    );
    for assignment in [
        "",
        "unseeded=3;tools={4};",
        "convenience=9;unseeded=3;tools={4};",
        "fixed=8;",
    ] {
        assert_eq!(
            serde_json::to_value(check(&model, assignment)).unwrap(),
            serde_json::to_value(check(&loaded, assignment)).unwrap()
        );
    }
    let valid = check(&model, "unseeded=3;tools={4};");
    assert_eq!(valid.status, Status::Valid);
    assert_eq!(valid.objective.unwrap().argument.value, Value::Integer(7));
    let invalid = check(&model, "convenience=9;unseeded=3;tools={4};");
    assert_eq!(invalid.violations.len(), 1);
    let CheckRef::Domain { variable, .. } = &invalid.violations[0].check else {
        panic!("domain expected")
    };
    assert_eq!(variable.name, "logical");
    assert_eq!(check(&model, "fixed=8;").violations.len(), 1);
    assert_eq!(
        model.check(&Assignment::default()).unwrap().status,
        Status::Incomplete
    );
}

#[test]
fn malformed_text_types_references_metadata_and_hooks_receive_locations() {
    for source in [
        "solve satisfy;var int:x;",
        "var int:x;int:p=1;solve satisfy;",
        "var int:x;constraint int_lin_le([1],[x],0);var int:y;solve satisfy;",
        "var int:x=y;var int:y;solve satisfy;",
        "int:p=1;int:q=p;solve satisfy;",
        "int:p::mzn_path(1)=1;solve satisfy;",
        "array[1..1] of int:p=[1];array[1..1] of int:q=p;solve satisfy;",
        "var int:x::computed_domain(1);solve satisfy;",
        "var int:x::output_only;solve satisfy;",
        "var int:x::mzn_output_section(\"section\",false);solve satisfy;",
        "var int:x::checker_value(\"a\",[])::checker_value(\"b\",[]);solve satisfy;",
        "var int:x;solve::warm_start([x],[true]) satisfy;",
        "var int:x;solve::warm_start_array([int_search([x],first_fail,indomain_min,complete)]) satisfy;",
        "var int:x;solve::int_search([x],indomain_min,first_fail,complete) satisfy;",
        "var int:x;solve::seq_search([int_search([missing],first_fail,indomain_min,complete)]) satisfy;",
        "var int:x;solve::int_search([x],mystery,indomain_min,complete) satisfy;",
        "var int:x;solve::restart_geometric(1.0e999,1) satisfy;",
        "var int:x;solve::restart_geometric(true,1) satisfy;",
        "var int:x;solve::restart_constant(x) satisfy;",
        "var int:x;solve::on_restart(x) satisfy;",
        "var int:x;constraint int_lin_le([1],[x],0)::int_search([x],first_fail,indomain_min,complete);solve satisfy;",
        "array[1..2] of var int:a::output_array([{1,3},1..2])=[1,2];solve satisfy;",
        "array[1..1] of var int:a=[1];var int:x=a[0];solve satisfy;",
        "array[1..1] of var int:a=[1];var int:x=a[1+0];solve satisfy;",
        "array[0..0] of int:a=[1];solve satisfy;",
        "array[var 1..1] of int:a=[1];solve satisfy;",
        "var int:x=+1;solve satisfy;",
        "par int:p=1;solve satisfy;",
        "predicate int_le_reif(var int:a,var bool:b,var bool:c);solve satisfy;",
        "predicate int_lin_eq(array[int] of int:a,array[int] of var int:b,int:c)::promise_total;solve satisfy;",
        "predicate set_card(var set of int:a,var int:b)=true;solve satisfy;",
        "predicate set_card(var set of int:a,var int:b::output_var);solve satisfy;",
        "predicate set_card(var set of int:a,var int:b);predicate set_card(var set of int:c,var int:d);solve satisfy;",
        "predicate mystery(var int:x);solve satisfy;",
        "predicate int_lin_le(array[1..2] of int:a,array[int] of var int:b,int:c);constraint int_lin_le([1],[0],0);solve satisfy;",
        "predicate int_lin_le(array[int] of int:a,array[int] of var int:b,int:c);var int:x;constraint int_lin_le([1],[x],x);solve satisfy;",
        "constraint fzn_disjunctive_strict([],[]);solve satisfy;",
        "constraint blackbox(1);solve satisfy;",
    ] {
        let errors = prepare("bad.fzn", source).unwrap_err();
        assert!(errors[0].location.is_some(), "{source}");
    }
    for assignment in [
        "x=0x8000000000000000;",
        "x=-0x8000000000000001;",
        "x=0o8;",
        "x=+1;",
        "x=0X10;",
    ] {
        assert!(
            parse_assignment("bad.dzn", assignment).is_err(),
            "{assignment}"
        );
    }
}

#[test]
fn float_metadata_checks_types_without_executing_search() {
    for precision in ["0.0", "-0.5", "1e-9"] {
        let source = format!(
            r#"
            predicate float_eq(var float:left,var float:right);
            predicate float_in(var float:value,float:lower,float:upper);
            array[1..2] of float:starts=[-0.0,1e-320];
            var -1e1..2.5:base;
            var float:alias=base;
            array[1..2] of var float:values=[base,alias];
            array[1..2] of var float:copies=values;
            constraint float_eq(base,alias);
            constraint float_in(base,-10.0,2.5);
            solve::seq_search([float_search(copies,{precision},first_fail,indomain_split,complete),warm_start(copies,starts)]) minimize base;
        "#
        );
        assert!(prepare("metadata.fzn", &source).is_ok(), "{source}");
    }
    for annotation in [
        "float_search([x],1,input_order,indomain_split,complete)",
        "float_search([x],1e999,input_order,indomain_split,complete)",
        "float_search([x],1e-9,input_order,indomain_min,complete)",
        "warm_start([x],[1])",
    ] {
        assert!(
            prepare(
                "bad.fzn",
                &format!("var float:x;solve::{annotation} satisfy;")
            )
            .is_err()
        );
    }
}

#[test]
fn compiler_definition_hints_and_copied_metadata_keep_their_roles() {
    let source = r#"
        var int: x ::is_defined_var;
        var 0..1: y;
        var int: alias = y;
        var bool: standalone ::is_defined_var;
        var int: low;
        var int: high;
        constraint sort([1,2],[low,high])::defines_var([low,high]);
        constraint int_plus(x,2,y)::defines_var(alias)::is_defined_var::output_var
            ::checker_constraint("offset",[2])::checker_value("y",[1]);
        constraint int_le(y,1)::checker_constraint("limit",[2]);
        constraint set_in(y,1..4::mzn_path("range"));
        constraint set_subset({2,4}::mzn_path(7),1..5);
        solve minimize y;
    "#;
    let model = prepare("copied.fzn", source).unwrap();
    let loaded = PreparedModel::from_json(&model.to_json().unwrap()).unwrap();
    for candidate in [
        "x=-1;standalone=true;",
        "x=0;standalone=true;",
        "x=0;y=0;standalone=true;",
        "",
    ] {
        let report = check(&model, candidate);
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::to_value(check(&loaded, candidate)).unwrap()
        );
    }
    let valid = check(&model, "x=-1;standalone=true;");
    assert_eq!(valid.status, Status::Valid);
    assert_eq!(valid.objective.unwrap().argument.value, Value::Integer(1));
    let invalid = check(&model, "x=0;standalone=true;");
    assert!(invalid.completeness.complete);
    assert_eq!(invalid.violations.len(), 2);
    let conflict = check(&model, "x=0;y=0;standalone=true;");
    assert_eq!(
        conflict.objective.as_ref().unwrap().argument.value,
        Value::Integer(0)
    );
    let json = serde_json::to_value(&conflict).unwrap();
    let metadata = &json["violations"][0]["check"]["constraint"]["metadata"];
    assert_eq!(metadata["application"]["id"], "offset");
    assert_eq!(metadata["value_application"]["id"], "y");
    let missing = check(&model, "");
    assert_eq!(missing.status, Status::Incomplete);
    let json = serde_json::to_value(&missing).unwrap().to_string();
    assert!(json.contains("range"));
    for annotation in [
        "mzn_path(true)",
        r#"mzn_path("x",1)"#,
        "mystery",
        "output_var",
    ] {
        assert!(
            prepare(
                "bad.fzn",
                &format!("constraint set_in(1,{{1}}::{annotation});solve satisfy;")
            )
            .is_err()
        );
    }
    for annotation in [
        "is_defined_var(1)",
        "mzn_path(1::mystery)",
        "output_var(1)",
        "checker_value(1,[])",
        r#"checker_value("a",[])::checker_value("b",[])"#,
        r#"checker_constraint("a",[])::checker_constraint("b",[])"#,
    ] {
        assert!(
            prepare(
                "bad.fzn",
                &format!("constraint int_eq(1,1)::{annotation};solve satisfy;")
            )
            .is_err()
        );
    }
}
