//! Classify written clauses, then retain child-first concrete callable instances.
use super::*;

pub(super) fn collect_root_clauses<'a>(source: &SourceInspector<'a>) -> Vec<Clause<'a>> {
    let interpreter = BodyInterpreter::without_bodies(source);
    let mut roots = Vec::new();
    for (file, file_source) in source.context.files.iter().enumerate() {
        if file_source.kind != SourceKind::User || !file_source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in file_source
            .parsed
            .tree()
            .child_nodes()
            .enumerate()
            .filter(|(_, n)| n.kind() == NodeKind::Constraint)
        {
            interpreter.clauses(file, item, node, source.calls, &[], &mut roots);
        }
    }
    roots
}

pub(super) fn discover_instances<'a>(
    source: &SourceInspector<'a>,
    roots: &[Clause<'a>],
) -> Vec<Instance<'a>> {
    let mut discovery = Discovery {
        source,
        instances: Vec::new(),
    };
    for clause in roots {
        discovery.discover(clause, source.calls, &[]);
    }
    discovery.discover_nested_bodies();
    discovery.instances
}

struct Discovery<'a, 's> {
    source: &'s SourceInspector<'a>,
    instances: Vec<Instance<'a>>,
}
impl<'a> Discovery<'a, '_> {
    fn discover_nested_bodies(&mut self) {
        let mut cursor = 0;
        while cursor < self.instances.len() {
            let instance = &self.instances[cursor];
            let mut ancestry = instance.ancestry.clone();
            ancestry.push(instance.id);
            let clauses = instance.clauses.clone();
            let view = instance.view.clone();
            for clause in &clauses {
                self.discover(clause, &view, &ancestry);
            }
            cursor += 1;
        }
    }
    fn discover(&mut self, clause: &Clause<'a>, view: &CallableFacts, ancestry: &[DeclarationId]) {
        for node in clause.node.child_nodes() {
            let nested = Clause {
                file: clause.file,
                item: clause.item,
                node,
                generators: clause.generators.clone(),
                kind: ClauseKind::Call,
            };
            self.discover(&nested, view, ancestry);
        }
        if !matches!(
            clause.node.kind(),
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
        ) || !matches!(clause.kind, ClauseKind::Call)
        {
            return;
        }
        let Some((id, parameters)) = self.source.resolved(clause.file, clause.node, view) else {
            return;
        };
        if ancestry.contains(&id) {
            if let Some(instance) = self
                .instances
                .iter_mut()
                .find(|i| i.id == id && i.parameters == parameters)
            {
                instance.recursive = true;
            }
            return;
        }
        if parameters.iter().any(|t| !t.known())
            || self
                .instances
                .iter()
                .any(|i| i.id == id && i.parameters == parameters)
        {
            return;
        }
        let d = &self.source.bindings.declarations[id.0];
        let Some(node) = find_node(
            self.source.context.files[d.file].parsed.tree(),
            &d.syntax_range,
            d.role,
        ) else {
            return;
        };
        let Some(body) = node
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .last()
        else {
            return;
        };
        if !self.source.callable_annotations_safe(d.file, node) {
            return;
        }
        let body_view = instantiated_body(
            self.source.context,
            self.source.bindings,
            self.source.calls,
            id,
            &parameters,
        );
        let mut clauses = Vec::new();
        BodyInterpreter::new(self.source, &self.instances, &[]).clauses(
            d.file,
            d.item,
            body,
            &body_view,
            &[],
            &mut clauses,
        );
        self.instances.push(Instance {
            id,
            parameters,
            ancestry: ancestry.to_vec(),
            view: body_view,
            clauses,
            recursive: false,
        });
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn clauses(
        &self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        out: &mut Vec<Clause<'a>>,
    ) {
        let add = |kind| Clause {
            file,
            item,
            node,
            generators: generators.to_vec(),
            kind,
        };
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::Constraint => {
                // An interpolated header label precedes the actual expression.
                if let Some(n) = children.last() {
                    self.clauses(file, item, n, view, generators, out);
                }
            }
            NodeKind::ParenthesizedExpression | NodeKind::NamedArgument => {
                if let Some(n) = children.first() {
                    self.clauses(file, item, n, view, generators, out);
                }
            }
            NodeKind::AnnotatedExpression => {
                let harmless = children.iter().filter(|n| n.kind() == NodeKind::Annotation)
                    .all(|n| n.child_nodes().next().is_some_and(|v| {
                        if v.children().iter().any(|c| matches!(c, SyntaxElement::Token(i)
                            if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::StringLiteral)) {
                            return true;
                        }
                        let value = unwrap(v);
                        value.kind() == NodeKind::Expression
                            && crate::domains::tokens(&self.source.context.files[file].parsed, value).len() == 1
                            && self.source.reference(file, value).is_some_and(|id| {
                                let declaration = &self.source.bindings.declarations[id.0];
                                declaration.role == DeclarationRole::Annotation
                                    && declaration.name == "domain"
                                    && self.source.context.files[declaration.file].kind == SourceKind::StandardLibrary
                                    && self.source.context.files[declaration.file].implicit
                            })
                    }));
                if harmless {
                    if let Some(n) = children.first() {
                        self.clauses(file, item, n, view, generators, out);
                    }
                } else {
                    out.push(add(ClauseKind::Unsupported));
                }
            }
            NodeKind::BinaryExpression => match operator(self.source.context, file, node) {
                Some(TokenKind::Equal | TokenKind::DoubleEqual)
                    if self.source.core(file, node, view, "=") =>
                {
                    out.push(add(ClauseKind::Equality))
                }
                Some(TokenKind::And) if self.source.core(file, node, view, "/\\") => {
                    for n in children {
                        self.clauses(file, item, n, view, generators, out);
                    }
                }
                Some(
                    TokenKind::Implies
                    | TokenKind::ReverseImplies
                    | TokenKind::Or
                    | TokenKind::Equivalence
                    | TokenKind::NotEqual
                    | TokenKind::Less
                    | TokenKind::LessEqual
                    | TokenKind::Greater
                    | TokenKind::GreaterEqual
                    | TokenKind::In,
                ) => out.push(add(ClauseKind::Relation)),
                _ => out.push(add(ClauseKind::Unsupported)),
            },
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression => {
                if node.kind() == NodeKind::CallExpression
                    && self.source.core(file, node, view, "assert")
                {
                    out.push(add(ClauseKind::Assertion));
                    return;
                }
                if !self.source.core(file, node, view, "forall") {
                    out.push(add(ClauseKind::Call));
                    return;
                }
                let quantified = if node.kind() == NodeKind::GeneratorCallExpression {
                    Some(node)
                } else {
                    children
                        .first()
                        .copied()
                        .filter(|n| n.kind() == NodeKind::ArrayComprehension)
                };
                let Some(q) = quantified else {
                    // Ordinary array actuals need the selected call's source/body inspector.
                    out.push(add(ClauseKind::Call));
                    return;
                };
                let Some(list) = q
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                else {
                    out.push(add(ClauseKind::Unsupported));
                    return;
                };
                let mut all = generators.to_vec();
                all.extend(list.child_nodes());
                if self
                    .relation_iterations(file, &all, generators.len(), view, false)
                    .is_ok()
                {
                    if let Some(body) = q
                        .child_nodes()
                        .find(|n| n.kind() != NodeKind::GeneratorList)
                    {
                        self.clauses(file, item, body, view, &all, out);
                    }
                } else {
                    // Direct filtered definitions already retain their partial
                    // extent. A demanded callable body needs a specific boundary.
                    let in_body = self.source.bindings.declarations.iter().any(|d| {
                        d.file == file
                            && matches!(
                                d.role,
                                DeclarationRole::Predicate
                                    | DeclarationRole::Function
                                    | DeclarationRole::Test
                            )
                            && d.syntax_range.start <= node.range().start
                            && node.range().end <= d.syntax_range.end
                    });
                    // A failed conditional traversal still needs its complete
                    // source inspection even though filters establish no output.
                    let conditional_body = node.kind() == NodeKind::GeneratorCallExpression
                        && q.child_nodes()
                            .find(|n| n.kind() != NodeKind::GeneratorList)
                            .is_some_and(|body| {
                                unwrap(body).kind() == NodeKind::ConditionalExpression
                            });
                    if in_body
                        || conditional_body
                        || !all
                            .iter()
                            .any(|g| g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter))
                    {
                        out.push(add(ClauseKind::Unsupported));
                    }
                }
            }
            NodeKind::ArrayAccessExpression => {
                // An enforced Boolean member is a relation, never a whole-array output.
                let range = self.source.context.files[file].location(node.range()).range;
                let boolean = self
                    .source
                    .view(file, node, view)
                    .expressions
                    .iter()
                    .any(|e| {
                        e.file == file
                            && e.location.range == range
                            && e.ty.kind == TypeKind::Bool
                            && !optional(&e.ty)
                    });
                out.push(add(if boolean {
                    ClauseKind::Relation
                } else {
                    ClauseKind::Unsupported
                }));
            }
            NodeKind::ConditionalExpression => out.push(add(ClauseKind::Conditional)),
            NodeKind::LetExpression => out.push(add(ClauseKind::Local)),
            NodeKind::UnaryExpression => out.push(add(ClauseKind::Relation)),
            NodeKind::Expression => {
                if self.source.reference(file, node).is_some() {
                    out.push(add(ClauseKind::Boolean));
                }
            }
            _ => out.push(add(ClauseKind::Unsupported)),
        }
    }
}
