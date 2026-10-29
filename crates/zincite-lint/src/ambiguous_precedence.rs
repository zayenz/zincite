//! Unparenthesized Boolean operators whose grouping is easy to misread.
use zincite_syntax::{NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

use crate::{LintDiagnostic, Rule};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Conjunction,
    Disjunction,
    Implication,
}

/// Report Boolean binary expressions in `node`. `chain_operand` marks the left
/// operand of an implication, whose chain the enclosing implication reports.
pub(super) fn check_precedence(
    node: &SyntaxNode,
    parsed: &ParsedFile,
    chain_operand: bool,
    warnings: &mut Vec<LintDiagnostic>,
) {
    let operator = boolean_operator(node, parsed);
    let level = operator.map(|(_, level)| level);
    if let Some((index, level)) = operator {
        let range = parsed.tokens()[index].range.clone();
        let written = &parsed.source()[range.clone()];
        let mut groups = Vec::new();
        let explanation = match level {
            Level::Disjunction => {
                groups.extend(
                    node.child_nodes()
                        .filter(|child| is_level(child, parsed, Level::Conjunction)),
                );
                format!("conjunction binds tighter than '{written}'")
            }
            Level::Implication if !chain_operand => {
                let mut left = node.child_nodes().next();
                while let Some(inner) =
                    left.filter(|inner| is_level(inner, parsed, Level::Implication))
                {
                    groups.push(inner);
                    left = inner.child_nodes().next();
                }
                "implications group from the left".into()
            }
            _ => String::new(),
        };
        if !groups.is_empty() {
            warnings.push(LintDiagnostic {
                fix: None,
                rule: Rule::AmbiguousPrecedence,
                severity: Rule::AmbiguousPrecedence.severity(),
                message: format!(
                    "{explanation}, so this reads as '{}'; add parentheses to show the grouping",
                    grouped(node, &groups, parsed)
                ),
                range,
            });
        }
    }
    for (position, child) in node.child_nodes().enumerate() {
        let chain_operand = level == Some(Level::Implication) && position == 0;
        check_precedence(child, parsed, chain_operand, warnings);
    }
}

fn is_level(node: &SyntaxNode, parsed: &ParsedFile, level: Level) -> bool {
    boolean_operator(node, parsed).is_some_and(|(_, found)| found == level)
}

/// The operator token of a binary expression over `/\`, `\/`, `xor`, `->` or
/// `<-` in any spelling. Backquoted infix calls bind differently and are not
/// these operators.
fn boolean_operator(node: &SyntaxNode, parsed: &ParsedFile) -> Option<(usize, Level)> {
    if node.kind() != NodeKind::BinaryExpression {
        return None;
    }
    node.children().iter().find_map(|child| {
        let SyntaxElement::Token(index) = child else {
            return None;
        };
        let level = match parsed.tokens()[*index].kind {
            TokenKind::And => Level::Conjunction,
            TokenKind::Or | TokenKind::Xor => Level::Disjunction,
            TokenKind::Implies | TokenKind::ReverseImplies => Level::Implication,
            _ => return None,
        };
        Some((*index, level))
    })
}

/// Render `node` on one line without comments, parenthesizing `groups`.
fn grouped(node: &SyntaxNode, groups: &[&SyntaxNode], parsed: &ParsedFile) -> String {
    let significant = |range: std::ops::Range<usize>| {
        parsed.tokens().iter().filter(move |token| {
            range.start <= token.range.start
                && token.range.end <= range.end
                && !matches!(
                    token.kind,
                    TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                )
        })
    };
    // Node ranges can include trivia, so bracket each group's written tokens.
    let bounds: Vec<_> = groups
        .iter()
        .filter_map(|group| {
            let first = significant(group.range()).next()?;
            let last = significant(group.range()).next_back()?;
            Some((first.range.start, last.range.end))
        })
        .collect();
    let mut text = String::new();
    let mut previous_end = None;
    for token in significant(node.range()) {
        if previous_end.is_some_and(|end| token.range.start > end) {
            text.push(' ');
        }
        for _ in bounds
            .iter()
            .filter(|(start, _)| *start == token.range.start)
        {
            text.push('(');
        }
        text.push_str(&parsed.source()[token.range.clone()]);
        for _ in bounds.iter().filter(|(_, end)| *end == token.range.end) {
            text.push(')');
        }
        previous_end = Some(token.range.end);
    }
    text
}
