use std::path::{Path, PathBuf};
use zincite_lint::{
    BindingResolution, DeclarationRole, Instantiation, LintOptions, ModelOptions, ReferenceKind,
    Rule, RuleOutcome, analyze_file, analyze_model, lint_with_options, load_model,
    resolve_bindings, write_analysis,
};
use zincite_syntax::{FileMode, parse, parse_with_mode};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn setup(name: &str) -> (PathBuf, ModelOptions) {
    let directory =
        std::env::temp_dir().join(format!("zincite-captures-{name}-{}", std::process::id()));
    let library = directory.join("library");
    write(
        &library.join("std/stdlib.mzn"),
        "var int: system_decision; function var int: system_capture() = system_decision;",
    );
    (
        directory,
        ModelOptions {
            include_dirs: Vec::new(),
            stdlib_dir: Some(library),
        },
    )
}

fn selected() -> LintOptions {
    LintOptions::from_selection("global-variable-in-function").unwrap()
}

#[test]
fn public_facts_and_capture_policy_share_declared_identity_and_scope_boundaries() {
    let (directory, options) = setup("scopes");
    let root = directory.join("root.mzn");
    let shared = directory.join("shared.mzn");
    let included = concat!(
        "\u{feff}% é\r\nvar 0..9: included;\r\n",
        "function var int: included_capture() = included;\r\n",
        "% zincite-lint: ignore global-variable-in-function\r\n",
        "function var int: suppressed_capture() = included;\r\n",
        "function var int: forward_capture() = global;\r\n",
    );
    let source = concat!(
        "include \"shared.mzn\";\n",
        "enum E = {A, B}; type D = var 0..9; annotation tag; type Alias :: tag = D; type P = par Alias;\n",
        "type Composite = record(Alias: value, int: count);\n",
        "Alias: global; int: symbolic; P: parameter_alias;\n",
        "array [1..2] of Alias: decisions; Composite: composite; tuple(int, var int): pair; var set of int: chosen;\n",
        "function var int: captures(int: local) = 'global' + decisions[1] + composite.value + pair.2 + card(chosen) + included + symbolic + parameter_alias + local;\n",
        "function var int: shadow(int: global) = let { int: nested = global; } in let { int: global = nested; } in global;\n",
        "function var int: initializer() = let { var int: global = global + 1; } in global;\n",
        "function var int: sequence() = sum([global + later | global in {global}, later in {global} where later > 0]);\n",
        "function var int: indices() = let { array [global in 1..2] of var 0..global: local_array = [global, global]; } in local_array[1];\n",
        "function var int: defaults(int: global = 1, var int: other = global) = global + other;\n",
        "function int: overloaded(int: value) = value; function float: overloaded(float: value) = value;\n",
        "function int: use_overload() = overloaded(1);\n",
        "type Outer = Later; type Later = var int;\n",
        "function var int: alias_scope(set of int: Later) = let { Outer: alias_local; } in alias_local;\n",
        "function int: local_domain(set of int: D) = let { D: domain_local = 1; } in domain_local;\n",
        "function int: named(int: global) = global; function int: labels() = named(global: 1);\n",
        "record(int: global): rec = (global: 1); function int: field() = rec.global;\n",
        "test enum_member() = A in E; predicate unnamed(var int) = true;\n",
        "predicate captures_predicate() = global > 0; test captures_test() = fix(global) > 0;\n",
        "solve satisfy;\n",
    );
    write(&root, source);
    write(&shared, included);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let facts = resolve_bindings(&context);
    let global = facts
        .declarations
        .iter()
        .find(|d| d.name == "global" && d.top_level && d.role == DeclarationRole::Value)
        .unwrap();
    for name in [
        "global",
        "decisions",
        "composite",
        "pair",
        "chosen",
        "included",
        "Alias",
        "Composite",
    ] {
        assert_eq!(
            facts
                .declarations
                .iter()
                .find(|d| d.name == name && d.top_level)
                .unwrap()
                .instantiation,
            Instantiation::Decision,
            "{name}"
        );
    }
    for name in ["symbolic", "parameter_alias", "P", "E", "A", "rec"] {
        assert_eq!(
            facts
                .declarations
                .iter()
                .find(|d| d.name == name && d.top_level)
                .unwrap()
                .instantiation,
            Instantiation::Parameter,
            "{name}"
        );
    }
    for (name, status) in [
        ("alias_local", Instantiation::Decision),
        ("domain_local", Instantiation::Parameter),
    ] {
        assert_eq!(
            facts
                .declarations
                .iter()
                .find(|d| d.name == name && d.role == DeclarationRole::Local)
                .unwrap()
                .instantiation,
            status
        );
    }
    let default_start = source.find("other = global").unwrap() + "other = ".len();
    let default = facts
        .references
        .iter()
        .find(|r| r.location.path == root && r.location.range.start == default_start)
        .unwrap();
    assert_eq!(default.resolution, BindingResolution::Resolved(global.id));
    assert!(default.callable.is_none());
    let body_start = source.find("= global + other").unwrap() + 2;
    let body = facts
        .references
        .iter()
        .find(|r| r.location.path == root && r.location.range.start == body_start)
        .unwrap();
    assert!(
        matches!(body.resolution, BindingResolution::Resolved(id) if id != global.id && facts.declarations[id.0].role == DeclarationRole::Parameter)
    );
    let quoted = facts
        .references
        .iter()
        .find(|r| r.location.path == root && &source[r.location.range.clone()] == "'global'")
        .unwrap();
    assert_eq!(quoted.resolution, BindingResolution::Resolved(global.id));
    let overload = facts
        .references
        .iter()
        .find(|r| r.kind == ReferenceKind::Callable && r.name == "overloaded")
        .unwrap();
    assert!(matches!(&overload.resolution, BindingResolution::Overloads(ids) if ids.len() == 2));
    for marker in ["global: 1", "global): rec", "rec.global"] {
        let start = source.find(marker).unwrap() + if marker == "rec.global" { 4 } else { 0 };
        assert!(
            !facts
                .references
                .iter()
                .any(|r| r.location.path == root && r.location.range.start == start)
        );
    }
    let source_global = source.find("{global}, later").unwrap() + 1;
    let later_global = source.find("later in {global}").unwrap() + "later in {".len();
    let first = facts
        .references
        .iter()
        .find(|r| r.location.path == root && r.location.range.start == source_global)
        .unwrap();
    let later = facts
        .references
        .iter()
        .find(|r| r.location.path == root && r.location.range.start == later_global)
        .unwrap();
    assert_eq!(first.resolution, BindingResolution::Resolved(global.id));
    assert!(
        matches!(later.resolution, BindingResolution::Resolved(id) if facts.declarations[id.0].role == DeclarationRole::Generator)
    );
    let index_start = source.find("0..global: local_array").unwrap() + 3;
    let index = facts
        .references
        .iter()
        .find(|r| r.location.path == root && r.location.range.start == index_start)
        .unwrap();
    assert!(
        matches!(index.resolution, BindingResolution::Resolved(id) if facts.declarations[id.0].role == DeclarationRole::Index)
    );

    let result = analyze_model(&context, &selected());
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?}",
        result.limitations
    );
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    let root_ranges: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.location.path == root)
        .map(|f| f.location.range.clone())
        .collect();
    let markers = [
        ("'global' + decisions", 0, "'global'"),
        ("decisions[1]", 0, "decisions"),
        ("composite.value", 0, "composite"),
        ("pair.2", 0, "pair"),
        ("card(chosen)", 5, "chosen"),
        ("included + symbolic", 0, "included"),
        ("= global + 1", 2, "global"),
        ("{global}, later", 1, "global"),
        ("[global, global]", 1, "global"),
        ("[global, global]", 9, "global"),
        ("captures_predicate() = global", 23, "global"),
        ("captures_test() = fix(global)", 22, "global"),
    ];
    let expected: Vec<_> = markers
        .iter()
        .map(|(marker, offset, name)| {
            let start = source.find(marker).unwrap() + offset;
            assert_eq!(&source[start..start + name.len()], *name);
            start..start + name.len()
        })
        .collect();
    assert_eq!(root_ranges, expected);
    let shared_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.location.path == shared)
        .collect();
    assert_eq!(shared_findings.len(), 2);
    assert_eq!(shared_findings[0].location.line, 3);
    assert_eq!(
        &included[shared_findings[0].location.range.clone()],
        "included"
    );
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.message.contains("as an argument") && !f.message.contains("faster"))
    );
    let system = load_model(
        options.stdlib_dir.as_ref().unwrap().join("std/stdlib.mzn"),
        &options,
    );
    assert_eq!(analyze_model(&system, &selected()).findings.len(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn incomplete_binding_facts_keep_proved_captures_and_report_limits_without_guesses() {
    let (directory, options) = setup("unknown");
    let root = directory.join("root.mzn");
    write(
        &root,
        concat!(
            "type CycleA = CycleB; type CycleB = CycleA; CycleA: unknown; MissingType: unsupported; unsupported: dependent;\n",
            "var int: known; var int: duplicate; int: 'duplicate';\n",
            "function var int: check() = known + unknown + unsupported + dependent + missing + duplicate; solve satisfy;",
        ),
    );
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty());
    let facts = resolve_bindings(&context);
    assert!(
        facts
            .declarations
            .iter()
            .filter(
                |d| ["unknown", "unsupported", "dependent", "CycleA", "CycleB"]
                    .contains(&d.name.as_str())
            )
            .all(|d| d.instantiation == Instantiation::Unknown)
    );
    assert!(
        facts
            .references
            .iter()
            .any(|r| r.name == "missing" && r.resolution == BindingResolution::Unresolved)
    );
    assert!(
        facts
            .references
            .iter()
            .any(|r| r.name == "duplicate"
                && matches!(r.resolution, BindingResolution::Ambiguous(_)))
    );
    let result = analyze_model(&context, &selected());
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.limitations.len(), 5);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let mut stderr = Vec::new();
    assert_eq!(write_analysis(&result, &mut stderr).unwrap(), 1);
    let stderr = String::from_utf8(stderr).unwrap();
    assert!(
        stderr.contains("analysis limitation")
            && stderr.contains("ambiguous")
            && stderr.contains("unresolved")
    );
    let missing_library = load_model(&root, &ModelOptions::default());
    let result = analyze_model(&missing_library, &selected());
    assert_eq!(result.findings.len(), 1);
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("builtin declarations"))
    );
    write(
        &root,
        "include \"missing.mzn\"; var int: known; function var int: check() = known;",
    );
    let failed = analyze_model(&load_model(&root, &options), &selected());
    assert_eq!(failed.status(), 2);
    assert_eq!(failed.findings.len(), 1);
    assert!(matches!(
        failed.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    write(
        &directory.join("independent.mzn"),
        "int: symbolic; function int: good() = symbolic;",
    );
    let independent = analyze_model(
        &load_model(directory.join("independent.mzn"), &options),
        &selected(),
    );
    assert_eq!(independent.status(), 0);
    assert_eq!(independent.rules[0].outcome, RuleOutcome::Completed);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn standalone_and_legacy_apis_distinguish_missing_context_from_data_applicability() {
    let options = LintOptions::from_selection("naming,global-variable-in-function").unwrap();
    let parsed = parse("var int: BadName; function var int: capture() = BadName;");
    let errors = lint_with_options(&parsed, &options).unwrap_err();
    assert!(errors[0].message.contains("ModelContext"));
    let result = analyze_file(&parsed, "buffer.mzn", 0, &options);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert!(matches!(
        result.rules[1].outcome,
        RuleOutcome::Limited { .. }
    ));
    let mut stderr = Vec::new();
    assert_eq!(write_analysis(&result, &mut stderr).unwrap(), 1);
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("requires a ModelContext")
    );
    let clean = analyze_file(&parse("int: symbolic;"), "buffer.mzn", 0, &selected());
    assert_eq!(clean.status(), 0);
    assert_eq!(clean.limitations.len(), 1);
    let data = analyze_file(
        &parse_with_mode("global=1;", FileMode::Data),
        "data.dzn",
        0,
        &options,
    );
    assert_eq!(data.status(), 0);
    assert!(matches!(
        data.rules[1].outcome,
        RuleOutcome::Inapplicable { .. }
    ));
    assert!(data.limitations.is_empty());
    assert!(Rule::GlobalVariableInFunction.is_available());
    assert_eq!(
        Rule::THESIS
            .iter()
            .filter(|rule| !rule.is_available())
            .count(),
        13
    );
}
