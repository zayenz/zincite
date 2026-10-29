//! Exact equality between parameter float expressions.
use crate::callables::{CallableView, core_operation};
use crate::domains::tokens;
use crate::{
    BindingFacts, FileFinding, FileId, Instantiation, InstantiationFacts, ModelContext, Rule,
    TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

pub(super) fn check_float_equality(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableView<'_>,
    instantiations: &InstantiationFacts,
) -> Vec<FileFinding> {
    let checker = Checker {
        context,
        bindings,
        calls,
        instantiations,
    };
    let mut findings = Vec::new();
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() {
            continue;
        }
        let Some(suppressions) = &source.suppressions else {
            continue;
        };
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            if !suppressions[item].contains(&Rule::FloatEquality) {
                checker.walk(file, node, &mut findings);
            }
        }
    }
    findings
}

struct Checker<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableView<'a>,
    instantiations: &'a InstantiationFacts,
}

impl Checker<'_> {
    fn walk(&self, file: FileId, node: &SyntaxNode, findings: &mut Vec<FileFinding>) {
        if node.kind() == NodeKind::BinaryExpression {
            let operator = tokens(&self.context.files[file].parsed, node)
                .first()
                .map(|t| t.kind);
            let name = match operator {
                Some(TokenKind::Equal | TokenKind::DoubleEqual) => Some("="),
                Some(TokenKind::NotEqual) => Some("!="),
                _ => None,
            };
            if let Some(name) = name
                && node.child_nodes().count() == 2
                && node
                    .child_nodes()
                    .all(|operand| self.par_float(file, operand))
                && core_operation(self.context, self.bindings, self.calls, file, node, name)
                    == Ok(true)
            {
                findings.push(FileFinding {
                    fix: None,
                    location: self.context.files[file].location(node.range()),
                    rule: Rule::FloatEquality,
                    severity: Rule::FloatEquality.severity(),
                    message: format!(
                        "exact {name} between parameter floats; computed floats rarely compare equal, so compare within a tolerance such as abs(a - b) < 1e-9"
                    ),
                });
            }
        }
        for child in node.child_nodes() {
            self.walk(file, child, findings);
        }
    }

    fn par_float(&self, file: FileId, operand: &SyntaxNode) -> bool {
        let range = self.context.files[file].location(operand.range()).range;
        let typed = self.calls.expression(file, &range).is_some_and(|e| {
            e.ty.kind == TypeKind::Float
                && !e.ty.optional
                && e.ty.instantiation == Instantiation::Parameter
        });
        // Instantiation facts are kept in (file, start, end) order.
        typed
            && self
                .instantiations
                .expressions
                .binary_search_by(|e| {
                    (e.file, e.location.range.start, e.location.range.end).cmp(&(
                        file,
                        range.start,
                        range.end,
                    ))
                })
                .is_ok_and(|row| {
                    self.instantiations.expressions[row].instantiation == Instantiation::Parameter
                })
    }
}
