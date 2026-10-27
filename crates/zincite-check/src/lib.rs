//! Check repeated candidates against a prepared FlatZinc model.
//! Preparation is optional; checking performs no I/O and starts no external tools.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

mod packing;
#[cfg(feature = "prepare")]
use packing::bin_packing_signature;
use packing::{BinPacking, is_bin_packing_name};
pub use packing::{BinPackingFault, BinPackingFinding, PackingItem, PackingLoad};
mod rectangles;
pub use rectangles::{RectangleArguments, RectangleDimension, RectangleFault, RectanglesFinding};
use rectangles::{RectangleName, Rectangles};
mod scheduling;
#[cfg(feature = "prepare")]
use scheduling::ScheduleTasks;
use scheduling::{Scheduling, SchedulingName};
pub use scheduling::{SchedulingFault, SchedulingFinding, SchedulingTask};
mod circuits;
use circuits::{Circuit, CircuitName};
pub use circuits::{CircuitArgument, CircuitFault, CircuitFinding};
mod ordering;
pub use ordering::{
    IndexFault, InverseFault, InverseSide, OrderingArgument, OrderingFinding, OrderingPair,
};
use ordering::{Ordering, OrderingName};
mod automata;
#[cfg(feature = "prepare")]
use automata::AutomatonEncoding;
use automata::{Automaton, AutomatonName};
pub use automata::{AutomatonFinding, AutomatonRejection};
mod tables;
pub use tables::TableData;
use tables::{Table, TableName};
mod counting;
pub use counting::{CountPosition, CountSelectorValue, WindowCount};
use counting::{Counting, CountingName};
mod boolean;
pub use boolean::BooleanOperation;
use boolean::{BooleanKind, BooleanName};
mod arithmetic;
pub use arithmetic::ArithmeticOperation;
mod relations;
pub use relations::IntegerRelation;
use relations::{RelationMode, RelationName, checked_sum};
mod floats;
use floats::FloatName;
pub use floats::{
    FiniteFloat, FloatConversion, FloatExtremum, FloatMembership, FloatOperation, FloatSet,
    FloatTolerance,
};
mod selection;
pub use selection::ElementLayout;
use selection::{SelectionForm, SelectionName, SelectionPosition};
mod serialized;
mod sets;
pub use sets::{IntegerSet, SetOperation, SetRelation};
use sets::{SetKind, SetName};

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
mod wasm;

#[cfg(feature = "prepare")]
mod prepare;
#[cfg(feature = "prepare")]
pub use prepare::{parse_assignment, prepare};
#[cfg(feature = "prepare")]
pub mod cli;

/// Mathematical integers use checked i64 arithmetic. JSON writes decimal strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Value {
    Float(FiniteFloat),
    FloatSet(FloatSet),
    IntegerSet(IntegerSet),
    Boolean(bool),
    Integer(
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        i64,
    ),
}

impl Value {
    fn float(&self) -> FiniteFloat {
        let Self::Float(value) = self else {
            unreachable!("admission validates float operands")
        };
        *value
    }

    fn integer(&self) -> i64 {
        let Self::Integer(value) = self else {
            unreachable!("preparation validates integer operands")
        };
        *value
    }

    fn boolean(&self) -> bool {
        let Self::Boolean(value) = self else {
            unreachable!("admission validates Boolean operands")
        };
        *value
    }

    fn membership_element(&self) -> i64 {
        match self {
            Self::Integer(value) => *value,
            Self::Boolean(value) => i64::from(*value),
            _ => unreachable!("admission validates membership elements"),
        }
    }

    fn set(&self) -> &IntegerSet {
        let Self::IntegerSet(value) = self else {
            unreachable!("preparation validates set operands")
        };
        value
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Float(value) => value.fmt(f),
            Self::FloatSet(value) => value.fmt(f),
            Self::Boolean(value) => value.fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::IntegerSet(value) => value.fmt(f),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ValueType {
    Float,
    FloatSet,
    IntegerSet,
    Boolean,
    Integer,
}

impl Value {
    fn value_type(&self) -> ValueType {
        match self {
            Self::Float(_) => ValueType::Float,
            Self::FloatSet(_) => ValueType::FloatSet,
            Self::Boolean(_) => ValueType::Boolean,
            Self::Integer(_) => ValueType::Integer,
            Self::IntegerSet(_) => ValueType::IntegerSet,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub source: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub location: Option<Location>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Application {
    pub id: String,
    #[serde(
        serialize_with = "serialize_integers",
        deserialize_with = "deserialize_integers"
    )]
    pub entities: Vec<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub application: Option<Application>,
    pub label: Option<String>,
    pub paths: Vec<String>,
    pub is_defined: bool,
    pub defines: Option<DefinitionTarget>,
    pub output: bool,
}

/// The declared name or explicit scalar names supplied to defines_var.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DefinitionTarget {
    Name(String),
    Elements(Vec<String>),
}

impl std::fmt::Display for DefinitionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => f.write_str(name),
            Self::Elements(names) => write!(f, "[{}]", names.join(", ")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableAlias {
    pub name: String,
    pub position: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableRef {
    pub name: String,
    /// One-based FlatZinc position for an array element.
    pub position: Option<usize>,
    pub location: Location,
    pub metadata: Metadata,
    pub aliases: Vec<VariableAlias>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintRef {
    pub ordinal: usize,
    pub predicate: String,
    pub location: Location,
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Domain {
    Float,
    FloatSet,
    FloatInterval {
        lower: FiniteFloat,
        upper: FiniteFloat,
    },
    IntegerSet {
        upper: Option<IntegerSet>,
    },
    Boolean,
    Integer,
    Interval {
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        lower: i64,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        upper: i64,
    },
    Enumeration {
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        values: Vec<i64>,
    },
    Fixed {
        value: Value,
    },
}

impl Domain {
    #[cfg(feature = "prepare")]
    fn contains(&self, value: &Value) -> bool {
        self.contains_with(value, FloatTolerance::new(0.0, 0.0).expect("exact policy"))
    }

    fn contains_with(&self, value: &Value, tolerance: FloatTolerance) -> bool {
        match (self, value) {
            (Self::Boolean, Value::Boolean(_))
            | (Self::Integer, Value::Integer(_))
            | (Self::Float, Value::Float(_))
            | (Self::FloatSet, Value::FloatSet(_)) => true,
            (Self::FloatInterval { lower, upper }, Value::Float(value)) => {
                tolerance.contains(*value, *lower, *upper)
            }
            (
                Self::Fixed {
                    value: Value::Float(fixed),
                },
                Value::Float(value),
            ) => tolerance.equal(*fixed, *value),
            (Self::Interval { lower, upper }, Value::Integer(value)) => {
                (*lower..=*upper).contains(value)
            }
            (Self::Enumeration { values }, Value::Integer(value)) => values.contains(value),
            (Self::Fixed { value: fixed }, value) => fixed == value,
            (Self::IntegerSet { upper }, Value::IntegerSet(value)) => {
                upper.as_ref().is_none_or(|upper| value.is_subset(upper))
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AssignmentValue {
    Scalar(Value),
    Array(Vec<Value>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentEntry {
    pub name: String,
    pub value: AssignmentValue,
    pub location: Option<Location>,
}

/// Entries keep their written order so duplicate names can be diagnosed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub entries: Vec<AssignmentEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Valid,
    Invalid,
    Incomplete,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CheckRef {
    Domain {
        variable: VariableRef,
        domain: Domain,
    },
    Constraint {
        constraint: ConstraintRef,
    },
}

impl CheckRef {
    pub fn location(&self) -> &Location {
        match self {
            Self::Domain { variable, .. } => &variable.location,
            Self::Constraint { constraint } => &constraint.location,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ArgumentValue {
    pub variable: Option<Box<VariableRef>>,
    pub value: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViolationKind {
    BinPacking {
        finding: BinPackingFinding,
    },
    Rectangles {
        finding: RectanglesFinding,
    },
    Scheduling {
        finding: SchedulingFinding,
    },
    Automaton {
        finding: AutomatonFinding,
    },
    Table {
        tuple: Vec<ArgumentValue>,
        table: TableData,
        width: usize,
        membership: bool,
        condition: RelationConditionValue,
    },
    Circuit {
        finding: CircuitFinding,
    },
    Ordering {
        finding: OrderingFinding,
    },
    Count {
        entries: Vec<ArgumentValue>,
        selector: CountSelectorValue,
        #[serde(serialize_with = "serialize_integer")]
        calculated: i64,
        supplied: ArgumentValue,
        relation: IntegerRelation,
        #[serde(serialize_with = "serialize_optional_integer")]
        code: Option<i64>,
        condition: RelationConditionValue,
    },
    Cardinality {
        entries: Vec<ArgumentValue>,
        counts: Vec<CountPosition>,
        outside_cover: Vec<usize>,
        closed: bool,
    },
    CountWindows {
        entries: Vec<ArgumentValue>,
        membership: Value,
        #[serde(serialize_with = "serialize_integer")]
        length: i64,
        #[serde(serialize_with = "serialize_integer")]
        lower: i64,
        #[serde(serialize_with = "serialize_integer")]
        upper: i64,
        windows: Vec<WindowCount>,
    },
    ElementRange {
        indices: Vec<Option<ArgumentValue>>,
        layout: ElementLayout,
        length: usize,
        result: Option<ArgumentValue>,
    },
    ElementMismatch {
        indices: Vec<ArgumentValue>,
        layout: ElementLayout,
        selected: ArgumentValue,
        result: ArgumentValue,
    },
    ConditionalMismatch {
        condition: ArgumentValue,
        selected: ArgumentValue,
        result: ArgumentValue,
    },
    FloatRelation {
        relation: IntegerRelation,
        left: ArgumentValue,
        right: ArgumentValue,
        condition: RelationConditionValue,
        #[serde(
            serialize_with = "serialize_optional_integer",
            deserialize_with = "deserialize_optional_integer"
        )]
        code: Option<i64>,
    },
    FloatLinear {
        relation: IntegerRelation,
        coefficients: Vec<FiniteFloat>,
        terms: Vec<ArgumentValue>,
        bound: FiniteFloat,
        condition: RelationConditionValue,
        sum: FiniteFloat,
    },
    FloatArithmetic {
        operation: FloatOperation,
        inputs: Vec<ArgumentValue>,
        result: ArgumentValue,
    },
    FloatIntegerPower {
        base: ArgumentValue,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        exponent: i64,
        result: ArgumentValue,
    },
    FloatArrayExtremum {
        extremum: FloatExtremum,
        inputs: Vec<ArgumentValue>,
        result: ArgumentValue,
    },
    FloatConversion {
        operation: FloatConversion,
        input: ArgumentValue,
        result: ArgumentValue,
    },
    FloatMembership {
        membership: FloatMembership,
        element: ArgumentValue,
        intervals: Vec<(FiniteFloat, FiniteFloat)>,
    },

    BooleanRelation {
        relation: IntegerRelation,
        left: ArgumentValue,
        right: ArgumentValue,
        condition: RelationConditionValue,
    },
    BooleanLinear {
        relation: IntegerRelation,
        #[serde(serialize_with = "serialize_integers")]
        coefficients: Vec<i64>,
        terms: Vec<ArgumentValue>,
        #[serde(serialize_with = "serialize_integer")]
        sum: i64,
        bound: ArgumentValue,
        condition: RelationConditionValue,
    },
    BooleanOperation {
        operation: BooleanOperation,
        inputs: Vec<ArgumentValue>,
        condition: RelationConditionValue,
    },
    BooleanClause {
        positive: Vec<ArgumentValue>,
        negative: Vec<ArgumentValue>,
        condition: RelationConditionValue,
    },
    BooleanToInteger {
        boolean: ArgumentValue,
        integer: ArgumentValue,
    },
    IntegerRelation {
        relation: IntegerRelation,
        left: ArgumentValue,
        right: ArgumentValue,
        condition: RelationConditionValue,
    },
    LinearRelation {
        relation: IntegerRelation,
        #[serde(serialize_with = "serialize_integers")]
        coefficients: Vec<i64>,
        terms: Vec<ArgumentValue>,
        #[serde(serialize_with = "serialize_integer")]
        sum: i64,
        #[serde(serialize_with = "serialize_integer")]
        bound: i64,
        condition: RelationConditionValue,
    },
    IntegerArithmetic {
        operation: ArithmeticOperation,
        inputs: Vec<ArgumentValue>,
        result: ArgumentValue,
    },
    SetRelation {
        relation: SetRelation,
        left: ArgumentValue,
        right: ArgumentValue,
        condition: RelationConditionValue,
        #[serde(serialize_with = "serialize_optional_integer")]
        native_code: Option<i64>,
    },
    SetMembership {
        element: ArgumentValue,
        set: ArgumentValue,
        condition: RelationConditionValue,
    },
    SetOperation {
        operation: SetOperation,
        left: ArgumentValue,
        right: ArgumentValue,
        result: ArgumentValue,
    },
    SetIn {
        element: ArgumentValue,
        set: ArgumentValue,
    },
    SetEqual {
        left: ArgumentValue,
        right: ArgumentValue,
    },
    SetSubset {
        left: ArgumentValue,
        right: ArgumentValue,
    },
    SetCardinality {
        set: ArgumentValue,
        cardinality: ArgumentValue,
    },
    SetUnion {
        left: ArgumentValue,
        right: ArgumentValue,
        result: ArgumentValue,
    },
    SetIntersection {
        left: ArgumentValue,
        right: ArgumentValue,
        result: ArgumentValue,
    },
    DisjunctiveStrict {
        tasks: Vec<TaskArguments>,
        conflicts: Vec<ConflictingPair>,
    },
    Domain {
        value: Value,
    },
    Linear {
        #[serde(serialize_with = "serialize_integers")]
        coefficients: Vec<i64>,
        terms: Vec<ArgumentValue>,
        #[serde(serialize_with = "serialize_integer")]
        sum: i64,
        #[serde(serialize_with = "serialize_integer")]
        bound: i64,
    },
    ReifiedLessEqual {
        left: ArgumentValue,
        right: ArgumentValue,
        result: ArgumentValue,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelationConditionValue {
    Plain,
    Reified { result: ArgumentValue },
    Implied { guard: ArgumentValue },
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskArguments {
    /// One-based position in the global's arrays.
    pub position: usize,
    pub start: ArgumentValue,
    pub duration: ArgumentValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConflictingPair {
    pub first: usize,
    pub second: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Violation {
    pub check: CheckRef,
    pub details: ViolationKind,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockedCheck {
    pub check: CheckRef,
    pub missing: Vec<VariableRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Completeness {
    pub complete: bool,
    pub unresolved_variables: Vec<VariableRef>,
    pub blocked_checks: Vec<BlockedCheck>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveDirection {
    Minimize,
    Maximize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObjectiveValue {
    pub direction: ObjectiveDirection,
    pub argument: ArgumentValue,
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
    pub status: Status,
    pub violations: Vec<Violation>,
    pub completeness: Completeness,
    pub objective: Option<ObjectiveValue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Assignment,
    Execution,
}

#[derive(Debug, Serialize)]
pub struct CheckError {
    pub kind: ErrorKind,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Operand {
    Literal(Value),
    Variable(usize),
}

impl Operand {
    fn value<'a>(&'a self, values: &'a [Option<Value>]) -> Option<&'a Value> {
        match self {
            Self::Literal(value) => Some(value),
            Self::Variable(slot) => values[*slot].as_ref(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Slot {
    kind: ValueType,
    default: Option<Value>,
    reference: VariableRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Binding {
    elements: Vec<Operand>,
    is_array: bool,
    kind: ValueType,
    assignable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Variable {
    reference: VariableRef,
    slot: usize,
    domains: Vec<Domain>,
}

#[derive(Debug, Serialize, Deserialize)]
enum Predicate {
    BinPacking(BinPacking),
    Rectangles(Rectangles),
    Scheduling(Scheduling),
    Automaton(Automaton),
    Circuit(Circuit),
    Ordering(Ordering),
    Counting(Counting),
    Table(Table),
    Element {
        indices: Vec<Operand>,
        elements: Vec<Operand>,
        result: Operand,
        layout: ElementLayout,
    },
    Conditional {
        condition: Operand,
        then_value: Operand,
        else_value: Operand,
        result: Operand,
    },
    FloatRelation {
        relation: IntegerRelation,
        left: Operand,
        right: Operand,
        condition: RelationCondition,
        #[serde(
            serialize_with = "serialize_optional_integer",
            deserialize_with = "deserialize_optional_integer"
        )]
        code: Option<i64>,
    },
    FloatLinear {
        relation: IntegerRelation,
        coefficients: Vec<FiniteFloat>,
        terms: Vec<Operand>,
        bound: FiniteFloat,
        condition: RelationCondition,
    },
    FloatArithmetic {
        operation: FloatOperation,
        inputs: Vec<Operand>,
        result: Operand,
    },
    FloatIntegerPower {
        base: Operand,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        exponent: i64,
        result: Operand,
    },
    FloatArrayExtremum {
        extremum: FloatExtremum,
        inputs: Vec<Operand>,
        result: Operand,
    },
    FloatConversion {
        operation: FloatConversion,
        input: Operand,
        result: Operand,
    },
    FloatMembership {
        membership: FloatMembership,
        element: Operand,
        intervals: Vec<(FiniteFloat, FiniteFloat)>,
    },

    BooleanRelation {
        relation: IntegerRelation,
        left: Operand,
        right: Operand,
        condition: RelationCondition,
    },
    BooleanLinear {
        relation: IntegerRelation,
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        coefficients: Vec<i64>,
        terms: Vec<Operand>,
        bound: Operand,
        condition: RelationCondition,
    },
    BooleanOperation {
        operation: BooleanOperation,
        inputs: Vec<Operand>,
        condition: RelationCondition,
    },
    BooleanClause {
        positive: Vec<Operand>,
        negative: Vec<Operand>,
        condition: RelationCondition,
    },
    BooleanToInteger {
        boolean: Operand,
        integer: Operand,
    },
    IntegerRelation {
        relation: IntegerRelation,
        left: Operand,
        right: Operand,
        condition: RelationCondition,
    },
    LinearRelation {
        relation: IntegerRelation,
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        coefficients: Vec<i64>,
        terms: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        bound: i64,
        condition: RelationCondition,
    },
    IntegerArithmetic {
        operation: ArithmeticOperation,
        inputs: Vec<Operand>,
        result: Operand,
    },
    SetRelation {
        relation: SetRelation,
        left: Operand,
        right: Operand,
        condition: RelationCondition,
        #[serde(
            serialize_with = "serialize_optional_integer",
            deserialize_with = "deserialize_optional_integer"
        )]
        native_code: Option<i64>,
    },
    SetMembership {
        element: Operand,
        set: Operand,
        condition: RelationCondition,
    },
    SetOperation {
        operation: SetOperation,
        left: Operand,
        right: Operand,
        result: Operand,
    },
    SetIn {
        element: Operand,
        set: Operand,
    },
    SetEqual {
        left: Operand,
        right: Operand,
    },
    SetSubset {
        left: Operand,
        right: Operand,
    },
    SetCardinality {
        set: Operand,
        cardinality: Operand,
    },
    SetUnion {
        left: Operand,
        right: Operand,
        result: Operand,
    },
    SetIntersection {
        left: Operand,
        right: Operand,
        result: Operand,
    },
    DisjunctiveStrict {
        starts: Vec<Operand>,
        durations: Vec<Operand>,
    },
    Linear {
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        coefficients: Vec<i64>,
        terms: Vec<Operand>,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        bound: i64,
        equality: bool,
    },
    ReifiedLessEqual {
        left: Operand,
        right: Operand,
        result: Operand,
    },
}

#[derive(Debug, Serialize, Deserialize)]
enum RelationCondition {
    Plain,
    Reified { result: Operand },
    Implied { guard: Operand },
}

impl RelationCondition {
    fn mode(&self) -> RelationMode {
        match self {
            Self::Plain => RelationMode::Plain,
            Self::Reified { .. } => RelationMode::Reified,
            Self::Implied { .. } => RelationMode::Implied,
        }
    }

    fn operand(&self) -> Option<&Operand> {
        match self {
            Self::Plain => None,
            Self::Reified { result } => Some(result),
            Self::Implied { guard } => Some(guard),
        }
    }

    fn inactive(&self, values: &[Option<Value>]) -> bool {
        matches!(self, Self::Implied { guard } if guard.value(values) == Some(&Value::Boolean(false)))
    }

    fn satisfied(&self, relation: bool, values: &[Option<Value>]) -> bool {
        match self {
            Self::Plain => relation,
            Self::Reified { result } => result.value(values) == Some(&Value::Boolean(relation)),
            Self::Implied { .. } => self.inactive(values) || relation,
        }
    }
}

impl Predicate {
    fn operands(&self) -> Vec<&Operand> {
        match self {
            Self::Automaton(automaton) => automaton.word.iter().collect(),
            Self::BinPacking(packing) => packing.operands(),
            Self::Rectangles(rectangles) => rectangles.operands(),
            Self::Scheduling(schedule) => schedule.operands(),
            Self::Circuit(circuit) => circuit.operands(),
            Self::Ordering(ordering) => ordering.operands(),
            Self::Counting(counting) => counting.operands(false),
            Self::Table(table) => table.operands(),
            Self::Element {
                indices,
                elements,
                result,
                ..
            } => indices
                .iter()
                .chain(elements)
                .chain(std::iter::once(result))
                .collect(),
            Self::Conditional {
                condition,
                then_value,
                else_value,
                result,
            } => vec![condition, then_value, else_value, result],
            Self::SetRelation {
                left,
                right,
                condition,
                ..
            }
            | Self::FloatRelation {
                left,
                right,
                condition,
                ..
            }
            | Self::BooleanRelation {
                left,
                right,
                condition,
                ..
            }
            | Self::IntegerRelation {
                left,
                right,
                condition,
                ..
            } => [left, right]
                .into_iter()
                .chain(condition.operand())
                .collect(),
            Self::BooleanLinear {
                terms,
                bound,
                condition,
                ..
            } => terms
                .iter()
                .chain(std::iter::once(bound))
                .chain(condition.operand())
                .collect(),
            Self::BooleanOperation {
                inputs, condition, ..
            } => inputs.iter().chain(condition.operand()).collect(),
            Self::BooleanClause {
                positive,
                negative,
                condition,
            } => positive
                .iter()
                .chain(negative)
                .chain(condition.operand())
                .collect(),
            Self::BooleanToInteger { boolean, integer } => vec![boolean, integer],
            Self::FloatLinear {
                terms, condition, ..
            }
            | Self::LinearRelation {
                terms, condition, ..
            } => terms.iter().chain(condition.operand()).collect(),
            Self::FloatArithmetic { inputs, result, .. }
            | Self::FloatArrayExtremum { inputs, result, .. }
            | Self::IntegerArithmetic { inputs, result, .. } => {
                inputs.iter().chain(std::iter::once(result)).collect()
            }
            Self::FloatIntegerPower { base, result, .. } => vec![base, result],
            Self::FloatConversion { input, result, .. } => vec![input, result],
            Self::FloatMembership { element, .. } => vec![element],
            Self::Linear { terms, .. } => terms.iter().collect(),
            Self::SetOperation {
                left,
                right,
                result,
                ..
            }
            | Self::ReifiedLessEqual {
                left,
                right,
                result,
            }
            | Self::SetUnion {
                left,
                right,
                result,
            }
            | Self::SetIntersection {
                left,
                right,
                result,
            } => vec![left, right, result],
            Self::SetMembership {
                element,
                set,
                condition,
            } => [element, set]
                .into_iter()
                .chain(condition.operand())
                .collect(),
            Self::SetIn { element, set } => vec![element, set],
            Self::SetEqual { left, right } | Self::SetSubset { left, right } => vec![left, right],
            Self::SetCardinality { set, cardinality } => vec![set, cardinality],
            Self::DisjunctiveStrict { starts, durations } => {
                starts.iter().chain(durations).collect()
            }
        }
    }

    fn set_truth(&self, values: &[Option<Value>]) -> Option<bool> {
        match self {
            Self::SetRelation {
                relation,
                left,
                right,
                ..
            } => Some(relation.holds(left.value(values)?.set(), right.value(values)?.set())),
            Self::SetMembership { element, set, .. } => Some(
                set.value(values)?
                    .set()
                    .contains(element.value(values)?.membership_element()),
            ),
            _ => unreachable!("only conditioned set predicates calculate truth"),
        }
    }

    fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        match self {
            Self::BinPacking(packing) => packing.required_operands(values),
            Self::Rectangles(rectangles) => rectangles.required_operands(values),
            Self::Scheduling(schedule) => schedule.required_operands(values),
            Self::Circuit(circuit) => circuit.required_operands(values),
            Self::Ordering(ordering) => ordering.required_operands(values),
            Self::Counting(counting) => counting.operands(true),
            Self::Element {
                indices,
                elements,
                result,
                layout,
            } => match layout.position(indices, elements.len(), values) {
                SelectionPosition::Missing => indices.iter().collect(),
                SelectionPosition::OutOfRange => Vec::new(),
                SelectionPosition::Selected(position) => vec![&elements[position], result],
            },
            Self::Conditional {
                condition, result, ..
            } => match self.selected_operand(values) {
                Some(selected) => vec![selected, result],
                None => vec![condition],
            },
            _ => self.operands(),
        }
    }

    fn inactive_implication(&self, values: &[Option<Value>]) -> bool {
        match self {
            Self::Table(table) => table.condition.inactive(values),
            Self::Counting(Counting::Count { condition, .. }) => condition.inactive(values),
            Self::SetRelation { condition, .. }
            | Self::SetMembership { condition, .. }
            | Self::FloatRelation { condition, .. }
            | Self::FloatLinear { condition, .. }
            | Self::BooleanRelation { condition, .. }
            | Self::BooleanLinear { condition, .. }
            | Self::BooleanOperation { condition, .. }
            | Self::BooleanClause { condition, .. }
            | Self::IntegerRelation { condition, .. }
            | Self::LinearRelation { condition, .. } => condition.inactive(values),
            _ => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Constraint {
    reference: ConstraintRef,
    predicate: Predicate,
}

impl Constraint {
    fn execution_error(&self, message: &str) -> CheckError {
        CheckError {
            kind: ErrorKind::Execution,
            diagnostics: vec![Diagnostic {
                location: Some(self.reference.location.clone()),
                message: message.into(),
            }],
        }
    }
    fn linear_overflow(&self) -> CheckError {
        CheckError {
            kind: ErrorKind::Execution,
            diagnostics: vec![Diagnostic {
                location: Some(self.reference.location.clone()),
                message: "checked i64 overflow in linear constraint".into(),
            }],
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum DefinitionDirection {
    BinPacking,
    Table,
    Circuit,
    Ordering,
    Counting,
    Selection,
    FloatEqual { input: usize },
    FloatReified,
    FloatLinear { term: usize },
    FloatArithmetic,
    FloatIntegerPower,
    FloatArrayExtremum,
    FloatConversion,
    BooleanCopy { input: usize, complement: bool },
    BooleanResult,
    BooleanLinearBound,
    BooleanLinearTerm { term: usize },
    BooleanToInteger,
    IntegerToBoolean,
    IntegerEqual { input: usize },
    ReifiedRelation,
    IntegerArithmetic,
    Linear { term: usize },
    ReifiedLessEqual,
    SetEqual { input: usize },
    SetCardinality,
    SetUnion,
    SetIntersection,
    SetOperation,
    SetReified,
}

#[derive(Debug)]
struct Definition {
    constraint: usize,
    output: usize,
    direction: DefinitionDirection,
}

/// An executable model. Its fields are private so checking uses validated inputs.
/// Export with `to_json`; `from_json` validates the executable model before use.
#[derive(Debug, Serialize)]
pub struct PreparedModel {
    float_tolerance: FloatTolerance,
    slots: Vec<Slot>,
    bindings: HashMap<String, Binding>,
    variables: Vec<Variable>,
    constraints: Vec<Constraint>,
    #[serde(skip)]
    definitions: Vec<Definition>,
    #[serde(skip)]
    dependents: Vec<Vec<usize>>,
    objective: Option<(ObjectiveDirection, Operand)>,
}

impl PreparedModel {
    pub fn float_tolerance(&self) -> FloatTolerance {
        self.float_tolerance
    }

    pub fn set_float_tolerance(&mut self, tolerance: FloatTolerance) {
        self.float_tolerance = tolerance;
    }

    /// Check every available domain and relation using fresh candidate values.
    /// Missing values produce a normal report, including any known violations.
    pub fn check(&self, assignment: &Assignment) -> Result<CheckReport, CheckError> {
        let mut values = self.assign(assignment)?;
        self.complete(&mut values)?;
        let unresolved_variables: Vec<_> = self
            .variables
            .iter()
            .filter(|variable| values[variable.slot].is_none())
            .map(|variable| variable.reference.clone())
            .collect();
        let mut violations = Vec::new();
        let mut blocked_checks = Vec::new();
        for variable in &self.variables {
            for domain in &variable.domains {
                let check = CheckRef::Domain {
                    variable: variable.reference.clone(),
                    domain: domain.clone(),
                };
                match &values[variable.slot] {
                    Some(value) if !domain.contains_with(value, self.float_tolerance) => violations
                        .push(Violation {
                            check,
                            details: ViolationKind::Domain {
                                value: value.clone(),
                            },
                        }),
                    None => blocked_checks.push(BlockedCheck {
                        check,
                        missing: vec![variable.reference.clone()],
                    }),
                    _ => {}
                }
            }
        }
        for constraint in &self.constraints {
            if constraint.predicate.inactive_implication(&values) {
                continue;
            }
            let check = CheckRef::Constraint {
                constraint: constraint.reference.clone(),
            };
            let missing = self.missing(&constraint.predicate, &values);
            if !missing.is_empty() {
                blocked_checks.push(BlockedCheck { check, missing });
                continue;
            }
            if let Some(details) = self.evaluate(constraint, &values)? {
                violations.push(Violation { check, details });
            }
        }
        violations.sort_by_key(|violation| violation.check.location().start);
        blocked_checks.sort_by_key(|blocked| blocked.check.location().start);
        let complete = unresolved_variables.is_empty() && blocked_checks.is_empty();
        let status = if !violations.is_empty() {
            Status::Invalid
        } else if !complete {
            Status::Incomplete
        } else {
            Status::Valid
        };
        let objective = self.objective.as_ref().and_then(|(direction, operand)| {
            self.argument(operand, &values)
                .map(|argument| ObjectiveValue {
                    direction: *direction,
                    argument,
                })
        });
        Ok(CheckReport {
            status,
            violations,
            completeness: Completeness {
                complete,
                unresolved_variables,
                blocked_checks,
            },
            objective,
        })
    }

    fn assign(&self, assignment: &Assignment) -> Result<Vec<Option<Value>>, CheckError> {
        let mut values = vec![None; self.slots.len()];
        let mut names = HashSet::new();
        for entry in &assignment.entries {
            let fail = |message| CheckError {
                kind: ErrorKind::Assignment,
                diagnostics: vec![Diagnostic {
                    location: entry.location.clone(),
                    message,
                }],
            };
            if !names.insert(&entry.name) {
                return Err(fail(format!("duplicate assignment to '{}'", entry.name)));
            }
            let binding = self
                .bindings
                .get(&entry.name)
                .ok_or_else(|| fail(format!("unknown variable '{}'", entry.name)))?;
            if !binding.assignable {
                return Err(fail(format!(
                    "'{}' is a parameter, not a candidate variable",
                    entry.name
                )));
            }
            let supplied = match (&entry.value, binding.is_array) {
                (AssignmentValue::Scalar(value), false) => std::slice::from_ref(value),
                (AssignmentValue::Array(values), true) => values.as_slice(),
                _ => {
                    return Err(fail(format!(
                        "assignment shape does not match '{}'",
                        entry.name
                    )));
                }
            };
            if supplied.len() != binding.elements.len() {
                return Err(fail(format!(
                    "'{}' expects {} values, got {}",
                    entry.name,
                    binding.elements.len(),
                    supplied.len()
                )));
            }
            for (operand, value) in binding.elements.iter().zip(supplied) {
                if value.value_type() != binding.kind {
                    return Err(fail(format!(
                        "assignment type does not match '{}'",
                        entry.name
                    )));
                }
                match operand {
                    Operand::Variable(slot) => {
                        if values[*slot]
                            .as_ref()
                            .is_some_and(|previous| previous != value)
                        {
                            return Err(fail(format!(
                                "contradictory assignments through alias '{}'",
                                entry.name
                            )));
                        }
                        values[*slot] = Some(value.clone());
                    }
                    Operand::Literal(_) => unreachable!("variable bindings contain value slots"),
                }
            }
        }
        for (value, slot) in values.iter_mut().zip(&self.slots) {
            if value.is_none() {
                *value = slot.default.clone();
            }
        }
        Ok(values)
    }

    fn complete(&self, values: &mut [Option<Value>]) -> Result<(), CheckError> {
        let mut pending: VecDeque<_> = (0..self.definitions.len()).collect();
        while let Some(index) = pending.pop_front() {
            let definition = &self.definitions[index];
            if values[definition.output].is_some() {
                continue;
            }
            if let Some(value) = self.derive(definition, values)? {
                values[definition.output] = Some(value);
                pending.extend(self.dependents[definition.output].iter().copied());
            }
        }
        Ok(())
    }

    fn derive(
        &self,
        definition: &Definition,
        values: &[Option<Value>],
    ) -> Result<Option<Value>, CheckError> {
        let constraint = &self.constraints[definition.constraint];
        match (&constraint.predicate, definition.direction) {
            (Predicate::BinPacking(packing), DefinitionDirection::BinPacking) => {
                packing.derive(definition.output, values, constraint)
            }
            (Predicate::Table(table), DefinitionDirection::Table) => {
                Ok(table.membership(values).map(Value::Boolean))
            }
            (Predicate::Circuit(circuit), DefinitionDirection::Circuit) => {
                circuit.derive(definition.output, values, constraint)
            }
            (Predicate::Ordering(ordering), DefinitionDirection::Ordering) => {
                ordering.derive(definition.output, values, constraint)
            }
            (Predicate::Counting(counting), DefinitionDirection::Counting) => {
                counting.derive(definition.output, values, constraint)
            }
            (predicate, DefinitionDirection::Selection) => Ok(predicate
                .selected_operand(values)
                .and_then(|operand| operand.value(values))
                .cloned()),
            (
                Predicate::FloatRelation { left, right, .. },
                DefinitionDirection::FloatEqual { input },
            ) => Ok([left, right][input].value(values).cloned()),
            (
                Predicate::FloatRelation {
                    relation,
                    left,
                    right,
                    ..
                },
                DefinitionDirection::FloatReified,
            ) => {
                let (Some(left), Some(right)) = (left.value(values), right.value(values)) else {
                    return Ok(None);
                };
                Ok(Some(Value::Boolean(self.float_tolerance.holds(
                    *relation,
                    left.float(),
                    right.float(),
                ))))
            }
            (
                Predicate::FloatLinear {
                    relation,
                    coefficients,
                    terms,
                    bound,
                    ..
                },
                DefinitionDirection::FloatReified,
            ) => Ok(self
                .float_sum(constraint, coefficients, terms, None, values)?
                .map(|sum| Value::Boolean(self.float_tolerance.holds(*relation, sum, *bound)))),
            (
                Predicate::FloatLinear {
                    coefficients,
                    terms,
                    bound,
                    ..
                },
                DefinitionDirection::FloatLinear { term },
            ) => {
                let Some(sum) =
                    self.float_sum(constraint, coefficients, terms, Some(term), values)?
                else {
                    return Ok(None);
                };
                let numerator = floats::finite_result(bound.get() - sum.get())
                    .map_err(|message| constraint.execution_error(message))?;
                let value = floats::finite_result(numerator.get() / coefficients[term].get())
                    .map_err(|message| constraint.execution_error(message))?;
                Ok(Some(Value::Float(value)))
            }
            (
                Predicate::FloatArithmetic {
                    operation, inputs, ..
                },
                DefinitionDirection::FloatArithmetic,
            ) => {
                let Some(inputs) = inputs
                    .iter()
                    .map(|input| input.value(values).map(Value::float))
                    .collect::<Option<Vec<_>>>()
                else {
                    return Ok(None);
                };
                Ok(Some(Value::Float(
                    operation
                        .calculate(&inputs)
                        .map_err(|message| constraint.execution_error(message))?,
                )))
            }
            (
                Predicate::FloatIntegerPower { base, exponent, .. },
                DefinitionDirection::FloatIntegerPower,
            ) => base
                .value(values)
                .map(|base| {
                    floats::integer_power(base.float(), *exponent)
                        .map(Value::Float)
                        .map_err(|message| constraint.execution_error(message))
                })
                .transpose(),
            (
                Predicate::FloatArrayExtremum {
                    extremum, inputs, ..
                },
                DefinitionDirection::FloatArrayExtremum,
            ) => Ok(inputs
                .iter()
                .map(|input| input.value(values).map(Value::float))
                .collect::<Option<Vec<_>>>()
                .map(|inputs| Value::Float(extremum.calculate(&inputs)))),
            (
                Predicate::FloatConversion {
                    operation, input, ..
                },
                DefinitionDirection::FloatConversion,
            ) => input
                .value(values)
                .map(|input| {
                    operation
                        .calculate(input)
                        .map_err(|message| constraint.execution_error(message))
                })
                .transpose(),

            (
                Predicate::BooleanRelation { left, right, .. },
                DefinitionDirection::BooleanCopy { input, complement },
            ) => Ok([left, right][input]
                .value(values)
                .map(|value| Value::Boolean(value.boolean() ^ complement))),
            (
                Predicate::BooleanOperation {
                    inputs, condition, ..
                },
                DefinitionDirection::BooleanCopy { input, complement },
            ) => {
                let operands: Vec<_> = inputs.iter().chain(condition.operand()).collect();
                Ok(operands[input]
                    .value(values)
                    .map(|value| Value::Boolean(value.boolean() ^ complement)))
            }
            (
                Predicate::BooleanRelation {
                    relation,
                    left,
                    right,
                    ..
                },
                DefinitionDirection::ReifiedRelation,
            ) => {
                let (Some(left), Some(right)) = (left.value(values), right.value(values)) else {
                    return Ok(None);
                };
                Ok(Some(Value::Boolean(relation.holds(
                    i64::from(left.boolean()),
                    i64::from(right.boolean()),
                ))))
            }
            (
                Predicate::BooleanLinear {
                    relation,
                    coefficients,
                    terms,
                    bound,
                    ..
                },
                DefinitionDirection::ReifiedRelation,
            ) => {
                let Some(bound) = bound.value(values) else {
                    return Ok(None);
                };
                Ok(self
                    .linear_sum(constraint, coefficients, terms, None, values)?
                    .map(|sum| Value::Boolean(relation.holds(sum, bound.integer()))))
            }
            (
                Predicate::BooleanOperation {
                    operation, inputs, ..
                },
                DefinitionDirection::BooleanResult,
            ) => Ok(boolean_values(inputs, values)
                .map(|inputs| Value::Boolean(operation.calculate(&inputs)))),
            (
                Predicate::BooleanClause {
                    positive, negative, ..
                },
                DefinitionDirection::BooleanResult,
            ) => {
                let (Some(positive), Some(negative)) = (
                    boolean_values(positive, values),
                    boolean_values(negative, values),
                ) else {
                    return Ok(None);
                };
                Ok(Some(Value::Boolean(boolean::clause(&positive, &negative))))
            }
            (
                Predicate::BooleanLinear {
                    coefficients,
                    terms,
                    ..
                },
                DefinitionDirection::BooleanLinearBound,
            ) => Ok(self
                .linear_sum(constraint, coefficients, terms, None, values)?
                .map(Value::Integer)),
            (
                Predicate::BooleanLinear {
                    coefficients,
                    terms,
                    bound,
                    ..
                },
                DefinitionDirection::BooleanLinearTerm { term },
            ) => {
                let Some(bound) = bound.value(values) else {
                    return Ok(None);
                };
                let Some(sum) =
                    self.linear_sum(constraint, coefficients, terms, Some(term), values)?
                else {
                    return Ok(None);
                };
                let numerator = bound
                    .integer()
                    .checked_sub(sum)
                    .ok_or_else(|| constraint.linear_overflow())?;
                let coefficient = coefficients[term];
                let remainder = numerator
                    .checked_rem(coefficient)
                    .ok_or_else(|| constraint.linear_overflow())?;
                if remainder != 0 {
                    return Ok(None);
                }
                let value = numerator
                    .checked_div(coefficient)
                    .ok_or_else(|| constraint.linear_overflow())?;
                Ok(boolean::from_integer(value).map(Value::Boolean))
            }
            (
                Predicate::BooleanToInteger { boolean, .. },
                DefinitionDirection::BooleanToInteger,
            ) => Ok(boolean
                .value(values)
                .map(|value| Value::Integer(i64::from(value.boolean())))),
            (
                Predicate::BooleanToInteger { integer, .. },
                DefinitionDirection::IntegerToBoolean,
            ) => Ok(integer
                .value(values)
                .and_then(|value| boolean::from_integer(value.integer()))
                .map(Value::Boolean)),

            (
                Predicate::IntegerRelation { left, right, .. },
                DefinitionDirection::IntegerEqual { input },
            ) => Ok([left, right][input].value(values).cloned()),
            (
                Predicate::IntegerRelation {
                    relation,
                    left,
                    right,
                    ..
                },
                DefinitionDirection::ReifiedRelation,
            ) => {
                let (Some(left), Some(right)) = (left.value(values), right.value(values)) else {
                    return Ok(None);
                };
                Ok(Some(Value::Boolean(
                    relation.holds(left.integer(), right.integer()),
                )))
            }
            (
                Predicate::LinearRelation {
                    relation,
                    coefficients,
                    terms,
                    bound,
                    ..
                },
                DefinitionDirection::ReifiedRelation,
            ) => Ok(self
                .linear_sum(constraint, coefficients, terms, None, values)?
                .map(|sum| Value::Boolean(relation.holds(sum, *bound)))),
            (
                Predicate::IntegerArithmetic {
                    operation, inputs, ..
                },
                DefinitionDirection::IntegerArithmetic,
            ) => {
                let Some(inputs) = inputs
                    .iter()
                    .map(|input| input.value(values).map(Value::integer))
                    .collect::<Option<Vec<_>>>()
                else {
                    return Ok(None);
                };
                let value = operation
                    .calculate(&inputs)
                    .map_err(|message| constraint.execution_error(message))?;
                Ok(Some(Value::Integer(value)))
            }
            (
                Predicate::Linear {
                    coefficients,
                    terms,
                    bound,
                    ..
                }
                | Predicate::LinearRelation {
                    coefficients,
                    terms,
                    bound,
                    ..
                },
                DefinitionDirection::Linear { term: output_term },
            ) => {
                let Some(sum) =
                    self.linear_sum(constraint, coefficients, terms, Some(output_term), values)?
                else {
                    return Ok(None);
                };
                let numerator = bound
                    .checked_sub(sum)
                    .ok_or_else(|| constraint.linear_overflow())?;
                let coefficient = coefficients[output_term];
                let remainder = numerator
                    .checked_rem(coefficient)
                    .ok_or_else(|| constraint.linear_overflow())?;
                if remainder != 0 {
                    return Ok(None);
                }
                let value = numerator
                    .checked_div(coefficient)
                    .ok_or_else(|| constraint.linear_overflow())?;
                Ok(Some(Value::Integer(value)))
            }
            (
                Predicate::ReifiedLessEqual { left, right, .. },
                DefinitionDirection::ReifiedLessEqual,
            ) => {
                let (Some(left), Some(right)) = (left.value(values), right.value(values)) else {
                    return Ok(None);
                };
                let (Value::Integer(left), Value::Integer(right)) = (left, right) else {
                    unreachable!("preparation validates reified operands")
                };
                Ok(Some(Value::Boolean(left <= right)))
            }
            (
                Predicate::SetRelation { .. } | Predicate::SetMembership { .. },
                DefinitionDirection::SetReified,
            ) => Ok(constraint.predicate.set_truth(values).map(Value::Boolean)),
            (
                Predicate::SetOperation {
                    operation,
                    left,
                    right,
                    ..
                },
                DefinitionDirection::SetOperation,
            ) => {
                let (Some(left), Some(right)) = (left.value(values), right.value(values)) else {
                    return Ok(None);
                };
                Ok(Some(Value::IntegerSet(
                    operation.calculate(left.set(), right.set()),
                )))
            }
            (Predicate::SetEqual { left, right }, DefinitionDirection::SetEqual { input }) => {
                Ok([left, right][input].value(values).cloned())
            }
            (Predicate::SetCardinality { set, .. }, DefinitionDirection::SetCardinality) => set
                .value(values)
                .map(|value| {
                    value
                        .set()
                        .cardinality()
                        .map(Value::Integer)
                        .ok_or_else(|| {
                            constraint.execution_error("checked i64 overflow in set cardinality")
                        })
                })
                .transpose(),
            (Predicate::SetUnion { left, right, .. }, DefinitionDirection::SetUnion)
            | (
                Predicate::SetIntersection { left, right, .. },
                DefinitionDirection::SetIntersection,
            ) => {
                let (Some(left), Some(right)) = (left.value(values), right.value(values)) else {
                    return Ok(None);
                };
                let set = if matches!(definition.direction, DefinitionDirection::SetUnion) {
                    left.set().union(right.set())
                } else {
                    left.set().intersection(right.set())
                };
                Ok(Some(Value::IntegerSet(set)))
            }
            _ => unreachable!("preparation selects supported definition directions"),
        }
    }

    fn argument(&self, operand: &Operand, values: &[Option<Value>]) -> Option<ArgumentValue> {
        let value = operand.value(values)?;
        Some(ArgumentValue {
            variable: match operand {
                Operand::Literal(_) => None,
                Operand::Variable(slot) => Some(Box::new(self.slots[*slot].reference.clone())),
            },
            value: value.clone(),
        })
    }

    fn float_sum(
        &self,
        constraint: &Constraint,
        coefficients: &[FiniteFloat],
        terms: &[Operand],
        skip: Option<usize>,
        values: &[Option<Value>],
    ) -> Result<Option<FiniteFloat>, CheckError> {
        let terms = coefficients
            .iter()
            .zip(terms)
            .enumerate()
            .filter(|(index, _)| Some(*index) != skip)
            .map(|(_, (&coefficient, term))| {
                term.value(values).map(|value| (coefficient, value.float()))
            })
            .collect::<Option<Vec<_>>>();
        terms
            .map(|terms| floats::sum(terms).map_err(|message| constraint.execution_error(message)))
            .transpose()
    }

    fn linear_sum(
        &self,
        constraint: &Constraint,
        coefficients: &[i64],
        terms: &[Operand],
        skip: Option<usize>,
        values: &[Option<Value>],
    ) -> Result<Option<i64>, CheckError> {
        let terms = coefficients
            .iter()
            .zip(terms)
            .enumerate()
            .filter(|(index, _)| Some(*index) != skip)
            .map(|(_, (&coefficient, term))| {
                term.value(values).map(|value| {
                    (
                        coefficient,
                        match value {
                            Value::Integer(value) => *value,
                            Value::Boolean(value) => i64::from(*value),
                            Value::IntegerSet(_) | Value::Float(_) | Value::FloatSet(_) => {
                                unreachable!("admission validates linear terms")
                            }
                        },
                    )
                })
            })
            .collect::<Option<Vec<_>>>();
        terms
            .map(|terms| checked_sum(terms).ok_or_else(|| constraint.linear_overflow()))
            .transpose()
    }

    fn condition_value(
        &self,
        condition: &RelationCondition,
        values: &[Option<Value>],
    ) -> RelationConditionValue {
        match condition {
            RelationCondition::Plain => RelationConditionValue::Plain,
            RelationCondition::Reified { result } => RelationConditionValue::Reified {
                result: self
                    .argument(result, values)
                    .expect("available reification result"),
            },
            RelationCondition::Implied { guard } => RelationConditionValue::Implied {
                guard: self
                    .argument(guard, values)
                    .expect("available implication guard"),
            },
        }
    }

    fn selected_matches_result(
        &self,
        selected: &Operand,
        result: &Operand,
        values: &[Option<Value>],
    ) -> bool {
        let selected = selected.value(values).expect("available selected input");
        let result = result.value(values).expect("available selection result");
        match (selected, result) {
            (Value::Float(left), Value::Float(right)) => {
                self.float_tolerance
                    .holds(IntegerRelation::Equal, *left, *right)
            }
            _ => selected == result,
        }
    }

    fn missing(&self, predicate: &Predicate, values: &[Option<Value>]) -> Vec<VariableRef> {
        let operands = predicate.required_operands(values);
        let mut slots = HashSet::new();
        operands
            .iter()
            .filter_map(|operand| match operand {
                Operand::Variable(slot) if values[*slot].is_none() && slots.insert(*slot) => {
                    Some(self.slots[*slot].reference.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn evaluate_disjunctive(
        &self,
        constraint: &Constraint,
        starts: &[Operand],
        durations: &[Operand],
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let integer = |operand: &Operand| {
            operand
                .value(values)
                .expect("available global operand")
                .integer()
        };
        let ends: Vec<_> = starts
            .iter()
            .zip(durations)
            .map(|(start, duration)| {
                integer(start)
                    .checked_add(integer(duration))
                    .ok_or_else(|| {
                        constraint
                            .execution_error("checked i64 overflow in strict disjunctive endpoint")
                    })
            })
            .collect::<Result<_, _>>()?;
        let mut conflicts = Vec::new();
        for (first, a) in starts.iter().enumerate() {
            for (second, b) in starts.iter().enumerate().skip(first + 1) {
                if ends[first] > integer(b) && ends[second] > integer(a) {
                    conflicts.push(ConflictingPair {
                        first: first + 1,
                        second: second + 1,
                    });
                }
            }
        }
        Ok((!conflicts.is_empty()).then(|| {
            let tasks = starts
                .iter()
                .zip(durations)
                .enumerate()
                .map(|(index, (start, duration))| TaskArguments {
                    position: index + 1,
                    start: self
                        .argument(start, values)
                        .expect("available global start"),
                    duration: self
                        .argument(duration, values)
                        .expect("available global duration"),
                })
                .collect();
            ViolationKind::DisjunctiveStrict { tasks, conflicts }
        }))
    }

    fn evaluate(
        &self,
        constraint: &Constraint,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let argument = |operand| {
            self.argument(operand, values)
                .expect("blocked constraints are excluded before evaluation")
        };
        match &constraint.predicate {
            Predicate::BinPacking(packing) => self.evaluate_packing(constraint, packing, values),
            Predicate::Rectangles(rectangles) => {
                self.evaluate_rectangles(constraint, rectangles, values)
            }
            Predicate::Scheduling(schedule) => self.evaluate_schedule(constraint, schedule, values),
            Predicate::Circuit(circuit) => self.evaluate_circuit(constraint, circuit, values),
            Predicate::Ordering(ordering) => self.evaluate_ordering(constraint, ordering, values),
            Predicate::Counting(counting) => self.evaluate_counting(constraint, counting, values),
            Predicate::Automaton(automaton) => Ok(self.evaluate_automaton(automaton, values)),
            Predicate::Table(table) => Ok(self.evaluate_table(table, values)),
            Predicate::Element {
                indices,
                elements,
                result,
                layout,
            } => {
                if matches!(
                    layout.position(indices, elements.len(), values),
                    SelectionPosition::OutOfRange
                ) {
                    return Ok(Some(ViolationKind::ElementRange {
                        indices: indices
                            .iter()
                            .map(|index| self.argument(index, values))
                            .collect(),
                        layout: layout.clone(),
                        length: elements.len(),
                        result: self.argument(result, values),
                    }));
                }
                let selected = constraint
                    .predicate
                    .selected_operand(values)
                    .expect("available valid selector");
                Ok(
                    (!self.selected_matches_result(selected, result, values)).then(|| {
                        ViolationKind::ElementMismatch {
                            indices: indices.iter().map(argument).collect(),
                            layout: layout.clone(),
                            selected: argument(selected),
                            result: argument(result),
                        }
                    }),
                )
            }
            Predicate::Conditional {
                condition, result, ..
            } => {
                let selected = constraint
                    .predicate
                    .selected_operand(values)
                    .expect("available condition");
                Ok(
                    (!self.selected_matches_result(selected, result, values)).then(|| {
                        ViolationKind::ConditionalMismatch {
                            condition: argument(condition),
                            selected: argument(selected),
                            result: argument(result),
                        }
                    }),
                )
            }
            Predicate::FloatRelation {
                relation,
                left,
                right,
                condition,
                code,
            } => {
                let holds = self.float_tolerance.holds(
                    *relation,
                    left.value(values).expect("available operand").float(),
                    right.value(values).expect("available operand").float(),
                );
                Ok(
                    (!condition.satisfied(holds, values)).then(|| ViolationKind::FloatRelation {
                        relation: *relation,
                        left: argument(left),
                        right: argument(right),
                        condition: self.condition_value(condition, values),
                        code: *code,
                    }),
                )
            }
            Predicate::FloatLinear {
                relation,
                coefficients,
                terms,
                bound,
                condition,
            } => {
                let sum = self
                    .float_sum(constraint, coefficients, terms, None, values)?
                    .expect("available terms");
                let holds = self.float_tolerance.holds(*relation, sum, *bound);
                Ok(
                    (!condition.satisfied(holds, values)).then(|| ViolationKind::FloatLinear {
                        relation: *relation,
                        coefficients: coefficients.clone(),
                        terms: terms.iter().map(argument).collect(),
                        bound: *bound,
                        condition: self.condition_value(condition, values),
                        sum,
                    }),
                )
            }
            Predicate::FloatArithmetic {
                operation,
                inputs,
                result,
            } => {
                let inputs_values: Vec<_> = inputs
                    .iter()
                    .map(|input| input.value(values).expect("available operand").float())
                    .collect();
                let expected = operation
                    .calculate(&inputs_values)
                    .map_err(|message| constraint.execution_error(message))?;
                Ok((!self.float_tolerance.equal(
                    expected,
                    result.value(values).expect("available operand").float(),
                ))
                .then(|| ViolationKind::FloatArithmetic {
                    operation: *operation,
                    inputs: inputs.iter().map(argument).collect(),
                    result: argument(result),
                }))
            }
            Predicate::FloatIntegerPower {
                base,
                exponent,
                result,
            } => {
                let expected = floats::integer_power(
                    base.value(values).expect("available operand").float(),
                    *exponent,
                )
                .map_err(|message| constraint.execution_error(message))?;
                Ok((!self.float_tolerance.equal(
                    expected,
                    result.value(values).expect("available operand").float(),
                ))
                .then(|| ViolationKind::FloatIntegerPower {
                    base: argument(base),
                    exponent: *exponent,
                    result: argument(result),
                }))
            }
            Predicate::FloatArrayExtremum {
                extremum,
                inputs,
                result,
            } => {
                let input_values: Vec<_> = inputs
                    .iter()
                    .map(|input| input.value(values).expect("available operand").float())
                    .collect();
                let expected = extremum.calculate(&input_values);
                Ok((!self.float_tolerance.equal(
                    expected,
                    result.value(values).expect("available operand").float(),
                ))
                .then(|| ViolationKind::FloatArrayExtremum {
                    extremum: *extremum,
                    inputs: inputs.iter().map(argument).collect(),
                    result: argument(result),
                }))
            }
            Predicate::FloatConversion {
                operation,
                input,
                result,
            } => {
                let expected = operation
                    .calculate(input.value(values).expect("available operand"))
                    .map_err(|message| constraint.execution_error(message))?;
                let actual = result.value(values).expect("available operand");
                let equal = match (&expected, actual) {
                    (Value::Float(a), Value::Float(b)) => self.float_tolerance.equal(*a, *b),
                    _ => expected == *actual,
                };
                Ok((!equal).then(|| ViolationKind::FloatConversion {
                    operation: *operation,
                    input: argument(input),
                    result: argument(result),
                }))
            }
            Predicate::FloatMembership {
                membership,
                element,
                intervals,
            } => {
                let value = element.value(values).expect("available operand").float();
                let holds = intervals
                    .iter()
                    .any(|&(lower, upper)| self.float_tolerance.contains(value, lower, upper));
                Ok((!holds).then(|| ViolationKind::FloatMembership {
                    membership: *membership,
                    element: argument(element),
                    intervals: intervals.clone(),
                }))
            }

            Predicate::BooleanRelation {
                relation,
                left,
                right,
                condition,
            } => {
                let holds = relation.holds(
                    i64::from(left.value(values).expect("available operand").boolean()),
                    i64::from(right.value(values).expect("available operand").boolean()),
                );
                Ok(
                    (!condition.satisfied(holds, values)).then(|| ViolationKind::BooleanRelation {
                        relation: *relation,
                        left: argument(left),
                        right: argument(right),
                        condition: self.condition_value(condition, values),
                    }),
                )
            }
            Predicate::BooleanLinear {
                relation,
                coefficients,
                terms,
                bound,
                condition,
            } => {
                let sum = self
                    .linear_sum(constraint, coefficients, terms, None, values)?
                    .expect("available linear terms");
                let holds =
                    relation.holds(sum, bound.value(values).expect("available bound").integer());
                Ok(
                    (!condition.satisfied(holds, values)).then(|| ViolationKind::BooleanLinear {
                        relation: *relation,
                        coefficients: coefficients.clone(),
                        terms: terms.iter().map(argument).collect(),
                        sum,
                        bound: argument(bound),
                        condition: self.condition_value(condition, values),
                    }),
                )
            }
            Predicate::BooleanOperation {
                operation,
                inputs,
                condition,
            } => {
                let inputs_values =
                    boolean_values(inputs, values).expect("available Boolean inputs");
                Ok(
                    (!condition.satisfied(operation.calculate(&inputs_values), values)).then(
                        || ViolationKind::BooleanOperation {
                            operation: *operation,
                            inputs: inputs.iter().map(argument).collect(),
                            condition: self.condition_value(condition, values),
                        },
                    ),
                )
            }
            Predicate::BooleanClause {
                positive,
                negative,
                condition,
            } => {
                let holds = boolean::clause(
                    &boolean_values(positive, values).expect("available positive inputs"),
                    &boolean_values(negative, values).expect("available negative inputs"),
                );
                Ok(
                    (!condition.satisfied(holds, values)).then(|| ViolationKind::BooleanClause {
                        positive: positive.iter().map(argument).collect(),
                        negative: negative.iter().map(argument).collect(),
                        condition: self.condition_value(condition, values),
                    }),
                )
            }
            Predicate::BooleanToInteger { boolean, integer } => {
                let holds = i64::from(
                    boolean
                        .value(values)
                        .expect("available Boolean input")
                        .boolean(),
                ) == integer
                    .value(values)
                    .expect("available integer input")
                    .integer();
                Ok((!holds).then(|| ViolationKind::BooleanToInteger {
                    boolean: argument(boolean),
                    integer: argument(integer),
                }))
            }

            Predicate::IntegerRelation {
                relation,
                left,
                right,
                condition,
            } => {
                let holds = relation.holds(
                    left.value(values).expect("available operand").integer(),
                    right.value(values).expect("available operand").integer(),
                );
                Ok(
                    (!condition.satisfied(holds, values)).then(|| ViolationKind::IntegerRelation {
                        relation: *relation,
                        left: argument(left),
                        right: argument(right),
                        condition: self.condition_value(condition, values),
                    }),
                )
            }
            Predicate::LinearRelation {
                relation,
                coefficients,
                terms,
                bound,
                condition,
            } => {
                let sum = self
                    .linear_sum(constraint, coefficients, terms, None, values)?
                    .expect("available linear terms");
                Ok(
                    (!condition.satisfied(relation.holds(sum, *bound), values)).then(|| {
                        ViolationKind::LinearRelation {
                            relation: *relation,
                            coefficients: coefficients.clone(),
                            terms: terms.iter().map(argument).collect(),
                            sum,
                            bound: *bound,
                            condition: self.condition_value(condition, values),
                        }
                    }),
                )
            }
            Predicate::IntegerArithmetic {
                operation,
                inputs,
                result,
            } => {
                let input_values: Vec<_> = inputs
                    .iter()
                    .map(|input| input.value(values).expect("available operand").integer())
                    .collect();
                let expected = operation
                    .calculate(&input_values)
                    .map_err(|message| constraint.execution_error(message))?;
                let actual = result.value(values).expect("available operand").integer();
                Ok(
                    (expected != actual).then(|| ViolationKind::IntegerArithmetic {
                        operation: *operation,
                        inputs: inputs.iter().map(argument).collect(),
                        result: argument(result),
                    }),
                )
            }
            Predicate::SetRelation {
                relation,
                left,
                right,
                condition,
                native_code,
            } => {
                let truth = constraint
                    .predicate
                    .set_truth(values)
                    .expect("available operands");
                Ok(
                    (!condition.satisfied(truth, values)).then(|| ViolationKind::SetRelation {
                        relation: *relation,
                        left: argument(left),
                        right: argument(right),
                        condition: self.condition_value(condition, values),
                        native_code: *native_code,
                    }),
                )
            }
            Predicate::SetMembership {
                element,
                set,
                condition,
            } => {
                let truth = constraint
                    .predicate
                    .set_truth(values)
                    .expect("available operands");
                Ok(
                    (!condition.satisfied(truth, values)).then(|| ViolationKind::SetMembership {
                        element: argument(element),
                        set: argument(set),
                        condition: self.condition_value(condition, values),
                    }),
                )
            }
            Predicate::SetOperation {
                operation,
                left,
                right,
                result,
            } => {
                let expected = operation.calculate(
                    left.value(values).expect("available operand").set(),
                    right.value(values).expect("available operand").set(),
                );
                Ok(
                    (expected != *result.value(values).expect("available operand").set()).then(
                        || ViolationKind::SetOperation {
                            operation: *operation,
                            left: argument(left),
                            right: argument(right),
                            result: argument(result),
                        },
                    ),
                )
            }
            Predicate::SetIn { element, set } => {
                let element_value = element
                    .value(values)
                    .expect("available operand")
                    .membership_element();
                let set_value = set.value(values).expect("available operand").set();
                Ok(
                    (!set_value.contains(element_value)).then(|| ViolationKind::SetIn {
                        element: argument(element),
                        set: argument(set),
                    }),
                )
            }
            Predicate::SetEqual { left, right } | Predicate::SetSubset { left, right } => {
                let left_value = left.value(values).expect("available operand").set();
                let right_value = right.value(values).expect("available operand").set();
                let equal = matches!(constraint.predicate, Predicate::SetEqual { .. });
                let satisfied = if equal {
                    left_value == right_value
                } else {
                    left_value.is_subset(right_value)
                };
                Ok((!satisfied).then(|| {
                    if equal {
                        ViolationKind::SetEqual {
                            left: argument(left),
                            right: argument(right),
                        }
                    } else {
                        ViolationKind::SetSubset {
                            left: argument(left),
                            right: argument(right),
                        }
                    }
                }))
            }
            Predicate::SetCardinality { set, cardinality } => {
                let size = set
                    .value(values)
                    .expect("available operand")
                    .set()
                    .cardinality()
                    .ok_or_else(|| {
                        constraint.execution_error("checked i64 overflow in set cardinality")
                    })?;
                let expected = cardinality
                    .value(values)
                    .expect("available operand")
                    .integer();
                Ok((size != expected).then(|| ViolationKind::SetCardinality {
                    set: argument(set),
                    cardinality: argument(cardinality),
                }))
            }
            Predicate::SetUnion {
                left,
                right,
                result,
            }
            | Predicate::SetIntersection {
                left,
                right,
                result,
            } => {
                let left_value = left.value(values).expect("available operand").set();
                let right_value = right.value(values).expect("available operand").set();
                let union = matches!(constraint.predicate, Predicate::SetUnion { .. });
                let expected = if union {
                    left_value.union(right_value)
                } else {
                    left_value.intersection(right_value)
                };
                let actual = result.value(values).expect("available operand").set();
                Ok((expected != *actual).then(|| {
                    if union {
                        ViolationKind::SetUnion {
                            left: argument(left),
                            right: argument(right),
                            result: argument(result),
                        }
                    } else {
                        ViolationKind::SetIntersection {
                            left: argument(left),
                            right: argument(right),
                            result: argument(result),
                        }
                    }
                }))
            }
            Predicate::DisjunctiveStrict { starts, durations } => {
                self.evaluate_disjunctive(constraint, starts, durations, values)
            }
            Predicate::Linear {
                coefficients,
                terms,
                bound,
                equality,
            } => {
                let sum = self
                    .linear_sum(constraint, coefficients, terms, None, values)?
                    .expect("available linear terms");
                let terms: Vec<_> = terms.iter().map(argument).collect();
                let satisfied = if *equality {
                    sum == *bound
                } else {
                    sum <= *bound
                };
                Ok((!satisfied).then(|| ViolationKind::Linear {
                    coefficients: coefficients.clone(),
                    terms,
                    sum,
                    bound: *bound,
                }))
            }
            Predicate::ReifiedLessEqual {
                left,
                right,
                result,
            } => {
                let (left, right, result) = (argument(left), argument(right), argument(result));
                let (Value::Integer(x), Value::Integer(y), Value::Boolean(b)) =
                    (&left.value, &right.value, &result.value)
                else {
                    unreachable!("preparation validates reified operands")
                };
                Ok(((x <= y) != *b).then_some(ViolationKind::ReifiedLessEqual {
                    left,
                    right,
                    result,
                }))
            }
        }
    }
}

fn serialize_optional_integer<S: serde::Serializer>(
    value: &Option<i64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    value.map(|value| value.to_string()).serialize(serializer)
}

fn deserialize_optional_integer<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i64>, D::Error> {
    Option::<String>::deserialize(deserializer)?
        .map(|value| value.parse().map_err(serde::de::Error::custom))
        .transpose()
}

fn serialize_integer<S: serde::Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

fn serialize_integers<S: serde::Serializer>(
    values: &[i64],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut sequence = serializer.serialize_seq(Some(values.len()))?;
    for value in values {
        sequence.serialize_element(&value.to_string())?;
    }
    sequence.end()
}

fn deserialize_integer<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

fn deserialize_integers<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<i64>, D::Error> {
    Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .map(|value| value.parse().map_err(serde::de::Error::custom))
        .collect()
}

impl PreparedModel {
    fn link_aliases(&mut self) {
        let mut aliases = vec![Vec::new(); self.slots.len()];
        for variable in &self.variables {
            aliases[variable.slot].push(VariableAlias {
                name: variable.reference.name.clone(),
                position: variable.reference.position,
            });
        }
        let link = |reference: &mut VariableRef, slot: usize| {
            reference.aliases = aliases[slot]
                .iter()
                .filter(|alias| {
                    alias.name != reference.name || alias.position != reference.position
                })
                .cloned()
                .collect();
        };
        for variable in &mut self.variables {
            link(&mut variable.reference, variable.slot);
        }
        for (slot, value) in self.slots.iter_mut().enumerate() {
            link(&mut value.reference, slot);
        }
    }

    fn definition_targets(&self, constraint: &Constraint) -> Result<Vec<usize>, Diagnostic> {
        let fail = |message: &str| Diagnostic {
            location: Some(constraint.reference.location.clone()),
            message: message.into(),
        };
        let Some(target) = &constraint.reference.metadata.defines else {
            return Ok(Vec::new());
        };
        let names: Vec<_> = match target {
            DefinitionTarget::Name(name) => vec![name],
            DefinitionTarget::Elements(names) => names.iter().collect(),
        };
        let mut slots = Vec::new();
        let mut target_kinds = Vec::new();
        let mut array_target = matches!(target, DefinitionTarget::Elements(_));
        for name in names {
            let binding = self
                .bindings
                .get(name)
                .ok_or_else(|| fail("defines_var names an unknown variable"))?;
            if !binding.assignable
                || (matches!(target, DefinitionTarget::Elements(_)) && binding.is_array)
            {
                return Err(fail("defines_var needs an assignable variable target"));
            }
            array_target |= binding.is_array;
            target_kinds.push(binding.kind);
            for operand in &binding.elements {
                let Operand::Variable(slot) = operand else {
                    return Err(fail("defines_var needs variable slots"));
                };
                if self
                    .slots
                    .get(*slot)
                    .is_none_or(|entry| entry.kind != binding.kind)
                {
                    return Err(fail("defines_var target has an invalid slot"));
                }
                if !self.variables.iter().any(|variable| {
                    variable.slot == *slot
                        && (binding.is_array
                            || matches!(target, DefinitionTarget::Elements(_))
                            || variable.reference.name == *name)
                        && variable.reference.metadata.is_defined
                }) {
                    return Err(fail("defines_var target must have is_defined_var"));
                }
                slots.push(*slot);
            }
        }
        if array_target {
            let outputs: Vec<(Vec<&Operand>, ValueType)> = match &constraint.predicate {
                Predicate::BinPacking(packing) => {
                    vec![(packing.loads.iter().collect(), ValueType::Integer)]
                }
                Predicate::Counting(Counting::Cardinality {
                    counts: counting::CardinalityCounts::Exact(counts),
                    ..
                }) => vec![(counts.iter().collect(), ValueType::Integer)],
                Predicate::Circuit(circuit) => circuit
                    .array_outputs()
                    .into_iter()
                    .map(|outputs| (outputs, ValueType::Integer))
                    .collect(),
                Predicate::Ordering(ordering) => ordering
                    .array_outputs()
                    .into_iter()
                    .map(|(outputs, kind)| (outputs.iter().collect(), kind))
                    .collect(),
                _ => Vec::new(),
            };
            // Annotation order identifies slots; predicate positions own the calculation.
            let mut targets = slots.clone();
            targets.sort_unstable();
            let matches = outputs.iter().any(|(outputs, kind)| {
                if target_kinds.iter().any(|target| target != kind) {
                    return false;
                }
                let Some(mut outputs) = outputs
                    .iter()
                    .map(|operand| match operand {
                        Operand::Variable(slot) => Some(*slot),
                        Operand::Literal(_) => None,
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                outputs.sort_unstable();
                outputs == targets
            });
            if !matches {
                return Err(fail(
                    "array defines_var target must match a supported complete output array",
                ));
            }
        } else if slots.len() != 1 {
            return Err(fail("defines_var needs a scalar variable"));
        }
        Ok(slots)
    }

    fn prepare_definitions(&mut self) -> Result<(), Diagnostic> {
        let mut defined = HashSet::new();
        self.definitions.clear();
        self.dependents = vec![Vec::new(); self.slots.len()];
        for (constraint_index, constraint) in self.constraints.iter().enumerate() {
            if constraint.reference.metadata.defines.is_none() {
                continue;
            }
            let fail = |message: &str| Diagnostic {
                location: Some(constraint.reference.location.clone()),
                message: message.into(),
            };
            let targets = self.definition_targets(constraint)?;
            let mut unique_targets = HashSet::new();
            for slot in &targets {
                if !unique_targets.insert(*slot) {
                    continue;
                }
                if !defined.insert(*slot) {
                    return Err(fail("a variable has multiple defines_var annotations"));
                }
                let direction = definition_direction(constraint, *slot)?;
                // A definition supplies its output even when an alias has a singleton domain.
                self.slots[*slot].default = None;
                if let Some(direction) = direction {
                    let inputs = constraint.predicate.operands();
                    let definition_index = self.definitions.len();
                    let mut inputs_seen = HashSet::new();
                    for operand in inputs {
                        if let Operand::Variable(input) = operand
                            && input != slot
                            && inputs_seen.insert(*input)
                        {
                            self.dependents[*input].push(definition_index);
                        }
                    }
                    self.definitions.push(Definition {
                        constraint: constraint_index,
                        output: *slot,
                        direction,
                    });
                }
            }
        }
        for variable in &self.variables {
            if variable.reference.metadata.is_defined && !defined.contains(&variable.slot) {
                return Err(Diagnostic {
                    location: Some(variable.reference.location.clone()),
                    message: "is_defined_var has no matching defines_var relation".into(),
                });
            }
        }
        Ok(())
    }
}

fn definition_direction(
    constraint: &Constraint,
    output: usize,
) -> Result<Option<DefinitionDirection>, Diagnostic> {
    let absent = || Diagnostic {
        location: Some(constraint.reference.location.clone()),
        message: "defines_var target does not occur in its relation".into(),
    };
    let operands = constraint.predicate.operands();
    let occurrences: Vec<_> = operands
        .iter()
        .enumerate()
        .filter_map(|(index, operand)| {
            matches!(operand, Operand::Variable(slot) if *slot == output).then_some(index)
        })
        .collect();
    if occurrences.is_empty() {
        return Err(absent());
    }
    match &constraint.predicate {
        Predicate::BinPacking(packing) => Ok(packing
            .can_define(output)
            .then_some(DefinitionDirection::BinPacking)),
        Predicate::Table(table) => Ok(table
            .can_define(output)
            .then_some(DefinitionDirection::Table)),
        Predicate::Circuit(circuit) => Ok(circuit
            .can_define(output)
            .then_some(DefinitionDirection::Circuit)),
        Predicate::Ordering(ordering) => Ok(ordering
            .can_define(output)
            .then_some(DefinitionDirection::Ordering)),
        Predicate::Counting(counting) => Ok(counting
            .can_define(output)
            .then_some(DefinitionDirection::Counting)),
        Predicate::Element { result, .. } | Predicate::Conditional { result, .. } => {
            Ok(matches!(result, Operand::Variable(slot) if *slot == output)
                .then_some(DefinitionDirection::Selection))
        }
        Predicate::FloatRelation {
            relation,
            condition,
            ..
        } => Ok(match condition {
            RelationCondition::Plain
                if *relation == IntegerRelation::Equal && occurrences.len() == 1 =>
            {
                Some(DefinitionDirection::FloatEqual {
                    input: 1 - occurrences[0],
                })
            }
            RelationCondition::Reified { .. } if occurrences.as_slice() == [2] => {
                Some(DefinitionDirection::FloatReified)
            }
            _ => None,
        }),
        Predicate::FloatLinear {
            relation,
            coefficients,
            terms,
            condition,
            ..
        } => Ok(match condition {
            RelationCondition::Plain
                if *relation == IntegerRelation::Equal
                    && occurrences.len() == 1
                    && coefficients[occurrences[0]].get() != 0.0 =>
            {
                Some(DefinitionDirection::FloatLinear {
                    term: occurrences[0],
                })
            }
            RelationCondition::Reified { .. } if occurrences.as_slice() == [terms.len()] => {
                Some(DefinitionDirection::FloatReified)
            }
            _ => None,
        }),
        Predicate::FloatArithmetic { inputs, .. } => Ok((occurrences.as_slice() == [inputs.len()])
            .then_some(DefinitionDirection::FloatArithmetic)),
        Predicate::FloatIntegerPower { .. } => {
            Ok((occurrences.as_slice() == [1]).then_some(DefinitionDirection::FloatIntegerPower))
        }
        Predicate::FloatArrayExtremum { inputs, .. } => Ok((occurrences.as_slice()
            == [inputs.len()])
        .then_some(DefinitionDirection::FloatArrayExtremum)),
        Predicate::FloatConversion { .. } => {
            Ok((occurrences.as_slice() == [1]).then_some(DefinitionDirection::FloatConversion))
        }
        Predicate::FloatMembership { .. } => Ok(None),

        Predicate::BooleanRelation {
            relation,
            condition,
            ..
        } => Ok(match condition {
            RelationCondition::Plain
                if occurrences.len() == 1
                    && matches!(relation, IntegerRelation::Equal | IntegerRelation::NotEqual) =>
            {
                Some(DefinitionDirection::BooleanCopy {
                    input: 1 - occurrences[0],
                    complement: *relation == IntegerRelation::NotEqual,
                })
            }
            RelationCondition::Reified { .. } if occurrences.as_slice() == [2] => {
                Some(DefinitionDirection::ReifiedRelation)
            }
            _ => None,
        }),
        Predicate::BooleanLinear {
            relation,
            coefficients,
            terms,
            condition,
            ..
        } => {
            let position = occurrences[0];
            Ok(match condition {
                RelationCondition::Plain
                    if *relation == IntegerRelation::Equal && occurrences.len() == 1 =>
                {
                    if position == terms.len() {
                        Some(DefinitionDirection::BooleanLinearBound)
                    } else if coefficients[position] != 0 {
                        Some(DefinitionDirection::BooleanLinearTerm { term: position })
                    } else {
                        None
                    }
                }
                RelationCondition::Reified { .. }
                    if occurrences.as_slice() == [terms.len() + 1] =>
                {
                    Some(DefinitionDirection::ReifiedRelation)
                }
                _ => None,
            })
        }
        Predicate::BooleanOperation {
            operation,
            inputs,
            condition,
        } => Ok(match condition {
            RelationCondition::Reified { .. }
                if *operation == BooleanOperation::Not && occurrences.len() == 1 =>
            {
                Some(DefinitionDirection::BooleanCopy {
                    input: 1 - occurrences[0],
                    complement: true,
                })
            }
            RelationCondition::Plain
                if *operation == BooleanOperation::Xor
                    && inputs.len() == 2
                    && occurrences.len() == 1
                    && matches!(
                        BooleanName::from_name(&constraint.reference.predicate)
                            .map(|name| name.kind),
                        Some(BooleanKind::Scalar(_))
                    ) =>
            {
                Some(DefinitionDirection::BooleanCopy {
                    input: 1 - occurrences[0],
                    complement: true,
                })
            }
            RelationCondition::Reified { .. } if occurrences.as_slice() == [inputs.len()] => {
                Some(DefinitionDirection::BooleanResult)
            }
            _ => None,
        }),
        Predicate::BooleanClause {
            positive,
            negative,
            condition,
        } => Ok((matches!(condition, RelationCondition::Reified { .. })
            && occurrences.as_slice() == [positive.len() + negative.len()])
        .then_some(DefinitionDirection::BooleanResult)),
        Predicate::BooleanToInteger { .. } => Ok(if occurrences.len() == 1 {
            Some(if occurrences[0] == 0 {
                DefinitionDirection::IntegerToBoolean
            } else {
                DefinitionDirection::BooleanToInteger
            })
        } else {
            None
        }),

        Predicate::IntegerRelation {
            relation,
            condition,
            ..
        } => Ok(match condition {
            RelationCondition::Plain
                if *relation == IntegerRelation::Equal && occurrences.len() == 1 =>
            {
                Some(DefinitionDirection::IntegerEqual {
                    input: 1 - occurrences[0],
                })
            }
            RelationCondition::Reified { .. } if occurrences.as_slice() == [2] => {
                Some(DefinitionDirection::ReifiedRelation)
            }
            _ => None,
        }),
        Predicate::LinearRelation {
            relation,
            coefficients,
            terms,
            condition,
            ..
        } => {
            let term = occurrences[0];
            Ok(match condition {
                RelationCondition::Plain
                    if *relation == IntegerRelation::Equal
                        && occurrences.len() == 1
                        && coefficients[term] != 0 =>
                {
                    Some(DefinitionDirection::Linear { term })
                }
                RelationCondition::Reified { .. } if occurrences.as_slice() == [terms.len()] => {
                    Some(DefinitionDirection::ReifiedRelation)
                }
                _ => None,
            })
        }
        Predicate::IntegerArithmetic { inputs, .. } => {
            let is_final_output = occurrences.as_slice() == [inputs.len()];
            Ok(is_final_output.then_some(DefinitionDirection::IntegerArithmetic))
        }
        Predicate::SetRelation { condition, .. } | Predicate::SetMembership { condition, .. } => {
            Ok((matches!(condition, RelationCondition::Reified { .. })
                && occurrences.as_slice() == [2])
            .then_some(DefinitionDirection::SetReified))
        }
        Predicate::SetOperation { .. } => {
            Ok((occurrences.as_slice() == [2]).then_some(DefinitionDirection::SetOperation))
        }
        Predicate::SetEqual { .. } => {
            Ok(
                (occurrences.len() == 1).then(|| DefinitionDirection::SetEqual {
                    input: 1 - occurrences[0],
                }),
            )
        }
        Predicate::SetCardinality { .. } => {
            Ok((occurrences.as_slice() == [1]).then_some(DefinitionDirection::SetCardinality))
        }
        Predicate::SetUnion { .. } => {
            Ok((occurrences.as_slice() == [2]).then_some(DefinitionDirection::SetUnion))
        }
        Predicate::SetIntersection { .. } => {
            Ok((occurrences.as_slice() == [2]).then_some(DefinitionDirection::SetIntersection))
        }
        Predicate::Automaton(_)
        | Predicate::Rectangles(_)
        | Predicate::Scheduling(_)
        | Predicate::SetIn { .. }
        | Predicate::SetSubset { .. }
        | Predicate::DisjunctiveStrict { .. } => Ok(None),
        Predicate::Linear {
            coefficients,
            equality,
            ..
        } => {
            let term = occurrences[0];
            Ok(
                (*equality && occurrences.len() == 1 && coefficients[term] != 0)
                    .then_some(DefinitionDirection::Linear { term }),
            )
        }
        Predicate::ReifiedLessEqual { result, .. } => {
            Ok(matches!(result, Operand::Variable(slot) if *slot == output)
                .then_some(DefinitionDirection::ReifiedLessEqual))
        }
    }
}

impl Domain {
    fn singleton(&self) -> Option<Value> {
        match self {
            Self::FloatInterval { lower, upper } if lower == upper => Some(Value::Float(*lower)),
            Self::Interval { lower, upper } if lower == upper => Some(Value::Integer(*lower)),
            Self::Enumeration { values } if values.len() == 1 => Some(Value::Integer(values[0])),
            Self::Fixed { value } => Some(value.clone()),
            Self::IntegerSet { upper: Some(upper) } if upper.is_empty() => {
                Some(Value::IntegerSet(IntegerSet::empty()))
            }
            _ => None,
        }
    }
}

fn boolean_values(operands: &[Operand], values: &[Option<Value>]) -> Option<Vec<bool>> {
    operands
        .iter()
        .map(|operand| operand.value(values).map(Value::boolean))
        .collect()
}
