//! Expression instantiation without lint policy or parameter-data evaluation.
use crate::callables::is_expression;
use crate::{
    BindingFacts, BindingResolution, CallOutcome, CallableFacts, DeclarationRole, FileId,
    Instantiation, ModelContext, SourceLocation, TypeInst, TypeKind,
};
use std::collections::BTreeMap;
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

#[derive(Clone, Debug)]
pub struct ExpressionInstantiation {
    pub file: FileId,
    pub location: SourceLocation,
    pub instantiation: Instantiation,
    /// Present precisely when instantiation is Unknown.
    pub reason: Option<String>,
}

#[derive(Debug, Default)]
pub struct InstantiationFacts {
    pub expressions: Vec<ExpressionInstantiation>,
}

/// Produce facts for every expression in readable user/explicit input files.
/// The bindings and callable facts must belong to this retained context. Calls
/// use resolved returns; projections use the selected component. Known par/var
/// conclusions do not require a fully supported type or supplied parameter data.
pub fn resolve_instantiations(
    context: &ModelContext,
    bindings: &BindingFacts,
    callables: &CallableFacts,
) -> InstantiationFacts {
    let mut producer = Producer {
        context,
        bindings,
        callables,
        types: callables
            .expressions
            .iter()
            .map(|e| {
                (
                    (e.file, e.location.range.start, e.location.range.end),
                    &e.ty,
                )
            })
            .collect(),
        facts: BTreeMap::new(),
        generators: BTreeMap::new(),
    };
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() || !source.parsed.diagnostics().is_empty() {
            continue;
        }
        producer.walk(file, source.parsed.tree());
    }
    InstantiationFacts {
        expressions: producer.facts.into_values().collect(),
    }
}

#[derive(Clone)]
struct Conclusion {
    instantiation: Instantiation,
    reason: Option<String>,
}
impl Conclusion {
    fn known(instantiation: Instantiation) -> Self {
        Self {
            instantiation,
            reason: None,
        }
    }
    fn unknown(reason: impl Into<String>) -> Self {
        Self {
            instantiation: Instantiation::Unknown,
            reason: Some(reason.into()),
        }
    }
}

fn combine(values: impl IntoIterator<Item = Conclusion>) -> Conclusion {
    let mut unknown = None;
    for value in values {
        match value.instantiation {
            Instantiation::Decision => return Conclusion::known(Instantiation::Decision),
            Instantiation::Unknown => unknown = value.reason,
            Instantiation::Parameter => {}
        }
    }
    unknown.map_or_else(
        || Conclusion::known(Instantiation::Parameter),
        Conclusion::unknown,
    )
}

struct Producer<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: &'a CallableFacts,
    types: BTreeMap<(FileId, usize, usize), &'a TypeInst>,
    facts: BTreeMap<(FileId, usize, usize), ExpressionInstantiation>,
    generators: BTreeMap<usize, Conclusion>,
}
impl Producer<'_> {
    fn key(&self, file: FileId, node: &SyntaxNode) -> (FileId, usize, usize) {
        let range = node.range();
        let offset = self.context.files[file].byte_offset;
        (file, range.start + offset, range.end + offset)
    }
    fn value(&self, file: FileId, node: &SyntaxNode) -> Conclusion {
        self.facts
            .get(&self.key(file, node))
            .map(|fact| Conclusion {
                instantiation: fact.instantiation,
                reason: fact.reason.clone(),
            })
            .unwrap_or_else(|| Conclusion::unknown("expression instantiation is unavailable"))
    }
    fn ty(&self, file: FileId, node: &SyntaxNode) -> Option<&TypeInst> {
        self.types.get(&self.key(file, node)).copied()
    }
    fn walk(&mut self, file: FileId, node: &SyntaxNode) {
        // Generator sources bind sequentially before heads, despite written CST
        // order. Set iteration binds par values; decision membership lifts heads.
        if node.kind() == NodeKind::Generator {
            if let Some(source) = node.child_nodes().next() {
                self.walk(file, source);
                let in_domain=node.children().iter().any(|c| matches!(c,SyntaxElement::Token(i) if self.context.files[file].parsed.tokens()[*i].kind==TokenKind::In));
                let conclusion = if !in_domain {
                    self.value(file, source)
                } else {
                    match self.ty(file, source).map(|t| &t.kind) {
                        Some(TypeKind::Set(_)) => Conclusion::known(Instantiation::Parameter),
                        Some(TypeKind::Array { element, .. })
                            if element.instantiation != Instantiation::Unknown =>
                        {
                            Conclusion::known(element.instantiation)
                        }
                        _ if matches!(
                            source.kind(),
                            NodeKind::RangeExpression
                                | NodeKind::SetLiteral
                                | NodeKind::SetComprehension
                        ) =>
                        {
                            Conclusion::known(Instantiation::Parameter)
                        }
                        _ => Conclusion::unknown("generator element instantiation is unavailable"),
                    }
                };
                for declaration in self.bindings.declarations.iter().filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == node.range()
                }) {
                    self.generators.insert(declaration.id.0, conclusion.clone());
                }
            }
            for filter in node.child_nodes().skip(1) {
                self.walk(file, filter);
            }
            return;
        }
        for child in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::GeneratorList)
        {
            self.walk(file, child);
        }
        for child in node
            .child_nodes()
            .filter(|n| n.kind() != NodeKind::GeneratorList)
        {
            self.walk(file, child);
        }
        if !is_expression(node.kind()) {
            return;
        }
        let conclusion = self.infer(file, node);
        self.facts.insert(
            self.key(file, node),
            ExpressionInstantiation {
                file,
                location: self.context.files[file].location(node.range()),
                instantiation: conclusion.instantiation,
                reason: conclusion.reason,
            },
        );
    }
    fn atom_instantiation(&self, file: FileId, node: &SyntaxNode) -> Conclusion {
        let source = &self.context.files[file];
        let token = node.children().iter().find_map(|child| {
            let SyntaxElement::Token(index) = child else {
                return None;
            };
            let token = &source.parsed.tokens()[*index];
            (!matches!(
                token.kind,
                TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
            ))
            .then_some(token)
        });
        let Some(token) = token else {
            return Conclusion::unknown("atom is missing");
        };
        if matches!(
            token.kind,
            TokenKind::IntegerLiteral
                | TokenKind::FloatLiteral
                | TokenKind::Infinity
                | TokenKind::StringLiteral
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Absent
        ) {
            return Conclusion::known(Instantiation::Parameter);
        }
        let reference = self.bindings.references.iter().find(|reference| {
            reference.file == file
                && reference.location.range.start == token.range.start + source.byte_offset
        });
        let Some(reference) = reference else {
            return Conclusion::unknown("atom instantiation is unsupported");
        };
        match reference.resolution {
            BindingResolution::Resolved(id) => {
                if let Some(conclusion) = self.generators.get(&id.0) {
                    return conclusion.clone();
                }
                let typed = self.callables.declarations[id.0].ty.instantiation;
                let instantiation = if typed == Instantiation::Unknown {
                    self.bindings.declarations[id.0].instantiation
                } else {
                    typed
                };
                if instantiation == Instantiation::Unknown {
                    Conclusion::unknown("resolved declaration has unknown instantiation")
                } else {
                    Conclusion::known(instantiation)
                }
            }
            BindingResolution::Ambiguous(_) => Conclusion::unknown("value binding is ambiguous"),
            _ => Conclusion::unknown("value binding is unresolved or callable"),
        }
    }

    fn call_instantiation(&self, file: FileId, node: &SyntaxNode) -> Conclusion {
        let source = &self.context.files[file];
        let head = node.children().iter().find_map(|child| match child {
            SyntaxElement::Token(index)
                if matches!(
                    source.parsed.tokens()[*index].kind,
                    TokenKind::Identifier
                        | TokenKind::QuotedIdentifier
                        | TokenKind::InfixIdentifier
                ) =>
            {
                Some(source.parsed.tokens()[*index].range.start + source.byte_offset)
            }
            _ => None,
        });
        let call = self
            .callables
            .calls
            .iter()
            .find(|call| call.file == file && Some(call.location.range.start) == head);
        match call.map(|call| &call.outcome) {
            Some(CallOutcome::Resolved { return_type, .. })
                if return_type.instantiation != Instantiation::Unknown =>
            {
                Conclusion::known(return_type.instantiation)
            }
            Some(CallOutcome::Resolved { .. }) => {
                Conclusion::unknown("resolved call return has unknown instantiation")
            }
            Some(
                CallOutcome::NoMatch { reason }
                | CallOutcome::Unresolved { reason }
                | CallOutcome::Unsupported { reason, .. },
            ) => Conclusion::unknown(reason.clone()),
            Some(CallOutcome::Ambiguous { .. }) => {
                Conclusion::unknown("callable selection is ambiguous")
            }
            None => Conclusion::unknown("callable facts are unavailable"),
        }
    }

    fn infer(&self, file: FileId, node: &SyntaxNode) -> Conclusion {
        use NodeKind::*;
        let children: Vec<_> = node.child_nodes().collect();
        // Detailed types may be unknown while their declared qualifier is known.
        if let Some(ty) = self
            .ty(file, node)
            .filter(|ty| ty.instantiation != Instantiation::Unknown)
        {
            return Conclusion::known(ty.instantiation);
        }
        let infix_call = node.children().iter().any(|child| matches!(child, SyntaxElement::Token(index) if self.context.files[file].parsed.tokens()[*index].kind == TokenKind::InfixIdentifier));
        match node.kind() {
            Expression => self.atom_instantiation(file, node),
            CallExpression | GeneratorCallExpression => self.call_instantiation(file, node),
            BinaryExpression if infix_call => self.call_instantiation(file, node),
            ParenthesizedExpression | AnnotatedExpression | NamedArgument | RecordLiteralField => {
                children
                    .first()
                    .map(|child| self.value(file, child))
                    .unwrap_or_else(|| Conclusion::unknown("expression is missing"))
            }
            InterpolatedString => Conclusion::known(Instantiation::Parameter),
            FieldAccessExpression => {
                Conclusion::unknown("selected field instantiation is unavailable")
            }
            ArrayAccessExpression => {
                let Some(subject) = children.first() else {
                    return Conclusion::unknown("array subject is missing");
                };
                let Some(TypeInst {
                    kind: TypeKind::Array { indices, element },
                    ..
                }) = self.ty(file, subject)
                else {
                    return Conclusion::unknown("array component type is unavailable");
                };
                if children.len() != indices.len() + 1
                    || children[1..].iter().any(|child| {
                        self.ty(file, child).is_none_or(|ty| {
                            matches!(ty.kind, TypeKind::Set(_) | TypeKind::Unknown(_))
                        })
                    })
                {
                    return Conclusion::unknown(
                        "array index or slicing instantiation is unsupported",
                    );
                }
                let component = if element.instantiation == Instantiation::Unknown {
                    Conclusion::unknown("array element instantiation is unknown")
                } else {
                    Conclusion::known(element.instantiation)
                };
                combine(
                    std::iter::once(component)
                        .chain(children[1..].iter().map(|child| self.value(file, child))),
                )
            }
            LetExpression => children
                .last()
                .map(|child| self.value(file, child))
                .unwrap_or_else(|| Conclusion::unknown("let body is missing")),
            ConditionalExpression => combine(
                children
                    .iter()
                    .flat_map(|branch| branch.child_nodes())
                    .map(|child| self.value(file, child)),
            ),
            SetComprehension | ArrayComprehension | IndexedArrayComprehension => {
                let mut values = Vec::new();
                for child in children {
                    if child.kind() == GeneratorList {
                        for generator in child.child_nodes() {
                            for source in generator.child_nodes() {
                                if source.kind() == WhereFilter {
                                    if let Some(filter) = source.child_nodes().next() {
                                        values.push(self.value(file, filter));
                                    }
                                } else {
                                    values.push(self.value(file, source));
                                }
                            }
                        }
                    } else {
                        values.push(self.value(file, child));
                    }
                }
                combine(values)
            }
            MatrixLiteral => combine(
                children
                    .iter()
                    .flat_map(|row| row.child_nodes())
                    .map(|child| self.value(file, child)),
            ),
            UnaryExpression | BinaryExpression | RangeExpression | SetLiteral | ArrayLiteral
            | TupleLiteral | RecordLiteral | IndexedArrayEntry | IndexTuple => {
                combine(children.iter().map(|child| self.value(file, child)))
            }
            _ => Conclusion::unknown("expression propagation is unsupported"),
        }
    }
}
