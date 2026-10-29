use zincite_lint::{LintOptions, Severity, lint_with_options};
use zincite_syntax::parse;

#[test]
fn misread_boolean_groupings_name_their_actual_grouping() {
    let source = concat!(
        "constraint a \\/ b /\\ c;\n",
        "constraint a -> b -> c;\n",
        "constraint a \\/ (b /\\ c);\n",
        "constraint a -> (b -> c);\n",
        "constraint (a -> b) -> c;\n",
        "constraint a /\\ b -> c;\n",
    );
    let parsed = parse(source);
    let warnings = lint_with_options(
        &parsed,
        &LintOptions::from_selection("ambiguous-precedence").unwrap(),
    )
    .unwrap();
    let found: Vec<_> = warnings
        .iter()
        .map(|w| (&source[w.range.clone()], w.message.as_str(), w.severity))
        .collect();
    assert_eq!(
        found,
        [
            (
                "\\/",
                "conjunction binds tighter than '\\/', so this reads as 'a \\/ (b /\\ c)'; add parentheses to show the grouping",
                Severity::Warning
            ),
            (
                "->",
                "implications group from the left, so this reads as '(a -> b) -> c'; add parentheses to show the grouping",
                Severity::Warning
            ),
        ]
    );
    assert_eq!(warnings[1].range.start, source.find("b -> c").unwrap() + 2);
}
