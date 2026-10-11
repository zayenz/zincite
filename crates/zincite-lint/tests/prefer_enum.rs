use std::path::PathBuf;
use zincite_lint::*;

const CORE: &str = concat!(
    "function set of int: '..'(int:a,int:b);\n",
    "function var bool: '='(var $T:a,var $T:b);\n",
    "function var bool: '!='(var $T:a,var $T:b);\n",
    "function var bool: 'in'(var $T:a,set of $T:b);\n",
    "function set of $T: 'union'(set of $T:a,set of $T:b);\n",
    "function set of $T: 'intersect'(set of $T:a,set of $T:b);\n",
    "function var bool: '<'(var int:a,var int:b);\n",
    "function var int: '+'(var int:a,var int:b);\n",
    "function var int: '*'(var int:a,var int:b);\n",
    "function int: card(set of $T:a);\n",
    "function var int: sum(array[$U] of var int:a);\n",
    "function int: sum(set of int:a);\n",
    "predicate forall(array[$U] of var bool:a);\n",
    "function var int: bool2int(var bool:a);\n",
    "function int: min(set of int:a);\n",
    "function float: int2float(int:a);\n",
    "function string: show(any $T:a);\n",
    "annotation no_output; annotation add_to_output; annotation output_only;\n",
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext) {
    let dir =
        std::env::temp_dir().join(format!("zincite-prefer-enum-{name}-{}", std::process::id()));
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
fn check(name: &str, source: &str) -> AnalysisResult {
    let (dir, context) = model(name, source);
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("prefer-enum").unwrap(),
    );
    std::fs::remove_dir_all(dir).unwrap();
    result
}

#[test]
fn identities_counts_and_indexed_payloads_keep_nominal_families() {
    let source = concat!(
        "set of int: People=10..12; set of int: Alias=People; type Person=Alias;\n",
        "var Person: p; var People:q=p; var int: copied=p;\n",
        "array[People] of int: costs=[10,20,30]; array[People] of var bool: chosen;\n",
        "function any $T: identity(any $T:x)=x;\n",
        "function var People: pass(var People:x)=x; function var People: pick(var People:x=p)=x;\n",
        "function int: payload(var People:x)=costs[x]+card(People); int:n=payload(p)+1;\n",
        "predicate same(any $T:x,any $T:y)=x=y;\n",
        "constraint copied=q; constraint pass(p)=identity(p); constraint same(x:p,y:q); constraint pick()=p;\n",
        "constraint forall(i in Alias)(chosen[i]=chosen[i]);\n",
        "constraint sum(i in People)(costs[i]+bool2int(chosen[i]))=2*card(People);\n",
        "constraint p in (People union Alias);\n",
        "array[People] of int: indexed=[i:costs[i]+1|i in People]; constraint indexed[p]=costs[p]+1;\n",
        "array[People] of int: forwarded=identity(costs); constraint forwarded[p]=costs[p];\n",
        "constraint let {int:p=0;} in p=0;\n",
        "set of int: Other={5,7}; var Other:r; constraint identity(r)=r;\n",
        "int:unrelated=missing(); set of int: Unused=1..4; set of int: Counts=1..4; int:n=2*card(Counts);\n",
        "int:iterated_count=sum(i in Counts)(1);\n",
        "solve satisfy; output [show(costs[q]+card(People))];\n",
    );
    let result = check("identities", source);
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{result:#?}"
    );
    assert_eq!(result.findings.len(), 2, "{result:#?}");
    for finding in &result.findings {
        assert!(finding.fix.is_none());
        assert!(["People", "Other"].contains(&&source[finding.location.range.clone()]));
        assert!(
            finding.message.contains("consider an enum")
                && finding.message.contains("compatibility")
        );
    }
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    let (dir, context) = model("mixed", source);
    let mixed = analyze_model(
        &context,
        &LintOptions::from_selection("prefer-enum,constant-variable").unwrap(),
    );
    assert_eq!(
        mixed
            .findings
            .iter()
            .filter(|f| f.rule == Rule::PreferEnum)
            .count(),
        2,
        "{mixed:#?}"
    );
    assert!(matches!(mixed.rules[0].outcome, RuleOutcome::Completed));
    std::fs::remove_dir_all(dir).unwrap();

    let cross = check(
        "nominal",
        "set of int: A=1..2; set of int: B=1..2; var A:a; var B:b; constraint a=b; solve satisfy; output [];",
    );
    assert!(
        cross.findings.is_empty() && cross.limitations.is_empty(),
        "{cross:#?}"
    );
}

#[test]
fn numeric_observations_and_output_withhold_advice() {
    let prefix = "set of int: People=10..12; var People:p; array[People] of var bool:chosen; ";
    for (name, extra) in [
        ("arithmetic", "constraint p+1=p; solve satisfy; output [];"),
        ("sentinel", "constraint p!=0; solve satisfy; output [];"),
        ("initializer", "var People:q=10; solve satisfy; output [];"),
        (
            "hidden",
            "function int: charge(int:x)=2*x; constraint sum(i in People)(charge(i))=66; solve satisfy; output [];",
        ),
        (
            "generic-hidden",
            "function any $T: charge(any $T:x)=x+1; constraint charge(p)=p; solve satisfy; output [];",
        ),
        (
            "concrete",
            "function var int: same(var int:x)=x; constraint same(p)=p; solve satisfy; output [];",
        ),
        (
            "result",
            "function var int: same(var People:x)=x; constraint same(p)=p; solve satisfy; output [];",
        ),
        ("ordering", "constraint p<p; solve satisfy; output [];"),
        (
            "extrema",
            "constraint min(People)=10; solve satisfy; output [];",
        ),
        (
            "conversion",
            "constraint int2float(p)=10.0; solve satisfy; output [];",
        ),
        (
            "aggregate",
            "constraint sum(People)=33; solve satisfy; output [];",
        ),
        (
            "numeric-domain",
            "constraint p=p; constraint forall(i in People)(let {var 1..i:x;} in x=x); solve satisfy; output [];",
        ),
        (
            "concrete-key",
            "array[int] of int: indexed=[i:1|i in People]; constraint p=p; solve satisfy; output [];",
        ),
        ("objective", "solve minimize p; output [];"),
        ("explicit-member", "solve satisfy; output [show(p)];"),
        ("explicit-axis", "solve satisfy; output [show(chosen)];"),
        ("default-member", "solve satisfy;"),
        ("default-axis", "var People:q=p; solve satisfy;"),
        (
            "shadowed-card",
            "function int: card(set of int:s)=sum(s); constraint card(People)=33; solve satisfy; output [];",
        ),
    ] {
        let result = check(name, &format!("{prefix}{extra}"));
        assert!(
            result.errors.is_empty() && result.findings.is_empty(),
            "{name}: {result:#?}"
        );
    }
    for (name, body) in [
        (
            "literal-result",
            "function var People: hidden()=1; var int:n=hidden()+1;",
        ),
        (
            "literal-actual",
            "function var People: pass(var People:x)=x; var int:n=pass(1)+1;",
        ),
    ] {
        let result = check(
            name,
            &format!(
                "set of int: People=1..3; var People:p; constraint p=p; {body} solve satisfy; output [];"
            ),
        );
        assert!(
            result.errors.is_empty() && result.findings.is_empty() && result.limitations.is_empty(),
            "{name}: {result:#?}"
        );
        assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    }
    for (name, source) in [
        (
            "hidden-output",
            "set of int: People=1..3; var People:p :: no_output; constraint p=p; solve satisfy;",
        ),
        (
            "independent-forced",
            "set of int: People=1..3; var People:p; var int:cost :: add_to_output; constraint p=p; solve satisfy;",
        ),
        (
            "initialized-output",
            "set of int: People=1..3; var People:p :: no_output; var People:q=p; constraint p=q; solve satisfy;",
        ),
    ] {
        let result = check(name, source);
        assert_eq!(result.findings.len(), 1, "{name}: {result:#?}");
        assert!(result.limitations.is_empty(), "{result:#?}");
    }
}

#[test]
fn affected_exclusions_keep_independent_findings_and_context_outcomes() {
    for (name, affected) in [
        (
            "recursive",
            "function var Bad: loop(var Bad:x)=loop(x); constraint loop(b)=b;",
        ),
        (
            "opaque",
            "function var Bad: opaque(var Bad:x); constraint opaque(b)=b;",
        ),
        ("unresolved", "constraint missing(b);"),
        (
            "unavailable-result",
            "function var Bad: hidden()=missing(); var int:n=hidden()+1;",
        ),
        (
            "unavailable-actual",
            "function var Bad: pass(var Bad:x)=x; var int:n=pass(missing())+1; int:origin=missing(); var int:m=pass(origin)+1;",
        ),
        (
            "opaque-result",
            "function var Bad: opaque(); constraint opaque()=b;",
        ),
        ("assignment", "int: alias; alias=b;"),
        ("storage", "array[1..2] of var Bad:members;"),
        ("record", "tuple(var Bad,int): pair;"),
        ("computed", "constraint forall(i in Bad union Bad)(i=i);"),
        (
            "computed-alias",
            "set of int: Alias=Bad union Bad; constraint forall(i in Alias)(i=i);",
        ),
        (
            "computed-call",
            "function set of $T: same_set(set of $T:s)=s; set of int: Alias=same_set(Bad); constraint forall(i in Alias)(i=i);",
        ),
        (
            "annotation",
            "annotation note(var int:x); var int:v :: note(b);",
        ),
    ] {
        let source = format!(
            "set of int: Good=1..2; var Good:g; constraint g=g; set of int: Bad=1..3; var Bad:b; {affected} solve satisfy; output [];"
        );
        let result = check(name, &source);
        assert_eq!(result.findings.len(), 1, "{name}: {result:#?}");
        assert_eq!(&source[result.findings[0].location.range.clone()], "Good");
        assert!(!result.limitations.is_empty(), "{name}: {result:#?}");
        assert!(
            result
                .limitations
                .iter()
                .all(|l| l.location.range.start < l.location.range.end
                    && l.message.starts_with("prefer-enum:"))
        );
        assert!(matches!(
            result.rules[0].outcome,
            RuleOutcome::Limited { .. }
        ));
    }
    let fragment = check(
        "fragment",
        "set of int: People=1..3; var People:p; constraint p=p;",
    );
    assert!(
        fragment.findings.is_empty()
            && matches!(fragment.rules[0].outcome, RuleOutcome::Inapplicable { .. })
    );
    let (dir, _) = model("dependency", "solve satisfy;");
    std::fs::write(
        dir.join("root.mzn"),
        "include \"missing.mzn\"; set of int: People=1..3; var People:p; solve satisfy; output [];",
    )
    .unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..ModelOptions::default()
        },
    );
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("prefer-enum").unwrap(),
    );
    assert_eq!(result.status(), 2);
    assert!(
        result.findings.is_empty()
            && matches!(result.rules[0].outcome, RuleOutcome::Limited { .. })
    );
    std::fs::remove_dir_all(dir).unwrap();
    let data = zincite_syntax::parse("People={1,2,3};");
    let result = analyze_file(
        &data,
        "data.dzn",
        0,
        &LintOptions::from_selection("prefer-enum").unwrap(),
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Inapplicable { .. }
    ));
    let multiple = check(
        "multiple",
        "set of int: People=1..3; var People:p; solve satisfy; solve satisfy; output [];",
    );
    assert!(
        multiple.findings.is_empty()
            && matches!(multiple.rules[0].outcome, RuleOutcome::Limited { .. })
    );
    assert!(!multiple.limitations.is_empty());
}

#[test]
fn selection_suppression_and_diagnostic_only_cli_preserve_bytes() {
    assert!(
        !Rule::DEFAULT.contains(&Rule::PreferEnum) && !Rule::THESIS.contains(&Rule::PreferEnum)
    );
    assert_eq!(Rule::PreferEnum.metadata().fix_support, FixSupport::None);
    for selection in ["prefer-enum", "family:style", "all"] {
        assert!(
            LintOptions::from_selection(selection)
                .unwrap()
                .rules
                .contains(&Rule::PreferEnum)
        );
    }
    let source =
        "set of int: People=1..3; var People:p; constraint p=p; solve satisfy; output [];\r\n";
    let (dir, context) = model("cli", source);
    let options = LintOptions::from_selection("prefer-enum").unwrap();
    assert_eq!(analyze_model(&context, &options).findings.len(), 1);
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
            .args([
                "--isolated",
                "--stdlib-dir",
                dir.join("library").to_str().unwrap(),
                "--rules",
                "prefer-enum",
            ])
            .args(args)
            .arg(dir.join("root.mzn"))
            .output()
            .unwrap()
    };
    for args in [&[][..], &["--fix"][..], &["--diff"][..]] {
        let output = run(args);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("[prefer-enum]"));
        assert!(output.stdout.is_empty());
        assert_eq!(
            std::fs::read(dir.join("root.mzn")).unwrap(),
            source.as_bytes()
        );
    }
    let suppressed = format!("% zincite-lint: ignore prefer-enum\n{source}");
    std::fs::write(dir.join("root.mzn"), &suppressed).unwrap();
    assert_eq!(run(&[]).status.code(), Some(0));
    std::fs::remove_dir_all(dir).unwrap();
}
