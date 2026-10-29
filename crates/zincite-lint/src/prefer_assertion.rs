//! Constraint items over parameters only, which an assert would explain.
use crate::callables::is_expression;
use crate::{
    FileFinding, Instantiation, InstantiationFacts, ModelContext, ModelFile, Rule, SourceDiagnostic,
};
use std::collections::BTreeMap;
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

pub(super) fn check_prefer_assertion(
    context: &ModelContext,
    facts: &InstantiationFacts,
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let facts: BTreeMap<_, _> = facts
        .expressions
        .iter()
        .map(|f| ((f.file, f.location.range.start, f.location.range.end), f))
        .collect();
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() {
            continue;
        }
        let Some(suppressions) = &source.suppressions else {
            continue;
        };
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            if node.kind() != NodeKind::Constraint
                || suppressions[item].contains(&Rule::PreferAssertion)
            {
                continue;
            }
            let Some(expression) = node.child_nodes().find(|c| is_expression(c.kind())) else {
                continue;
            };
            // Local constraint items can enforce decisions even when the
            // let's value is a parameter, so withhold advice for them.
            if is_assert_call(source, expression) || has_local_constraint(expression) {
                continue;
            }
            let location = source.location(expression.range());
            match facts.get(&(file, location.range.start, location.range.end)) {
                Some(fact) if fact.instantiation == Instantiation::Parameter => {
                    findings.push(FileFinding {
                        fix: None,
                        location,
                        rule: Rule::PreferAssertion,
                        severity: Rule::PreferAssertion.severity(),
                        message: "this constraint depends only on parameters; consider assert(condition, \"message\") to explain failed input".into(),
                    });
                }
                Some(fact) if fact.instantiation == Instantiation::Decision => {}
                fact => {
                    let reason = fact
                        .and_then(|f| f.reason.clone())
                        .unwrap_or_else(|| "expression instantiation is unavailable".into());
                    limitations.push(SourceDiagnostic {
                        location,
                        message: format!("{}: {reason}", Rule::PreferAssertion.id()),
                    });
                }
            }
        }
    }
    (findings, limitations)
}

fn is_assert_call(source: &ModelFile, mut expression: &SyntaxNode) -> bool {
    while matches!(
        expression.kind(),
        NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression
    ) {
        let Some(child) = expression.child_nodes().next() else {
            return false;
        };
        expression = child;
    }
    let tokens = source.parsed.tokens();
    let head = expression.children().iter().find_map(|child| match child {
        SyntaxElement::Token(index)
            if matches!(
                tokens[*index].kind,
                TokenKind::Identifier | TokenKind::QuotedIdentifier | TokenKind::InfixIdentifier
            ) =>
        {
            Some(source.parsed.source()[tokens[*index].range.clone()].trim_matches(['\'', '`']))
        }
        _ => None,
    });
    matches!(
        expression.kind(),
        NodeKind::CallExpression | NodeKind::BinaryExpression
    ) && head == Some("assert")
}

fn has_local_constraint(node: &SyntaxNode) -> bool {
    (node.kind() == NodeKind::LetBlock
        && node.child_nodes().any(|c| c.kind() == NodeKind::Constraint))
        || node.child_nodes().any(has_local_constraint)
}
