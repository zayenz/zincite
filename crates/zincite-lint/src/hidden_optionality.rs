//! Count-intent advice over optional facts. A bound does not prove absence is
//! attainable or a constraint is wrong; this policy never proposes a count fix.
use crate::callables::core_operation;
use crate::definitions::resolved_reference;
use crate::domains::tokens;
use crate::{
    BindingFacts, CallableFacts, Cardinality, CollectionCardinality, FileFinding, FileId,
    GuardActivation, GuardedFacts, GuardedOutcome, Instantiation, InstantiationFacts, ModelContext,
    OptionalConditionKind, OptionalFacts, Presence, Rule, Severity, SourceDiagnostic, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

pub(super) fn check_hidden_optionality(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    optional: &OptionalFacts,
    guarded: &GuardedFacts,
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
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
            if suppressions[item].contains(&Rule::HiddenOptionality) {
                continue;
            }
            let mut comparisons = Vec::new();
            collect_comparisons(&source.parsed, node, &mut comparisons);
            for comparison in comparisons {
                let comparison_location = source.location(comparison.range());
                if guarded
                    .expression(file, &comparison_location)
                    .is_some_and(|e| e.context.activation == GuardActivation::Inactive)
                {
                    continue;
                }
                let operands: Vec<_> = comparison.child_nodes().collect();
                for operand in &operands {
                    let length = unwrap(operand);
                    if length.kind() != NodeKind::CallExpression
                        || !tokens(&source.parsed, length).first().is_some_and(|t| {
                            matches!(t.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)
                                && source.parsed.source()[t.range.clone()].trim_matches('\'')
                                    == "length"
                        })
                    {
                        continue;
                    }
                    let mut arguments = length.child_nodes();
                    let Some(argument) = arguments.next() else {
                        continue;
                    };
                    if arguments.next().is_some() {
                        continue;
                    }
                    let argument = unwrap(argument);
                    let location = source.location(
                        tokens(&source.parsed, length).first().unwrap().range.start
                            ..length.range().end,
                    );
                    let argument_location = source.location(argument.range());
                    let counts = optional.collection(file, &argument_location).or_else(|| {
                        (argument.kind() == NodeKind::Expression)
                            .then(|| resolved_reference(context, bindings, file, argument))
                            .flatten()
                            .and_then(|id| optional.declaration(id))
                            .and_then(|d| d.collection.as_ref())
                    });
                    let Some(counts) = counts else {
                        continue;
                    };
                    if counts.capacity == Cardinality::Exact(0)
                        || counts.element_presence == Presence::Present
                        || matches!((&counts.capacity,&counts.present),(Cardinality::Exact(a),Cardinality::Exact(b)) if a==b)
                    {
                        continue;
                    }
                    let gates: Vec<_> = counts
                        .conditions
                        .iter()
                        .filter(|condition| {
                            matches!(
                                condition.kind,
                                OptionalConditionKind::Membership { .. }
                                    | OptionalConditionKind::Filter
                            ) && instantiations.expressions.iter().any(|e| {
                                e.file == condition.file
                                    && e.location.range == condition.location.range
                                    && e.instantiation == Instantiation::Decision
                            })
                        })
                        .collect();
                    let known_mismatch = matches!(
                        argument.kind(),
                        NodeKind::ArrayComprehension | NodeKind::IndexedArrayComprehension
                    ) && matches!((counts.capacity.bounds(),counts.present.bounds()),(Some((capacity,_)),Some((_,present))) if present<capacity);
                    if gates.is_empty() && !known_mismatch {
                        continue;
                    }
                    if gates.iter().all(|c| {
                        matches!(c.kind, OptionalConditionKind::Filter)
                            && guarded.expression(c.file, &c.location).is_some_and(|e| {
                                e.definedness == GuardedOutcome::Proven
                                    && e.truth == Some(GuardedOutcome::Proven)
                            })
                    }) && !gates.is_empty()
                        && head_present(context, optional, guarded, file, argument, counts)
                    {
                        continue;
                    }
                    let length_identity =
                        core_operation(context, bindings, calls, file, length, "length");
                    if length_identity == Ok(false) {
                        continue;
                    }
                    let op = tokens(&source.parsed, comparison)
                        .first()
                        .and_then(|t| crate::bindings::symbolic_operator(t.kind))
                        .unwrap();
                    let comparison_identity =
                        core_operation(context, bindings, calls, file, comparison, op);
                    if comparison_identity == Ok(false) {
                        continue;
                    }
                    let error = length_identity
                        .err()
                        .or_else(|| comparison_identity.err())
                        .or_else(|| unsupported(counts));
                    if let Some(reason) = error {
                        limitations.push(SourceDiagnostic {
                            location,
                            message: format!("hidden-optionality: {reason}"),
                        });
                        continue;
                    }
                    let integer_comparison = calls.expressions.iter().any(|e| {
                        e.file == file
                            && e.location.range == comparison_location.range
                            && e.ty.kind == TypeKind::Bool
                            && !e.ty.optional
                    }) && operands.iter().all(|operand| {
                        calls.expressions.iter().any(|e| {
                            e.file == file
                                && e.location.range == source.location(operand.range()).range
                                && e.ty.kind == TypeKind::Int
                                && !e.ty.optional
                        })
                    });
                    if !integer_comparison {
                        limitations.push(SourceDiagnostic{location,message:"hidden-optionality: required integer comparison types are unavailable".into()});
                        continue;
                    }
                    if findings.iter().any(|f: &FileFinding| {
                        f.location.path == location.path && f.location.range == location.range
                    }) {
                        continue;
                    }
                    let cause = if gates
                        .iter()
                        .any(|c| matches!(c.kind, OptionalConditionKind::Membership { .. }))
                    {
                        "decision-set membership"
                    } else if !gates.is_empty() {
                        "a decision where filter"
                    } else {
                        "explicitly absent comprehension elements"
                    };
                    findings.push(FileFinding{fix:None,location,rule:Rule::HiddenOptionality,severity:Severity::Warning,message:format!("if this comparison intends to count selected or present elements, length does not measure that count: {cause} gates presence, while length measures array capacity {} when the collection is defined{}",capacity(&counts.capacity),present(&counts.present))});
                }
            }
        }
    }
    limitations.dedup_by(|a, b| {
        a.location.path == b.location.path
            && a.location.range == b.location.range
            && a.message == b.message
    });
    (findings, limitations)
}
fn unwrap(mut node: &SyntaxNode) -> &SyntaxNode {
    while matches!(
        node.kind(),
        NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression | NodeKind::NamedArgument
    ) {
        let Some(child) = node.child_nodes().next() else {
            break;
        };
        node = child;
    }
    node
}
fn collect_comparisons<'a>(
    parsed: &zincite_syntax::ParsedFile,
    node: &'a SyntaxNode,
    out: &mut Vec<&'a SyntaxNode>,
) {
    if node.kind() == NodeKind::BinaryExpression
        && tokens(parsed, node).first().is_some_and(|t| {
            matches!(
                t.kind,
                TokenKind::Equal
                    | TokenKind::DoubleEqual
                    | TokenKind::NotEqual
                    | TokenKind::Less
                    | TokenKind::LessEqual
                    | TokenKind::Greater
                    | TokenKind::GreaterEqual
            )
        })
    {
        out.push(node);
    }
    for child in node.child_nodes() {
        collect_comparisons(parsed, child, out);
    }
}
fn head_present(
    context: &ModelContext,
    optional: &OptionalFacts,
    guarded: &GuardedFacts,
    file: FileId,
    node: &SyntaxNode,
    counts: &CollectionCardinality,
) -> bool {
    let origin = if matches!(
        node.kind(),
        NodeKind::ArrayComprehension | NodeKind::IndexedArrayComprehension
    ) {
        Some((file, node))
    } else {
        // Alias cardinalities retain their original gate locations. Join that
        // source comprehension rather than interpreting the alias initializer.
        counts.conditions.iter().find_map(|condition| {
            let source = &context.files[condition.file];
            comprehension_containing(
                source.parsed.tree(),
                &(condition.location.range.start - source.byte_offset
                    ..condition.location.range.end - source.byte_offset),
            )
            .map(|node| (condition.file, node))
        })
    };
    let Some((file, node)) = origin else {
        return false;
    };
    node.child_nodes()
        .find(|n| n.kind() != NodeKind::GeneratorList)
        .and_then(|head| {
            let head = if head.kind() == NodeKind::IndexedArrayEntry {
                head.child_nodes().last()?
            } else {
                head
            };
            let location = context.files[file].location(head.range());
            Some(
                optional
                    .expression(file, &location)
                    .is_some_and(|e| e.presence == Presence::Present)
                    || guarded
                        .expression(file, &location)
                        .is_some_and(|e| e.presence == Some(Presence::Present)),
            )
        })
        .unwrap_or(false)
}
fn comprehension_containing<'a>(
    node: &'a SyntaxNode,
    range: &std::ops::Range<usize>,
) -> Option<&'a SyntaxNode> {
    if node.range().start > range.start || node.range().end < range.end {
        return None;
    }
    node.child_nodes()
        .find_map(|child| comprehension_containing(child, range))
        .or_else(|| {
            matches!(
                node.kind(),
                NodeKind::ArrayComprehension | NodeKind::IndexedArrayComprehension
            )
            .then_some(node)
        })
}
fn unsupported(counts: &CollectionCardinality) -> Option<String> {
    [&counts.capacity, &counts.present]
        .into_iter()
        .find_map(|c| {
            if let Cardinality::Unsupported(reason) = c {
                Some(reason.clone())
            } else {
                None
            }
        })
        .or_else(|| {
            if let Presence::Unsupported(reason) = &counts.element_presence {
                Some(reason.clone())
            } else {
                None
            }
        })
}
fn capacity(count: &Cardinality) -> String {
    match count {
        Cardinality::Exact(n) => format!("({n} slots)"),
        Cardinality::Bounds { lower, upper } => format!("({lower}..{upper} slots)"),
        _ => "(a count not known here)".into(),
    }
}
fn present(count: &Cardinality) -> String {
    match count {
        Cardinality::Exact(n) => format!("; {n} present elements are known"),
        Cardinality::Bounds { lower, upper } => {
            format!("; present elements have a supported bound of {lower}..{upper}")
        }
        _ => "; the present count remains unknown".into(),
    }
}
