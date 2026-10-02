//! Compare complete scalar integer RHS values with their own explicit contracts.
//! An interval is a conservative hull. It neither enumerates possible values nor
//! supplies a model-feasibility witness, and nested children inherit no contract.
use crate::domains::invariant_integer;
use crate::{
    BindingFacts, DeclarationId, DefinitionCoverage, DefinitionFacts, Domain, FileFinding, FileId,
    ModelContext, NumericDeclaration, NumericDomainRelation, NumericFacts, NumericOutcome, Rule,
    Severity, SourceDiagnostic, SourceLocation,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumericContractRelation {
    Contradiction,
    Compatible,
    /// The hull overlaps but is not contained. No actual failing value is proved.
    PossibleConflict,
    Unknown(String),
    Unsupported(String),
}
#[derive(Clone, Debug)]
pub struct NumericContract {
    pub destination: DeclarationId,
    pub destination_location: SourceLocation,
    pub file: FileId,
    /// The consuming initializer/equality item owns suppression, even across files.
    pub item: usize,
    /// Complete consumed RHS; no destination domain is assigned to its children.
    pub location: SourceLocation,
    pub declared: Domain,
    pub required: NumericOutcome,
    pub derived: NumericOutcome,
    pub relation: NumericContractRelation,
}
/// Supply prerequisites from this same ModelContext. Reuses definition safety,
/// enforcement and checked numeric evidence; no call-instance or float evaluator.
/// Replaceable model defaults and unrestricted declarations are not candidates.
pub fn resolve_numeric_contracts(
    context: &ModelContext,
    bindings: &BindingFacts,
    definitions: &DefinitionFacts,
    numeric: &NumericFacts,
) -> Vec<NumericContract> {
    let mut contracts = Vec::new();
    for rhs in &numeric.definitions {
        if rhs.parameter_default || !context.files[rhs.file].warnings_enabled() {
            continue;
        }
        let Some(required) = numeric
            .declarations
            .iter()
            .find(|d| d.declaration == rhs.target && explicit_contract(&d.declared))
        else {
            continue;
        };
        let Some(definition) = definitions.definitions.iter().find(|d| {
            d.target == rhs.target
                && d.file == rhs.file
                && d.value == rhs.location
                && d.coverage == DefinitionCoverage::Scalar
        }) else {
            continue;
        };
        let destination = &bindings.declarations[rhs.target.0];
        contracts.push(NumericContract {
            destination: rhs.target,
            destination_location: destination.location.clone(),
            file: rhs.file,
            item: definition.item,
            location: rhs.location.clone(),
            declared: required.declared.clone(),
            required: required.required.clone(),
            derived: rhs.outcome.clone(),
            relation: compare(&rhs.outcome, required),
        });
    }
    contracts
}
fn explicit_contract(domain: &Domain) -> bool {
    match domain {
        Domain::Named { domain, .. } => explicit_contract(domain),
        Domain::Range { .. } | Domain::LiteralSet(_) | Domain::Unsupported(_) | Domain::Unknown => {
            true
        }
        _ => false,
    }
}
fn compare(derived: &NumericOutcome, required: &NumericDeclaration) -> NumericContractRelation {
    use NumericContractRelation::*;
    if let Some(reason) = [derived, &required.required].iter().find_map(|value| {
        if let NumericOutcome::Unsupported(reason) = value {
            Some(reason)
        } else {
            None
        }
    }) {
        return Unsupported(reason.clone());
    }
    for value in [derived, &required.required] {
        match value {
            NumericOutcome::Unknown(reason) => return Unknown(reason.clone()),
            NumericOutcome::Symbolic { .. } => {
                return Unknown(
                    "numeric bounds depend on parameters; no closed comparison is available".into(),
                );
            }
            _ => {}
        }
    }
    if matches!(
        derived.domain_relation(required),
        NumericDomainRelation::ExactOutside | NumericDomainRelation::Disjoint
    ) {
        return Contradiction;
    }
    let Some((lower, upper)) = derived.interval() else {
        return Unknown("derived integer range is unavailable".into());
    };
    let mut domain = &required.declared;
    while let Domain::Named { domain: inner, .. } = domain {
        domain = inner;
    }
    let contained = match domain {
        Domain::Range { lower: l, upper: u } => {
            match (invariant_integer(l), invariant_integer(u)) {
                (Ok(Some(l)), Ok(Some(u))) => lower >= l && upper <= u,
                (Err(reason), _) | (_, Err(reason)) => return Unsupported(reason),
                _ => return Unknown("required endpoints depend on parameters".into()),
            }
        }
        Domain::LiteralSet(members) => {
            let values = members
                .iter()
                .map(invariant_integer)
                .collect::<Result<Option<BTreeSet<_>>, _>>();
            let values = match values {
                Ok(Some(values)) => values,
                Ok(None) => return Unknown("required members depend on parameters".into()),
                Err(reason) => return Unsupported(reason),
            };
            // Count actual members, without enumerating a potentially large hull.
            values.range(lower..=upper).count() as i128 == i128::from(upper) - i128::from(lower) + 1
        }
        _ => return Unknown("actual integer-domain membership is unavailable".into()),
    };
    if contained {
        Compatible
    } else {
        PossibleConflict
    }
}
pub(super) fn check_numeric_contracts(
    context: &ModelContext,
    bindings: &BindingFacts,
    contracts: &[NumericContract],
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    for contract in contracts {
        let source = &context.files[contract.file];
        if !source.warnings_enabled()
            || source
                .suppressions
                .as_ref()
                .is_none_or(|s| s[contract.item].contains(&Rule::SuspiciousDomain))
        {
            continue;
        }
        let rhs = source.parsed.source()[contract.location.range.start - source.byte_offset
            ..contract.location.range.end - source.byte_offset]
            .trim();
        let destination = &bindings.declarations[contract.destination.0];
        let proof = match &contract.relation {
            NumericContractRelation::Compatible => continue,
            NumericContractRelation::Unknown(reason)
            | NumericContractRelation::Unsupported(reason) => {
                let diagnostic = SourceDiagnostic {
                    location: contract.location.clone(),
                    message: format!(
                        "suspicious-domain: cannot compare RHS '{rhs}' with '{}' domain: {reason}",
                        destination.name
                    ),
                };
                if !limitations.iter().any(|d: &SourceDiagnostic| {
                    d.location == diagnostic.location && d.message == diagnostic.message
                }) {
                    limitations.push(diagnostic);
                }
                continue;
            }
            NumericContractRelation::Contradiction => {
                "proved domain contradiction: the independently derived value/range is disjoint from the required domain"
            }
            NumericContractRelation::PossibleConflict => {
                "conservative possible conflict: the derived interval hull overlaps but is not contained in the required domain; the hull may contain impossible values or lose correlations, so no actual failing value or model-feasibility witness is proved"
            }
        };
        findings.push(FileFinding {
            rule: Rule::SuspiciousDomain,
            severity: Severity::Warning,
            location: contract.location.clone(),
            message: format!(
                "{proof}; complete RHS '{rhs}' derives {}; destination '{}' requires {} (declared at {}:{}:{})",
                describe_value(&contract.derived), destination.name,
                describe_domain(&contract.declared, bindings),
                destination.location.path.display(), destination.location.line, destination.location.column
            ),
        });
    }
    (findings, limitations)
}
fn describe_value(value: &NumericOutcome) -> String {
    match value {
        NumericOutcome::Exact(n) => n.to_string(),
        NumericOutcome::Interval { lower, upper } => format!("{lower}..{upper}"),
        _ => format!("{value:?}"),
    }
}
fn describe_domain(domain: &Domain, bindings: &BindingFacts) -> String {
    let bound = |n| match invariant_integer(n) {
        Ok(Some(n)) => n.to_string(),
        _ => format!("{n:?}"),
    };
    match domain {
        Domain::Named {
            declaration,
            domain,
        } => format!(
            "{} ({})",
            bindings.declarations[declaration.0].name,
            describe_domain(domain, bindings)
        ),
        Domain::Range { lower, upper } => format!("{}..{}", bound(lower), bound(upper)),
        Domain::LiteralSet(values) => format!(
            "{{{}}}",
            values.iter().map(bound).collect::<Vec<_>>().join(", ")
        ),
        _ => format!("{domain:?}"),
    }
}
