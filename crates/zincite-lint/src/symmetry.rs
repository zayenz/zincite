//! Resolved likely symmetry-breaking predicates and enclosing explicit markers.
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

use crate::callables::operation_fact;
use crate::{
    BindingFacts, CallOutcome, CallableFacts, DeclarationId, DeclarationRole, FileFinding, FileId,
    Instantiation, ModelContext, Rule, Severity, SourceDiagnostic, SourceKind, SourceLocation,
    TypeKind,
};

const FAMILY: [&str; 11] = [
    "lex2",
    "lex_greater",
    "lex_greatereq",
    "lex_less",
    "lex_lesseq",
    "strict_lex2",
    "seq_precede_chain",
    "value_precede",
    "value_precede_chain",
    "increasing",
    "decreasing",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymmetryUseOutcome {
    Marked,
    Unmarked,
    Unsupported(String),
}

#[derive(Clone, Debug)]
pub struct SymmetryUse {
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    /// None when a potentially matching standard predicate could not be selected.
    pub declaration: Option<DeclarationId>,
    pub outcome: SymmetryUseOutcome,
}

#[derive(Debug, Default)]
pub struct SymmetryFacts {
    pub uses: Vec<SymmetryUse>,
}

/// Interpret the finite predicate family and core symmetry wrapper without
/// inferring actual model symmetries. Bindings and callables must belong to the
/// same retained ModelContext. User lookalikes retain their own identities.
pub fn resolve_symmetry_uses(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
) -> SymmetryFacts {
    let mut walker = Walker {
        context,
        bindings,
        calls,
        facts: SymmetryFacts::default(),
    };
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() || source.suppressions.is_none() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            walker.walk(file, item, node, Some(false));
        }
    }
    walker.facts
}

struct Walker<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    facts: SymmetryFacts,
}
impl Walker<'_> {
    fn standard_predicate(&self, id: DeclarationId, implicit: bool) -> bool {
        let declaration = &self.bindings.declarations[id.0];
        let source = &self.context.files[declaration.file];
        declaration.role == DeclarationRole::Predicate
            && source.kind == SourceKind::StandardLibrary
            && source.implicit == implicit
    }
    fn family_predicate(&self, id: DeclarationId) -> bool {
        self.standard_predicate(id, false)
            && FAMILY.contains(&self.bindings.declarations[id.0].name.as_str())
    }
    fn wrapper(&self, id: DeclarationId) -> bool {
        let declaration = &self.bindings.declarations[id.0];
        declaration.role == DeclarationRole::Predicate
            && self.context.files[declaration.file].kind == SourceKind::StandardLibrary
            && declaration.name == "symmetry_breaking_constraint"
    }
    fn location(&self, file: FileId, node: &SyntaxNode) -> SourceLocation {
        let range = node.range();
        let mut tokens = self.context.files[file]
            .parsed
            .tokens()
            .iter()
            .filter(|token| {
                !matches!(
                    token.kind,
                    TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                ) && token.range.start >= range.start
                    && token.range.end <= range.end
            });
        let first = tokens.next().unwrap();
        let last = tokens.next_back().unwrap_or(first);
        self.context.files[file].location(first.range.start..last.range.end)
    }
    fn walk(&mut self, file: FileId, item: usize, node: &SyntaxNode, marked: Option<bool>) {
        let mut child_marker = marked;
        if matches!(
            node.kind(),
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
        ) && let Some(call) = operation_fact(self.context, self.calls, file, node)
        {
            let mut target = None;
            match &call.outcome {
                CallOutcome::Resolved {
                    declaration,
                    parameters,
                    return_type,
                } => {
                    if self.wrapper(*declaration)
                        && return_type.kind == TypeKind::Bool
                        && !return_type.optional
                        && return_type.instantiation == Instantiation::Decision
                        && matches!(parameters.as_slice(), [parameter] if parameter == return_type)
                    {
                        child_marker = Some(true);
                    }
                    if self.family_predicate(*declaration) && return_type.kind == TypeKind::Bool {
                        target = Some((
                            Some(*declaration),
                            match marked {
                                Some(true) => SymmetryUseOutcome::Marked,
                                Some(false) => SymmetryUseOutcome::Unmarked,
                                None => SymmetryUseOutcome::Unsupported(
                                    "enclosing symmetry wrapper identity is unknown".into(),
                                ),
                            },
                        ));
                    }
                }
                CallOutcome::Unsupported { candidates, reason } => {
                    if candidates.iter().any(|id| self.wrapper(*id)) && marked != Some(true) {
                        child_marker = None;
                    }
                    if candidates.iter().any(|id| self.family_predicate(*id)) {
                        target = Some((None, SymmetryUseOutcome::Unsupported(reason.clone())));
                    }
                }
                CallOutcome::Ambiguous { candidates } => {
                    if candidates.iter().any(|id| self.wrapper(*id)) && marked != Some(true) {
                        child_marker = None;
                    }
                    if candidates.iter().any(|id| self.family_predicate(*id)) {
                        target = Some((
                            None,
                            SymmetryUseOutcome::Unsupported(
                                "standard predicate selection is ambiguous".into(),
                            ),
                        ));
                    }
                }
                CallOutcome::Unresolved { reason } => {
                    // Spelling only identifies an uncertainty; it never proves a
                    // standard identity or establishes a symmetry marker.
                    if call.name == "symmetry_breaking_constraint" && marked != Some(true) {
                        child_marker = None;
                    }
                    if FAMILY.contains(&call.name.as_str()) {
                        target = Some((None, SymmetryUseOutcome::Unsupported(reason.clone())));
                    }
                }
                _ => {}
            }
            if let Some((declaration, outcome)) = target {
                self.facts.uses.push(SymmetryUse {
                    file,
                    item,
                    location: self.location(file, node),
                    declaration,
                    outcome,
                });
            }
        }
        for child in node.child_nodes() {
            self.walk(file, item, child, child_marker);
        }
    }
}

pub(super) fn check_symmetry_uses(
    context: &ModelContext,
    facts: &SymmetryFacts,
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    for usage in &facts.uses {
        let file = &context.files[usage.file];
        if file
            .suppressions
            .as_ref()
            .is_none_or(|s| s[usage.item].contains(&Rule::UnmarkedSymmetryBreaking))
        {
            continue;
        }
        match &usage.outcome {
            SymmetryUseOutcome::Unmarked => findings.push(FileFinding { fix: None, location: usage.location.clone(), rule: Rule::UnmarkedSymmetryBreaking, severity: Severity::Warning, message: "is this constraint intended to break symmetry? If so, consider symmetry_breaking_constraint(...); it may instead express required model logic".into() }),
            SymmetryUseOutcome::Unsupported(reason) => limitations.push(SourceDiagnostic { location: usage.location.clone(), message: format!("unmarked-symmetry-breaking: {reason}") }),
            SymmetryUseOutcome::Marked => {}
        }
    }
    (findings, limitations)
}
