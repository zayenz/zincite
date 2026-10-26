#[cfg(feature = "prepare")]
use crate::prepare::ArgumentType;
use crate::{IntegerRelation, Value, ValueType, relations::RelationMode};
use serde::{Deserialize, Deserializer, Serialize};

/// A finite binary64 value. Equality is exact; either signed zero becomes positive zero.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FiniteFloat(f64);
impl Eq for FiniteFloat {}
impl FiniteFloat {
    pub fn new(value: f64) -> Result<Self, &'static str> {
        if !value.is_finite() {
            return Err("float values must be finite");
        }
        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }
    pub fn get(self) -> f64 {
        self.0
    }
}
impl<'de> Deserialize<'de> for FiniteFloat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
impl std::fmt::Display for FiniteFloat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

/// Absolute and relative comparison tolerances owned by a prepared model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct FloatTolerance {
    absolute: FiniteFloat,
    relative: FiniteFloat,
}
impl FloatTolerance {
    pub fn new(absolute: f64, relative: f64) -> Result<Self, &'static str> {
        if absolute < 0.0 || relative < 0.0 {
            return Err("float tolerances must be nonnegative");
        }
        Ok(Self {
            absolute: FiniteFloat::new(absolute)?,
            relative: FiniteFloat::new(relative)?,
        })
    }
    pub fn absolute(self) -> f64 {
        self.absolute.get()
    }
    pub fn relative(self) -> f64 {
        self.relative.get()
    }
    pub fn equal(self, left: FiniteFloat, right: FiniteFloat) -> bool {
        let (a, b) = (left.get(), right.get());
        if a == b {
            return true;
        }
        let distance = (a - b).abs();
        if distance.is_finite() && distance <= self.absolute() {
            return true;
        }
        if self.relative() == 0.0 {
            return false;
        }
        let scale = a.abs().max(b.abs());
        let relative_distance = if distance.is_finite() {
            distance / scale
        } else {
            (a * 0.5 - b * 0.5).abs() / (scale * 0.5)
        };
        relative_distance <= self.relative()
    }
    pub(crate) fn holds(
        self,
        relation: IntegerRelation,
        left: FiniteFloat,
        right: FiniteFloat,
    ) -> bool {
        use IntegerRelation::*;
        let equal = self.equal(left, right);
        match relation {
            Equal => equal,
            NotEqual => !equal,
            Less => left < right && !equal,
            LessEqual => left < right || equal,
            Greater => left > right && !equal,
            GreaterEqual => left > right || equal,
        }
    }
    pub(crate) fn contains(
        self,
        value: FiniteFloat,
        lower: FiniteFloat,
        upper: FiniteFloat,
    ) -> bool {
        lower <= upper
            && self.holds(IntegerRelation::LessEqual, lower, value)
            && self.holds(IntegerRelation::LessEqual, value, upper)
    }
}
impl Default for FloatTolerance {
    fn default() -> Self {
        Self::new(1e-9, 1e-9).expect("finite nonnegative defaults")
    }
}
impl<'de> Deserialize<'de> for FloatTolerance {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Settings {
            absolute: FiniteFloat,
            relative: FiniteFloat,
        }
        let settings = Settings::deserialize(deserializer)?;
        Self::new(settings.absolute.get(), settings.relative.get())
            .map_err(serde::de::Error::custom)
    }
}

/// A fixed union of closed float ranges, ordered and nonoverlapping by exact bounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct FloatSet(Vec<(FiniteFloat, FiniteFloat)>);
impl FloatSet {
    pub fn from_ranges(mut ranges: Vec<(FiniteFloat, FiniteFloat)>) -> Self {
        ranges.retain(|(lower, upper)| lower <= upper);
        ranges.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("finite endpoints"));
        let mut union: Vec<(FiniteFloat, FiniteFloat)> = Vec::new();
        for (lower, upper) in ranges {
            if let Some(last) = union.last_mut()
                && lower <= last.1
            {
                if upper > last.1 {
                    last.1 = upper;
                }
            } else {
                union.push((lower, upper));
            }
        }
        Self(union)
    }
    pub fn ranges(&self) -> &[(FiniteFloat, FiniteFloat)] {
        &self.0
    }
}
impl<'de> Deserialize<'de> for FloatSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ranges = Vec::<(FiniteFloat, FiniteFloat)>::deserialize(deserializer)?;
        if ranges.iter().any(|(lower, upper)| lower > upper)
            || ranges.windows(2).any(|pair| pair[0].1 >= pair[1].0)
        {
            return Err(serde::de::Error::custom(
                "float set ranges must be ordered, nonoverlapping and not reversed",
            ));
        }
        Ok(Self(ranges))
    }
}
impl std::fmt::Display for FloatSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("{")?;
        for (index, (lower, upper)) in self.0.iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            if lower == upper {
                write!(f, "{lower}")?;
            } else {
                write!(f, "{lower}..{upper}")?;
            }
        }
        f.write_str("}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FloatOperation {
    Absolute,
    Add,
    Multiply,
    Divide,
    Minimum,
    Maximum,
    SquareRoot,
    Power,
    FixedBasePower,
    Exponential,
    NaturalLog,
    Log10,
    Log2,
    Sine,
    Cosine,
    Tangent,
    ArcSine,
    ArcCosine,
    ArcTangent,
    HyperbolicSine,
    HyperbolicCosine,
    HyperbolicTangent,
    InverseHyperbolicSine,
    InverseHyperbolicCosine,
    InverseHyperbolicTangent,
}
impl FloatOperation {
    pub(crate) fn input_count(self) -> usize {
        match self {
            Self::Add
            | Self::Multiply
            | Self::Divide
            | Self::Minimum
            | Self::Maximum
            | Self::Power
            | Self::FixedBasePower => 2,
            _ => 1,
        }
    }
    pub(crate) fn calculate(self, inputs: &[FiniteFloat]) -> Result<FiniteFloat, &'static str> {
        let x = inputs[0].get();
        let result = match self {
            Self::Absolute => x.abs(),
            Self::Add => x + inputs[1].get(),
            Self::Multiply => x * inputs[1].get(),
            Self::Divide => {
                let divisor = inputs[1].get();
                if divisor == 0.0 {
                    return Err("division by zero in float arithmetic");
                }
                x / divisor
            }
            Self::Minimum => x.min(inputs[1].get()),
            Self::Maximum => x.max(inputs[1].get()),
            Self::SquareRoot => {
                if x < 0.0 {
                    return Err("sqrt requires a nonnegative argument");
                }
                libm::sqrt(x)
            }
            Self::Power | Self::FixedBasePower => {
                let exponent = inputs[1].get();
                if self == Self::FixedBasePower && x <= 0.0 {
                    return Err("fixed-base power requires a positive base");
                }
                if x < 0.0 && exponent.trunc() != exponent {
                    return Err("negative float power base requires an integral exponent");
                }
                if x == 0.0 && exponent < 0.0 {
                    return Err("zero float power base requires a nonnegative exponent");
                }
                libm::pow(x, exponent)
            }
            Self::Exponential => libm::exp(x),
            Self::NaturalLog | Self::Log10 | Self::Log2 => {
                if x <= 0.0 {
                    return Err("logarithm requires a positive argument");
                }
                match self {
                    Self::NaturalLog => libm::log(x),
                    Self::Log10 => libm::log10(x),
                    _ => libm::log2(x),
                }
            }
            Self::Sine => libm::sin(x),
            Self::Cosine => libm::cos(x),
            Self::Tangent => libm::tan(x),
            Self::ArcSine | Self::ArcCosine => {
                if !(-1.0..=1.0).contains(&x) {
                    return Err("inverse sine/cosine requires an argument in -1..1");
                }
                if self == Self::ArcSine {
                    libm::asin(x)
                } else {
                    libm::acos(x)
                }
            }
            Self::ArcTangent => libm::atan(x),
            Self::HyperbolicSine => libm::sinh(x),
            Self::HyperbolicCosine => libm::cosh(x),
            Self::HyperbolicTangent => libm::tanh(x),
            Self::InverseHyperbolicSine => libm::asinh(x),
            Self::InverseHyperbolicCosine => {
                if x < 1.0 {
                    return Err("acosh requires an argument at least one");
                }
                libm::acosh(x)
            }
            Self::InverseHyperbolicTangent => {
                if x <= -1.0 || x >= 1.0 {
                    return Err("atanh requires an argument strictly between -1 and 1");
                }
                libm::atanh(x)
            }
        };
        finite_result(result)
    }
}
/// Exact-order selection; tolerance only checks the supplied result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FloatExtremum {
    Minimum,
    Maximum,
}
impl FloatExtremum {
    pub(crate) fn calculate(self, inputs: &[FiniteFloat]) -> FiniteFloat {
        inputs
            .iter()
            .copied()
            .reduce(|left, right| match self {
                Self::Minimum if right < left => right,
                Self::Maximum if right > left => right,
                _ => left,
            })
            .expect("validated nonempty float extremum")
    }
}

pub(crate) fn validate_integer_exponent(exponent: i64) -> Result<(), &'static str> {
    if !(0..=2147483646).contains(&exponent) {
        return Err("Gecode float power exponent must be in 0..2147483646");
    }
    Ok(())
}

pub(crate) fn integer_power(
    base: FiniteFloat,
    mut exponent: i64,
) -> Result<FiniteFloat, &'static str> {
    if base.get() == 0.0 && exponent == 0 {
        return Err("Gecode integer power excludes zero to the zero power");
    }
    let mut factor = base;
    let mut result = FiniteFloat::new(1.0)?;
    while exponent > 0 {
        if exponent % 2 == 1 {
            result = finite_result(result.get() * factor.get())?;
        }
        exponent /= 2;
        if exponent > 0 {
            factor = finite_result(factor.get() * factor.get())?;
        }
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FloatConversion {
    IntegerToFloat,
    Ceiling,
    Floor,
    Round,
}
impl FloatConversion {
    pub(crate) fn input_type(self) -> ValueType {
        if self == Self::IntegerToFloat {
            ValueType::Integer
        } else {
            ValueType::Float
        }
    }
    pub(crate) fn output_type(self) -> ValueType {
        if self == Self::IntegerToFloat {
            ValueType::Float
        } else {
            ValueType::Integer
        }
    }
    pub(crate) fn calculate(self, input: &Value) -> Result<Value, &'static str> {
        if self == Self::IntegerToFloat {
            return Ok(Value::Float(FiniteFloat::new(input.integer() as f64)?));
        }
        let x = input.float().get();
        let integer = match self {
            Self::Ceiling => x.ceil(),
            Self::Floor => x.floor(),
            Self::Round => x.round(),
            Self::IntegerToFloat => unreachable!(),
        };
        if !(-9223372036854775808.0..9223372036854775808.0).contains(&integer) {
            return Err("float conversion result is outside i64 range");
        }
        Ok(Value::Integer(integer as i64))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FloatMembership {
    Interval,
    Domain,
    Set,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FloatName {
    Relation {
        relation: IntegerRelation,
        linear: bool,
        mode: RelationMode,
    },
    CodedImplication,
    Arithmetic(FloatOperation),
    IntegerPower,
    ArrayExtremum {
        extremum: FloatExtremum,
        result_first: bool,
    },
    Conversion(FloatConversion),
    Membership(FloatMembership),
}
impl FloatName {
    pub fn from_name(name: &str) -> Option<Self> {
        use IntegerRelation::*;
        use RelationMode::*;
        Some(match name {
            "float_eq" | "gecode_float_eq" | "fzn_float_eq" => Self::Relation {
                relation: Equal,
                linear: false,
                mode: Plain,
            },
            "float_eq_reif" | "gecode_float_eq_reif" | "fzn_float_eq_reif" => Self::Relation {
                relation: Equal,
                linear: false,
                mode: Reified,
            },
            "float_eq_imp" => Self::Relation {
                relation: Equal,
                linear: false,
                mode: Implied,
            },
            "float_ne" | "gecode_float_ne" | "fzn_float_ne" => Self::Relation {
                relation: NotEqual,
                linear: false,
                mode: Plain,
            },
            "float_ne_reif" => Self::Relation {
                relation: NotEqual,
                linear: false,
                mode: Reified,
            },
            "float_ne_imp" => Self::Relation {
                relation: NotEqual,
                linear: false,
                mode: Implied,
            },
            "float_le" | "gecode_float_le" | "fzn_float_le" => Self::Relation {
                relation: LessEqual,
                linear: false,
                mode: Plain,
            },
            "float_le_reif" | "gecode_float_le_reif" | "fzn_float_le_reif" => Self::Relation {
                relation: LessEqual,
                linear: false,
                mode: Reified,
            },
            "float_le_imp" => Self::Relation {
                relation: LessEqual,
                linear: false,
                mode: Implied,
            },
            "float_lt" | "gecode_float_lt" | "fzn_float_lt" => Self::Relation {
                relation: Less,
                linear: false,
                mode: Plain,
            },
            "float_lt_reif" | "gecode_float_lt_reif" | "fzn_float_lt_reif" => Self::Relation {
                relation: Less,
                linear: false,
                mode: Reified,
            },
            "float_lt_imp" => Self::Relation {
                relation: Less,
                linear: false,
                mode: Implied,
            },
            "float_lin_eq" | "gecode_float_lin_eq" | "fzn_float_lin_eq" => Self::Relation {
                relation: Equal,
                linear: true,
                mode: Plain,
            },
            "float_lin_eq_reif" | "gecode_float_lin_eq_reif" | "fzn_float_lin_eq_reif" => {
                Self::Relation {
                    relation: Equal,
                    linear: true,
                    mode: Reified,
                }
            }
            "float_lin_eq_imp" => Self::Relation {
                relation: Equal,
                linear: true,
                mode: Implied,
            },
            "float_lin_ne" => Self::Relation {
                relation: NotEqual,
                linear: true,
                mode: Plain,
            },
            "float_lin_ne_reif" => Self::Relation {
                relation: NotEqual,
                linear: true,
                mode: Reified,
            },
            "float_lin_ne_imp" => Self::Relation {
                relation: NotEqual,
                linear: true,
                mode: Implied,
            },
            "float_lin_le" | "gecode_float_lin_le" | "fzn_float_lin_le" => Self::Relation {
                relation: LessEqual,
                linear: true,
                mode: Plain,
            },
            "float_lin_le_reif" | "gecode_float_lin_le_reif" | "fzn_float_lin_le_reif" => {
                Self::Relation {
                    relation: LessEqual,
                    linear: true,
                    mode: Reified,
                }
            }
            "float_lin_le_imp" => Self::Relation {
                relation: LessEqual,
                linear: true,
                mode: Implied,
            },
            "float_lin_lt" | "gecode_float_lin_lt" | "fzn_float_lin_lt" => Self::Relation {
                relation: Less,
                linear: true,
                mode: Plain,
            },
            "float_lin_lt_reif" | "gecode_float_lin_lt_reif" | "fzn_float_lin_lt_reif" => {
                Self::Relation {
                    relation: Less,
                    linear: true,
                    mode: Reified,
                }
            }
            "float_lin_lt_imp" => Self::Relation {
                relation: Less,
                linear: true,
                mode: Implied,
            },
            "float_abs" | "gecode_float_abs" | "fzn_float_abs" => {
                Self::Arithmetic(FloatOperation::Absolute)
            }
            "float_plus" | "gecode_float_plus" | "fzn_float_plus" => {
                Self::Arithmetic(FloatOperation::Add)
            }
            "float_times" | "gecode_float_times" | "fzn_float_times" => {
                Self::Arithmetic(FloatOperation::Multiply)
            }
            "float_div" | "gecode_float_div" | "fzn_float_div" => {
                Self::Arithmetic(FloatOperation::Divide)
            }
            "float_min" | "gecode_float_min" | "fzn_float_min" => {
                Self::Arithmetic(FloatOperation::Minimum)
            }
            "float_max" | "gecode_float_max" | "fzn_float_max" => {
                Self::Arithmetic(FloatOperation::Maximum)
            }
            "float_sqrt" | "gecode_float_sqrt" | "fzn_float_sqrt" => {
                Self::Arithmetic(FloatOperation::SquareRoot)
            }
            "float_acos" | "gecode_float_acos" | "fzn_float_acos" => {
                Self::Arithmetic(FloatOperation::ArcCosine)
            }
            "float_asin" | "gecode_float_asin" | "fzn_float_asin" => {
                Self::Arithmetic(FloatOperation::ArcSine)
            }
            "float_atan" | "gecode_float_atan" | "fzn_float_atan" => {
                Self::Arithmetic(FloatOperation::ArcTangent)
            }
            "float_cos" | "gecode_float_cos" | "fzn_float_cos" => {
                Self::Arithmetic(FloatOperation::Cosine)
            }
            "float_exp" | "gecode_float_exp" | "fzn_float_exp" => {
                Self::Arithmetic(FloatOperation::Exponential)
            }
            "float_ln" | "gecode_float_ln" | "fzn_float_ln" => {
                Self::Arithmetic(FloatOperation::NaturalLog)
            }
            "float_log10" | "gecode_float_log10" | "fzn_float_log10" => {
                Self::Arithmetic(FloatOperation::Log10)
            }
            "float_log2" | "gecode_float_log2" | "fzn_float_log2" => {
                Self::Arithmetic(FloatOperation::Log2)
            }
            "float_sin" | "gecode_float_sin" | "fzn_float_sin" => {
                Self::Arithmetic(FloatOperation::Sine)
            }
            "float_tan" | "gecode_float_tan" | "fzn_float_tan" => {
                Self::Arithmetic(FloatOperation::Tangent)
            }
            "float_acosh" => Self::Arithmetic(FloatOperation::InverseHyperbolicCosine),
            "float_asinh" => Self::Arithmetic(FloatOperation::InverseHyperbolicSine),
            "float_atanh" => Self::Arithmetic(FloatOperation::InverseHyperbolicTangent),
            "float_cosh" => Self::Arithmetic(FloatOperation::HyperbolicCosine),
            "float_sinh" => Self::Arithmetic(FloatOperation::HyperbolicSine),
            "float_tanh" => Self::Arithmetic(FloatOperation::HyperbolicTangent),
            "float_pow" => Self::Arithmetic(FloatOperation::Power),
            "gecode_float_pow" | "gecode_gecode_float_pow" | "fzn_gecode_float_pow" => {
                Self::IntegerPower
            }
            "gecode_float_pow_base"
            | "gecode_gecode_float_pow_base"
            | "fzn_gecode_float_pow_base" => Self::Arithmetic(FloatOperation::FixedBasePower),
            "array_float_minimum" => Self::ArrayExtremum {
                extremum: FloatExtremum::Minimum,
                result_first: true,
            },
            "gecode_float_array_min"
            | "gecode_gecode_float_array_min"
            | "fzn_gecode_float_array_min" => Self::ArrayExtremum {
                extremum: FloatExtremum::Minimum,
                result_first: false,
            },
            "array_float_maximum" => Self::ArrayExtremum {
                extremum: FloatExtremum::Maximum,
                result_first: true,
            },
            "gecode_float_array_max"
            | "gecode_gecode_float_array_max"
            | "fzn_gecode_float_array_max" => Self::ArrayExtremum {
                extremum: FloatExtremum::Maximum,
                result_first: false,
            },
            "int2float" | "gecode_int2float" | "fzn_int2float" => {
                Self::Conversion(FloatConversion::IntegerToFloat)
            }
            "float_ceil" => Self::Conversion(FloatConversion::Ceiling),
            "float_floor" => Self::Conversion(FloatConversion::Floor),
            "float_round" => Self::Conversion(FloatConversion::Round),
            "float_in" => Self::Membership(FloatMembership::Interval),
            "float_dom" => Self::Membership(FloatMembership::Domain),
            "float_set_in" => Self::Membership(FloatMembership::Set),
            "gecode_float_rel_imp_fzn"
            | "gecode_gecode_float_rel_imp_fzn"
            | "fzn_gecode_float_rel_imp_fzn" => Self::CodedImplication,
            _ => return None,
        })
    }
    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<ArgumentType> {
        let scalar = |kind, is_var| ArgumentType {
            kind,
            is_var,
            is_array: false,
        };
        let array = |kind, is_var| ArgumentType {
            kind,
            is_var,
            is_array: true,
        };
        match self {
            Self::Relation { linear, mode, .. } => {
                let mut args = if linear {
                    vec![
                        array(ValueType::Float, false),
                        array(ValueType::Float, true),
                        scalar(ValueType::Float, false),
                    ]
                } else {
                    vec![scalar(ValueType::Float, true); 2]
                };
                if mode != RelationMode::Plain {
                    args.push(scalar(ValueType::Boolean, true));
                }
                args
            }
            Self::CodedImplication => vec![
                scalar(ValueType::Float, true),
                scalar(ValueType::Integer, false),
                scalar(ValueType::Float, true),
                scalar(ValueType::Boolean, true),
            ],
            Self::Arithmetic(operation) => {
                let mut args = vec![scalar(ValueType::Float, true); operation.input_count() + 1];
                if operation == FloatOperation::FixedBasePower {
                    args[0].is_var = false;
                }
                args
            }
            Self::IntegerPower => vec![
                scalar(ValueType::Float, true),
                scalar(ValueType::Integer, false),
                scalar(ValueType::Float, true),
            ],
            Self::ArrayExtremum { result_first, .. } => {
                let result = scalar(ValueType::Float, true);
                let inputs = array(ValueType::Float, true);
                if result_first {
                    vec![result, inputs]
                } else {
                    vec![inputs, result]
                }
            }
            Self::Conversion(operation) => vec![
                scalar(operation.input_type(), true),
                scalar(operation.output_type(), true),
            ],
            Self::Membership(FloatMembership::Interval) => vec![
                scalar(ValueType::Float, true),
                scalar(ValueType::Float, false),
                scalar(ValueType::Float, false),
            ],
            Self::Membership(FloatMembership::Domain) => vec![
                scalar(ValueType::Float, true),
                array(ValueType::Float, false),
            ],
            Self::Membership(FloatMembership::Set) => vec![
                scalar(ValueType::Float, true),
                scalar(ValueType::FloatSet, false),
            ],
        }
    }
}
pub(crate) fn finite_result(value: f64) -> Result<FiniteFloat, &'static str> {
    FiniteFloat::new(value).map_err(|_| "nonfinite result in float arithmetic")
}
pub(crate) fn sum(
    terms: impl IntoIterator<Item = (FiniteFloat, FiniteFloat)>,
) -> Result<FiniteFloat, &'static str> {
    let mut sum = 0.0;
    for (coefficient, value) in terms {
        let product = finite_result(coefficient.get() * value.get())?;
        sum = finite_result(sum + product.get())?.get();
    }
    finite_result(sum)
}
