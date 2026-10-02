//! Two bounded decomposition facts, independent of advisory policy.
//! Actual index membership, selected core identities and raw totality are required;
//! these facts authorize neither source edits nor claims about solving speed.
use crate::callables::core_operation;
use crate::definitions::{annotations_safe, resolved_reference};
use crate::domains::tokens;
use crate::{
    BindingFacts, CallableFacts, DeclarationId, DefinitionEnforcement, FileFinding, FileId,
    GuardActivation, GuardedFacts, GuardedOutcome, Instantiation, IterationFacts,
    IterationIndexSet, ModelContext, OptionalFacts, Rule, Severity, SourceDiagnostic,
    SourceLocation, TypeInst, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

#[derive(Clone, Debug)]
pub enum GlobalPattern {
    /// Both binders cover this array's actual index set; the sole i<j or i!=j
    /// filter selects every unordered pair. No pair enumeration is needed.
    PairwiseDisequality {
        array: DeclarationId,
        binders: [DeclarationId; 2],
    },
    /// Each cover position has matching full-array indicator sums and par bounds.
    /// xs is proved nonempty; values outside cover remain unrestricted.
    OccurrenceBounds {
        array: DeclarationId,
        cover: DeclarationId,
        lower: DeclarationId,
        upper: DeclarationId,
    },
}
#[derive(Clone, Debug)]
pub enum GlobalPatternOutcome {
    Complete(GlobalPattern),
    /// A known exclusion from these two documented forms, not invalid source.
    Incomplete(String),
    Unknown(String),
    Unsupported(String),
}
#[derive(Clone, Debug)]
pub struct GlobalPatternFact {
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    pub outcome: GlobalPatternOutcome,
}

/// Inspect the supported generator-forall forms using prerequisites from the
/// same ModelContext. Conditional/reified contexts, semantic annotations,
/// optional equality and unknown evaluation safety do not prove a decomposition.
/// String labels and all original source locations are retained. No global call
/// is executed and no solver, generic pattern search or source rewrite is used.
pub fn resolve_global_patterns(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    optional: &OptionalFacts,
    guarded: &GuardedFacts,
    iteration: &IterationFacts,
) -> Vec<GlobalPatternFact> {
    let producer = Producer {
        context,
        bindings,
        calls,
        optional,
        guarded,
        iteration,
    };
    let mut facts = Vec::new();
    for (file, source) in context.files.iter().enumerate() {
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            producer.walk(file, item, node, &mut facts);
        }
    }
    facts
}
struct Producer<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    optional: &'a OptionalFacts,
    guarded: &'a GuardedFacts,
    iteration: &'a IterationFacts,
}
type Check<T> = Result<T, GlobalPatternOutcome>;
struct OccurrenceBound {
    lower: bool,
    array: DeclarationId,
    cover: DeclarationId,
    bounds: DeclarationId,
}
impl Producer<'_> {
    fn walk(
        &self,
        file: FileId,
        item: usize,
        node: &SyntaxNode,
        facts: &mut Vec<GlobalPatternFact>,
    ) {
        if node.kind() == NodeKind::GeneratorCallExpression
            && tokens(&self.context.files[file].parsed, node)
                .first()
                .is_some_and(|t| {
                    self.context.files[file].parsed.source()[t.range.clone()].trim_matches('\'')
                        == "forall"
                })
            && let Some(body) = node
                .child_nodes()
                .find(|n| n.kind() != NodeKind::GeneratorList)
        {
            let body = unwrap(body);
            let result = match self.operator(file, body) {
                Some(TokenKind::NotEqual)
                    if body
                        .child_nodes()
                        .all(|c| unwrap(c).kind() == NodeKind::ArrayAccessExpression) =>
                {
                    Some(self.pairwise(file, node, body))
                }
                Some(TokenKind::And)
                    if body.child_nodes().count() == 2
                        && body.child_nodes().all(|n| self.count_bound_shape(file, n)) =>
                {
                    Some(self.occurrences(file, node, body))
                }
                _ => None,
            };
            if let Some(result) = result {
                facts.push(GlobalPatternFact {
                    file,
                    item,
                    location: self.location(file, node),
                    outcome: result
                        .map(GlobalPatternOutcome::Complete)
                        .unwrap_or_else(|o| o),
                });
            }
        }
        for child in node.child_nodes() {
            self.walk(file, item, child, facts);
        }
    }
    // Syntax only admits candidates here; selected core identities below prove them.
    fn count_bound_shape(&self, file: FileId, node: &SyntaxNode) -> bool {
        let node = unwrap(node);
        if !matches!(
            self.operator(file, node),
            Some(TokenKind::LessEqual | TokenKind::GreaterEqual)
        ) {
            return false;
        }
        node.child_nodes().any(|n| {
            let n = unwrap(n);
            n.kind() == NodeKind::GeneratorCallExpression
                && self.head_is(file, n, "sum")
                && n.child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList)
                    .is_some_and(|n| {
                        let n = unwrap(n);
                        n.kind() == NodeKind::CallExpression
                            && self.head_is(file, n, "bool2int")
                            && n.child_nodes().next().is_some_and(|e| {
                                matches!(
                                    self.operator(file, unwrap(e)),
                                    Some(TokenKind::Equal | TokenKind::DoubleEqual)
                                )
                            })
                    })
        })
    }
    fn head_is(&self, file: FileId, node: &SyntaxNode, name: &str) -> bool {
        tokens(&self.context.files[file].parsed, node)
            .first()
            .is_some_and(|t| {
                self.context.files[file].parsed.source()[t.range.clone()].trim_matches('\'') == name
            })
    }
    fn location(&self, file: FileId, node: &SyntaxNode) -> SourceLocation {
        self.context.files[file].location(node.range())
    }
    fn operator(&self, file: FileId, node: &SyntaxNode) -> Option<TokenKind> {
        (node.kind() == NodeKind::BinaryExpression)
            .then(|| {
                tokens(&self.context.files[file].parsed, node)
                    .first()
                    .map(|t| t.kind)
            })
            .flatten()
    }
    fn core(&self, file: FileId, node: &SyntaxNode, name: &str) -> Check<()> {
        match core_operation(self.context, self.bindings, self.calls, file, node, name) {
            Ok(true) => Ok(()),
            Ok(false) => Err(GlobalPatternOutcome::Incomplete(format!(
                "selected {name} is not a core operation"
            ))),
            Err(reason) => Err(GlobalPatternOutcome::Unsupported(reason)),
        }
    }
    fn total(&self, file: FileId, node: &SyntaxNode) -> Check<()> {
        let Some(g) = self.guarded.expression(file, &self.location(file, node)) else {
            return unknown("evaluation facts are unavailable");
        };
        match &g.raw_definedness {
            GuardedOutcome::Proven if g.definedness == GuardedOutcome::Proven => Ok(()),
            GuardedOutcome::Unsupported(reason) => {
                Err(GlobalPatternOutcome::Unsupported(reason.clone()))
            }
            GuardedOutcome::Refuted => {
                incomplete("raw evaluation is refuted despite a possible relational Boolean value")
            }
            _ => unknown("raw evaluation totality is unproved"),
        }
    }
    fn enforced(&self, file: FileId, node: &SyntaxNode) -> Check<()> {
        let Some(g) = self.guarded.expression(file, &self.location(file, node)) else {
            return unknown("enforced context is unavailable");
        };
        match &g.context.enforcement {
            DefinitionEnforcement::Unsupported(reason) => {
                return Err(GlobalPatternOutcome::Unsupported(reason.clone()));
            }
            DefinitionEnforcement::Enforced if g.context.activation == GuardActivation::Active => {}
            _ => return incomplete("decomposition is conditional, inactive or reified"),
        }
        self.annotation_tree(file, node)?;
        Ok(())
    }
    fn annotation_tree(&self, file: FileId, node: &SyntaxNode) -> Check<()> {
        if !annotations_safe(self.context, file, node) {
            return Err(GlobalPatternOutcome::Unsupported(
                "semantic annotation lacks a transparent contract".into(),
            ));
        }
        for c in node.child_nodes() {
            self.annotation_tree(file, c)?;
        }
        Ok(())
    }
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let node = unwrap(node);
        (node.kind() == NodeKind::Expression)
            .then(|| resolved_reference(self.context, self.bindings, file, node))
            .flatten()
    }
    fn access(&self, file: FileId, node: &SyntaxNode) -> Check<(DeclarationId, DeclarationId)> {
        let node = unwrap(node);
        let children: Vec<_> = node.child_nodes().collect();
        if node.kind() != NodeKind::ArrayAccessExpression || children.len() != 2 {
            return incomplete("access is not a direct one-dimensional array[binder]");
        }
        let Some(array) = self.reference(file, children[0]) else {
            return incomplete("array expression is not a direct declaration");
        };
        let Some(binder) = self.reference(file, children[1]) else {
            return incomplete("index is shifted or not a resolved binder");
        };
        self.total(file, node)?;
        Ok((array, binder))
    }
    fn array_type(&self, array: DeclarationId) -> Check<&TypeInst> {
        let Some(t) = self
            .calls
            .declarations
            .iter()
            .find(|d| d.declaration == array)
            .map(|d| &d.ty)
        else {
            return unknown("array type is unavailable");
        };
        match &t.kind {
            TypeKind::Array { indices, element }
                if indices.len() == 1 && !t.optional && !element.optional =>
            {
                Ok(element)
            }
            TypeKind::Unknown(reason) => Err(GlobalPatternOutcome::Unknown(reason.clone())),
            _ => incomplete("array must be one-dimensional and nonoptional"),
        }
    }
    fn equality_type(&self, array: DeclarationId) -> Check<&TypeKind> {
        let element = self.array_type(array)?;
        match &element.kind {
            TypeKind::Int | TypeKind::Enum(_) => Ok(&element.kind),
            TypeKind::Unknown(reason) => Err(GlobalPatternOutcome::Unknown(reason.clone())),
            _ => incomplete("equality element type is outside nonoptional integer/enum support"),
        }
    }
    fn dimension(&self, array: DeclarationId) -> Check<&IterationIndexSet> {
        let Some(a) = self
            .iteration
            .arrays
            .iter()
            .find(|a| a.declaration == array)
        else {
            return unknown("actual array indices are unavailable");
        };
        if a.dimensions.len() != 1 {
            return incomplete("array rank differs from one");
        }
        Ok(&a.dimensions[0])
    }
    fn equal_indices(&self, left: &IterationIndexSet, right: &IterationIndexSet) -> Check<()> {
        match left.equal_members(right) {
            GuardedOutcome::Proven => Ok(()),
            GuardedOutcome::Refuted => incomplete("actual index sets differ"),
            GuardedOutcome::Unknown => unknown("actual index equality is unproved"),
            GuardedOutcome::Unsupported(reason) => Err(GlobalPatternOutcome::Unsupported(reason)),
        }
    }
    fn traversal(
        &self,
        file: FileId,
        node: &SyntaxNode,
        slots: usize,
    ) -> Check<&crate::IterationFact> {
        let Some(i) = self.iteration.iteration(file, &self.location(file, node)) else {
            return unknown("iteration facts are unavailable");
        };
        if i.generators.iter().map(|g| g.slots).sum::<usize>() != slots
            || i.generators.iter().any(|g| {
                !g.membership
                    || g.bindings.len() != g.slots
                    || g.source_instantiation != Instantiation::Parameter
                    || g.dependencies.iter().any(|id| {
                        self.bindings.declarations[id.0].role == crate::DeclarationRole::Generator
                    })
            })
        {
            return incomplete("requires independent named parameter-domain binders");
        }
        Ok(i)
    }
    fn pairwise(&self, file: FileId, node: &SyntaxNode, body: &SyntaxNode) -> Check<GlobalPattern> {
        self.enforced(file, node)?;
        let children: Vec<_> = body.child_nodes().collect();
        if children.len() != 2 {
            return incomplete("disequality arity differs from two");
        }
        let (array, a) = self.access(file, children[0])?;
        let (other, b) = self.access(file, children[1])?;
        if array != other || a == b {
            return incomplete("pair accesses do not use one array and distinct binders");
        }
        self.equality_type(array)?;
        self.core(file, body, "!=")?;
        self.total(file, body)?;
        let i = self.traversal(file, node, 2)?;
        let binders: Vec<_> = i
            .generators
            .iter()
            .flat_map(|g| g.bindings.iter().copied())
            .collect();
        if binders.len() != 2
            || !binders.contains(&a)
            || !binders.contains(&b)
            || i.filters.len() != 1
        {
            return incomplete("requires exactly two binders and one unordered-pair selector");
        }
        for g in &i.generators {
            self.equal_indices(&g.index_set, self.dimension(array)?)?;
        }
        let Some(filter) = find_node(
            node,
            &i.filters[0].location,
            self.context.files[file].byte_offset,
        ) else {
            return unknown("selector source is unavailable");
        };
        let filter = unwrap(filter);
        let op = self.operator(file, filter);
        let name = match op {
            Some(TokenKind::Less) => "<",
            Some(TokenKind::NotEqual) => "!=",
            _ => return incomplete("selector is not i<j or i!=j"),
        };
        self.core(file, filter, name)?;
        self.total(file, filter)?;
        let c: Vec<_> = filter.child_nodes().collect();
        if c.len() != 2 {
            return incomplete("selector arity differs from two");
        }
        let ids = [self.reference(file, c[0]), self.reference(file, c[1])];
        if !([Some(a), Some(b)] == ids || [Some(b), Some(a)] == ids) {
            return incomplete("selector subjects differ from the accessed binders");
        }
        self.core(file, node, "forall")?;
        self.total(file, node)?;
        Ok(GlobalPattern::PairwiseDisequality {
            array,
            binders: [a, b],
        })
    }
    fn occurrence_sum(
        &self,
        file: FileId,
        sum: &SyntaxNode,
        outer: DeclarationId,
    ) -> Check<(DeclarationId, DeclarationId)> {
        let sum = unwrap(sum);
        if sum.kind() != NodeKind::GeneratorCallExpression {
            return incomplete("count is not a generator sum");
        }
        let i = self.traversal(file, sum, 1)?;
        if !i.filters.is_empty() {
            return incomplete("indicator count is filtered");
        }
        let inner = i.generators[0].bindings[0];
        let Some(body) = sum
            .child_nodes()
            .find(|n| n.kind() != NodeKind::GeneratorList)
        else {
            return incomplete("sum body is absent");
        };
        let body = unwrap(body);
        let c: Vec<_> = body.child_nodes().collect();
        if body.kind() != NodeKind::CallExpression || c.len() != 1 {
            return incomplete("count body is not scalar bool2int(equality)");
        }
        let equality = unwrap(c[0]);
        if !matches!(
            self.operator(file, equality),
            Some(TokenKind::Equal | TokenKind::DoubleEqual)
        ) {
            return incomplete("indicator body is not equality");
        }
        let c: Vec<_> = equality.child_nodes().collect();
        if c.len() != 2 {
            return incomplete("equality arity differs from two");
        }
        let left = self.access(file, c[0])?;
        let right = self.access(file, c[1])?;
        let ((array, _), (cover, _)) = if left.1 == inner && right.1 == outer {
            (left, right)
        } else if right.1 == inner && left.1 == outer {
            (right, left)
        } else {
            return incomplete("indicator does not compare xs[inner] with cover[outer]");
        };
        if self.equality_type(array)? != self.equality_type(cover)? {
            return incomplete("xs and cover equality types differ");
        }
        self.equal_indices(&i.generators[0].index_set, self.dimension(array)?)?;
        self.core(file, equality, "=")?;
        self.total(file, equality)?;
        self.core(file, body, "bool2int")?;
        self.core(file, sum, "sum")?;
        self.total(file, sum)?;
        Ok((array, cover))
    }
    fn bound(
        &self,
        file: FileId,
        node: &SyntaxNode,
        outer: DeclarationId,
    ) -> Check<OccurrenceBound> {
        let node = unwrap(node);
        let op = self.operator(file, node);
        let c: Vec<_> = node.child_nodes().collect();
        if c.len() != 2 || !matches!(op, Some(TokenKind::LessEqual | TokenKind::GreaterEqual)) {
            return incomplete("bound is not a supported <= or >= comparison");
        }
        let left_sum = unwrap(c[0]).kind() == NodeKind::GeneratorCallExpression;
        let (sum, bound) = if left_sum { (c[0], c[1]) } else { (c[1], c[0]) };
        let lower = (op == Some(TokenKind::GreaterEqual)) == left_sum;
        let (array, cover) = self.occurrence_sum(file, sum, outer)?;
        let (bounds, index) = self.access(file, bound)?;
        if index != outer {
            return incomplete("bound index differs from cover position");
        }
        let ty = self.array_type(bounds)?;
        if ty.kind != TypeKind::Int || ty.instantiation != Instantiation::Parameter {
            return incomplete("bounds must be par nonoptional integer arrays");
        }
        self.core(
            file,
            node,
            if op == Some(TokenKind::LessEqual) {
                "<="
            } else {
                ">="
            },
        )?;
        self.total(file, node)?;
        Ok(OccurrenceBound {
            lower,
            array,
            cover,
            bounds,
        })
    }
    fn occurrences(
        &self,
        file: FileId,
        node: &SyntaxNode,
        body: &SyntaxNode,
    ) -> Check<GlobalPattern> {
        self.enforced(file, node)?;
        let i = self.traversal(file, node, 1)?;
        if !i.filters.is_empty() {
            return incomplete("cover traversal is filtered");
        }
        let outer = i.generators[0].bindings[0];
        let c: Vec<_> = body.child_nodes().collect();
        if c.len() != 2 {
            return incomplete("requires one lower and one upper occurrence bound");
        }
        let a = self.bound(file, c[0], outer)?;
        let b = self.bound(file, c[1], outer)?;
        if a.lower == b.lower || a.array != b.array || a.cover != b.cover {
            return incomplete(
                "bounds do not count the same xs and cover with opposite directions",
            );
        }
        let (lower, upper) = if a.lower {
            (a.bounds, b.bounds)
        } else {
            (b.bounds, a.bounds)
        };
        if self.array_type(a.cover)?.instantiation != Instantiation::Parameter {
            return incomplete("cover must be a par array");
        }
        self.equal_indices(&i.generators[0].index_set, self.dimension(a.cover)?)?;
        for bound in [lower, upper] {
            self.equal_indices(self.dimension(a.cover)?, self.dimension(bound)?)?;
        }
        let nonempty = self
            .optional
            .declaration(a.array)
            .and_then(|d| d.collection.as_ref())
            .and_then(|c| c.capacity.bounds())
            .is_some_and(|(lower, _)| lower > 0);
        if !nonempty {
            return unknown(
                "xs nonemptiness is unproved; empty-array global assertion behavior is excluded",
            );
        }
        self.core(file, body, "/\\")?;
        self.core(file, node, "forall")?;
        self.total(file, node)?;
        Ok(GlobalPattern::OccurrenceBounds {
            array: a.array,
            cover: a.cover,
            lower,
            upper,
        })
    }
}
fn incomplete<T>(reason: &str) -> Check<T> {
    Err(GlobalPatternOutcome::Incomplete(reason.into()))
}
fn unknown<T>(reason: &str) -> Check<T> {
    Err(GlobalPatternOutcome::Unknown(reason.into()))
}
fn unwrap(mut node: &SyntaxNode) -> &SyntaxNode {
    while matches!(
        node.kind(),
        NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression
    ) {
        let Some(c) = node.child_nodes().next() else {
            break;
        };
        node = c;
    }
    node
}
fn find_node<'a>(
    node: &'a SyntaxNode,
    location: &SourceLocation,
    offset: usize,
) -> Option<&'a SyntaxNode> {
    if node.range().start + offset == location.range.start
        && node.range().end + offset == location.range.end
    {
        return Some(node);
    }
    node.child_nodes()
        .find_map(|c| find_node(c, location, offset))
}
pub(super) fn check_global_patterns(
    context: &ModelContext,
    bindings: &BindingFacts,
    facts: &[GlobalPatternFact],
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    for fact in facts {
        let file = &context.files[fact.file];
        if !file.warnings_enabled()
            || file
                .suppressions
                .as_ref()
                .is_none_or(|s| s[fact.item].contains(&Rule::GlobalConstraintOpportunity))
        {
            continue;
        }
        let message = match &fact.outcome {
            GlobalPatternOutcome::Complete(GlobalPattern::PairwiseDisequality {
                array, ..
            }) => format!(
                "complete pairwise disequality covers every unordered pair of '{}'; consider all_different({}) with matching nonoptional equality and actual indices",
                bindings.declarations[array.0].name, bindings.declarations[array.0].name
            ),
            GlobalPatternOutcome::Complete(GlobalPattern::OccurrenceBounds {
                array,
                cover,
                lower,
                upper,
            }) => format!(
                "matching whole-array lower/upper occurrence counts; consider open global_cardinality({}, {}, {}, {}), leaving values outside cover unrestricted",
                bindings.declarations[array.0].name,
                bindings.declarations[cover.0].name,
                bindings.declarations[lower.0].name,
                bindings.declarations[upper.0].name
            ),
            GlobalPatternOutcome::Unsupported(reason) => {
                limitations.push(SourceDiagnostic {
                    location: fact.location.clone(),
                    message: format!("global-constraint-opportunity: {reason}"),
                });
                continue;
            }
            _ => continue,
        };
        findings.push(FileFinding {
            location: fact.location.clone(),
            rule: Rule::GlobalConstraintOpportunity,
            severity: Severity::Warning,
            message: format!("{message}; raw evaluation is proved total in this enforced context; advice offers no automatic edit or solving-speed claim"),
        });
    }
    (findings, limitations)
}
