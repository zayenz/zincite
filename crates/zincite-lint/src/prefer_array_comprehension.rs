//! Direct comprehension advice over standard call signatures and fixed set identity.
use crate::{
    BindingFacts, DeclarationRole, Domain, EditPart, FileFinding, FileId, Fix, FixSafety,
    Instantiation, ModelContext, NumericBound, Rule, Severity, SourceSnapshot, TextEdit, TypeKind,
};
use std::collections::BTreeMap;
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

pub(super) fn check(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::callables::CallableView<'_>,
    snapshots: &mut BTreeMap<FileId, SourceSnapshot>,
) -> Vec<FileFinding> {
    let mut findings = Vec::new();
    let proof = Proof {
        context,
        bindings,
        calls,
    };
    for (file, source) in context.files.iter().enumerate() {
        let Some(suppressions) = &source.suppressions else {
            continue;
        };
        if !source.warnings_enabled() || !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, top) in source.parsed.tree().child_nodes().enumerate() {
            if suppressions[item].contains(&Rule::PreferArrayComprehension) {
                continue;
            }
            let mut pending = vec![top];
            while let Some(node) = pending.pop() {
                if node.kind() == NodeKind::AnnotatedExpression {
                    continue;
                }
                if let Some((comprehension, keys)) = proof.candidate(file, node) {
                    let location = source.location(node.range());
                    let retained = source.location(comprehension.range()).range;
                    let removed_comment = source.parsed.tokens().iter().any(|token| {
                        matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment)
                            && node.range().start <= token.range.start
                            && token.range.end <= node.range().end
                            && !(comprehension.range().start <= token.range.start
                                && token.range.end <= comprehension.range().end)
                    });
                    let fix = (!removed_comment).then(|| {
                        let mut replacement = vec![EditPart::Original(retained.start..retained.start + 1)];
                        if !keys.is_empty() {
                            replacement.push(EditPart::Text(format!("{keys}: ")));
                        }
                        replacement.push(EditPart::Original(retained.start + 1..retained.end));
                        Fix {
                            title: "Replace the array wrapper with a direct comprehension".into(),
                            safety: FixSafety::Safe,
                            applicability: "Matching fixed sets and independent ordered generators preserve every index axis and body evaluation; multidimensional axes are nonempty".into(),
                            snapshot: crate::semantic_fixes::snapshot(context, file, snapshots).clone(),
                            edits: vec![TextEdit { range: location.range.clone(), replacement }],
                        }
                    });
                    findings.push(FileFinding {
                        location,
                        rule: Rule::PreferArrayComprehension,
                        severity: Severity::Warning,
                        message: "Prefer a direct array comprehension with the same index axes"
                            .into(),
                        fix,
                    });
                }
                pending.extend(node.child_nodes());
            }
        }
    }
    findings
}

struct Proof<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
}
impl Proof<'_> {
    fn candidate<'a>(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
    ) -> Option<(&'a SyntaxNode, String)> {
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let id = crate::definitions::resolved_call(self.context, self.calls, file, node)?;
        let rank = match self.bindings.declarations[id.0].name.as_str() {
            "array1d" => 1,
            "array2d" => 2,
            "array3d" => 3,
            _ => return None,
        };
        if !crate::definitions::core_callable(
            self.context,
            self.bindings,
            id,
            &format!("array{rank}d"),
        ) {
            return None;
        }
        let arguments: Vec<_> = node.child_nodes().collect();
        if arguments.len() != rank + 1 {
            return None;
        }
        let comprehension = arguments[rank];
        if comprehension.kind() != NodeKind::ArrayComprehension {
            return None;
        }
        let mut descendants = vec![node];
        while let Some(child) = descendants.pop() {
            if matches!(
                child.kind(),
                NodeKind::Annotation | NodeKind::AnnotatedExpression
            ) {
                return None;
            }
            descendants.extend(child.child_nodes());
        }
        let list = comprehension
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)?;
        let source = &self.context.files[file];
        let mut axes = Vec::new();
        let mut names = Vec::new();
        for generator in list.child_nodes() {
            let children: Vec<_> = generator.child_nodes().collect();
            let [domain] = children.as_slice() else {
                return None;
            };
            if !crate::domains::tokens(&source.parsed, generator)
                .iter()
                .any(|t| t.kind == TokenKind::In)
            {
                return None;
            }
            let declarations: Vec<_> = self
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == generator.range()
                })
                .collect();
            if declarations.len() != crate::domains::generator_slots(&source.parsed, generator) {
                return None;
            }
            // Binding facts are source ordered, including grouped Cartesian slots.
            for declaration in declarations {
                if declaration.name == "_" {
                    return None;
                }
                names.push(
                    source.parsed.source()[declaration.location.range.start - source.byte_offset
                        ..declaration.location.range.end - source.byte_offset]
                        .to_owned(),
                );
                axes.push(*domain);
            }
        }
        if axes.len() != rank {
            return None;
        }
        let mut domains = Vec::new();
        for (argument, generator) in arguments[..rank].iter().zip(axes) {
            let left = self.fixed_set(file, argument)?;
            let right = self.fixed_set(file, generator)?;
            if crate::domains::same_members(&left, &right) != Ok(Some(true)) {
                return None;
            }
            if rank > 1 && !self.nonempty(&left) {
                return None;
            }
            domains.push(left);
        }
        let keys = if rank == 1 && one_based(&domains[0]) {
            String::new()
        } else if rank == 1 {
            names.remove(0)
        } else {
            format!("({})", names.join(","))
        };
        Some((comprehension, keys))
    }

    fn fixed_set(&self, file: FileId, mut node: &SyntaxNode) -> Option<Domain> {
        while node.kind() == NodeKind::ParenthesizedExpression {
            node = node.child_nodes().next()?
        }
        if node.kind() == NodeKind::Expression {
            let id =
                crate::definitions::resolved_reference(self.context, self.bindings, file, node)?;
            let declaration = &self.bindings.declarations[id.0];
            let ty = &self.calls.declarations[id.0].ty;
            if declaration.role == DeclarationRole::Enum {
                return Some(Domain::Enum(id));
            }
            if !matches!(
                declaration.role,
                DeclarationRole::Value | DeclarationRole::Parameter | DeclarationRole::Local
            ) || ty.instantiation != Instantiation::Parameter
                || crate::value_safety::optional(ty)
                || !matches!(&ty.kind, TypeKind::Set(element) if matches!(element.kind, TypeKind::Int | TypeKind::Enum(_)))
            {
                return None;
            }
            let written = crate::callables::find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )?;
            if written
                .child_nodes()
                .any(|n| n.kind() == NodeKind::Annotation)
            {
                return None;
            }
            // Ordinary unannotated initializers are assignments, not overridable defaults.
            // Inspect only direct closed sets; references still retain identity if unknown.
            let known = written
                .child_nodes()
                .find(|n| crate::callables::is_expression(n.kind()))
                .filter(|n| matches!(n.kind(), NodeKind::RangeExpression | NodeKind::SetLiteral))
                .and_then(|n| self.fixed_set(declaration.file, n))
                .unwrap_or(Domain::Unknown);
            return Some(Domain::Named {
                declaration: id,
                domain: Box::new(known),
            });
        }
        let mut pending = vec![node];
        while let Some(child) = pending.pop() {
            if matches!(
                child.kind(),
                NodeKind::CallExpression | NodeKind::AnnotatedExpression | NodeKind::Annotation
            ) {
                return None;
            }
            pending.extend(child.child_nodes());
        }
        if node.kind() == NodeKind::RangeExpression {
            let name = crate::domains::tokens(&self.context.files[file].parsed, node)
                .iter()
                .find_map(|t| crate::bindings::symbolic_operator(t.kind))?;
            if crate::callables::core_operation(
                self.context,
                self.bindings,
                self.calls,
                file,
                node,
                name,
            ) != Ok(true)
            {
                return None;
            }
        }
        crate::domains::core_arithmetic(self.context, self.bindings, self.calls, file, node)
            .ok()?;
        let domain = crate::domains::expression_domain(self.context, self.bindings, file, node);
        match &domain {
            Domain::Range { lower, upper } => {
                for bound in [lower, upper] {
                    // Reading a fixed scalar endpoint is pure even when its value is
                    // unknown. Unproved arithmetic does not get the same exemption.
                    if crate::domains::invariant_integer(bound)
                        .ok()
                        .flatten()
                        .is_none()
                    {
                        let id = match bound {
                            NumericBound::Symbol(id)
                            | NumericBound::Defined {
                                declaration: id, ..
                            } => id,
                            _ => return None,
                        };
                        let ty = &self.calls.declarations[id.0].ty;
                        if ty.instantiation != Instantiation::Parameter
                            || ty.kind != TypeKind::Int
                            || crate::value_safety::optional(ty)
                        {
                            return None;
                        }
                    }
                }
            }
            Domain::LiteralSet(values) => {
                for value in values {
                    crate::domains::invariant_integer(value).ok()??;
                }
            }
            _ => return None,
        }
        Some(domain)
    }

    fn nonempty(&self, domain: &Domain) -> bool {
        match domain {
            Domain::Named { domain, .. } => self.nonempty(domain),
            Domain::Range { lower, upper } => {
                matches!((crate::domains::invariant_integer(lower), crate::domains::invariant_integer(upper)), (Ok(Some(l)), Ok(Some(u))) if l <= u)
            }
            Domain::LiteralSet(values) => !values.is_empty(),
            Domain::Enum(id) => {
                let declaration = &self.bindings.declarations[id.0];
                self.bindings.declarations.iter().any(|d| {
                    d.role == DeclarationRole::EnumMember
                        && d.file == declaration.file
                        && d.item == declaration.item
                })
            }
            _ => false,
        }
    }
}

fn one_based(domain: &Domain) -> bool {
    match domain {
        Domain::Named { domain, .. } => one_based(domain),
        Domain::Range { lower, .. } => crate::domains::invariant_integer(lower) == Ok(Some(1)),
        Domain::LiteralSet(values) => {
            let values: Option<std::collections::BTreeSet<_>> = values
                .iter()
                .map(|v| crate::domains::invariant_integer(v).ok().flatten())
                .collect();
            values.is_some_and(|v| {
                v.is_empty()
                    || (v.first() == Some(&1)
                        && v.last()
                            .is_some_and(|&last| usize::try_from(last) == Ok(v.len())))
            })
        }
        _ => false,
    }
}
