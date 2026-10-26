//! Check repeated candidates against a prepared Boolean/integer FlatZinc model.
//! Preparation is optional; checking performs no I/O and starts no external tools.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

mod serialized;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
mod wasm;

#[cfg(feature = "prepare")]
mod prepare;
#[cfg(feature = "prepare")]
pub use prepare::{parse_assignment, prepare};
#[cfg(feature = "prepare")]
pub mod cli;

/// Mathematical integers use checked i64 arithmetic. JSON writes decimal strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Value {
    Boolean(bool),
    Integer(
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        i64,
    ),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boolean(value) => value.fmt(f),
            Self::Integer(value) => value.fmt(f),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Application {
    pub id: String,
    #[serde(
        serialize_with = "serialize_integers",
        deserialize_with = "deserialize_integers"
    )]
    pub entities: Vec<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub application: Option<Application>,
    pub label: Option<String>,
    pub paths: Vec<String>,
    pub is_defined: bool,
    pub defines: Option<String>,
    pub output: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableAlias {
    pub name: String,
    pub position: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableRef {
    pub name: String,
    /// One-based FlatZinc position for an array element.
    pub position: Option<usize>,
    pub location: Location,
    pub metadata: Metadata,
    pub aliases: Vec<VariableAlias>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintRef {
    pub ordinal: usize,
    pub predicate: String,
    pub location: Location,
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Domain {
    Boolean,
    Integer,
    Interval {
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        lower: i64,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        upper: i64,
    },
    Enumeration {
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AssignmentValue {
    Scalar(Value),
    Array(Vec<Value>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentEntry {
    pub name: String,
    pub value: AssignmentValue,
    pub location: Option<Location>,
}

/// Entries keep their written order so duplicate names can be diagnosed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum Operand {
    Literal(Value),
    Variable(usize),
}

impl Operand {
    fn value(self, values: &[Option<Value>]) -> Option<Value> {
        match self {
            Self::Literal(value) => Some(value),
            Self::Variable(slot) => values[slot],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Slot {
    kind: ValueType,
    default: Option<Value>,
    reference: VariableRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Binding {
    elements: Vec<Operand>,
    is_array: bool,
    kind: ValueType,
    assignable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Variable {
    reference: VariableRef,
    slot: usize,
    domains: Vec<Domain>,
}

#[derive(Debug, Serialize, Deserialize)]
enum Predicate {
    Linear {
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        coefficients: Vec<i64>,
        terms: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        bound: i64,
        equality: bool,
    },
    ReifiedLessEqual {
        left: Operand,
        right: Operand,
        result: Operand,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Constraint {
    reference: ConstraintRef,
    predicate: Predicate,
}

impl Constraint {
    fn linear_overflow(&self) -> CheckError {
        CheckError {
            kind: ErrorKind::Execution,
            diagnostics: vec![Diagnostic {
                location: Some(self.reference.location.clone()),
                message: "checked i64 overflow in linear constraint".into(),
            }],
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum DefinitionDirection {
    Linear { term: usize },
    ReifiedLessEqual,
}

#[derive(Debug)]
struct Definition {
    constraint: usize,
    output: usize,
    direction: DefinitionDirection,
}

/// An executable model. Its fields are private so checking uses validated inputs.
/// Export with `to_json`; `from_json` validates the executable model before use.
#[derive(Debug, Serialize)]
pub struct PreparedModel {
    slots: Vec<Slot>,
    bindings: HashMap<String, Binding>,
    variables: Vec<Variable>,
    constraints: Vec<Constraint>,
    #[serde(skip)]
    definitions: Vec<Definition>,
    #[serde(skip)]
    dependents: Vec<Vec<usize>>,
    objective: Option<(ObjectiveDirection, Operand)>,
}

impl PreparedModel {
    /// Check every available domain and relation using fresh candidate values.
    /// Missing values produce a normal report, including any known violations.
    pub fn check(&self, assignment: &Assignment) -> Result<CheckReport, CheckError> {
        let mut values = self.assign(assignment)?;
        self.complete(&mut values)?;
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

    fn complete(&self, values: &mut [Option<Value>]) -> Result<(), CheckError> {
        let mut pending: VecDeque<_> = (0..self.definitions.len()).collect();
        while let Some(index) = pending.pop_front() {
            let definition = &self.definitions[index];
            if values[definition.output].is_some() {
                continue;
            }
            if let Some(value) = self.derive(definition, values)? {
                values[definition.output] = Some(value);
                pending.extend(self.dependents[definition.output].iter().copied());
            }
        }
        Ok(())
    }

    fn derive(
        &self,
        definition: &Definition,
        values: &[Option<Value>],
    ) -> Result<Option<Value>, CheckError> {
        let constraint = &self.constraints[definition.constraint];
        match (&constraint.predicate, definition.direction) {
            (
                Predicate::Linear {
                    coefficients,
                    terms,
                    bound,
                    ..
                },
                DefinitionDirection::Linear { term: output_term },
            ) => {
                if terms
                    .iter()
                    .enumerate()
                    .any(|(index, operand)| index != output_term && operand.value(values).is_none())
                {
                    return Ok(None);
                }
                let mut sum = 0i64;
                for (index, (&coefficient, operand)) in coefficients.iter().zip(terms).enumerate() {
                    if index == output_term {
                        continue;
                    }
                    let Value::Integer(value) = operand
                        .value(values)
                        .expect("definition inputs are available before calculation")
                    else {
                        unreachable!("preparation validates linear operands")
                    };
                    sum = coefficient
                        .checked_mul(value)
                        .and_then(|product| sum.checked_add(product))
                        .ok_or_else(|| constraint.linear_overflow())?;
                }
                let numerator = bound
                    .checked_sub(sum)
                    .ok_or_else(|| constraint.linear_overflow())?;
                let coefficient = coefficients[output_term];
                let remainder = numerator
                    .checked_rem(coefficient)
                    .ok_or_else(|| constraint.linear_overflow())?;
                if remainder != 0 {
                    return Ok(None);
                }
                let value = numerator
                    .checked_div(coefficient)
                    .ok_or_else(|| constraint.linear_overflow())?;
                Ok(Some(Value::Integer(value)))
            }
            (
                Predicate::ReifiedLessEqual { left, right, .. },
                DefinitionDirection::ReifiedLessEqual,
            ) => {
                let (Some(left), Some(right)) = (left.value(values), right.value(values)) else {
                    return Ok(None);
                };
                let (Value::Integer(left), Value::Integer(right)) = (left, right) else {
                    unreachable!("preparation validates reified operands")
                };
                Ok(Some(Value::Boolean(left <= right)))
            }
            _ => unreachable!("preparation selects supported definition directions"),
        }
    }

    fn argument(&self, operand: Operand, values: &[Option<Value>]) -> Option<ArgumentValue> {
        let value = operand.value(values)?;
        Some(ArgumentValue {
            variable: match operand {
                Operand::Literal(_) => None,
                Operand::Variable(slot) => Some(Box::new(self.slots[slot].reference.clone())),
            },
            value,
        })
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
                        .ok_or_else(|| constraint.linear_overflow())?;
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

fn deserialize_integer<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

fn deserialize_integers<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<i64>, D::Error> {
    Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .map(|value| value.parse().map_err(serde::de::Error::custom))
        .collect()
}

impl PreparedModel {
    fn link_aliases(&mut self) {
        let mut aliases = vec![Vec::new(); self.slots.len()];
        for variable in &self.variables {
            aliases[variable.slot].push(VariableAlias {
                name: variable.reference.name.clone(),
                position: variable.reference.position,
            });
        }
        let link = |reference: &mut VariableRef, slot: usize| {
            reference.aliases = aliases[slot]
                .iter()
                .filter(|alias| {
                    alias.name != reference.name || alias.position != reference.position
                })
                .cloned()
                .collect();
        };
        for variable in &mut self.variables {
            link(&mut variable.reference, variable.slot);
        }
        for (slot, value) in self.slots.iter_mut().enumerate() {
            link(&mut value.reference, slot);
        }
    }

    fn prepare_definitions(&mut self) -> Result<(), Diagnostic> {
        let mut defined = HashSet::new();
        self.definitions.clear();
        self.dependents = vec![Vec::new(); self.slots.len()];
        for (constraint_index, constraint) in self.constraints.iter().enumerate() {
            let Some(name) = &constraint.reference.metadata.defines else {
                continue;
            };
            let fail = |message: &str| Diagnostic {
                location: Some(constraint.reference.location.clone()),
                message: message.into(),
            };
            let binding = self
                .bindings
                .get(name)
                .ok_or_else(|| fail("defines_var names an unknown variable"))?;
            let [Operand::Variable(slot)] = binding.elements.as_slice() else {
                return Err(fail("defines_var needs a scalar variable"));
            };
            if binding.is_array || !binding.assignable {
                return Err(fail("defines_var needs a scalar variable"));
            }
            if !self.variables.iter().any(|variable| {
                variable.reference.name == *name && variable.reference.metadata.is_defined
            }) {
                return Err(fail("defines_var target must have is_defined_var"));
            }
            if !defined.insert(*slot) {
                return Err(fail("a variable has multiple defines_var annotations"));
            }
            let direction = definition_direction(constraint, *slot)?;
            // A definition supplies its output even when an alias has a singleton domain.
            self.slots[*slot].default = None;
            if let Some(direction) = direction {
                let inputs: &[Operand] = match &constraint.predicate {
                    Predicate::Linear { terms, .. } => terms,
                    Predicate::ReifiedLessEqual { left, right, .. } => &[*left, *right],
                };
                let definition_index = self.definitions.len();
                let mut inputs_seen = HashSet::new();
                for operand in inputs {
                    if let Operand::Variable(input) = operand
                        && input != slot
                        && inputs_seen.insert(*input)
                    {
                        self.dependents[*input].push(definition_index);
                    }
                }
                self.definitions.push(Definition {
                    constraint: constraint_index,
                    output: *slot,
                    direction,
                });
            }
        }
        for variable in &self.variables {
            if variable.reference.metadata.is_defined && !defined.contains(&variable.slot) {
                return Err(Diagnostic {
                    location: Some(variable.reference.location.clone()),
                    message: "is_defined_var has no matching defines_var relation".into(),
                });
            }
        }
        Ok(())
    }
}

fn definition_direction(
    constraint: &Constraint,
    output: usize,
) -> Result<Option<DefinitionDirection>, Diagnostic> {
    let absent = || Diagnostic {
        location: Some(constraint.reference.location.clone()),
        message: "defines_var target does not occur in its relation".into(),
    };
    match &constraint.predicate {
        Predicate::Linear {
            coefficients,
            terms,
            equality,
            ..
        } => {
            let occurrences: Vec<_> = terms
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    matches!(operand, Operand::Variable(slot) if *slot == output).then_some(index)
                })
                .collect();
            let &term = occurrences.first().ok_or_else(absent)?;
            Ok(
                (*equality && occurrences.len() == 1 && coefficients[term] != 0)
                    .then_some(DefinitionDirection::Linear { term }),
            )
        }
        Predicate::ReifiedLessEqual {
            left,
            right,
            result,
        } => {
            if ![left, right, result]
                .iter()
                .any(|operand| matches!(operand, Operand::Variable(slot) if *slot == output))
            {
                return Err(absent());
            }
            Ok(matches!(result, Operand::Variable(slot) if *slot == output)
                .then_some(DefinitionDirection::ReifiedLessEqual))
        }
    }
}

impl Domain {
    fn singleton(&self) -> Option<Value> {
        match self {
            Self::Interval { lower, upper } if lower == upper => Some(Value::Integer(*lower)),
            Self::Enumeration { values } if values.len() == 1 => Some(Value::Integer(values[0])),
            Self::Fixed { value } => Some(*value),
            _ => None,
        }
    }
}
