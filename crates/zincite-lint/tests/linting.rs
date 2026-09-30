use zincite_lint::{Rule, Severity, lint};
use zincite_syntax::parse;

#[test]
fn labels_are_direct_and_suppressions_cover_the_next_complete_item() {
    let source = concat!(
        "constraint :: \"explanation\" true;\n",
        "constraint :: \"value \\(1)\" true;\n",
        "constraint assert(true, \"nested string\", true);\n",
        "constraint (true :: \"expression annotation\");\n",
        "function bool: local() = let { constraint true; } in true;\n",
        "% zincite-lint: ignore naming\n",
        "% zincite-lint: ignore missing-constraint-label\n",
        "function bool: suppressed() = let { constraint true; } in true;\n",
        "constraint false;\n",
    );
    let parsed = parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let warnings = lint(&parsed).unwrap();
    let expected = [
        "constraint assert",
        "constraint (",
        "constraint true",
        "constraint false",
    ];
    assert_eq!(warnings.len(), expected.len());
    for (warning, marker) in warnings.iter().zip(expected) {
        let start = source.find(marker).unwrap();
        assert_eq!(warning.range, start..start + "constraint".len());
        assert_eq!(warning.rule, Rule::MissingConstraintLabel);
        assert_eq!(warning.severity, Severity::Warning);
        assert!(
            warning.message.contains("consider") && warning.message.contains("modelling intent")
        );
    }
    assert_eq!(parsed.source(), source);
    // Naming is a valid suppression ID but does not suppress this rule.
    assert_eq!(
        lint(&parse("% zincite-lint: ignore naming\nconstraint true;"))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn invalid_suppressions_and_syntax_errors_omit_all_warnings() {
    for source in [
        "constraint true;\n% zincite-lint: ignore other\nconstraint false;",
        "% zincite-lint: ignore\nconstraint true;",
        "constraint true; % zincite-lint: ignore naming\nconstraint false;",
        "constraint (\n% zincite-lint: ignore naming\ntrue);",
        "constraint true;\n% zincite-lint: ignore naming\n",
    ] {
        let parsed = parse(source);
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        let errors = lint(&parsed).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            &source[errors[0].range.clone()],
            source
                .lines()
                .find(|line| line.contains("zincite-lint:"))
                .unwrap()
                .trim_start_matches("constraint true; ")
        );
    }
    let parsed = parse("constraint true; int: broken = ;");
    assert!(!parsed.diagnostics().is_empty());
    assert_eq!(lint(&parsed).unwrap_err(), parsed.diagnostics());
}
