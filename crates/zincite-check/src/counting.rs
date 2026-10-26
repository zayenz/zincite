//! Exact counting signatures, aggregates and findings.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CountingName {
    Count(RelationMode),
    AtLeast,
    AtMost,
    Among,
    Nvalue,
    Cardinality { closed: bool, bounded: bool },
    Window { boolean: bool },
    Coded { matches: bool },
}

impl CountingName {
    pub fn from_name(name: &str) -> Option<Self> {
        // Registry::add prefixes the whole literal, including an existing gecode_ prefix.
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            let found = match base {
                "count" | "count_eq" => Self::Count(RelationMode::Plain),
                "count_reif" | "count_eq_reif" => Self::Count(RelationMode::Reified),
                "count_imp" | "count_eq_imp" => Self::Count(RelationMode::Implied),
                "at_least_int" => Self::AtLeast,
                "at_most_int" => Self::AtMost,
                "among" => Self::Among,
                "nvalue" => Self::Nvalue,
                "gecode_global_cardinality" => Self::Cardinality {
                    closed: false,
                    bounded: false,
                },
                "gecode_global_cardinality_closed" => Self::Cardinality {
                    closed: true,
                    bounded: false,
                },
                "global_cardinality_low_up" => Self::Cardinality {
                    closed: false,
                    bounded: true,
                },
                "global_cardinality_low_up_closed" => Self::Cardinality {
                    closed: true,
                    bounded: true,
                },
                "gecode_among_seq_int" => Self::Window { boolean: false },
                "gecode_among_seq_bool" => Self::Window { boolean: true },
                "gecode_count_int_fzn" => Self::Coded { matches: false },
                "gecode_count_matches_fzn" => Self::Coded { matches: true },
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
        let int = scalar(Integer, true);
        let fixed = scalar(Integer, false);
        let entries = array(Integer, true);
        match self {
            Self::Count(mode) => {
                let mut args = vec![entries, int.clone(), int];
                if mode != RelationMode::Plain {
                    args.push(scalar(Boolean, true));
                }
                args
            }
            Self::AtLeast | Self::AtMost => vec![fixed.clone(), entries, fixed],
            Self::Among => vec![int, entries, scalar(IntegerSet, false)],
            Self::Nvalue => vec![int, entries],
            Self::Cardinality { bounded, .. } => {
                let mut args = vec![entries, array(Integer, false), array(Integer, !bounded)];
                if bounded {
                    args.push(array(Integer, false));
                }
                args
            }
            Self::Window { boolean } => vec![
                array(if boolean { Boolean } else { Integer }, true),
                scalar(if boolean { Boolean } else { IntegerSet }, false),
                fixed.clone(),
                fixed.clone(),
                fixed,
            ],
            Self::Coded { matches } => vec![
                entries,
                if matches {
                    array(Integer, false)
                } else {
                    int.clone()
                },
                fixed,
                int,
            ],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum CountSelector {
    Needle(Operand),
    Matches(
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        Vec<i64>,
    ),
    Members(IntegerSet),
    Distinct,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum CardinalityCounts {
    Exact(Vec<Operand>),
    Bounds {
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        lower: Vec<i64>,
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        upper: Vec<i64>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Counting {
    Count {
        entries: Vec<Operand>,
        selector: CountSelector,
        count: Operand,
        relation: IntegerRelation,
        condition: RelationCondition,
        #[serde(
            serialize_with = "serialize_optional_integer",
            deserialize_with = "deserialize_optional_integer"
        )]
        code: Option<i64>,
    },
    Cardinality {
        entries: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        cover: Vec<i64>,
        counts: CardinalityCounts,
        closed: bool,
    },
    Window {
        entries: Vec<Operand>,
        membership: Value,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        length: i64,
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
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CountSelectorValue {
    Needle {
        value: Option<ArgumentValue>,
        unresolved: Option<Box<VariableRef>>,
        irrelevant: bool,
    },
    Matches {
        #[serde(serialize_with = "serialize_integers")]
        values: Vec<i64>,
    },
    Members {
        set: IntegerSet,
    },
    Distinct,
}

#[derive(Clone, Debug, Serialize)]
pub struct CountPosition {
    pub position: usize,
    #[serde(serialize_with = "serialize_integer")]
    pub cover: i64,
    #[serde(serialize_with = "serialize_integer")]
    pub calculated: i64,
    pub supplied: Option<ArgumentValue>,
    #[serde(serialize_with = "serialize_optional_integer")]
    pub lower: Option<i64>,
    #[serde(serialize_with = "serialize_optional_integer")]
    pub upper: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WindowCount {
    pub position: usize,
    #[serde(serialize_with = "serialize_integer")]
    pub calculated: i64,
}

impl Counting {
    pub fn operands(&self, required: bool) -> Vec<&Operand> {
        match self {
            Self::Count {
                entries,
                selector,
                count,
                condition,
                ..
            } => entries
                .iter()
                .chain(match selector {
                    CountSelector::Needle(needle) if !required || !entries.is_empty() => {
                        Some(needle)
                    }
                    _ => None,
                })
                .chain(std::iter::once(count))
                .chain(condition.operand())
                .collect(),
            Self::Cardinality {
                entries, counts, ..
            } => entries
                .iter()
                .chain(match counts {
                    CardinalityCounts::Exact(counts) => counts.iter(),
                    _ => [].iter(),
                })
                .collect(),
            Self::Window { entries, .. } => entries.iter().collect(),
        }
    }

    pub fn validate_shape(&self) -> Result<(), &'static str> {
        match self {
            Self::Count {
                entries,
                selector: CountSelector::Matches(matches),
                ..
            } if entries.len() != matches.len() => {
                Err("positional count arrays must have equal lengths")
            }
            Self::Cardinality { cover, counts, .. } => {
                let valid = match counts {
                    CardinalityCounts::Exact(counts) => counts.len() == cover.len(),
                    CardinalityCounts::Bounds { lower, upper } => {
                        lower.len() == cover.len() && upper.len() == cover.len()
                    }
                };
                if valid {
                    Ok(())
                } else {
                    Err("cardinality cover and count arrays must have equal lengths")
                }
            }
            Self::Window {
                entries, length, ..
            } if entries.is_empty()
                || *length < 1
                || usize::try_from(*length)
                    .ok()
                    .is_none_or(|q| q > entries.len()) =>
            {
                Err("count windows require nonempty entries and length in 1..array length")
            }
            _ => Ok(()),
        }
    }

    fn calculate(&self, values: &[Option<Value>]) -> Option<i64> {
        let Self::Count {
            entries, selector, ..
        } = self
        else {
            unreachable!()
        };
        let concrete = integers(entries, values)?;
        let count = match selector {
            CountSelector::Needle(needle) => {
                if concrete.is_empty() {
                    0
                } else {
                    let needle = needle.value(values)?.integer();
                    concrete.iter().filter(|&&value| value == needle).count()
                }
            }
            CountSelector::Matches(matches) => {
                concrete.iter().zip(matches).filter(|(a, b)| a == b).count()
            }
            CountSelector::Members(set) => concrete
                .iter()
                .filter(|&&value| set.contains(value))
                .count(),
            CountSelector::Distinct => concrete.iter().collect::<HashSet<_>>().len(),
        };
        // Vectors cannot have more than isize::MAX entries on the supported targets.
        Some(i64::try_from(count).expect("addressable aggregate fits i64"))
    }

    pub fn can_define(&self, output: usize) -> bool {
        let is_output =
            |operand: &Operand| matches!(operand, Operand::Variable(slot) if *slot == output);
        match self {
            Self::Count {
                entries,
                selector,
                count,
                relation,
                condition,
                ..
            } => {
                let input = entries.iter().any(is_output)
                    || matches!(selector, CountSelector::Needle(needle) if is_output(needle) && !entries.is_empty());
                if input {
                    return false;
                }
                match condition {
                    RelationCondition::Plain => {
                        *relation == IntegerRelation::Equal && is_output(count)
                    }
                    RelationCondition::Reified { result } => is_output(result) && !is_output(count),
                    _ => false,
                }
            }
            Self::Cardinality {
                entries,
                counts: CardinalityCounts::Exact(counts),
                ..
            } => counts.iter().any(is_output) && !entries.iter().any(is_output),
            _ => false,
        }
    }

    pub fn derive(
        &self,
        output: usize,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<Value>, CheckError> {
        match self {
            Self::Count {
                count,
                relation,
                condition,
                ..
            } => {
                let Some(calculated) = self.calculate(values) else {
                    return Ok(None);
                };
                Ok(match condition {
                    RelationCondition::Plain => Some(Value::Integer(calculated)),
                    RelationCondition::Reified { .. } => count
                        .value(values)
                        .map(|count| Value::Boolean(relation.holds(calculated, count.integer()))),
                    _ => None,
                })
            }
            Self::Cardinality {
                entries,
                cover,
                counts: CardinalityCounts::Exact(counts),
                ..
            } => {
                let Some(entries) = integers(entries, values) else {
                    return Ok(None);
                };
                let mut result = None;
                for (&cover, count) in cover.iter().zip(counts) {
                    if matches!(count, Operand::Variable(slot) if *slot == output) {
                        let count = occurrence(&entries, cover)
                            .map_err(|message| constraint.execution_error(message))?;
                        if result.is_some_and(|prior| prior != count) {
                            return Ok(None);
                        }
                        result = Some(count);
                    }
                }
                Ok(result.map(Value::Integer))
            }
            _ => Ok(None),
        }
    }
}

fn integers(entries: &[Operand], values: &[Option<Value>]) -> Option<Vec<i64>> {
    entries
        .iter()
        .map(|entry| entry.value(values).map(Value::integer))
        .collect()
}
fn occurrence(entries: &[i64], value: i64) -> Result<i64, &'static str> {
    i64::try_from(entries.iter().filter(|&&entry| entry == value).count())
        .map_err(|_| "checked i64 overflow in counting aggregate")
}

impl PreparedModel {
    pub(crate) fn evaluate_counting(
        &self,
        constraint: &Constraint,
        counting: &Counting,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let argument = |operand| {
            self.argument(operand, values)
                .expect("available counting operand")
        };
        Ok(match counting {
            Counting::Count {
                entries,
                selector,
                count,
                relation,
                condition,
                code,
            } => {
                let calculated = counting
                    .calculate(values)
                    .expect("available counting inputs");
                if condition.satisfied(
                    relation.holds(calculated, count.value(values).unwrap().integer()),
                    values,
                ) {
                    return Ok(None);
                }
                let selector = match selector {
                    CountSelector::Needle(needle) => CountSelectorValue::Needle {
                        value: self.argument(needle, values),
                        unresolved: match needle {
                            Operand::Variable(slot) if values[*slot].is_none() => {
                                Some(Box::new(self.slots[*slot].reference.clone()))
                            }
                            _ => None,
                        },
                        irrelevant: entries.is_empty(),
                    },
                    CountSelector::Matches(values) => CountSelectorValue::Matches {
                        values: values.clone(),
                    },
                    CountSelector::Members(set) => CountSelectorValue::Members { set: set.clone() },
                    CountSelector::Distinct => CountSelectorValue::Distinct,
                };
                Some(ViolationKind::Count {
                    entries: entries.iter().map(argument).collect(),
                    selector,
                    calculated,
                    supplied: argument(count),
                    relation: *relation,
                    code: *code,
                    condition: self.condition_value(condition, values),
                })
            }
            Counting::Cardinality {
                entries,
                cover,
                counts,
                closed,
            } => {
                let concrete = integers(entries, values).expect("available cardinality inputs");
                let mut failures = Vec::new();
                for (position, &value) in cover.iter().enumerate() {
                    let calculated = occurrence(&concrete, value)
                        .map_err(|message| constraint.execution_error(message))?;
                    let (supplied, lower, upper, valid) = match counts {
                        CardinalityCounts::Exact(counts) => {
                            let count = argument(&counts[position]);
                            let valid = count.value.integer() == calculated;
                            (Some(count), None, None, valid)
                        }
                        CardinalityCounts::Bounds { lower, upper } => (
                            None,
                            Some(lower[position]),
                            Some(upper[position]),
                            lower[position] <= calculated && calculated <= upper[position],
                        ),
                    };
                    if !valid {
                        failures.push(CountPosition {
                            position: position + 1,
                            cover: value,
                            calculated,
                            supplied,
                            lower,
                            upper,
                        });
                    }
                }
                let outside_cover: Vec<_> = concrete
                    .iter()
                    .enumerate()
                    .filter_map(|(index, value)| {
                        (*closed && !cover.contains(value)).then_some(index + 1)
                    })
                    .collect();
                (!failures.is_empty() || !outside_cover.is_empty()).then(|| {
                    ViolationKind::Cardinality {
                        entries: entries.iter().map(argument).collect(),
                        counts: failures,
                        outside_cover,
                        closed: *closed,
                    }
                })
            }
            Counting::Window {
                entries,
                membership,
                length,
                lower,
                upper,
            } => {
                let matches: Vec<_> = entries
                    .iter()
                    .map(|entry| match membership {
                        Value::IntegerSet(set) => {
                            set.contains(entry.value(values).unwrap().integer())
                        }
                        Value::Boolean(member) => entry.value(values).unwrap().boolean() == *member,
                        _ => unreachable!("admitted window membership"),
                    })
                    .collect();
                let mut windows = Vec::new();
                for (position, window) in matches.windows(*length as usize).enumerate() {
                    let calculated = i64::try_from(window.iter().filter(|&&member| member).count())
                        .map_err(|_| {
                            constraint.execution_error("checked i64 overflow in counting aggregate")
                        })?;
                    if calculated < *lower || calculated > *upper {
                        windows.push(WindowCount {
                            position: position + 1,
                            calculated,
                        });
                    }
                }
                (!windows.is_empty()).then(|| ViolationKind::CountWindows {
                    entries: entries.iter().map(argument).collect(),
                    membership: membership.clone(),
                    length: *length,
                    lower: *lower,
                    upper: *upper,
                    windows,
                })
            }
        })
    }
}
