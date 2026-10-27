//! Compact aggregate, ordered and weighted set relations and scalar outputs.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetAggregate {
    Union,
    Intersection,
    Partition,
    SequenceUnion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SetGlobalName {
    Aggregate(SetAggregate),
    Convex,
    Hull,
    Sequence,
    AtMostOne,
    Weights,
    Extremum { maximum: bool, mode: RelationMode },
}
impl SetGlobalName {
    pub fn from_name(name: &str) -> Option<Self> {
        // Registry::add prefixes each literal root exactly once.
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            let found = match base {
                "array_set_union" => Self::Aggregate(SetAggregate::Union),
                "array_set_partition" => Self::Aggregate(SetAggregate::Partition),
                "array_set_seq_union" => Self::Aggregate(SetAggregate::SequenceUnion),
                "gecode_array_set_intersect" => Self::Aggregate(SetAggregate::Intersection),
                "set_convex" => Self::Convex,
                "array_set_seq" => Self::Sequence,
                "gecode_set_convex_hull" => Self::Hull,
                "gecode_set_atmost_one" => Self::AtMostOne,
                "gecode_set_weights" => Self::Weights,
                "gecode_set_min" => Self::Extremum {
                    maximum: false,
                    mode: RelationMode::Plain,
                },
                "gecode_set_max" => Self::Extremum {
                    maximum: true,
                    mode: RelationMode::Plain,
                },
                "gecode_set_min_reif" => Self::Extremum {
                    maximum: false,
                    mode: RelationMode::Reified,
                },
                "gecode_set_max_reif" => Self::Extremum {
                    maximum: true,
                    mode: RelationMode::Reified,
                },
                "gecode_set_min_imp" => Self::Extremum {
                    maximum: false,
                    mode: RelationMode::Implied,
                },
                "gecode_set_max_imp" => Self::Extremum {
                    maximum: true,
                    mode: RelationMode::Implied,
                },
                _ => continue,
            };
            return Some(found);
        }
        None
    }
    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<prepare::ArgumentType> {
        let scalar = |kind, is_var| prepare::ArgumentType {
            kind,
            is_var,
            is_array: false,
        };
        let array = |kind, is_var| prepare::ArgumentType {
            kind,
            is_var,
            is_array: true,
        };
        let set = scalar(ValueType::IntegerSet, true);
        let sets = array(ValueType::IntegerSet, true);
        match self {
            Self::Aggregate(_) => vec![sets, set],
            Self::Convex => vec![set],
            Self::Hull => vec![set.clone(), set],
            Self::Sequence => vec![sets],
            Self::AtMostOne => vec![sets, scalar(ValueType::Integer, false)],
            Self::Weights => vec![
                array(ValueType::Integer, false),
                array(ValueType::Integer, false),
                set,
                scalar(ValueType::Integer, true),
            ],
            Self::Extremum { mode, .. } => {
                let mut args = vec![set, scalar(ValueType::Integer, true)];
                if mode != RelationMode::Plain {
                    args.push(scalar(ValueType::Boolean, true));
                }
                args
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum SetGlobal {
    Aggregate {
        operation: SetAggregate,
        entries: Vec<Operand>,
        result: Operand,
    },
    Convex {
        set: Operand,
    },
    Hull {
        set: Operand,
        result: Operand,
    },
    Sequence {
        entries: Vec<Operand>,
    },
    AtMostOne {
        entries: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        cardinality: i64,
    },
    Weights {
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        elements: Vec<i64>,
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        weights: Vec<i64>,
        set: Operand,
        result: Operand,
    },
    Extremum {
        set: Operand,
        integer: Operand,
        maximum: bool,
        condition: RelationCondition,
    },
}

/// Array positions are one-based; unavailable values carry their variable reference.
#[derive(Clone, Debug, Serialize)]
pub struct SetArgument {
    pub position: Option<usize>,
    pub value: Option<ArgumentValue>,
    pub unresolved: Option<Box<VariableRef>>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SetPair {
    pub first: usize,
    pub second: usize,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SetCardinalityFault {
    Cardinality {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        actual: i64,
    },
    Intersection {
        first: usize,
        second: usize,
        #[serde(serialize_with = "serialize_integer")]
        actual: i64,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct SetWeight {
    pub position: usize,
    #[serde(serialize_with = "serialize_integer")]
    pub element: i64,
    #[serde(serialize_with = "serialize_integer")]
    pub weight: i64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum SetGlobalFinding {
    Aggregate {
        aggregate: SetAggregate,
        entries: Vec<SetArgument>,
        result: SetArgument,
        expected: Option<IntegerSet>,
        conflicts: Vec<SetPair>,
    },
    Convex {
        set: SetArgument,
        hull: IntegerSet,
    },
    Hull {
        set: SetArgument,
        result: SetArgument,
        expected: IntegerSet,
    },
    Sequence {
        entries: Vec<SetArgument>,
        conflicts: Vec<SetPair>,
    },
    AtMostOne {
        entries: Vec<SetArgument>,
        #[serde(serialize_with = "serialize_integer")]
        cardinality: i64,
        faults: Vec<SetCardinalityFault>,
    },
    Weights {
        rows: Vec<SetWeight>,
        set: SetArgument,
        result: SetArgument,
        uncovered: IntegerSet,
        #[serde(serialize_with = "serialize_optional_integer")]
        expected: Option<i64>,
    },
    Extremum {
        set: SetArgument,
        integer: SetArgument,
        maximum: bool,
        #[serde(serialize_with = "serialize_optional_integer")]
        expected: Option<i64>,
        truth: bool,
        condition: RelationConditionValue,
    },
}

impl SetGlobal {
    pub fn name(&self) -> SetGlobalName {
        match self {
            Self::Aggregate { operation, .. } => SetGlobalName::Aggregate(*operation),
            Self::Convex { .. } => SetGlobalName::Convex,
            Self::Hull { .. } => SetGlobalName::Hull,
            Self::Sequence { .. } => SetGlobalName::Sequence,
            Self::AtMostOne { .. } => SetGlobalName::AtMostOne,
            Self::Weights { .. } => SetGlobalName::Weights,
            Self::Extremum {
                maximum, condition, ..
            } => SetGlobalName::Extremum {
                maximum: *maximum,
                mode: condition.mode(),
            },
        }
    }
    pub fn validate_shape(&self) -> Result<(), &'static str> {
        match self {
            Self::AtMostOne { cardinality, .. } if *cardinality < 0 => {
                Err("set cardinality must be nonnegative")
            }
            Self::Weights {
                elements, weights, ..
            } if elements.len() != weights.len() => {
                Err("set element and weight arrays must have equal lengths")
            }
            _ => Ok(()),
        }
    }
    pub fn operands(&self) -> Vec<&Operand> {
        match self {
            Self::Aggregate {
                entries, result, ..
            } => entries.iter().chain(std::iter::once(result)).collect(),
            Self::Convex { set } => vec![set],
            Self::Hull { set, result } | Self::Weights { set, result, .. } => vec![set, result],
            Self::Sequence { entries } | Self::AtMostOne { entries, .. } => {
                entries.iter().collect()
            }
            Self::Extremum {
                set,
                integer,
                condition,
                ..
            } => [set, integer]
                .into_iter()
                .chain(condition.operand())
                .collect(),
        }
    }
    pub fn inactive(&self, values: &[Option<Value>]) -> bool {
        matches!(self, Self::Extremum { condition, .. } if condition.inactive(values))
    }
    pub fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        match self {
            Self::Aggregate {
                operation, entries, ..
            } if !aggregate_conflicts(*operation, entries, values).is_empty() => Vec::new(),
            Self::Sequence { entries }
                if entries.len() < 2 || !sequence_conflicts(entries, values).is_empty() =>
            {
                Vec::new()
            }
            Self::AtMostOne {
                entries,
                cardinality,
            } if entries.iter().any(|entry| {
                entry
                    .value(values)
                    .is_some_and(|value| value.set().cardinality() != Some(*cardinality))
            }) || has_intersection_fault(entries, values) =>
            {
                Vec::new()
            }
            Self::Weights { elements, set, .. }
                if set
                    .value(values)
                    .is_some_and(|value| !uncovered(elements, value.set()).is_empty()) =>
            {
                vec![set]
            }
            Self::Extremum { set, condition, .. }
                if set
                    .value(values)
                    .is_some_and(|value| value.set().is_empty()) =>
            {
                std::iter::once(set).chain(condition.operand()).collect()
            }
            _ => self.operands(),
        }
    }
    fn output(&self) -> Option<&Operand> {
        match self {
            Self::Aggregate { result, .. }
            | Self::Hull { result, .. }
            | Self::Weights { result, .. } => Some(result),
            Self::Extremum {
                integer,
                condition: RelationCondition::Plain,
                ..
            } => Some(integer),
            Self::Extremum {
                condition: RelationCondition::Reified { result },
                ..
            } => Some(result),
            _ => None,
        }
    }
    pub fn can_define(&self, output: usize) -> bool {
        matches!(self.output(), Some(Operand::Variable(slot)) if *slot == output)
            && self
                .operands()
                .iter()
                .filter(|operand| matches!(operand, Operand::Variable(slot) if *slot == output))
                .count()
                == 1
    }
    pub fn derive(
        &self,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<Value>, CheckError> {
        Ok(match self {
            Self::Aggregate {
                operation, entries, ..
            } => aggregate(*operation, entries, values).map(Value::IntegerSet),
            Self::Hull { set, .. } => set
                .value(values)
                .map(|value| Value::IntegerSet(value.set().hull())),
            Self::Weights {
                elements,
                weights,
                set,
                ..
            } => {
                let Some(set) = set.value(values) else {
                    return Ok(None);
                };
                weighted_sum(elements, weights, set.set(), constraint)?.map(Value::Integer)
            }
            Self::Extremum {
                set,
                integer,
                maximum,
                condition,
            } => {
                let Some(set) = set.value(values) else {
                    return Ok(None);
                };
                let best = extremum(set.set(), *maximum);
                match condition {
                    RelationCondition::Plain => best.map(Value::Integer),
                    RelationCondition::Reified { .. } => {
                        let truth = match best {
                            None => Some(false),
                            Some(best) => {
                                integer.value(values).map(|value| value.integer() == best)
                            }
                        };
                        truth.map(Value::Boolean)
                    }
                    RelationCondition::Implied { .. } => None,
                }
            }
            _ => None,
        })
    }
}
fn aggregate(
    operation: SetAggregate,
    entries: &[Operand],
    values: &[Option<Value>],
) -> Option<IntegerSet> {
    let mut concrete = entries
        .iter()
        .map(|entry| entry.value(values).map(Value::set));
    let mut result = if operation == SetAggregate::Intersection {
        match concrete.next() {
            Some(first) => first?.clone(),
            None => IntegerSet::interval(GECODE_SET_MIN, GECODE_SET_MAX),
        }
    } else {
        IntegerSet::empty()
    };
    for set in concrete {
        result = if operation == SetAggregate::Intersection {
            result.intersection(set?)
        } else {
            result.union(set?)
        };
    }
    Some(result)
}
fn aggregate_conflicts(
    operation: SetAggregate,
    entries: &[Operand],
    values: &[Option<Value>],
) -> Vec<SetPair> {
    match operation {
        SetAggregate::Partition => pairs(entries, values, |a, b| !a.is_disjoint(b)),
        SetAggregate::SequenceUnion => sequence_conflicts(entries, values),
        _ => Vec::new(),
    }
}
fn pairs(
    entries: &[Operand],
    values: &[Option<Value>],
    conflict: impl Fn(&IntegerSet, &IntegerSet) -> bool,
) -> Vec<SetPair> {
    let mut conflicts = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        let Some(a) = entry.value(values) else {
            continue;
        };
        for (j, other) in entries.iter().enumerate().skip(i + 1) {
            if let Some(b) = other.value(values)
                && conflict(a.set(), b.set())
            {
                conflicts.push(SetPair {
                    first: i + 1,
                    second: j + 1,
                });
            }
        }
    }
    conflicts
}
fn sequence_conflicts(entries: &[Operand], values: &[Option<Value>]) -> Vec<SetPair> {
    pairs(entries, values, |a, b| match (a.maximum(), b.minimum()) {
        (Some(a), Some(b)) => a >= b,
        _ => false,
    })
}
fn has_intersection_fault(entries: &[Operand], values: &[Option<Value>]) -> bool {
    !pairs(entries, values, |a, b| {
        a.intersection(b).cardinality().is_none_or(|size| size > 1)
    })
    .is_empty()
}
fn cardinality_faults(
    entries: &[Operand],
    cardinality: i64,
    values: &[Option<Value>],
    constraint: &Constraint,
) -> Result<Vec<SetCardinalityFault>, CheckError> {
    let size = |set: &IntegerSet| {
        set.cardinality()
            .ok_or_else(|| constraint.execution_error("set cardinality exceeds i64::MAX"))
    };
    let mut faults = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        let Some(a) = entry.value(values) else {
            continue;
        };
        let actual = size(a.set())?;
        if actual != cardinality {
            faults.push(SetCardinalityFault::Cardinality {
                position: i + 1,
                actual,
            });
        }
        for (j, other) in entries.iter().enumerate().skip(i + 1) {
            if let Some(b) = other.value(values) {
                let actual = size(&a.set().intersection(b.set()))?;
                if actual > 1 {
                    faults.push(SetCardinalityFault::Intersection {
                        first: i + 1,
                        second: j + 1,
                        actual,
                    });
                }
            }
        }
    }
    Ok(faults)
}
fn extremum(set: &IntegerSet, maximum: bool) -> Option<i64> {
    if maximum {
        set.maximum()
    } else {
        set.minimum()
    }
}
fn uncovered(elements: &[i64], selected: &IntegerSet) -> IntegerSet {
    selected.difference(&IntegerSet::from_values(elements.iter().copied()))
}
fn weighted_sum(
    elements: &[i64],
    weights: &[i64],
    selected: &IntegerSet,
    constraint: &Constraint,
) -> Result<Option<i64>, CheckError> {
    if !uncovered(elements, selected).is_empty() {
        return Ok(None);
    }
    // First original row wins; key order fixes checked intermediate arithmetic.
    let mut mapping = BTreeMap::new();
    for (&element, &weight) in elements.iter().zip(weights) {
        mapping.entry(element).or_insert(weight);
    }
    let mut sum = 0i64;
    for (element, weight) in mapping {
        if selected.contains(element) {
            sum = sum.checked_add(weight).ok_or_else(|| {
                constraint.execution_error("checked i64 overflow in set weighted sum")
            })?;
        }
    }
    Ok(Some(sum))
}
impl PreparedModel {
    fn set_argument(
        &self,
        operand: &Operand,
        position: Option<usize>,
        values: &[Option<Value>],
    ) -> SetArgument {
        SetArgument {
            position,
            value: self.argument(operand, values),
            unresolved: match operand {
                Operand::Variable(slot) if values[*slot].is_none() => {
                    Some(Box::new(self.slots[*slot].reference.clone()))
                }
                _ => None,
            },
        }
    }
    pub(crate) fn evaluate_set_global(
        &self,
        constraint: &Constraint,
        global: &SetGlobal,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let argument = |operand: &Operand| self.set_argument(operand, None, values);
        let arguments = |entries: &[Operand]| {
            entries
                .iter()
                .enumerate()
                .map(|(i, entry)| self.set_argument(entry, Some(i + 1), values))
                .collect()
        };
        let finding = match global {
            SetGlobal::Aggregate {
                operation,
                entries,
                result,
            } => {
                let expected = aggregate(*operation, entries, values);
                let conflicts = aggregate_conflicts(*operation, entries, values);
                let mismatch = expected
                    .as_ref()
                    .zip(result.value(values))
                    .is_some_and(|(expected, actual)| expected != actual.set());
                if !mismatch && conflicts.is_empty() {
                    return Ok(None);
                }
                SetGlobalFinding::Aggregate {
                    aggregate: *operation,
                    entries: arguments(entries),
                    result: argument(result),
                    expected,
                    conflicts,
                }
            }
            SetGlobal::Convex { set } => {
                let value = set.value(values).expect("required convex set").set();
                if value.is_convex() {
                    return Ok(None);
                }
                SetGlobalFinding::Convex {
                    set: argument(set),
                    hull: value.hull(),
                }
            }
            SetGlobal::Hull { set, result } => {
                let expected = set.value(values).expect("required hull set").set().hull();
                if result.value(values).expect("required hull result").set() == &expected {
                    return Ok(None);
                }
                SetGlobalFinding::Hull {
                    set: argument(set),
                    result: argument(result),
                    expected,
                }
            }
            SetGlobal::Sequence { entries } => {
                let conflicts = sequence_conflicts(entries, values);
                if conflicts.is_empty() {
                    return Ok(None);
                }
                SetGlobalFinding::Sequence {
                    entries: arguments(entries),
                    conflicts,
                }
            }
            SetGlobal::AtMostOne {
                entries,
                cardinality,
            } => {
                let faults = cardinality_faults(entries, *cardinality, values, constraint)?;
                if faults.is_empty() {
                    return Ok(None);
                }
                SetGlobalFinding::AtMostOne {
                    entries: arguments(entries),
                    cardinality: *cardinality,
                    faults,
                }
            }
            SetGlobal::Weights {
                elements,
                weights,
                set,
                result,
            } => {
                let selected = set.value(values).expect("required weighted set").set();
                let missing = uncovered(elements, selected);
                let expected = weighted_sum(elements, weights, selected, constraint)?;
                let mismatch = expected
                    .zip(result.value(values))
                    .is_some_and(|(expected, actual)| expected != actual.integer());
                if missing.is_empty() && !mismatch {
                    return Ok(None);
                }
                let rows = elements
                    .iter()
                    .zip(weights)
                    .enumerate()
                    .map(|(i, (&element, &weight))| SetWeight {
                        position: i + 1,
                        element,
                        weight,
                    })
                    .collect();
                SetGlobalFinding::Weights {
                    rows,
                    set: argument(set),
                    result: argument(result),
                    uncovered: missing,
                    expected,
                }
            }
            SetGlobal::Extremum {
                set,
                integer,
                maximum,
                condition,
            } => {
                let expected = extremum(
                    set.value(values).expect("required extremum set").set(),
                    *maximum,
                );
                let truth = expected.is_some_and(|best| {
                    integer
                        .value(values)
                        .expect("required nonempty extremum integer")
                        .integer()
                        == best
                });
                if condition.satisfied(truth, values) {
                    return Ok(None);
                }
                SetGlobalFinding::Extremum {
                    set: argument(set),
                    integer: argument(integer),
                    maximum: *maximum,
                    expected,
                    truth,
                    condition: self.condition_value(condition, values),
                }
            }
        };
        Ok(Some(ViolationKind::SetGlobal { finding }))
    }
}
