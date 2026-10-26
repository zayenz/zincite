//! Native circuit topology, selected edge costs and deterministic cost outputs.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CircuitName {
    Plain,
    Cost,
    CostArray,
}
impl CircuitName {
    pub fn from_name(name: &str) -> Option<Self> {
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            return match base {
                "gecode_circuit" => Some(Self::Plain),
                "gecode_circuit_cost" => Some(Self::Cost),
                "gecode_circuit_cost_array" => Some(Self::CostArray),
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
            Self::Plain => vec![argument(false, false), argument(true, true)],
            Self::Cost => vec![
                argument(true, false),
                argument(true, true),
                argument(false, true),
            ],
            Self::CostArray => vec![
                argument(true, false),
                argument(true, true),
                argument(true, true),
                argument(false, true),
            ],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Circuit {
    Plain {
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        offset: i64,
        successors: Vec<Operand>,
    },
    Cost {
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        matrix: Vec<i64>,
        successors: Vec<Operand>,
        costs: Option<Vec<Operand>>,
        total: Operand,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct CircuitArgument {
    pub value: Option<ArgumentValue>,
    pub unresolved: Option<Box<VariableRef>>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CircuitFault {
    OutOfRange {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        successor: i64,
    },
    RepeatedSuccessor {
        first: usize,
        second: usize,
        #[serde(serialize_with = "serialize_integer")]
        successor: i64,
    },
    Subcycle {
        positions: Vec<usize>,
    },
    EdgeCost {
        position: usize,
        target_position: usize,
        #[serde(serialize_with = "serialize_integer")]
        expected: i64,
        #[serde(serialize_with = "serialize_integer")]
        actual: i64,
    },
    TotalCost {
        #[serde(serialize_with = "serialize_integer")]
        expected: i64,
        #[serde(serialize_with = "serialize_integer")]
        actual: i64,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct CircuitFinding {
    pub successors: Vec<CircuitArgument>,
    #[serde(serialize_with = "serialize_integer")]
    pub offset: i64,
    pub costs: Option<Vec<CircuitArgument>>,
    pub total: Option<CircuitArgument>,
    pub faults: Vec<CircuitFault>,
}

impl Circuit {
    pub fn name(&self) -> CircuitName {
        match self {
            Self::Plain { .. } => CircuitName::Plain,
            Self::Cost { costs: None, .. } => CircuitName::Cost,
            Self::Cost { costs: Some(_), .. } => CircuitName::CostArray,
        }
    }
    fn successors(&self) -> &[Operand] {
        match self {
            Self::Plain { successors, .. } | Self::Cost { successors, .. } => successors,
        }
    }
    fn offset(&self) -> i64 {
        match self {
            Self::Plain { offset, .. } => *offset,
            Self::Cost { .. } => 0,
        }
    }
    pub fn operands(&self) -> Vec<&Operand> {
        let mut operands: Vec<_> = self.successors().iter().collect();
        if let Self::Cost { costs, total, .. } = self {
            operands.extend(costs.iter().flatten());
            operands.push(total);
        }
        operands
    }
    pub fn validate_shape(&self) -> Result<(), &'static str> {
        let n = self.successors().len();
        if n == 0 {
            return Err("circuit requires nonempty successors");
        }
        if self.offset() < 0 {
            return Err("circuit offset must be nonnegative");
        }
        if i64::try_from(n - 1)
            .ok()
            .and_then(|last| self.offset().checked_add(last))
            .is_none()
        {
            return Err("fixed circuit successor interval overflows i64");
        }
        if let Self::Cost { matrix, costs, .. } = self {
            if n.checked_mul(n) != Some(matrix.len()) {
                return Err("circuit matrix requires n*n row-major costs");
            }
            if costs.as_ref().is_some_and(|costs| costs.len() != n) {
                return Err("circuit edge costs and successors need equal lengths");
            }
        }
        Ok(())
    }
    fn position(&self, successor: i64) -> Option<usize> {
        let upper = self
            .offset()
            .checked_add(i64::try_from(self.successors().len() - 1).ok()?)?;
        if successor < self.offset() || successor > upper {
            return None;
        }
        usize::try_from(successor.checked_sub(self.offset())?).ok()
    }
    fn selected_cost(&self, position: usize, values: &[Option<Value>]) -> Option<i64> {
        let Self::Cost {
            matrix, successors, ..
        } = self
        else {
            return None;
        };
        let target = self.position(successors[position].value(values)?.integer())?;
        let index = position
            .checked_mul(successors.len())?
            .checked_add(target)?;
        matrix.get(index).copied()
    }
    fn concrete_costs(&self, values: &[Option<Value>]) -> Option<Vec<i64>> {
        let Self::Cost {
            costs, successors, ..
        } = self
        else {
            return None;
        };
        match costs {
            Some(costs) => costs
                .iter()
                .map(|cost| cost.value(values).map(Value::integer))
                .collect(),
            None => (0..successors.len())
                .map(|position| self.selected_cost(position, values))
                .collect(),
        }
    }
    fn total_cost(
        &self,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<i64>, CheckError> {
        self.concrete_costs(values)
            .map(|costs| {
                checked_sum(costs.into_iter().map(|cost| (1, cost))).ok_or_else(|| {
                    constraint.execution_error("checked i64 overflow in circuit cost sum")
                })
            })
            .transpose()
    }
    fn topology_faults(&self, values: &[Option<Value>]) -> Vec<CircuitFault> {
        let successors = self.successors();
        let mut faults = Vec::new();
        let mut seen = HashMap::new();
        let mut targets = Vec::with_capacity(successors.len());
        for (i, successor) in successors.iter().enumerate() {
            let target = successor.value(values).and_then(|value| {
                let successor = value.integer();
                let target = self.position(successor);
                if target.is_none() {
                    faults.push(CircuitFault::OutOfRange {
                        position: i + 1,
                        successor,
                    });
                }
                if let Some(first) = seen.insert(successor, i + 1) {
                    faults.push(CircuitFault::RepeatedSuccessor {
                        first,
                        second: i + 1,
                        successor,
                    });
                }
                target
            });
            targets.push(target);
        }
        // Follow known edges once; an already closed proper cycle is decisive.
        let mut visited = vec![false; successors.len()];
        let mut path_positions = vec![None; successors.len()];
        for start in 0..successors.len() {
            if visited[start] {
                continue;
            }
            let mut path = Vec::new();
            let mut cursor = start;
            while !visited[cursor] {
                visited[cursor] = true;
                path_positions[cursor] = Some(path.len());
                path.push(cursor);
                let Some(next) = targets[cursor] else {
                    break;
                };
                if let Some(begin) = path_positions[next] {
                    let cycle = &path[begin..];
                    if cycle.len() < successors.len() {
                        faults.push(CircuitFault::Subcycle {
                            positions: cycle.iter().map(|position| position + 1).collect(),
                        });
                    }
                    break;
                }
                cursor = next;
            }
            for position in path {
                path_positions[position] = None;
            }
        }
        faults
    }
    fn edge_faults(&self, values: &[Option<Value>]) -> Vec<CircuitFault> {
        let Self::Cost {
            costs: Some(costs), ..
        } = self
        else {
            return Vec::new();
        };
        let mut faults = Vec::new();
        for (position, cost) in costs.iter().enumerate() {
            if let (Some(expected), Some(actual)) =
                (self.selected_cost(position, values), cost.value(values))
            {
                let actual = actual.integer();
                if expected != actual {
                    faults.push(CircuitFault::EdgeCost {
                        position: position + 1,
                        target_position: self
                            .position(self.successors()[position].value(values).unwrap().integer())
                            .unwrap()
                            + 1,
                        expected,
                        actual,
                    });
                }
            }
        }
        faults
    }
    pub fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        if !self.topology_faults(values).is_empty() || !self.edge_faults(values).is_empty() {
            return Vec::new();
        }
        if let Self::Cost { total, .. } = self
            && let Some(costs) = self.concrete_costs(values)
        {
            let sum = checked_sum(costs.into_iter().map(|cost| (1, cost)));
            if sum.is_none()
                || total
                    .value(values)
                    .is_some_and(|total| Some(total.integer()) != sum)
            {
                return Vec::new();
            }
        }
        self.operands()
    }
    pub fn array_outputs(&self) -> Vec<Vec<&Operand>> {
        match self {
            Self::Cost {
                costs: Some(costs),
                total,
                ..
            } => vec![
                costs.iter().collect(),
                costs.iter().chain(std::iter::once(total)).collect(),
            ],
            _ => Vec::new(),
        }
    }
    pub fn can_define(&self, output: usize) -> bool {
        let is_output =
            |operand: &Operand| matches!(operand, Operand::Variable(slot) if *slot == output);
        if self.successors().iter().any(is_output) {
            return false;
        }
        match self {
            Self::Cost { costs, total, .. } => {
                let is_edge_output = costs.iter().flatten().any(is_output);
                let is_total_output = is_output(total);
                is_edge_output != is_total_output
            }
            Self::Plain { .. } => false,
        }
    }
    pub fn derive(
        &self,
        output: usize,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<Value>, CheckError> {
        let Self::Cost { costs, total, .. } = self else {
            return Ok(None);
        };
        if matches!(total, Operand::Variable(slot) if *slot == output) {
            return Ok(self.total_cost(values, constraint)?.map(Value::Integer));
        }
        let mut selected = None;
        for (position, cost) in costs.iter().flatten().enumerate() {
            if matches!(cost, Operand::Variable(slot) if *slot == output) {
                let Some(cost) = self.selected_cost(position, values) else {
                    return Ok(None);
                };
                if selected.is_some_and(|previous| previous != cost) {
                    return Ok(None);
                }
                selected = Some(cost);
            }
        }
        Ok(selected.map(Value::Integer))
    }
}

impl PreparedModel {
    pub(crate) fn evaluate_circuit(
        &self,
        constraint: &Constraint,
        circuit: &Circuit,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let mut faults = circuit.topology_faults(values);
        faults.extend(circuit.edge_faults(values));
        if let Circuit::Cost { total, .. } = circuit
            && let (Some(expected), Some(actual)) =
                (circuit.total_cost(values, constraint)?, total.value(values))
        {
            let actual = actual.integer();
            if expected != actual {
                faults.push(CircuitFault::TotalCost { expected, actual });
            }
        }
        if faults.is_empty() {
            return Ok(None);
        }
        let argument = |operand: &Operand| CircuitArgument {
            value: self.argument(operand, values),
            unresolved: match operand {
                Operand::Variable(slot) if values[*slot].is_none() => {
                    Some(Box::new(self.slots[*slot].reference.clone()))
                }
                _ => None,
            },
        };
        let (costs, total) = match circuit {
            Circuit::Cost { costs, total, .. } => (
                costs
                    .as_ref()
                    .map(|costs| costs.iter().map(argument).collect()),
                Some(argument(total)),
            ),
            Circuit::Plain { .. } => (None, None),
        };
        Ok(Some(ViolationKind::Circuit {
            finding: CircuitFinding {
                successors: circuit.successors().iter().map(argument).collect(),
                offset: circuit.offset(),
                costs,
                total,
                faults,
            },
        }))
    }
}
