//! Compact finite integer sets. Canonical ranges never overlap or touch.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

use super::RelationMode;
#[cfg(feature = "prepare")]
use super::{ValueType, prepare::ArgumentType};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegerRange {
    #[serde(
        serialize_with = "super::serialize_integer",
        deserialize_with = "super::deserialize_integer"
    )]
    lower: i64,
    #[serde(
        serialize_with = "super::serialize_integer",
        deserialize_with = "super::deserialize_integer"
    )]
    upper: i64,
}

/// A finite set of i64 values stored as sorted, disjoint inclusive ranges.
/// JSON endpoints are decimal strings; loading rejects noncanonical ranges.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct IntegerSet {
    ranges: Vec<IntegerRange>,
}

impl IntegerSet {
    pub fn empty() -> Self {
        Self::default()
    }

    /// A reversed interval denotes the empty set, as in MiniZinc.
    pub fn interval(lower: i64, upper: i64) -> Self {
        if lower > upper {
            Self::empty()
        } else {
            Self {
                ranges: vec![IntegerRange { lower, upper }],
            }
        }
    }

    pub fn from_values(values: impl IntoIterator<Item = i64>) -> Self {
        Self::normalize(
            values
                .into_iter()
                .map(|value| IntegerRange {
                    lower: value,
                    upper: value,
                })
                .collect(),
        )
    }

    fn normalize(mut ranges: Vec<IntegerRange>) -> Self {
        ranges.sort_unstable_by_key(|range| range.lower);
        let mut merged: Vec<IntegerRange> = Vec::new();
        for range in ranges {
            if let Some(previous) = merged.last_mut()
                && i128::from(range.lower) <= i128::from(previous.upper) + 1
            {
                previous.upper = previous.upper.max(range.upper);
            } else {
                merged.push(range);
            }
        }
        Self { ranges: merged }
    }

    pub(crate) fn contiguous_bounds(&self) -> Option<(i64, i64)> {
        match self.ranges.as_slice() {
            [range] => Some((range.lower, range.upper)),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn contains(&self, value: i64) -> bool {
        let index = self.ranges.partition_point(|range| range.upper < value);
        self.ranges
            .get(index)
            .is_some_and(|range| range.lower <= value)
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        let mut index = 0;
        for range in &self.ranges {
            while other
                .ranges
                .get(index)
                .is_some_and(|candidate| candidate.upper < range.lower)
            {
                index += 1;
            }
            if !other.ranges.get(index).is_some_and(|candidate| {
                candidate.lower <= range.lower && range.upper <= candidate.upper
            }) {
                return false;
            }
        }
        true
    }

    pub fn union(&self, other: &Self) -> Self {
        Self::normalize(self.ranges.iter().chain(&other.ranges).cloned().collect())
    }

    pub fn intersection(&self, other: &Self) -> Self {
        let mut ranges = Vec::new();
        let (mut left, mut right) = (0, 0);
        while let (Some(a), Some(b)) = (self.ranges.get(left), other.ranges.get(right)) {
            let (lower, upper) = (a.lower.max(b.lower), a.upper.min(b.upper));
            if lower <= upper {
                ranges.push(IntegerRange { lower, upper });
            }
            if a.upper < b.upper {
                left += 1;
            } else {
                right += 1;
            }
        }
        Self { ranges }
    }

    pub fn difference(&self, other: &Self) -> Self {
        let mut ranges = Vec::new();
        let mut right = 0;
        for range in &self.ranges {
            let mut lower = i128::from(range.lower);
            let upper = i128::from(range.upper);
            while other
                .ranges
                .get(right)
                .is_some_and(|r| i128::from(r.upper) < lower)
            {
                right += 1;
            }
            let mut index = right;
            while let Some(excluded) = other.ranges.get(index) {
                if i128::from(excluded.lower) > upper {
                    break;
                }
                if lower < i128::from(excluded.lower) {
                    ranges.push(IntegerRange {
                        lower: lower as i64,
                        upper: excluded.lower - 1,
                    });
                }
                lower = lower.max(i128::from(excluded.upper) + 1);
                if lower > upper {
                    break;
                }
                index += 1;
            }
            right = index;
            if lower <= upper {
                ranges.push(IntegerRange {
                    lower: lower as i64,
                    upper: range.upper,
                });
            }
        }
        Self { ranges }
    }

    pub fn symmetric_difference(&self, other: &Self) -> Self {
        self.difference(other).union(&other.difference(self))
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.intersection(other).is_empty()
    }

    /// Compare the sorted element sequences, skipping each shared interval.
    pub fn lex_cmp(&self, other: &Self) -> Ordering {
        let (mut left, mut right) = (self.ranges.iter(), other.ranges.iter());
        let (mut a, mut b) = (left.next().cloned(), right.next().cloned());
        loop {
            match (&mut a, &mut b) {
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(x), Some(y)) => {
                    if x.lower != y.lower {
                        return x.lower.cmp(&y.lower);
                    }
                    match x.upper.cmp(&y.upper) {
                        Ordering::Equal => {
                            a = left.next().cloned();
                            b = right.next().cloned();
                        }
                        Ordering::Less => {
                            y.lower = x.upper + 1;
                            a = left.next().cloned();
                        }
                        Ordering::Greater => {
                            x.lower = y.upper + 1;
                            b = right.next().cloned();
                        }
                    }
                }
            }
        }
    }

    /// Gecode compares membership bits from the smallest differing element: 0 < 1.
    pub fn characteristic_cmp(&self, other: &Self) -> Ordering {
        let difference = self.symmetric_difference(other);
        match difference.ranges.first() {
            None => Ordering::Equal,
            Some(range) if self.contains(range.lower) => Ordering::Greater,
            Some(_) => Ordering::Less,
        }
    }

    /// None means the mathematical cardinality exceeds i64::MAX.
    pub fn cardinality(&self) -> Option<i64> {
        let size: i128 = self
            .ranges
            .iter()
            .map(|range| i128::from(range.upper) - i128::from(range.lower) + 1)
            .sum();
        i64::try_from(size).ok()
    }
}

impl<'de> Deserialize<'de> for IntegerSet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct SetFile {
            ranges: Vec<IntegerRange>,
        }
        let set = SetFile::deserialize(deserializer)?;
        if set.ranges.iter().any(|range| range.lower > range.upper)
            || set
                .ranges
                .windows(2)
                .any(|pair| i128::from(pair[0].upper) + 1 >= i128::from(pair[1].lower))
        {
            return Err(serde::de::Error::custom(
                "integer set ranges must be sorted, nonempty, disjoint and nonadjacent",
            ));
        }
        Ok(Self { ranges: set.ranges })
    }
}

impl std::fmt::Display for IntegerSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("{")?;
        for (index, range) in self.ranges.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            if range.lower == range.upper {
                write!(f, "{}", range.lower)?;
            } else {
                write!(f, "{}..{}", range.lower, range.upper)?;
            }
        }
        f.write_str("}")
    }
}

/// Named MiniZinc order uses sorted elements; native codes use membership bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetRelation {
    Equal,
    NotEqual,
    Subset,
    Superset,
    LessEqual,
    Less,
    Disjoint,
    Complement,
    CharacteristicLessEqual,
    CharacteristicLess,
    CharacteristicGreaterEqual,
    CharacteristicGreater,
}

impl SetRelation {
    pub(crate) fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            1 => Self::Equal,
            2 => Self::NotEqual,
            3 => Self::Subset,
            4 => Self::Superset,
            5 => Self::Disjoint,
            6 => Self::Complement,
            7 => Self::CharacteristicLessEqual,
            8 => Self::CharacteristicLess,
            9 => Self::CharacteristicGreaterEqual,
            10 => Self::CharacteristicGreater,
            _ => return None,
        })
    }

    pub(crate) fn holds(self, left: &IntegerSet, right: &IntegerSet) -> bool {
        match self {
            Self::Equal => left == right,
            Self::NotEqual => left != right,
            Self::Subset => left.is_subset(right),
            Self::Superset => right.is_subset(left),
            Self::LessEqual => left.lex_cmp(right).is_le(),
            Self::Less => left.lex_cmp(right).is_lt(),
            Self::Disjoint => left.is_disjoint(right),
            Self::Complement => {
                let universe = IntegerSet::interval(-1073741822, 1073741822);
                left.is_subset(&universe)
                    && right.is_subset(&universe)
                    && left.is_disjoint(right)
                    && left.union(right) == universe
            }
            Self::CharacteristicLessEqual => left.characteristic_cmp(right).is_le(),
            Self::CharacteristicLess => left.characteristic_cmp(right).is_lt(),
            Self::CharacteristicGreaterEqual => left.characteristic_cmp(right).is_ge(),
            Self::CharacteristicGreater => left.characteristic_cmp(right).is_gt(),
        }
    }
}

impl std::fmt::Display for SetRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Equal => "=",
            Self::NotEqual => "!=",
            Self::Subset => "subset",
            Self::Superset => "superset",
            Self::LessEqual => "<= (sorted elements)",
            Self::Less => "< (sorted elements)",
            Self::Disjoint => "disjoint",
            Self::Complement => "complement (-1073741822..1073741822)",
            Self::CharacteristicLessEqual => "<= (characteristic order)",
            Self::CharacteristicLess => "< (characteristic order)",
            Self::CharacteristicGreaterEqual => ">= (characteristic order)",
            Self::CharacteristicGreater => "> (characteristic order)",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetOperation {
    Union,
    Intersection,
    Difference,
    SymmetricDifference,
}

impl SetOperation {
    pub(crate) fn calculate(self, left: &IntegerSet, right: &IntegerSet) -> IntegerSet {
        match self {
            Self::Union => left.union(right),
            Self::Intersection => left.intersection(right),
            Self::Difference => left.difference(right),
            Self::SymmetricDifference => left.symmetric_difference(right),
        }
    }
}

impl std::fmt::Display for SetOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Union => "union",
            Self::Intersection => "intersect",
            Self::Difference => "diff",
            Self::SymmetricDifference => "symdiff",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SetKind {
    Relation(SetRelation),
    Membership { fixed: bool },
    Cardinality,
    Operation(SetOperation),
    NativeRelation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SetName {
    pub kind: SetKind,
    pub mode: RelationMode,
}

impl SetName {
    pub fn from_name(name: &str) -> Option<Self> {
        use RelationMode::*;
        use SetKind::*;
        use SetOperation::*;
        use SetRelation::*;
        let (kind, mode) = match name {
            "set_eq" | "gecode_set_eq" | "fzn_set_eq" => (Relation(Equal), Plain),
            "set_eq_reif" | "gecode_set_eq_reif" | "fzn_set_eq_reif" => (Relation(Equal), Reified),
            "set_ne" | "gecode_set_ne" | "fzn_set_ne" => (Relation(NotEqual), Plain),
            "set_ne_reif" | "gecode_set_ne_reif" | "fzn_set_ne_reif" => {
                (Relation(NotEqual), Reified)
            }
            "set_le" | "gecode_set_le" | "fzn_set_le" => (Relation(LessEqual), Plain),
            "set_le_reif" | "gecode_set_le_reif" | "fzn_set_le_reif" => {
                (Relation(LessEqual), Reified)
            }
            "set_lt" | "gecode_set_lt" | "fzn_set_lt" => (Relation(Less), Plain),
            "set_lt_reif" | "gecode_set_lt_reif" | "fzn_set_lt_reif" => (Relation(Less), Reified),
            "set_subset" | "gecode_set_subset" | "fzn_set_subset" => (Relation(Subset), Plain),
            "set_subset_reif" | "gecode_set_subset_reif" | "fzn_set_subset_reif" => {
                (Relation(Subset), Reified)
            }
            "set_superset" | "gecode_set_superset" | "fzn_set_superset" => {
                (Relation(Superset), Plain)
            }
            "set_superset_reif" | "gecode_set_superset_reif" | "fzn_set_superset_reif" => {
                (Relation(Superset), Reified)
            }
            "equal" | "gecode_equal" | "fzn_equal" => (Relation(Equal), Plain),
            "equal_reif" | "gecode_equal_reif" | "fzn_equal_reif" => (Relation(Equal), Reified),
            "set_in" | "gecode_set_in" | "fzn_set_in" => (Membership { fixed: false }, Plain),
            "set_in_reif" | "gecode_set_in_reif" | "fzn_set_in_reif" => {
                (Membership { fixed: false }, Reified)
            }
            "set_in_imp" | "gecode_set_in_imp" | "fzn_set_in_imp" => {
                (Membership { fixed: false }, Implied)
            }
            "int_in" | "gecode_int_in" | "fzn_int_in" => (Membership { fixed: true }, Plain),
            "int_in_reif" | "gecode_int_in_reif" | "fzn_int_in_reif" => {
                (Membership { fixed: true }, Reified)
            }
            "int_in_imp" | "gecode_int_in_imp" | "fzn_int_in_imp" => {
                (Membership { fixed: true }, Implied)
            }
            "set_union" | "gecode_set_union" | "fzn_set_union" => (Operation(Union), Plain),
            "set_intersect" | "gecode_set_intersect" | "fzn_set_intersect" => {
                (Operation(Intersection), Plain)
            }
            "set_diff" | "gecode_set_diff" | "fzn_set_diff" => (Operation(Difference), Plain),
            "set_symdiff" | "gecode_set_symdiff" | "fzn_set_symdiff" => {
                (Operation(SymmetricDifference), Plain)
            }
            "set_card" | "gecode_set_card" | "fzn_set_card" => (Cardinality, Plain),
            "gecode_set_rel_imp_fzn"
            | "gecode_gecode_set_rel_imp_fzn"
            | "fzn_gecode_set_rel_imp_fzn" => (NativeRelation, Implied),
            "set_eq_imp" => (Relation(Equal), Implied),
            "set_ne_imp" => (Relation(NotEqual), Implied),
            "set_le_imp" => (Relation(LessEqual), Implied),
            "set_lt_imp" => (Relation(Less), Implied),
            "set_subset_imp" => (Relation(Subset), Implied),
            "set_superset_imp" => (Relation(Superset), Implied),
            _ => return None,
        };
        Some(Self { kind, mode })
    }

    #[cfg(feature = "prepare")]
    pub fn signature(self, element: ValueType) -> Vec<ArgumentType> {
        let scalar = |kind, is_var| ArgumentType {
            kind,
            is_var,
            is_array: false,
        };
        let set = scalar(ValueType::IntegerSet, true);
        let mut arguments = match self.kind {
            SetKind::Relation(_) => vec![set; 2],
            SetKind::Membership { fixed } => vec![
                scalar(element, true),
                scalar(
                    ValueType::IntegerSet,
                    !fixed && element != ValueType::Boolean,
                ),
            ],
            SetKind::Cardinality => vec![set, scalar(ValueType::Integer, true)],
            SetKind::Operation(_) => vec![set; 3],
            SetKind::NativeRelation => vec![set.clone(), scalar(ValueType::Integer, false), set],
        };
        if self.mode != RelationMode::Plain {
            arguments.push(scalar(ValueType::Boolean, true));
        }
        arguments
    }
}
