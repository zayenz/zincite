//! Domain advice over declared-domain and complete enforced-definition facts.
use crate::callables::{find_node, is_expression};
use crate::value_safety::optional;
use crate::{
    BindingFacts, CallableFacts, DeclarationId, DeclarationRole, DefinitionCoverage,
    DefinitionEnforcement, DefinitionFacts, DefinitionSafety, Domain, DomainFacts, FileFinding,
    Instantiation, ModelContext, Rule, Severity, SourceDiagnostic, TypeKind,
};

pub(super) struct UnboundedResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}

pub(super) fn check_unbounded_variables(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    domains: &DomainFacts,
    definitions: &DefinitionFacts,
    inspected_locals: &[DeclarationId],
) -> UnboundedResult {
    let mut result = UnboundedResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    let defined = definitions.bounded_or_defined_targets(bindings, domains);
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
