//! Exact ordering signatures, concrete relations and uniquely determined outputs.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OrderingName {
    Distinct,
    OffsetDistinct,
    OptionalDistinct,
    ExceptDistinct,
    Equal,
    NotAllEqual,
    Lex {
        boolean: bool,
        strict: bool,
    },
    NotEqual {
        boolean: bool,
    },
    Monotone {
        boolean: bool,
        decreasing: bool,
    },
    Extremum {
        maximum: bool,
    },
    ArgExtremum {
        boolean: bool,
        maximum: bool,
        explicit_ties: bool,
    },
    Sort,
    SortPermutation,
    Inverse,
    Channel,
    Precede,
    Member {
        boolean: bool,
        reified: bool,
    },
}
impl OrderingName {
    pub fn from_name(name: &str) -> Option<Self> {
        // Registry::add prefixes the complete literal, including gecode_.
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            let found = match base {
                "all_different_int" => Self::Distinct,
                "all_different_offset" | "gecode_all_different_offset_fzn" => Self::OffsetDistinct,
                "gecode_all_different_optional" => Self::OptionalDistinct,
                "gecode_all_different_except" => Self::ExceptDistinct,
                "all_equal_int" => Self::Equal,
                "gecode_not_all_equal_int" => Self::NotAllEqual,
                "array_int_lt" => Self::Lex {
                    boolean: false,
                    strict: true,
                },
                "array_int_lq" => Self::Lex {
                    boolean: false,
                    strict: false,
                },
                "array_bool_lt" => Self::Lex {
                    boolean: true,
                    strict: true,
                },
                "array_bool_lq" => Self::Lex {
                    boolean: true,
                    strict: false,
                },
                "gecode_array_int_ne" => Self::NotEqual { boolean: false },
                "gecode_array_bool_ne" => Self::NotEqual { boolean: true },
                "increasing_int" => Self::Monotone {
                    boolean: false,
                    decreasing: false,
                },
                "increasing_bool" => Self::Monotone {
                    boolean: true,
                    decreasing: false,
                },
                "decreasing_int" => Self::Monotone {
                    boolean: false,
                    decreasing: true,
                },
                "decreasing_bool" => Self::Monotone {
                    boolean: true,
                    decreasing: true,
                },
                "array_int_minimum" => Self::Extremum { maximum: false },
                "array_int_maximum" => Self::Extremum { maximum: true },
                "gecode_minimum_arg_int_offset" => Self::ArgExtremum {
                    boolean: false,
                    maximum: false,
                    explicit_ties: false,
                },
                "gecode_maximum_arg_int_offset" => Self::ArgExtremum {
                    boolean: false,
                    maximum: true,
                    explicit_ties: false,
                },
                "gecode_minimum_arg_bool_offset" => Self::ArgExtremum {
                    boolean: true,
                    maximum: false,
                    explicit_ties: false,
                },
                "gecode_maximum_arg_bool_offset" => Self::ArgExtremum {
                    boolean: true,
                    maximum: true,
                    explicit_ties: false,
                },
                "gecode_arg_min_int" => Self::ArgExtremum {
                    boolean: false,
                    maximum: false,
                    explicit_ties: true,
                },
                "gecode_arg_max_int" => Self::ArgExtremum {
                    boolean: false,
                    maximum: true,
                    explicit_ties: true,
                },
                "gecode_arg_min_bool" => Self::ArgExtremum {
                    boolean: true,
                    maximum: false,
                    explicit_ties: true,
                },
                "gecode_arg_max_bool" => Self::ArgExtremum {
                    boolean: true,
                    maximum: true,
                    explicit_ties: true,
                },
                "sort" => Self::Sort,
                "gecode_sort_permutation" => Self::SortPermutation,
                "inverse_offsets" => Self::Inverse,
                "gecode_bool_int_channel" => Self::Channel,
                "gecode_precede" => Self::Precede,
                "member_int" => Self::Member {
                    boolean: false,
                    reified: false,
                },
                "member_bool" => Self::Member {
                    boolean: true,
                    reified: false,
                },
                "gecode_member_int_reif" => Self::Member {
                    boolean: false,
                    reified: true,
                },
                "gecode_member_bool_reif" => Self::Member {
                    boolean: true,
                    reified: true,
                },
                _ => continue,
            };
            return Some(found);
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
        let array = |kind, is_var| prepare::ArgumentType {
            kind,
            is_var,
            is_array: true,
        };
        let ints = array(Integer, true);
        let int = scalar(Integer, true);
        let fixed = scalar(Integer, false);
        let kind = |boolean| if boolean { Boolean } else { Integer };
        match self {
            Self::Distinct | Self::Equal | Self::NotAllEqual => vec![ints],
            Self::OffsetDistinct => vec![array(Integer, false), ints],
            Self::OptionalDistinct => vec![ints, array(Boolean, true)],
            Self::ExceptDistinct => vec![ints, fixed],
            Self::Lex { boolean, .. } | Self::NotEqual { boolean } => {
                vec![array(kind(boolean), true), array(kind(boolean), true)]
            }
            Self::Monotone { boolean, .. } => vec![array(kind(boolean), true)],
            Self::Extremum { .. } => vec![int, ints],
            Self::ArgExtremum {
                boolean,
                explicit_ties,
                ..
            } => {
                let mut args = vec![array(kind(boolean), true), fixed, int];
                if explicit_ties {
                    args.push(scalar(Boolean, false));
                }
                args
            }
            Self::Sort => vec![ints.clone(), ints],
            Self::SortPermutation => vec![ints.clone(), ints.clone(), ints, fixed],
            Self::Inverse => vec![ints.clone(), fixed.clone(), ints, fixed],
            Self::Channel => vec![array(Boolean, true), int, fixed],
            Self::Precede => vec![ints, fixed.clone(), fixed],
            Self::Member { boolean, reified } => {
                let mut args = vec![array(kind(boolean), true), scalar(kind(boolean), true)];
                if reified {
                    args.push(scalar(Boolean, true));
                }
                args
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum DistinctMode {
    Plain,
    Offsets(
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        Vec<i64>,
    ),
    Optional(Vec<Operand>),
    Except(
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        i64,
    ),
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Ordering {
    Distinct {
        entries: Vec<Operand>,
        mode: DistinctMode,
    },
    Equal {
        entries: Vec<Operand>,
        negated: bool,
    },
    Lex {
        left: Vec<Operand>,
        right: Vec<Operand>,
        boolean: bool,
        strict: bool,
    },
    NotEqual {
        left: Vec<Operand>,
        right: Vec<Operand>,
        boolean: bool,
    },
    Monotone {
        entries: Vec<Operand>,
        boolean: bool,
        decreasing: bool,
    },
    Extremum {
        entries: Vec<Operand>,
        result: Operand,
        maximum: bool,
    },
    ArgExtremum {
        entries: Vec<Operand>,
        result: Operand,
        boolean: bool,
        maximum: bool,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        offset: i64,
        break_ties: bool,
        explicit_ties: bool,
    },
    Sort {
        entries: Vec<Operand>,
        sorted: Vec<Operand>,
        permutation: Option<Vec<Operand>>,
        #[serde(
            serialize_with = "serialize_optional_integer",
            deserialize_with = "deserialize_optional_integer"
        )]
        offset: Option<i64>,
    },
    Inverse {
        entries: Vec<Operand>,
        inverse: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        offset: i64,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        inverse_offset: i64,
    },
    Channel {
        entries: Vec<Operand>,
        value: Operand,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        offset: i64,
    },
    Precede {
        entries: Vec<Operand>,
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
    Member {
        entries: Vec<Operand>,
        needle: Operand,
        boolean: bool,
        result: Option<Operand>,
    },
}

/// An irrelevant or unavailable operand has no fabricated concrete value.
#[derive(Clone, Debug, Serialize)]
pub struct OrderingArgument {
    pub value: Option<ArgumentValue>,
    pub unresolved: Option<Box<VariableRef>>,
}
#[derive(Clone, Debug, Serialize)]
pub struct OrderingPair {
    pub first: usize,
    pub second: usize,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IndexFault {
    OutOfRange {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        index: i64,
    },
    Repeated {
        first: usize,
        second: usize,
        #[serde(serialize_with = "serialize_integer")]
        index: i64,
    },
    Mismatch {
        position: usize,
        target_position: usize,
    },
    BackLink {
        position: usize,
        target_position: usize,
        #[serde(serialize_with = "serialize_integer")]
        expected: i64,
        #[serde(serialize_with = "serialize_integer")]
        actual: i64,
    },
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InverseSide {
    Forward,
    Inverse,
}
#[derive(Clone, Debug, Serialize)]
pub struct InverseFault {
    pub side: InverseSide,
    pub fault: IndexFault,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum OrderingFinding {
    Distinct {
        entries: Vec<OrderingArgument>,
        presence: Option<Vec<OrderingArgument>>,
        #[serde(serialize_with = "serialize_optional_integers")]
        offsets: Option<Vec<i64>>,
        #[serde(serialize_with = "serialize_optional_integer")]
        excluded: Option<i64>,
        conflicts: Vec<OrderingPair>,
    },
    Equal {
        entries: Vec<OrderingArgument>,
        negated: bool,
        differing: Vec<OrderingPair>,
    },
    Lex {
        left: Vec<OrderingArgument>,
        right: Vec<OrderingArgument>,
        strict: bool,
        decisive_position: Option<usize>,
        left_length: usize,
        right_length: usize,
    },
    NotEqual {
        left: Vec<OrderingArgument>,
        right: Vec<OrderingArgument>,
    },
    Monotone {
        entries: Vec<OrderingArgument>,
        decreasing: bool,
        conflicts: Vec<OrderingPair>,
    },
    Extremum {
        entries: Vec<OrderingArgument>,
        maximum: bool,
        result: OrderingArgument,
        #[serde(serialize_with = "serialize_integer")]
        calculated: i64,
    },
    ArgExtremum {
        entries: Vec<OrderingArgument>,
        maximum: bool,
        break_ties: bool,
        #[serde(serialize_with = "serialize_integer")]
        offset: i64,
        result: OrderingArgument,
        extremum: Option<Value>,
        #[serde(serialize_with = "serialize_integers")]
        permitted_indexes: Vec<i64>,
    },
    Sort {
        entries: Vec<OrderingArgument>,
        sorted: Vec<OrderingArgument>,
        permutation: Option<Vec<OrderingArgument>>,
        #[serde(serialize_with = "serialize_optional_integer")]
        offset: Option<i64>,
        mismatches: Vec<usize>,
        faults: Vec<IndexFault>,
    },
    Inverse {
        entries: Vec<OrderingArgument>,
        inverse: Vec<OrderingArgument>,
        #[serde(serialize_with = "serialize_integer")]
        offset: i64,
        #[serde(serialize_with = "serialize_integer")]
        inverse_offset: i64,
        faults: Vec<InverseFault>,
    },
    Channel {
        entries: Vec<OrderingArgument>,
        value: OrderingArgument,
        #[serde(serialize_with = "serialize_integer")]
        offset: i64,
        true_positions: Vec<usize>,
    },
    Precede {
        entries: Vec<OrderingArgument>,
        #[serde(serialize_with = "serialize_integer")]
        first: i64,
        #[serde(serialize_with = "serialize_integer")]
        second: i64,
        positions: Vec<usize>,
    },
    Member {
        entries: Vec<OrderingArgument>,
        needle: OrderingArgument,
        truth: bool,
        result: Option<OrderingArgument>,
    },
}
fn serialize_optional_integers<S: serde::Serializer>(
    values: &Option<Vec<i64>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    values
        .as_ref()
        .map(|values| values.iter().map(i64::to_string).collect::<Vec<_>>())
        .serialize(serializer)
}

impl Ordering {
    pub fn name(&self) -> OrderingName {
        match self {
            Self::Distinct { mode, .. } => match mode {
                DistinctMode::Plain => OrderingName::Distinct,
                DistinctMode::Offsets(_) => OrderingName::OffsetDistinct,
                DistinctMode::Optional(_) => OrderingName::OptionalDistinct,
                DistinctMode::Except(_) => OrderingName::ExceptDistinct,
            },
            Self::Equal { negated, .. } => {
                if *negated {
                    OrderingName::NotAllEqual
                } else {
                    OrderingName::Equal
                }
            }
            Self::Lex {
                boolean, strict, ..
            } => OrderingName::Lex {
                boolean: *boolean,
                strict: *strict,
            },
            Self::NotEqual { boolean, .. } => OrderingName::NotEqual { boolean: *boolean },
            Self::Monotone {
                boolean,
                decreasing,
                ..
            } => OrderingName::Monotone {
                boolean: *boolean,
                decreasing: *decreasing,
            },
            Self::Extremum { maximum, .. } => OrderingName::Extremum { maximum: *maximum },
            Self::ArgExtremum {
                boolean,
                maximum,
                explicit_ties,
                ..
            } => OrderingName::ArgExtremum {
                boolean: *boolean,
                maximum: *maximum,
                explicit_ties: *explicit_ties,
            },
            Self::Sort { permutation, .. } => {
                if permutation.is_some() {
                    OrderingName::SortPermutation
                } else {
                    OrderingName::Sort
                }
            }
            Self::Inverse { .. } => OrderingName::Inverse,
            Self::Channel { .. } => OrderingName::Channel,
            Self::Precede { .. } => OrderingName::Precede,
            Self::Member {
                boolean, result, ..
            } => OrderingName::Member {
                boolean: *boolean,
                reified: result.is_some(),
            },
        }
    }
    pub fn operands(&self) -> Vec<&Operand> {
        match self {
            Self::Distinct { entries, mode } => entries
                .iter()
                .chain(match mode {
                    DistinctMode::Optional(presence) => presence.iter(),
                    _ => [].iter(),
                })
                .collect(),
            Self::Equal { entries, .. }
            | Self::Monotone { entries, .. }
            | Self::Precede { entries, .. } => entries.iter().collect(),
            Self::Lex { left, right, .. } | Self::NotEqual { left, right, .. } => {
                left.iter().chain(right).collect()
            }
            Self::Extremum {
                entries, result, ..
            }
            | Self::ArgExtremum {
                entries, result, ..
            } => entries.iter().chain(std::iter::once(result)).collect(),
            Self::Sort {
                entries,
                sorted,
                permutation,
                ..
            } => entries
                .iter()
                .chain(sorted)
                .chain(permutation.iter().flatten())
                .collect(),
            Self::Inverse {
                entries, inverse, ..
            } => entries.iter().chain(inverse).collect(),
            Self::Channel { entries, value, .. } => {
                entries.iter().chain(std::iter::once(value)).collect()
            }
            Self::Member {
                entries,
                needle,
                result,
                ..
            } => entries
                .iter()
                .chain(std::iter::once(needle))
                .chain(result.iter())
                .collect(),
        }
    }
    pub fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        match self {
            Self::Distinct {
                entries,
                mode: DistinctMode::Optional(presence),
            } => {
                let active: Vec<_> = entries
                    .iter()
                    .zip(presence)
                    .filter(|(_, present)| {
                        !matches!(present.value(values), Some(Value::Boolean(false)))
                    })
                    .map(|(entry, _)| entry)
                    .collect();
                if active.len() < 2 {
                    Vec::new()
                } else {
                    presence.iter().chain(active).collect()
                }
            }
            Self::Distinct {
                entries,
                mode: DistinctMode::Plain | DistinctMode::Except(_),
            }
            | Self::Equal { entries, .. }
            | Self::Monotone { entries, .. }
                if entries.len() < 2 =>
            {
                Vec::new()
            }
            Self::NotEqual { left, right, .. } if left.len() != right.len() => Vec::new(),
            Self::Member {
                entries, result, ..
            } if entries.is_empty() => result.iter().collect(),
            Self::Channel { entries, .. } if entries.is_empty() => Vec::new(),
            Self::Channel {
                entries,
                value,
                offset,
            } => {
                if value.value(values).is_some_and(|value| {
                    position(value.integer(), *offset, entries.len()).is_none()
                }) {
                    return vec![value];
                }
                if let Some(flags) = ordered_values(entries, values)
                    && flags.iter().filter(|&&flag| flag == 1).count() != 1
                {
                    return entries.iter().collect();
                }
                self.operands()
            }
            Self::Inverse {
                entries,
                inverse,
                offset,
                inverse_offset,
            } => {
                for (side, offset) in [(entries, *offset), (inverse, *inverse_offset)] {
                    if let Some(concrete) = ordered_values(side, values)
                        && !permutation_faults(&concrete, offset).is_empty()
                    {
                        return side.iter().collect();
                    }
                }
                self.operands()
            }
            Self::ArgExtremum {
                entries,
                result,
                offset,
                ..
            } if result.value(values).is_some_and(|value| {
                position(value.integer(), *offset, entries.len()).is_none()
            }) =>
            {
                vec![result]
            }
            _ => self.operands(),
        }
    }
    pub fn validate_shape(&self) -> Result<(), &'static str> {
        match self {
            Self::Distinct {
                entries,
                mode: DistinctMode::Offsets(offsets),
            } if entries.len() != offsets.len() => {
                Err("distinct offsets and entries need equal lengths")
            }
            Self::Distinct {
                entries,
                mode: DistinctMode::Optional(presence),
            } if entries.len() != presence.len() => {
                Err("optional distinct entries and presence need equal lengths")
            }
            Self::Extremum { entries, .. } | Self::ArgExtremum { entries, .. }
                if entries.is_empty() =>
            {
                Err("extrema require a nonempty array")
            }
            Self::ArgExtremum {
                entries,
                offset,
                explicit_ties,
                break_ties,
                ..
            } => {
                if *offset < 0 {
                    return Err("arg-extremum offset must be nonnegative");
                }
                if !explicit_ties && !break_ties {
                    return Err("legacy arg-extremum requires first-tie behavior");
                }
                validate_interval(*offset, entries.len())
            }
            Self::Sort {
                entries,
                sorted,
                permutation,
                offset,
            } => {
                if entries.len() != sorted.len()
                    || permutation
                        .as_ref()
                        .is_some_and(|p| p.len() != entries.len())
                {
                    return Err("sort arrays need equal lengths");
                }
                match (permutation, offset) {
                    (Some(_), Some(offset)) => validate_interval(*offset, entries.len()),
                    (None, None) => Ok(()),
                    _ => Err("sort permutation requires its fixed offset"),
                }
            }
            Self::Inverse {
                entries,
                inverse,
                offset,
                inverse_offset,
            } => {
                if entries.len() != inverse.len() {
                    return Err("inverse arrays need equal lengths");
                }
                if *offset < 0 || *inverse_offset < 0 {
                    return Err("inverse offsets must be nonnegative");
                }
                validate_interval(*offset, entries.len())?;
                validate_interval(*inverse_offset, entries.len())
            }
            Self::Channel {
                entries, offset, ..
            } => validate_interval(*offset, entries.len()),
            _ => Ok(()),
        }
    }
    /// Each slice is one complete array output accepted by defines_var.
    pub fn array_outputs(&self) -> Vec<(&[Operand], ValueType)> {
        match self {
            Self::Sort {
                sorted,
                permutation,
                ..
            } => std::iter::once((sorted.as_slice(), ValueType::Integer))
                .chain(
                    permutation
                        .as_deref()
                        .map(|entries| (entries, ValueType::Integer)),
                )
                .collect(),
            Self::Inverse {
                entries, inverse, ..
            } => vec![(entries, ValueType::Integer), (inverse, ValueType::Integer)],
            Self::Channel { entries, .. } => vec![(entries, ValueType::Boolean)],
            _ => Vec::new(),
        }
    }
    pub fn can_define(&self, output: usize) -> bool {
        let is_output =
            |operand: &Operand| matches!(operand, Operand::Variable(slot) if *slot == output);
        match self {
            Self::Extremum {
                entries, result, ..
            }
            | Self::ArgExtremum {
                entries, result, ..
            } => is_output(result) && !entries.iter().any(is_output),
            Self::Sort {
                entries,
                sorted,
                permutation,
                ..
            } => {
                !entries.iter().any(is_output)
                    && sorted
                        .iter()
                        .chain(permutation.iter().flatten())
                        .any(is_output)
            }
            Self::Inverse {
                entries, inverse, ..
            } => entries.iter().any(is_output) != inverse.iter().any(is_output),
            Self::Channel { entries, value, .. } => {
                is_output(value) || entries.iter().any(is_output)
            }
            Self::Member {
                entries,
                needle,
                result: Some(result),
                ..
            } => {
                is_output(result)
                    && (entries.is_empty()
                        || (!entries.iter().any(is_output) && !is_output(needle)))
            }
            _ => false,
        }
    }
    pub fn derive(
        &self,
        output: usize,
        values: &[Option<Value>],
        _constraint: &Constraint,
    ) -> Result<Option<Value>, CheckError> {
        let is_output =
            |operand: &Operand| matches!(operand, Operand::Variable(slot) if *slot == output);
        Ok(match self {
            Self::Extremum {
                entries, maximum, ..
            } => ordered_values(entries, values)
                .map(|entries| Value::Integer(extremum(&entries, *maximum))),
            Self::ArgExtremum {
                entries,
                maximum,
                offset,
                break_ties,
                ..
            } => ordered_values(entries, values).and_then(|entries| {
                let indexes = extremum_indexes(&entries, *maximum, *offset, *break_ties);
                (indexes.len() == 1).then(|| Value::Integer(indexes[0]))
            }),
            Self::Sort {
                entries,
                sorted,
                permutation,
                offset,
            } => {
                let Some(entries) = ordered_values(entries, values) else {
                    return Ok(None);
                };
                let mut calculated = entries.clone();
                calculated.sort_unstable();
                let mut candidates = Vec::new();
                for (operand, &value) in sorted.iter().zip(&calculated) {
                    if is_output(operand) {
                        candidates.push(Value::Integer(value));
                    }
                }
                if let Some(permutation) = permutation
                    && permutation.iter().any(is_output)
                {
                    if calculated.windows(2).any(|pair| pair[0] == pair[1]) {
                        return Ok(None);
                    }
                    for (operand, value) in permutation.iter().zip(&entries) {
                        if is_output(operand) {
                            candidates.push(Value::Integer(index_value(
                                offset.unwrap(),
                                calculated.binary_search(value).unwrap(),
                            )));
                        }
                    }
                }
                agreed_output(candidates)
            }
            Self::Inverse {
                entries,
                inverse,
                offset,
                inverse_offset,
            } => {
                let (input, input_offset, targets, target_offset) = if entries.iter().any(is_output)
                {
                    (inverse, *inverse_offset, entries, *offset)
                } else {
                    (entries, *offset, inverse, *inverse_offset)
                };
                let Some(input) = ordered_values(input, values) else {
                    return Ok(None);
                };
                if !permutation_faults(&input, input_offset).is_empty() {
                    return Ok(None);
                }
                let mut calculated = vec![0; input.len()];
                for (i, &value) in input.iter().enumerate() {
                    calculated[position(value, input_offset, input.len()).unwrap()] =
                        index_value(target_offset, i);
                }
                agreed_output(
                    targets
                        .iter()
                        .zip(calculated)
                        .filter(|(operand, _)| is_output(operand))
                        .map(|(_, value)| Value::Integer(value))
                        .collect(),
                )
            }
            Self::Channel {
                entries,
                value,
                offset,
            } => {
                if is_output(value) {
                    let Some(flags) = ordered_values(entries, values) else {
                        return Ok(None);
                    };
                    let positions: Vec<_> = flags
                        .iter()
                        .enumerate()
                        .filter(|(_, flag)| **flag == 1)
                        .map(|(i, _)| i)
                        .collect();
                    (positions.len() == 1)
                        .then(|| Value::Integer(index_value(*offset, positions[0])))
                } else {
                    let Some(value) = value.value(values) else {
                        return Ok(None);
                    };
                    let Some(selected) = position(value.integer(), *offset, entries.len()) else {
                        return Ok(None);
                    };
                    agreed_output(
                        entries
                            .iter()
                            .enumerate()
                            .filter(|(_, operand)| is_output(operand))
                            .map(|(i, _)| Value::Boolean(i == selected))
                            .collect(),
                    )
                }
            }
            Self::Member {
                entries, needle, ..
            } => member_truth(entries, needle, values).map(Value::Boolean),
            _ => None,
        })
    }
}
fn validate_interval(offset: i64, length: usize) -> Result<(), &'static str> {
    if length > 0
        && i64::try_from(length - 1)
            .ok()
            .and_then(|last| offset.checked_add(last))
            .is_none()
    {
        Err("fixed ordering index interval overflows i64")
    } else {
        Ok(())
    }
}
fn index_value(offset: i64, position: usize) -> i64 {
    offset
        .checked_add(i64::try_from(position).expect("addressable position fits i64"))
        .expect("validated index interval")
}
fn position(index: i64, offset: i64, length: usize) -> Option<usize> {
    if length == 0 || index < offset || index > index_value(offset, length - 1) {
        return None;
    }
    usize::try_from(index.checked_sub(offset)?).ok()
}
fn ordered_values(entries: &[Operand], values: &[Option<Value>]) -> Option<Vec<i64>> {
    entries
        .iter()
        .map(|entry| entry.value(values).map(Value::membership_element))
        .collect()
}
fn agreed_output(values: Vec<Value>) -> Option<Value> {
    let first = values.first()?;
    values
        .iter()
        .all(|value| value == first)
        .then(|| first.clone())
}
fn extremum(entries: &[i64], maximum: bool) -> i64 {
    if maximum {
        *entries.iter().max().unwrap()
    } else {
        *entries.iter().min().unwrap()
    }
}
fn extremum_indexes(entries: &[i64], maximum: bool, offset: i64, break_ties: bool) -> Vec<i64> {
    let best = extremum(entries, maximum);
    let mut indexes: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| **entry == best)
        .map(|(i, _)| index_value(offset, i))
        .collect();
    if break_ties {
        indexes.truncate(1);
    }
    indexes
}
fn member_truth(entries: &[Operand], needle: &Operand, values: &[Option<Value>]) -> Option<bool> {
    if entries.is_empty() {
        return Some(false);
    }
    let needle = needle.value(values)?;
    Some(
        entries
            .iter()
            .map(|entry| entry.value(values))
            .collect::<Option<Vec<_>>>()?
            .contains(&needle),
    )
}
fn permutation_faults(entries: &[i64], offset: i64) -> Vec<IndexFault> {
    let mut faults = Vec::new();
    let mut seen = HashMap::new();
    for (i, &index) in entries.iter().enumerate() {
        if position(index, offset, entries.len()).is_none() {
            faults.push(IndexFault::OutOfRange {
                position: i + 1,
                index,
            });
        }
        if let Some(first) = seen.insert(index, i + 1) {
            faults.push(IndexFault::Repeated {
                first,
                second: i + 1,
                index,
            });
        }
    }
    faults
}

fn distinct_conflicts(
    entries: &[Operand],
    mode: &DistinctMode,
    values: &[Option<Value>],
    constraint: &Constraint,
) -> Result<Vec<OrderingPair>, CheckError> {
    if let DistinctMode::Optional(presence) = mode
        && presence
            .iter()
            .filter(|present| !matches!(present.value(values), Some(Value::Boolean(false))))
            .count()
            < 2
    {
        return Ok(Vec::new());
    }
    let mut concrete = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        if let DistinctMode::Optional(presence) = mode
            && !presence[i].value(values).unwrap().boolean()
        {
            continue;
        }
        // Vacuous relations need no value, except offset arithmetic itself must be defined.
        if entries.len() < 2 && !matches!(mode, DistinctMode::Offsets(_)) {
            continue;
        }
        let mut value = entry
            .value(values)
            .expect("required distinct entry")
            .integer();
        match mode {
            DistinctMode::Offsets(offsets) => {
                value = value.checked_add(offsets[i]).ok_or_else(|| {
                    constraint.execution_error("checked i64 overflow in distinct offset sum")
                })?
            }
            DistinctMode::Except(excluded) if value == *excluded => continue,
            _ => (),
        }
        concrete.push((i + 1, value));
    }
    let mut conflicts = Vec::new();
    for (i, &(first, value)) in concrete.iter().enumerate() {
        for &(second, other) in &concrete[i + 1..] {
            if value == other {
                conflicts.push(OrderingPair { first, second });
            }
        }
    }
    Ok(conflicts)
}

struct SortFaults {
    mismatches: Vec<usize>,
    faults: Vec<IndexFault>,
}
fn sort_faults(
    input: &[i64],
    supplied: &[i64],
    permutation: Option<&[i64]>,
    offset: Option<i64>,
) -> SortFaults {
    let mut calculated = input.to_vec();
    calculated.sort_unstable();
    let mismatches: Vec<_> = calculated
        .iter()
        .zip(supplied)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i + 1)
        .collect();
    let mut faults = Vec::new();
    if let Some(permutation) = permutation {
        let offset = offset.unwrap();
        faults = permutation_faults(permutation, offset);
        for (i, &index) in permutation.iter().enumerate() {
            if let Some(target) = position(index, offset, input.len())
                && input[i] != supplied[target]
            {
                faults.push(IndexFault::Mismatch {
                    position: i + 1,
                    target_position: target + 1,
                });
            }
        }
    }
    SortFaults { mismatches, faults }
}

fn inverse_side_faults(
    side: InverseSide,
    input: Option<&[i64]>,
    offset: i64,
    other: Option<&[i64]>,
    other_offset: i64,
) -> Vec<InverseFault> {
    let Some(input) = input else {
        return Vec::new();
    };
    let mut faults: Vec<_> = permutation_faults(input, offset)
        .into_iter()
        .map(|fault| InverseFault { side, fault })
        .collect();
    if let Some(other) = other {
        for (i, &index) in input.iter().enumerate() {
            if let Some(target) = position(index, offset, input.len()) {
                let expected = index_value(other_offset, i);
                if other[target] != expected {
                    faults.push(InverseFault {
                        side,
                        fault: IndexFault::BackLink {
                            position: i + 1,
                            target_position: target + 1,
                            expected,
                            actual: other[target],
                        },
                    });
                }
            }
        }
    }
    faults
}

impl PreparedModel {
    fn ordering_argument(&self, operand: &Operand, values: &[Option<Value>]) -> OrderingArgument {
        OrderingArgument {
            value: self.argument(operand, values),
            unresolved: match operand {
                Operand::Variable(slot) if values[*slot].is_none() => {
                    Some(Box::new(self.slots[*slot].reference.clone()))
                }
                _ => None,
            },
        }
    }
    pub(crate) fn evaluate_ordering(
        &self,
        constraint: &Constraint,
        ordering: &Ordering,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let argument = |operand: &Operand| self.ordering_argument(operand, values);
        let arguments = |entries: &[Operand]| entries.iter().map(argument).collect::<Vec<_>>();
        let number = |operand: &Operand| operand.value(values).unwrap().membership_element();
        let finding = match ordering {
            Ordering::Distinct { entries, mode } => {
                let conflicts = distinct_conflicts(entries, mode, values, constraint)?;
                if conflicts.is_empty() {
                    return Ok(None);
                }
                OrderingFinding::Distinct {
                    entries: arguments(entries),
                    presence: if let DistinctMode::Optional(presence) = mode {
                        Some(arguments(presence))
                    } else {
                        None
                    },
                    offsets: if let DistinctMode::Offsets(offsets) = mode {
                        Some(offsets.clone())
                    } else {
                        None
                    },
                    excluded: if let DistinctMode::Except(value) = mode {
                        Some(*value)
                    } else {
                        None
                    },
                    conflicts,
                }
            }
            Ordering::Equal { entries, negated } => {
                let mut differing = Vec::new();
                for i in 0..entries.len() {
                    for j in i + 1..entries.len() {
                        if number(&entries[i]) != number(&entries[j]) {
                            differing.push(OrderingPair {
                                first: i + 1,
                                second: j + 1,
                            });
                        }
                    }
                }
                if differing.is_empty() != *negated {
                    return Ok(None);
                }
                OrderingFinding::Equal {
                    entries: arguments(entries),
                    negated: *negated,
                    differing,
                }
            }
            Ordering::Lex {
                left,
                right,
                strict,
                ..
            } => {
                let left_values = ordered_values(left, values).unwrap();
                let right_values = ordered_values(right, values).unwrap();
                let comparison = left_values.cmp(&right_values);
                if comparison.is_lt() || (!strict && comparison.is_eq()) {
                    return Ok(None);
                }
                let decisive_position = left_values
                    .iter()
                    .zip(&right_values)
                    .position(|(a, b)| a != b)
                    .map(|i| i + 1);
                OrderingFinding::Lex {
                    left: arguments(left),
                    right: arguments(right),
                    strict: *strict,
                    decisive_position,
                    left_length: left.len(),
                    right_length: right.len(),
                }
            }
            Ordering::NotEqual { left, right, .. } => {
                if left.len() != right.len()
                    || ordered_values(left, values).unwrap()
                        != ordered_values(right, values).unwrap()
                {
                    return Ok(None);
                }
                OrderingFinding::NotEqual {
                    left: arguments(left),
                    right: arguments(right),
                }
            }
            Ordering::Monotone {
                entries,
                decreasing,
                ..
            } => {
                let conflicts: Vec<_> = entries
                    .windows(2)
                    .enumerate()
                    .filter(|(_, pair)| {
                        if *decreasing {
                            number(&pair[0]) < number(&pair[1])
                        } else {
                            number(&pair[0]) > number(&pair[1])
                        }
                    })
                    .map(|(i, _)| OrderingPair {
                        first: i + 1,
                        second: i + 2,
                    })
                    .collect();
                if conflicts.is_empty() {
                    return Ok(None);
                }
                OrderingFinding::Monotone {
                    entries: arguments(entries),
                    decreasing: *decreasing,
                    conflicts,
                }
            }
            Ordering::Extremum {
                entries,
                result,
                maximum,
            } => {
                let calculated = extremum(&ordered_values(entries, values).unwrap(), *maximum);
                if number(result) == calculated {
                    return Ok(None);
                }
                OrderingFinding::Extremum {
                    entries: arguments(entries),
                    maximum: *maximum,
                    result: argument(result),
                    calculated,
                }
            }
            Ordering::ArgExtremum {
                entries,
                result,
                maximum,
                offset,
                break_ties,
                boolean,
                ..
            } => {
                let concrete = ordered_values(entries, values);
                let permitted_indexes = concrete
                    .as_ref()
                    .map(|entries| extremum_indexes(entries, *maximum, *offset, *break_ties))
                    .unwrap_or_default();
                if permitted_indexes.contains(&number(result)) {
                    return Ok(None);
                }
                let best = concrete
                    .map(|entries| extremum(&entries, *maximum))
                    .map(|value| {
                        if *boolean {
                            Value::Boolean(value != 0)
                        } else {
                            Value::Integer(value)
                        }
                    });
                OrderingFinding::ArgExtremum {
                    entries: arguments(entries),
                    maximum: *maximum,
                    break_ties: *break_ties,
                    offset: *offset,
                    result: argument(result),
                    extremum: best,
                    permitted_indexes,
                }
            }
            Ordering::Sort {
                entries,
                sorted,
                permutation,
                offset,
            } => {
                let input = ordered_values(entries, values).unwrap();
                let supplied = ordered_values(sorted, values).unwrap();
                let permutation_values = permutation
                    .as_ref()
                    .map(|entries| ordered_values(entries, values).expect("required permutation"));
                let SortFaults { mismatches, faults } =
                    sort_faults(&input, &supplied, permutation_values.as_deref(), *offset);
                if mismatches.is_empty() && faults.is_empty() {
                    return Ok(None);
                }
                OrderingFinding::Sort {
                    entries: arguments(entries),
                    sorted: arguments(sorted),
                    permutation: permutation.as_ref().map(|p| arguments(p)),
                    offset: *offset,
                    mismatches,
                    faults,
                }
            }
            Ordering::Inverse {
                entries,
                inverse,
                offset,
                inverse_offset,
            } => {
                let forward = ordered_values(entries, values);
                let backward = ordered_values(inverse, values);
                let mut faults = inverse_side_faults(
                    InverseSide::Forward,
                    forward.as_deref(),
                    *offset,
                    backward.as_deref(),
                    *inverse_offset,
                );
                faults.extend(inverse_side_faults(
                    InverseSide::Inverse,
                    backward.as_deref(),
                    *inverse_offset,
                    forward.as_deref(),
                    *offset,
                ));
                if faults.is_empty() {
                    return Ok(None);
                }
                OrderingFinding::Inverse {
                    entries: arguments(entries),
                    inverse: arguments(inverse),
                    offset: *offset,
                    inverse_offset: *inverse_offset,
                    faults,
                }
            }
            Ordering::Channel {
                entries,
                value,
                offset,
            } => {
                let true_positions: Vec<_> = entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| matches!(entry.value(values), Some(Value::Boolean(true))))
                    .map(|(i, _)| i + 1)
                    .collect();
                if true_positions.len() == 1
                    && value.value(values).is_some_and(|value| {
                        value.integer() == index_value(*offset, true_positions[0] - 1)
                    })
                {
                    return Ok(None);
                }
                OrderingFinding::Channel {
                    entries: arguments(entries),
                    value: argument(value),
                    offset: *offset,
                    true_positions,
                }
            }
            Ordering::Precede {
                entries,
                first,
                second,
            } => {
                let mut seen = false;
                let mut positions = Vec::new();
                for (i, entry) in entries.iter().enumerate() {
                    let value = number(entry);
                    if value == *second && (!seen || first == second) {
                        positions.push(i + 1);
                    }
                    if value == *first {
                        seen = true;
                    }
                }
                if positions.is_empty() {
                    return Ok(None);
                }
                OrderingFinding::Precede {
                    entries: arguments(entries),
                    first: *first,
                    second: *second,
                    positions,
                }
            }
            Ordering::Member {
                entries,
                needle,
                result,
                ..
            } => {
                let truth = member_truth(entries, needle, values).unwrap();
                if result.as_ref().map_or(truth, |result| {
                    result.value(values).unwrap().boolean() == truth
                }) {
                    return Ok(None);
                }
                OrderingFinding::Member {
                    entries: arguments(entries),
                    needle: argument(needle),
                    truth,
                    result: result.as_ref().map(argument),
                }
            }
        };
        Ok(Some(ViolationKind::Ordering { finding }))
    }
}
