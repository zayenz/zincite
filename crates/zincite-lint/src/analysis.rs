//! File-aware selected results and the shared text diagnostic renderer.
use std::io::{self, Write};
use std::path::Path;
use zincite_syntax::{FileMode, ParsedFile};

use crate::{
    LintDiagnostic, LintOptions, ModelContext, ModelRootState, Rule, Severity, SourceDiagnostic,
    SourceLocation, array_indices::check_array_indices, captures::check_captures,
    compact_if::check_compact_ifs, constant_variable::check_constant_variables,
    decision_use::check_decision_use, effective_zero_one::check_effective_zero_one,
    element::check_element, global_uses::check_global_uses, lint_items, lint_with_options,
    resolve_bindings, resolve_callables, resolve_compact_ifs, resolve_definitions, resolve_domains,
    resolve_effective_zero_one, resolve_global_uses, resolve_instantiations,
    resolve_integer_bounds, resolve_search_coverage, resolve_symmetry_uses,
    resolve_unused_declarations, search_coverage::check_search_coverage,
    symmetry::check_symmetry_uses, unbounded_variable::check_unbounded_variables,
    unused_declarations::check_unused_declarations,
};

#[derive(Debug)]
pub struct FileFinding {
    pub location: SourceLocation,
    pub rule: Rule,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RuleOutcome {
    Completed,
    Inapplicable { reason: String },
    Limited { reason: String },
    NotRun { reason: String },
}

#[derive(Debug)]
pub struct RuleExecution {
    pub rule: Rule,
    pub outcome: RuleOutcome,
}

#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub findings: Vec<FileFinding>,
    pub rules: Vec<RuleExecution>,
    pub limitations: Vec<SourceDiagnostic>,
    pub errors: Vec<SourceDiagnostic>,
}

impl AnalysisResult {
    pub fn status(&self) -> u8 {
        if !self.errors.is_empty() {
            2
        } else {
            u8::from(!self.findings.is_empty())
        }
    }
}

fn finding(diagnostic: LintDiagnostic, location: SourceLocation) -> FileFinding {
    FileFinding {
        location,
        rule: diagnostic.rule,
        severity: diagnostic.severity,
        message: diagnostic.message,
    }
}

fn unavailable(options: &LintOptions, location: SourceLocation) -> Option<AnalysisResult> {
    let message = options.check_available().err()?;
    Some(AnalysisResult {
        errors: vec![SourceDiagnostic {
            location,
            message: message.clone(),
        }],
        rules: options
            .rules
            .iter()
            .map(|&rule| RuleExecution {
                rule,
                outcome: RuleOutcome::NotRun {
                    reason: message.clone(),
                },
            })
            .collect(),
        ..AnalysisResult::default()
    })
}

/// Selected syntax analysis without filesystem loading. byte_offset accounts
/// for a BOM removed by the caller before parsing.
pub fn analyze_file(
    parsed: &ParsedFile,
    path: impl AsRef<Path>,
    byte_offset: usize,
    options: &LintOptions,
) -> AnalysisResult {
    let path = path.as_ref();
    let location =
        |range| SourceLocation::new(path.to_path_buf(), parsed.source(), range, byte_offset);
    if let Some(result) = unavailable(options, location(0..0)) {
        return result;
    }
    let mut result = AnalysisResult::default();
    let syntax_options = LintOptions {
        rules: options
            .rules
            .iter()
            .copied()
            .filter(|rule| !rule.requires_model())
            .collect(),
        parameters: options.parameters.clone(),
    };
    match lint_with_options(parsed, &syntax_options) {
        Ok(warnings) => {
            result.findings = warnings
                .into_iter()
                .map(|warning| {
                    let position = location(warning.range.clone());
                    finding(warning, position)
                })
                .collect()
        }
        Err(errors) => {
            result.errors = errors
                .into_iter()
                .map(|error| SourceDiagnostic {
                    location: location(error.range),
                    message: error.message,
                })
                .collect()
        }
    }
    result.rules = options
        .rules
        .iter()
        .map(|&rule| RuleExecution {
            rule,
            outcome: if !result.errors.is_empty() {
                RuleOutcome::NotRun {
                    reason: "syntax or suppression errors".into(),
                }
            } else if rule.requires_model() {
                if FileMode::from_path(path) == FileMode::Data {
                    RuleOutcome::Inapplicable {
                        reason: "standalone data has no model context".into(),
                    }
                } else {
                    RuleOutcome::Limited {
                        reason: "this rule requires a model context".into(),
                    }
                }
            } else {
                RuleOutcome::Completed
            },
        })
        .collect();
    if result.errors.is_empty() && FileMode::from_path(path) == FileMode::Model {
        for rule in options.rules.iter().filter(|rule| rule.requires_model()) {
            result.limitations.push(SourceDiagnostic {
                location: location(0..0),
                message: format!("{} requires a ModelContext; use analyze_model", rule.id()),
            });
        }
    }
    result
}

/// Analyse retained sources, keeping syntax rule completion separate from an
/// incomplete model closure. Binding facts are produced once when required.
/// Standard-library files receive style findings only
/// when they were explicitly selected as inputs.
pub fn analyze_model(context: &ModelContext, options: &LintOptions) -> AnalysisResult {
    if let Some(result) = unavailable(
        options,
        SourceLocation::new(context.root.clone(), "", 0..0, 0),
    ) {
        return result;
    }
    let mut result = AnalysisResult {
        errors: context.errors.clone(),
        limitations: context.limitations.clone(),
        ..AnalysisResult::default()
    };
    for file in context.files.iter().filter(|file| file.warnings_enabled()) {
        let Some(suppressions) = &file.suppressions else {
            continue;
        };
        for warning in lint_items(&file.parsed, suppressions, options) {
            let location = file.location(warning.range.clone());
            result.findings.push(finding(warning, location));
        }
    }
    let data = FileMode::from_path(&context.root) == FileMode::Data;
    let shared_incomplete = !context.limitations.is_empty() || !context.errors.is_empty();
    let mut array_incomplete = false;
    let mut index_mismatch_incomplete = false;
    let mut optionality_incomplete = false;
    let mut search_state = ModelRootState::Complete;
    let mut search_incomplete = false;
    let mut compact_incomplete = false;
    let mut effective_incomplete = false;
    let mut constant_incomplete = false;
    let mut unbounded_incomplete = false;
    let mut capture_incomplete = false;
    let mut element_incomplete = false;
    let mut global_incomplete = false;
    let mut symmetry_incomplete = false;
    let mut decision_incomplete = Vec::new();
    let mut unused_state = ModelRootState::Complete;
    let mut unused_incomplete = false;
    if !data && options.requires_model() {
        let facts = resolve_bindings(context);
        let domains = if options.rules.contains(&Rule::ArrayIndexStart)
            || options.rules.contains(&Rule::ConstantVariable)
            || options.rules.contains(&Rule::UnboundedVariable)
            || options.rules.contains(&Rule::SearchCoverage)
            || options.rules.contains(&Rule::IndexSetMismatch)
            || options.rules.contains(&Rule::HiddenOptionality)
            || options.rules.contains(&Rule::EffectiveZeroOne)
        {
            Some(resolve_domains(context, &facts))
        } else {
            None
        };
        if options.rules.contains(&Rule::ArrayIndexStart) {
            let domains = domains.as_ref().unwrap();
            let indices = check_array_indices(context, domains);
            array_incomplete = !indices.limitations.is_empty();
            result.findings.extend(indices.findings);
            result.limitations.extend(indices.limitations);
        }
        if options.rules.contains(&Rule::GlobalVariableInFunction) {
            let captures = check_captures(context, &facts);
            capture_incomplete = !captures.limitations.is_empty();
            result.findings.extend(captures.findings);
            result.limitations.extend(captures.limitations);
        }
        let decision_rules: Vec<_> = options
            .rules
            .iter()
            .copied()
            .filter(|rule| {
                matches!(
                    rule,
                    Rule::DecisionVariableOperator
                        | Rule::DecisionVariableGenerator
                        | Rule::DecisionVariableCondition
                )
            })
            .collect();
        if options.rules.contains(&Rule::UnmarkedSymmetryBreaking)
            || options.rules.contains(&Rule::ReifiedGlobal)
            || options.rules.contains(&Rule::ElementPredicate)
            || options.rules.contains(&Rule::CompactIf)
            || !decision_rules.is_empty()
            || options.rules.contains(&Rule::ConstantVariable)
            || options.rules.contains(&Rule::UnboundedVariable)
            || options.rules.contains(&Rule::SearchCoverage)
            || options.rules.contains(&Rule::IndexSetMismatch)
            || options.rules.contains(&Rule::HiddenOptionality)
            || options.rules.contains(&Rule::EffectiveZeroOne)
            || options.rules.contains(&Rule::UnusedDeclaration)
        {
            let calls = resolve_callables(context, &facts);
            if options.rules.contains(&Rule::UnmarkedSymmetryBreaking) {
                let symmetry = resolve_symmetry_uses(context, &facts, &calls);
                let (findings, limitations) = check_symmetry_uses(context, &symmetry);
                symmetry_incomplete = !limitations.is_empty();
                result.findings.extend(findings);
                result.limitations.extend(limitations);
            }
            if options.rules.contains(&Rule::UnusedDeclaration) {
                let usage = resolve_unused_declarations(context, &facts, &calls);
                unused_state = usage.root_state;
                unused_incomplete =
                    !usage.limitations.is_empty() || unused_state == ModelRootState::Incomplete;
                result
                    .findings
                    .extend(check_unused_declarations(context, &facts, &usage));
                result.limitations.extend(usage.limitations);
            }
            if options.rules.contains(&Rule::ElementPredicate) {
                let elements = check_element(context, &facts, &calls);
                element_incomplete = !elements.limitations.is_empty();
                result.findings.extend(elements.findings);
                result.limitations.extend(elements.limitations);
            }
            if options.rules.contains(&Rule::ReifiedGlobal)
                || !decision_rules.is_empty()
                || options.rules.contains(&Rule::CompactIf)
                || options.rules.contains(&Rule::ConstantVariable)
                || options.rules.contains(&Rule::UnboundedVariable)
                || options.rules.contains(&Rule::SearchCoverage)
                || options.rules.contains(&Rule::IndexSetMismatch)
                || options.rules.contains(&Rule::HiddenOptionality)
                || options.rules.contains(&Rule::EffectiveZeroOne)
            {
                let instantiations = resolve_instantiations(context, &facts, &calls);
                if options.rules.contains(&Rule::ReifiedGlobal) {
                    let uses = resolve_global_uses(context, &facts, &calls, &instantiations);
                    let (findings, limitations) = check_global_uses(context, &uses);
                    global_incomplete = !limitations.is_empty();
                    result.findings.extend(findings);
                    result.limitations.extend(limitations);
                }
                if options.rules.contains(&Rule::EffectiveZeroOne) {
                    let domains = domains.as_ref().unwrap();
                    let bounds = resolve_integer_bounds(context, &facts, &calls, domains);
                    let zero_one = resolve_effective_zero_one(
                        context,
                        &facts,
                        &calls,
                        &instantiations,
                        domains,
                        &bounds,
                    );
                    let checked = check_effective_zero_one(context, &zero_one);
                    effective_incomplete = !checked.limitations.is_empty();
                    result.findings.extend(checked.findings);
                    result.limitations.extend(checked.limitations);
                }
                if options.rules.contains(&Rule::CompactIf) {
                    let conditionals =
                        resolve_compact_ifs(context, &facts, &calls, &instantiations);
                    let checked = check_compact_ifs(context, &conditionals);
                    compact_incomplete = !checked.limitations.is_empty();
                    result.findings.extend(checked.findings);
                    result.limitations.extend(checked.limitations);
                }
                if options.rules.contains(&Rule::ConstantVariable)
                    || options.rules.contains(&Rule::UnboundedVariable)
                    || options.rules.contains(&Rule::SearchCoverage)
                    || options.rules.contains(&Rule::IndexSetMismatch)
                    || options.rules.contains(&Rule::HiddenOptionality)
                {
                    let definitions = resolve_definitions(
                        context,
                        &facts,
                        &calls,
                        &instantiations,
                        domains.as_ref().unwrap(),
                    );
                    if options.rules.contains(&Rule::IndexSetMismatch)
                        || options.rules.contains(&Rule::HiddenOptionality)
                    {
                        let domains = domains.as_ref().unwrap();
                        let numeric = crate::resolve_numeric_facts(
                            context,
                            &facts,
                            &calls,
                            &instantiations,
                            domains,
                            &definitions,
                        );
                        let optional = crate::resolve_optional_facts(
                            context,
                            &facts,
                            &calls,
                            &instantiations,
                            domains,
                            &numeric,
                            &definitions,
                        );
                        let guarded = crate::resolve_guarded_facts_with_options(
                            context,
                            &facts,
                            &calls,
                            &instantiations,
                            domains,
                            &numeric,
                            &optional,
                        );
                        if options.rules.contains(&Rule::HiddenOptionality) {
                            let (findings, limitations) =
                                crate::hidden_optionality::check_hidden_optionality(
                                    context,
                                    &facts,
                                    &calls,
                                    &instantiations,
                                    &optional,
                                    &guarded,
                                );
                            optionality_incomplete = !limitations.is_empty();
                            result.findings.extend(findings);
                            result.limitations.extend(limitations);
                        }
                        if options.rules.contains(&Rule::IndexSetMismatch) {
                            let iteration = crate::resolve_iteration_facts(
                                context,
                                &facts,
                                &calls,
                                &instantiations,
                                domains,
                                &numeric,
                                &optional,
                                &guarded,
                            );
                            let (findings, limitations) =
                                crate::index_set_mismatch::check_index_set_mismatches(
                                    context, &facts, &guarded, &iteration,
                                );
                            index_mismatch_incomplete = !limitations.is_empty();
                            result.findings.extend(findings);
                            result.limitations.extend(limitations);
                        }
                    }
                    if options.rules.contains(&Rule::SearchCoverage) {
                        let search = resolve_search_coverage(
                            context,
                            &facts,
                            &calls,
                            &instantiations,
                            domains.as_ref().unwrap(),
                            &definitions,
                        );
                        search_state = search.root_state;
                        search_incomplete = !search.limitations.is_empty()
                            || search_state == ModelRootState::Incomplete;
                        result
                            .findings
                            .extend(check_search_coverage(context, &facts, &search));
                        result.limitations.extend(search.limitations);
                    }
                    if options.rules.contains(&Rule::UnboundedVariable) {
                        let checked = check_unbounded_variables(
                            context,
                            &facts,
                            domains.as_ref().unwrap(),
                            &definitions,
                        );
                        unbounded_incomplete = !checked.limitations.is_empty();
                        result.findings.extend(checked.findings);
                        result.limitations.extend(checked.limitations);
                    }
                    if options.rules.contains(&Rule::ConstantVariable) {
                        let checked =
                            check_constant_variables(context, &facts, &calls, &definitions);
                        constant_incomplete = !checked.limitations.is_empty();
                        result.findings.extend(checked.findings);
                        result.limitations.extend(checked.limitations);
                    }
                }
                for rule in decision_rules {
                    let checked =
                        check_decision_use(context, &facts, &calls, &instantiations, rule);
                    if !checked.limitations.is_empty() {
                        decision_incomplete.push(rule);
                    }
                    result.findings.extend(checked.findings);
                    result.limitations.extend(checked.limitations);
                }
            }
        }
    }
    result.rules = options
        .rules
        .iter()
        .map(|&rule| RuleExecution {
            rule,
            outcome: if data && rule.requires_model() {
                RuleOutcome::Inapplicable {
                    reason: "standalone data has no model context".into(),
                }
            } else if rule == Rule::SearchCoverage && search_state == ModelRootState::Fragment {
                RuleOutcome::Inapplicable {
                    reason: "a fragment without a solve item has no solve search to check".into(),
                }
            } else if rule == Rule::UnusedDeclaration && unused_state == ModelRootState::Fragment {
                RuleOutcome::Inapplicable {
                    reason: "a model fragment without a solve item cannot establish unused exports"
                        .into(),
                }
            } else if rule.requires_model()
                && (shared_incomplete
                    || (rule == Rule::GlobalVariableInFunction && capture_incomplete)
                    || (rule == Rule::ElementPredicate && element_incomplete)
                    || (rule == Rule::ReifiedGlobal && global_incomplete)
                    || (rule == Rule::UnmarkedSymmetryBreaking && symmetry_incomplete)
                    || (rule == Rule::ArrayIndexStart && array_incomplete)
                    || (rule == Rule::IndexSetMismatch && index_mismatch_incomplete)
                    || (rule == Rule::HiddenOptionality && optionality_incomplete)
                    || (rule == Rule::CompactIf && compact_incomplete)
                    || (rule == Rule::EffectiveZeroOne && effective_incomplete)
                    || (rule == Rule::UnusedDeclaration && unused_incomplete)
                    || (rule == Rule::SearchCoverage && search_incomplete)
                    || (rule == Rule::ConstantVariable && constant_incomplete)
                    || (rule == Rule::UnboundedVariable && unbounded_incomplete)
                    || decision_incomplete.contains(&rule))
            {
                RuleOutcome::Limited {
                    reason: "model dependencies or required semantic facts are incomplete".into(),
                }
            } else if result.errors.is_empty() {
                RuleOutcome::Completed
            } else {
                RuleOutcome::Limited {
                    reason: "model loading errors leave files unchecked".into(),
                }
            },
        })
        .collect();
    result
}

/// Render only to the supplied diagnostic writer (stderr in the CLI). Model
/// limitations alone retain status 0/1; actual errors take precedence with 2.
pub fn write_analysis(result: &AnalysisResult, output: &mut impl Write) -> io::Result<u8> {
    for finding in &result.findings {
        write_diagnostic(
            output,
            &finding.location,
            &format!("warning [{}]: {}", finding.rule.id(), finding.message),
        )?;
    }
    for limitation in &result.limitations {
        write_diagnostic(
            output,
            &limitation.location,
            &format!("analysis limitation: {}", limitation.message),
        )?;
    }
    for error in &result.errors {
        write_diagnostic(
            output,
            &error.location,
            &format!("error: {}", error.message),
        )?;
    }
    Ok(result.status())
}

fn write_diagnostic(
    output: &mut impl Write,
    location: &SourceLocation,
    message: &str,
) -> io::Result<()> {
    writeln!(
        output,
        "{}:{}:{}: bytes {}..{}: {message}",
        location.path.display(),
        location.line,
        location.column,
        location.range.start,
        location.range.end
    )
}
