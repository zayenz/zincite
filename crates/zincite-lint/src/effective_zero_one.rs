//! Instance-invariant zero/one formulation facts, separate from advice.
use crate::callables::{core_operation, operation_fact};
use crate::definitions::complete_array_coverage;
use crate::value_safety::{optional, traversal_expression_safety};
use crate::{
    BindingFacts, BindingResolution, CallableFacts, DeclarationId, DefinitionCoverage,
    DefinitionSafety, DomainFacts, FileFinding, FileId, InstantiationFacts, IntegerBoundsFacts,
    IntegerBoundsOutcome, ModelContext, Rule, Severity, SourceDiagnostic, SourceLocation, TypeKind,
    expression_safety,
};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectiveZeroOneFamily {
    Implication { comparison: Option<i64> },
    WholeArraySum,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectiveZeroOneOutcome {
    Eligible { formulation: String },
    NotApplicable,
    Unknown(String),
    Unsupported(String),
}
#[derive(Clone, Debug)]
pub struct EffectiveZeroOneFact {
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    pub family: EffectiveZeroOneFamily,
    pub outcome: EffectiveZeroOneOutcome,
}
#[derive(Debug, Default)]
pub struct EffectiveZeroOneFacts {
    pub expressions: Vec<EffectiveZeroOneFact>,
}

/// Interpret both same-bit implication polarities and whole-array equality sums
/// without enabling lint or rewriting source. All prerequisites belong to context.
/// Bounds/constants must hold independently of parameter defaults. A complete
/// unfiltered traversal supplies membership only for its exact array access.
pub fn resolve_effective_zero_one(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    bounds: &IntegerBoundsFacts,
) -> EffectiveZeroOneFacts {
    let interpreter = Interpreter {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        bounds,
    };
    let mut facts = EffectiveZeroOneFacts::default();
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() || !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            interpreter.walk(file, item, node, &mut facts)
        }
    }
    facts
}
struct Interpreter<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    bounds: &'a IntegerBoundsFacts,
}
impl Interpreter<'_> {
    fn walk(
        &self,
        file: FileId,
        item: usize,
        node: &SyntaxNode,
        facts: &mut EffectiveZeroOneFacts,
    ) {
        let candidate = if node.kind() == NodeKind::BinaryExpression
            && matches!(
                self.operator(file, node),
                Some(TokenKind::Implies | TokenKind::ReverseImplies)
            ) {
            let (comparison, outcome) = self.implication(file, node);
            Some((EffectiveZeroOneFamily::Implication { comparison }, outcome))
        } else if matches!(
            node.kind(),
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
        ) && operation_fact(self.context, self.calls, file, node)
            .is_some_and(|call| call.name == "sum")
        {
            Some((EffectiveZeroOneFamily::WholeArraySum, self.sum(file, node)))
        } else {
            None
        };
        if let Some((family, outcome)) = candidate {
            facts.expressions.push(EffectiveZeroOneFact {
                file,
                item,
                location: self.context.files[file].location(node.range()),
                family,
                outcome,
            });
        }
        for child in node.child_nodes() {
            self.walk(file, item, child, facts)
        }
    }
    fn operator(&self, file: FileId, node: &SyntaxNode) -> Option<TokenKind> {
        node.children().iter().find_map(|child| {
            let SyntaxElement::Token(index) = child else {
                return None;
            };
            let kind = self.context.files[file].parsed.tokens()[*index].kind;
            (!matches!(
                kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment
                    | TokenKind::BlockComment
                    | TokenKind::LeftParen
                    | TokenKind::RightParen
            ))
            .then_some(kind)
        })
    }
    fn bounds(&self, file: FileId, node: &SyntaxNode) -> IntegerBoundsOutcome {
        let range = self.context.files[file].location(node.range()).range;
        self.bounds
            .expressions
            .iter()
            .find(|fact| fact.file == file && fact.location.range == range)
            .map(|fact| fact.outcome.clone())
            .unwrap_or_else(|| {
                IntegerBoundsOutcome::Unsupported("integer bounds are unavailable".into())
            })
    }
    fn integer(&self, file: FileId, node: &SyntaxNode) -> Result<bool, String> {
        let range = self.context.files[file].location(node.range()).range;
        let ty = self
            .calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
            .ok_or("expression type is unavailable")?;
        if optional(ty) {
            return Ok(false);
        }
        if !ty.known() {
            return Err("expression type is unsupported".into());
        }
        Ok(ty.kind == TypeKind::Int)
    }
    fn equality<'n>(
        &self,
        file: FileId,
        node: &'n SyntaxNode,
    ) -> Result<(&'n SyntaxNode, &'n SyntaxNode, i64), EffectiveZeroOneOutcome> {
        use EffectiveZeroOneOutcome::*;
        let node = parentheses(node);
        if node.kind() != NodeKind::BinaryExpression
            || !matches!(
                self.operator(file, node),
                Some(TokenKind::Equal | TokenKind::DoubleEqual)
            )
        {
            return Err(NotApplicable);
        }
        match core_operation(self.context, self.bindings, self.calls, file, node, "=") {
            Ok(true) => {}
            Ok(false) => return Err(NotApplicable),
            Err(r) => return Err(Unsupported(r)),
        }
        let children: Vec<_> = node.child_nodes().collect();
        if children.len() != 2 {
            return Err(NotApplicable);
        }
        for child in &children {
            match self.integer(file, child) {
                Ok(true) => {}
                Ok(false) => return Err(NotApplicable),
                Err(r) => return Err(Unsupported(r)),
            }
        }
        let values = [
            self.bounds(file, children[0]),
            self.bounds(file, children[1]),
        ];
        for (index, value) in values.iter().enumerate() {
            if let IntegerBoundsOutcome::Known { lower, upper } = value
                && lower == upper
                && matches!(lower, 0 | 1)
            {
                return Ok((children[1 - index], children[index], *lower));
            }
        }
        if let Some(reason) = values.iter().find_map(|v| {
            if let IntegerBoundsOutcome::Unsupported(r) = v {
                Some(r)
            } else {
                None
            }
        }) {
            return Err(Unsupported(reason.clone()));
        }
        if let Some(reason) = values.iter().find_map(|v| {
            if let IntegerBoundsOutcome::Unknown(r) = v {
                Some(r)
            } else {
                None
            }
        }) {
            return Err(Unknown(reason.clone()));
        }
        Err(NotApplicable)
    }
    fn implication(
        &self,
        file: FileId,
        node: &SyntaxNode,
    ) -> (Option<i64>, EffectiveZeroOneOutcome) {
        use EffectiveZeroOneOutcome::*;
        let name = if self.operator(file, node) == Some(TokenKind::ReverseImplies) {
            "<-"
        } else {
            "->"
        };
        match core_operation(self.context, self.bindings, self.calls, file, node, name) {
            Ok(true) => {}
            Ok(false) => return (None, NotApplicable),
            Err(r) => return (None, Unsupported(r)),
        }
        let mut children: Vec<_> = node.child_nodes().collect();
        if children.len() != 2 {
            return (None, NotApplicable);
        }
        if name == "<-" {
            children.swap(0, 1)
        }
        let (left, lc, lbit) = match self.equality(file, children[0]) {
            Ok(v) => v,
            Err(outcome) => return (None, outcome),
        };
        let (right, rc, rbit) = match self.equality(file, children[1]) {
            Ok(v) => v,
            Err(outcome) => return (Some(lbit), outcome),
        };
        if lbit != rbit {
            return (Some(lbit), NotApplicable);
        }
        for value in [left, right] {
            match self.bounds(file, value) {
                IntegerBoundsOutcome::Known { lower, upper } if lower >= 0 && upper <= 1 => {}
                IntegerBoundsOutcome::Known { .. } => return (Some(lbit), NotApplicable),
                IntegerBoundsOutcome::Unknown(r) => return (Some(lbit), Unknown(r)),
                IntegerBoundsOutcome::Unsupported(r) => return (Some(lbit), Unsupported(r)),
            }
        }
        for value in [left, right, lc, rc] {
            match expression_safety(self.context, self.bindings, self.calls, file, value) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(r) => return (Some(lbit), Unknown(r)),
                DefinitionSafety::Unsupported(r) => return (Some(lbit), Unsupported(r)),
            }
        }
        let relation = if lbit == 1 { "<=" } else { ">=" };
        (
            Some(lbit),
            Eligible {
                formulation: format!(
                    "{} {relation} {}",
                    self.text(file, left),
                    self.text(file, right)
                ),
            },
        )
    }
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let range = self.context.files[file].location(node.range()).range;
        self.bindings
            .references
            .iter()
            .find(|r| {
                r.file == file
                    && range.start <= r.location.range.start
                    && r.location.range.end <= range.end
            })
            .and_then(|r| {
                if let BindingResolution::Resolved(id) = r.resolution {
                    Some(id)
                } else {
                    None
                }
            })
    }
    fn sum(&self, file: FileId, node: &SyntaxNode) -> EffectiveZeroOneOutcome {
        use EffectiveZeroOneOutcome::*;
        match core_operation(self.context, self.bindings, self.calls, file, node, "sum") {
            Ok(true) => {}
            Ok(false) => return NotApplicable,
            Err(r) => return Unsupported(r),
        }
        let Some(crate::CallOutcome::Resolved {
            parameters,
            return_type,
            ..
        }) = operation_fact(self.context, self.calls, file, node).map(|call| &call.outcome)
        else {
            return Unsupported("sum signature is unavailable".into());
        };
        if optional(return_type)
            || return_type.kind != TypeKind::Int
            || parameters.len() != 1
            || !matches!(&parameters[0].kind,TypeKind::Array{element,..} if element.kind==TypeKind::Int && !optional(element))
        {
            return NotApplicable;
        }
        let quantified = if node.kind() == NodeKind::GeneratorCallExpression {
            node
        } else {
            let Some(child) = node
                .child_nodes()
                .next()
                .map(parentheses)
                .filter(|n| n.kind() == NodeKind::ArrayComprehension)
            else {
                return NotApplicable;
            };
            child
        };
        let Some(body) = quantified
            .child_nodes()
            .find(|n| n.kind() != NodeKind::GeneratorList)
        else {
            return NotApplicable;
        };
        let (value, constant, bit) = match self.equality(file, body) {
            Ok(v) => v,
            Err(o) => return o,
        };
        if bit != 1 {
            return NotApplicable;
        }
        let access = parentheses(value);
        if access.kind() != NodeKind::ArrayAccessExpression {
            return NotApplicable;
        }
        let parts: Vec<_> = access.child_nodes().collect();
        let Some(subject) = parts
            .first()
            .copied()
            .map(parentheses)
            .filter(|n| n.kind() == NodeKind::Expression)
        else {
            return NotApplicable;
        };
        let Some(id) = self.reference(file, subject) else {
            return Unsupported("array identity is unresolved or ambiguous".into());
        };
        let ty = &self.calls.declarations[id.0].ty;
        if optional(ty) {
            return NotApplicable;
        }
        let TypeKind::Array { indices, element } = &ty.kind else {
            return NotApplicable;
        };
        if element.kind != TypeKind::Int {
            return NotApplicable;
        }
        let Some(list) = quantified
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)
        else {
            return NotApplicable;
        };
        let generators: Vec<_> = list.child_nodes().collect();
        match complete_array_coverage(
            self.context,
            self.bindings,
            self.instantiations,
            self.domains,
            (file, id),
            (&parts[1..], &generators),
        ) {
            DefinitionCoverage::WholeArray => {}
            DefinitionCoverage::Unproved(r) => return Unknown(r),
            DefinitionCoverage::Unsupported(r) => return Unsupported(r),
            _ => return NotApplicable,
        }
        if indices.len() != 1 {
            return Unsupported("sum(array) is supported only for one-dimensional arrays".into());
        }
        match self.bounds(file, access) {
            IntegerBoundsOutcome::Known { lower, upper } if lower >= 0 && upper <= 1 => {}
            IntegerBoundsOutcome::Known { .. } => return NotApplicable,
            IntegerBoundsOutcome::Unknown(r) => return Unknown(r),
            IntegerBoundsOutcome::Unsupported(r) => return Unsupported(r),
        }
        for value in [access, constant] {
            match traversal_expression_safety(
                self.context,
                self.bindings,
                self.calls,
                file,
                value,
                access,
            ) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(r) => return Unknown(r),
                DefinitionSafety::Unsupported(r) => return Unsupported(r),
            }
        }
        for generator in generators {
            let Some(source) = generator.child_nodes().next() else {
                return NotApplicable;
            };
            if let Err(reason) = crate::domains::core_arithmetic(
                self.context,
                self.bindings,
                self.calls,
                file,
                source,
            ) {
                return Unsupported(reason);
            }
            match traversal_expression_safety(
                self.context,
                self.bindings,
                self.calls,
                file,
                source,
                access,
            ) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(r) => return Unknown(r),
                DefinitionSafety::Unsupported(r) => return Unsupported(r),
            }
        }
        Eligible {
            formulation: format!("sum({})", self.text(file, subject)),
        }
    }
    fn text(&self, file: FileId, node: &SyntaxNode) -> &str {
        self.context.files[file].parsed.source()[node.range()].trim()
    }
}
fn parentheses(mut node: &SyntaxNode) -> &SyntaxNode {
    while node.kind() == NodeKind::ParenthesizedExpression {
        let Some(child) = node.child_nodes().next() else {
            break;
        };
        node = child;
    }
    node
}
pub(super) struct EffectiveResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}
pub(super) fn check_effective_zero_one(
    context: &ModelContext,
    facts: &EffectiveZeroOneFacts,
) -> EffectiveResult {
    let mut result = EffectiveResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    for fact in &facts.expressions {
        let file = &context.files[fact.file];
        let Some(suppressions) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled() || suppressions[fact.item].contains(&Rule::EffectiveZeroOne) {
            continue;
        }
        match &fact.outcome {
            EffectiveZeroOneOutcome::Eligible{formulation}=>result.findings.push(FileFinding{location:fact.location.clone(),rule:Rule::EffectiveZeroOne,severity:Severity::Warning,message:format!("consider {formulation} as an equivalent formulation for these proved zero/one values; compare readability for your model")}),
            EffectiveZeroOneOutcome::Unsupported(reason)=>result.limitations.push(SourceDiagnostic{location:fact.location.clone(),message:format!("effective-zero-one: {reason}")}),
            EffectiveZeroOneOutcome::NotApplicable|EffectiveZeroOneOutcome::Unknown(_)=>{},
        }
    }
    result
}
