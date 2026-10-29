//! Bounded expression safety shared by definition and compact-if interpretation.
use crate::domains::{expression_integer, invariant_expression_integer};
use crate::{
    BindingFacts, DefinitionSafety, FileId, Instantiation, ModelContext, TypeInst, TypeKind,
};
use std::collections::HashMap;
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

pub(crate) fn expression_safety_view(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::callables::CallableView<'_>,
    file: FileId,
    node: &SyntaxNode,
) -> DefinitionSafety {
    Safety {
        context,
        bindings,
        calls,
        membership: None,
        expression_indices: None,
    }
    .check(file, node)
}
/// Use the caller's first-match index for these same original callable facts.
pub(super) fn indexed_expression_safety(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::callables::CallableView<'_>,
    file: FileId,
    node: &SyntaxNode,
    expression_indices: &HashMap<(FileId, usize, usize), usize>,
) -> DefinitionSafety {
    Safety {
        context,
        bindings,
        calls,
        membership: None,
        expression_indices: Some(expression_indices),
    }
    .check(file, node)
}
pub(crate) fn opaque_expression_safety(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::callables::CallableView<'_>,
    file: FileId,
    node: &SyntaxNode,
) -> DefinitionSafety {
    Safety {
        context,
        bindings,
        calls,
        membership: None,
        expression_indices: None,
    }
    .opaque(file, node, "definition source safety is unproved")
}
struct Safety<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    membership: Option<std::ops::Range<usize>>,
    expression_indices: Option<&'a HashMap<(FileId, usize, usize), usize>>,
}
impl Safety<'_> {
    fn ty(&self, file: FileId, node: &SyntaxNode) -> Option<&TypeInst> {
        let location = self.context.files[file].location(node.range());
        if let Some(indices) = self.expression_indices {
            return indices
                .get(&(file, location.range.start, location.range.end))
                .map(|&row| &self.calls.expressions[row].ty);
        }
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
                let mut safety = DefinitionSafety::Supported;
                for value in children.iter().flat_map(|row| row.child_nodes()) {
                    match self.check(file, value) {
                        unsupported @ DefinitionSafety::Unsupported(_) => return unsupported,
                        unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                        DefinitionSafety::Supported => {}
                    }
                }
                safety
            }
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression => {
                DefinitionSafety::Unsupported(
                    "arbitrary calls do not establish definition value safety".into(),
                )
            }
            NodeKind::ArrayAccessExpression if self.membership.as_ref() == Some(&node.range()) => {
                let mut safety = DefinitionSafety::Supported;
                for child in children {
                    match self.check(file, child) {
                        unsupported @ DefinitionSafety::Unsupported(_) => return unsupported,
                        unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                        DefinitionSafety::Supported => {}
                    }
                }
                safety
            }
            NodeKind::ArrayAccessExpression if ty.known() && ty.kind == TypeKind::Int => {
                self.opaque(file, node, "definition array membership is unproved")
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
                let value = if self.membership.is_some() {
                    invariant_expression_integer(self.context, self.bindings, file, node)
                } else {
                    expression_integer(self.context, self.bindings, file, node)
                };
                match value {
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
            NodeKind::ArrayComprehension
                if ty.known()
                    && matches!(&ty.kind, TypeKind::Array { element, .. }
                        if element.kind == TypeKind::Int) =>
            {
                self.opaque(file, node, "definition comprehension safety is unproved")
            }
            NodeKind::LetExpression
            | NodeKind::ConditionalExpression
            | NodeKind::ArrayComprehension
            | NodeKind::SetComprehension
            | NodeKind::IndexedArrayComprehension => DefinitionSafety::Unsupported(
                "definition value control/iteration safety is unsupported".into(),
            ),
            _ => {
                let mut safety = DefinitionSafety::Supported;
                for child in children {
                    let status = self.check(file, child);
                    match status {
                        DefinitionSafety::Unsupported(_) => return status,
                        DefinitionSafety::Unknown(_) => safety = status,
                        DefinitionSafety::Supported => {}
                    }
                }
                safety
            }
        }
    }
    fn opaque(&self, file: FileId, node: &SyntaxNode, reason: &str) -> DefinitionSafety {
        let mut pending = vec![node];
        while let Some(node) = pending.pop() {
            if !crate::definitions::annotations_safe(self.context, file, node) {
                return DefinitionSafety::Unsupported(
                    "definition source annotations are unsupported".into(),
                );
            }
            if crate::callables::is_expression(node.kind()) {
                let Some(ty) = self.ty(file, node).filter(|ty| ty.known()) else {
                    return DefinitionSafety::Unsupported(
                        "definition source type is unavailable".into(),
                    );
                };
                if optional(ty) {
                    return DefinitionSafety::Unsupported(
                        "optional definition values are not supported".into(),
                    );
                }
                if matches!(
                    node.kind(),
                    NodeKind::CallExpression | NodeKind::GeneratorCallExpression
                ) && let Err(reason) = crate::callables::selected_call_type(
                    crate::callables::operation_fact(self.context, self.calls, file, node),
                    Some(ty),
                ) {
                    return DefinitionSafety::Unsupported(reason);
                }
            }
            pending.extend(node.child_nodes());
        }
        if let Some(reason) = (ClosedSourceErrors {
            context: self.context,
            bindings: self.bindings,
            calls: self.calls,
        })
        .check(file, node, false, true, true)
        {
            DefinitionSafety::Unsupported(reason)
        } else {
            DefinitionSafety::Unknown(reason.into())
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

/// Only the exact access whose full generator/index correspondence was proved.
/// Ordinary expression_safety retains Unknown without this membership proof.
/// Traversal safety also retains parameter dependencies in closed div/mod checks.
pub(super) fn traversal_expression_safety(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::callables::CallableView<'_>,
    file: FileId,
    node: &SyntaxNode,
    access: &SyntaxNode,
) -> DefinitionSafety {
    Safety {
        context,
        bindings,
        calls,
        membership: Some(access.range()),
        expression_indices: None,
    }
    .check(file, node)
}

// Observe closed failures without requesting initialized-body admission or a value proof.
pub(crate) struct ClosedSourceErrors<'a> {
    pub context: &'a ModelContext,
    pub bindings: &'a BindingFacts,
    pub calls: &'a crate::callables::CallableView<'a>,
}
impl ClosedSourceErrors<'_> {
    fn expression_type(&self, file: FileId, node: &SyntaxNode) -> Option<&crate::ExpressionType> {
        let range = self.context.files[file].location(node.range()).range;
        self.calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
    }
    fn operation_fact<'b>(
        &self,
        facts: &'b crate::callables::CallableView<'b>,
        file: FileId,
        node: &SyntaxNode,
    ) -> Option<&'b crate::CallFact> {
        crate::callables::operation_fact(self.context, facts, file, node)
    }
    fn core(
        &self,
        file: FileId,
        node: &SyntaxNode,
        facts: &crate::callables::CallableView<'_>,
        name: &str,
    ) -> bool {
        crate::callables::core_operation(self.context, self.bindings, facts, file, node, name)
            == Ok(true)
    }
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<crate::DeclarationId> {
        let range = self.context.files[file].location(node.range()).range;
        self.bindings
            .value_reference_within(file, &range)
            .and_then(|reference| match reference.resolution {
                crate::BindingResolution::Resolved(id) => Some(id),
                _ => None,
            })
    }
    fn assertion_literal(&self, file: FileId, mut node: &SyntaxNode) -> Option<TokenKind> {
        while node.kind() == NodeKind::ParenthesizedExpression {
            node = node.child_nodes().next()?;
        }
        (node.kind() == NodeKind::Expression)
            .then(|| {
                crate::domains::tokens(&self.context.files[file].parsed, node)
                    .first()
                    .map(|token| token.kind)
            })
            .flatten()
    }
    pub(crate) fn false_integer_assertion(&self, file: FileId, node: &SyntaxNode) -> bool {
        if node.kind() != NodeKind::CallExpression
            || !self.core(file, node, self.calls, "assert")
            || crate::callables::selected_call_type(
                self.operation_fact(self.calls, file, node),
                self.expression_type(file, node)
                    .map(|expression| &expression.ty),
            )
            .is_err()
        {
            return false;
        }
        let arguments: Vec<_> = node.child_nodes().collect();
        arguments.len() == 3
            && arguments
                .iter()
                .zip([TypeKind::Bool, TypeKind::String, TypeKind::Int])
                .all(|(argument, kind)| {
                    argument.kind() != NodeKind::NamedArgument
                        && self
                            .expression_type(file, argument)
                            .is_some_and(|expression| {
                                expression.ty.known()
                                    && !optional(&expression.ty)
                                    && expression.ty.instantiation == Instantiation::Parameter
                                    && expression.ty.kind == kind
                            })
                })
            && self.assertion_literal(file, arguments[0]) == Some(TokenKind::False)
    }
    pub(crate) fn float_divisor_error(&self, file: FileId, node: &SyntaxNode) -> Option<String> {
        let children: Vec<_> = node.child_nodes().collect();
        let [_, divisor] = children.as_slice() else {
            return None;
        };
        let float = TypeInst::par(TypeKind::Float);
        if node.kind() != NodeKind::BinaryExpression
            || operator(self.context, file, node) != Some(TokenKind::Slash)
            || !self.core(file, node, self.calls, "/")
            || self
                .expression_type(file, node)
                .is_none_or(|e| e.ty != float)
            || !matches!(self.operation_fact(self.calls, file, node).map(|call| &call.outcome),
                Some(crate::CallOutcome::Resolved { parameters, return_type, .. })
                if parameters.as_slice() == [float.clone(), float.clone()] && return_type == &float)
            || children.iter().any(|value| {
                self.expression_type(file, value).is_none_or(|e| {
                    !e.ty.known()
                        || optional(&e.ty)
                        || e.ty.instantiation != Instantiation::Parameter
                        || !matches!(e.ty.kind, TypeKind::Int | TypeKind::Float)
                })
            })
        {
            return None;
        }
        if self
            .expression_type(file, divisor)
            .is_some_and(|e| e.ty.kind == TypeKind::Int)
            && expression_integer(self.context, self.bindings, file, divisor)
                .is_ok_and(|value| value == Some(0))
        {
            return Some("Float division by closed integer zero is unsupported".into());
        }
        let mut divisor = *divisor;
        while divisor.kind() == NodeKind::ParenthesizedExpression {
            divisor = divisor.child_nodes().next()?;
        }
        let tokens = crate::domains::tokens(&self.context.files[file].parsed, divisor);
        if divisor.kind() == NodeKind::Expression
            && matches!(tokens.as_slice(), [token] if token.kind == TokenKind::FloatLiteral)
            && self.context.files[file].parsed.source()[tokens[0].range.clone()]
                .split(['e', 'E'])
                .next()
                .is_some_and(|mantissa| mantissa.bytes().all(|b| matches!(b, b'0' | b'.')))
        {
            return Some("Float division by literal zero is unsupported".into());
        }
        None
    }
    pub(crate) fn check(
        &self,
        file: FileId,
        node: &SyntaxNode,
        eager_only: bool,
        follow_collections: bool,
        type_branches: bool,
    ) -> Option<String> {
        // Both scans must omit the same literal-inactive body, including its references.
        fn source_children<'b>(
            inspector: &ClosedSourceErrors<'_>,
            file: FileId,
            node: &'b SyntaxNode,
            type_branches: bool,
        ) -> Vec<&'b SyntaxNode> {
            let children: Vec<_> = node.child_nodes().collect();
            if !type_branches || node.kind() != NodeKind::ConditionalExpression {
                return children;
            }
            let [then, otherwise] = children.as_slice() else {
                return children;
            };
            let first: Vec<_> = then.child_nodes().collect();
            let last: Vec<_> = otherwise.child_nodes().collect();
            let ([guard, body], [fallback]) = (first.as_slice(), last.as_slice()) else {
                return children;
            };
            let parsed = &inspector.context.files[file].parsed;
            if then.kind() != NodeKind::ConditionalBranch
                || otherwise.kind() != NodeKind::ElseBranch
                || guard.kind() != NodeKind::Expression
                || ![node, *then, *otherwise, *guard].iter().all(|source| {
                    crate::definitions::annotations_safe(inspector.context, file, source)
                })
                || !matches!(crate::domains::tokens(parsed, guard).as_slice(),
                    [token] if matches!(token.kind, TokenKind::True | TokenKind::False))
                || inspector
                    .expression_type(file, guard)
                    .is_none_or(|expression| {
                        expression.ty
                            != TypeInst {
                                instantiation: Instantiation::Parameter,
                                optional: false,
                                kind: TypeKind::Bool,
                            }
                    })
            {
                return children;
            }
            match inspector.assertion_literal(file, guard) {
                Some(TokenKind::True) => vec![*guard, *body],
                Some(TokenKind::False) => vec![*guard, *fallback],
                _ => children,
            }
        }
        fn literal_parts(
            inspector: &ClosedSourceErrors<'_>,
            file: FileId,
            node: &SyntaxNode,
            eager_only: bool,
            type_branches: bool,
        ) -> Result<bool, String> {
            let context = inspector.context;
            let bindings = inspector.bindings;
            if eager_only
                && (node.kind() == NodeKind::ConditionalExpression
                    || node.kind() == NodeKind::BinaryExpression
                        && matches!(
                            operator(context, file, node)
                                .and_then(crate::bindings::symbolic_operator),
                            Some("/\\" | "\\/" | "<->" | "->" | "<-" | "default")
                        ))
            {
                return Err("lazy initialized arithmetic inspection is unavailable".into());
            }
            if inspector.false_integer_assertion(file, node) {
                return Err("false assertion condition aborts evaluation".into());
            }
            if let Some(reason) = inspector.float_divisor_error(file, node) {
                return Err(reason);
            }
            let mut children = Vec::new();
            for child in source_children(inspector, file, node, type_branches) {
                children.push((
                    child,
                    literal_parts(inspector, file, child, eager_only, type_branches)?,
                ));
            }
            let literal = match node.kind() {
                NodeKind::Expression => {
                    matches!(crate::domains::tokens(&context.files[file].parsed, node).as_slice(),
                    [token] if token.kind == TokenKind::IntegerLiteral)
                }
                NodeKind::ParenthesizedExpression => children.len() == 1 && children[0].1,
                NodeKind::UnaryExpression => {
                    children.len() == 1
                        && children[0].1
                        && matches!(
                            operator(context, file, node),
                            Some(TokenKind::Plus | TokenKind::Minus)
                        )
                }
                NodeKind::BinaryExpression => {
                    children.len() == 2
                        && children.iter().all(|(_, literal)| *literal)
                        && matches!(
                            operator(context, file, node),
                            Some(
                                TokenKind::Plus
                                    | TokenKind::Minus
                                    | TokenKind::Star
                                    | TokenKind::Div
                                    | TokenKind::Mod
                            )
                        )
                }
                _ => false,
            };
            if node.kind() == NodeKind::BinaryExpression
                && children.len() == 2
                && children[1].1
                && matches!(
                    operator(context, file, node),
                    Some(TokenKind::Div | TokenKind::Mod)
                )
                && crate::domains::invariant_expression_integer(
                    context,
                    bindings,
                    file,
                    children[1].0,
                )? == Some(0)
            {
                return Err("integer division by zero".into());
            }
            if !literal {
                for (child, literal) in children {
                    if literal {
                        crate::domains::invariant_expression_integer(
                            context, bindings, file, child,
                        )?;
                    }
                }
            }
            Ok(literal)
        }
        let mut sources = Vec::new();
        let mut pending = vec![(file, node)];
        while let Some((file, root)) = pending.pop() {
            match literal_parts(self, file, root, eager_only, type_branches) {
                Ok(true) => {
                    if let Err(reason) = crate::domains::invariant_expression_integer(
                        self.context,
                        self.bindings,
                        file,
                        root,
                    ) {
                        return Some(reason);
                    }
                }
                Err(reason) => return Some(reason),
                Ok(false) => {}
            }
            let mut nodes = vec![root];
            while let Some(node) = nodes.pop() {
                nodes.extend(source_children(self, file, node, type_branches));
                if node.kind() == NodeKind::Expression
                    && let Some(id) = self.reference(file, node)
                    && !sources.contains(&id)
                {
                    let declaration = &self.bindings.declarations[id.0];
                    let ty = &self.calls.declarations[id.0].ty;
                    if (declaration.top_level && declaration.role == crate::DeclarationRole::Value
                        || follow_collections && declaration.role == crate::DeclarationRole::Local)
                        && ty.known()
                        && (!optional(ty)
                            && ty.instantiation == Instantiation::Parameter
                            && matches!(ty.kind, TypeKind::Int | TypeKind::Float)
                            || follow_collections
                                && matches!(ty.kind, TypeKind::Set(_) | TypeKind::Array { .. }))
                        && let Some(written) = crate::callables::find_node(
                            self.context.files[declaration.file].parsed.tree(),
                            &declaration.syntax_range,
                            declaration.role,
                        )
                    {
                        sources.push(id);
                        pending.push((declaration.file, written));
                    }
                }
            }
        }
        None
    }
}

pub(crate) fn operator(
    context: &ModelContext,
    file: FileId,
    node: &SyntaxNode,
) -> Option<TokenKind> {
    node.children().iter().find_map(|c| {
        if let SyntaxElement::Token(i) = c {
            let kind = context.files[file].parsed.tokens()[*i].kind;
            crate::bindings::symbolic_operator(kind).map(|_| kind)
        } else {
            None
        }
    })
}

/// Check supported value safety without running lint or evaluating model code.
/// The node, file and prerequisite facts must belong to this same ModelContext.
/// Typed integer selections and comprehensions retain Unknown without proving
/// membership or iteration safety. This is not a general definedness proof.
pub fn expression_safety(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::CallableFacts,
    file: FileId,
    node: &SyntaxNode,
) -> DefinitionSafety {
    expression_safety_view(context, bindings, &calls.view(), file, node)
}
