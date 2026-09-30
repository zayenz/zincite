use zincite_syntax::{NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

use crate::{LintDiagnostic, Rule, Severity};

#[derive(Clone, Copy)]
enum Style {
    Snake,
    UpperCamel,
}

pub(super) fn check_names(
    node: &SyntaxNode,
    parsed: &ParsedFile,
    warnings: &mut Vec<LintDiagnostic>,
) {
    let style = match node.kind() {
        NodeKind::EnumDeclaration
        | NodeKind::TypeAlias
        | NodeKind::EnumCase
        | NodeKind::EnumConstructor => Some(Style::UpperCamel),
        NodeKind::Declaration => node
            .child_nodes()
            .next()
            .and_then(|ty| declaration_style(ty, parsed)),
        NodeKind::FunctionDeclaration
        | NodeKind::PredicateDeclaration
        | NodeKind::TestDeclaration
        | NodeKind::AnnotationDeclaration
        | NodeKind::Parameter
        | NodeKind::RecordField
        | NodeKind::ArrayIndexBinding
        | NodeKind::Generator => Some(Style::Snake),
        _ => None,
    };
    if let Some(style) = style {
        // Only direct identifier leaves declare these names. Child expressions
        // retain references, quoted names and anonymous bindings are exempt.
        for token in node.children().iter().filter_map(|child| match child {
            SyntaxElement::Token(index)
                if parsed.tokens()[*index].kind == TokenKind::Identifier =>
            {
                Some(&parsed.tokens()[*index])
            }
            _ => None,
        }) {
            let written = &parsed.source()[token.range.clone()];
            let name = written.strip_prefix('_').unwrap_or(written);
            let (valid, expected) = match style {
                Style::Snake => (snake_case(name), "snake_case"),
                Style::UpperCamel => (upper_camel_case(name), "UpperCamelCase"),
            };
            if !valid {
                warnings.push(LintDiagnostic {
                    rule: Rule::Naming,
                    severity: Severity::Warning,
                    range: token.range.clone(),
                    message: format!("use {expected} for declared name '{written}'"),
                });
            }
        }
    }
    for child in node.child_nodes() {
        check_names(child, parsed, warnings);
    }
}

fn declaration_style(ty: &SyntaxNode, parsed: &ParsedFile) -> Option<Style> {
    let decision = ty.children().iter().any(|child| {
        matches!(child,
        SyntaxElement::Token(index) if parsed.tokens()[*index].kind == TokenKind::Var)
    });
    match ty.kind() {
        NodeKind::SetType if !decision => Some(Style::UpperCamel),
        NodeKind::ScalarType
            if !decision
                && ty.children().iter().any(|child| {
                    matches!(child,
            SyntaxElement::Token(index) if parsed.tokens()[*index].kind == TokenKind::Any)
                }) =>
        {
            None
        }
        NodeKind::ScalarType
        | NodeKind::SetType
        | NodeKind::ArrayType
        | NodeKind::ListType
        | NodeKind::TupleType
        | NodeKind::RecordType => Some(Style::Snake),
        _ if decision => Some(Style::Snake),
        NodeKind::DomainType => {
            // A bare named domain can also be a type alias (including a set
            // alias). Classifying it needs resolution; explicit domains do not.
            let mut domain = ty.child_nodes().next()?;
            while domain.kind() == NodeKind::ParenthesizedExpression {
                domain = domain.child_nodes().next()?;
            }
            let named = domain.kind() == NodeKind::Expression && domain.children().iter().any(|child| matches!(child,
                SyntaxElement::Token(index) if matches!(parsed.tokens()[*index].kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)));
            (!named).then_some(Style::Snake)
        }
        _ => None,
    }
}

fn snake_case(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !name.ends_with('_')
        && !name.contains("__")
}

fn upper_camel_case(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
        && name.bytes().all(|byte| byte.is_ascii_alphanumeric())
        && (name.bytes().any(|byte| byte.is_ascii_lowercase())
            || name.bytes().filter(u8::is_ascii_uppercase).count() == 1)
}
