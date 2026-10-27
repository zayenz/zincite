//! Scheduling relations over concrete candidate values.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SchedulingName {
    Strict,
    Unary,
    OptionalUnary,
    OptionalCumulative,
    Cumulative,
    Cumulatives,
    UnaryFlex,
    OptionalUnaryFlex,
    CumulativeFlex,
    OptionalCumulativeFlex,
    CumulativesFlex,
}
impl SchedulingName {
    pub fn from_name(name: &str, arity: usize) -> Option<Self> {
        if name == "fzn_disjunctive_strict" {
            return (arity == 2).then_some(Self::Strict);
        }
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            let kind = match base {
                "gecode_schedule_unary" => Self::Unary,
                "gecode_schedule_unary_optional" => Self::OptionalUnary,
                "gecode_schedule_cumulative_optional" => Self::OptionalCumulative,
                "gecode_unary_flex" => Self::UnaryFlex,
                "gecode_unary_flex_optional" => Self::OptionalUnaryFlex,
                "gecode_cumulative_flex" => Self::CumulativeFlex,
                "gecode_cumulative_flex_optional" => Self::OptionalCumulativeFlex,
                "gecode_cumulatives_flex" => Self::CumulativesFlex,
                "cumulatives" => match arity {
                    4 => Self::Cumulative,
                    6 => Self::Cumulatives,
                    7 => Self::CumulativesFlex,
                    _ => return None,
                },
                _ => continue,
            };
            return (arity == kind.arity()).then_some(kind);
        }
        None
    }
    fn arity(self) -> usize {
        match self {
            Self::Strict | Self::Unary => 2,
            Self::OptionalUnary => 3,
            Self::OptionalCumulative => 5,
            Self::Cumulative => 4,
            Self::Cumulatives => 6,
            Self::UnaryFlex => 3,
            Self::OptionalUnaryFlex => 4,
            Self::CumulativeFlex => 5,
            Self::OptionalCumulativeFlex => 6,
            Self::CumulativesFlex => 7,
        }
    }
    #[cfg(feature = "prepare")]
    pub fn uses_explicit_ends(self) -> bool {
        matches!(
            self,
            Self::UnaryFlex
                | Self::OptionalUnaryFlex
                | Self::CumulativeFlex
                | Self::OptionalCumulativeFlex
                | Self::CumulativesFlex
        )
    }
    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<prepare::ArgumentType> {
        let argument = |kind, is_array, is_var| prepare::ArgumentType {
            kind,
            is_array,
            is_var,
        };
        let integer_array = |is_var| argument(ValueType::Integer, true, is_var);
        if self.uses_explicit_ends() {
            let mut signature = vec![integer_array(true); 3];
            match self {
                Self::UnaryFlex => {}
                Self::OptionalUnaryFlex => signature.push(argument(ValueType::Boolean, true, true)),
                Self::CumulativeFlex | Self::OptionalCumulativeFlex => {
                    signature.extend([
                        integer_array(false),
                        argument(ValueType::Integer, false, true),
                    ]);
                    if self == Self::OptionalCumulativeFlex {
                        signature.push(argument(ValueType::Boolean, true, true));
                    }
                }
                Self::CumulativesFlex => signature.extend([
                    integer_array(true),
                    integer_array(true),
                    integer_array(false),
                    argument(ValueType::Boolean, false, false),
                ]),
                _ => unreachable!(),
            }
            return signature;
        }
        let mut signature = vec![
            integer_array(true),
            integer_array(matches!(
                self,
                Self::Strict | Self::Cumulative | Self::Cumulatives
            )),
        ];
        match self {
            Self::Strict | Self::Unary => {}
            Self::OptionalUnary => signature.push(argument(ValueType::Boolean, true, true)),
            Self::OptionalCumulative => signature.extend([
                integer_array(false),
                argument(ValueType::Boolean, true, true),
                argument(ValueType::Integer, false, true),
            ]),
            Self::Cumulative => signature.extend([
                integer_array(true),
                argument(ValueType::Integer, false, true),
            ]),
            Self::Cumulatives => signature.extend([
                integer_array(true),
                integer_array(true),
                integer_array(false),
                argument(ValueType::Boolean, false, false),
            ]),
            _ => unreachable!(),
        }
        signature
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ScheduleTasks {
    pub starts: Vec<Operand>,
    pub durations: Vec<Operand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ends: Option<Vec<Operand>>,
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Scheduling {
    Strict(ScheduleTasks),
    Unary {
        tasks: ScheduleTasks,
        present: Option<Vec<Operand>>,
    },
    OptionalCumulative {
        tasks: ScheduleTasks,
        usages: Vec<Operand>,
        present: Vec<Operand>,
        capacity: Operand,
    },
    Cumulative {
        tasks: ScheduleTasks,
        usages: Vec<Operand>,
        capacity: Operand,
    },
    Cumulatives {
        tasks: ScheduleTasks,
        usages: Vec<Operand>,
        machines: Vec<Operand>,
        capacities: Vec<Operand>,
        upper: bool,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct SchedulingTask {
    pub position: usize,
    pub start: Option<ArgumentValue>,
    pub duration: Option<ArgumentValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<ArgumentValue>,
    pub usage: Option<ArgumentValue>,
    pub present: Option<ArgumentValue>,
    pub machine: Option<ArgumentValue>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SchedulingFault {
    Duration {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        value: i64,
    },
    Machine {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        value: i64,
    },
    Capacity {
        #[serde(serialize_with = "serialize_integer")]
        value: i64,
    },
    Conflict {
        first: usize,
        second: usize,
    },
    TaskUsage {
        position: usize,
        #[serde(serialize_with = "serialize_integer")]
        usage: i64,
        #[serde(serialize_with = "serialize_integer")]
        bound: i64,
    },
    Resource {
        #[serde(serialize_with = "serialize_optional_integer")]
        machine: Option<i64>,
        #[serde(serialize_with = "serialize_integer")]
        time: i64,
        #[serde(serialize_with = "serialize_integer")]
        usage: i64,
        #[serde(serialize_with = "serialize_integer")]
        bound: i64,
        upper: bool,
        tasks: Vec<usize>,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct SchedulingFinding {
    pub tasks: Vec<SchedulingTask>,
    pub capacities: Vec<ArgumentValue>,
    pub faults: Vec<SchedulingFault>,
}

impl Scheduling {
    pub fn name(&self) -> SchedulingName {
        if self.tasks().ends.is_some() {
            return match self {
                Self::Unary { present: None, .. } => SchedulingName::UnaryFlex,
                Self::Unary {
                    present: Some(_), ..
                } => SchedulingName::OptionalUnaryFlex,
                Self::Cumulative { .. } => SchedulingName::CumulativeFlex,
                Self::OptionalCumulative { .. } => SchedulingName::OptionalCumulativeFlex,
                Self::Cumulatives { .. } => SchedulingName::CumulativesFlex,
                Self::Strict(_) => SchedulingName::Strict,
            };
        }
        match self {
            Self::Strict(_) => SchedulingName::Strict,
            Self::Unary { present: None, .. } => SchedulingName::Unary,
            Self::Unary {
                present: Some(_), ..
            } => SchedulingName::OptionalUnary,
            Self::OptionalCumulative { .. } => SchedulingName::OptionalCumulative,
            Self::Cumulative { .. } => SchedulingName::Cumulative,
            Self::Cumulatives { .. } => SchedulingName::Cumulatives,
        }
    }
    pub fn arity(&self) -> usize {
        self.name().arity()
    }
    fn tasks(&self) -> &ScheduleTasks {
        match self {
            Self::Strict(tasks)
            | Self::Unary { tasks, .. }
            | Self::OptionalCumulative { tasks, .. }
            | Self::Cumulative { tasks, .. }
            | Self::Cumulatives { tasks, .. } => tasks,
        }
    }
    pub fn presence(&self) -> Option<&[Operand]> {
        match self {
            Self::Unary { present, .. } => present.as_deref(),
            Self::OptionalCumulative { present, .. } => Some(present),
            _ => None,
        }
    }
    fn usages(&self) -> Option<&[Operand]> {
        match self {
            Self::OptionalCumulative { usages, .. }
            | Self::Cumulative { usages, .. }
            | Self::Cumulatives { usages, .. } => Some(usages),
            _ => None,
        }
    }
    fn capacities(&self) -> &[Operand] {
        match self {
            Self::OptionalCumulative { capacity, .. } | Self::Cumulative { capacity, .. } => {
                std::slice::from_ref(capacity)
            }
            Self::Cumulatives { capacities, .. } => capacities,
            _ => &[],
        }
    }
    fn machines(&self) -> Option<&[Operand]> {
        match self {
            Self::Cumulatives { machines, .. } => Some(machines),
            _ => None,
        }
    }
    pub fn integer_operands(&self) -> Vec<&Operand> {
        let tasks = self.tasks();
        tasks
            .starts
            .iter()
            .chain(&tasks.durations)
            .chain(tasks.ends.iter().flatten())
            .chain(self.usages().into_iter().flatten())
            .chain(self.machines().into_iter().flatten())
            .chain(self.capacities())
            .collect()
    }
    pub fn operands(&self) -> Vec<&Operand> {
        let mut operands = self.integer_operands();
        operands.extend(self.presence().into_iter().flatten());
        operands
    }
    pub fn validate_shape(&self) -> Result<(), &'static str> {
        let tasks = self.tasks();
        let n = tasks.starts.len();
        if tasks.durations.len() != n
            || tasks.ends.as_ref().is_some_and(|ends| ends.len() != n)
            || self.usages().is_some_and(|entries| entries.len() != n)
            || self.presence().is_some_and(|entries| entries.len() != n)
            || self.machines().is_some_and(|entries| entries.len() != n)
        {
            return Err("scheduling task arrays must have equal lengths");
        }
        if matches!(self, Self::Strict(_)) && tasks.ends.is_some() {
            return Err("strict disjunctive does not accept explicit ends");
        }
        let fixed_nonnegative = |entries: &[Operand]| {
            entries.iter().all(
                |operand| matches!(operand, Operand::Literal(Value::Integer(value)) if *value >= 0),
            )
        };
        if matches!(self, Self::Unary { .. } | Self::OptionalCumulative { .. })
            && tasks.ends.is_none()
            && !fixed_nonnegative(&tasks.durations)
        {
            return Err(
                "native unary and optional cumulative durations must be fixed nonnegative integers",
            );
        }
        if (matches!(self, Self::OptionalCumulative { .. })
            || self.name() == SchedulingName::CumulativeFlex)
            && let Some(usages) = self.usages()
            && !fixed_nonnegative(usages)
        {
            return Err("native cumulative usages must be fixed nonnegative integers");
        }
        if let Self::Cumulatives { capacities, .. } = self
            && !capacities
                .iter()
                .all(|operand| matches!(operand, Operand::Literal(Value::Integer(_))))
        {
            return Err("multi-machine capacities must be fixed integers");
        }
        Ok(())
    }
    pub fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        if self.precondition_faults(values).is_empty() {
            self.relation_operands(values)
        } else {
            Vec::new()
        }
    }
    fn precondition_faults(&self, values: &[Option<Value>]) -> Vec<SchedulingFault> {
        let mut faults = Vec::new();
        if (matches!(self, Self::OptionalCumulative { .. })
            || self.name() == SchedulingName::CumulativeFlex)
            && let Some(Value::Integer(value)) = self.capacities()[0].value(values)
            && *value < 0
        {
            faults.push(SchedulingFault::Capacity { value: *value });
        }
        if self.name() != SchedulingName::Cumulative
            && self.name() != SchedulingName::CumulativesFlex
        {
            for (i, duration) in self.tasks().durations.iter().enumerate() {
                if let Some(Value::Integer(value)) = duration.value(values)
                    && *value < 0
                {
                    faults.push(SchedulingFault::Duration {
                        position: i + 1,
                        value: *value,
                    });
                }
            }
        }
        if self.tasks().ends.is_none()
            && let Some(machines) = self.machines()
        {
            for (i, machine) in machines.iter().enumerate() {
                if let Some(Value::Integer(value)) = machine.value(values)
                    && usize::try_from(*value)
                        .ok()
                        .is_none_or(|index| index >= self.capacities().len())
                {
                    faults.push(SchedulingFault::Machine {
                        position: i + 1,
                        value: *value,
                    });
                }
            }
        }
        faults
    }
    fn relation_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        let tasks = self.tasks();
        if let Some(ends) = &tasks.ends {
            return self.explicit_operands(ends, values);
        }
        if matches!(self, Self::Cumulative { .. }) {
            return match tasks.starts.len() {
                0 => Vec::new(),
                1 => {
                    let mut required = vec![&tasks.durations[0]];
                    if tasks.durations[0]
                        .value(values)
                        .is_some_and(|value| value.integer() > 0)
                    {
                        required.extend(self.usages().into_iter().flatten());
                        required.extend(self.capacities());
                    }
                    required
                }
                _ => self.operands(),
            };
        }
        let mut required = Vec::new();
        required.extend(self.capacities());
        required.extend(self.presence().into_iter().flatten());
        for i in 0..tasks.starts.len() {
            if self
                .presence()
                .is_none_or(|present| present[i].value(values).is_some_and(Value::boolean))
            {
                required.extend([&tasks.starts[i], &tasks.durations[i]]);
                if let Some(usages) = self.usages() {
                    required.push(&usages[i]);
                }
                if let Some(machines) = self.machines() {
                    required.push(&machines[i]);
                }
            }
        }
        required
    }
    fn explicit_operands<'a>(
        &'a self,
        ends: &'a [Operand],
        values: &[Option<Value>],
    ) -> Vec<&'a Operand> {
        let tasks = self.tasks();
        let mut required = Vec::new();
        required.extend(self.capacities());
        required.extend(self.presence().into_iter().flatten());
        if self.machines().is_none() {
            // Native unary/cumulative require nonnegative durations even for absent tasks.
            required.extend(&tasks.durations);
        }
        for (i, end) in ends.iter().enumerate() {
            if self
                .presence()
                .is_some_and(|present| !present[i].value(values).is_some_and(Value::boolean))
            {
                continue;
            }
            if let Some(machines) = self.machines() {
                required.push(&machines[i]);
                if machines[i]
                    .value(values)
                    .is_some_and(|value| !self.valid_machine(value.integer()))
                {
                    continue;
                }
            }
            required.extend([&tasks.starts[i], end]);
            if self.machines().is_some()
                && tasks.starts[i]
                    .value(values)
                    .zip(end.value(values))
                    .is_some_and(|(start, end)| start.integer() >= end.integer())
            {
                continue;
            }
            if let Some(usages) = self.usages() {
                required.push(&usages[i]);
            }
        }
        required
    }
    fn valid_machine(&self, machine: i64) -> bool {
        usize::try_from(machine).is_ok_and(|index| index < self.capacities().len())
    }
}

struct ConcreteTask {
    position: usize,
    start: i64,
    end: i64,
    zero_duration: bool,
    usage: i64,
    machine: Option<i64>,
}
impl Scheduling {
    fn concrete_tasks(
        &self,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Vec<ConcreteTask>, CheckError> {
        let tasks = self.tasks();
        let integer = |operand: &Operand| {
            operand
                .value(values)
                .expect("available schedule operand")
                .integer()
        };
        let mut concrete = Vec::new();
        for i in 0..tasks.starts.len() {
            if self.presence().is_some_and(|present| {
                !present[i]
                    .value(values)
                    .expect("available presence")
                    .boolean()
            }) {
                continue;
            }
            let machine = self.machines().map(|machines| integer(&machines[i]));
            if let Some(machine) = machine
                && !self.valid_machine(machine)
            {
                continue;
            }
            let duration = tasks.durations[i].value(values).map(Value::integer);
            if duration.is_some_and(|duration| duration < 0)
                && !matches!(
                    self.name(),
                    SchedulingName::Cumulative | SchedulingName::CumulativesFlex
                )
            {
                continue;
            }
            let start = integer(&tasks.starts[i]);
            let end = if let Some(ends) = &tasks.ends {
                integer(&ends[i])
            } else {
                start
                    .checked_add(duration.expect("available legacy duration"))
                    .ok_or_else(|| {
                        constraint.execution_error("checked i64 overflow in scheduling endpoint")
                    })?
            };
            if machine.is_some() && tasks.ends.is_some() && start >= end {
                continue;
            }
            concrete.push(ConcreteTask {
                position: i + 1,
                start,
                end,
                zero_duration: duration == Some(0),
                usage: self.usages().map_or(1, |usages| integer(&usages[i])),
                machine,
            });
        }
        Ok(concrete)
    }
    fn check_relation(
        &self,
        values: &[Option<Value>],
        constraint: &Constraint,
        faults: &mut Vec<SchedulingFault>,
    ) -> Result<(), CheckError> {
        let tasks = self.tasks();
        let integer = |operand: &Operand| {
            operand
                .value(values)
                .expect("available schedule operand")
                .integer()
        };
        if self.name() == SchedulingName::Cumulative && tasks.starts.len() <= 1 {
            if tasks.starts.len() == 1 && integer(&tasks.durations[0]) > 0 {
                let usage = integer(&self.usages().expect("cumulative usages")[0]);
                let bound = integer(&self.capacities()[0]);
                if usage > bound {
                    faults.push(SchedulingFault::TaskUsage {
                        position: 1,
                        usage,
                        bound,
                    });
                }
            }
        } else {
            let concrete = self.concrete_tasks(values, constraint)?;
            match self {
                Self::Strict(_) | Self::Unary { .. } => {
                    for (i, first) in concrete.iter().enumerate() {
                        for second in &concrete[i + 1..] {
                            if first.end > second.start && second.end > first.start {
                                faults.push(SchedulingFault::Conflict {
                                    first: first.position,
                                    second: second.position,
                                });
                            }
                        }
                    }
                }
                Self::OptionalCumulative { capacity, .. } => {
                    let bound = integer(capacity);
                    if bound >= 0 {
                        check_resources(
                            &concrete,
                            ResourceLimit {
                                machine: None,
                                bound,
                                upper: true,
                            },
                            if tasks.ends.is_some() {
                                ResourceChecks::TaskStarts
                            } else {
                                ResourceChecks::ZeroDurationStarts
                            },
                            constraint,
                            faults,
                        )?;
                    }
                }
                Self::Cumulative { capacity, .. } => {
                    let bound = integer(capacity);
                    if tasks.ends.is_none() || bound >= 0 {
                        check_resources(
                            &concrete,
                            ResourceLimit {
                                machine: None,
                                bound,
                                upper: true,
                            },
                            if tasks.ends.is_some() {
                                ResourceChecks::TaskStarts
                            } else {
                                ResourceChecks::ConcreteHorizon
                            },
                            constraint,
                            faults,
                        )?;
                    }
                }
                Self::Cumulatives {
                    capacities, upper, ..
                } => {
                    for (machine, capacity) in capacities.iter().enumerate() {
                        let machine = i64::try_from(machine)
                            .map_err(|_| constraint.execution_error("machine index exceeds i64"))?;
                        check_resources(
                            &concrete,
                            ResourceLimit {
                                machine: Some(machine),
                                bound: integer(capacity),
                                upper: *upper,
                            },
                            ResourceChecks::ActiveIntervals,
                            constraint,
                            faults,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}
impl PreparedModel {
    pub(crate) fn evaluate_schedule(
        &self,
        constraint: &Constraint,
        schedule: &Scheduling,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let mut faults = schedule.precondition_faults(values);
        if schedule
            .relation_operands(values)
            .iter()
            .all(|operand| operand.value(values).is_some())
        {
            schedule.check_relation(values, constraint, &mut faults)?;
        }
        let tasks = schedule.tasks();
        if faults.is_empty() {
            return Ok(None);
        }
        let argument = |operand: &Operand| self.argument(operand, values);
        let tasks = (0..tasks.starts.len())
            .map(|i| SchedulingTask {
                position: i + 1,
                start: argument(&tasks.starts[i]),
                duration: argument(&tasks.durations[i]),
                end: tasks.ends.as_ref().and_then(|ends| argument(&ends[i])),
                usage: schedule.usages().and_then(|usages| argument(&usages[i])),
                present: schedule
                    .presence()
                    .and_then(|present| argument(&present[i])),
                machine: schedule
                    .machines()
                    .and_then(|machines| argument(&machines[i])),
            })
            .collect();
        Ok(Some(ViolationKind::Scheduling {
            finding: SchedulingFinding {
                tasks,
                capacities: schedule.capacities().iter().filter_map(argument).collect(),
                faults,
            },
        }))
    }
}

#[derive(Clone, Copy)]
struct ResourceLimit {
    machine: Option<i64>,
    bound: i64,
    upper: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResourceChecks {
    ActiveIntervals,
    ConcreteHorizon,
    ZeroDurationStarts,
    TaskStarts,
}

// Values are constant between boundaries, so even huge horizons need only task events.
fn check_resources(
    tasks: &[ConcreteTask],
    limit: ResourceLimit,
    checks: ResourceChecks,
    constraint: &Constraint,
    faults: &mut Vec<SchedulingFault>,
) -> Result<(), CheckError> {
    let tasks: Vec<_> = tasks
        .iter()
        .filter(|task| task.machine == limit.machine)
        .collect();
    let mut boundaries: Vec<_> = tasks
        .iter()
        .flat_map(|task| [task.start, task.end])
        .collect();
    boundaries.sort_unstable();
    boundaries.dedup();
    let horizon_end = tasks.iter().map(|task| task.end).max();
    let horizon_start = tasks.iter().map(|task| task.start).min();
    for time in boundaries {
        let active: Vec<_> = tasks
            .iter()
            .copied()
            .filter(|task| task.start <= time && time < task.end)
            .collect();
        if !active.is_empty()
            || (checks == ResourceChecks::ConcreteHorizon
                && horizon_start.is_some_and(|start| start <= time)
                && horizon_end.is_some_and(|end| time < end))
        {
            check_resource(&active, None, limit, time, constraint, faults)?;
        }
        if matches!(
            checks,
            ResourceChecks::ZeroDurationStarts | ResourceChecks::TaskStarts
        ) {
            let spanning: Vec<_> = tasks
                .iter()
                .copied()
                .filter(|task| task.start < time && time < task.end)
                .collect();
            for starting in tasks.iter().copied().filter(|task| {
                task.start == time && (checks == ResourceChecks::TaskStarts || task.zero_duration)
            }) {
                check_resource(&spanning, Some(starting), limit, time, constraint, faults)?;
            }
        }
    }
    Ok(())
}
fn check_resource(
    active: &[&ConcreteTask],
    starting: Option<&ConcreteTask>,
    limit: ResourceLimit,
    time: i64,
    constraint: &Constraint,
    faults: &mut Vec<SchedulingFault>,
) -> Result<(), CheckError> {
    let usage = checked_sum(
        active
            .iter()
            .copied()
            .chain(starting)
            .map(|task| (1, task.usage)),
    )
    .ok_or_else(|| constraint.execution_error("checked i64 overflow in scheduling resource sum"))?;
    let ResourceLimit {
        machine,
        bound,
        upper,
    } = limit;
    if (upper && usage > bound) || (!upper && usage < bound) {
        faults.push(SchedulingFault::Resource {
            machine,
            time,
            usage,
            bound,
            upper,
            tasks: active
                .iter()
                .copied()
                .chain(starting)
                .map(|task| task.position)
                .collect(),
        });
    }
    Ok(())
}
