//! Exact number theory, products and fixed-degree floor roots.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberTheoryOperation {
    Gcd,
    Divides,
    Product,
    FixedModularProduct,
    VariableModularProduct,
    Nroot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NumberTheoryName {
    pub operation: NumberTheoryOperation,
    pub mode: RelationMode,
}
impl NumberTheoryName {
    pub fn from_name(name: &str) -> Option<Self> {
        use NumberTheoryOperation::*;
        use RelationMode::*;
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            let (operation, mode) = match base {
                "gecode_gcd_int" => (Gcd, Plain),
                "gecode_gcd_int_reif" => (Gcd, Reified),
                "gecode_gcd_int_imp" => (Gcd, Implied),
                "gecode_divides_int" => (Divides, Plain),
                "gecode_divides_int_reif" => (Divides, Reified),
                "gecode_divides_int_imp" => (Divides, Implied),
                "gecode_product_int" => (Product, Plain),
                "gecode_product_int_reif" => (Product, Reified),
                "gecode_product_int_imp" => (Product, Implied),
                "gecode_product_mod_fixed_fzn" => (FixedModularProduct, Plain),
                "gecode_product_mod_fixed_fzn_reif" => (FixedModularProduct, Reified),
                "gecode_product_mod_fixed_fzn_imp" => (FixedModularProduct, Implied),
                "gecode_product_mod_var_fzn" => (VariableModularProduct, Plain),
                "gecode_product_mod_var_fzn_reif" => (VariableModularProduct, Reified),
                "gecode_product_mod_var_fzn_imp" => (VariableModularProduct, Implied),
                "gecode_int_nroot" => (Nroot, Plain),
                _ => continue,
            };
            return Some(Self { operation, mode });
        }
        None
    }
    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<prepare::ArgumentType> {
        use NumberTheoryOperation::*;
        let scalar = |is_var| prepare::ArgumentType {
            kind: ValueType::Integer,
            is_array: false,
            is_var,
        };
        let array = prepare::ArgumentType {
            kind: ValueType::Integer,
            is_array: true,
            is_var: true,
        };
        let mut args = match self.operation {
            Gcd => vec![scalar(true), scalar(true), scalar(true)],
            Divides => vec![scalar(true), scalar(true)],
            Product => vec![array, scalar(true)],
            FixedModularProduct => vec![array, scalar(false), scalar(true)],
            VariableModularProduct => vec![array, scalar(true), scalar(true)],
            Nroot => vec![scalar(true), scalar(false), scalar(true)],
        };
        if self.mode != RelationMode::Plain {
            args.push(prepare::ArgumentType {
                kind: ValueType::Boolean,
                is_array: false,
                is_var: true,
            });
        }
        args
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum NumberTheory {
    Gcd {
        left: Operand,
        right: Operand,
        result: Operand,
        condition: RelationCondition,
    },
    Divides {
        divisor: Operand,
        dividend: Operand,
        condition: RelationCondition,
    },
    Product {
        factors: Vec<Operand>,
        result: Operand,
        condition: RelationCondition,
    },
    ModularProduct {
        factors: Vec<Operand>,
        modulus: Modulus,
        result: Operand,
        condition: RelationCondition,
    },
    Nroot {
        radicand: Operand,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        degree: i64,
        result: Operand,
    },
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Modulus {
    Fixed(
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        i64,
    ),
    Variable(Operand),
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberTheoryRole {
    Left,
    Right,
    Divisor,
    Dividend,
    Factor,
    Modulus,
    Radicand,
    Degree,
    Result,
}
#[derive(Clone, Debug, Serialize)]
pub struct NumberTheoryArgument {
    pub role: NumberTheoryRole,
    pub position: Option<usize>,
    pub value: Option<ArgumentValue>,
    pub unresolved: Option<Box<VariableRef>>,
}
#[derive(Clone, Debug, Serialize)]
pub struct NumberTheoryFinding {
    pub operation: NumberTheoryOperation,
    pub arguments: Vec<NumberTheoryArgument>,
    #[serde(serialize_with = "serialize_optional_integer")]
    pub expected: Option<i64>,
    pub relation: bool,
    pub condition: RelationConditionValue,
}

impl NumberTheory {
    pub fn name(&self) -> NumberTheoryName {
        use NumberTheoryOperation::*;
        let operation = match self {
            Self::Gcd { .. } => Gcd,
            Self::Divides { .. } => Divides,
            Self::Product { .. } => Product,
            Self::ModularProduct {
                modulus: Modulus::Fixed(_),
                ..
            } => FixedModularProduct,
            Self::ModularProduct {
                modulus: Modulus::Variable(_),
                ..
            } => VariableModularProduct,
            Self::Nroot { .. } => Nroot,
        };
        NumberTheoryName {
            operation,
            mode: self.condition().mode(),
        }
    }
    pub fn condition(&self) -> &RelationCondition {
        match self {
            Self::Gcd { condition, .. }
            | Self::Divides { condition, .. }
            | Self::Product { condition, .. }
            | Self::ModularProduct { condition, .. } => condition,
            Self::Nroot { .. } => &RelationCondition::Plain,
        }
    }
    pub fn integer_operands(&self) -> Vec<&Operand> {
        match self {
            Self::Gcd {
                left,
                right,
                result,
                ..
            } => vec![left, right, result],
            Self::Divides {
                divisor, dividend, ..
            } => vec![divisor, dividend],
            Self::Product {
                factors, result, ..
            } => factors.iter().chain(std::iter::once(result)).collect(),
            Self::ModularProduct {
                factors,
                modulus,
                result,
                ..
            } => factors
                .iter()
                .chain(match modulus {
                    Modulus::Fixed(_) => None,
                    Modulus::Variable(value) => Some(value),
                })
                .chain(std::iter::once(result))
                .collect(),
            Self::Nroot {
                radicand, result, ..
            } => vec![radicand, result],
        }
    }
    pub fn operands(&self) -> Vec<&Operand> {
        self.integer_operands()
            .into_iter()
            .chain(self.condition().operand())
            .collect()
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if let Self::ModularProduct {
            modulus: Modulus::Fixed(modulus),
            ..
        } = self
            && !(1..=2_147_483_646).contains(modulus)
        {
            return Err("fixed modulus must be in 1..2147483646");
        }
        if let Self::Nroot { degree, .. } = self
            && !(1..=2_147_483_646).contains(degree)
        {
            return Err("root degree must be in 1..2147483646");
        }
        Ok(())
    }
    fn result(&self) -> Option<&Operand> {
        match self {
            Self::Gcd { result, .. }
            | Self::Product { result, .. }
            | Self::ModularProduct { result, .. }
            | Self::Nroot { result, .. } => Some(result),
            Self::Divides { .. } => None,
        }
    }
    pub fn can_define(&self, output: usize) -> bool {
        let target = match self.condition() {
            RelationCondition::Plain => self.result(),
            RelationCondition::Reified { result } => Some(result),
            RelationCondition::Implied { .. } => None,
        };
        matches!(target, Some(Operand::Variable(slot)) if *slot == output)
            && self
                .operands()
                .iter()
                .filter(|operand| matches!(operand, Operand::Variable(slot) if *slot == output))
                .count()
                == 1
    }
    fn modulus(&self, values: &[Option<Value>]) -> Option<i64> {
        let Self::ModularProduct { modulus, .. } = self else {
            return None;
        };
        match modulus {
            Modulus::Fixed(value) => Some(*value),
            Modulus::Variable(value) => Some(value.value(values)?.integer()),
        }
    }
    fn calculate(
        &self,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<i64>, CheckError> {
        let integer = |operand: &Operand| operand.value(values).map(Value::integer);
        let overflow =
            || constraint.execution_error("checked i64 overflow in number theory arithmetic");
        let result = match self {
            Self::Gcd { left, right, .. } => {
                let (Some(left), Some(right)) = (integer(left), integer(right)) else {
                    return Ok(None);
                };
                let (mut a, mut b) = (left.unsigned_abs(), right.unsigned_abs());
                while b != 0 {
                    (a, b) = (b, a % b);
                }
                i64::try_from(a).map_err(|_| overflow())?
            }
            Self::Product { factors, .. } | Self::ModularProduct { factors, .. } => {
                let Some(factors) = factors.iter().map(integer).collect::<Option<Vec<_>>>() else {
                    return Ok(None);
                };
                if matches!(self, Self::ModularProduct { .. }) {
                    let Some(modulus) = self.modulus(values) else {
                        return Ok(None);
                    };
                    if modulus <= 0 {
                        return Ok(None);
                    }
                    let mut product = 1 % modulus;
                    for factor in factors {
                        product = ((i128::from(product) * i128::from(factor.rem_euclid(modulus)))
                            % i128::from(modulus)) as i64;
                    }
                    product
                } else if factors.contains(&0) {
                    0
                } else {
                    factors
                        .into_iter()
                        .try_fold(1i64, |product, factor| product.checked_mul(factor))
                        .ok_or_else(overflow)?
                }
            }
            Self::Nroot {
                radicand, degree, ..
            } => {
                let Some(radicand) = integer(radicand) else {
                    return Ok(None);
                };
                if radicand < 0 && degree % 2 == 0 {
                    return Err(
                        constraint.execution_error("even integer root of a negative radicand")
                    );
                }
                floor_root(radicand, *degree as u64)
            }
            Self::Divides { .. } => return Ok(None),
        };
        Ok(Some(result))
    }
    fn truth(
        &self,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<bool>, CheckError> {
        if let Self::Divides {
            divisor, dividend, ..
        } = self
        {
            return Ok(divisor.value(values).zip(dividend.value(values)).map(
                |(divisor, dividend)| {
                    let (divisor, dividend) = (divisor.integer(), dividend.integer());
                    if divisor == 0 {
                        dividend == 0
                    } else {
                        divisor == -1 || dividend % divisor == 0
                    }
                },
            ));
        }
        if matches!(self, Self::ModularProduct { .. })
            && self.modulus(values).is_some_and(|modulus| modulus <= 0)
        {
            return Ok(Some(false));
        }
        Ok(self
            .calculate(values, constraint)?
            .zip(self.result().and_then(|result| result.value(values)))
            .map(|(expected, actual)| expected == actual.integer()))
    }
    pub fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        if self.modulus(values).is_some_and(|modulus| modulus <= 0) {
            self.condition().operand().into_iter().collect()
        } else {
            self.operands()
        }
    }
    pub fn derive(
        &self,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<Value>, CheckError> {
        match self.condition() {
            RelationCondition::Plain => Ok(self.calculate(values, constraint)?.map(Value::Integer)),
            RelationCondition::Reified { .. } => {
                Ok(self.truth(values, constraint)?.map(Value::Boolean))
            }
            RelationCondition::Implied { .. } => Ok(None),
        }
    }
}

// Compare powers using capped integer multiplication, in logarithmic time in the degree.
fn power_order(mut base: u64, mut degree: u64, bound: u64) -> std::cmp::Ordering {
    let mut result = 1u64;
    while degree != 0 {
        if degree & 1 != 0 {
            let Some(product) = result.checked_mul(base).filter(|product| *product <= bound) else {
                return std::cmp::Ordering::Greater;
            };
            result = product;
        }
        degree >>= 1;
        if degree != 0 {
            let Some(square) = base.checked_mul(base).filter(|square| *square <= bound) else {
                return std::cmp::Ordering::Greater;
            };
            base = square;
        }
    }
    result.cmp(&bound)
}
fn floor_root(radicand: i64, degree: u64) -> i64 {
    if degree == 1 || radicand == 0 {
        return radicand;
    }
    let magnitude = radicand.unsigned_abs();
    let (mut lower, mut upper) = (0u64, magnitude);
    while lower < upper {
        let middle = lower + (upper - lower).div_ceil(2);
        if power_order(middle, degree, magnitude).is_gt() {
            upper = middle - 1;
        } else {
            lower = middle;
        }
    }
    if radicand < 0 {
        let ceiling = lower + u64::from(!power_order(lower, degree, magnitude).is_eq());
        -(ceiling as i64)
    } else {
        lower as i64
    }
}

impl PreparedModel {
    pub(crate) fn evaluate_number_theory(
        &self,
        constraint: &Constraint,
        arithmetic: &NumberTheory,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let Some(relation) = arithmetic.truth(values, constraint)? else {
            return Ok(None);
        };
        if arithmetic.condition().satisfied(relation, values) {
            return Ok(None);
        }
        Ok(Some(ViolationKind::NumberTheory {
            finding: NumberTheoryFinding {
                operation: arithmetic.name().operation,
                arguments: self.number_theory_arguments(arithmetic, values),
                expected: arithmetic.calculate(values, constraint)?,
                relation,
                condition: self.condition_value(arithmetic.condition(), values),
            },
        }))
    }
    fn number_theory_arguments(
        &self,
        arithmetic: &NumberTheory,
        values: &[Option<Value>],
    ) -> Vec<NumberTheoryArgument> {
        let mut arguments = Vec::new();
        let mut argument = |role, position, operand: &Operand| {
            arguments.push(NumberTheoryArgument {
                role,
                position,
                value: self.argument(operand, values),
                unresolved: match operand {
                    Operand::Variable(slot) if values[*slot].is_none() => {
                        Some(Box::new(self.slots[*slot].reference.clone()))
                    }
                    _ => None,
                },
            })
        };
        use NumberTheoryRole::*;
        match arithmetic {
            NumberTheory::Gcd {
                left,
                right,
                result,
                ..
            } => {
                argument(Left, None, left);
                argument(Right, None, right);
                argument(Result, None, result);
            }
            NumberTheory::Divides {
                divisor, dividend, ..
            } => {
                argument(Divisor, None, divisor);
                argument(Dividend, None, dividend);
            }
            NumberTheory::Product {
                factors, result, ..
            }
            | NumberTheory::ModularProduct {
                factors, result, ..
            } => {
                for (position, factor) in factors.iter().enumerate() {
                    argument(Factor, Some(position + 1), factor);
                }
                if let NumberTheory::ModularProduct { modulus, .. } = arithmetic {
                    match modulus {
                        self::Modulus::Fixed(value) => {
                            argument(Modulus, None, &Operand::Literal(Value::Integer(*value)))
                        }
                        self::Modulus::Variable(value) => argument(Modulus, None, value),
                    }
                }
                argument(Result, None, result);
            }
            NumberTheory::Nroot {
                radicand,
                degree,
                result,
            } => {
                argument(Radicand, None, radicand);
                argument(Degree, None, &Operand::Literal(Value::Integer(*degree)));
                argument(Result, None, result);
            }
        }
        arguments
    }
}
