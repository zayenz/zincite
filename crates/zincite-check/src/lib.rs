//! Check repeated candidates against a prepared Boolean/integer FlatZinc model.
//! Preparation is optional; checking performs no I/O and starts no external tools.

use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[cfg(feature = "prepare")]
mod prepare;
#[cfg(feature = "prepare")]
pub use prepare::{parse_assignment, prepare};
#[cfg(feature = "prepare")]
pub mod cli;

/// Mathematical integers use checked i64 arithmetic. JSON writes decimal strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Value {
    Boolean(bool),
    Integer(#[serde(serialize_with = "serialize_integer")] i64),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boolean(value) => value.fmt(f),
            Self::Integer(value) => value.fmt(f),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueType {
    Boolean,
    Integer,
}

impl Value {
    fn value_type(self) -> ValueType {
        match self {
            Self::Boolean(_) => ValueType::Boolean,
            Self::Integer(_) => ValueType::Integer,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Location {
    pub source: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub location: Option<Location>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Application {
    pub id: String,
    #[serde(serialize_with = "serialize_integers")]
    pub entities: Vec<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Metadata {
    pub application: Option<Application>,
    pub label: Option<String>,
    pub paths: Vec<String>,
    pub is_defined: bool,
    pub defines: Option<String>,
    pub output: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VariableAlias {
    pub name: String,
    pub position: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VariableRef {
    pub name: String,
    /// One-based FlatZinc position for an array element.
    pub position: Option<usize>,
    pub location: Location,
    pub metadata: Metadata,
    pub aliases: Vec<VariableAlias>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConstraintRef {
    pub ordinal: usize,
    pub predicate: String,
    pub location: Location,
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Domain {
    Boolean,
    Integer,
    Interval {
        #[serde(serialize_with = "serialize_integer")]
        lower: i64,
        #[serde(serialize_with = "serialize_integer")]
        upper: i64,
    },
    Enumeration {
        #[serde(serialize_with = "serialize_integers")]
        values: Vec<i64>,
    },
    Fixed {
        value: Value,
    },
}

impl Domain {
    fn contains(&self, value: Value) -> bool {
        match (self, value) {
            (Self::Boolean, Value::Boolean(_)) | (Self::Integer, Value::Integer(_)) => true,
            (Self::Interval { lower, upper }, Value::Integer(value)) => {
                (*lower..=*upper).contains(&value)
            }
            (Self::Enumeration { values }, Value::Integer(value)) => values.contains(&value),
            (Self::Fixed { value: fixed }, value) => *fixed == value,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AssignmentValue {
    Scalar(Value),
    Array(Vec<Value>),
}

#[derive(Clone, Debug)]
pub struct AssignmentEntry {
    pub name: String,
    pub value: AssignmentValue,
    pub location: Option<Location>,
}

/// Entries keep their written order so duplicate names can be diagnosed.
#[derive(Clone, Debug, Default)]
pub struct Assignment {
    pub entries: Vec<AssignmentEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Valid,
    Invalid,
    Incomplete,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CheckRef {
    Domain {
        variable: VariableRef,
        domain: Domain,
    },
    Constraint {
        constraint: ConstraintRef,
    },
}

impl CheckRef {
    pub fn location(&self) -> &Location {
        match self {
            Self::Domain { variable, .. } => &variable.location,
            Self::Constraint { constraint } => &constraint.location,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ArgumentValue {
    pub variable: Option<Box<VariableRef>>,
    pub value: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViolationKind {
    Domain {
        value: Value,
    },
    Linear {
        #[serde(serialize_with = "serialize_integers")]
        coefficients: Vec<i64>,
        terms: Vec<ArgumentValue>,
        #[serde(serialize_with = "serialize_integer")]
        sum: i64,
        #[serde(serialize_with = "serialize_integer")]
        bound: i64,
    },
    ReifiedLessEqual {
        left: ArgumentValue,
        right: ArgumentValue,
        result: ArgumentValue,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct Violation {
    pub check: CheckRef,
    pub details: ViolationKind,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockedCheck {
    pub check: CheckRef,
    pub missing: Vec<VariableRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Completeness {
    pub complete: bool,
    pub unresolved_variables: Vec<VariableRef>,
    pub blocked_checks: Vec<BlockedCheck>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveDirection {
    Minimize,
    Maximize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObjectiveValue {
    pub direction: ObjectiveDirection,
    pub argument: ArgumentValue,
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
    pub status: Status,
    pub violations: Vec<Violation>,
    pub completeness: Completeness,
    pub objective: Option<ObjectiveValue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Assignment,
    Execution,
}

#[derive(Debug, Serialize)]
pub struct CheckError {
    pub kind: ErrorKind,
    pub diagnostics: Vec<Diagnostic>,
}

// Parser-free model construction arrives with prepared-model loading.
#[cfg_attr(not(feature = "prepare"), allow(dead_code))]
#[derive(Clone, Copy, Debug)]
enum Operand {
    Literal(Value),
    Variable(usize),
}

#[derive(Debug)]
struct Slot {
    #[cfg_attr(not(feature = "prepare"), allow(dead_code))]
    kind: ValueType,
    default: Option<Value>,
    reference: VariableRef,
}

#[derive(Clone, Debug)]
struct Binding {
    elements: Vec<Operand>,
    is_array: bool,
    kind: ValueType,
    assignable: bool,
}

#[derive(Debug)]
struct Variable {
    reference: VariableRef,
    slot: usize,
    domains: Vec<Domain>,
}

#[cfg_attr(not(feature = "prepare"), allow(dead_code))]
#[derive(Debug)]
enum Predicate {
    Linear {
        coefficients: Vec<i64>,
        terms: Vec<Operand>,
        bound: i64,
        equality: bool,
    },
    ReifiedLessEqual {
        left: Operand,
        right: Operand,
        result: Operand,
    },
}

#[derive(Debug)]
struct Constraint {
    reference: ConstraintRef,
    predicate: Predicate,
}

/// An executable model. Its fields are private so checking uses validated inputs.
/// Prepared-model serialization/loading and functional completion are later work.
#[derive(Debug)]
pub struct PreparedModel {
    slots: Vec<Slot>,
    bindings: HashMap<String, Binding>,
    variables: Vec<Variable>,
    constraints: Vec<Constraint>,
    objective: Option<(ObjectiveDirection, Operand)>,
}

impl PreparedModel {
    /// Check every available domain and relation using fresh candidate values.
    /// Missing values produce a normal report, including any known violations.
    pub fn check(&self, assignment: &Assignment) -> Result<CheckReport, CheckError> {
        let values = self.assign(assignment)?;
        let unresolved_variables: Vec<_> = self
            .variables
            .iter()
            .filter(|variable| values[variable.slot].is_none())
            .map(|variable| variable.reference.clone())
            .collect();
        let mut violations = Vec::new();
        let mut blocked_checks = Vec::new();
        for variable in &self.variables {
            for domain in &variable.domains {
                let check = CheckRef::Domain {
                    variable: variable.reference.clone(),
                    domain: domain.clone(),
                };
                match values[variable.slot] {
                    Some(value) if !domain.contains(value) => violations.push(Violation {
                        check,
                        details: ViolationKind::Domain { value },
                    }),
                    None => blocked_checks.push(BlockedCheck {
                        check,
                        missing: vec![variable.reference.clone()],
                    }),
                    _ => {}
                }
            }
        }
        for constraint in &self.constraints {
            let check = CheckRef::Constraint {
                constraint: constraint.reference.clone(),
            };
            let missing = self.missing(&constraint.predicate, &values);
            if !missing.is_empty() {
                blocked_checks.push(BlockedCheck { check, missing });
                continue;
            }
            if let Some(details) = self.evaluate(constraint, &values)? {
                violations.push(Violation { check, details });
            }
        }
        violations.sort_by_key(|violation| violation.check.location().start);
        blocked_checks.sort_by_key(|blocked| blocked.check.location().start);
        let complete = unresolved_variables.is_empty() && blocked_checks.is_empty();
        let status = if !violations.is_empty() {
            Status::Invalid
        } else if !complete {
            Status::Incomplete
        } else {
            Status::Valid
        };
        let objective = self.objective.and_then(|(direction, operand)| {
            self.argument(operand, &values)
                .map(|argument| ObjectiveValue {
                    direction,
                    argument,
                })
        });
        Ok(CheckReport {
            status,
            violations,
            completeness: Completeness {
                complete,
                unresolved_variables,
                blocked_checks,
            },
            objective,
        })
    }

    fn assign(&self, assignment: &Assignment) -> Result<Vec<Option<Value>>, CheckError> {
        let mut values = vec![None; self.slots.len()];
        let mut names = HashSet::new();
        for entry in &assignment.entries {
            let fail = |message| CheckError {
                kind: ErrorKind::Assignment,
                diagnostics: vec![Diagnostic {
                    location: entry.location.clone(),
                    message,
                }],
            };
            if !names.insert(&entry.name) {
                return Err(fail(format!("duplicate assignment to '{}'", entry.name)));
            }
            let binding = self
                .bindings
                .get(&entry.name)
                .ok_or_else(|| fail(format!("unknown variable '{}'", entry.name)))?;
            if !binding.assignable {
                return Err(fail(format!(
                    "'{}' is a parameter, not a candidate variable",
                    entry.name
                )));
            }
            let supplied = match (&entry.value, binding.is_array) {
                (AssignmentValue::Scalar(value), false) => std::slice::from_ref(value),
                (AssignmentValue::Array(values), true) => values.as_slice(),
                _ => {
                    return Err(fail(format!(
                        "assignment shape does not match '{}'",
                        entry.name
                    )));
                }
            };
            if supplied.len() != binding.elements.len() {
                return Err(fail(format!(
                    "'{}' expects {} values, got {}",
                    entry.name,
                    binding.elements.len(),
                    supplied.len()
                )));
            }
            for (&operand, &value) in binding.elements.iter().zip(supplied) {
                if value.value_type() != binding.kind {
                    return Err(fail(format!(
                        "assignment type does not match '{}'",
                        entry.name
                    )));
                }
                match operand {
                    Operand::Variable(slot) => {
                        if values[slot].is_some_and(|previous| previous != value) {
                            return Err(fail(format!(
                                "contradictory assignments through alias '{}'",
                                entry.name
                            )));
                        }
                        values[slot] = Some(value);
                    }
                    Operand::Literal(_) => unreachable!("variable bindings contain value slots"),
                }
            }
        }
        for (value, slot) in values.iter_mut().zip(&self.slots) {
            if value.is_none() {
                *value = slot.default;
            }
        }
        Ok(values)
    }

    fn argument(&self, operand: Operand, values: &[Option<Value>]) -> Option<ArgumentValue> {
        match operand {
            Operand::Literal(value) => Some(ArgumentValue {
                variable: None,
                value,
            }),
            Operand::Variable(slot) => values[slot].map(|value| ArgumentValue {
                variable: Some(Box::new(self.slots[slot].reference.clone())),
                value,
            }),
        }
    }

    fn missing(&self, predicate: &Predicate, values: &[Option<Value>]) -> Vec<VariableRef> {
        let operands: &[Operand] = match predicate {
            Predicate::Linear { terms, .. } => terms,
            Predicate::ReifiedLessEqual {
                left,
                right,
                result,
            } => &[*left, *right, *result],
        };
        let mut slots = HashSet::new();
        operands
            .iter()
            .filter_map(|operand| match operand {
                Operand::Variable(slot) if values[*slot].is_none() && slots.insert(*slot) => {
                    Some(self.slots[*slot].reference.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn evaluate(
        &self,
        constraint: &Constraint,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let argument = |operand| {
            self.argument(operand, values)
                .expect("blocked constraints are excluded before evaluation")
        };
        match &constraint.predicate {
            Predicate::Linear {
                coefficients,
                terms,
                bound,
                equality,
            } => {
                let terms: Vec<_> = terms.iter().map(|&operand| argument(operand)).collect();
                let mut sum = 0i64;
                for (&coefficient, term) in coefficients.iter().zip(&terms) {
                    let Value::Integer(value) = term.value else {
                        unreachable!("preparation validates linear operands")
                    };
                    sum = coefficient
                        .checked_mul(value)
                        .and_then(|product| sum.checked_add(product))
                        .ok_or_else(|| CheckError {
                            kind: ErrorKind::Execution,
                            diagnostics: vec![Diagnostic {
                                location: Some(constraint.reference.location.clone()),
                                message: "checked i64 overflow in linear constraint".into(),
                            }],
                        })?;
                }
                let satisfied = if *equality {
                    sum == *bound
                } else {
                    sum <= *bound
                };
                Ok((!satisfied).then(|| ViolationKind::Linear {
                    coefficients: coefficients.clone(),
                    terms,
                    sum,
                    bound: *bound,
                }))
            }
            Predicate::ReifiedLessEqual {
                left,
                right,
                result,
            } => {
                let (left, right, result) = (argument(*left), argument(*right), argument(*result));
                let (Value::Integer(x), Value::Integer(y), Value::Boolean(b)) =
                    (left.value, right.value, result.value)
                else {
                    unreachable!("preparation validates reified operands")
                };
                Ok(((x <= y) != b).then_some(ViolationKind::ReifiedLessEqual {
                    left,
                    right,
                    result,
                }))
            }
        }
    }
}

fn serialize_integer<S: serde::Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

fn serialize_integers<S: serde::Serializer>(
    values: &[i64],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut sequence = serializer.serialize_seq(Some(values.len()))?;
    for value in values {
        sequence.serialize_element(&value.to_string())?;
    }
    sequence.end()
}
