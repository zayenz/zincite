//! File-aware selected results and the shared text diagnostic renderer.
use std::io::{self, Write};
use std::path::Path;
use zincite_syntax::{FileMode, ParsedFile};

use crate::{
    LintDiagnostic, LintOptions, ModelContext, Rule, Severity, SourceDiagnostic, SourceLocation,
    captures::check_captures, decision_use::check_decision_use, element::check_element, lint_items,
    lint_with_options, resolve_bindings, resolve_callables, resolve_instantiations,
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
    let mut capture_incomplete = false;
    let mut element_incomplete = false;
    let mut decision_incomplete = Vec::new();
    if !data && options.requires_model() {
        let facts = resolve_bindings(context);
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
        if options.rules.contains(&Rule::ElementPredicate) || !decision_rules.is_empty() {
            let calls = resolve_callables(context, &facts);
            if options.rules.contains(&Rule::ElementPredicate) {
                let elements = check_element(context, &facts, &calls);
                element_incomplete = !elements.limitations.is_empty();
                result.findings.extend(elements.findings);
                result.limitations.extend(elements.limitations);
            }
            if !decision_rules.is_empty() {
                let instantiations = resolve_instantiations(context, &facts, &calls);
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
            } else if rule.requires_model()
                && (shared_incomplete
                    || (rule == Rule::GlobalVariableInFunction && capture_incomplete)
                    || (rule == Rule::ElementPredicate && element_incomplete)
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
