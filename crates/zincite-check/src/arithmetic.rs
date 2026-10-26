use serde::{Deserialize, Serialize};

/// The integer calculation checked by an arithmetic relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArithmeticOperation {
    Absolute,
    Add,
    Subtract,
    Negate,
    Multiply,
    Divide,
    Modulo,
    Minimum,
    Maximum,
    Power,
    FixedPower,
    GecodePower,
}

impl ArithmeticOperation {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "int_abs" | "gecode_int_abs" | "fzn_int_abs" => Self::Absolute,
            "int_plus" | "gecode_int_plus" | "fzn_int_plus" => Self::Add,
            "int_minus" | "gecode_int_minus" | "fzn_int_minus" => Self::Subtract,
            "int_negate" | "gecode_int_negate" | "fzn_int_negate" => Self::Negate,
            "int_times" | "gecode_int_times" | "fzn_int_times" => Self::Multiply,
            "int_div" | "gecode_int_div" | "fzn_int_div" => Self::Divide,
            "int_mod" | "gecode_int_mod" | "fzn_int_mod" => Self::Modulo,
            "int_min" | "gecode_int_min" | "fzn_int_min" => Self::Minimum,
            "int_max" | "gecode_int_max" | "fzn_int_max" => Self::Maximum,
            "int_pow" => Self::Power,
            "int_pow_fixed" => Self::FixedPower,
            "gecode_int_pow" | "gecode_gecode_int_pow" | "fzn_gecode_int_pow" => Self::GecodePower,
            _ => return None,
        })
    }

    pub(crate) fn input_count(self) -> usize {
        match self {
            Self::Absolute | Self::Negate => 1,
            _ => 2,
        }
    }

    pub(crate) fn has_fixed_exponent(self) -> bool {
        matches!(self, Self::FixedPower | Self::GecodePower)
    }

    pub(crate) fn validate_exponent(self, exponent: i64) -> Result<(), &'static str> {
        if self == Self::GecodePower && !(0..=2_147_483_646).contains(&exponent) {
            return Err("Gecode power exponent must be in 0..2147483646");
        }
        Ok(())
    }

    // Preparation and loading validate the input count and fixed exponent restrictions.
    pub(crate) fn calculate(self, inputs: &[i64]) -> Result<i64, &'static str> {
        const OVERFLOW: &str = "checked i64 overflow in integer arithmetic";
        let x = inputs[0];
        let checked = match self {
            Self::Absolute => x.checked_abs(),
            Self::Negate => x.checked_neg(),
            Self::Add => x.checked_add(inputs[1]),
            Self::Subtract => x.checked_sub(inputs[1]),
            Self::Multiply => x.checked_mul(inputs[1]),
            Self::Minimum => Some(x.min(inputs[1])),
            Self::Maximum => Some(x.max(inputs[1])),
            Self::Divide | Self::Modulo => {
                let y = inputs[1];
                if y == 0 {
                    return Err("division by zero in integer arithmetic");
                }
                if self == Self::Modulo {
                    // MIN mod -1 is zero even though MIN / -1 overflows.
                    if y == -1 { Some(0) } else { x.checked_rem(y) }
                } else {
                    x.checked_div(y)
                }
            }
            Self::Power | Self::FixedPower | Self::GecodePower => {
                let exponent = inputs[1];
                let magnitude = exponent
                    .checked_abs()
                    .ok_or("checked i64 overflow in exponent magnitude")?;
                let value = power(x, magnitude as u64).ok_or(OVERFLOW)?;
                if exponent < 0 {
                    if value == 0 {
                        return Err("zero base with negative integer exponent");
                    }
                    Some(1 / value)
                } else {
                    Some(value)
                }
            }
        };
        checked.ok_or(OVERFLOW)
    }
}

fn power(mut base: i64, mut exponent: u64) -> Option<i64> {
    let mut result = 1i64;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = result.checked_mul(base)?;
        }
        exponent >>= 1;
        if exponent != 0 {
            base = base.checked_mul(base)?;
        }
    }
    Some(result)
}
