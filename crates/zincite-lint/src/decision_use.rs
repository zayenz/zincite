//! Conditional modelling advice over independent expression facts.
use crate::{
    BindingFacts, CallOutcome, CallableFacts, DeclarationRole, ExpressionInstantiation,
    FileFinding, FileId, Instantiation, InstantiationFacts, ModelContext, Rule, Severity,
    SourceDiagnostic, SourceKind, SourceLocation,
};
use std::collections::BTreeMap;
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

pub(super) struct DecisionUseResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}

pub(super) fn check_decision_use(
    context: &ModelContext,
    bindings: &BindingFacts,
    callables: &CallableFacts,
    facts: &InstantiationFacts,
    rule: Rule,
) -> DecisionUseResult {
    let mut checker = Checker {
        context,
        bindings,
        callables,
        rule,
        facts: facts
            .expressions
            .iter()
            .map(|f| ((f.file, f.location.range.start, f.location.range.end), f))
            .collect(),
        result: DecisionUseResult {
            findings: Vec::new(),
            limitations: Vec::new(),
        },
    };
    for (file, source) in context.files.iter().enumerate() {
        let Some(suppressions) = &source.suppressions else {
            continue;
        };
        if !source.warnings_enabled() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            if !suppressions[item].contains(&rule) {
                checker.walk(file, node);
            }
        }
    }
    checker.result
}

struct Checker<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: &'a CallableFacts,
    rule: Rule,
    facts: BTreeMap<(FileId, usize, usize), &'a ExpressionInstantiation>,
    result: DecisionUseResult,
}

fn advised_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Power
            | TokenKind::Div
            | TokenKind::Mod
            | TokenKind::Slash
            | TokenKind::Xor
            | TokenKind::Or
            | TokenKind::Implies
            | TokenKind::ReverseImplies
            | TokenKind::Equivalence
            | TokenKind::Not
    )
}

fn operator_name(name: &str) -> bool {
    matches!(
        name,
        "^" | "div" | "mod" | "/" | "xor" | "\\/" | "->" | "<-" | "<->" | "not"
    )
}

impl Checker<'_> {
    fn fact(&self, file: FileId, node: &SyntaxNode) -> Option<&ExpressionInstantiation> {
        let location = self.context.files[file].location(node.range());
        self.facts
            .get(&(file, location.range.start, location.range.end))
            .copied()
    }
    fn candidate(
        &mut self,
        file: FileId,
        nodes: &[&SyntaxNode],
        location: SourceLocation,
        description: &str,
    ) {
        if nodes.iter().any(|n| {
            self.fact(file, n)
                .is_some_and(|f| f.instantiation == Instantiation::Decision)
        }) {
            let message=match self.rule {
                Rule::DecisionVariableOperator=>format!("consider reformulating decision-dependent operator {description}; tabling may help when its finite domain is manageable"),
                Rule::DecisionVariableGenerator=>"consider a parameter-valued generator source where the model allows it; this source depends on decisions".into(),
                Rule::DecisionVariableCondition=>"consider a parameter-valued condition where the model allows it; this condition depends on decisions".into(),
                _=>unreachable!(),
            };
            self.result.findings.push(FileFinding {
                location,
                rule: self.rule,
                severity: Severity::Warning,
                message,
            });
        } else if let Some(reason) = nodes.iter().find_map(|n| match self.fact(file, n) {
            Some(f) if f.instantiation == Instantiation::Unknown => Some(f.reason.clone().unwrap()),
            None => Some("expression instantiation is unavailable".into()),
            _ => None,
        }) {
            self.limit(location, reason);
        }
    }
    fn limit(&mut self, location: SourceLocation, reason: String) {
        self.result.limitations.push(SourceDiagnostic {
            location,
            message: format!("{}: {reason}", self.rule.id()),
        });
    }
    fn builtin_operator(&self, id: crate::DeclarationId) -> bool {
        let declaration = &self.bindings.declarations[id.0];
        let file = &self.context.files[declaration.file];
        declaration.role == DeclarationRole::Function
            && file.kind == SourceKind::StandardLibrary
            && file.implicit
            && operator_name(&declaration.name)
    }
    fn walk(&mut self, file: FileId, node: &SyntaxNode) {
        let source = &self.context.files[file];
        match self.rule {
            Rule::DecisionVariableGenerator if node.kind() == NodeKind::Generator => {
                if let Some(value) = node.child_nodes().next() {
                    self.candidate(file, &[value], source.location(value.range()), "");
                }
            }
            Rule::DecisionVariableCondition
                if matches!(
                    node.kind(),
                    NodeKind::ConditionalBranch | NodeKind::WhereFilter
                ) =>
            {
                if let Some(condition) = node.child_nodes().next() {
                    self.candidate(file, &[condition], source.location(condition.range()), "");
                }
            }
            Rule::DecisionVariableOperator => {
                if matches!(
                    node.kind(),
                    NodeKind::UnaryExpression | NodeKind::BinaryExpression
                ) {
                    for child in node.children() {
                        if let SyntaxElement::Token(index) = child {
                            let token = &source.parsed.tokens()[*index];
                            if advised_operator(token.kind) {
                                self.candidate(
                                    file,
                                    &node.child_nodes().collect::<Vec<_>>(),
                                    source.location(token.range.clone()),
                                    &format!("'{}'", &source.parsed.source()[token.range.clone()]),
                                );
                            }
                        }
                    }
                }
                if matches!(
                    node.kind(),
                    NodeKind::CallExpression | NodeKind::BinaryExpression
                ) {
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
                    if let Some(call) = self
                        .callables
                        .calls
                        .iter()
                        .find(|c| c.file == file && Some(c.location.range.start) == head)
                    {
                        match &call.outcome {
                            CallOutcome::Resolved { declaration, .. }
                                if self.builtin_operator(*declaration) =>
                            {
                                self.candidate(
                                    file,
                                    &node.child_nodes().collect::<Vec<_>>(),
                                    call.location.clone(),
                                    &format!("'{}'", call.name),
                                )
                            }
                            CallOutcome::Ambiguous { candidates }
                            | CallOutcome::Unsupported { candidates, .. }
                                if candidates.iter().any(|id| self.builtin_operator(*id)) =>
                            {
                                self.limit(
                                    call.location.clone(),
                                    "builtin operator selection is ambiguous or unsupported".into(),
                                )
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        for child in node.child_nodes() {
            self.walk(file, child);
        }
    }
}
