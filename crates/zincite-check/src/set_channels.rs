//! Selected set operations and channels with compact virtual prefixes.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SetChannelName {
    Link,
    IntSet,
    Inverse,
    Range,
    Precede,
    Sorted,
    Selected {
        operation: SetAggregate,
        explicit_universe: bool,
    },
}
impl SetChannelName {
    pub fn from_name(name: &str) -> Option<Self> {
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            return Some(match base {
                "gecode_link_set_to_booleans" => Self::Link,
                "gecode_int_set_channel" => Self::IntSet,
                "gecode_inverse_set" => Self::Inverse,
                "gecode_range" => Self::Range,
                "gecode_precede_set" => Self::Precede,
                "gecode_set_channel_sorted" => Self::Sorted,
                "gecode_array_set_element_union" => Self::Selected {
                    operation: SetAggregate::Union,
                    explicit_universe: false,
                },
                "gecode_array_set_element_intersect" => Self::Selected {
                    operation: SetAggregate::Intersection,
                    explicit_universe: false,
                },
                "gecode_array_set_element_partition" => Self::Selected {
                    operation: SetAggregate::Partition,
                    explicit_universe: false,
                },
                "gecode_array_set_element_intersect_in" => Self::Selected {
                    operation: SetAggregate::Intersection,
                    explicit_universe: true,
                },
                _ => continue,
            });
        }
        None
    }
    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<prepare::ArgumentType> {
        use ValueType::*;
        let scalar = |kind, is_var| prepare::ArgumentType {
            kind,
            is_var,
            is_array: false,
        };
        let array = |kind| prepare::ArgumentType {
            kind,
            is_var: true,
            is_array: true,
        };
        let set = scalar(IntegerSet, true);
        let fixed = scalar(Integer, false);
        match self {
            Self::Link => vec![set, array(Boolean), fixed],
            Self::IntSet => vec![array(Integer), fixed.clone(), array(IntegerSet), fixed],
            Self::Inverse => vec![array(IntegerSet), array(IntegerSet), fixed.clone(), fixed],
            Self::Range => vec![array(Integer), fixed, set.clone(), set],
            Self::Precede => vec![array(IntegerSet), fixed.clone(), fixed],
            Self::Sorted => vec![array(Integer), set],
            Self::Selected {
                explicit_universe, ..
            } => {
                let mut args = vec![set.clone(), array(IntegerSet), set];
                if explicit_universe {
                    args.push(scalar(IntegerSet, false));
                }
                args
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum SetChannel {
    Link {
        set: Operand,
        flags: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        offset: i64,
    },
    IntSet {
        integers: Vec<Operand>,
        sets: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        integer_offset: i64,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        set_offset: i64,
    },
    Inverse {
        forward: Vec<Operand>,
        inverse: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        forward_offset: i64,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        inverse_offset: i64,
    },
    Range {
        integers: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        offset: i64,
        selection: Operand,
        result: Operand,
    },
    Precede {
        sets: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        first: i64,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        second: i64,
    },
    Sorted {
        integers: Vec<Operand>,
        set: Operand,
    },
    Selected {
        operation: SetAggregate,
        indices: Operand,
        sets: Vec<Operand>,
        result: Operand,
        universe: Option<IntegerSet>,
    },
}

/// Visible positions are one-based; logical labels include the caller's offset.
#[derive(Clone, Debug, Serialize)]
pub struct SetChannelArgument {
    pub role: SetChannelSide,
    pub position: Option<usize>,
    #[serde(serialize_with = "serialize_optional_integer")]
    pub index: Option<i64>,
    pub value: Option<ArgumentValue>,
    pub unresolved: Option<Box<VariableRef>>,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SetChannelSide {
    Set,
    Flags,
    Integers,
    Sets,
    Forward,
    Inverse,
    Selection,
    Result,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SetChannelFault {
    Outside {
        side: SetChannelSide,
        position: Option<usize>,
        members: IntegerSet,
        allowed: IntegerSet,
    },
    Membership {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        index: i64,
        set_position: usize,
        #[serde(serialize_with = "serialize_integer")]
        set_index: i64,
        expected: bool,
    },
    MembershipCount {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        index: i64,
        memberships: IntegerSet,
    },
    Padding {
        integer_indices: IntegerSet,
        set_indices: IntegerSet,
    },
    Mismatch {
        side: SetChannelSide,
        position: Option<usize>,
        expected: Value,
    },
    Partition {
        first: usize,
        second: usize,
        intersection: IntegerSet,
    },
    Precedence {
        position: usize,
    },
    Order {
        first: usize,
        second: usize,
    },
    Cardinality {
        #[serde(serialize_with = "serialize_integer")]
        expected: i64,
        #[serde(serialize_with = "serialize_integer")]
        actual: i64,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum SetChannelFinding {
    Link {
        arguments: Vec<SetChannelArgument>,
        #[serde(serialize_with = "serialize_integer")]
        offset: i64,
        faults: Vec<SetChannelFault>,
    },
    IntSet {
        arguments: Vec<SetChannelArgument>,
        #[serde(serialize_with = "serialize_integer")]
        integer_offset: i64,
        #[serde(serialize_with = "serialize_integer")]
        set_offset: i64,
        faults: Vec<SetChannelFault>,
    },
    Inverse {
        arguments: Vec<SetChannelArgument>,
        #[serde(serialize_with = "serialize_integer")]
        forward_offset: i64,
        #[serde(serialize_with = "serialize_integer")]
        inverse_offset: i64,
        faults: Vec<SetChannelFault>,
    },
    Range {
        arguments: Vec<SetChannelArgument>,
        #[serde(serialize_with = "serialize_integer")]
        offset: i64,
        faults: Vec<SetChannelFault>,
    },
    Precede {
        arguments: Vec<SetChannelArgument>,
        #[serde(serialize_with = "serialize_integer")]
        first: i64,
        #[serde(serialize_with = "serialize_integer")]
        second: i64,
        faults: Vec<SetChannelFault>,
    },
    Sorted {
        arguments: Vec<SetChannelArgument>,
        faults: Vec<SetChannelFault>,
    },
    Selected {
        arguments: Vec<SetChannelArgument>,
        aggregate: SetAggregate,
        universe: Option<IntegerSet>,
        faults: Vec<SetChannelFault>,
    },
}

impl SetChannel {
    pub fn name(&self) -> SetChannelName {
        match self {
            Self::Link { .. } => SetChannelName::Link,
            Self::IntSet { .. } => SetChannelName::IntSet,
            Self::Inverse { .. } => SetChannelName::Inverse,
            Self::Range { .. } => SetChannelName::Range,
            Self::Precede { .. } => SetChannelName::Precede,
            Self::Sorted { .. } => SetChannelName::Sorted,
            Self::Selected {
                operation,
                universe,
                ..
            } => SetChannelName::Selected {
                operation: *operation,
                explicit_universe: universe.is_some(),
            },
        }
    }
    pub fn validate_shape(&self) -> Result<(), &'static str> {
        match self {
            Self::Link { flags, offset, .. } => {
                checked_length(*offset, flags.len())?;
                if *offset - 1 > GECODE_SET_MAX {
                    return Err("Boolean channel prefix exceeds Gecode set limits");
                }
            }
            Self::IntSet {
                integers,
                sets,
                integer_offset,
                set_offset,
            } => {
                let n = integer_length(*integer_offset, integers.len())?;
                let m = checked_length(*set_offset, sets.len())?;
                if *set_offset > 0 {
                    check_set_constant(*integer_offset - 1)?;
                }
                // The registry posts dom on each visible bucket/integer, even for empty intervals.
                if !sets.is_empty() {
                    check_set_constant(*integer_offset)?;
                    check_set_constant(n - 1)?;
                }
                if !integers.is_empty() && (*set_offset > 2147483646 || m - 1 > 2147483646) {
                    return Err("integer channel domain exceeds Gecode integer limits");
                }
            }
            Self::Inverse {
                forward,
                inverse,
                forward_offset,
                inverse_offset,
            } => {
                checked_length(*forward_offset, forward.len())?;
                checked_length(*inverse_offset, inverse.len())?;
            }
            Self::Range {
                integers, offset, ..
            } => {
                integer_length(*offset, integers.len())?;
            }
            Self::Precede { first, second, .. } => {
                check_set_constant(*first)?;
                check_set_constant(*second)?;
            }
            Self::Selected {
                operation,
                sets,
                universe,
                ..
            } => {
                if !matches!(
                    operation,
                    SetAggregate::Union | SetAggregate::Intersection | SetAggregate::Partition
                ) || universe.is_some() && *operation != SetAggregate::Intersection
                {
                    return Err("invalid selected-set operation or universe");
                }
                checked_length(1, sets.len())?;
                if let Some(universe) = universe
                    && !universe.is_subset(&IntegerSet::interval(
                        i64::from(i32::MIN),
                        i64::from(i32::MAX),
                    ))
                {
                    return Err("fixed intersection universe exceeds native integer bounds");
                }
            }
            Self::Sorted { .. } => {}
        }
        Ok(())
    }
    pub fn typed_operands(&self) -> Vec<(&Operand, ValueType)> {
        use ValueType::*;
        fn array(entries: &[Operand], kind: ValueType) -> Vec<(&Operand, ValueType)> {
            entries.iter().map(|entry| (entry, kind)).collect()
        }
        match self {
            Self::Link { set, flags, .. } => std::iter::once((set, IntegerSet))
                .chain(array(flags, Boolean))
                .collect(),
            Self::IntSet { integers, sets, .. } => array(integers, Integer)
                .into_iter()
                .chain(array(sets, IntegerSet))
                .collect(),
            Self::Inverse {
                forward, inverse, ..
            } => array(forward, IntegerSet)
                .into_iter()
                .chain(array(inverse, IntegerSet))
                .collect(),
            Self::Range {
                integers,
                selection,
                result,
                ..
            } => array(integers, Integer)
                .into_iter()
                .chain([(selection, IntegerSet), (result, IntegerSet)])
                .collect(),
            Self::Precede { sets, .. } => array(sets, IntegerSet),
            Self::Sorted { integers, set } => array(integers, Integer)
                .into_iter()
                .chain([(set, IntegerSet)])
                .collect(),
            Self::Selected {
                indices,
                sets,
                result,
                ..
            } => std::iter::once((indices, IntegerSet))
                .chain(array(sets, IntegerSet))
                .chain([(result, IntegerSet)])
                .collect(),
        }
    }
    pub fn operands(&self) -> Vec<&Operand> {
        self.typed_operands()
            .into_iter()
            .map(|(operand, _)| operand)
            .collect()
    }
    pub fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        if self
            .faults(values)
            .map_or(true, |faults| !faults.is_empty())
        {
            return Vec::new();
        }
        match self {
            Self::Selected {
                indices,
                sets,
                result,
                ..
            } => {
                let mut required = vec![indices, result];
                if let Some(selected) = indices.value(values) {
                    required.extend(
                        sets.iter()
                            .enumerate()
                            .filter(|(i, _)| selected.set().contains(label(1, *i)))
                            .map(|(_, operand)| operand),
                    );
                }
                required
            }
            Self::Range {
                integers,
                offset,
                selection,
                result,
            } => {
                let mut required = vec![selection, result];
                if let Some(selected) = selection.value(values) {
                    required.extend(
                        integers
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| selected.set().contains(label(*offset, *i)))
                            .map(|(_, operand)| operand),
                    );
                }
                required
            }
            Self::Precede {
                sets,
                first,
                second,
            } if sets.is_empty() || first == second => Vec::new(),
            _ => self.operands(),
        }
    }
    pub fn array_outputs(&self) -> Vec<(&[Operand], ValueType)> {
        match self {
            Self::Link { flags, .. } => vec![(flags, ValueType::Boolean)],
            Self::IntSet { integers, sets, .. } => vec![
                (integers, ValueType::Integer),
                (sets, ValueType::IntegerSet),
            ],
            Self::Inverse {
                forward, inverse, ..
            } => vec![
                (forward, ValueType::IntegerSet),
                (inverse, ValueType::IntegerSet),
            ],
            Self::Sorted { integers, .. } => vec![(integers, ValueType::Integer)],
            _ => Vec::new(),
        }
    }
    pub fn can_define(&self, output: usize) -> bool {
        let is_output =
            |operand: &Operand| matches!(operand, Operand::Variable(slot) if *slot == output);
        let contains_output = |operands: &[Operand]| operands.iter().any(is_output);
        match self {
            Self::Link { set, flags, .. } => is_output(set) != contains_output(flags),
            Self::IntSet { integers, sets, .. } => {
                contains_output(integers) != contains_output(sets)
            }
            Self::Inverse {
                forward, inverse, ..
            } => contains_output(forward) != contains_output(inverse),
            Self::Sorted { integers, set } => is_output(set) != contains_output(integers),
            Self::Range {
                integers,
                selection,
                result,
                ..
            } => is_output(result) && !is_output(selection) && !contains_output(integers),
            // A self-reference in an unselected set does not supply an input.
            Self::Selected {
                indices, result, ..
            } => is_output(result) && !is_output(indices),
            Self::Precede { .. } => false,
        }
    }
    pub fn derive(
        &self,
        output: usize,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<Value>, CheckError> {
        let is_output =
            |operand: &Operand| matches!(operand, Operand::Variable(slot) if *slot == output);
        let mut candidates = Vec::new();
        match self {
            Self::Selected {
                operation,
                indices,
                sets,
                universe,
                ..
            } => {
                return Ok(
                    selected_set(*operation, indices, sets, universe.as_ref(), values)
                        .map(Value::IntegerSet),
                );
            }
            Self::Range {
                integers,
                offset,
                selection,
                ..
            } => return Ok(range_set(integers, *offset, selection, values).map(Value::IntegerSet)),
            Self::Link { set, flags, offset } => {
                if is_output(set) {
                    return Ok(link_set(flags, *offset, values).map(Value::IntegerSet));
                }
                let Some(set) = set.value(values) else {
                    return Ok(None);
                };
                for (i, _) in flags.iter().enumerate().filter(|(_, flag)| is_output(flag)) {
                    candidates.push(Value::Boolean(set.set().contains(label(*offset, i))));
                }
            }
            Self::IntSet {
                integers,
                sets,
                integer_offset,
                set_offset,
            } => {
                if sets.iter().any(is_output) {
                    let Some(integers) = integer_values(integers, values) else {
                        return Ok(None);
                    };
                    for (j, _) in sets.iter().enumerate().filter(|(_, set)| is_output(set)) {
                        let bucket = label(*set_offset, j);
                        let mut members = IntegerSet::from_values(
                            integers
                                .iter()
                                .enumerate()
                                .filter(|(_, value)| **value == bucket)
                                .map(|(i, _)| label(*integer_offset, i)),
                        );
                        if bucket == 0 && !integers.is_empty() {
                            members = members.union(&IntegerSet::interval(0, *integer_offset - 1));
                        }
                        candidates.push(Value::IntegerSet(members));
                    }
                } else {
                    let Some(sets) = set_values(sets, values) else {
                        return Ok(None);
                    };
                    for (i, _) in integers
                        .iter()
                        .enumerate()
                        .filter(|(_, integer)| is_output(integer))
                    {
                        let memberships: Vec<_> = sets
                            .iter()
                            .enumerate()
                            .filter(|(_, set)| set.contains(label(*integer_offset, i)))
                            .map(|(j, _)| label(*set_offset, j))
                            .collect();
                        if memberships.len() != 1 {
                            return Ok(None);
                        }
                        candidates.push(Value::Integer(memberships[0]));
                    }
                }
            }
            Self::Inverse {
                forward,
                inverse,
                forward_offset,
                inverse_offset,
            } => {
                let (inputs, input_offset, targets, target_offset) =
                    if forward.iter().any(is_output) {
                        (inverse, *inverse_offset, forward, *forward_offset)
                    } else {
                        (forward, *forward_offset, inverse, *inverse_offset)
                    };
                let Some(inputs) = set_values(inputs, values) else {
                    return Ok(None);
                };
                for (i, _) in targets
                    .iter()
                    .enumerate()
                    .filter(|(_, target)| is_output(target))
                {
                    candidates.push(Value::IntegerSet(IntegerSet::from_values(
                        inputs
                            .iter()
                            .enumerate()
                            .filter(|(_, set)| set.contains(label(target_offset, i)))
                            .map(|(j, _)| label(input_offset, j)),
                    )));
                }
            }
            Self::Sorted { integers, set } => {
                if is_output(set) {
                    return Ok(integer_values(integers, values)
                        .map(|integers| Value::IntegerSet(IntegerSet::from_values(integers))));
                }
                let Some(set) = set.value(values) else {
                    return Ok(None);
                };
                let cardinality = checked_cardinality(set.set(), constraint)?;
                if cardinality != integers.len() as i64 {
                    return Ok(None);
                }
                // Enumerate only after the cardinality matches the actual output array.
                let members = set.set().members();
                for (_, member) in integers
                    .iter()
                    .zip(members)
                    .filter(|(operand, _)| is_output(operand))
                {
                    candidates.push(Value::Integer(member));
                }
            }
            Self::Precede { .. } => return Ok(None),
        }
        Ok(agreed(candidates))
    }
    fn faults(&self, values: &[Option<Value>]) -> Result<Vec<SetChannelFault>, &'static str> {
        use SetChannelFault::*;
        use SetChannelSide as Side;
        let mut faults = Vec::new();
        match self {
            Self::Selected {
                operation,
                indices,
                sets,
                result,
                universe,
            } => {
                outside(
                    &mut faults,
                    Side::Selection,
                    None,
                    indices,
                    &IntegerSet::interval(1, sets.len() as i64),
                    values,
                );
                if let Some(indices) = indices.value(values)
                    && *operation == SetAggregate::Partition
                {
                    for (i, a) in sets
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| indices.set().contains(label(1, *i)))
                    {
                        if let Some(a) = a.value(values) {
                            for (j, b) in sets
                                .iter()
                                .enumerate()
                                .skip(i + 1)
                                .filter(|(j, _)| indices.set().contains(label(1, *j)))
                            {
                                if let Some(b) = b.value(values) {
                                    let intersection = a.set().intersection(b.set());
                                    if !intersection.is_empty() {
                                        faults.push(Partition {
                                            first: i + 1,
                                            second: j + 1,
                                            intersection,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(universe) = universe {
                    outside(&mut faults, Side::Result, None, result, universe, values);
                }
                mismatch(
                    &mut faults,
                    Side::Result,
                    None,
                    result,
                    selected_set(*operation, indices, sets, universe.as_ref(), values)
                        .map(Value::IntegerSet),
                    values,
                );
            }
            Self::Link { set, flags, offset } => {
                outside(
                    &mut faults,
                    Side::Set,
                    None,
                    set,
                    &visible(*offset, flags.len()),
                    values,
                );
                if let Some(set) = set.value(values) {
                    for (i, flag) in flags.iter().enumerate() {
                        mismatch(
                            &mut faults,
                            Side::Flags,
                            Some(i + 1),
                            flag,
                            Some(Value::Boolean(set.set().contains(label(*offset, i)))),
                            values,
                        );
                    }
                }
                mismatch(
                    &mut faults,
                    Side::Set,
                    None,
                    set,
                    link_set(flags, *offset, values).map(Value::IntegerSet),
                    values,
                );
            }
            Self::Range {
                integers,
                offset,
                selection,
                result,
            } => {
                let n = integer_length(*offset, integers.len()).expect("validated interval");
                outside(
                    &mut faults,
                    Side::Selection,
                    None,
                    selection,
                    &IntegerSet::interval(0, n - 1),
                    values,
                );
                mismatch(
                    &mut faults,
                    Side::Result,
                    None,
                    result,
                    range_set(integers, *offset, selection, values).map(Value::IntegerSet),
                    values,
                );
            }
            Self::IntSet {
                integers,
                sets,
                integer_offset: a,
                set_offset: b,
            } => {
                let n = integer_length(*a, integers.len()).expect("validated interval");
                let prefix = if integers.is_empty() {
                    IntegerSet::empty()
                } else {
                    IntegerSet::interval(0, *a - 1)
                };
                if !integers.is_empty() && sets.is_empty() {
                    faults.push(Padding {
                        integer_indices: visible(*a, integers.len()).union(&prefix),
                        set_indices: IntegerSet::empty(),
                    });
                }
                if *b > 0 && integers.is_empty() && *a > 0 {
                    faults.push(Padding {
                        integer_indices: IntegerSet::interval(0, *a - 1),
                        set_indices: IntegerSet::interval(0, 0),
                    });
                }
                // Visible bucket zero cannot contain integer-prefix positions: its dom excludes them.
                if *b == 0 && !sets.is_empty() && !prefix.is_empty() {
                    faults.push(Padding {
                        integer_indices: prefix.clone(),
                        set_indices: IntegerSet::interval(0, 0),
                    });
                }
                for (i, integer) in integers.iter().enumerate() {
                    if let Some(value) = integer.value(values)
                        && !visible(*b, sets.len()).contains(value.integer())
                    {
                        faults.push(Outside {
                            side: Side::Integers,
                            position: Some(i + 1),
                            members: IntegerSet::from_values([value.integer()]),
                            allowed: visible(*b, sets.len()),
                        });
                    }
                }
                for (j, set) in sets.iter().enumerate() {
                    outside(
                        &mut faults,
                        Side::Sets,
                        Some(j + 1),
                        set,
                        &IntegerSet::interval(*a, n - 1),
                        values,
                    );
                    if let Some(set) = set.value(values) {
                        let expected_prefix = if label(*b, j) == 0 {
                            prefix.clone()
                        } else {
                            IntegerSet::empty()
                        };
                        let actual_prefix =
                            set.set().intersection(&IntegerSet::interval(0, *a - 1));
                        if expected_prefix != actual_prefix {
                            faults.push(Padding {
                                integer_indices: expected_prefix
                                    .symmetric_difference(&actual_prefix),
                                set_indices: IntegerSet::interval(label(*b, j), label(*b, j)),
                            });
                        }
                    }
                }
                if let Some(sets) = set_values(sets, values) {
                    for i in 0..integers.len() {
                        let index = label(*a, i);
                        let memberships = IntegerSet::from_values(
                            sets.iter()
                                .enumerate()
                                .filter(|(_, set)| set.contains(index))
                                .map(|(j, _)| label(*b, j)),
                        );
                        if memberships.cardinality() != Some(1) {
                            faults.push(MembershipCount {
                                position: i + 1,
                                index,
                                memberships,
                            });
                        }
                    }
                }
                membership_faults(&mut faults, integers, sets, *a, *b, values, false);
            }
            Self::Inverse {
                forward,
                inverse,
                forward_offset: a,
                inverse_offset: b,
            } => {
                for (i, set) in forward.iter().enumerate() {
                    outside(
                        &mut faults,
                        Side::Forward,
                        Some(i + 1),
                        set,
                        &visible(*b, inverse.len()),
                        values,
                    );
                }
                for (i, set) in inverse.iter().enumerate() {
                    outside(
                        &mut faults,
                        Side::Inverse,
                        Some(i + 1),
                        set,
                        &visible(*a, forward.len()),
                        values,
                    );
                }
                membership_faults(&mut faults, forward, inverse, *a, *b, values, true);
            }
            Self::Precede {
                sets,
                first,
                second,
            } => {
                let mut earlier = false;
                let mut unknown = false;
                for (i, set) in sets.iter().enumerate() {
                    let Some(set) = set.value(values) else {
                        unknown = true;
                        continue;
                    };
                    let (s, t) = (set.set().contains(*first), set.set().contains(*second));
                    if t && !s && !earlier && !unknown {
                        faults.push(Precedence { position: i + 1 });
                    }
                    earlier |= s && !t;
                }
            }
            Self::Sorted { integers, set } => {
                for (i, pair) in integers.windows(2).enumerate() {
                    if let (Some(a), Some(b)) = (pair[0].value(values), pair[1].value(values))
                        && a.integer() >= b.integer()
                    {
                        faults.push(Order {
                            first: i + 1,
                            second: i + 2,
                        });
                    }
                }
                if let Some(set) = set.value(values) {
                    let actual = set
                        .set()
                        .cardinality()
                        .ok_or("checked i64 overflow in sorted channel cardinality")?;
                    let expected = integers.len() as i64;
                    if actual != expected {
                        faults.push(Cardinality { expected, actual });
                    }
                    for (i, integer) in integers.iter().enumerate() {
                        if let Some(integer) = integer.value(values)
                            && !set.set().contains(integer.integer())
                        {
                            faults.push(Outside {
                                side: Side::Integers,
                                position: Some(i + 1),
                                members: IntegerSet::from_values([integer.integer()]),
                                allowed: set.set().clone(),
                            });
                        }
                    }
                }
                mismatch(
                    &mut faults,
                    Side::Set,
                    None,
                    set,
                    integer_values(integers, values)
                        .map(|integers| Value::IntegerSet(IntegerSet::from_values(integers))),
                    values,
                );
            }
        }
        Ok(faults)
    }
}
fn check_set_constant(value: i64) -> Result<(), &'static str> {
    if (GECODE_SET_MIN..=GECODE_SET_MAX).contains(&value) {
        Ok(())
    } else {
        Err("fixed parameter exceeds Gecode set limits")
    }
}
fn checked_length(offset: i64, length: usize) -> Result<i64, &'static str> {
    if offset < 0 {
        return Err("set channel offsets must be nonnegative");
    }
    let length = i64::try_from(length).map_err(|_| "set channel array length exceeds i64")?;
    offset
        .checked_add(length)
        .filter(|n| *n <= i64::from(i32::MAX))
        .ok_or("fixed set channel interval exceeds native index bounds")
}
fn integer_length(offset: i64, length: usize) -> Result<i64, &'static str> {
    let n = checked_length(offset, length)?;
    Ok(if length == 0 { 0 } else { n })
}
fn label(offset: i64, position: usize) -> i64 {
    offset + position as i64
}
fn visible(offset: i64, length: usize) -> IntegerSet {
    IntegerSet::interval(offset, label(offset, length) - 1)
}
fn integer_values(entries: &[Operand], values: &[Option<Value>]) -> Option<Vec<i64>> {
    entries
        .iter()
        .map(|entry| entry.value(values).map(Value::integer))
        .collect()
}
fn set_values<'a>(
    entries: &'a [Operand],
    values: &'a [Option<Value>],
) -> Option<Vec<&'a IntegerSet>> {
    entries
        .iter()
        .map(|entry| entry.value(values).map(Value::set))
        .collect()
}
fn agreed(values: Vec<Value>) -> Option<Value> {
    let first = values.first()?;
    values
        .iter()
        .all(|value| value == first)
        .then(|| first.clone())
}
fn checked_cardinality(set: &IntegerSet, constraint: &Constraint) -> Result<i64, CheckError> {
    set.cardinality().ok_or_else(|| {
        constraint.execution_error("checked i64 overflow in sorted channel cardinality")
    })
}
fn selected_set(
    operation: SetAggregate,
    indices: &Operand,
    sets: &[Operand],
    universe: Option<&IntegerSet>,
    values: &[Option<Value>],
) -> Option<IntegerSet> {
    let indices = indices.value(values)?.set();
    if !indices.is_subset(&IntegerSet::interval(1, sets.len() as i64)) {
        return None;
    }
    let mut result = None;
    for (_, set) in sets
        .iter()
        .enumerate()
        .filter(|(i, _)| indices.contains(label(1, *i)))
    {
        let set = set.value(values)?.set();
        result = Some(match result {
            None => set.clone(),
            Some(result) => {
                if operation == SetAggregate::Intersection {
                    set.intersection(&result)
                } else {
                    set.union(&result)
                }
            }
        });
    }
    Some(result.unwrap_or_else(|| {
        if operation == SetAggregate::Intersection {
            universe
                .cloned()
                .unwrap_or_else(|| IntegerSet::interval(GECODE_SET_MIN, GECODE_SET_MAX))
        } else {
            IntegerSet::empty()
        }
    }))
}
fn range_set(
    integers: &[Operand],
    offset: i64,
    selection: &Operand,
    values: &[Option<Value>],
) -> Option<IntegerSet> {
    let selection = selection.value(values)?.set();
    let n = integer_length(offset, integers.len()).expect("validated interval");
    if !selection.is_subset(&IntegerSet::interval(0, n - 1)) {
        return None;
    }
    let mut result = Vec::new();
    if !selection
        .intersection(&IntegerSet::interval(0, offset - 1))
        .is_empty()
    {
        result.push(0);
    }
    for (_, integer) in integers
        .iter()
        .enumerate()
        .filter(|(i, _)| selection.contains(label(offset, *i)))
    {
        result.push(integer.value(values)?.integer());
    }
    Some(IntegerSet::from_values(result))
}
fn link_set(flags: &[Operand], offset: i64, values: &[Option<Value>]) -> Option<IntegerSet> {
    let flags: Vec<_> = flags
        .iter()
        .map(|flag| flag.value(values).map(Value::boolean))
        .collect::<Option<_>>()?;
    Some(IntegerSet::from_values(
        flags
            .iter()
            .enumerate()
            .filter(|(_, flag)| **flag)
            .map(|(i, _)| label(offset, i)),
    ))
}
fn outside(
    faults: &mut Vec<SetChannelFault>,
    side: SetChannelSide,
    position: Option<usize>,
    operand: &Operand,
    allowed: &IntegerSet,
    values: &[Option<Value>],
) {
    if let Some(set) = operand.value(values) {
        let members = set.set().difference(allowed);
        if !members.is_empty() {
            faults.push(SetChannelFault::Outside {
                side,
                position,
                members,
                allowed: allowed.clone(),
            });
        }
    }
}
fn mismatch(
    faults: &mut Vec<SetChannelFault>,
    side: SetChannelSide,
    position: Option<usize>,
    operand: &Operand,
    expected: Option<Value>,
    values: &[Option<Value>],
) {
    if let Some(expected) = expected
        && operand
            .value(values)
            .is_some_and(|actual| *actual != expected)
    {
        faults.push(SetChannelFault::Mismatch {
            side,
            position,
            expected,
        });
    }
}
fn membership_faults(
    faults: &mut Vec<SetChannelFault>,
    left: &[Operand],
    right: &[Operand],
    a: i64,
    b: i64,
    values: &[Option<Value>],
    left_is_set: bool,
) {
    for (i, left) in left.iter().enumerate() {
        if let Some(left) = left.value(values) {
            for (j, right) in right.iter().enumerate() {
                if let Some(right) = right.value(values) {
                    let expected = if left_is_set {
                        left.set().contains(label(b, j))
                    } else {
                        left.integer() == label(b, j)
                    };
                    if expected != right.set().contains(label(a, i)) {
                        faults.push(SetChannelFault::Membership {
                            position: i + 1,
                            index: label(a, i),
                            set_position: j + 1,
                            set_index: label(b, j),
                            expected,
                        });
                    }
                }
            }
        }
    }
}

impl PreparedModel {
    pub(crate) fn evaluate_set_channel(
        &self,
        constraint: &Constraint,
        channel: &SetChannel,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let faults = channel
            .faults(values)
            .map_err(|message| constraint.execution_error(message))?;
        if faults.is_empty() {
            return Ok(None);
        }
        let argument = |operand: &Operand, role, position, index| SetChannelArgument {
            role,
            position,
            index,
            value: self.argument(operand, values),
            unresolved: match operand {
                Operand::Variable(slot) if values[*slot].is_none() => {
                    Some(Box::new(self.slots[*slot].reference.clone()))
                }
                _ => None,
            },
        };
        let array = |entries: &[Operand], role, offset| {
            entries
                .iter()
                .enumerate()
                .map(|(i, operand)| argument(operand, role, Some(i + 1), Some(label(offset, i))))
                .collect::<Vec<_>>()
        };
        use SetChannelSide as Side;
        let finding = match channel {
            SetChannel::Link { set, flags, offset } => SetChannelFinding::Link {
                arguments: std::iter::once(argument(set, Side::Set, None, None))
                    .chain(array(flags, Side::Flags, *offset))
                    .collect(),
                offset: *offset,
                faults,
            },
            SetChannel::IntSet {
                integers,
                sets,
                integer_offset,
                set_offset,
            } => SetChannelFinding::IntSet {
                arguments: array(integers, Side::Integers, *integer_offset)
                    .into_iter()
                    .chain(array(sets, Side::Sets, *set_offset))
                    .collect(),
                integer_offset: *integer_offset,
                set_offset: *set_offset,
                faults,
            },
            SetChannel::Inverse {
                forward,
                inverse,
                forward_offset,
                inverse_offset,
            } => SetChannelFinding::Inverse {
                arguments: array(forward, Side::Forward, *forward_offset)
                    .into_iter()
                    .chain(array(inverse, Side::Inverse, *inverse_offset))
                    .collect(),
                forward_offset: *forward_offset,
                inverse_offset: *inverse_offset,
                faults,
            },
            SetChannel::Range {
                integers,
                offset,
                selection,
                result,
            } => SetChannelFinding::Range {
                arguments: array(integers, Side::Integers, *offset)
                    .into_iter()
                    .chain([
                        argument(selection, Side::Selection, None, None),
                        argument(result, Side::Result, None, None),
                    ])
                    .collect(),
                offset: *offset,
                faults,
            },
            SetChannel::Precede {
                sets,
                first,
                second,
            } => SetChannelFinding::Precede {
                arguments: array(sets, Side::Sets, 0),
                first: *first,
                second: *second,
                faults,
            },
            SetChannel::Sorted { integers, set } => SetChannelFinding::Sorted {
                arguments: array(integers, Side::Integers, 0)
                    .into_iter()
                    .chain([argument(set, Side::Set, None, None)])
                    .collect(),
                faults,
            },
            SetChannel::Selected {
                operation,
                indices,
                sets,
                result,
                universe,
            } => SetChannelFinding::Selected {
                arguments: std::iter::once(argument(indices, Side::Selection, None, None))
                    .chain(array(sets, Side::Sets, 1))
                    .chain([argument(result, Side::Result, None, None)])
                    .collect(),
                aggregate: *operation,
                universe: universe.clone(),
                faults,
            },
        };
        Ok(Some(ViolationKind::SetChannel { finding }))
    }
}
