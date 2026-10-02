//! Readability advice for resolved standard element predicates.
use crate::{
    BindingFacts, CallOutcome, CallableFacts, DeclarationRole, FileFinding, ModelContext, Rule,
    Severity, SourceDiagnostic, SourceKind,
};

pub(super) struct ElementResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}

pub(super) fn check_element(
    context: &ModelContext,
    bindings: &BindingFacts,
    facts: &CallableFacts,
    rewrites: &[ElementRewrite],
    snapshots: &mut std::collections::BTreeMap<crate::FileId, crate::SourceSnapshot>,
) -> ElementResult {
    let mut result = ElementResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    for call in &facts.calls {
        if call.symbolic_operator {
            continue;
        }
        let file = &context.files[call.file];
        let Some(suppressed) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled() || suppressed[call.item].contains(&Rule::ElementPredicate) {
            continue;
        }
        let reason = match &call.outcome {
            CallOutcome::Resolved {
                declaration,
                parameters,
                ..
            } => {
                let declaration = &bindings.declarations[declaration.0];
                let source = &context.files[declaration.file];
                if declaration.name == "element"
                    && declaration.role == DeclarationRole::Predicate
                    && parameters.len() == 3
                    && source.kind == SourceKind::StandardLibrary
                    && context.standard_element.as_ref() == Some(&source.canonical_path)
                {
                    let fix = rewrites
                        .iter()
                        .find(|f| f.file == call.file && f.head == call.location)
                        .map(|f| {
                            crate::semantic_fixes::element(
                                crate::semantic_fixes::snapshot(context, call.file, snapshots),
                                &f.call,
                                &f.head,
                                f.open,
                                f.commas,
                                f.close,
                            )
                        });
                    result.findings.push(FileFinding {
                        fix,
                        location: call.location.clone(),
                        rule: Rule::ElementPredicate,
                        severity: Severity::Warning,
                        message: "consider indexing equality (value = array[index]) to make this element constraint easier to read".into(),
                    });
                }
                continue;
            }
            CallOutcome::NoMatch { .. } | CallOutcome::Intrinsic { .. } => continue,
            CallOutcome::Unresolved { reason } | CallOutcome::Unsupported { reason, .. } => {
                reason.as_str()
            }
            CallOutcome::Ambiguous { .. } => "callable selection is ambiguous",
        };
        result.limitations.push(SourceDiagnostic {
            location: call.location.clone(),
            message: format!("element-predicate: {reason} for '{}'", call.name),
        });
    }
    result
}

/// A standard element call whose indexing-equality rewrite obligations hold.
/// This is independent of rule selection/suppression. Absence means the bounded
/// producer has not proved eligibility, not that the rewrite is incorrect.
#[derive(Clone, Debug)]
pub struct ElementRewrite {
    pub file: crate::FileId,
    pub call: crate::SourceLocation,
    pub head: crate::SourceLocation,
    pub arguments: [crate::SourceLocation; 3],
    open: usize,
    commas: [usize; 2],
    close: usize,
}

/// Prove the supported standard relation and prospective indexing precondition
/// from same-context prerequisites. No CST is fabricated, parameter defaults are
/// not substituted, and relational Boolean totality does not prove raw operands.
/// Only a declared one-dimensional array and nonoptional scalar types are
/// supported. Unknown/unsupported operands, overloads or annotation effects
/// withhold edits; existing advice/resolution limitations remain independent.
#[allow(clippy::too_many_arguments)]
pub fn resolve_element_rewrites(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &crate::InstantiationFacts,
    domains: &crate::DomainFacts,
    numeric: &crate::NumericFacts,
    optional: &crate::OptionalFacts,
    guarded: &crate::GuardedFacts,
) -> Vec<ElementRewrite> {
    use crate::callables::{
        core_operation, formal_parameter, instantiated_body, is_expression, operation_fact,
        prospective_core_equality,
    };
    use crate::definitions::resolved_reference;
    use crate::{GuardedOutcome, TypeKind};
    use zincite_syntax::{NodeKind, SyntaxElement, TokenKind};
    let mut result = Vec::new();
    for call in &calls.calls {
        let crate::CallOutcome::Resolved {
            declaration,
            parameters,
            ..
        } = &call.outcome
        else {
            continue;
        };
        let d = &bindings.declarations[declaration.0];
        if call.symbolic_operator
            || d.name != "element"
            || d.role != DeclarationRole::Predicate
            || parameters.len() != 3
            || context.files[d.file].kind != SourceKind::StandardLibrary
            || context.standard_element.as_ref() != Some(&context.files[d.file].canonical_path)
        {
            continue;
        }
        let Some(node) = call_node(context, calls, call.file, &call.location) else {
            continue;
        };
        let args: Vec<_> = node.child_nodes().collect();
        if args.len() != 3
            || args.iter().any(|n| n.kind() == NodeKind::NamedArgument)
            || !annotation_path_safe(
                context,
                call.file,
                context.files[call.file].parsed.tree(),
                node,
            )
            || !tree_annotations_safe(context, call.file, node)
        {
            continue;
        }
        let array_node = unwrap(args[1]);
        if array_node.kind() != NodeKind::Expression {
            continue;
        }
        let Some(array) = resolved_reference(context, bindings, call.file, array_node) else {
            continue;
        };
        let argument_types: Vec<_> = args
            .iter()
            .filter_map(|arg| {
                calls
                    .expressions
                    .iter()
                    .find(|e| {
                        e.file == call.file
                            && e.location.range
                                == context.files[call.file].location(arg.range()).range
                    })
                    .map(|e| e.ty.clone())
            })
            .collect();
        let [index_ty, array_ty, value_ty] = argument_types.as_slice() else {
            continue;
        };
        let TypeKind::Array { indices, element } = &array_ty.kind else {
            continue;
        };
        let [index_set_ty] = indices.as_slice() else {
            continue;
        };
        if [index_ty, array_ty, value_ty, element.as_ref(), index_set_ty]
            .iter()
            .any(|t| t.optional || !t.known())
            || !matches!(
                element.kind,
                TypeKind::Bool | TypeKind::Int | TypeKind::Float | TypeKind::Enum(_)
            )
            || !matches!(
                (&index_ty.kind, &index_set_ty.kind),
                (TypeKind::Int, TypeKind::Int)
            ) && index_ty.kind != index_set_ty.kind
            || !crate::types::coerces(array_ty, &parameters[1])
            || !crate::types::coerces(value_ty, &parameters[2])
        {
            continue;
        }
        if args.iter().any(|arg| {
            guarded
                .expression(call.file, &context.files[call.file].location(arg.range()))
                .is_none_or(|e| e.raw_definedness != GuardedOutcome::Proven)
        }) {
            continue;
        }
        let Some(call_context) =
            guarded.expression(call.file, &context.files[call.file].location(node.range()))
        else {
            continue;
        };
        if crate::guarded::prospective_index_membership(
            context,
            bindings,
            calls,
            instantiations,
            domains,
            numeric,
            optional,
            call_context,
            args[0],
            array,
        ) != GuardedOutcome::Proven
        {
            continue;
        }
        if !prospective_core_equality(
            context,
            bindings,
            calls,
            call.file,
            &[value_ty.clone(), *element.clone()],
        ) {
            continue;
        }
        let Some(declaration_node) = crate::callables::find_node(
            context.files[d.file].parsed.tree(),
            &d.syntax_range,
            d.role,
        ) else {
            continue;
        };
        let Some(body) = declaration_node
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .last()
            .map(unwrap)
        else {
            continue;
        };
        let children: Vec<_> = body.child_nodes().collect();
        if body.kind() != NodeKind::BinaryExpression
            || children.len() != 2
            || !tree_annotations_safe(context, d.file, declaration_node)
        {
            continue;
        }
        let access = unwrap(children[1]);
        let access_args: Vec<_> = access.child_nodes().collect();
        if access.kind() != NodeKind::ArrayAccessExpression || access_args.len() != 2 {
            continue;
        }
        if [children[0], access_args[0], access_args[1]]
            .iter()
            .zip([2, 1, 0])
            .any(|(node, position)| {
                resolved_reference(context, bindings, d.file, unwrap(node))
                    != formal_parameter(context, bindings, *declaration, position)
            })
        {
            continue;
        }
        let body_facts = instantiated_body(context, bindings, calls, *declaration, parameters);
        if core_operation(context, bindings, &body_facts, d.file, body, "=") != Ok(true) {
            continue;
        }
        let delimiters: Vec<_> = node
            .children()
            .iter()
            .filter_map(|child| {
                if let SyntaxElement::Token(index) = child {
                    let token = &context.files[call.file].parsed.tokens()[*index];
                    matches!(
                        token.kind,
                        TokenKind::LeftParen | TokenKind::Comma | TokenKind::RightParen
                    )
                    .then_some(token)
                } else {
                    None
                }
            })
            .collect();
        let [open, first, second, close] = delimiters.as_slice() else {
            continue;
        };
        if [open.kind, first.kind, second.kind, close.kind]
            != [
                TokenKind::LeftParen,
                TokenKind::Comma,
                TokenKind::Comma,
                TokenKind::RightParen,
            ]
        {
            continue;
        }
        let offset = context.files[call.file].byte_offset;
        if operation_fact(context, calls, call.file, node).is_none() {
            continue;
        }
        result.push(ElementRewrite {
            file: call.file,
            call: context.files[call.file].location(node.range()),
            head: call.location.clone(),
            arguments: std::array::from_fn(|i| context.files[call.file].location(args[i].range())),
            open: open.range.start + offset,
            commas: [first.range.start + offset, second.range.start + offset],
            close: close.range.start + offset,
        });
    }
    result
}
fn unwrap(mut node: &zincite_syntax::SyntaxNode) -> &zincite_syntax::SyntaxNode {
    while node.kind() == zincite_syntax::NodeKind::ParenthesizedExpression
        && node.child_nodes().count() == 1
    {
        node = node.child_nodes().next().unwrap();
    }
    node
}
fn call_node<'a>(
    context: &'a ModelContext,
    calls: &CallableFacts,
    file: crate::FileId,
    head: &crate::SourceLocation,
) -> Option<&'a zincite_syntax::SyntaxNode> {
    fn find<'a>(
        context: &ModelContext,
        calls: &CallableFacts,
        file: crate::FileId,
        node: &'a zincite_syntax::SyntaxNode,
        head: &crate::SourceLocation,
    ) -> Option<&'a zincite_syntax::SyntaxNode> {
        if node.kind() == zincite_syntax::NodeKind::CallExpression
            && crate::callables::operation_fact(context, calls, file, node)
                .is_some_and(|c| c.location.range == head.range)
        {
            return Some(node);
        }
        node.child_nodes()
            .find_map(|c| find(context, calls, file, c, head))
    }
    find(
        context,
        calls,
        file,
        context.files[file].parsed.tree(),
        head,
    )
}
fn tree_annotations_safe(
    context: &ModelContext,
    file: crate::FileId,
    node: &zincite_syntax::SyntaxNode,
) -> bool {
    crate::definitions::annotations_safe(context, file, node)
        && node
            .child_nodes()
            .all(|c| tree_annotations_safe(context, file, c))
}
fn annotation_path_safe(
    context: &ModelContext,
    file: crate::FileId,
    node: &zincite_syntax::SyntaxNode,
    target: &zincite_syntax::SyntaxNode,
) -> bool {
    if node.range().start > target.range().start || node.range().end < target.range().end {
        return true;
    }
    crate::definitions::annotations_safe(context, file, node)
        && node
            .child_nodes()
            .all(|c| annotation_path_safe(context, file, c, target))
}
