use std::path::PathBuf;
use zincite_lint::*;

const CORE: &str = concat!(
    "function int: '+'(int:a,int:b); function int: '-'(int:a,int:b); function set of int: '..'(int:a,int:b);\n",
    "function bool: '<'(int:a,int:b); function bool: 'in'(int:a,set of int:b);\n",
    "function var bool: '='(any $T:a,any $T:b); function var bool: '->'(var bool:a,var bool:b); function var bool: '\\/'(var bool:a,var bool:b);\n",
    "function var bool: forall(array[int] of var bool:a); function set of $I: index_set(array[$I] of $T:a);\n",
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext, AnalysisResult) {
    let dir = std::env::temp_dir().join(format!(
        "zincite-index-mismatch-{name}-{}",
        std::process::id()
    ));
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
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("index-set-mismatch").unwrap(),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    (dir, context, result)
}
fn spelling<'a>(context: &'a ModelContext, finding: &FileFinding) -> &'a str {
    let file = context
        .files
        .iter()
        .find(|f| f.path == finding.location.path)
        .unwrap();
    file.parsed.source()[finding.location.range.start - file.byte_offset
        ..finding.location.range.end - file.byte_offset]
        .trim()
}
#[test]
fn ranges_offsets_and_dimensions_report_actual_membership_not_length() {
    let source = concat!(
        "array[1..3] of int:a; array[0..2] of int:z; array[1..3,0..2] of int:grid;\n",
        "constraint forall(i in 0..2)(a[i]=0); constraint forall(j in 1..3)(a[j+1]=0);\n",
        "constraint forall(k in 0..2)(a[k+1]=0); constraint forall(q in 0..2)(z[q]=0);\n",
        "int:outside=a[4]; array[0..2] of int:bad_slice=a[0..2]; array[1..2] of int:valid_subset=a[1..2];\n",
        "constraint forall(r in 1..3,c in 1..3)(grid[r,c]=0);\n",
        "array[2..2] of int:center; constraint forall(s in {1,3})(center[s]=0); constraint forall(t in {1,3})(a[t]=0); solve satisfy;\n",
    );
    let (dir, context, result) = model("spaces", source);
    assert_eq!(result.status(), 1);
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Completed),
        "{result:?}"
    );
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        [
            "a[i]",
            "a[j+1]",
            "a[4]",
            "a[0..2]",
            "grid[r,c]",
            "center[s]"
        ]
    );
    for f in &result.findings {
        assert_eq!(f.rule, Rule::IndexSetMismatch);
        assert!(
            f.message.contains("declared at") && f.message.contains("root.mzn:"),
            "{}",
            f.message
        );
        assert!(!f.message.contains("runtime error"));
    }
    assert!(
        result.findings[0]
            .message
            .contains("some candidate indices")
    );
    assert!(result.findings[1].message.contains("2..4"));
    assert!(result.findings[2].message.contains("wholly outside"));
    assert!(result.findings[3].message.contains("0..2"));
    assert!(
        result.findings[5].message.contains("{1, 3}")
            && result.findings[5].message.contains("wholly outside")
    );
    assert!(
        result.findings[4]
            .message
            .contains("array 'grid' dimension 2 (0..2")
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn local_guards_keep_safe_neighbors_and_unknown_restrictions_quiet() {
    let source = concat!(
        "array[1..3] of int:a; bool:keep;\n",
        "constraint forall(i in 1..3)(a[i+1]=0);\n",
        "constraint forall(j in 1..3)(if j<3 then a[j+1]=0 else true endif);\n",
        "constraint forall(k in 1..3 where k<3)(a[k+1]=0);\n",
        "constraint forall(q in 1..3)(if q+1 in index_set(a) then a[q+1]=0 else true endif);\n",
        "constraint forall(e in 1..0)(a[e+1]=0); int:inactive=if false then a[9] else 0 endif;\n",
        "constraint forall(u in 1..3 where keep)(a[u+1]=0);\n",
        "constraint forall(v in 1..3)(if keep then a[v+1]=0 else true endif);\n",
        "constraint forall(w in 1..3)(w<3 -> a[w+1]=0); bool:relational=a[4]=0; solve satisfy;\n",
    );
    let (dir, context, result) = model("guards", source);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        ["a[i+1]", "a[w+1]", "a[4]"]
    );
    assert!(
        result
            .findings
            .iter()
            .all(|f| !f.message.contains("inconsistent") && !f.message.contains("runtime error"))
    );
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let defs = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let numeric = resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &defs);
    let guarded = resolve_guarded_facts(&context, &bindings, &calls, &inst, &domains, &numeric);
    let file = context
        .files
        .iter()
        .position(|f| f.path == dir.join("root.mzn"))
        .unwrap();
    let protected = guarded
        .obligations
        .iter()
        .find(|o| o.file == file && &source[o.operation.range.clone()] == "a[j+1]")
        .unwrap();
    assert_eq!(protected.invariant, GuardedOutcome::Unknown);
    assert_eq!(protected.outcome, GuardedOutcome::Proven);
    assert!(
        matches!(&protected.kind,GuardObligationKind::Index{selection:Some(s),..} if s.exact && matches!(s.domain,Domain::Range{lower:NumericBound::Integer(2),upper:NumericBound::Integer(3)}))
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn enum_identity_unknown_data_and_unsupported_slices_have_distinct_outcomes() {
    let source = concat!(
        "enum X={A,B,C}; enum Y={D,E,F}; array[X] of int:a; array[0..2] of int:z;\n",
        "constraint forall(x in X)(a[x]=0); constraint forall(y in Y)(a[y]=0); constraint forall(q in 0..2)(z[q]=0);\n",
        "X:good; Y:other; int:valid_scalar=a[good]; int:invalid_scalar=a[other];\n",
        "int:N=3; int:K=3; set of int:P={1,2,3}; set of int:Q={1,2,3}; array[1..N] of int:b; array[P] of int:c;\n",
        "constraint forall(n in 1..K)(b[n]=0); constraint forall(p in Q)(c[p]=0); solve satisfy;\n",
    );
    let (dir, context, result) = model("identities", source);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        ["a[y]", "a[other]"]
    );
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.message.contains("incompatible enum index type"))
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    std::fs::remove_dir_all(dir).unwrap();
    let source = concat!(
        "enum X={A,B}; array[X,1..2] of int:grid;\n",
        "array[X,1..2] of int:whole=grid[..,..]; array[1..2] of int:row=grid[A,..]; array[X] of int:column=grid[..,1]; solve satisfy;\n",
    );
    let (dir, context, result) = model("full-axes", source);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert!(
        result.findings.is_empty() && result.limitations.is_empty(),
        "{result:?}"
    );
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let axis = bindings
        .declarations
        .iter()
        .find(|d| d.name == "X")
        .unwrap()
        .id;
    for (access, expected) in [
        ("grid[..,..]", vec![TypeKind::Enum(axis), TypeKind::Int]),
        ("grid[A,..]", vec![TypeKind::Int]),
        ("grid[..,1]", vec![TypeKind::Enum(axis)]),
    ] {
        let ty = &calls
            .expressions
            .iter()
            .find(|e| {
                e.location.path == dir.join("root.mzn")
                    && &source[e.location.range.clone()] == access
            })
            .unwrap()
            .ty;
        assert_eq!(ty.instantiation, Instantiation::Parameter);
        assert!(!ty.optional);
        let TypeKind::Array { indices, element } = &ty.kind else {
            panic!("{access}: {ty:?}");
        };
        assert_eq!(
            indices.iter().map(|i| i.kind.clone()).collect::<Vec<_>>(),
            expected
        );
        assert!(
            indices
                .iter()
                .all(|i| i.instantiation == Instantiation::Parameter && !i.optional)
        );
        assert_eq!(element.kind, TypeKind::Int);
        assert_eq!(element.instantiation, Instantiation::Parameter);
        assert!(!element.optional);
    }
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, result) = model(
        "decision-slice",
        "enum X={A,B}; array[X,1..2] of int:grid; var int:k; array[X] of var int:slice=grid[..,k]; solve satisfy;",
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(result.findings.is_empty());
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("type is unavailable")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, result) = model(
        "unsupported",
        "array[1..2,1..2] of int:grid; array[int,int] of int:slice=grid[..,1..2]; solve satisfy;",
    );
    assert_eq!(result.status(), 0);
    assert!(result.findings.is_empty());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.location.path == dir.join("root.mzn") && l.message.contains("sliced")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
