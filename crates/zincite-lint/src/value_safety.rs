//! Bounded expression safety shared by definition and compact-if interpretation.
use crate::domains::expression_integer;
use crate::{
    BindingFacts, CallableFacts, DefinitionSafety, FileId, ModelContext, TypeInst, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

/// Check supported value safety without running lint or evaluating model code.
/// The node, file and prerequisite facts must belong to this same ModelContext.
/// This is a bounded check: opaque calls, index membership, options and control
/// flow remain explicit unsupported facts, not a general definedness proof.
pub fn expression_safety(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    file: FileId,
    node: &SyntaxNode,
) -> DefinitionSafety {
    Safety {
        context,
        bindings,
        calls,
    }
    .check(file, node)
}
struct Safety<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
}
impl Safety<'_> {
    fn ty(&self, file: FileId, node: &SyntaxNode) -> Option<&TypeInst> {
        let location = self.context.files[file].location(node.range());
        self.calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == location.range)
            .map(|e| &e.ty)
    }
    fn tokens(&self, file: FileId, node: &SyntaxNode) -> Vec<&zincite_syntax::Token> {
        node.children()
            .iter()
            .filter_map(|child| {
                if let SyntaxElement::Token(index) = child {
                    let token = &self.context.files[file].parsed.tokens()[*index];
                    (!matches!(
                        token.kind,
                        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                    ))
                    .then_some(token)
                } else {
                    None
                }
            })
            .collect()
    }
    fn check(&self, file: FileId, node: &SyntaxNode) -> DefinitionSafety {
        let Some(ty) = self.ty(file, node) else {
            return DefinitionSafety::Unsupported("value type is unavailable".into());
        };
        if optional(ty) {
            return DefinitionSafety::Unsupported(
                "optional definition values are not supported".into(),
            );
        }
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::Expression => {
                if matches!(ty.kind, TypeKind::Unknown(_)) {
                    DefinitionSafety::Unsupported("definition value type is unsupported".into())
                } else {
                    DefinitionSafety::Supported
                }
            }
            NodeKind::MatrixLiteral => {
                for value in children.iter().flat_map(|row| row.child_nodes()) {
                    let safety = self.check(file, value);
                    if safety != DefinitionSafety::Supported {
                        return safety;
                    }
                }
                DefinitionSafety::Supported
            }
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression => {
                DefinitionSafety::Unsupported(
                    "arbitrary calls do not establish definition value safety".into(),
                )
            }
            NodeKind::ArrayAccessExpression => DefinitionSafety::Unsupported(
                "definition array access requires an index-membership proof".into(),
            ),
            NodeKind::BinaryExpression
                if self
                    .tokens(file, node)
                    .iter()
                    .any(|t| matches!(t.kind, TokenKind::Div | TokenKind::Mod)) =>
            {
                match expression_integer(self.context, self.bindings, file, node) {
                    Ok(Some(_)) => DefinitionSafety::Supported,
                    Ok(None) => DefinitionSafety::Unknown(
                        "divisor/value is not a known closed integer expression".into(),
                    ),
                    Err(reason) => DefinitionSafety::Unsupported(reason),
                }
            }
            NodeKind::BinaryExpression
                if !self.tokens(file, node).iter().any(|t| {
                    matches!(
                        t.kind,
                        TokenKind::Plus
                            | TokenKind::Minus
                            | TokenKind::Star
                            | TokenKind::And
                            | TokenKind::Or
                            | TokenKind::Equal
                            | TokenKind::DoubleEqual
                            | TokenKind::Less
                            | TokenKind::LessEqual
                            | TokenKind::Greater
                            | TokenKind::GreaterEqual
                            | TokenKind::NotEqual
                    )
                }) =>
            {
                DefinitionSafety::Unsupported(
                    "definition operator partiality is unsupported".into(),
                )
            }
            NodeKind::LetExpression
            | NodeKind::ConditionalExpression
            | NodeKind::ArrayComprehension
            | NodeKind::SetComprehension
            | NodeKind::IndexedArrayComprehension => DefinitionSafety::Unsupported(
                "definition value control/iteration safety is unsupported".into(),
            ),
            _ => {
                for child in children {
                    let status = self.check(file, child);
                    if status != DefinitionSafety::Supported {
                        return status;
                    }
                }
                DefinitionSafety::Supported
            }
        }
    }
}
pub(super) fn optional(ty: &TypeInst) -> bool {
    ty.optional
        || match &ty.kind {
            TypeKind::Array { element, .. } | TypeKind::Set(element) => optional(element),
            TypeKind::Tuple(fields) => fields.iter().any(optional),
            TypeKind::Record(fields) => fields.iter().any(|(_, ty)| optional(ty)),
            _ => false,
        }
}
