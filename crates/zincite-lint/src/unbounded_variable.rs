//! Domain advice over declared domains, complete definitions and explicit ranges.
use crate::callables::{core_operation, find_node, is_expression};
use crate::definitions::{annotations_safe, complete_array_coverage, resolved_reference};
use crate::value_safety::optional;
use crate::{
    BindingFacts, CallableFacts, DeclarationId, DeclarationRole, DefinitionCoverage,
    DefinitionEnforcement, DefinitionFacts, DefinitionSafety, Domain, DomainFacts, FileFinding,
    FileId, Instantiation, InstantiationFacts, ModelContext, Rule, Severity, SourceDiagnostic,
    TypeInst, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

pub(super) struct UnboundedResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}

pub(super) fn check_unbounded_variables(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    definitions: &DefinitionFacts,
    inspected_locals: &[DeclarationId],
) -> UnboundedResult {
    let mut result = UnboundedResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    let defined = definitions.bounded_or_defined_targets(bindings, domains);
    let ranges = RangeConstraints {
        context,
        bindings,
        calls,
        instantiations,
        domains,
    }
    .collect();
    for declaration in &bindings.declarations {
        let file = &context.files[declaration.file];
        let Some(suppressions) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled()
            || !matches!(
                declaration.role,
                DeclarationRole::Value | DeclarationRole::Local
            )
            || declaration.instantiation != Instantiation::Decision
            || defined.contains(&declaration.id)
            || ranges
                .iter()
                .any(|(id, checked)| *id == declaration.id && checked.is_ok())
            || suppressions[declaration.item].contains(&Rule::UnboundedVariable)
        {
            continue;
        }
        let unbounded = domains.declarations[declaration.id.0]
            .domain
            .has_unbounded_numeric_elements();
        let ty = &calls.declarations[declaration.id.0].ty;
        let unknown_domain = unbounded.is_err()
            && ty.known()
            && !optional(ty)
            && symbolic_parameter_domain(
                context,
                bindings,
                calls,
                &domains.declarations[declaration.id.0].domain,
            );
        let mut limitation = match unbounded {
            Ok(false) => continue,
            Ok(true) => None,
            Err(_) if unknown_domain => None,
            Err(reason) => Some(reason.to_owned()),
        };
        let inspected_initializer = if declaration.role == DeclarationRole::Local
            && inspected_locals.contains(&declaration.id)
            && ty.known()
            && !optional(ty)
            && ty.instantiation == Instantiation::Decision
            && ty.kind == TypeKind::Int
        {
            find_node(
                file.parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .and_then(|local| {
                local
                    .child_nodes()
                    .find(|node| is_expression(node.kind()))
                    .map(|value| {
                        (
                            file.location(local.range()).range,
                            file.location(value.range()).range,
                        )
                    })
            })
        } else {
            None
        };

        let mut unknown_definition = false;
        for definition in definitions
            .definitions
            .iter()
            .filter(|d| d.target == declaration.id)
        {
            if definition.enforcement == DefinitionEnforcement::Conditional {
                continue;
            }
            // Reverse equality candidates can form a cycle around a bounded
            // source. Keep missing safety explicit when dependencies are anchored;
            // a genuine unanchored/self cycle still cannot define this value.
            if definition.cyclic
                && !definition
                    .dependencies
                    .iter()
                    .all(|id| defined.contains(id))
            {
                continue;
            }
            if !matches!(
                definition.enforcement,
                DefinitionEnforcement::Unsupported(_)
            ) && matches!(
                definition.coverage,
                DefinitionCoverage::ArrayElement | DefinitionCoverage::Unproved(_)
            ) {
                continue;
            }
            let reason = match (
                &definition.enforcement,
                &definition.coverage,
                &definition.safety,
            ) {
                (DefinitionEnforcement::Unsupported(reason), _, _)
                | (_, DefinitionCoverage::Unsupported(reason), _) => Some(reason),
                (_, DefinitionCoverage::Scalar, DefinitionSafety::Unsupported(reason))
                    if reason == "definition array access requires an index-membership proof"
                        && inspected_initializer
                            .as_ref()
                            .is_some_and(|(local, value)| {
                                definition.file == declaration.file
                                    && definition.item == declaration.item
                                    && definition.location.range == *local
                                    && definition.value.range == *value
                            }) =>
                {
                    // Full model-let inspection supplies no output when an
                    // initializer is uncertain. Keep that barrier in this advice.
                    unknown_definition = true;
                    None
                }
                (_, _, DefinitionSafety::Unsupported(reason)) => Some(reason),
                (_, _, DefinitionSafety::Unknown(_)) => {
                    unknown_definition = true;
                    None
                }
                _ => None,
            };
            if let Some(reason) = reason {
                limitation = Some(reason.clone());
            }
        }
        if let Some((_, Err(diagnostic))) = ranges
            .iter()
            .find(|(id, checked)| *id == declaration.id && checked.is_err())
        {
            result.limitations.push(diagnostic.clone());
            continue;
        }
        if let Some(reason) = limitation {
            result.limitations.push(SourceDiagnostic {
                location: declaration.location.clone(),
                message: format!("unbounded-variable: {reason} for '{}'", declaration.name),
            });
        } else if !unknown_definition && !unknown_domain {
            result.findings.push(FileFinding { fix: None,
                location: declaration.location.clone(),
                rule: Rule::UnboundedVariable,
                severity: Severity::Warning,
                message: format!("consider an explicit domain for '{}'; its numeric value or array elements have no explicit bounds or proven complete definition", declaration.name),
            });
        }
    }
    result
}

struct RangeConstraints<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
}
impl<'a> RangeConstraints<'a> {
    fn collect(&self) -> Vec<(DeclarationId, Result<(), SourceDiagnostic>)> {
        let mut ranges = Vec::new();
        for (id, file) in self.context.files.iter().enumerate() {
            for item in file.parsed.tree().child_nodes() {
                if item.kind() == NodeKind::Constraint {
                    self.walk(id, item, &[], None, &mut ranges);
                }
            }
        }
        ranges
    }

    fn walk(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        generators: &[&'a SyntaxNode],
        refusal: Option<SourceDiagnostic>,
        ranges: &mut Vec<(DeclarationId, Result<(), SourceDiagnostic>)>,
    ) {
        let refusal = refusal.or_else(|| {
            (!annotations_safe(self.context, file, node)).then(|| {
                self.limitation(
                    file,
                    node,
                    "constraint annotation enforcement is unsupported",
                )
            })
        });
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::Constraint
            | NodeKind::ParenthesizedExpression
            | NodeKind::AnnotatedExpression => {
                for child in children.into_iter().filter(|n| is_expression(n.kind())) {
                    self.walk(file, child, generators, refusal.clone(), ranges);
                }
            }
            NodeKind::LetExpression => {
                // Let bindings do not condition the Boolean body. Required
                // local initializers are checked by the existing source reader.
                for child in children {
                    if child.kind() == NodeKind::LetBlock {
                        for constraint in child
                            .child_nodes()
                            .filter(|n| n.kind() == NodeKind::Constraint)
                        {
                            self.walk(file, constraint, generators, refusal.clone(), ranges);
                        }
                    } else if is_expression(child.kind()) {
                        self.walk(file, child, generators, refusal.clone(), ranges);
                    }
                }
            }
            NodeKind::BinaryExpression if self.operator(file, node) == Some(TokenKind::And) => {
                let refusal = match core_operation(
                    self.context,
                    self.bindings,
                    self.calls,
                    file,
                    node,
                    "/\\",
                ) {
                    Ok(true) => refusal,
                    Ok(false) => return,
                    Err(reason) => Some(self.limitation(file, node, &reason)),
                };
                for child in children {
                    self.walk(file, child, generators, refusal.clone(), ranges);
                }
            }
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression => {
                if core_operation(
                    self.context,
                    self.bindings,
                    self.calls,
                    file,
                    node,
                    "forall",
                ) != Ok(true)
                {
                    return;
                }
                let quantified = if node.kind() == NodeKind::GeneratorCallExpression {
                    node
                } else {
                    let Some(array) = children
                        .first()
                        .copied()
                        .map(parentheses)
                        .filter(|n| n.kind() == NodeKind::ArrayComprehension)
                    else {
                        return;
                    };
                    array
                };
                let Some(list) = quantified
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                else {
                    return;
                };
                let mut scope = generators.to_vec();
                scope.extend(list.child_nodes());
                if let Some(body) = quantified
                    .child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList)
                {
                    self.walk(file, body, &scope, refusal, ranges);
                }
            }
            NodeKind::BinaryExpression if self.operator(file, node) == Some(TokenKind::In) => {
                if let Some((id, checked)) = self.membership(file, node, generators) {
                    ranges.push((id, refusal.map_or(checked, Err)));
                }
            }
            // Implication, branching, disjunction and ordinary callable
            // arguments cannot establish unconditional range enforcement.
            _ => {}
        }
    }

    fn membership(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        generators: &[&'a SyntaxNode],
    ) -> Option<(DeclarationId, Result<(), SourceDiagnostic>)> {
        let children: Vec<_> = node.child_nodes().map(parentheses).collect();
        let [target, range] = children.as_slice() else {
            return None;
        };
        if target.kind() != NodeKind::ArrayAccessExpression
            || range.kind() != NodeKind::RangeExpression
            || self.operator(file, range) != Some(TokenKind::RangeInclusive)
        {
            return None;
        }
        let parts: Vec<_> = target.child_nodes().collect();
        let subject = parentheses(parts.first()?);
        if subject.kind() != NodeKind::Expression {
            return None;
        }
        let id = resolved_reference(self.context, self.bindings, file, subject)?;
        let declaration = &self.bindings.declarations[id.0];
        let ty = &self.calls.declarations[id.0].ty;
        if !matches!(
            declaration.role,
            DeclarationRole::Value | DeclarationRole::Local
        ) || ty.instantiation != Instantiation::Decision
            || optional(ty)
            || !matches!(&ty.kind, TypeKind::Array { element, .. } if element.kind == TypeKind::Int)
        {
            return None;
        }
        match complete_array_coverage(
            self.context,
            self.bindings,
            (self.calls, self.calls),
            self.instantiations,
            self.domains,
            (file, id),
            (&parts[1..], generators),
        ) {
            DefinitionCoverage::WholeArray => {}
            DefinitionCoverage::Unsupported(reason) => {
                return Some((id, Err(self.limitation(file, target, &reason))));
            }
            _ => return None,
        }
        for (operation, name) in [(node, "in"), (*range, "..")] {
            match core_operation(
                self.context,
                self.bindings,
                self.calls,
                file,
                operation,
                name,
            ) {
                Ok(true) => {}
                Ok(false) => return None,
                Err(reason) => return Some((id, Err(self.limitation(file, operation, &reason)))),
            }
        }
        let endpoints: Vec<_> = range.child_nodes().collect();
        if endpoints.len() != 2 {
            return None;
        }
        let sources = std::iter::once((*target, Instantiation::Decision)).chain(
            endpoints
                .into_iter()
                .map(|endpoint| (endpoint, Instantiation::Parameter)),
        );
        for (source, instantiation) in sources {
            let Some(ty) = self.ty(file, source) else {
                return Some((
                    id,
                    Err(self.limitation(file, source, "range source type is unavailable")),
                ));
            };
            if !ty.known() {
                return Some((
                    id,
                    Err(self.limitation(file, source, "range source type is unsupported")),
                ));
            }
            if optional(ty) || ty.instantiation != instantiation || ty.kind != TypeKind::Int {
                return None;
            }
            match crate::callable_definitions::direct_expression_safety(
                self.context,
                self.bindings,
                self.calls,
                self.instantiations,
                self.domains,
                (file, source),
                generators,
            ) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(_) => return None,
                DefinitionSafety::Unsupported(reason) => {
                    return Some((id, Err(self.limitation(file, source, &reason))));
                }
            }
        }
        Some((id, Ok(())))
    }

    fn ty(&self, file: FileId, node: &SyntaxNode) -> Option<&TypeInst> {
        let range = self.context.files[file].location(node.range()).range;
        self.calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
    }

    fn operator(&self, file: FileId, node: &SyntaxNode) -> Option<TokenKind> {
        node.children().iter().find_map(|child| {
            let SyntaxElement::Token(index) = child else {
                return None;
            };
            let kind = self.context.files[file].parsed.tokens()[*index].kind;
            (!matches!(
                kind,
                TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
            ))
            .then_some(kind)
        })
    }

    fn limitation(&self, file: FileId, node: &SyntaxNode, reason: &str) -> SourceDiagnostic {
        SourceDiagnostic {
            location: self.context.files[file].location(node.range()),
            message: format!("unbounded-variable: {reason}"),
        }
    }
}

fn parentheses(mut node: &SyntaxNode) -> &SyntaxNode {
    while node.kind() == NodeKind::ParenthesizedExpression {
        let Some(inner) = node.child_nodes().next() else {
            break;
        };
        node = inner;
    }
    node
}

fn symbolic_parameter_domain(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    domain: &Domain,
) -> bool {
    match domain {
        Domain::Array { element, .. } => {
            symbolic_parameter_domain(context, bindings, calls, element)
        }
        Domain::Named {
            declaration,
            domain,
        } if domain.as_ref() == &Domain::Unknown => {
            let declaration = &bindings.declarations[declaration.0];
            let ty = &calls.declarations[declaration.id.0].ty;
            let supported_element = match &ty.kind {
                TypeKind::Set(element)
                    if element.known()
                        && !optional(element)
                        && element.instantiation == Instantiation::Parameter =>
                {
                    match element.kind {
                        TypeKind::Int => true,
                        TypeKind::Enum(id) => {
                            // A symbolic subset of this plain input enum supplies
                            // no members, extent or numeric interval.
                            let enumeration = &bindings.declarations[id.0];
                            enumeration.top_level
                                && enumeration.role == DeclarationRole::Enum
                                && find_node(
                                    context.files[enumeration.file].parsed.tree(),
                                    &enumeration.syntax_range,
                                    enumeration.role,
                                )
                                .is_some_and(|node| {
                                    node.kind() == zincite_syntax::NodeKind::EnumDeclaration
                                        && node.child_nodes().next().is_none()
                                })
                        }
                        _ => false,
                    }
                }
                _ => false,
            };
            declaration.top_level
                && declaration.role == DeclarationRole::Value
                && declaration.instantiation == Instantiation::Parameter
                && ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Parameter
                && supported_element
                && find_node(
                    context.files[declaration.file].parsed.tree(),
                    &declaration.syntax_range,
                    declaration.role,
                )
                .is_some_and(|node| !node.child_nodes().any(|child| is_expression(child.kind())))
        }
        _ => false,
    }
}
