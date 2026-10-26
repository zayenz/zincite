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
    let mut float_absolute = None;
    let mut float_relative = None;
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
            Some("--float-absolute-tolerance" | "--float-relative-tolerance") if options => {
                let text = arguments
                    .next()
                    .ok_or("float tolerance option expects a number")?;
                let value: f64 = text
                    .to_str()
                    .ok_or("float tolerance must be UTF-8")?
                    .parse()
                    .map_err(|_| "invalid float tolerance")?;
                crate::FloatTolerance::new(value, 0.0).map_err(str::to_owned)?;
                if argument == "--float-absolute-tolerance" {
                    float_absolute = Some(value);
                } else {
                    float_relative = Some(value);
                }
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
    let mut model = if matches!(mode, InputMode::Load) {
        PreparedModel::from_json(&model_source)
            .map_err(|error| format!("{}: {}", paths[0].display(), error.message))?
    } else {
        prepare(&paths[0].to_string_lossy(), &model_source)
            .map_err(|errors| diagnostics(&errors))?
    };
    let saved = model.float_tolerance();
    model.set_float_tolerance(
        crate::FloatTolerance::new(
            float_absolute.unwrap_or(saved.absolute()),
            float_relative.unwrap_or(saved.relative()),
        )
        .map_err(str::to_owned)?,
    );
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
        "Usage: {invocation} [--format text|json] MODEL.fzn ASSIGNMENT.dzn\n       {invocation} --export MODEL.json MODEL.fzn\n       {invocation} --load [--format text|json] MODEL.json ASSIGNMENT.dzn\n\nCheck a candidate against the supported Boolean, integer, finite float and integer set FlatZinc subset.\nDerive supported linear-equality, full-reification and set outputs from supplied inputs.\nStrict disjunctive checks accept fixed nonnegative durations.\nMissing inputs or unsupported directions produce an incomplete report.\nNo compiler or solver runs. JSON mathematical integers are decimal strings.\nExport prepares the model and creates a new JSON file, refusing existing paths.\nLoad validates the executable JSON without parsing FlatZinc source.\n--float-absolute-tolerance NUMBER and --float-relative-tolerance NUMBER override one saved setting.\nDefaults: 1e-9 each; finite nonnegative settings required; 0/0 gives exact comparisons.\n\nExit status: 0 valid or exported, 1 invalid or incomplete, 2 input or execution error.\n"
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
            ViolationKind::Automaton { finding } => {
                let word = finding
                    .word
                    .iter()
                    .map(argument_text)
                    .collect::<Vec<_>>()
                    .join(", ");
                let reason = match finding.rejection {
                    crate::AutomatonRejection::OutOfAlphabet { position, symbol } => {
                        format!("symbol {symbol} at position {position} is outside the alphabet")
                    }
                    crate::AutomatonRejection::DeadPath { position, symbol } => {
                        format!("dead path at position {position} on symbol {symbol}")
                    }
                    crate::AutomatonRejection::NonacceptingEnd => "nonaccepting end".into(),
                };
                let _ = writeln!(
                    text,
                    "  automaton word [{word}]: {reason}; initial {}, accepting {}, reached {}, alphabet {}..{}",
                    finding.initial,
                    finding.accepting,
                    finding.reached,
                    finding.minimum_symbol,
                    finding.maximum_symbol
                );
            }
            ViolationKind::Table {
                tuple,
                table,
                width,
                membership,
                condition,
            } => {
                let entries = tuple
                    .iter()
                    .map(argument_text)
                    .collect::<Vec<_>>()
                    .join(", ");
                let fixed = match table {
                    crate::TableData::Integer(values) => values
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    crate::TableData::Boolean(values) => values
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                };
                let _ = writeln!(
                    text,
                    "  table membership {membership}: [{entries}] in rows of width {width} from [{fixed}]{}",
                    condition_text(condition)
                );
            }
            ViolationKind::Ordering { finding } => {
                let _ = writeln!(text, "  {}", ordering_text(finding));
            }
            ViolationKind::Count {
                calculated,
                supplied,
                relation,
                selector,
                code,
                condition,
                ..
            } => {
                let _ = write!(
                    text,
                    "calculated count {calculated} {relation} {}",
                    argument_text(supplied)
                );
                text.push_str(&condition_text(condition));
                match selector {
                    CountSelectorValue::Needle {
                        value,
                        unresolved,
                        irrelevant,
                    } => {
                        let _ = write!(
                            text,
                            "; needle {}",
                            value
                                .as_ref()
                                .map(argument_text)
                                .or_else(|| unresolved
                                    .as_ref()
                                    .map(|variable| format!("{} unavailable", variable.name)))
                                .unwrap_or_else(|| "unavailable".into())
                        );
                        if *irrelevant {
                            text.push_str(" (irrelevant for empty entries)");
                        }
                    }
                    CountSelectorValue::Matches { values } => {
                        let _ = write!(text, "; positional values {values:?}");
                    }
                    CountSelectorValue::Members { set } => {
                        let _ = write!(text, "; membership {set}");
                    }
                    CountSelectorValue::Distinct => text.push_str("; distinct values"),
                }
                if let Some(code) = code {
                    let _ = write!(text, "; code={code}");
                }
                text.push('\n');
            }
            ViolationKind::Cardinality {
                counts,
                outside_cover,
                closed,
                ..
            } => {
                let _ = write!(
                    text,
                    "{} cardinality",
                    if *closed { "closed" } else { "open" }
                );
                for count in counts {
                    let _ = write!(
                        text,
                        "; count[{}] for {} calculated {}",
                        count.position, count.cover, count.calculated
                    );
                    if let Some(supplied) = &count.supplied {
                        let _ = write!(text, ", supplied {}", argument_text(supplied));
                    }
                    if let (Some(lower), Some(upper)) = (count.lower, count.upper) {
                        let _ = write!(text, ", bounds {lower}..{upper}");
                    }
                }
                if !outside_cover.is_empty() {
                    let _ = write!(text, "; outside cover at {outside_cover:?}");
                }
                text.push('\n');
            }
            ViolationKind::CountWindows {
                membership,
                length,
                lower,
                upper,
                windows,
                ..
            } => {
                let _ = write!(
                    text,
                    "membership {membership}, window length {length}, bounds {lower}..{upper}"
                );
                for window in windows {
                    let _ = write!(
                        text,
                        "; window[{}] calculated {}",
                        window.position, window.calculated
                    );
                }
                text.push('\n');
            }
            ViolationKind::ElementRange {
                indices,
                layout,
                length,
                result,
            } => {
                let indices = indices
                    .iter()
                    .map(|index| {
                        index
                            .as_ref()
                            .map(argument_text)
                            .unwrap_or_else(|| "unavailable".into())
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    text,
                    "  element indices [{indices}] outside {layout} (length {length})"
                );
                if let Some(result) = result {
                    let _ = writeln!(text, "  result {}", argument_text(result));
                }
            }
            ViolationKind::ElementMismatch {
                indices,
                layout,
                selected,
                result,
            } => {
                let indices = indices
                    .iter()
                    .map(argument_text)
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    text,
                    "  element indices [{indices}] in {layout}: selected {} differs from result {}",
                    argument_text(selected),
                    argument_text(result)
                );
            }
            ViolationKind::ConditionalMismatch {
                condition,
                selected,
                result,
            } => {
                let _ = writeln!(
                    text,
                    "  condition {} selects {}, which differs from result {}",
                    argument_text(condition),
                    argument_text(selected),
                    argument_text(result)
                );
            }
            ViolationKind::FloatRelation {
                relation,
                left,
                right,
                condition,
                ..
            } => {
                let _ = writeln!(
                    text,
                    "  {} {relation} {}{}",
                    argument_text(left),
                    argument_text(right),
                    condition_text(condition)
                );
            }
            ViolationKind::FloatLinear {
                relation,
                coefficients,
                terms,
                sum,
                bound,
                condition,
            } => {
                let _ = writeln!(
                    text,
                    "  [{}] * [{}] = {sum}; expected {relation} {bound}{}",
                    coefficients
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    terms
                        .iter()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(", "),
                    condition_text(condition)
                );
            }
            ViolationKind::FloatArithmetic {
                operation,
                inputs,
                result,
            } => {
                let _ = writeln!(
                    text,
                    "  {operation:?}({}) disagrees with {}",
                    inputs
                        .iter()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(", "),
                    argument_text(result)
                );
            }
            ViolationKind::FloatIntegerPower {
                base,
                exponent,
                result,
            } => {
                let _ = writeln!(
                    text,
                    "  integer power({}, {exponent}) disagrees with {}",
                    argument_text(base),
                    argument_text(result)
                );
            }
            ViolationKind::FloatArrayExtremum {
                extremum,
                inputs,
                result,
            } => {
                let _ = writeln!(
                    text,
                    "  array {extremum:?}({}) disagrees with {}",
                    inputs
                        .iter()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(", "),
                    argument_text(result)
                );
            }
            ViolationKind::FloatConversion {
                operation,
                input,
                result,
            } => {
                let _ = writeln!(
                    text,
                    "  {operation:?}({}) disagrees with {}",
                    argument_text(input),
                    argument_text(result)
                );
            }
            ViolationKind::FloatMembership {
                membership,
                element,
                intervals,
            } => {
                let _ = writeln!(
                    text,
                    "  {} is outside {membership:?} [{}]",
                    argument_text(element),
                    intervals
                        .iter()
                        .map(|(lower, upper)| format!("{lower}..{upper}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            ViolationKind::BooleanRelation {
                relation,
                left,
                right,
                condition,
            }
            | ViolationKind::IntegerRelation {
                relation,
                left,
                right,
                condition,
            } => {
                let _ = writeln!(
                    text,
                    "  ({}) {relation} ({}){}",
                    argument_text(left),
                    argument_text(right),
                    condition_text(condition)
                );
            }
            ViolationKind::LinearRelation {
                relation,
                coefficients,
                terms,
                sum,
                bound,
                condition,
            } => {
                let _ = writeln!(
                    text,
                    "  coefficients = {coefficients:?}; terms = [{}]; sum = {sum}; {sum} {relation} {bound}{}",
                    terms
                        .iter()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(", "),
                    condition_text(condition)
                );
            }
            ViolationKind::BooleanLinear {
                relation,
                coefficients,
                terms,
                sum,
                bound,
                condition,
            } => {
                let _ = writeln!(
                    text,
                    "  coefficients = {coefficients:?}; terms = [{}]; sum = {sum}; {sum} {relation} ({}){}",
                    terms
                        .iter()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(", "),
                    argument_text(bound),
                    condition_text(condition)
                );
            }
            ViolationKind::BooleanOperation {
                operation,
                inputs,
                condition,
            } => {
                let _ = writeln!(
                    text,
                    "  {operation:?}([{}]){}",
                    inputs
                        .iter()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(", "),
                    condition_text(condition)
                );
            }
            ViolationKind::BooleanClause {
                positive,
                negative,
                condition,
            } => {
                let _ = writeln!(
                    text,
                    "  clause positive = [{}]; negative = [{}]{}",
                    positive
                        .iter()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(", "),
                    negative
                        .iter()
                        .map(argument_text)
                        .collect::<Vec<_>>()
                        .join(", "),
                    condition_text(condition)
                );
            }
            ViolationKind::BooleanToInteger { boolean, integer } => {
                let _ = writeln!(
                    text,
                    "  bool2int({}) = ({})",
                    argument_text(boolean),
                    argument_text(integer)
                );
            }
            ViolationKind::IntegerArithmetic {
                operation,
                inputs,
                result,
            } => {
                let inputs = inputs
                    .iter()
                    .map(argument_text)
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    text,
                    "  {operation:?}({inputs}) = ({})",
                    argument_text(result)
                );
            }
            ViolationKind::SetRelation {
                relation,
                left,
                right,
                condition,
                ..
            } => {
                let _ = writeln!(
                    text,
                    "  ({}) {relation} ({}){}",
                    argument_text(left),
                    argument_text(right),
                    condition_text(condition)
                );
            }
            ViolationKind::SetMembership {
                element,
                set,
                condition,
            } => {
                let _ = writeln!(
                    text,
                    "  ({}) in ({}){}",
                    argument_text(element),
                    argument_text(set),
                    condition_text(condition)
                );
            }
            ViolationKind::SetOperation {
                operation,
                left,
                right,
                result,
            } => {
                let _ = writeln!(
                    text,
                    "  ({}) {operation} ({}) = ({})",
                    argument_text(left),
                    argument_text(right),
                    argument_text(result)
                );
            }
            ViolationKind::SetIn { element, set } => {
                let _ = writeln!(
                    text,
                    "  ({}) in ({})",
                    argument_text(element),
                    argument_text(set)
                );
            }
            ViolationKind::SetEqual { left, right } | ViolationKind::SetSubset { left, right } => {
                let relation = if matches!(violation.details, ViolationKind::SetEqual { .. }) {
                    "="
                } else {
                    "subset"
                };
                let _ = writeln!(
                    text,
                    "  ({}) {relation} ({})",
                    argument_text(left),
                    argument_text(right)
                );
            }
            ViolationKind::SetCardinality { set, cardinality } => {
                let _ = writeln!(
                    text,
                    "  card({}) = ({})",
                    argument_text(set),
                    argument_text(cardinality)
                );
            }
            ViolationKind::SetUnion {
                left,
                right,
                result,
            }
            | ViolationKind::SetIntersection {
                left,
                right,
                result,
            } => {
                let operation = if matches!(violation.details, ViolationKind::SetUnion { .. }) {
                    "union"
                } else {
                    "intersect"
                };
                let _ = writeln!(
                    text,
                    "  ({}) {operation} ({}) = ({})",
                    argument_text(left),
                    argument_text(right),
                    argument_text(result)
                );
            }
            ViolationKind::DisjunctiveStrict { tasks, conflicts } => {
                for task in tasks {
                    let _ = writeln!(
                        text,
                        "  task {}: start ({}); duration ({})",
                        task.position,
                        argument_text(&task.start),
                        argument_text(&task.duration)
                    );
                }
                for pair in conflicts {
                    let _ = writeln!(
                        text,
                        "  conflicting positions: {}, {}",
                        pair.first, pair.second
                    );
                }
            }
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

fn condition_text(condition: &RelationConditionValue) -> String {
    match condition {
        RelationConditionValue::Plain => String::new(),
        RelationConditionValue::Reified { result } => format!(" <-> ({})", argument_text(result)),
        RelationConditionValue::Implied { guard } => format!(" when ({})", argument_text(guard)),
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
        Domain::Float => "float".into(),
        Domain::FloatSet => "set of float".into(),
        Domain::FloatInterval { lower, upper } => format!("{lower}..{upper}"),
        Domain::Boolean => "bool".into(),
        Domain::IntegerSet { upper: Some(upper) } => format!("set of {upper}"),
        Domain::IntegerSet { upper: None } => "set of int".into(),
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

fn ordering_text(finding: &crate::OrderingFinding) -> String {
    use crate::OrderingFinding::*;
    match finding {
        Distinct { conflicts, .. } => format!(
            "distinct values conflict at pairs {}",
            ordering_pairs(conflicts)
        ),
        Equal {
            negated, differing, ..
        } => format!(
            "{}all equal; differing pairs {}",
            if *negated { "not " } else { "" },
            ordering_pairs(differing)
        ),
        Lex {
            strict,
            decisive_position,
            left_length,
            right_length,
            ..
        } => format!(
            "lexicographic {} fails at position {decisive_position:?}; lengths {left_length}, {right_length}",
            if *strict { "less" } else { "less or equal" }
        ),
        NotEqual { .. } => "array disequality fails: all positions agree".into(),
        Monotone {
            decreasing,
            conflicts,
            ..
        } => format!(
            "{} order fails at pairs {}",
            if *decreasing {
                "decreasing"
            } else {
                "increasing"
            },
            ordering_pairs(conflicts)
        ),
        Extremum {
            maximum,
            calculated,
            ..
        } => format!(
            "{} should equal {calculated}",
            if *maximum { "maximum" } else { "minimum" }
        ),
        ArgExtremum {
            maximum,
            permitted_indexes,
            ..
        } => format!(
            "arg-{} permits indexes {permitted_indexes:?}",
            if *maximum { "maximum" } else { "minimum" }
        ),
        Sort {
            mismatches, faults, ..
        } => format!("sort mismatches at {mismatches:?}; permutation faults {faults:?}"),
        Inverse { faults, .. } => format!("inverse faults {faults:?}"),
        Channel {
            offset,
            true_positions,
            value,
            ..
        } => format!(
            "one-hot channel offset {offset}; true positions {true_positions:?}; index {}",
            value
                .value
                .as_ref()
                .map(argument_text)
                .unwrap_or_else(|| "unresolved".into())
        ),
        Precede {
            first,
            second,
            positions,
            ..
        } => format!("{second} lacks an earlier {first} at positions {positions:?}"),
        Member { truth, .. } => format!("membership truth is {truth}; supplied relation conflicts"),
    }
}
fn ordering_pairs(pairs: &[crate::OrderingPair]) -> String {
    pairs
        .iter()
        .map(|pair| format!("({}, {})", pair.first, pair.second))
        .collect::<Vec<_>>()
        .join(", ")
}
