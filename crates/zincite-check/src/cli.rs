//! Filesystem input and rendering shared by both native command entry points.

use crate::*;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

/// Run the native checker. Status 1 covers both invalid and incomplete reports.
pub fn run(arguments: Vec<OsString>, invocation: &str) -> ExitCode {
    match command(arguments, invocation) {
        Ok((output, status)) => match io::stdout().write_all(output.as_bytes()) {
            Ok(()) => ExitCode::from(status),
            Err(error) => fail(&format!("stdout: {error}")),
        },
        Err(error) => fail(&error),
    }
}

enum InputMode {
    Source,
    Load,
    Export(PathBuf),
}

fn command(arguments: Vec<OsString>, invocation: &str) -> Result<(String, u8), String> {
    let mut paths = Vec::new();
    let mut mode = InputMode::Source;
    let mut json = false;
    let mut options = true;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--") if options => options = false,
            Some("-h" | "--help") if options => return Ok((help(invocation), 0)),
            Some("--load" | "--export") if options => {
                if !matches!(mode, InputMode::Source) {
                    return Err(
                        "--load and --export can be used only once and cannot be combined".into(),
                    );
                }
                mode = if argument == "--load" {
                    InputMode::Load
                } else {
                    InputMode::Export(PathBuf::from(
                        arguments.next().ok_or("--export expects an output path")?,
                    ))
                };
            }
            Some("--format") if options => {
                json = match arguments.next().as_deref().and_then(|value| value.to_str()) {
                    Some("json") => true,
                    Some("text") => false,
                    _ => return Err("--format expects text or json".into()),
                };
            }
            Some(value) if options && value.starts_with('-') => {
                return Err(format!("unknown option '{value}'; use {invocation} --help"));
            }
            _ => paths.push(PathBuf::from(argument)),
        }
    }
    let expected = if matches!(mode, InputMode::Export(_)) {
        1
    } else {
        2
    };
    if paths.len() != expected {
        return Err(format!(
            "expected {expected} input file(s); use {invocation} --help"
        ));
    }
    let read = |path: &std::path::Path| {
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let model_source = read(&paths[0])?;
    let model = if matches!(mode, InputMode::Load) {
        PreparedModel::from_json(&model_source)
            .map_err(|error| format!("{}: {}", paths[0].display(), error.message))?
    } else {
        prepare(&paths[0].to_string_lossy(), &model_source)
            .map_err(|errors| diagnostics(&errors))?
    };
    if let InputMode::Export(output) = mode {
        export_model(&model, &output)?;
        return Ok((String::new(), 0));
    }
    let assignment_source = read(&paths[1])?;
    let assignment = parse_assignment(&paths[1].to_string_lossy(), &assignment_source)
        .map_err(|errors| diagnostics(&errors))?;
    let report = model
        .check(&assignment)
        .map_err(|error| diagnostics(&error.diagnostics))?;
    let status = if report.status == Status::Valid { 0 } else { 1 };
    let output = if json {
        let mut output = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("cannot render report: {error}"))?;
        output.push('\n');
        output
    } else {
        render_text(&report)
    };
    Ok((output, status))
}

fn export_model(model: &PreparedModel, output: &std::path::Path) -> Result<(), String> {
    let json = model
        .to_json()
        .map_err(|error| format!("cannot export model: {error}"))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| format!("{}: {error}", output.display()))?;
    writeln!(file, "{json}").map_err(|error| format!("{}: {error}", output.display()))?;
    Ok(())
}

fn fail(message: &str) -> ExitCode {
    let _ = writeln!(io::stderr(), "{message}");
    ExitCode::from(2)
}

fn help(invocation: &str) -> String {
    format!(
        "Usage: {invocation} [--format text|json] MODEL.fzn ASSIGNMENT.dzn\n       {invocation} --export MODEL.json MODEL.fzn\n       {invocation} --load [--format text|json] MODEL.json ASSIGNMENT.dzn\n\nCheck a candidate against the supported Boolean/integer FlatZinc subset.\nDerive supported linear-equality and full-reification outputs from supplied inputs.\nMissing inputs or unsupported directions produce an incomplete report.\nNo compiler or solver runs. JSON mathematical integers are decimal strings.\nExport prepares the model and creates a new JSON file, refusing existing paths.\nLoad validates the executable JSON without parsing FlatZinc source.\n\nExit status: 0 valid or exported, 1 invalid or incomplete, 2 input or execution error.\n"
    )
}

fn diagnostics(errors: &[Diagnostic]) -> String {
    errors
        .iter()
        .map(|error| match &error.location {
            Some(location) => format!(
                "{}:{}:{}: {}",
                location.source, location.line, location.column, error.message
            ),
            None => error.message.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the structured report, including metadata and independent completeness.
pub fn render_text(report: &CheckReport) -> String {
    let status = match report.status {
        Status::Valid => "valid",
        Status::Invalid => "invalid",
        Status::Incomplete => "incomplete",
    };
    let completeness = if report.completeness.complete {
        "complete"
    } else {
        "incomplete"
    };
    let mut text = format!(
        "{status} ({completeness}): {} violation(s)\n",
        report.violations.len()
    );
    for violation in &report.violations {
        let _ = writeln!(
            text,
            "{}: violation: {}",
            location_text(violation.check.location()),
            check_text(&violation.check)
        );
        match &violation.details {
            ViolationKind::Domain { value } => {
                let _ = writeln!(text, "  value = {value}");
            }
            ViolationKind::Linear {
                coefficients,
                terms,
                sum,
                bound,
            } => {
                let _ = writeln!(
                    text,
                    "  coefficients = {coefficients:?}; terms = [{}]; sum = {sum}; bound = {bound}",
                    terms
                        .iter()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            ViolationKind::ReifiedLessEqual {
                left,
                right,
                result,
            } => {
                let _ = writeln!(
                    text,
                    "  ({}) <= ({}) <-> ({})",
                    argument_text(left),
                    argument_text(right),
                    argument_text(result)
                );
            }
        }
    }
    for variable in &report.completeness.unresolved_variables {
        let _ = writeln!(
            text,
            "{}: unresolved: {}{}",
            location_text(&variable.location),
            variable_name(variable),
            metadata_text(&variable.metadata)
        );
    }
    for blocked in &report.completeness.blocked_checks {
        let _ = writeln!(
            text,
            "{}: blocked: {}; missing {}",
            location_text(blocked.check.location()),
            check_text(&blocked.check),
            blocked
                .missing
                .iter()
                .map(variable_name)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if let Some(objective) = &report.objective {
        let direction = match objective.direction {
            ObjectiveDirection::Minimize => "minimize",
            ObjectiveDirection::Maximize => "maximize",
        };
        let _ = writeln!(
            text,
            "objective ({direction}): {}",
            argument_text(&objective.argument)
        );
    }
    text
}

fn location_text(location: &Location) -> String {
    format!("{}:{}:{}", location.source, location.line, location.column)
}

fn variable_name(variable: &VariableRef) -> String {
    let mut name = indexed_name(&variable.name, variable.position);
    if !variable.aliases.is_empty() {
        let _ = write!(
            name,
            " (aliases: {})",
            variable
                .aliases
                .iter()
                .map(|alias| indexed_name(&alias.name, alias.position))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    name
}

fn indexed_name(name: &str, position: Option<usize>) -> String {
    match position {
        Some(position) => format!("{name}[{position}]"),
        None => name.into(),
    }
}

fn argument_text(argument: &ArgumentValue) -> String {
    match &argument.variable {
        Some(variable) => format!("{} = {}", variable_name(variable), argument.value),
        None => argument.value.to_string(),
    }
}

fn check_text(check: &CheckRef) -> String {
    match check {
        CheckRef::Domain { variable, domain } => format!(
            "{} in {}{}",
            variable_name(variable),
            domain_text(domain),
            metadata_text(&variable.metadata)
        ),
        CheckRef::Constraint { constraint } => format!(
            "constraint {} ({}){}",
            constraint.ordinal,
            constraint.predicate,
            metadata_text(&constraint.metadata)
        ),
    }
}

fn domain_text(domain: &Domain) -> String {
    match domain {
        Domain::Boolean => "bool".into(),
        Domain::Integer => "int".into(),
        Domain::Interval { lower, upper } => format!("{lower}..{upper}"),
        Domain::Enumeration { values } => format!(
            "{{{}}}",
            values
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Domain::Fixed { value } => format!("{{{value}}}"),
    }
}

fn metadata_text(metadata: &Metadata) -> String {
    let mut text = String::new();
    if let Some(application) = &metadata.application {
        let _ = write!(
            text,
            " [id={:?}, entities={:?}]",
            application.id, application.entities
        );
    }
    if let Some(label) = &metadata.label {
        let _ = write!(text, " [label={label:?}]");
    }
    for path in &metadata.paths {
        let _ = write!(text, " [mzn_path={path:?}]");
    }
    if metadata.is_defined {
        text.push_str(" [is_defined_var]");
    }
    if let Some(defines) = &metadata.defines {
        let _ = write!(text, " [defines_var={defines}]");
    }
    if metadata.output {
        text.push_str(" [output]");
    }
    text
}
