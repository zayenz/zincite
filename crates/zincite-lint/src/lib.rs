//! MiniZinc modelling advice with syntax-only defaults and explicit model loading.

use std::ops::Range;
use zincite_syntax::{Diagnostic, NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

mod analysis;
mod model;
mod naming;
mod rules;
pub use analysis::{
    AnalysisResult, FileFinding, RuleExecution, RuleOutcome, analyze_file, analyze_model,
    write_analysis,
};
pub use model::{
    FileId, IncludeEdge, ModelContext, ModelFile, ModelOptions, SourceDiagnostic, SourceKind,
    SourceLocation, load_model,
};
pub use rules::{LintOptions, Rule};

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
    lint_with_options(parsed, &LintOptions::default())
}

/// Lint selected rules. Syntax, suppression or unavailable-rule errors omit all
/// warnings. Selection errors use the zero-length range at the start of the file.
pub fn lint_with_options(
    parsed: &ParsedFile,
    options: &LintOptions,
) -> Result<Vec<LintDiagnostic>, Vec<Diagnostic>> {
    if !parsed.diagnostics().is_empty() {
        return Err(parsed.diagnostics().to_vec());
    }
    let items: Vec<_> = parsed.tree().child_nodes().collect();
    let suppressed = item_suppressions(parsed, &items)?;
    options.check_available().map_err(|message| {
        vec![Diagnostic {
            range: 0..0,
            message,
        }]
    })?;
    Ok(lint_items(parsed, &suppressed, options))
}

fn lint_items(
    parsed: &ParsedFile,
    suppressed: &[Vec<Rule>],
    options: &LintOptions,
) -> Vec<LintDiagnostic> {
    let mut warnings = Vec::new();
    for (item, suppressed) in parsed.tree().child_nodes().zip(suppressed) {
        if options.rules.contains(&Rule::Naming) && !suppressed.contains(&Rule::Naming) {
            naming::check_names(item, parsed, &mut warnings);
        }
        if options.rules.contains(&Rule::MissingConstraintLabel)
            && !suppressed.contains(&Rule::MissingConstraintLabel)
        {
            missing_labels(item, parsed, &mut warnings);
        }
    }
    warnings.sort_by_key(|warning| warning.range.start);
    warnings
}

fn item_suppressions(
    parsed: &ParsedFile,
    items: &[&SyntaxNode],
) -> Result<Vec<Vec<Rule>>, Vec<Diagnostic>> {
    let mut suppressed = vec![Vec::new(); items.len()];
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
        let rule = text
            .trim_end()
            .strip_prefix("% zincite-lint: ignore ")
            .and_then(Rule::from_id)
            .ok_or_else(|| error("unknown or malformed lint suppression directive"))?;
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
        suppressed[next].push(rule);
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
