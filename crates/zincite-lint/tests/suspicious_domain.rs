use std::path::PathBuf;
use zincite_lint::*;

const CORE: &str = concat!(
    "function var bool: '='(var int:a,var int:b); function var bool: '->'(var bool:a,var bool:b);\n",
    "function int: '+'(int:a,int:b); function int: '-'(int:a,int:b); function int: '-'(int:a); function int: '*'(int:a,int:b);\n",
    "function var int: '+'(var int:a,var int:b); function var int: '-'(var int:a,var int:b); function var int: '*'(var int:a,var int:b);\n"
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext) {
    let dir = std::env::temp_dir().join(format!("zincite-contract-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), CORE).unwrap();
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            include_dirs: vec![],
            stdlib_dir: Some(dir.join("library")),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    (dir, context)
}
fn facts(context: &ModelContext) -> (BindingFacts, NumericFacts, Vec<NumericContract>) {
    let b = resolve_bindings(context);
    let c = resolve_callables(context, &b);
    let i = resolve_instantiations(context, &b, &c);
    let d = resolve_domains(context, &b);
    let defs = resolve_definitions(context, &b, &c, &i, &d);
    let n = resolve_numeric_facts(context, &b, &c, &i, &d, &defs);
    let contracts = resolve_numeric_contracts(context, &b, &defs, &n);
    (b, n, contracts)
}
fn analyze(context: &ModelContext, selection: &str) -> AnalysisResult {
    let r = analyze_model(context, &LintOptions::from_selection(selection).unwrap());
    assert!(r.errors.is_empty(), "{r:?}");
    r
}
#[test]
fn complete_scalar_contracts_keep_actual_members_locals_and_parameter_defaults_distinct() {
    let source = concat!(
        "\u{feff}% π\r\nvar 0..1:outside=2+3; var {1,3}:hole=2; var 1..3:ok=2; 0..1:replaceable=5;\r\n",
        "var 0..1:equality; constraint equality=2+3;\r\n",
        "var int:wrapped=let {0..1:local_bad=5; int:alias=local_bad; 0..1:local_chain=alias+1;} in local_chain; solve satisfy;\r\n",
    );
    let (dir, context) = model("exact", source);
    let (b, n, c) = facts(&context);
    let conflicts: Vec<_> = c
        .iter()
        .filter(|c| c.relation == NumericContractRelation::Contradiction)
        .map(|c| b.declarations[c.destination.0].name.as_str())
        .collect();
    assert_eq!(
        conflicts,
        ["outside", "hole", "local_bad", "local_chain", "equality"]
    );
    assert!(
        !c.iter()
            .any(|c| b.declarations[c.destination.0].name == "replaceable")
    );
    for name in ["local_bad", "local_chain", "replaceable"] {
        let id = b.declarations.iter().find(|d| d.name == name).unwrap().id;
        let rhs = n.definitions.iter().find(|d| d.target == id).unwrap();
        let required = n.declarations.iter().find(|d| d.declaration == id).unwrap();
        assert_eq!(rhs.parameter_default, name == "replaceable");
        assert_eq!(
            rhs.domain_relation(required),
            if name == "replaceable" {
                NumericDomainRelation::Inconclusive
            } else {
                NumericDomainRelation::ExactOutside
            }
        );
    }
    let result = analyze(&context, "suspicious-domain");
    assert_eq!(result.findings.len(), 5, "{result:?}");
    assert!(result.limitations.is_empty(), "{result:?}");
    assert_eq!(result.status(), 1);
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.message.contains("proved domain contradiction")
                && f.message.contains("complete RHS")
                && f.message.contains("declared at")
                && f.location.range.start >= 3)
    );
    assert!(
        result
            .findings
            .iter()
            .any(|f| f.message.contains("'hole' requires {1, 3}"))
    );
    assert_eq!(context.files[0].byte_offset, 3);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn intermediate_destination_ranges_distinguish_disjoint_hulls_and_complete_consumption() {
    let source = concat!(
        "var 2..4:input; var 0..1:disjoint=(input+2)*2; var 0..3:overlap=input+0;\n",
        "var {1,3}:sparse; var {-2,0,2}:sparse_overlap=sparse-sparse;\n",
        "var -100..100:broad=(input+2)*2; var 0..1:cancelled=(1000+1000)-2000;\n",
        "var int:large=9223372036854775807; var int:unbounded; solve satisfy;\n"
    );
    let (dir, context) = model("intermediate", source);
    let (b, _, contracts) = facts(&context);
    let named = |name: &str| {
        contracts
            .iter()
            .find(|c| b.declarations[c.destination.0].name == name)
            .unwrap()
    };
    assert_eq!(
        named("disjoint").derived,
        NumericOutcome::Interval {
            lower: 8,
            upper: 12
        }
    );
    assert_eq!(
        named("disjoint").relation,
        NumericContractRelation::Contradiction
    );
    assert_eq!(
        named("overlap").relation,
        NumericContractRelation::PossibleConflict
    );
    assert_eq!(
        named("sparse_overlap").relation,
        NumericContractRelation::PossibleConflict
    );
    assert_eq!(named("broad").relation, NumericContractRelation::Compatible);
    assert_eq!(named("cancelled").derived, NumericOutcome::Exact(0));
    assert_eq!(
        named("cancelled").relation,
        NumericContractRelation::Compatible
    );
    let result = analyze(&context, "suspicious-domain");
    assert_eq!(result.findings.len(), 3, "{result:?}");
    assert!(result.limitations.is_empty(), "{result:?}");
    assert_eq!(
        result
            .findings
            .iter()
            .filter(|f| f.message.contains("conservative possible conflict"))
            .count(),
        2
    );
    assert!(
        result
            .findings
            .iter()
            .filter(|f| f.message.contains("conservative possible conflict"))
            .all(|f| f.message.contains("no actual failing value") && f.message.contains("hull"))
    );
    assert!(
        !result
            .findings
            .iter()
            .any(|f| f.message.contains("cancelled")
                || f.message.contains("large")
                || f.message.contains("unbounded"))
    );
    let joint = analyze(&context, "suspicious-domain,unbounded-variable");
    assert_eq!(
        joint
            .findings
            .iter()
            .filter(|f| f.rule == Rule::SuspiciousDomain)
            .count(),
        3
    );
    assert_eq!(
        joint
            .findings
            .iter()
            .filter(|f| f.rule == Rule::UnboundedVariable)
            .count(),
        1
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn unknown_symbolic_unsafe_and_overflow_candidates_are_located_limits_only() {
    let source = concat!(
        "int:N; var 0..3:symbol=N+1; var 1..N:required=2; var 0..1:overflow=9223372036854775807+1;\n",
        "int:u; var 0..1:unknown; constraint unknown=u; var 0..(9223372036854775807+1):bad_required=N;\n",
        "var int:cycle_a; var int:cycle_b; constraint cycle_a=cycle_b; constraint cycle_b=cycle_a; var 0..1:cycle=cycle_a;\n",
        "bool:choice; var 0..1:conditional; constraint choice -> conditional=2;\n",
        "set of int:S={1,3}; set of int:T={1,3}; var S:set_default=2; var T:other_set=2;\n",
        "annotation semantic; var 0..1:annotated; constraint annotated=2 :: semantic; solve satisfy;\n"
    );
    let (dir, context) = model("unknown", source);
    let (bindings, _, contracts) = facts(&context);
    assert!(
        contracts.iter().any(
            |c| bindings.declarations[c.destination.0].name == "bad_required"
                && matches!(c.relation, NumericContractRelation::Unsupported(_))
        ),
        "{contracts:?}"
    );
    assert!(
        contracts.iter().all(|c| matches!(
            c.relation,
            NumericContractRelation::Unknown(_) | NumericContractRelation::Unsupported(_)
        )),
        "{contracts:?}"
    );
    let result = analyze(&context, "suspicious-domain");
    assert!(result.findings.is_empty(), "{result:?}");
    assert_eq!(result.status(), 0);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("overflow"))
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("parameters") || l.message.contains("parameter set"))
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("conditional"))
    );
    assert!(
        result
            .limitations
            .iter()
            .all(|l| l.message.starts_with("suspicious-domain:")
                && l.message.contains("RHS")
                && l.location.path == dir.join("root.mzn")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
