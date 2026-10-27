//! Pair separation and matrix minimum distances over zero-based site labels.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DistanceName {
    Inter,
    Minimum,
    Required,
}
impl DistanceName {
    pub fn from_name(name: &str) -> Option<Self> {
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            return match base {
                "gecode_inter_distance" | "gecode_inter_distance_fzn" => Some(Self::Inter),
                "gecode_minimum_distance_fzn" => Some(Self::Minimum),
                "gecode_minimum_distance_required_fzn" => Some(Self::Required),
                _ => continue,
            };
        }
        None
    }
    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<prepare::ArgumentType> {
        let argument = |is_array, is_var| prepare::ArgumentType {
            kind: ValueType::Integer,
            is_array,
            is_var,
        };
        match self {
            Self::Inter => vec![argument(true, true), argument(false, true)],
            Self::Minimum => vec![
                argument(true, true),
                argument(false, false),
                argument(true, false),
                argument(false, true),
            ],
            Self::Required => vec![
                argument(true, true),
                argument(false, false),
                argument(true, false),
                argument(false, true),
                argument(true, false),
            ],
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Distance {
    Inter {
        positions: Vec<Operand>,
        distance: Operand,
    },
    Minimum {
        sites: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        size: i64,
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        distances: Vec<i64>,
        result: Operand,
        #[serde(
            serialize_with = "serialize_optional_integers",
            deserialize_with = "deserialize_optional_integers"
        )]
        requirements: Option<Vec<i64>>,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct DistanceArgument {
    pub position: Option<usize>,
    pub value: Option<ArgumentValue>,
    pub unresolved: Option<Box<VariableRef>>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DistanceFault {
    NegativeSeparation {
        #[serde(serialize_with = "serialize_integer")]
        distance: i64,
    },
    PairSeparation {
        first: usize,
        second: usize,
        #[serde(serialize_with = "serialize_integer")]
        actual: i64,
        #[serde(serialize_with = "serialize_integer")]
        required: i64,
    },
    SiteOutOfRange {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        site: i64,
        #[serde(serialize_with = "serialize_integer")]
        size: i64,
    },
    Minimum {
        #[serde(serialize_with = "serialize_integer")]
        expected: i64,
        #[serde(serialize_with = "serialize_integer")]
        actual: i64,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DistanceFinding {
    Inter {
        positions: Vec<DistanceArgument>,
        distance: DistanceArgument,
        faults: Vec<DistanceFault>,
    },
    Minimum {
        sites: Vec<DistanceArgument>,
        #[serde(serialize_with = "serialize_integer")]
        size: i64,
        result: DistanceArgument,
        #[serde(serialize_with = "serialize_optional_integer")]
        expected: Option<i64>,
        faults: Vec<DistanceFault>,
    },
}

// Optional fixed matrices use the same decimal-string transport as other integer arrays.
fn serialize_optional_integers<S: serde::Serializer>(
    values: &Option<Vec<i64>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    values
        .as_ref()
        .map(|values| values.iter().map(ToString::to_string).collect::<Vec<_>>())
        .serialize(serializer)
}
fn deserialize_optional_integers<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<i64>>, D::Error> {
    Option::<Vec<String>>::deserialize(deserializer)?
        .map(|values| {
            values
                .into_iter()
                .map(|value| value.parse().map_err(serde::de::Error::custom))
                .collect()
        })
        .transpose()
}

impl Distance {
    pub fn name(&self) -> DistanceName {
        match self {
            Self::Inter { .. } => DistanceName::Inter,
            Self::Minimum {
                requirements: None, ..
            } => DistanceName::Minimum,
            Self::Minimum {
                requirements: Some(_),
                ..
            } => DistanceName::Required,
        }
    }
    fn positions(&self) -> &[Operand] {
        match self {
            Self::Inter { positions, .. } => positions,
            Self::Minimum { sites, .. } => sites,
        }
    }
    pub fn operands(&self) -> Vec<&Operand> {
        self.positions()
            .iter()
            .chain(std::iter::once(match self {
                Self::Inter { distance, .. } => distance,
                Self::Minimum { result, .. } => result,
            }))
            .collect()
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if let Self::Minimum {
            sites,
            size,
            distances,
            requirements,
            ..
        } = self
        {
            if sites.len() < 2 {
                return Err("minimum distance requires at least two positions");
            }
            if sites
                .len()
                .checked_mul(sites.len() - 1)
                .map(|count| count / 2)
                .is_none_or(|count| count > 2_147_483_646)
            {
                return Err("minimum distance pair count exceeds Gecode's integer limit");
            }
            if !(1..=2_147_483_646).contains(size) {
                return Err("distance site count must be in 1..2147483646");
            }
            let size =
                usize::try_from(*size).map_err(|_| "distance site count exceeds platform size")?;
            validate_matrix(distances, size)?;
            if let Some(requirements) = requirements {
                validate_matrix(requirements, sites.len())?;
            }
        }
        Ok(())
    }
    pub fn can_define(&self, output: usize) -> bool {
        matches!(self, Self::Minimum { result: Operand::Variable(slot), .. } if *slot == output)
            && !self
                .positions()
                .iter()
                .any(|position| matches!(position, Operand::Variable(slot) if *slot == output))
    }
    pub fn derive(&self, values: &[Option<Value>]) -> Option<Value> {
        self.minimum(values).map(Value::Integer)
    }
    fn minimum(&self, values: &[Option<Value>]) -> Option<i64> {
        let Self::Minimum {
            sites,
            size,
            distances,
            ..
        } = self
        else {
            return None;
        };
        let labels = sites
            .iter()
            .map(|site| {
                site.value(values)
                    .map(Value::integer)
                    .filter(|site| (0..*size).contains(site))
            })
            .collect::<Option<Vec<_>>>()?;
        let mut minimum = None;
        for (first, &a) in labels.iter().enumerate() {
            for &b in labels.iter().skip(first + 1) {
                let distance = distances[a as usize * *size as usize + b as usize];
                minimum = Some(minimum.map_or(distance, |previous: i64| previous.min(distance)));
            }
        }
        minimum
    }
    fn faults(&self, values: &[Option<Value>]) -> Result<Vec<DistanceFault>, &'static str> {
        let mut faults = Vec::new();
        match self {
            Self::Inter {
                positions,
                distance,
            } => {
                let Some(distance) = distance.value(values).map(Value::integer) else {
                    return Ok(faults);
                };
                if distance < 0 {
                    faults.push(DistanceFault::NegativeSeparation { distance });
                    return Ok(faults);
                }
                for (first, a) in positions.iter().enumerate() {
                    for (second, b) in positions.iter().enumerate().skip(first + 1) {
                        if let (Some(a), Some(b)) = (a.value(values), b.value(values)) {
                            let actual = i64::try_from(a.integer().abs_diff(b.integer()))
                                .map_err(|_| "checked i64 overflow in inter-distance separation")?;
                            if actual < distance {
                                faults.push(DistanceFault::PairSeparation {
                                    first: first + 1,
                                    second: second + 1,
                                    actual,
                                    required: distance,
                                });
                            }
                        }
                    }
                }
            }
            Self::Minimum {
                sites,
                size,
                distances,
                result,
                requirements,
            } => {
                let labels: Vec<_> = sites
                    .iter()
                    .enumerate()
                    .map(|(position, site)| {
                        site.value(values).and_then(|value| {
                            let site = value.integer();
                            if !(0..*size).contains(&site) {
                                faults.push(DistanceFault::SiteOutOfRange {
                                    position: position + 1,
                                    site,
                                    size: *size,
                                });
                                None
                            } else {
                                Some(site as usize)
                            }
                        })
                    })
                    .collect();
                if let Some(requirements) = requirements {
                    for (first, a) in labels.iter().enumerate() {
                        for (second, b) in labels.iter().enumerate().skip(first + 1) {
                            if let (Some(a), Some(b)) = (a, b) {
                                let actual = distances[a * *size as usize + b];
                                let required = requirements[first * sites.len() + second];
                                if actual < required {
                                    faults.push(DistanceFault::PairSeparation {
                                        first: first + 1,
                                        second: second + 1,
                                        actual,
                                        required,
                                    });
                                }
                            }
                        }
                    }
                }
                if let (Some(expected), Some(actual)) = (self.minimum(values), result.value(values))
                    && expected != actual.integer()
                {
                    faults.push(DistanceFault::Minimum {
                        expected,
                        actual: actual.integer(),
                    });
                }
            }
        }
        Ok(faults)
    }
    pub fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        if self
            .faults(values)
            .map_or(true, |faults| !faults.is_empty())
        {
            Vec::new()
        } else {
            self.operands()
        }
    }
}
fn validate_matrix(matrix: &[i64], size: usize) -> Result<(), &'static str> {
    if size.checked_mul(size) != Some(matrix.len()) {
        return Err("distance matrix requires size*size row-major entries");
    }
    for first in 0..size {
        for second in 0..size {
            let value = matrix[first * size + second];
            if !(0..=2_147_483_646).contains(&value) {
                return Err("distance matrix entries must be in 0..2147483646");
            }
            if first == second && value != 0 {
                return Err("distance matrix diagonal must be zero");
            }
            if value != matrix[second * size + first] {
                return Err("distance matrix must be symmetric");
            }
        }
    }
    Ok(())
}
impl PreparedModel {
    pub(crate) fn evaluate_distance(
        &self,
        constraint: &Constraint,
        distance: &Distance,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let faults = distance
            .faults(values)
            .map_err(|message| constraint.execution_error(message))?;
        if faults.is_empty() {
            return Ok(None);
        }
        let argument = |position, operand: &Operand| DistanceArgument {
            position,
            value: self.argument(operand, values),
            unresolved: match operand {
                Operand::Variable(slot) if values[*slot].is_none() => {
                    Some(Box::new(self.slots[*slot].reference.clone()))
                }
                _ => None,
            },
        };
        let arguments = distance
            .positions()
            .iter()
            .enumerate()
            .map(|(position, operand)| argument(Some(position + 1), operand))
            .collect();
        let finding = match distance {
            Distance::Inter { distance, .. } => DistanceFinding::Inter {
                positions: arguments,
                distance: argument(None, distance),
                faults,
            },
            Distance::Minimum { size, result, .. } => DistanceFinding::Minimum {
                sites: arguments,
                size: *size,
                result: argument(None, result),
                expected: distance.minimum(values),
                faults,
            },
        };
        Ok(Some(ViolationKind::Distance { finding }))
    }
}
