use serde::{Deserialize, Serialize};

/// The comparison applied to two exact integer values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegerRelation {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl IntegerRelation {
    pub(crate) fn holds(self, left: i64, right: i64) -> bool {
        match self {
            Self::Equal => left == right,
            Self::NotEqual => left != right,
            Self::Less => left < right,
            Self::LessEqual => left <= right,
            Self::Greater => left > right,
            Self::GreaterEqual => left >= right,
        }
    }
}

impl std::fmt::Display for IntegerRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Equal => "=",
            Self::NotEqual => "!=",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelationMode {
    Plain,
    Reified,
    Implied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RelationName {
    pub relation: IntegerRelation,
    pub linear: bool,
    pub mode: RelationMode,
}

impl RelationName {
    pub fn from_name(name: &str) -> Option<Self> {
        use IntegerRelation::*;
        use RelationMode::*;
        let (relation, linear, mode) = match name {
            "int_eq" | "gecode_int_eq" | "fzn_int_eq" => (Equal, false, Plain),
            "int_eq_reif" | "gecode_int_eq_reif" | "fzn_int_eq_reif" => (Equal, false, Reified),
            "int_eq_imp" | "gecode_int_eq_imp" | "fzn_int_eq_imp" => (Equal, false, Implied),
            "int_ne" | "gecode_int_ne" | "fzn_int_ne" => (NotEqual, false, Plain),
            "int_ne_reif" | "gecode_int_ne_reif" | "fzn_int_ne_reif" => (NotEqual, false, Reified),
            "int_ne_imp" | "gecode_int_ne_imp" | "fzn_int_ne_imp" => (NotEqual, false, Implied),
            "int_lt" | "gecode_int_lt" | "fzn_int_lt" => (Less, false, Plain),
            "int_lt_reif" | "gecode_int_lt_reif" | "fzn_int_lt_reif" => (Less, false, Reified),
            "int_lt_imp" | "gecode_int_lt_imp" | "fzn_int_lt_imp" => (Less, false, Implied),
            "int_le" | "gecode_int_le" | "fzn_int_le" => (LessEqual, false, Plain),
            "int_le_reif" | "gecode_int_le_reif" | "fzn_int_le_reif" => (LessEqual, false, Reified),
            "int_le_imp" | "gecode_int_le_imp" | "fzn_int_le_imp" => (LessEqual, false, Implied),
            "int_gt" | "gecode_int_gt" | "fzn_int_gt" => (Greater, false, Plain),
            "int_gt_reif" | "gecode_int_gt_reif" | "fzn_int_gt_reif" => (Greater, false, Reified),
            "int_gt_imp" | "gecode_int_gt_imp" | "fzn_int_gt_imp" => (Greater, false, Implied),
            "int_ge" | "gecode_int_ge" | "fzn_int_ge" => (GreaterEqual, false, Plain),
            "int_ge_reif" | "gecode_int_ge_reif" | "fzn_int_ge_reif" => {
                (GreaterEqual, false, Reified)
            }
            "int_ge_imp" | "gecode_int_ge_imp" | "fzn_int_ge_imp" => (GreaterEqual, false, Implied),
            "int_lin_eq" | "gecode_int_lin_eq" | "fzn_int_lin_eq" => (Equal, true, Plain),
            "int_lin_eq_reif" | "gecode_int_lin_eq_reif" | "fzn_int_lin_eq_reif" => {
                (Equal, true, Reified)
            }
            "int_lin_eq_imp" | "gecode_int_lin_eq_imp" | "fzn_int_lin_eq_imp" => {
                (Equal, true, Implied)
            }
            "int_lin_ne" | "gecode_int_lin_ne" | "fzn_int_lin_ne" => (NotEqual, true, Plain),
            "int_lin_ne_reif" | "gecode_int_lin_ne_reif" | "fzn_int_lin_ne_reif" => {
                (NotEqual, true, Reified)
            }
            "int_lin_ne_imp" | "gecode_int_lin_ne_imp" | "fzn_int_lin_ne_imp" => {
                (NotEqual, true, Implied)
            }
            "int_lin_lt" | "gecode_int_lin_lt" | "fzn_int_lin_lt" => (Less, true, Plain),
            "int_lin_lt_reif" | "gecode_int_lin_lt_reif" | "fzn_int_lin_lt_reif" => {
                (Less, true, Reified)
            }
            "int_lin_lt_imp" | "gecode_int_lin_lt_imp" | "fzn_int_lin_lt_imp" => {
                (Less, true, Implied)
            }
            "int_lin_le" | "gecode_int_lin_le" | "fzn_int_lin_le" => (LessEqual, true, Plain),
            "int_lin_le_reif" | "gecode_int_lin_le_reif" | "fzn_int_lin_le_reif" => {
                (LessEqual, true, Reified)
            }
            "int_lin_le_imp" | "gecode_int_lin_le_imp" | "fzn_int_lin_le_imp" => {
                (LessEqual, true, Implied)
            }
            "int_lin_gt" | "gecode_int_lin_gt" | "fzn_int_lin_gt" => (Greater, true, Plain),
            "int_lin_gt_reif" | "gecode_int_lin_gt_reif" | "fzn_int_lin_gt_reif" => {
                (Greater, true, Reified)
            }
            "int_lin_gt_imp" | "gecode_int_lin_gt_imp" | "fzn_int_lin_gt_imp" => {
                (Greater, true, Implied)
            }
            "int_lin_ge" | "gecode_int_lin_ge" | "fzn_int_lin_ge" => (GreaterEqual, true, Plain),
            "int_lin_ge_reif" | "gecode_int_lin_ge_reif" | "fzn_int_lin_ge_reif" => {
                (GreaterEqual, true, Reified)
            }
            "int_lin_ge_imp" | "gecode_int_lin_ge_imp" | "fzn_int_lin_ge_imp" => {
                (GreaterEqual, true, Implied)
            }
            _ => return None,
        };
        Some(Self {
            relation,
            linear,
            mode,
        })
    }

    #[cfg(feature = "prepare")]
    pub fn arity(self) -> usize {
        (if self.linear { 3 } else { 2 }) + usize::from(self.mode != RelationMode::Plain)
    }
}

pub(crate) fn checked_sum(terms: impl IntoIterator<Item = (i64, i64)>) -> Option<i64> {
    let mut sum = 0i64;
    for (coefficient, value) in terms {
        sum = sum.checked_add(coefficient.checked_mul(value)?)?;
    }
    Some(sum)
}
