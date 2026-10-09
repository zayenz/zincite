//! Boolean use contexts and argument dependencies of resolved standard globals.
use std::collections::HashMap;
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

use crate::callables::{is_expression, operation_head_start};
use crate::{
    BindingFacts, CallFact, CallOutcome, CallableFacts, DeclarationId, FileFinding, FileId,
    Instantiation, InstantiationFacts, ModelContext, Rule, Severity, SourceDiagnostic, SourceKind,
    SourceLocation, TypeKind,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobalUseOutcome {
    Enforced,
    Parameter,
    DecisionDependent,
    Unsupported(String),
}

#[derive(Clone, Debug)]
pub struct GlobalUse {
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    /// None when callable selection could not establish a unique global identity.
    pub declaration: Option<DeclarationId>,
    pub outcome: GlobalUseOutcome,
}

#[derive(Debug, Default)]
pub struct GlobalUseFacts {
    pub uses: Vec<GlobalUse>,
}

/// Interpret Boolean call contexts without lint policy. All supplied facts must
/// belong to this retained ModelContext. Included standard Boolean callables are
/// globals; core implicit declarations and user declarations are distinct.
/// An uncertain selection retains no declaration ID and an Unsupported outcome.
/// Parameter-only actual arguments do not become decisions merely because a
/// predicate's declared return is var bool. This does not evaluate parameter data.
pub fn resolve_global_uses(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
) -> GlobalUseFacts {
    let mut expression_indices = HashMap::with_capacity(calls.expressions.len());
    for (row, expression) in calls.expressions.iter().enumerate() {
        expression_indices
            .entry((
                expression.file,
                expression.location.range.start,
                expression.location.range.end,
            ))
            .or_insert(row);
    }
    let mut call_indices = HashMap::with_capacity(calls.calls.len());
    for (row, call) in calls.calls.iter().enumerate() {
        call_indices
            .entry((call.file, call.location.range.start))
            .or_insert(row);
    }
    let mut walker = Walker {
        context,
        bindings,
        calls,
        instantiations,
        expression_indices,
        call_indices,
        facts: GlobalUseFacts::default(),
    };
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() || source.suppressions.is_none() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            walker.walk(file, item, node, Some(node.kind() == NodeKind::Constraint));
        }
    }
    walker.facts
}

struct Walker<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    expression_indices: HashMap<(FileId, usize, usize), usize>,
    call_indices: HashMap<(FileId, usize), usize>,
    facts: GlobalUseFacts,
}
impl Walker<'_> {
    fn expression_kind(&self, file: FileId, node: &SyntaxNode) -> Option<&TypeKind> {
        let location = self.context.files[file].location(node.range());
        self.expression_indices
            .get(&(file, location.range.start, location.range.end))
            .map(|row| &self.calls.expressions[*row].ty.kind)
    }
    fn operation_fact(&self, file: FileId, node: &SyntaxNode) -> Option<&CallFact> {
        let start = operation_head_start(self.context, file, node)?;
        self.call_indices
            .get(&(file, start))
            .map(|row| &self.calls.calls[*row])
    }
    fn value_context(
        &self,
        file: FileId,
        node: &SyntaxNode,
        enclosing: Option<bool>,
    ) -> Option<bool> {
        if node.kind() == NodeKind::NamedArgument {
            return node
                .child_nodes()
                .next()
                .and_then(|value| self.value_context(file, value, enclosing));
        }
        if !is_expression(node.kind()) {
            return enclosing;
        }
        // Boolean operands are values; known non-Boolean expressions do not
        // introduce a Boolean scope for constraints floating out of a let.
        match self.expression_kind(file, node) {
            Some(TypeKind::Bool) => Some(false),
            Some(TypeKind::Unknown(_) | TypeKind::Variable { .. } | TypeKind::Bottom) | None => {
                None
            }
            _ => enclosing,
        }
    }
    fn call_location(&self, file: FileId, node: &SyntaxNode) -> SourceLocation {
        let range = node.range();
        let tokens: Vec<_> = self.context.files[file]
            .parsed
            .tokens()
            .iter()
            .filter(|token| {
                !matches!(
                    token.kind,
                    TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                ) && token.range.start >= range.start
                    && token.range.end <= range.end
            })
            .collect();
        self.context.files[file]
            .location(tokens.first().unwrap().range.start..tokens.last().unwrap().range.end)
    }
    fn uncertain_selection(&self, file: FileId, node: &SyntaxNode) -> Option<String> {
        let call = self.operation_fact(file, node)?;
        let (candidates, reason) = match &call.outcome {
            CallOutcome::Unsupported { candidates, reason } => (candidates, reason.as_str()),
            CallOutcome::Ambiguous { candidates } => {
                (candidates, "global callable selection is ambiguous")
            }
            CallOutcome::Unresolved { reason } => return Some(reason.clone()),
            _ => return None,
        };
        candidates
            .iter()
            .any(|id| {
                let source = &self.context.files[self.bindings.declarations[id.0].file];
                source.kind == SourceKind::StandardLibrary
                    && !source.implicit
                    && self
                        .calls
                        .signatures
                        .iter()
                        .find(|s| s.declaration == *id)
                        .is_none_or(|s| {
                            matches!(s.return_type.kind, TypeKind::Bool | TypeKind::Unknown(_))
                        })
            })
            .then(|| reason.to_owned())
    }
    fn instantiation(&self, file: FileId, node: &SyntaxNode) -> Instantiation {
        let location = self.context.files[file].location(node.range());
        self.instantiations
            .expressions
            .iter()
            .find(|fact| fact.file == file && fact.location.range == location.range)
            .map_or(Instantiation::Unknown, |fact| fact.instantiation)
    }
    fn global(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let CallOutcome::Resolved {
            declaration,
            return_type,
            ..
        } = &self.operation_fact(file, node)?.outcome
        else {
            return None;
        };
        let source = &self.context.files[self.bindings.declarations[declaration.0].file];
        (return_type.kind == TypeKind::Bool
            && source.kind == SourceKind::StandardLibrary
            && !source.implicit)
            .then_some(*declaration)
    }
    fn dependency(&self, file: FileId, node: &SyntaxNode) -> Instantiation {
        if self.global(file, node).is_none() {
            return self.instantiation(file, node);
        }
        if self.operation_fact(file, node).is_some_and(|call| matches!(&call.outcome, CallOutcome::Resolved { return_type, .. } if return_type.instantiation == Instantiation::Parameter)) {
            return Instantiation::Parameter;
        }
        let arguments: Vec<_> = node.child_nodes().collect();
        if arguments.is_empty() {
            return Instantiation::Unknown;
        }
        // An omitted default in a standard body has no retained expression
        // instantiation here; keep its dependency unknown rather than guessing.
        let mut unknown = node.kind() == NodeKind::CallExpression && self.operation_fact(file, node).is_some_and(|call| matches!(&call.outcome, CallOutcome::Resolved { parameters, .. } if parameters.len() > arguments.len()));
        for argument in arguments {
            // Named arguments retain their value expression as a child.
            let argument = if argument.kind() == NodeKind::NamedArgument {
                argument.child_nodes().next().unwrap_or(argument)
            } else {
                argument
            };
            let dependency = if is_expression(argument.kind()) {
                self.dependency(file, argument)
            } else {
                self.generator_dependency(file, argument)
            };
            match dependency {
                Instantiation::Decision => return Instantiation::Decision,
                Instantiation::Unknown => unknown = true,
                Instantiation::Parameter => {}
            }
        }
        if unknown {
            Instantiation::Unknown
        } else {
            Instantiation::Parameter
        }
    }
    fn generator_dependency(&self, file: FileId, node: &SyntaxNode) -> Instantiation {
        if is_expression(node.kind()) {
            return self.instantiation(file, node);
        }
        let mut unknown = false;
        for child in node.child_nodes() {
            match self.generator_dependency(file, child) {
                Instantiation::Decision => return Instantiation::Decision,
                Instantiation::Unknown => unknown = true,
                Instantiation::Parameter => {}
            }
        }
        if unknown {
            Instantiation::Unknown
        } else {
            Instantiation::Parameter
        }
    }
    fn walk(&mut self, file: FileId, item: usize, node: &SyntaxNode, enforced: Option<bool>) {
        if matches!(
            node.kind(),
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
        ) {
            self.walk_call(file, item, node, enforced);
            return;
        }
        if node.kind() == NodeKind::ConditionalExpression {
            self.walk_conditional(file, item, node, enforced);
            return;
        }
        let children: Vec<_> = node.child_nodes().collect();
        if node.kind() == NodeKind::AnnotatedExpression {
            for (index, child) in children.into_iter().enumerate() {
                self.walk(
                    file,
                    item,
                    child,
                    if index == 0 { enforced } else { Some(false) },
                );
            }
            return;
        }
        if node.kind() == NodeKind::Declaration {
            for child in children {
                let initializer_context = if is_expression(child.kind()) {
                    // A Boolean initializer has its own value context. Numeric
                    // local initializers retain the surrounding Boolean context
                    // for constraints floating out of a nested let.
                    self.value_context(file, child, enforced)
                } else {
                    Some(false)
                };
                self.walk(file, item, child, initializer_context);
            }
            return;
        }
        let conjunction = node.kind() == NodeKind::BinaryExpression && node.children().iter().any(|element| matches!(element, SyntaxElement::Token(index) if self.context.files[file].parsed.tokens()[*index].kind == TokenKind::And));
        let child_enforced = match node.kind() {
            // Only the top-level constraint introduces enforcement. Let-local
            // constraints float to their nearest enclosing Boolean context.
            NodeKind::Constraint | NodeKind::LetExpression | NodeKind::LetBlock => enforced,
            NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression => enforced,
            NodeKind::BinaryExpression if conjunction => {
                match self.operation_fact(file, node).map(|fact| &fact.outcome) {
                    Some(CallOutcome::Resolved { declaration, .. })
                        if crate::definitions::core_callable(
                            self.context,
                            self.bindings,
                            *declaration,
                            "/\\",
                        ) =>
                    {
                        enforced
                    }
                    Some(CallOutcome::Resolved { .. } | CallOutcome::Intrinsic { .. }) => {
                        Some(false)
                    }
                    _ if enforced == Some(false) => Some(false),
                    _ => None,
                }
            }
            _ => {
                for child in children {
                    self.walk(file, item, child, self.value_context(file, child, enforced));
                }
                return;
            }
        };
        for child in children {
            self.walk(file, item, child, child_enforced);
        }
    }
    fn walk_call(&mut self, file: FileId, item: usize, node: &SyntaxNode, enforced: Option<bool>) {
        let children: Vec<_> = node.child_nodes().collect();
        if let Some(declaration) = self.global(file, node) {
            let outcome = if enforced == Some(true) {
                GlobalUseOutcome::Enforced
            } else if enforced.is_none() {
                GlobalUseOutcome::Unsupported("Boolean enforcement context is unknown".into())
            } else {
                match self.dependency(file, node) {
                    Instantiation::Parameter => GlobalUseOutcome::Parameter,
                    Instantiation::Decision => GlobalUseOutcome::DecisionDependent,
                    Instantiation::Unknown => GlobalUseOutcome::Unsupported(
                        "global argument dependency is unknown".into(),
                    ),
                }
            };
            self.facts.uses.push(GlobalUse {
                file,
                item,
                declaration: Some(declaration),
                location: self.call_location(file, node),
                outcome,
            });
        }
        if enforced != Some(true)
            && let Some(reason) = self.uncertain_selection(file, node)
        {
            self.facts.uses.push(GlobalUse {
                file,
                item,
                declaration: None,
                location: self.call_location(file, node),
                outcome: GlobalUseOutcome::Unsupported(reason),
            });
        }
        let core_forall = self.operation_fact(file, node).is_some_and(|call| {
            let CallOutcome::Resolved { declaration, .. } = &call.outcome else {
                return false;
            };
            let declaration = &self.bindings.declarations[declaration.0];
            let source = &self.context.files[declaration.file];
            declaration.name == "forall"
                && source.kind == SourceKind::StandardLibrary
                && source.implicit
        });
        if core_forall {
            let mut argument = children.first().copied();
            while let Some(wrapper) = argument.filter(|n| {
                matches!(
                    n.kind(),
                    NodeKind::ParenthesizedExpression | NodeKind::NamedArgument
                )
            }) {
                argument = wrapper.child_nodes().next();
            }
            let quantified = if node.kind() == NodeKind::GeneratorCallExpression {
                Some(node)
            } else {
                argument.filter(|n| n.kind() == NodeKind::ArrayComprehension)
            };
            if let Some(quantified) = quantified {
                let generators = quantified
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList);
                let body_context = match generators.map(|g| self.generator_dependency(file, g)) {
                    Some(Instantiation::Parameter) => enforced,
                    Some(Instantiation::Decision) => Some(false),
                    _ if enforced == Some(false) => Some(false),
                    _ => None,
                };
                for child in quantified.child_nodes() {
                    self.walk(
                        file,
                        item,
                        child,
                        if child.kind() == NodeKind::GeneratorList {
                            Some(false)
                        } else {
                            body_context
                        },
                    );
                }
                return;
            }
            // A literal Boolean array is also an enforced conjunction.
            if let Some(array) = argument.filter(|n| n.kind() == NodeKind::ArrayLiteral) {
                for child in array.child_nodes() {
                    self.walk(file, item, child, enforced);
                }
                return;
            }
        }
        for child in children {
            self.walk(file, item, child, self.value_context(file, child, enforced));
        }
    }
    fn walk_conditional(
        &mut self,
        file: FileId,
        item: usize,
        node: &SyntaxNode,
        enforced: Option<bool>,
    ) {
        let children: Vec<_> = node.child_nodes().collect();
        let guards: Vec<_> = children
            .iter()
            .filter(|n| n.kind() == NodeKind::ConditionalBranch)
            .filter_map(|branch| branch.child_nodes().next())
            .map(|guard| self.instantiation(file, guard))
            .collect();
        let branch_context = if matches!(
            self.expression_kind(file, node),
            None | Some(TypeKind::Unknown(_) | TypeKind::Variable { .. } | TypeKind::Bottom)
        ) {
            None
        } else if guards.contains(&Instantiation::Decision) {
            Some(false)
        } else if guards.contains(&Instantiation::Unknown) && enforced != Some(false) {
            None
        } else {
            enforced
        };
        for branch in children {
            for (index, child) in branch.child_nodes().enumerate() {
                let condition = branch.kind() == NodeKind::ConditionalBranch && index == 0;
                self.walk(
                    file,
                    item,
                    child,
                    if condition {
                        Some(false)
                    } else {
                        branch_context
                    },
                );
            }
        }
    }
}

pub(super) fn check_global_uses(
    context: &ModelContext,
    facts: &GlobalUseFacts,
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    for usage in &facts.uses {
        let file = &context.files[usage.file];
        if file
            .suppressions
            .as_ref()
            .is_none_or(|s| s[usage.item].contains(&Rule::ReifiedGlobal))
        {
            continue;
        }
        match &usage.outcome {
            GlobalUseOutcome::DecisionDependent => findings.push(FileFinding { fix: None, location: usage.location.clone(), rule: Rule::ReifiedGlobal, severity: Severity::Warning, message: "this Boolean-valued global use may require reification or half-reification; consider whether an enforced formulation expresses the model more clearly".into() }),
            GlobalUseOutcome::Unsupported(reason) => limitations.push(SourceDiagnostic { location: usage.location.clone(), message: format!("reified-global: {reason}") }),
            _ => {}
        }
    }
    (findings, limitations)
}
