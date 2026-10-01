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
fn naming_skips_unnamed_parameters_and_type_or_annotation_references() {
    let parsed = parse(include_str!(
        "../../../tests/fixtures/library-parameters.mzn"
    ));
    assert!(lint(&parsed).unwrap().is_empty());
    let source =
        "predicate library_signature(MissingType, int: BadParameter :: MissingAnnotation);";
    let parsed = parse(source);
    let warnings = lint(&parsed).unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].rule, Rule::Naming);
    let start = source.find("BadParameter").unwrap();
    assert_eq!(warnings[0].range, start..start + "BadParameter".len());
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

#[test]
fn naming_checks_declared_roles_without_matching_references_or_resolving_aliases() {
    let source = concat!(
        "enum bad_enum = {bad_case, GoodCase} ++ bad_constructor({1});\n",
        "type bad_alias = int;\n",
        "set of int: bad_set; var set of int: BadDecision;\n",
        "array [BadIndex in 1..2] of set of int: BadArray;\n",
        "function bool: BadFunction(int: BadParameter) = let { int: BadLocal; } in true;\n",
        "record(int: BadField): good_record;\n",
        "predicate BadPredicate = true; test BadTest = true;\n",
        "annotation BadAnnotation(int: BadAnnotationParameter);\n",
        "array [1..2] of int: values = [BadGenerator | BadGenerator in 1..2];\n",
        "int: _BadUnused = 1; enum SHOUTY = {A};\n",
    );
    let parsed = parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let warnings = lint(&parsed).unwrap();
    let expected = [
        "bad_enum",
        "bad_case",
        "bad_constructor",
        "bad_alias",
        "bad_set",
        "BadDecision",
        "BadIndex",
        "BadArray",
        "BadFunction",
        "BadParameter",
        "BadLocal",
        "BadField",
        "BadPredicate",
        "BadTest",
        "BadAnnotation",
        "BadAnnotationParameter",
        "BadGenerator in",
        "_BadUnused",
        "SHOUTY",
    ];
    assert_eq!(warnings.len(), expected.len(), "{warnings:?}");
    for (warning, marker) in warnings.iter().zip(expected) {
        let name = marker.split_whitespace().next().unwrap();
        let start = source.find(marker).unwrap();
        assert_eq!(warning.range, start..start + name.len());
        assert_eq!(warning.rule, Rule::Naming);
        assert_eq!(warning.severity, Severity::Warning);
    }
    let valid = concat!(
        "enum GoodEnum = {GoodCase} ++ GoodConstructor(BadReference);\n",
        "type GoodAlias = int; set of int: GoodDomain; var set of int: good_set;\n",
        "array [index in 1..2] of set of int: good_array;\n",
        "function bool: good_function(int: good_parameter, set of int: good_set_parameter) = let { int: good_local; } in true;\n",
        "record(int: good_field): good_record; annotation good_annotation;\n",
        "array [1..2] of int: good_values = [BadReference | good_generator in 1..2];\n",
        "int: _unused = BadReference; int: 'Quoted NAME' = 1;\n",
        "Alias: UnclassifiedName; (Alias): OtherUnclassified; any: InferredUnknown; var Alias: good_decision;\n",
        "constraint :: \"label\" forall (_ in 1..2) (true);\n",
    );
    let parsed = parse(valid);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert!(lint(&parsed).unwrap().is_empty());
    let data = zincite_syntax::parse_with_mode("BAD_TARGET = 1;", zincite_syntax::FileMode::Data);
    assert!(lint(&data).unwrap().is_empty());
}

#[test]
fn naming_and_label_suppressions_are_independent_and_stop_at_the_next_item() {
    let source = concat!(
        "% zincite-lint: ignore naming\n",
        "function bool: BadName(int: BadParameter) = let { constraint true; } in true;\n",
        "% zincite-lint: ignore missing-constraint-label\n",
        "function bool: OtherBadName = let { constraint true; int: BadLocal; } in true;\n",
        "% zincite-lint: ignore naming\n",
        "% zincite-lint: ignore missing-constraint-label\n",
        "function bool: SuppressedBadName = let { constraint true; int: SuppressedBadLocal; } in true;\n",
        "int: NextBadName;\n",
    );
    let warnings = lint(&parse(source)).unwrap();
    assert_eq!(warnings.len(), 4, "{warnings:?}");
    assert_eq!(warnings[0].rule, Rule::MissingConstraintLabel);
    for (warning, name) in warnings[1..]
        .iter()
        .zip(["OtherBadName", "BadLocal", "NextBadName"])
    {
        assert_eq!(warning.rule, Rule::Naming);
        assert_eq!(&source[warning.range.clone()], name);
    }
}

#[test]
fn compiler_source_extensions_reach_nested_naming_and_constraint_checks() {
    let source = "var -infinity..infinity: BadBound :: output; record(int: BadField): rec=(BadField:1); var bool: BadFlag == let {var bool: BadLocal :: output == true; constraint BadLocal;} in BadLocal; solve satisfy;";
    let parsed = parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let warnings = lint(&parsed).unwrap();
    let names = warnings
        .iter()
        .filter(|warning| warning.rule == Rule::Naming)
        .map(|warning| &source[warning.range.clone()])
        .collect::<Vec<_>>();
    assert_eq!(names, ["BadBound", "BadField", "BadFlag", "BadLocal"]);
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.rule == Rule::MissingConstraintLabel)
            .count(),
        1
    );
}

#[test]
fn selections_keep_exact_presets_and_disabled_suppressions_independent() {
    use zincite_lint::{LintOptions, lint_with_options};
    let defaults = ["naming", "missing-constraint-label"];
    let thesis = [
        "array-index-start",
        "compact-if",
        "constant-variable",
        "effective-zero-one",
        "element-predicate",
        "reified-global",
        "global-variable-in-function",
        "unbounded-variable",
        "search-coverage",
        "decision-variable-operator",
        "unmarked-symmetry-breaking",
        "unused-declaration",
        "decision-variable-generator",
        "decision-variable-condition",
    ];
    for (selection, expected) in [
        ("default", defaults.to_vec()),
        ("thesis", thesis.to_vec()),
        ("all", defaults.into_iter().chain(thesis).collect()),
    ] {
        let options = LintOptions::from_selection(selection).unwrap();
        assert_eq!(
            options
                .rules
                .iter()
                .map(|rule| rule.id())
                .collect::<Vec<_>>(),
            expected
        );
    }
    let source = "function bool: BadName = let { constraint true; } in true;";
    let parsed = parse(source);
    assert_eq!(
        lint(&parsed),
        lint_with_options(&parsed, &LintOptions::default())
    );
    for (selection, rule) in [
        ("naming", Rule::Naming),
        ("missing-constraint-label", Rule::MissingConstraintLabel),
    ] {
        let options = LintOptions::from_selection(selection).unwrap();
        let warnings = lint_with_options(&parsed, &options).unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].rule, rule);
    }
    let comments: String = thesis
        .iter()
        .map(|id| format!("% zincite-lint: ignore {id}\n"))
        .collect();
    assert_eq!(
        lint(&parse(format!("{comments}{source}"))).unwrap().len(),
        2
    );
    let naming = LintOptions::from_selection("naming,naming").unwrap();
    assert_eq!(naming.rules, [Rule::Naming]);
    let disabled = parse(format!(
        "% zincite-lint: ignore missing-constraint-label\n{source}"
    ));
    assert_eq!(lint_with_options(&disabled, &naming).unwrap().len(), 1);
    for selection in ["thesis", "all", "search-coverage", "naming,search-coverage"] {
        let options = LintOptions::from_selection(selection).unwrap();
        let errors = lint_with_options(&parsed, &options).unwrap_err();
        assert!(errors[0].message.contains("ModelContext"));
        assert_eq!(errors[0].range, 0..0);
    }
    for selection in ["", "unknown", "naming,", "default,naming"] {
        assert!(LintOptions::from_selection(selection).is_err());
    }
    for source in [
        "% zincite-lint: ignore compact-if\n",
        "constraint (\n% zincite-lint: ignore compact-if\ntrue);",
    ] {
        assert!(lint(&parse(source)).is_err());
    }
}
