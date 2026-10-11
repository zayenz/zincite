//! Bounded identity-use advice. Values retain a candidate family, never integer bounds.
use std::collections::BTreeMap;

use crate::{
    BindingFacts, CallOutcome, CallableFacts, DeclarationId, DeclarationRole, Domain, DomainFacts,
    FileFinding, FileId, Instantiation, ModelContext, ModelRootState, Rule, Severity,
    SourceDiagnostic, SourceKind, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

pub(super) struct CheckResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
    pub state: ModelRootState,
}

pub(super) fn check(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    domains: &DomainFacts,
) -> CheckResult {
    let solves: Vec<_> = context
        .files
        .iter()
        .filter(|f| f.kind == SourceKind::User)
        .flat_map(|f| {
            f.parsed
                .tree()
                .child_nodes()
                .filter(|n| {
                    matches!(
                        n.kind(),
                        NodeKind::Solve | NodeKind::SolveMinimize | NodeKind::SolveMaximize
                    )
                })
                .map(|n| f.location(n.range()))
        })
        .collect();
    let state = if !context.errors.is_empty()
        || !context.limitations.is_empty()
        || context.root_file.is_none()
        || solves.len() > 1
    {
        ModelRootState::Incomplete
    } else if solves.is_empty() {
        ModelRootState::Fragment
    } else {
        ModelRootState::Complete
    };
    let mut result = CheckResult {
        findings: Vec::new(),
        limitations: Vec::new(),
        state,
    };
    if solves.len() > 1 {
        result.limitations.push(SourceDiagnostic {
            location: solves[1].clone(),
            message: "prefer-enum: multiple solve items prevent complete model-use analysis".into(),
        });
    }
    if state != ModelRootState::Complete {
        return result;
    }
    let mut analysis = Uses {
        context,
        bindings,
        calls,
        domains,
        candidates: Vec::new(),
        roots: vec![None; bindings.declarations.len()],
        values: vec![None; bindings.declarations.len()],
        active_values: Vec::new(),
        active_calls: Vec::new(),
        limitations: Vec::new(),
    };
    analysis.discover();
    if analysis.candidates.is_empty() {
        return result;
    }
    for (file, source) in context
        .files
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == SourceKind::User)
    {
        for node in source.parsed.tree().child_nodes() {
            analysis.inspect(file, node, &mut BTreeMap::new(), calls);
        }
    }
    analysis.default_output();
    for candidate in &analysis.candidates {
        let d = &bindings.declarations[candidate.declaration.0];
        let source = &context.files[d.file];
        if candidate.used
            && !candidate.rejected
            && source.warnings_enabled()
            && source
                .suppressions
                .as_ref()
                .is_some_and(|s| !s[d.item].contains(&Rule::PreferEnum))
        {
            result.findings.push(FileFinding {
                location: d.location.clone(), rule: Rule::PreferEnum, severity: Severity::Warning, fix: None,
                message: format!("Members of '{}' appear to serve as identities in the supported model uses; consider an enum. Review member names and external data/output compatibility before converting", d.name),
            });
        }
    }
    result.limitations = analysis.limitations;
    result
}

struct Candidate {
    declaration: DeclarationId,
    used: bool,
    rejected: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Independent,
    // An unrelated expression lacks supported source facts; retain that gap at named contracts.
    Unavailable,
    Member(usize),
    Set(usize),
    ComputedSet(usize),
    // An array's axes are independent of its payload. Member payload storage is excluded.
    Array(Vec<Option<usize>>),
}
impl Value {
    fn independent(values: &[Self]) -> Self {
        if values.contains(&Self::Unavailable) {
            Self::Unavailable
        } else {
            Self::Independent
        }
    }
    fn set_family(&self) -> Option<usize> {
        match self {
            Self::Set(f) | Self::ComputedSet(f) => Some(*f),
            _ => None,
        }
    }
    fn families(&self) -> Vec<usize> {
        match self {
            Self::Member(f) | Self::Set(f) | Self::ComputedSet(f) => vec![*f],
            Self::Array(axes) => axes.iter().flatten().copied().collect(),
            Self::Independent | Self::Unavailable => Vec::new(),
        }
    }
}
type Frame = BTreeMap<usize, Value>;
struct Uses<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    domains: &'a DomainFacts,
    candidates: Vec<Candidate>,
    roots: Vec<Option<usize>>,
    values: Vec<Option<Value>>,
    active_values: Vec<DeclarationId>,
    active_calls: Vec<DeclarationId>,
    limitations: Vec<SourceDiagnostic>,
}
impl<'a> Uses<'a> {
    fn node(&self, id: DeclarationId) -> Option<&'a SyntaxNode> {
        let d = &self.bindings.declarations[id.0];
        crate::callables::find_node(
            self.context.files[d.file].parsed.tree(),
            &d.syntax_range,
            d.role,
        )
    }
    fn discover(&mut self) {
        for d in &self.bindings.declarations {
            if self.context.files[d.file].kind != SourceKind::User
                || !d.top_level
                || d.role != DeclarationRole::Value
            {
                continue;
            }
            let ty = &self.calls.declarations[d.id.0].ty;
            if ty.instantiation != Instantiation::Parameter
                || crate::value_safety::optional(ty)
                || !matches!(&ty.kind, TypeKind::Set(t) if t.kind == TypeKind::Int)
            {
                continue;
            }
            let Some(value) = self.node(d.id).and_then(initializer) else {
                continue;
            };
            if !matches!(
                value.kind(),
                NodeKind::SetLiteral | NodeKind::RangeExpression
            ) {
                continue;
            }
            let tokens = crate::domains::tokens(&self.context.files[d.file].parsed, value);
            if tokens.iter().any(|t| {
                matches!(
                    t.kind,
                    TokenKind::Identifier
                        | TokenKind::QuotedIdentifier
                        | TokenKind::InfixIdentifier
                )
            }) {
                continue;
            }
            let domain =
                crate::domains::expression_domain(self.context, self.bindings, d.file, value);
            let nonempty = match domain {
                Domain::Range { lower, upper } => {
                    matches!((crate::domains::invariant_integer(&lower), crate::domains::invariant_integer(&upper)), (Ok(Some(l)), Ok(Some(u))) if l <= u)
                }
                Domain::LiteralSet(values) => {
                    !values.is_empty()
                        && values.iter().all(|v| {
                            crate::domains::invariant_integer(v)
                                .ok()
                                .flatten()
                                .is_some()
                        })
                }
                _ => false,
            };
            if !nonempty
                || crate::domains::core_arithmetic(
                    self.context,
                    self.bindings,
                    self.calls,
                    d.file,
                    value,
                )
                .is_err()
            {
                continue;
            }
            let family = self.candidates.len();
            self.roots[d.id.0] = Some(family);
            self.candidates.push(Candidate {
                declaration: d.id,
                used: false,
                rejected: false,
            });
        }
        // Only direct aliases retain family identity; equal domains do not.
        loop {
            let mut changed = false;
            for d in &self.bindings.declarations {
                if self.roots[d.id.0].is_some() {
                    continue;
                }
                let set = matches!(self.calls.declarations[d.id.0].ty.kind, TypeKind::Set(_));
                if !set && d.role != DeclarationRole::TypeAlias {
                    continue;
                }
                let target = self
                    .node(d.id)
                    .and_then(|n| {
                        if set {
                            initializer(n)
                        } else {
                            n.child_nodes().next()
                        }
                    })
                    .and_then(|n| self.direct_reference(d.file, n));
                if let Some(family) = target.and_then(|id| self.roots[id.0]) {
                    self.roots[d.id.0] = Some(family);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    fn direct_reference(&self, file: FileId, mut node: &SyntaxNode) -> Option<DeclarationId> {
        while matches!(
            node.kind(),
            NodeKind::ParenthesizedExpression | NodeKind::DomainType | NodeKind::ScalarType
        ) {
            node = node.child_nodes().next()?;
        }
        (node.kind() == NodeKind::Expression && node.child_nodes().next().is_none())
            .then(|| {
                crate::definitions::resolved_reference(self.context, self.bindings, file, node)
            })
            .flatten()
    }
    fn family(&self, domain: &Domain) -> Option<usize> {
        match domain {
            Domain::Named {
                declaration,
                domain,
            } => self.roots[declaration.0].or_else(|| self.family(domain)),
            _ => None,
        }
    }
    fn declared(&self, id: DeclarationId) -> Value {
        let domain = &self.domains.declarations[id.0].domain;
        if let Some(f) = self.family(domain) {
            return Value::Member(f);
        }
        match domain {
            Domain::Set(element) => self
                .family(element)
                .map(Value::Set)
                .unwrap_or(Value::Independent),
            Domain::Array { indices, .. } => {
                Value::Array(indices.iter().map(|i| self.family(i)).collect())
            }
            _ => Value::Independent,
        }
    }
    fn reject(
        &mut self,
        values: &[Value],
        file: FileId,
        node: &SyntaxNode,
        limitation: Option<&str>,
    ) {
        let families: Vec<_> = values.iter().flat_map(Value::families).collect();
        if families.is_empty() {
            return;
        }
        for family in families {
            self.candidates[family].rejected = true;
        }
        if let Some(reason) = limitation {
            let location = self.context.files[file].location(node.range());
            let message = format!("prefer-enum: {reason}");
            if !self
                .limitations
                .iter()
                .any(|d| d.location == location && d.message == message)
            {
                self.limitations
                    .push(SourceDiagnostic { location, message });
            }
        }
    }
    fn used(&mut self, value: &Value) {
        for f in value.families() {
            self.candidates[f].used = true;
        }
    }
    fn declaration(&mut self, id: DeclarationId, frame: &mut Frame, view: &CallableFacts) -> Value {
        if let Some(value) = frame.get(&id.0) {
            return value.clone();
        }
        let d = &self.bindings.declarations[id.0];
        let global = d.top_level;
        let view = if global { self.calls } else { view };
        if global && let Some(value) = &self.values[id.0] {
            return value.clone();
        }
        let Some(node) = self.node(id) else {
            return Value::Unavailable;
        };
        let file = d.file;
        let mut value = if let Some(f) = self.roots[id.0] {
            if d.role == DeclarationRole::TypeAlias {
                Value::Independent
            } else {
                Value::Set(f)
            }
        } else {
            self.declared(id)
        };
        if self.active_values.contains(&id) {
            self.reject(
                &[value.clone()],
                file,
                node,
                Some("cyclic identity aliases are unsupported"),
            );
            return value;
        }
        self.active_values.push(id);
        let candidate_root = self.candidates.iter().any(|c| c.declaration == id);
        if !candidate_root && let Some(init) = initializer(node) {
            let actual = self.inspect(file, init, frame, view);
            if value.set_family().is_some() && value.set_family() == actual.set_family() {
                value = actual;
            } else if matches!(value, Value::Member(_) | Value::Set(_)) && actual != value {
                self.reject(
                    &[value.clone(), actual.clone()],
                    file,
                    init,
                    (actual == Value::Unavailable)
                        .then_some("the candidate member or set origin is unavailable"),
                );
            } else if value == Value::Independent {
                value = actual;
            } else if let (Value::Array(expected), Value::Array(actual_axes)) = (&value, &actual) {
                if expected != actual_axes {
                    self.reject(&[value.clone(), actual], file, init, None);
                }
            } else if let (
                Value::Array(_),
                Value::Member(_) | Value::Set(_) | Value::ComputedSet(_),
            ) = (&value, &actual)
            {
                self.reject(
                    &[actual],
                    file,
                    init,
                    Some("candidate-member array storage is unsupported"),
                );
            }
        }
        if let Some(ty) = node.child_nodes().next() {
            self.inspect_type(file, ty, frame, view, false);
        }
        for annotation in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::Annotation)
        {
            let marker = crate::unused_declarations::output_marker(
                self.context,
                self.bindings,
                view,
                file,
                annotation,
            );
            if marker.is_err() {
                self.inspect(file, annotation, frame, view);
                self.reject(
                    &[value.clone()],
                    file,
                    annotation,
                    Some("an affected declaration annotation is not interpreted"),
                );
            }
        }
        self.active_values.pop();
        if global {
            self.values[id.0] = Some(value.clone());
        } else {
            frame.insert(id.0, value.clone());
        }
        value
    }
    // Domains consume whole candidate sets; numeric domain expressions consume members.
    fn inspect_type(
        &mut self,
        file: FileId,
        ty: &SyntaxNode,
        frame: &mut Frame,
        view: &CallableFacts,
        stored_members: bool,
    ) {
        if matches!(ty.kind(), NodeKind::DomainType | NodeKind::ScalarType) {
            if let Some(family) = self
                .direct_reference(file, ty)
                .and_then(|id| self.roots[id.0])
            {
                if stored_members {
                    self.reject(&[Value::Member(family)], file, ty, Some("candidate-member storage in arrays or structured fields is unsupported"));
                }
                return;
            }
            for expression in ty
                .child_nodes()
                .filter(|n| crate::callables::is_expression(n.kind()))
            {
                let value = self.inspect(file, expression, frame, view);
                match value {
                    Value::Set(_) if !stored_members => {}
                    Value::Member(_) => self.reject(&[value], file, ty, None),
                    _ => self.reject(
                        &[value],
                        file,
                        ty,
                        Some("computed candidate domains or member storage are unsupported"),
                    ),
                }
            }
            return;
        }
        let children: Vec<_> = ty.child_nodes().collect();
        for (position, child) in children.iter().enumerate() {
            let stored = stored_members
                || match ty.kind() {
                    NodeKind::ArrayType => position + 1 == children.len(),
                    NodeKind::TupleType | NodeKind::RecordType | NodeKind::ListType => true,
                    _ => false,
                };
            if crate::callables::is_expression(child.kind()) {
                let value = self.inspect(file, child, frame, view);
                self.reject(&[value], file, child, None);
            } else {
                self.inspect_type(file, child, frame, view, stored);
            }
        }
    }
    fn inspect(
        &mut self,
        file: FileId,
        node: &SyntaxNode,
        frame: &mut Frame,
        view: &CallableFacts,
    ) -> Value {
        use NodeKind as N;
        match node.kind() {
            N::Expression => self
                .direct_reference(file, node)
                .map(|id| self.declaration(id, frame, view))
                .unwrap_or_else(|| {
                    if view.expressions.iter().any(|e| {
                        e.file == file
                            && e.location.range
                                == self.context.files[file].location(node.range()).range
                            && e.ty.known()
                    }) {
                        Value::Independent
                    } else {
                        Value::Unavailable
                    }
                }),
            N::Declaration | N::Parameter | N::TypeAlias => {
                let id = self
                    .bindings
                    .declarations
                    .iter()
                    .find(|d| d.file == file && d.syntax_range == node.range())
                    .map(|d| d.id);
                id.map(|id| self.declaration(id, frame, view))
                    .unwrap_or(Value::Independent)
            }
            N::ParenthesizedExpression | N::NamedArgument => node
                .child_nodes()
                .next()
                .map(|n| self.inspect(file, n, frame, view))
                .unwrap_or(Value::Independent),
            N::FunctionDeclaration
            | N::PredicateDeclaration
            | N::TestDeclaration
            | N::AnnotationDeclaration => {
                let mut local = Frame::new();
                for parameter in node
                    .child_nodes()
                    .filter(|n| n.kind() == N::ParameterList)
                    .flat_map(|n| n.child_nodes())
                {
                    self.inspect(file, parameter, &mut local, view);
                }
                if node.kind() == N::FunctionDeclaration
                    && let Some(ty) = node.child_nodes().next()
                {
                    self.inspect_type(file, ty, &mut local, view, false);
                }
                for body in node.child_nodes().filter(|n| {
                    crate::callables::is_expression(n.kind()) || n.kind() == N::Annotation
                }) {
                    let value = self.inspect(file, body, &mut local, view);
                    if crate::callables::is_expression(body.kind()) {
                        self.check_result_contract(file, node, body, value);
                    }
                }
                Value::Independent
            }
            N::LetExpression => {
                let mut local = frame.clone();
                let mut result = Value::Independent;
                for part in node.child_nodes() {
                    result = self.inspect(file, part, &mut local, view);
                }
                result
            }
            N::GeneratorCallExpression
            | N::ArrayComprehension
            | N::IndexedArrayComprehension
            | N::SetComprehension => self.generator_expression(file, node, frame, view),
            N::ArrayAccessExpression => {
                let values: Vec<_> = node
                    .child_nodes()
                    .map(|n| self.inspect(file, n, frame, view))
                    .collect();
                if let Some(Value::Array(axes)) = values.first() {
                    for (axis, key) in axes.iter().zip(&values[1..]) {
                        if let Some(f) = axis {
                            if *key == Value::Member(*f) {
                                self.used(key);
                            } else {
                                self.reject(&[Value::Member(*f), key.clone()], file, node, None);
                            }
                        } else if !key.families().is_empty() {
                            self.reject(std::slice::from_ref(key), file, node, None);
                        }
                    }
                    if values.len() != axes.len() + 1 {
                        self.reject(
                            &values,
                            file,
                            node,
                            Some("candidate-indexed array slicing is unsupported"),
                        );
                    }
                } else {
                    self.reject(
                        &values,
                        file,
                        node,
                        Some("the indexed candidate's array axes are unavailable"),
                    );
                }
                Value::independent(&values)
            }
            N::CallExpression | N::BinaryExpression | N::UnaryExpression | N::RangeExpression => {
                let values: Vec<_> = node
                    .child_nodes()
                    .map(|n| self.inspect(file, n, frame, view))
                    .collect();
                self.operation(file, node, values, frame, view)
            }
            N::SetLiteral => {
                let values: Vec<_> = node
                    .child_nodes()
                    .map(|n| self.inspect(file, n, frame, view))
                    .collect();
                if let Some(Value::Member(f)) = values.first()
                    && values.iter().all(|v| *v == Value::Member(*f))
                {
                    Value::ComputedSet(*f)
                } else {
                    self.reject(&values, file, node, None);
                    Value::independent(&values)
                }
            }
            N::IndexedArrayEntry => {
                let parts: Vec<_> = node.child_nodes().collect();
                let Some(key) = parts.first() else {
                    return Value::Independent;
                };
                let keys: Vec<_> = if key.kind() == N::IndexTuple {
                    key.child_nodes().collect()
                } else {
                    vec![*key]
                };
                let mut axes = Vec::new();
                for key in keys {
                    let value = self.inspect(file, key, frame, view);
                    if let Value::Member(f) = value {
                        self.used(&value);
                        axes.push(Some(f));
                    } else {
                        self.reject(
                            &[value],
                            file,
                            key,
                            Some("computed candidate index keys are unsupported"),
                        );
                        axes.push(None);
                    }
                }
                for payload in &parts[1..] {
                    let value = self.inspect(file, payload, frame, view);
                    self.reject(
                        &[value],
                        file,
                        payload,
                        Some("candidate-member array storage is unsupported"),
                    );
                }
                Value::Array(axes)
            }
            N::ArrayLiteral => {
                let values: Vec<_> = node
                    .child_nodes()
                    .map(|n| self.inspect(file, n, frame, view))
                    .collect();
                if let Some(Value::Array(axes)) = values.first()
                    && values.iter().all(|v| *v == Value::Array(axes.clone()))
                {
                    Value::Array(axes.clone())
                } else {
                    self.reject(
                        &values,
                        file,
                        node,
                        Some("candidate-member array storage or mixed index keys are unsupported"),
                    );
                    Value::independent(&values)
                }
            }
            N::MatrixLiteral | N::TupleLiteral | N::RecordLiteral | N::FieldAccessExpression => {
                let values: Vec<_> = node
                    .child_nodes()
                    .map(|n| self.inspect(file, n, frame, view))
                    .collect();
                self.reject(
                    &values,
                    file,
                    node,
                    Some("candidate-member storage or projection is unsupported"),
                );
                Value::independent(&values)
            }
            N::ConditionalExpression => {
                let values: Vec<_> = node
                    .child_nodes()
                    .map(|n| self.inspect(file, n, frame, view))
                    .collect();
                self.reject(
                    &values,
                    file,
                    node,
                    Some("computed candidate-member results are unsupported"),
                );
                Value::independent(&values)
            }
            N::ConditionalBranch => {
                let mut children = node.child_nodes();
                if let Some(condition) = children.next() {
                    let value = self.inspect(file, condition, frame, view);
                    self.reject(&[value], file, condition, None);
                }
                children
                    .next()
                    .map(|n| self.inspect(file, n, frame, view))
                    .unwrap_or(Value::Independent)
            }
            N::ElseBranch => node
                .child_nodes()
                .next()
                .map(|n| self.inspect(file, n, frame, view))
                .unwrap_or(Value::Independent),
            N::Assignment => {
                let values: Vec<_> = node
                    .child_nodes()
                    .map(|n| self.inspect(file, n, frame, view))
                    .collect();
                self.reject(
                    &values,
                    file,
                    node,
                    Some("candidate propagation through separate assignments is unsupported"),
                );
                Value::Independent
            }
            N::Output
            | N::SolveMinimize
            | N::SolveMaximize
            | N::InterpolatedString
            | N::Annotation => {
                let values: Vec<_> = node
                    .child_nodes()
                    .map(|n| self.inspect(file, n, frame, view))
                    .collect();
                self.reject(&values, file, node, None);
                Value::Independent
            }
            N::AnnotatedExpression => {
                let mut parts = node.child_nodes();
                let value = parts
                    .next()
                    .map(|n| self.inspect(file, n, frame, view))
                    .unwrap_or(Value::Independent);
                for annotation in parts {
                    self.inspect(file, annotation, frame, view);
                    self.reject(
                        std::slice::from_ref(&value),
                        file,
                        annotation,
                        Some("annotated candidate-member expressions are unsupported"),
                    );
                }
                value
            }
            // Types are checked with their owning declaration; includes contain no expressions.
            N::ScalarType
            | N::DomainType
            | N::ArrayType
            | N::SetType
            | N::TupleType
            | N::RecordType
            | N::TypeInstVariable
            | N::Include => Value::Independent,
            _ => {
                let mut value = Value::Independent;
                for part in node.child_nodes() {
                    value = self.inspect(file, part, frame, view);
                }
                value
            }
        }
    }
    fn generator_expression(
        &mut self,
        file: FileId,
        node: &SyntaxNode,
        frame: &Frame,
        view: &CallableFacts,
    ) -> Value {
        let mut local = frame.clone();
        if let Some(list) = node
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)
        {
            for generator in list.child_nodes() {
                let mut parts = generator.child_nodes();
                let Some(source) = parts.next() else {
                    continue;
                };
                let value = self.inspect(file, source, &mut local, view);
                let membership =
                    crate::domains::tokens(&self.context.files[file].parsed, generator)
                        .iter()
                        .any(|t| t.kind == TokenKind::In);
                let bound = if membership {
                    if let Value::Set(f) = value {
                        if self.direct_reference(file, source).is_none() {
                            self.reject(
                                &[value],
                                file,
                                source,
                                Some("computed candidate membership is unsupported"),
                            );
                        }
                        Value::Member(f)
                    } else {
                        self.reject(
                            &[value],
                            file,
                            source,
                            Some("candidate generator membership is unavailable"),
                        );
                        Value::Independent
                    }
                } else {
                    value
                };
                for d in self.bindings.declarations.iter().filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == generator.range()
                }) {
                    local.insert(d.id.0, bound.clone());
                }
                for filter in parts {
                    self.inspect(file, filter, &mut local, view);
                }
            }
        }
        let body = node
            .child_nodes()
            .find(|n| n.kind() != NodeKind::GeneratorList);
        let value = body
            .map(|n| self.inspect(file, n, &mut local, view))
            .unwrap_or(Value::Independent);
        if node.kind() == NodeKind::GeneratorCallExpression {
            self.operation(file, node, vec![value], &mut local, view)
        } else if node.kind() == NodeKind::IndexedArrayComprehension
            && matches!(value, Value::Array(_))
        {
            value
        } else {
            self.reject(
                &[value],
                file,
                node,
                Some("candidate-member collection storage or computed membership is unsupported"),
            );
            Value::Independent
        }
    }
    fn operation(
        &mut self,
        file: FileId,
        node: &SyntaxNode,
        values: Vec<Value>,
        frame: &mut Frame,
        view: &CallableFacts,
    ) -> Value {
        let fact = crate::callables::operation_fact(self.context, view, file, node);
        let core = fact.is_some_and(|f| {
            crate::callables::core_operation_outcome(
                self.context,
                self.bindings,
                Some(&f.outcome),
                node.kind(),
                &f.name,
            ) == Ok(true)
        });
        if core {
            let name = fact.unwrap().name.as_str();
            match (name, values.as_slice()) {
                ("card", [Value::Set(_) | Value::ComputedSet(_)]) => return Value::Independent,
                (
                    "sum" | "product" | "min" | "max" | "forall" | "exists" | "length",
                    [Value::Array(_)],
                ) => return Value::Independent,
                ("index_set", [Value::Array(axes)]) if axes.len() == 1 => {
                    return axes[0]
                        .map(Value::ComputedSet)
                        .unwrap_or(Value::Independent);
                }
                ("=" | "==" | "!=" | "<>", [Value::Member(a), Value::Member(b)]) if a == b => {
                    self.used(&values[0]);
                    return Value::Independent;
                }
                ("=" | "==" | "!=" | "<>", [left, right])
                    if left.set_family().is_some() && left.set_family() == right.set_family() =>
                {
                    return Value::Independent;
                }
                ("in", [Value::Member(a), set]) if set.set_family() == Some(*a) => {
                    self.used(&values[0]);
                    return Value::Independent;
                }
                ("union" | "intersect" | "diff" | "symdiff", [left, right])
                    if left.set_family().is_some() && left.set_family() == right.set_family() =>
                {
                    return Value::ComputedSet(left.set_family().unwrap());
                }
                ("subset" | "superset", [left, right])
                    if left.set_family().is_some() && left.set_family() == right.set_family() =>
                {
                    return Value::Independent;
                }
                _ => {}
            }
            let sensitive = matches!(
                name,
                "=" | "=="
                    | "!="
                    | "<>"
                    | "in"
                    | "union"
                    | "intersect"
                    | "diff"
                    | "symdiff"
                    | "subset"
                    | "superset"
                    | "+"
                    | "-"
                    | "*"
                    | "/"
                    | "div"
                    | "mod"
                    | "^"
                    | "<"
                    | ">"
                    | "<="
                    | ">="
                    | ".."
                    | "<.."
                    | "..<"
                    | "<..<"
                    | "sum"
                    | "product"
                    | "min"
                    | "max"
                    | "int2float"
                    | "float2int"
                    | "enum2int"
                    | "enum_next"
                    | "enum_prev"
                    | "show"
                    | "showDzn"
                    | "showJSON"
                    | "show_int"
                    | "show_float"
            );
            self.reject(&values, file, node, if sensitive { None } else { Some("the affected standard operation is outside the supported identity operations") });
            return Value::independent(&values);
        }
        if let Some(crate::CallFact {
            outcome:
                CallOutcome::Resolved {
                    declaration,
                    parameters,
                    ..
                },
            ..
        }) = fact
            && self.context.files[self.bindings.declarations[declaration.0].file].kind
                == SourceKind::User
        {
            return self.user_call((file, node), *declaration, parameters, values, frame, view);
        }
        let mut affected = values;
        if let Some(crate::CallFact {
            outcome:
                CallOutcome::Unsupported { candidates, .. } | CallOutcome::Ambiguous { candidates },
            ..
        }) = fact
        {
            for id in candidates {
                affected.extend(self.callable_contracts(*id));
            }
        }
        self.reject(
            &affected,
            file,
            node,
            Some("affected callable selection or implementation is unavailable"),
        );
        Value::Unavailable
    }
    fn callable_contracts(&self, id: DeclarationId) -> Vec<Value> {
        let mut values = vec![self.declared(id)];
        if let Some(signature) = self.calls.signatures.iter().find(|s| s.declaration == id) {
            for position in 0..signature.parameters.len() {
                if let Some(formal) =
                    crate::callables::formal_parameter(self.context, self.bindings, id, position)
                {
                    values.push(self.declared(formal));
                }
            }
        }
        values
    }
    fn check_result_contract(
        &mut self,
        file: FileId,
        node: &SyntaxNode,
        body: &SyntaxNode,
        value: Value,
    ) -> Value {
        if node.kind() != NodeKind::FunctionDeclaration {
            return value;
        }
        let Some(id) = self
            .bindings
            .declarations
            .iter()
            .find(|d| d.file == file && d.syntax_range == node.range())
            .map(|d| d.id)
        else {
            return value;
        };
        self.reconcile_contract(id, (file, body), value)
    }
    fn reconcile_contract(
        &mut self,
        id: DeclarationId,
        site: (FileId, &SyntaxNode),
        actual: Value,
    ) -> Value {
        let (file, expression) = site;
        let declaration = &self.bindings.declarations[id.0];
        let Some(ty) = self.node(id).and_then(|n| n.child_nodes().next()) else {
            return actual;
        };
        if !self.contract(ty, declaration.file, &actual) {
            self.reject(std::slice::from_ref(&actual), file, expression, None);
        }
        let expected = self.declared(id);
        if expected.families().is_empty()
            || expected == actual
            || expected.set_family().is_some() && expected.set_family() == actual.set_family()
        {
            return actual;
        }
        // Named contracts retain their family even when an origin is numeric or unavailable.
        // A plain array literal can adopt declared axes without changing its payload category.
        if !(matches!(expected, Value::Array(_)) && actual == Value::Independent) {
            self.reject(
                &[expected.clone(), actual.clone()],
                file,
                expression,
                (actual == Value::Unavailable)
                    .then_some("the candidate-domain argument or result origin is unavailable"),
            );
        }
        expected
    }
    fn contract(&self, ty: &SyntaxNode, file: FileId, value: &Value) -> bool {
        if ty.kind() == NodeKind::TypeInstVariable {
            return true;
        }
        match value {
            Value::Independent | Value::Unavailable => true,
            Value::Member(f) => {
                self.direct_reference(file, ty)
                    .and_then(|id| self.roots[id.0])
                    == Some(*f)
            }
            Value::Set(f) | Value::ComputedSet(f) => {
                ty.kind() == NodeKind::SetType
                    && ty
                        .child_nodes()
                        .last()
                        .is_some_and(|t| self.contract(t, file, &Value::Member(*f)))
            }
            Value::Array(axes) => {
                ty.kind() == NodeKind::ArrayType && {
                    let children: Vec<_> = ty.child_nodes().collect();
                    children.len() == axes.len() + 1
                        && axes.iter().zip(&children).all(|(f, t)| {
                            f.is_none_or(|f| self.contract(t, file, &Value::Member(f)))
                        })
                }
            }
        }
    }
    fn user_call(
        &mut self,
        site: (FileId, &SyntaxNode),
        id: DeclarationId,
        parameters: &[crate::TypeInst],
        values: Vec<Value>,
        frame: &mut Frame,
        view: &CallableFacts,
    ) -> Value {
        let (file, call) = site;
        let d = &self.bindings.declarations[id.0];
        let node = self.node(id).unwrap();
        let body = initializer(node);
        if self.active_calls.contains(&id)
            || body.is_none()
            || call.kind() == NodeKind::GeneratorCallExpression
        {
            let mut affected = values;
            affected.extend(self.callable_contracts(id));
            self.reject(
                &affected,
                file,
                call,
                Some("recursive, opaque or higher-order candidate propagation is unsupported"),
            );
            return Value::Unavailable;
        }
        let mut local = Frame::new();
        if let Some(list) = node
            .child_nodes()
            .find(|n| n.kind() == NodeKind::ParameterList)
        {
            for (position, parameter) in list.child_nodes().enumerate() {
                let Some(formal) =
                    crate::callables::formal_parameter(self.context, self.bindings, id, position)
                else {
                    continue;
                };
                if let Some((argument_file, argument)) = crate::callables::call_argument(
                    self.context,
                    self.bindings,
                    view,
                    file,
                    call,
                    id,
                    position,
                ) {
                    let actual = self.inspect(argument_file, argument, frame, view);
                    let actual = self.reconcile_contract(formal, (argument_file, argument), actual);
                    local.insert(formal.0, actual);
                }
                for annotation in parameter
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::Annotation)
                {
                    self.inspect(d.file, annotation, &mut local, view);
                }
            }
        }
        self.active_calls.push(id);
        // Only source typing/call selection is specialized; no value or relation proofs.
        let typed = crate::callables::instantiated_body(
            self.context,
            self.bindings,
            self.calls,
            id,
            parameters,
        );
        for annotation in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::Annotation)
        {
            self.inspect(d.file, annotation, &mut local, &typed);
        }
        let body = body.unwrap();
        let result = self.inspect(d.file, body, &mut local, &typed);
        let result = self.check_result_contract(d.file, node, body, result);
        self.active_calls.pop();
        match result {
            Value::Set(f) => Value::ComputedSet(f),
            _ => result,
        }
    }
    fn default_output(&mut self) {
        if self
            .context
            .files
            .iter()
            .filter(|f| f.kind == SourceKind::User)
            .any(|f| {
                f.parsed
                    .tree()
                    .child_nodes()
                    .any(|n| n.kind() == NodeKind::Output)
            })
        {
            return;
        }
        let mut automatic = Vec::new();
        let mut forced = Vec::new();
        let mut supplement = Vec::new();
        for d in self.bindings.declarations.iter().filter(|d| {
            d.top_level
                && d.role == DeclarationRole::Value
                && self.context.files[d.file].kind == SourceKind::User
        }) {
            let Some(node) = self.node(d.id) else {
                continue;
            };
            let value = self.values[d.id.0].clone().unwrap_or(Value::Independent);
            let mut hidden = false;
            let mut added = false;
            for annotation in node
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::Annotation)
            {
                match crate::unused_declarations::output_marker(
                    self.context,
                    self.bindings,
                    self.calls,
                    d.file,
                    annotation,
                ) {
                    Ok("no_output") => hidden = true,
                    Ok("add_to_output") => {
                        added = true;
                        forced.push(d.id);
                    }
                    Ok("output") => supplement.push(d.id),
                    Ok(_) => {}
                    Err(_) => self.reject(
                        std::slice::from_ref(&value),
                        d.file,
                        annotation,
                        Some("affected default-output markers are unknown"),
                    ),
                }
            }
            if hidden && added {
                self.reject(
                    std::slice::from_ref(&value),
                    d.file,
                    node,
                    Some("conflicting default-output markers"),
                );
            }
            let ty = &self.calls.declarations[d.id.0].ty;
            if hidden || ty.instantiation != Instantiation::Decision {
                continue;
            }
            match initializer(node) {
                None => automatic.push(d.id),
                Some(init)
                    if crate::unused_declarations::contains_anonymous(
                        self.context,
                        d.file,
                        init,
                    ) =>
                {
                    if matches!(ty.kind, TypeKind::Array { .. })
                        && matches!(
                            init.kind(),
                            NodeKind::ArrayLiteral | NodeKind::MatrixLiteral
                        )
                    {
                        automatic.push(d.id);
                    } else {
                        self.reject(
                            std::slice::from_ref(&value),
                            d.file,
                            init,
                            Some("anonymous initializer output is unsupported"),
                        );
                    }
                }
                _ => {}
            }
        }
        let selected = if forced.is_empty() { automatic } else { forced };
        for id in selected.into_iter().chain(supplement) {
            let d = &self.bindings.declarations[id.0];
            if let Some(node) = self.node(id) {
                self.reject(
                    &[self.values[id.0].clone().unwrap_or(Value::Independent)],
                    d.file,
                    node,
                    None,
                );
            }
        }
    }
}
fn initializer(node: &SyntaxNode) -> Option<&SyntaxNode> {
    node.child_nodes()
        .filter(|n| crate::callables::is_expression(n.kind()))
        .last()
}
