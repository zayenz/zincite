use std::path::PathBuf;
use zincite_lint::*;
const CORE: &str = concat!(
    "function set of int: '..'(int:a,int:b); function int: 'div'(int:a,int:b); function bool: '>'(int:a,int:b);\n",
    "function int: sum(array[int] of int:a); function int: product(array[int] of int:a);\n",
    "function bool: forall(array[int] of bool:a); function bool: exists(array[int] of bool:a);\n",
    "annotation mark(int:x);\n",
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext) {
    let dir = std::env::temp_dir().join(format!(
        "zincite-unused-generator-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), CORE).unwrap();
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..ModelOptions::default()
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    (dir, context)
}

#[test]
fn full_chain_uses_and_repetition_advice_preserve_declaration_identity() {
    let source = concat!(
        "constraint :: \"Repeated check\" forall(repeat in 1..3)(true);\n",
        "int:total=sum(term in 1..3)(2); int:multiplied=product(factor in 1..2)(3);\n",
        "array[int] of int:later_domain=[later | source in 1..2,later in 1..source];\n",
        "array[int] of int:later_filter=[target | choice in 1..2,target in 1..2 where choice>0];\n",
        "array[int] of int:annotation_use=[1::mark(ann_index) | ann_index in 1..2];\n",
        "array[int] of int:shadowed=[let {int:shadow=1;} in shadow | shadow in 1..2];\n",
        "array[int] of int:used=[actual | actual in 1..2];\n",
        "constraint forall(_ in 1..2)(true); constraint forall(_ignored in 1..2)(true);\n",
        "% zincite-lint: ignore unused-generator-binding\n",
        "array[int] of int:suppressed=[1 | muted in 1..2]; solve satisfy;\n",
    );
    let (dir, context) = model("chain", source);
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("unused-generator-binding").unwrap(),
    );
    assert!(result.errors.is_empty(), "{result:?}");
    let names: Vec<_> = result
        .findings
        .iter()
        .map(|f| &source[f.location.range.clone()])
        .collect();
    assert_eq!(names, ["repeat", "term", "factor", "shadow"]);
    assert_eq!(result.status(), 1);
    assert!(
        result.findings[0].message.contains("repeat 3 times")
            && result.findings[0].message.contains("idempotent")
    );
    assert!(
        result.findings[1].message.contains("repeat 3 times")
            && result.findings[1]
                .message
                .contains("does not make iterations removable")
    );
    assert!(result.findings[2].message.contains("terms or factors"));
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.message.contains("definedness requirement"))
    );
    assert!(
        analyze_model(&context, &LintOptions::default())
            .findings
            .iter()
            .all(|f| f.rule != Rule::UnusedGeneratorBinding)
    );
    assert!(Rule::UnusedGeneratorBinding.is_available());
    assert_eq!(
        Rule::UnusedGeneratorBinding.metadata().family,
        RuleFamily::Suspicious
    );
    assert_eq!(
        Rule::UnusedGeneratorBinding.metadata().fix_support,
        FixSupport::Sometimes
    );
    assert!(
        LintOptions::from_selection("family:suspicious")
            .unwrap()
            .rules
            .contains(&Rule::UnusedGeneratorBinding)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unused_name_facts_retain_unknown_refuted_and_unsupported_evaluation_safety() {
    let source = concat!(
        "int:d; function int:opaque(int:x);\n",
        "int:unknown=sum(unknown_index in 1..2)(1 div d);\n",
        "int:failed=sum(failed_index in 1..2)(1 div 0);\n",
        "int:unsupported=sum(opaque_index in 1..2)(opaque(1));\n",
        "int:known=sum(known_index in 1..2)(2);\n",
        "array[int] of int:unknown_values=[1 div d | unknown_array in 1..2];\n",
        "array[int] of int:failed_values=[1 div 0 | failed_array in 1..2];\n",
        "array[int] of int:opaque_values=[opaque(1) | opaque_array in 1..2]; solve satisfy;\n",
    );
    let (dir, context) = model("safety", source);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let numeric = resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
    let optional = resolve_optional_facts(
        &context,
        &bindings,
        &calls,
        &inst,
        &domains,
        &numeric,
        &definitions,
    );
    let guarded = resolve_guarded_facts_with_options(
        &context, &bindings, &calls, &inst, &domains, &numeric, &optional,
    );
    let iteration = resolve_iteration_facts(
        &context, &bindings, &calls, &inst, &domains, &numeric, &optional, &guarded,
    );
    let usage = resolve_generator_binding_usage(&bindings, &guarded, &iteration);
    assert_eq!(usage.len(), 7);
    assert!(usage[3].anonymization_eligible);
    assert!(
        usage[..3]
            .iter()
            .all(|u| !u.used && u.usage_complete && !u.anonymization_eligible)
    );
    assert!(
        usage[0]
            .obligations
            .iter()
            .any(|o| o.outcome == GuardedOutcome::Unknown)
    );
    assert!(
        usage[1]
            .obligations
            .iter()
            .any(|o| o.outcome == GuardedOutcome::Refuted)
    );
    assert!(
        usage[2]
            .evaluation_definedness
            .iter()
            .any(|(_, d)| matches!(d, GuardedOutcome::Unsupported(_)))
    );
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("unused-generator-binding").unwrap(),
    );
    assert_eq!(result.findings.len(), 7, "{result:?}");
    assert!(
        result.findings.iter().all(|finding| finding.fix.is_none()),
        "{result:?}"
    );
    assert!(result.findings[0].message.contains("unknown"));
    assert!(
        result.findings[1]
            .message
            .contains("without proving that the operation is evaluated")
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert_eq!(result.status(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn comprehension_fixes_preserve_anonymous_cartesian_terms_and_trivia() {
    let source = concat!(
        "\u{feff}% é retained\r\n",
        "int:total=sum([2 /* same repeated term */ | 'unused one', extra in 1..3]);\r\n",
        "set of int:values={2 | unused_set in 1..3};\r\n",
        "array[int] of int:indexed=[position:2 | unused_key in 1..1,position in 1..3];\r\n",
        "constraint :: \"kept annotation\" forall(shorthand in 1..3)(true);\r\n",
        "array[int] of int:kept=[used | used in 1..2];\r\n",
        "solve satisfy;\r\n",
    );
    let (dir, context) = model("fix", source);
    let options = LintOptions::from_selection("unused-generator-binding").unwrap();
    let result = analyze_model(&context, &options);
    let fixes: Vec<_> = result
        .findings
        .iter()
        .filter_map(|f| f.fix.as_ref())
        .collect();
    assert_eq!(fixes.len(), 4, "{result:?}");
    assert!(
        fixes
            .iter()
            .all(|f| f.safety == FixSafety::Safe && f.edits.len() == 1)
    );
    let root = &context.files[context.root_file.unwrap()];
    let prepared = prepare_fixes(
        &root.source_snapshot(),
        source,
        &result.findings,
        &FixOptions::default(),
    )
    .unwrap();
    let expected = source
        .replace("'unused one'", "_")
        .replace("extra in", "_ in")
        .replace("unused_set in", "_ in")
        .replace("unused_key in", "_ in");
    assert_eq!(prepared.candidate, expected);
    assert!(prepared.candidate.contains("/* same repeated term */"));
    assert!(prepared.candidate.contains(":: \"kept annotation\""));
    assert!(
        result
            .findings
            .iter()
            .find(|f| &source[f.location.range.clone()] == "shorthand")
            .unwrap()
            .fix
            .is_none()
    );
    replace_fixed_file(&root.source_snapshot(), &prepared.candidate).unwrap();
    let final_context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..ModelOptions::default()
        },
    );
    let remaining = analyze_model(&final_context, &options);
    assert!(
        remaining.findings.iter().all(|f| f.fix.is_none()),
        "{remaining:?}"
    );
    assert_eq!(
        prepare_fixes(
            &final_context.files[final_context.root_file.unwrap()].source_snapshot(),
            &expected,
            &remaining.findings,
            &FixOptions::default()
        )
        .unwrap()
        .candidate,
        expected
    );
    std::fs::remove_dir_all(dir).unwrap();
}
