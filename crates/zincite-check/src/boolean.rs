use super::{IntegerRelation, RelationMode};
use serde::{Deserialize, Serialize};

/// The truth calculation applied to Boolean inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOperation {
    Not,
    And,
    Or,
    Xor,
    LeftImplication,
    RightImplication,
}

impl BooleanOperation {
    pub(crate) fn calculate(self, inputs: &[bool]) -> bool {
        match self {
            Self::Not => !inputs[0],
            Self::And => inputs.iter().all(|value| *value),
            Self::Or => inputs.iter().any(|value| *value),
            Self::Xor => inputs.iter().fold(false, |parity, value| parity ^ value),
            Self::LeftImplication => !inputs[1] || inputs[0],
            Self::RightImplication => !inputs[0] || inputs[1],
        }
    }

    pub(crate) fn input_count(self) -> usize {
        if self == Self::Not { 1 } else { 2 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BooleanKind {
    Relation(IntegerRelation),
    Linear(IntegerRelation),
    Scalar(BooleanOperation),
    Reduction(BooleanOperation),
    Clause,
    ToInteger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BooleanName {
    pub kind: BooleanKind,
    mode: RelationMode,
    allows_assertion: bool,
}

impl BooleanName {
    pub fn from_name(name: &str) -> Option<Self> {
        use BooleanKind::*;
        use BooleanOperation::*;
        use IntegerRelation::*;
        use RelationMode::*;
        let (kind, mode, allows_assertion) = match name {
            "bool_eq" | "gecode_bool_eq" | "fzn_bool_eq" => (Relation(Equal), Plain, false),
            "bool_eq_reif" | "gecode_bool_eq_reif" | "fzn_bool_eq_reif" => {
                (Relation(Equal), Reified, false)
            }
            "bool_eq_imp" | "gecode_bool_eq_imp" | "fzn_bool_eq_imp" => {
                (Relation(Equal), Implied, false)
            }
            "bool_ne" | "gecode_bool_ne" | "fzn_bool_ne" => (Relation(NotEqual), Plain, false),
            "bool_ne_reif" | "gecode_bool_ne_reif" | "fzn_bool_ne_reif" => {
                (Relation(NotEqual), Reified, false)
            }
            "bool_ne_imp" | "gecode_bool_ne_imp" | "fzn_bool_ne_imp" => {
                (Relation(NotEqual), Implied, false)
            }
            "bool_lt" | "gecode_bool_lt" | "fzn_bool_lt" => (Relation(Less), Plain, false),
            "bool_lt_reif" | "gecode_bool_lt_reif" | "fzn_bool_lt_reif" => {
                (Relation(Less), Reified, false)
            }
            "bool_lt_imp" | "gecode_bool_lt_imp" | "fzn_bool_lt_imp" => {
                (Relation(Less), Implied, false)
            }
            "bool_le" | "gecode_bool_le" | "fzn_bool_le" => (Relation(LessEqual), Plain, false),
            "bool_le_reif" | "gecode_bool_le_reif" | "fzn_bool_le_reif" => {
                (Relation(LessEqual), Reified, false)
            }
            "bool_le_imp" | "gecode_bool_le_imp" | "fzn_bool_le_imp" => {
                (Relation(LessEqual), Implied, false)
            }
            "bool_gt" | "gecode_bool_gt" | "fzn_bool_gt" => (Relation(Greater), Plain, false),
            "bool_gt_reif" | "gecode_bool_gt_reif" | "fzn_bool_gt_reif" => {
                (Relation(Greater), Reified, false)
            }
            "bool_gt_imp" | "gecode_bool_gt_imp" | "fzn_bool_gt_imp" => {
                (Relation(Greater), Implied, false)
            }
            "bool_ge" | "gecode_bool_ge" | "fzn_bool_ge" => (Relation(GreaterEqual), Plain, false),
            "bool_ge_reif" | "gecode_bool_ge_reif" | "fzn_bool_ge_reif" => {
                (Relation(GreaterEqual), Reified, false)
            }
            "bool_ge_imp" | "gecode_bool_ge_imp" | "fzn_bool_ge_imp" => {
                (Relation(GreaterEqual), Implied, false)
            }
            "bool_lin_eq" | "gecode_bool_lin_eq" | "fzn_bool_lin_eq" => {
                (Linear(Equal), Plain, false)
            }
            "bool_lin_eq_reif" | "gecode_bool_lin_eq_reif" | "fzn_bool_lin_eq_reif" => {
                (Linear(Equal), Reified, false)
            }
            "bool_lin_eq_imp" | "gecode_bool_lin_eq_imp" | "fzn_bool_lin_eq_imp" => {
                (Linear(Equal), Implied, false)
            }
            "bool_lin_ne" | "gecode_bool_lin_ne" | "fzn_bool_lin_ne" => {
                (Linear(NotEqual), Plain, false)
            }
            "bool_lin_ne_reif" | "gecode_bool_lin_ne_reif" | "fzn_bool_lin_ne_reif" => {
                (Linear(NotEqual), Reified, false)
            }
            "bool_lin_ne_imp" | "gecode_bool_lin_ne_imp" | "fzn_bool_lin_ne_imp" => {
                (Linear(NotEqual), Implied, false)
            }
            "bool_lin_lt" | "gecode_bool_lin_lt" | "fzn_bool_lin_lt" => {
                (Linear(Less), Plain, false)
            }
            "bool_lin_lt_reif" | "gecode_bool_lin_lt_reif" | "fzn_bool_lin_lt_reif" => {
                (Linear(Less), Reified, false)
            }
            "bool_lin_lt_imp" | "gecode_bool_lin_lt_imp" | "fzn_bool_lin_lt_imp" => {
                (Linear(Less), Implied, false)
            }
            "bool_lin_le" | "gecode_bool_lin_le" | "fzn_bool_lin_le" => {
                (Linear(LessEqual), Plain, false)
            }
            "bool_lin_le_reif" | "gecode_bool_lin_le_reif" | "fzn_bool_lin_le_reif" => {
                (Linear(LessEqual), Reified, false)
            }
            "bool_lin_le_imp" | "gecode_bool_lin_le_imp" | "fzn_bool_lin_le_imp" => {
                (Linear(LessEqual), Implied, false)
            }
            "bool_lin_gt" | "gecode_bool_lin_gt" | "fzn_bool_lin_gt" => {
                (Linear(Greater), Plain, false)
            }
            "bool_lin_gt_reif" | "gecode_bool_lin_gt_reif" | "fzn_bool_lin_gt_reif" => {
                (Linear(Greater), Reified, false)
            }
            "bool_lin_gt_imp" | "gecode_bool_lin_gt_imp" | "fzn_bool_lin_gt_imp" => {
                (Linear(Greater), Implied, false)
            }
            "bool_lin_ge" | "gecode_bool_lin_ge" | "fzn_bool_lin_ge" => {
                (Linear(GreaterEqual), Plain, false)
            }
            "bool_lin_ge_reif" | "gecode_bool_lin_ge_reif" | "fzn_bool_lin_ge_reif" => {
                (Linear(GreaterEqual), Reified, false)
            }
            "bool_lin_ge_imp" | "gecode_bool_lin_ge_imp" | "fzn_bool_lin_ge_imp" => {
                (Linear(GreaterEqual), Implied, false)
            }
            "bool_not" | "gecode_bool_not" | "fzn_bool_not" => (Scalar(Not), Reified, false),
            "bool_and" | "gecode_bool_and" | "fzn_bool_and" => (Scalar(And), Reified, false),
            "bool_and_imp" | "gecode_bool_and_imp" | "fzn_bool_and_imp" => {
                (Scalar(And), Implied, false)
            }
            "bool_or" | "gecode_bool_or" | "fzn_bool_or" => (Scalar(Or), Reified, false),
            "bool_or_imp" | "gecode_bool_or_imp" | "fzn_bool_or_imp" => {
                (Scalar(Or), Implied, false)
            }
            "bool_xor" => (Scalar(Xor), Reified, true),
            "gecode_bool_xor" | "fzn_bool_xor" => (Scalar(Xor), Reified, false),
            "bool_xor_imp" | "gecode_bool_xor_imp" | "fzn_bool_xor_imp" => {
                (Scalar(Xor), Implied, false)
            }
            "bool_left_imp" | "gecode_bool_left_imp" | "fzn_bool_left_imp" => {
                (Scalar(LeftImplication), Reified, false)
            }
            "bool_right_imp" | "gecode_bool_right_imp" | "fzn_bool_right_imp" => {
                (Scalar(RightImplication), Reified, false)
            }
            "array_bool_and" | "gecode_array_bool_and" | "fzn_array_bool_and" => {
                (Reduction(And), Reified, true)
            }
            "array_bool_and_imp" | "gecode_array_bool_and_imp" | "fzn_array_bool_and_imp" => {
                (Reduction(And), Implied, false)
            }
            "array_bool_or" | "gecode_array_bool_or" | "fzn_array_bool_or" => {
                (Reduction(Or), Reified, true)
            }
            "array_bool_or_imp" | "gecode_array_bool_or_imp" | "fzn_array_bool_or_imp" => {
                (Reduction(Or), Implied, false)
            }
            "array_bool_xor" | "gecode_array_bool_xor" | "fzn_array_bool_xor" => {
                (Reduction(Xor), Reified, true)
            }
            "array_bool_xor_imp" | "gecode_array_bool_xor_imp" | "fzn_array_bool_xor_imp" => {
                (Reduction(Xor), Implied, false)
            }
            "bool_clause" | "gecode_bool_clause" | "fzn_bool_clause" => (Clause, Plain, false),
            "bool_clause_reif" | "gecode_bool_clause_reif" | "fzn_bool_clause_reif" => {
                (Clause, Reified, false)
            }
            // The pinned Gecode clause_imp handler imposes equivalence.
            "bool_clause_imp" | "gecode_bool_clause_imp" | "fzn_bool_clause_imp" => {
                (Clause, Reified, false)
            }
            "bool2int" | "gecode_bool2int" | "fzn_bool2int" => (ToInteger, Plain, false),
            _ => return None,
        };
        Some(Self {
            kind,
            mode,
            allows_assertion,
        })
    }

    pub fn mode_for_arity(self, arity: usize) -> Option<RelationMode> {
        let inputs = match self.kind {
            BooleanKind::Relation(_) | BooleanKind::Clause | BooleanKind::ToInteger => 2,
            BooleanKind::Linear(_) => 3,
            BooleanKind::Scalar(operation) => operation.input_count(),
            BooleanKind::Reduction(_) => 1,
        };
        if arity == inputs + usize::from(self.mode != RelationMode::Plain) {
            Some(self.mode)
        } else if self.allows_assertion && arity == inputs {
            Some(RelationMode::Plain)
        } else {
            None
        }
    }

    #[cfg(feature = "prepare")]
    pub fn signature(self, arity: usize) -> Option<Vec<super::prepare::ArgumentType>> {
        use super::{ValueType, prepare::ArgumentType};
        let mode = self.mode_for_arity(arity)?;
        let scalar = |kind| ArgumentType {
            kind,
            is_var: true,
            is_array: false,
        };
        let array = |kind, is_var| ArgumentType {
            kind,
            is_var,
            is_array: true,
        };
        let mut arguments = match self.kind {
            BooleanKind::Linear(_) => vec![
                array(ValueType::Integer, false),
                array(ValueType::Boolean, true),
                scalar(ValueType::Integer),
            ],
            BooleanKind::Reduction(_) => vec![array(ValueType::Boolean, true)],
            BooleanKind::Clause => vec![array(ValueType::Boolean, true); 2],
            BooleanKind::ToInteger => vec![scalar(ValueType::Boolean), scalar(ValueType::Integer)],
            BooleanKind::Relation(_) => vec![scalar(ValueType::Boolean); 2],
            BooleanKind::Scalar(operation) => {
                vec![scalar(ValueType::Boolean); operation.input_count()]
            }
        };
        if mode != RelationMode::Plain {
            arguments.push(scalar(ValueType::Boolean));
        }
        Some(arguments)
    }
}

pub(crate) fn clause(positive: &[bool], negative: &[bool]) -> bool {
    positive.iter().any(|value| *value) || negative.iter().any(|value| !value)
}

pub(crate) fn from_integer(value: i64) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}
