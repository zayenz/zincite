//! Syntax-based MiniZinc modelling advice, without name or type resolution.

use std::ops::Range;
use zincite_syntax::{Diagnostic, NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

mod naming;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Naming,
    MissingConstraintLabel,
}

impl Rule {
    pub fn id(self) -> &'static str {
        match self {
            Self::Naming => "naming",
            Self::MissingConstraintLabel => "missing-constraint-label",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Warning,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LintDiagnostic {
    pub rule: Rule,
    pub severity: Severity,
    pub range: Range<usize>,
    pub message: String,
}

/// Lint a complete parsed file. Syntax or suppression errors omit all warnings.
/// Suppressions apply independently throughout the next top-level item. Naming
/// checks declared syntax roles; missing-label warnings are modelling advice.
pub fn lint(parsed: &ParsedFile) -> Result<Vec<LintDiagnostic>, Vec<Diagnostic>> {
    if !parsed.diagnostics().is_empty() {
        return Err(parsed.diagnostics().to_vec());
    }
    let items: Vec<_> = parsed.tree().child_nodes().collect();
    let suppressed = item_suppressions(parsed, &items)?;
    let mut warnings = Vec::new();
    for (item, suppressed) in items.into_iter().zip(suppressed) {
        if !suppressed.naming {
            naming::check_names(item, parsed, &mut warnings);
        }
        if !suppressed.missing_label {
            missing_labels(item, parsed, &mut warnings);
        }
    }
    warnings.sort_by_key(|warning| warning.range.start);
    Ok(warnings)
}

#[derive(Clone, Copy, Default)]
struct Suppression {
    naming: bool,
    missing_label: bool,
}

fn item_suppressions(
    parsed: &ParsedFile,
    items: &[&SyntaxNode],
) -> Result<Vec<Suppression>, Vec<Diagnostic>> {
    let mut suppressed = vec![Suppression::default(); items.len()];
    for token in parsed.tokens() {
        if token.kind != TokenKind::LineComment {
            continue;
        }
        let text = &parsed.source()[token.range.clone()];
        if !text.starts_with("% zincite-lint:") {
            continue;
        }
        let error = |message: &str| {
            vec![Diagnostic {
                range: token.range.clone(),
                message: message.into(),
            }]
        };
        let rule = match text.trim_end() {
            "% zincite-lint: ignore naming" => Rule::Naming,
            "% zincite-lint: ignore missing-constraint-label" => Rule::MissingConstraintLabel,
            _ => return Err(error("unknown or malformed lint suppression directive")),
        };
        let line_start = parsed.source()[..token.range.start]
            .rfind(['\r', '\n'])
            .map_or(0, |position| position + 1);
        if !parsed.source()[line_start..token.range.start]
            .chars()
            .all(|character| matches!(character, ' ' | '\t'))
        {
            return Err(error("lint suppression must be a standalone comment"));
        }
        if items
            .iter()
            .any(|item| item.range().contains(&token.range.start))
        {
            return Err(error(
                "lint suppression must be between complete top-level items",
            ));
        }
        let Some(next) = items
            .iter()
            .position(|item| item.range().start >= token.range.end)
        else {
            return Err(error("lint suppression has no following item"));
        };
        match rule {
            Rule::Naming => suppressed[next].naming = true,
            Rule::MissingConstraintLabel => suppressed[next].missing_label = true,
        }
    }
    Ok(suppressed)
}

fn missing_labels(node: &SyntaxNode, parsed: &ParsedFile, warnings: &mut Vec<LintDiagnostic>) {
    if node.kind() == NodeKind::Constraint {
        // Constraint-header labels are direct children, unlike expression
        // annotations and strings inside calls, parentheses or interpolation.
        let labelled = node.children().iter().any(|child| matches!(child,
            SyntaxElement::Token(index) if parsed.tokens()[*index].kind == TokenKind::AnnotationMarker));
        if !labelled {
            let keyword = node
                .children()
                .iter()
                .find_map(|child| match child {
                    SyntaxElement::Token(index)
                        if parsed.tokens()[*index].kind == TokenKind::Constraint =>
                    {
                        Some(&parsed.tokens()[*index])
                    }
                    _ => None,
                })
                .expect("constraint nodes contain their keyword");
            warnings.push(LintDiagnostic {
                rule: Rule::MissingConstraintLabel,
                severity: Severity::Warning,
                range: keyword.range.clone(),
                message:
                    "consider adding a string label to explain this constraint's modelling intent"
                        .into(),
            });
        }
    }
    for child in node.child_nodes() {
        missing_labels(child, parsed, warnings);
    }
}
