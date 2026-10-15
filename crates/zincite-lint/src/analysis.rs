//! File-aware selected results and the shared text diagnostic renderer.
use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
use std::io::{self, Write};
use std::path::Path;
use zincite_syntax::{FileMode, ParsedFile};

use crate::{
    LintDiagnostic, LintOptions, ModelContext, ModelRootState, Rule, Severity, SourceDiagnostic,
    SourceLocation, array_indices::check_array_indices, captures::check_captures,
    compact_if::check_compact_ifs, constant_variable::check_constant_variables,
    decision_use::check_decision_use, effective_zero_one::check_effective_zero_one,
    element::check_element, global_uses::check_global_uses, lint_items, lint_with_options,
    search_coverage::check_search_coverage, symmetry::check_symmetry_uses,
    unbounded_variable::check_unbounded_variables, unused_declarations::check_unused_declarations,
};

#[derive(Debug)]
pub struct FileFinding {
    /// Optional producer-supplied atomic fix; capability is separate rule metadata.
    pub fix: Option<crate::Fix>,
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
            u8::from(
                self.findings
                    .iter()
                    .any(|finding| finding.severity == Severity::Warning),
            )
        }
    }
}

impl FileFinding {
    /// Attach a supplied original-file location to a syntax diagnostic.
    /// Fix snapshots and edit ranges already include any BOM; they are forwarded
    /// unchanged rather than shifted with the diagnostic location.
    pub fn from_diagnostic(diagnostic: LintDiagnostic, location: SourceLocation) -> Self {
        Self {
            fix: diagnostic.fix,
            location,
            rule: diagnostic.rule,
            severity: diagnostic.severity,
            message: diagnostic.message,
        }
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
        |range| SourceLocation::from_parsed(path.to_path_buf(), parsed, range, byte_offset);
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
                    FileFinding::from_diagnostic(warning, position)
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
    analyze_model_with_facts(&crate::ModelFacts::new(context), options)
}

/// Run fresh diagnostic policy while reusing this model's requested semantic facts.
pub fn analyze_model_with_facts(
    facts: &crate::ModelFacts<'_>,
    options: &LintOptions,
) -> AnalysisResult {
    let context = facts.model();
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
            result
                .findings
                .push(FileFinding::from_diagnostic(warning, location));
        }
    }
    let data = FileMode::from_path(&context.root) == FileMode::Data;
    let shared_incomplete = !context.limitations.is_empty() || !context.errors.is_empty();
    let mut array_incomplete = false;
    let mut index_mismatch_incomplete = false;
    let mut optionality_incomplete = false;
    let mut partial_incomplete = false;
    let mut vacuous_incomplete = false;
    let mut search_state = ModelRootState::Complete;
    let mut search_incomplete = false;
    let mut compact_incomplete = false;
    let mut effective_incomplete = false;
    let mut constant_incomplete = false;
    let mut unbounded_incomplete = false;
    let mut capture_incomplete = false;
    let mut shadowing_incomplete = false;
    let mut element_incomplete = false;
    let mut global_incomplete = false;
    let mut symmetry_incomplete = false;
    let mut decision_incomplete = Vec::new();
    let mut unused_state = ModelRootState::Complete;
    let mut unused_incomplete = false;
    let mut unused_generator_incomplete = false;
    let mut patterns_incomplete = false;
    let mut comprehension_incomplete = false;
    let mut input_incomplete = false;
    let mut domain_contract_incomplete = false;
    let mut enum_state = ModelRootState::Complete;
    let mut enum_incomplete = false;
    let mut fix_snapshots = std::collections::BTreeMap::new();
    if !data && options.requires_model() {
        let bindings = facts.bindings();
        if options.rules.contains(&Rule::SuspiciousShadowing) {
            let (findings, limitations) = crate::shadowing::check_shadowing(
                context,
                bindings,
                &options.parameters.shadowing_ignore_names,
            );
            shadowing_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        if options.rules.contains(&Rule::ArrayIndexStart) {
            let checked = check_array_indices(
                context,
                bindings,
                &facts.source_view(),
                facts.domains(),
                facts.definitions(),
                facts.instantiations(),
            );
            array_incomplete = !checked.limitations.is_empty();
            result.findings.extend(checked.findings);
            result.limitations.extend(checked.limitations);
        }
        if options.rules.contains(&Rule::GlobalVariableInFunction) {
            let checked = check_captures(context, bindings);
            capture_incomplete = !checked.limitations.is_empty();
            result.findings.extend(checked.findings);
            result.limitations.extend(checked.limitations);
        }
        if options.rules.contains(&Rule::PreferEnum) {
            let checked = crate::prefer_enum::check(
                context,
                bindings,
                &facts.source_view(),
                facts.source_domains(),
            );
            enum_state = checked.state;
            enum_incomplete =
                !checked.limitations.is_empty() || enum_state == ModelRootState::Incomplete;
            result.findings.extend(checked.findings);
            result.limitations.extend(checked.limitations);
        }
        if options.rules.contains(&Rule::PreferArrayComprehension) {
            result
                .findings
                .extend(crate::prefer_array_comprehension::check(
                    context,
                    bindings,
                    &facts.source_view(),
                    &mut fix_snapshots,
                ));
        }
        if options.rules.contains(&Rule::UnmarkedSymmetryBreaking) {
            let (findings, limitations) = check_symmetry_uses(context, facts.symmetry_uses());
            symmetry_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        if options.rules.contains(&Rule::UnusedDeclaration) {
            let usage = facts.usage();
            unused_state = usage.root_state;
            unused_incomplete =
                !usage.limitations.is_empty() || unused_state == ModelRootState::Incomplete;
            result
                .findings
                .extend(check_unused_declarations(context, bindings, usage));
            result.limitations.extend(usage.limitations.iter().cloned());
        }
        if options.rules.contains(&Rule::ReifiedGlobal) {
            let (findings, limitations) = check_global_uses(context, facts.global_uses());
            global_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        for &rule in &options.rules {
            if matches!(
                rule,
                Rule::DecisionVariableOperator
                    | Rule::DecisionVariableGenerator
                    | Rule::DecisionVariableCondition
            ) {
                let checked = check_decision_use(
                    context,
                    bindings,
                    &facts.source_view(),
                    facts.instantiations(),
                    rule,
                );
                if !checked.limitations.is_empty() {
                    decision_incomplete.push(rule);
                }
                result.findings.extend(checked.findings);
                result.limitations.extend(checked.limitations);
            }
        }
        if options.rules.contains(&Rule::EffectiveZeroOne) {
            let checked = check_effective_zero_one(context, facts.effective_zero_one());
            effective_incomplete = !checked.limitations.is_empty();
            result.findings.extend(checked.findings);
            result.limitations.extend(checked.limitations);
        }
        if options.rules.contains(&Rule::CompactIf) {
            let checked = check_compact_ifs(context, facts.compact_ifs());
            compact_incomplete = !checked.limitations.is_empty();
            result.findings.extend(checked.findings);
            result.limitations.extend(checked.limitations);
        }
        if options.rules.contains(&Rule::SuspiciousDomain) {
            let (findings, limitations) = crate::suspicious_domain::check_numeric_contracts(
                context,
                bindings,
                facts.numeric_contracts(),
            );
            domain_contract_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        if options.rules.contains(&Rule::ElementPredicate) {
            let checked = check_element(
                context,
                bindings,
                &facts.source_view(),
                facts.element_rewrites(),
                &mut fix_snapshots,
            );
            element_incomplete = !checked.limitations.is_empty();
            result.findings.extend(checked.findings);
            result.limitations.extend(checked.limitations);
        }
        if options.rules.contains(&Rule::HiddenOptionality) {
            let (findings, limitations) = crate::hidden_optionality::check_hidden_optionality(
                context,
                bindings,
                &facts.source_view(),
                facts.instantiations(),
                facts.optional(),
                facts.guarded(),
            );
            optionality_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        if options.rules.contains(&Rule::ExpensiveComprehension) {
            let (findings, limitations) = crate::expensive_comprehension::check_comprehensions(
                context,
                bindings,
                facts.comprehension_structure(),
                options.parameters.comprehension_max_candidates,
            );
            comprehension_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        if options.rules.contains(&Rule::GlobalConstraintOpportunity) {
            let (findings, limitations) = crate::global_patterns::check_global_patterns(
                context,
                bindings,
                facts.global_patterns(),
            );
            patterns_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        if options.rules.contains(&Rule::UnusedGeneratorBinding) {
            let (findings, limitations) = crate::unused_generator::check_unused_generators(
                context,
                bindings,
                facts.generator_binding_usage(),
                &mut fix_snapshots,
            );
            unused_generator_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        if options.rules.contains(&Rule::IndexSetMismatch) {
            let (findings, limitations) = crate::index_set_mismatch::check_index_set_mismatches(
                context,
                bindings,
                facts.guarded(),
                facts.iteration(),
            );
            index_mismatch_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        let mut input_keys = Vec::new();
        if options.rules.contains(&Rule::MissingInputPrecondition) {
            let (findings, limitations, keys) =
                crate::input_preconditions::check_input_preconditions(
                    context,
                    bindings,
                    facts.callable_inputs(),
                );
            input_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
            input_keys = keys;
        }
        if options.rules.contains(&Rule::PartialExpression) {
            let (findings, limitations) = crate::partial_expression::check_partial_expressions(
                context,
                bindings,
                &facts.source_view(),
                facts.guarded(),
                facts.iteration(),
                &result.findings,
                &input_keys,
            );
            partial_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        if options.rules.contains(&Rule::VacuousConstraint) {
            let (findings, limitations) = crate::vacuous_constraint::check_vacuous_constraints(
                context,
                bindings,
                &facts.source_view(),
                facts.guarded(),
                facts.optional(),
                facts.iteration(),
            );
            vacuous_incomplete = !limitations.is_empty();
            result.findings.extend(findings);
            result.limitations.extend(limitations);
        }
        if options.rules.contains(&Rule::SearchCoverage) {
            let search = facts.search();
            search_state = search.root_state;
            search_incomplete =
                !search.limitations.is_empty() || search_state == ModelRootState::Incomplete;
            result
                .findings
                .extend(check_search_coverage(context, bindings, search));
            result
                .limitations
                .extend(search.limitations.iter().cloned());
        }
        if options.rules.contains(&Rule::UnboundedVariable) {
            let checked = check_unbounded_variables(
                context,
                bindings,
                &facts.source_view(),
                facts.instantiations(),
                facts.domains(),
                facts.complete_definitions(),
                &facts.callable_definitions().unavailable,
            );
            unbounded_incomplete = !checked.limitations.is_empty();
            result.findings.extend(checked.findings);
            result.limitations.extend(checked.limitations);
        }
        if options.rules.contains(&Rule::ConstantVariable) {
            let checked = check_constant_variables(
                context,
                bindings,
                &facts.source_view(),
                facts.definitions(),
            );
            constant_incomplete = !checked.limitations.is_empty();
            result.findings.extend(checked.findings);
            result.limitations.extend(checked.limitations);
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
            } else if rule == Rule::PreferEnum && enum_state == ModelRootState::Fragment {
                RuleOutcome::Inapplicable {
                    reason: "a model fragment cannot establish all identity-set uses".into(),
                }
            } else if rule.requires_model()
                && (shared_incomplete
                    || (rule == Rule::GlobalVariableInFunction && capture_incomplete)
                    || (rule == Rule::SuspiciousShadowing && shadowing_incomplete)
                    || (rule == Rule::ElementPredicate && element_incomplete)
                    || (rule == Rule::ReifiedGlobal && global_incomplete)
                    || (rule == Rule::UnmarkedSymmetryBreaking && symmetry_incomplete)
                    || (rule == Rule::ArrayIndexStart && array_incomplete)
                    || (rule == Rule::IndexSetMismatch && index_mismatch_incomplete)
                    || (rule == Rule::HiddenOptionality && optionality_incomplete)
                    || (rule == Rule::PartialExpression && partial_incomplete)
                    || (rule == Rule::VacuousConstraint && vacuous_incomplete)
                    || (rule == Rule::UnusedGeneratorBinding && unused_generator_incomplete)
                    || (rule == Rule::GlobalConstraintOpportunity && patterns_incomplete)
                    || (rule == Rule::ExpensiveComprehension && comprehension_incomplete)
                    || (rule == Rule::MissingInputPrecondition && input_incomplete)
                    || (rule == Rule::SuspiciousDomain && domain_contract_incomplete)
                    || (rule == Rule::CompactIf && compact_incomplete)
                    || (rule == Rule::EffectiveZeroOne && effective_incomplete)
                    || (rule == Rule::UnusedDeclaration && unused_incomplete)
                    || (rule == Rule::SearchCoverage && search_incomplete)
                    || (rule == Rule::ConstantVariable && constant_incomplete)
                    || (rule == Rule::UnboundedVariable && unbounded_incomplete)
                    || (rule == Rule::PreferEnum && enum_incomplete)
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

/// Render compact diagnostics without source excerpts to the supplied writer. Model
/// limitations alone retain status 0/1; actual errors take precedence with 2.
pub fn write_analysis(result: &AnalysisResult, output: &mut impl Write) -> io::Result<u8> {
    write_report(result, None, true, |_| None, output)
}

/// CLI presentation keeps each root's conclusions separate, while retaining
/// located limitations in the analysis result and the public detailed renderer.
pub(crate) fn write_root_analysis<'a>(
    result: &AnalysisResult,
    root: &Path,
    show_limitations: bool,
    source: impl Fn(&Path) -> Option<(&'a [u8], usize)>,
    output: &mut impl Write,
) -> io::Result<u8> {
    write_report(result, Some(root), show_limitations, source, output)
}

fn write_report<'a>(
    result: &AnalysisResult,
    root: Option<&Path>,
    show_limitations: bool,
    source: impl Fn(&Path) -> Option<(&'a [u8], usize)>,
    output: &mut impl Write,
) -> io::Result<u8> {
    for finding in &result.findings {
        let severity = match finding.severity {
            Severity::Informational => "info",
            Severity::Warning => "warning",
        };
        write_diagnostic(
            output,
            root,
            &finding.location,
            &format!("{severity} [{}]: {}", finding.rule.id(), finding.message),
            source(&finding.location.path),
        )?;
    }
    if show_limitations {
        for limitation in &result.limitations {
            write_diagnostic(
                output,
                root,
                &limitation.location,
                &format!("analysis limitation: {}", limitation.message),
                source(&limitation.location.path),
            )?;
        }
    } else if !result.limitations.is_empty() {
        let plural = if result.limitations.len() == 1 {
            ""
        } else {
            "s"
        };
        writeln!(
            output,
            "{}: analysis limitation: {} recorded limitation{plural}; analysis is incomplete. Use --show-limitations for details.",
            root.expect("compact reports have a root").display(),
            result.limitations.len(),
        )?;
    }
    for error in &result.errors {
        write_diagnostic(
            output,
            root,
            &error.location,
            &format!("error: {}", error.message),
            source(&error.location.path),
        )?;
    }
    Ok(result.status())
}

fn write_diagnostic(
    output: &mut impl Write,
    root: Option<&Path>,
    location: &SourceLocation,
    message: &str,
    source: Option<(&[u8], usize)>,
) -> io::Result<()> {
    if let Some(root) = root {
        write!(output, "[root {}] ", root.display())?;
    }
    writeln!(
        output,
        "{}:{}:{}: bytes {}..{}: {message}",
        location.path.display(),
        location.line,
        location.column,
        location.range.start,
        location.range.end
    )?;
    if let Some((source, offset)) = source {
        write_snippet(output, source, offset, location)?;
    }
    Ok(())
}

fn write_snippet(
    output: &mut impl Write,
    source: &[u8],
    offset: usize,
    location: &SourceLocation,
) -> io::Result<()> {
    let Some(start) = location.range.start.checked_sub(offset) else {
        return Ok(());
    };
    let Some(end) = location.range.end.checked_sub(offset) else {
        return Ok(());
    };
    if start > end || end > source.len() {
        return Ok(());
    }
    // Limit rendering to the affected lines. Convert opaque comment bytes only
    // for display, mapping the existing byte span into that displayed excerpt.
    let line_start = source[..start]
        .iter()
        .rposition(|&byte| byte == b'\n' || byte == b'\r')
        .map_or(0, |position| position + 1);
    let line_end = source[end..]
        .iter()
        .position(|&byte| byte == b'\n' || byte == b'\r')
        .map_or(source.len(), |position| end + position);
    let prefix = String::from_utf8_lossy(&source[line_start..start]);
    let marked = String::from_utf8_lossy(&source[start..end]);
    let excerpt = String::from_utf8_lossy(&source[line_start..line_end]);
    let span = prefix.len()..prefix.len() + marked.len();
    let report = [Group::with_level(Level::WARNING).element(
        Snippet::source(excerpt.as_ref())
            .line_start(location.line)
            .annotation(AnnotationKind::Primary.span(span)),
    )];
    writeln!(output, "{}", Renderer::plain().render(&report))
}
