//! Bin loads over fixed weights and concrete bin labels.
use super::*;

pub(crate) fn is_bin_packing_name(name: &str) -> bool {
    [
        name,
        name.strip_prefix("gecode_").unwrap_or(""),
        name.strip_prefix("fzn_").unwrap_or(""),
    ]
    .contains(&"gecode_bin_packing_load")
}

#[cfg(feature = "prepare")]
pub(crate) fn bin_packing_signature() -> Vec<prepare::ArgumentType> {
    vec![
        prepare::ArgumentType {
            kind: ValueType::Integer,
            is_array: true,
            is_var: true,
        },
        prepare::ArgumentType {
            kind: ValueType::Integer,
            is_array: true,
            is_var: true,
        },
        prepare::ArgumentType {
            kind: ValueType::Integer,
            is_array: true,
            is_var: false,
        },
        prepare::ArgumentType {
            kind: ValueType::Integer,
            is_array: false,
            is_var: false,
        },
    ]
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BinPacking {
    pub loads: Vec<Operand>,
    pub bins: Vec<Operand>,
    #[serde(
        serialize_with = "serialize_integers",
        deserialize_with = "deserialize_integers"
    )]
    pub weights: Vec<i64>,
    #[serde(
        serialize_with = "serialize_integer",
        deserialize_with = "deserialize_integer"
    )]
    pub minimum: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PackingItem {
    pub position: usize,
    pub bin: Option<ArgumentValue>,
    #[serde(serialize_with = "serialize_integer")]
    pub weight: i64,
}
#[derive(Clone, Debug, Serialize)]
pub struct PackingLoad {
    pub position: usize,
    #[serde(serialize_with = "serialize_integer")]
    pub bin: i64,
    pub supplied: Option<ArgumentValue>,
    #[serde(serialize_with = "serialize_optional_integer")]
    pub calculated: Option<i64>,
    pub items: Vec<usize>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BinPackingFault {
    Bin {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        value: i64,
    },
    Load {
        position: usize,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct BinPackingFinding {
    #[serde(serialize_with = "serialize_integer")]
    pub minimum: i64,
    pub items: Vec<PackingItem>,
    pub loads: Vec<PackingLoad>,
    pub faults: Vec<BinPackingFault>,
}

impl BinPacking {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.bins.len() != self.weights.len() {
            return Err("bin and weight arrays must have equal lengths");
        }
        if self.weights.iter().any(|&weight| weight < 0) {
            return Err("bin weights must be fixed nonnegative integers");
        }
        if !self.loads.is_empty()
            && i64::try_from(self.loads.len() - 1)
                .ok()
                .and_then(|last| self.minimum.checked_add(last))
                .is_none()
        {
            return Err("bin label interval exceeds i64");
        }
        Ok(())
    }
    pub fn operands(&self) -> Vec<&Operand> {
        self.loads.iter().chain(&self.bins).collect()
    }
    fn bin_index(&self, bin: i64) -> Option<usize> {
        bin.checked_sub(self.minimum)
            .and_then(|offset| usize::try_from(offset).ok())
            .filter(|&offset| offset < self.loads.len())
    }
    fn bin_faults(&self, values: &[Option<Value>]) -> Vec<BinPackingFault> {
        self.bins
            .iter()
            .enumerate()
            .filter_map(|(i, bin)| {
                let value = bin.value(values)?.integer();
                self.bin_index(value)
                    .is_none()
                    .then_some(BinPackingFault::Bin {
                        position: i + 1,
                        value,
                    })
            })
            .collect()
    }
    pub fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        if self.bin_faults(values).is_empty() {
            self.operands()
        } else {
            Vec::new()
        }
    }
    fn calculate(
        &self,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<Vec<i64>>, CheckError> {
        let mut loads = vec![0_i64; self.loads.len()];
        for (bin, &weight) in self.bins.iter().zip(&self.weights) {
            let Some(bin) = bin.value(values) else {
                return Ok(None);
            };
            if let Some(position) = self.bin_index(bin.integer()) {
                loads[position] = loads[position].checked_add(weight).ok_or_else(|| {
                    constraint.execution_error("checked i64 overflow in bin load sum")
                })?;
            }
        }
        Ok(Some(loads))
    }
    pub fn can_define(&self, output: usize) -> bool {
        let matches =
            |operand: &Operand| matches!(operand, Operand::Variable(slot) if *slot == output);
        self.loads.iter().any(matches) && !self.bins.iter().any(matches)
    }
    pub fn derive(
        &self,
        output: usize,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<Value>, CheckError> {
        let Some(loads) = self.calculate(values, constraint)? else {
            return Ok(None);
        };
        let mut result = None;
        for (operand, load) in self.loads.iter().zip(loads) {
            if matches!(operand, Operand::Variable(slot) if *slot == output) {
                if result.is_some_and(|prior| prior != load) {
                    return Ok(None);
                }
                result = Some(load);
            }
        }
        Ok(result.map(Value::Integer))
    }
}
impl PreparedModel {
    pub(crate) fn evaluate_packing(
        &self,
        constraint: &Constraint,
        packing: &BinPacking,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let mut faults = packing.bin_faults(values);
        let calculated = packing.calculate(values, constraint)?;
        for (i, load) in packing.loads.iter().enumerate() {
            if let (Some(supplied), Some(calculated)) = (load.value(values), &calculated)
                && supplied.integer() != calculated[i]
            {
                faults.push(BinPackingFault::Load { position: i + 1 });
            }
        }
        if faults.is_empty() {
            return Ok(None);
        }
        let loads = packing
            .loads
            .iter()
            .enumerate()
            .map(|(i, load)| {
                let supplied = self.argument(load, values);
                let calculated = calculated.as_ref().map(|loads| loads[i]);
                let bin = packing.minimum + i64::try_from(i).expect("validated bin interval");
                PackingLoad {
                    position: i + 1,
                    bin,
                    supplied,
                    calculated,
                    items: packing
                        .bins
                        .iter()
                        .enumerate()
                        .filter_map(|(j, operand)| {
                            (operand.value(values) == Some(&Value::Integer(bin))).then_some(j + 1)
                        })
                        .collect(),
                }
            })
            .collect();
        Ok(Some(ViolationKind::BinPacking {
            finding: BinPackingFinding {
                minimum: packing.minimum,
                items: packing
                    .bins
                    .iter()
                    .zip(&packing.weights)
                    .enumerate()
                    .map(|(i, (bin, &weight))| PackingItem {
                        position: i + 1,
                        bin: self.argument(bin, values),
                        weight,
                    })
                    .collect(),
                loads,
                faults,
            },
        }))
    }
}
