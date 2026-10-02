use zincite_lint::{
    BindingResolution, LintOptions, ModelOptions, Rule, RuleOutcome, analyze_model, load_model,
    resolve_bindings, settings::LintSettings,
};

#[test]
fn lexical_shadowing_uses_enclosing_identity_and_keeps_scope_exceptions_quiet() {
    let dir = std::env::temp_dir().join(format!("zincite-shadowing-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    let root = dir.join("root.mzn");
    let shared = dir.join("shared.mzn");
    let outer = "\u{feff}% é\r\nvar 0..9: captured;\r\n";
    std::fs::write(&shared, outer).unwrap();
    let source = concat!(
        "include \"shared.mzn\";\n",
        "function var int: captured_use() = captured;\n",
        "function int: parameter(int: captured) = captured;\n",
        "function int: local() = let { int: captured=1; } in captured;\n",
        "constraint forall(i in 1..2)(forall(i in 1..2)(i>0));\n",
        "constraint forall(reused in 1..2)(reused>0) /\\ forall(reused in 1..2)(reused>0);\n",
        "int: _unused=1; function int: intentional(int: _unused) = _unused;\n",
        "record(int: captured): fields=(captured: 1);\n",
        "function int: labels() = fields.captured;\n",
        "% zincite-lint: ignore suspicious-shadowing\n",
        "function int: suppressed(int: captured) = let { int: captured=1; } in captured;\n",
        "solve satisfy;\n",
    );
    std::fs::write(&root, source).unwrap();
    let model = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..ModelOptions::default()
    };
    let context = load_model(&root, &model);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let options = LintOptions::from_selection("suspicious-shadowing").unwrap();
    let result = analyze_model(&context, &options);
    assert_eq!(result.status(), 1);
    assert_eq!(result.findings.len(), 3, "{:?}", result.findings);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    let facts = resolve_bindings(&context);
    for finding in &result.findings {
        let declaration = facts
            .declarations
            .iter()
            .find(|d| d.location == finding.location)
            .unwrap();
        let BindingResolution::Resolved(id) = declaration.shadowed else {
            panic!("expected enclosing identity");
        };
        let outer_location = &facts.declarations[id.0].location;
        assert!(finding.message.contains(&format!(
            "{}:{}:{} bytes {}..{}",
            outer_location.path.display(),
            outer_location.line,
            outer_location.column,
            outer_location.range.start,
            outer_location.range.end
        )));
        assert_eq!(&source[finding.location.range.clone()], declaration.name);
    }
    assert!(
        result.findings[0]
            .message
            .contains(&format!("{}:2:11 bytes 19..27", shared.display()))
    );
    let settings = LintSettings::from_toml("[lint]\nselect=['suspicious-shadowing']\n[lint.options.suspicious-shadowing]\nignore-names=['captured']").unwrap();
    let excepted = analyze_model(&context, &settings.resolve().unwrap());
    assert_eq!(excepted.findings.len(), 1);
    assert_eq!(&source[excepted.findings[0].location.range.clone()], "i");
    assert!(Rule::SuspiciousShadowing.is_available());
    assert_eq!(Rule::DEFAULT.len(), 2);
    assert_eq!(Rule::THESIS.len(), 14);
    assert_eq!(std::fs::read_to_string(&shared).unwrap(), outer);
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ambiguous_outer_bindings_are_limited_without_inventing_a_shadowed_declaration() {
    let dir = std::env::temp_dir().join(format!(
        "zincite-shadowing-ambiguous-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let root = dir.join("root.mzn");
    std::fs::write(
        &root,
        "int: duplicated=1; int: duplicated=2; function int: inner(int: duplicated)=duplicated;",
    )
    .unwrap();
    let context = load_model(&root, &ModelOptions::default());
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("suspicious-shadowing").unwrap(),
    );
    assert!(result.findings.is_empty());
    assert!(result.limitations.iter().any(|d| {
        d.message
            .contains("enclosing binding for 'duplicated' is ambiguous")
    }));
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert_eq!(result.status(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}
