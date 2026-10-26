//! Compact finite integer sets. Canonical ranges never overlap or touch.

use serde::{Deserialize, Serialize};

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
