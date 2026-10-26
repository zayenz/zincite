use std::{env, error::Error, fs};
use zincite_check::{
    Assignment, AssignmentEntry, AssignmentValue, CheckRef, CheckReport, Domain, Value,
    VariableRef, Violation, ViolationKind, prepare,
};

const JOBS: [(i64, &str); 2] = [(1, "Cut panels"), (2, "Assemble panels")];

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: schedule-feedback MODEL.fzn")?;
    let source = fs::read_to_string(&path)?;
    let model = prepare(&path, &source).map_err(|errors| format!("{errors:?}"))?;

    for (edit, starts) in [
        ("Feasible edit", Some([1, 4])),
        ("Infeasible edit", Some([8, 8])),
        ("Omitted input", None),
    ] {
        let report = model
            .check(&assignment(starts))
            .map_err(|error| format!("{error:?}"))?;
        show_feedback(edit, &report)?;
    }
    Ok(())
}

// The application sends its entire decision array for each complete edit.
fn assignment(starts: Option<[i64; 2]>) -> Assignment {
    let Some(starts) = starts else {
        return Assignment::default();
    };
    Assignment {
        entries: vec![AssignmentEntry {
            name: "start".into(),
            value: AssignmentValue::Array(starts.into_iter().map(Value::Integer).collect()),
            location: None,
        }],
    }
}

fn show_feedback(edit: &str, report: &CheckReport) -> Result<(), Box<dyn Error>> {
    println!("{edit}: {:?}", report.status);
    for violation in &report.violations {
        let entities = affected_jobs(&violation.check)?;
        let jobs = entities
            .iter()
            .map(|id| {
                JOBS.iter()
                    .find(|(job_id, _)| job_id == id)
                    .map(|(_, name)| *name)
                    .ok_or("model refers to an unknown application job")
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        println!("  {jobs}: {}", feedback_message(violation)?);
    }
    println!(
        "  Complete: {}; unresolved values: {}; blocked checks: {}",
        report.completeness.complete,
        report.completeness.unresolved_variables.len(),
        report.completeness.blocked_checks.len()
    );
    Ok(())
}

fn affected_jobs(check: &CheckRef) -> Result<Vec<i64>, &'static str> {
    match check {
        CheckRef::Constraint { constraint } => constraint
            .metadata
            .application
            .as_ref()
            .map(|application| application.entities.clone())
            .ok_or("constraint lacks application metadata"),
        CheckRef::Domain { variable, .. } => {
            let application = variable
                .metadata
                .application
                .as_ref()
                .ok_or("variable lacks application metadata")?;
            let position = array_position(variable, &application.id)
                .ok_or("domain finding lacks an application array position")?;
            application
                .entities
                .get(position - 1)
                .map(|id| vec![*id])
                .ok_or("array position has no application job")
        }
    }
}

fn array_position(variable: &VariableRef, array: &str) -> Option<usize> {
    if variable.name == array {
        variable.position
    } else {
        variable
            .aliases
            .iter()
            .find(|alias| alias.name == array)
            .and_then(|alias| alias.position)
    }
}

// The application chooses wording from typed fields, never checker-written text.
fn feedback_message(violation: &Violation) -> Result<String, &'static str> {
    match (&violation.check, &violation.details) {
        (
            CheckRef::Domain {
                variable,
                domain: Domain::Interval { lower, upper },
            },
            ViolationKind::Domain {
                value: Value::Integer(value),
            },
        ) => {
            let id = variable
                .metadata
                .application
                .as_ref()
                .ok_or("variable lacks application metadata")?
                .id
                .as_str();
            match id {
                "start" | "finish" => Ok(format!(
                    "{id} time {value} must be between {lower} and {upper}"
                )),
                _ => Err("unexpected scheduling value"),
            }
        }
        (CheckRef::Constraint { constraint }, ViolationKind::Linear { sum, bound, .. }) => {
            let id = constraint
                .metadata
                .application
                .as_ref()
                .ok_or("constraint lacks application metadata")?
                .id
                .as_str();
            match id {
                "finish" => Ok(format!("elapsed time {sum} differs from duration {bound}")),
                "precedence" => Ok(format!(
                    "the first job finishes {sum} hours after the next starts (limit {bound})"
                )),
                "start_budget" => Ok(format!(
                    "combined start times {sum} exceed the budget {bound}"
                )),
                _ => Err("unexpected scheduling rule"),
            }
        }
        (
            _,
            ViolationKind::ReifiedLessEqual {
                left,
                right,
                result,
            },
        ) => {
            let (Value::Integer(start), Value::Integer(threshold), Value::Boolean(early)) =
                (&left.value, &right.value, &result.value)
            else {
                return Err("unexpected early-start value types");
            };
            Ok(format!(
                "early-start flag {early} disagrees with start {start} and threshold {threshold}"
            ))
        }
        _ => Err("unexpected scheduling finding"),
    }
}
