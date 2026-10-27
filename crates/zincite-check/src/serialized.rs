//! Versioned executable JSON and admission checks for parser-free loading.

use super::*;

const FORMAT_VERSION: u32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelFile {
    format_version: u32,
    model: Executable,
}

// Only this private transport type can deserialize executable fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Executable {
    #[serde(default)]
    float_tolerance: FloatTolerance,
    slots: Vec<Slot>,
    bindings: HashMap<String, Binding>,
    variables: Vec<Variable>,
    constraints: Vec<Constraint>,
    objective: Option<(ObjectiveDirection, Operand)>,
}

impl PreparedModel {
    /// Export executable values, operations and reporting metadata as versioned JSON.
    /// Mathematical integers are decimal strings; no source text or CST is included.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        #[derive(Serialize)]
        struct Export<'a> {
            format_version: u32,
            model: &'a PreparedModel,
        }
        serde_json::to_string_pretty(&Export {
            format_version: FORMAT_VERSION,
            model: self,
        })
    }

    /// Load and validate an exported executable without parsing FlatZinc source.
    /// Rebuild aliases, definition directions and dependencies before checking.
    pub fn from_json(json: &str) -> Result<Self, Diagnostic> {
        let file: ModelFile = serde_json::from_str(json)
            .map_err(|error| invalid(format!("invalid prepared model JSON: {error}")))?;
        if file.format_version != FORMAT_VERSION {
            return Err(invalid(format!(
                "unsupported prepared model format version {}; expected {FORMAT_VERSION}",
                file.format_version
            )));
        }
        let mut model = Self {
            float_tolerance: file.model.float_tolerance,
            slots: file.model.slots,
            bindings: file.model.bindings,
            variables: file.model.variables,
            constraints: file.model.constraints,
            objective: file.model.objective,
            definitions: Vec::new(),
            dependents: Vec::new(),
        };
        model.validate_executable()?;
        model.link_aliases();
        model.prepare_definitions()?;
        Ok(model)
    }

    fn validate_executable(&self) -> Result<(), Diagnostic> {
        self.validate_bindings()?;
        let seeds = self.validate_variables()?;
        self.validate_slots(&seeds)?;
        self.validate_constraints()?;
        if let Some((_, operand)) = &self.objective {
            match operand {
                Operand::Literal(Value::Float(_)) => {
                    self.validate_operand(operand, ValueType::Float)?
                }
                Operand::Variable(slot)
                    if self
                        .slots
                        .get(*slot)
                        .is_some_and(|slot| slot.kind == ValueType::Float) =>
                {
                    self.validate_operand(operand, ValueType::Float)?
                }
                _ => self.validate_operand(operand, ValueType::Integer)?,
            }
        }
        Ok(())
    }

    fn validate_bindings(&self) -> Result<(), Diagnostic> {
        for (name, binding) in &self.bindings {
            if binding.assignable && binding.kind == ValueType::FloatSet {
                return Err(invalid("assignable float sets are unsupported"));
            }
            if name.is_empty() || (!binding.is_array && binding.elements.len() != 1) {
                return Err(invalid("invalid binding name or scalar shape"));
            }
            for operand in &binding.elements {
                self.validate_operand(operand, binding.kind)?;
                if binding.assignable != matches!(operand, Operand::Variable(_)) {
                    return Err(invalid(
                        "variable bindings need slots; parameters need literals",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_variables(&self) -> Result<Vec<Vec<Value>>, Diagnostic> {
        let mut references = HashSet::new();
        let mut seeds = vec![Vec::new(); self.slots.len()];
        for variable in &self.variables {
            self.validate_reference(&variable.reference, variable.slot)?;
            if !references.insert((&variable.reference.name, variable.reference.position)) {
                return Err(invalid("duplicate variable reference"));
            }
            let slot = &self.slots[variable.slot];
            if variable.domains.is_empty() {
                return Err(invalid("a variable needs a domain"));
            }
            for domain in &variable.domains {
                let kind = match domain {
                    Domain::Float => ValueType::Float,
                    Domain::FloatInterval { .. } => ValueType::Float,
                    Domain::FloatSet => {
                        return Err(invalid("assignable float sets are unsupported"));
                    }
                    Domain::Boolean => ValueType::Boolean,
                    Domain::IntegerSet { .. } => ValueType::IntegerSet,
                    Domain::Fixed { value } => value.value_type(),
                    _ => ValueType::Integer,
                };
                if kind != slot.kind {
                    return Err(invalid("variable domain type does not match its slot"));
                }
                if let Some(value) = domain.singleton() {
                    seeds[variable.slot].push(value);
                }
            }
        }
        // Every assignable name/position needs its domain and report reference.
        for (name, binding) in &self.bindings {
            if binding.assignable {
                for position in 0..binding.elements.len() {
                    let position = binding.is_array.then_some(position + 1);
                    if !references.contains(&(name, position)) {
                        return Err(invalid("variable binding has no declaration reference"));
                    }
                }
            }
        }
        Ok(seeds)
    }

    fn validate_slots(&self, seeds: &[Vec<Value>]) -> Result<(), Diagnostic> {
        let mut defined_slots = HashSet::new();
        for constraint in &self.constraints {
            defined_slots.extend(self.definition_targets(constraint)?);
        }
        for (index, slot) in self.slots.iter().enumerate() {
            self.validate_reference(&slot.reference, index)?;
            if !self.variables.iter().any(|variable| {
                variable.slot == index
                    && variable.reference.name == slot.reference.name
                    && variable.reference.position == slot.reference.position
                    && variable.reference.location == slot.reference.location
                    && variable.reference.metadata == slot.reference.metadata
            }) {
                return Err(invalid("slot report reference has no matching declaration"));
            }
            if defined_slots.contains(&index) {
                if slot.default.is_some() {
                    return Err(invalid("a defined slot cannot have a default"));
                }
            } else {
                match &slot.default {
                    Some(value)
                        if value.value_type() == slot.kind && seeds[index].contains(value) => {}
                    None if seeds[index].is_empty() => {}
                    _ => {
                        return Err(invalid(
                            "slot default must match a fixed or singleton declaration",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_constraints(&self) -> Result<(), Diagnostic> {
        for (index, constraint) in self.constraints.iter().enumerate() {
            validate_location(&constraint.reference.location)?;
            if constraint.reference.ordinal != index + 1 {
                return Err(invalid("constraint ordinals must follow executable order"));
            }
            let predicate_name = match &constraint.predicate {
                Predicate::Automaton(automaton) => {
                    if AutomatonName::from_name(&constraint.reference.predicate)
                        != Some(automaton.name())
                    {
                        return Err(invalid(
                            "automaton predicate name does not match its executable contract",
                        ));
                    }
                    automaton.validate().map_err(invalid)?;
                    for entry in &automaton.word {
                        self.validate_operand(entry, ValueType::Integer)?;
                    }
                    &constraint.reference.predicate
                }
                Predicate::Table(table) => {
                    self.validate_table(constraint, table)?;
                    &constraint.reference.predicate
                }
                Predicate::BinPacking(packing) => {
                    if !is_bin_packing_name(&constraint.reference.predicate) {
                        return Err(invalid(
                            "bin packing predicate name does not match its executable contract",
                        ));
                    }
                    packing.validate().map_err(invalid)?;
                    for operand in packing.operands() {
                        self.validate_operand(operand, ValueType::Integer)?;
                    }
                    &constraint.reference.predicate
                }
                Predicate::Rectangles(rectangles) => {
                    if RectangleName::from_name(&constraint.reference.predicate)
                        != Some(rectangles.name)
                    {
                        return Err(invalid(
                            "rectangle predicate name does not match its executable contract",
                        ));
                    }
                    rectangles.validate().map_err(invalid)?;
                    for operand in rectangles.integer_operands() {
                        self.validate_operand(operand, ValueType::Integer)?;
                    }
                    for operand in rectangles.present.iter().flatten() {
                        self.validate_operand(operand, ValueType::Boolean)?;
                    }
                    &constraint.reference.predicate
                }
                Predicate::Scheduling(schedule) => {
                    if SchedulingName::from_name(&constraint.reference.predicate, schedule.arity())
                        != Some(schedule.name())
                    {
                        return Err(invalid(
                            "scheduling predicate name does not match its executable contract",
                        ));
                    }
                    schedule.validate_shape().map_err(invalid)?;
                    for operand in schedule.integer_operands() {
                        self.validate_operand(operand, ValueType::Integer)?;
                    }
                    for operand in schedule.presence().into_iter().flatten() {
                        self.validate_operand(operand, ValueType::Boolean)?;
                    }
                    &constraint.reference.predicate
                }
                Predicate::NumberTheory(arithmetic) => {
                    if NumberTheoryName::from_name(&constraint.reference.predicate)
                        != Some(arithmetic.name())
                    {
                        return Err(invalid(
                            "number theory predicate name does not match its operation or mode",
                        ));
                    }
                    arithmetic.validate().map_err(invalid)?;
                    for operand in arithmetic.integer_operands() {
                        self.validate_operand(operand, ValueType::Integer)?;
                    }
                    if let Some(operand) = arithmetic.condition().operand() {
                        self.validate_operand(operand, ValueType::Boolean)?;
                    }
                    &constraint.reference.predicate
                }
                Predicate::Distance(distance) => {
                    if DistanceName::from_name(&constraint.reference.predicate)
                        != Some(distance.name())
                    {
                        return Err(invalid(
                            "distance predicate name does not match its executable contract",
                        ));
                    }
                    distance.validate().map_err(invalid)?;
                    for operand in distance.operands() {
                        self.validate_operand(operand, ValueType::Integer)?;
                    }
                    &constraint.reference.predicate
                }
                Predicate::Circuit(circuit) => {
                    if CircuitName::from_name(&constraint.reference.predicate)
                        != Some(circuit.name())
                    {
                        return Err(invalid(
                            "circuit predicate name does not match its executable contract",
                        ));
                    }
                    circuit.validate_shape().map_err(invalid)?;
                    for operand in circuit.operands() {
                        self.validate_operand(operand, ValueType::Integer)?;
                    }
                    &constraint.reference.predicate
                }
                Predicate::SetChannel(channel) => {
                    self.validate_set_channel(constraint, channel)?;
                    &constraint.reference.predicate
                }
                Predicate::SetGlobal(global) => {
                    self.validate_set_global(constraint, global)?;
                    &constraint.reference.predicate
                }
                Predicate::Ordering(ordering) => {
                    self.validate_ordering(constraint, ordering)?;
                    &constraint.reference.predicate
                }
                Predicate::Counting(counting) => {
                    self.validate_counting(constraint, counting)?;
                    &constraint.reference.predicate
                }
                Predicate::Element { .. } | Predicate::Conditional { .. } => {
                    self.validate_selection(constraint)?;
                    &constraint.reference.predicate
                }
                Predicate::FloatRelation { .. }
                | Predicate::FloatLinear { .. }
                | Predicate::FloatArithmetic { .. }
                | Predicate::FloatIntegerPower { .. }
                | Predicate::FloatArrayExtremum { .. }
                | Predicate::FloatConversion { .. }
                | Predicate::FloatMembership { .. } => {
                    self.validate_float(constraint)?;
                    &constraint.reference.predicate
                }
                Predicate::BooleanRelation { .. }
                | Predicate::BooleanLinear { .. }
                | Predicate::BooleanOperation { .. }
                | Predicate::BooleanClause { .. }
                | Predicate::BooleanToInteger { .. } => {
                    self.validate_boolean(constraint)?;
                    &constraint.reference.predicate
                }
                Predicate::IntegerRelation {
                    relation,
                    left,
                    right,
                    condition,
                } => {
                    self.validate_operand(left, ValueType::Integer)?;
                    self.validate_operand(right, ValueType::Integer)?;
                    self.validate_relation(constraint, *relation, false, condition)?;
                    &constraint.reference.predicate
                }
                Predicate::LinearRelation {
                    relation,
                    coefficients,
                    terms,
                    condition,
                    ..
                } => {
                    self.validate_linear(coefficients, terms)?;
                    self.validate_relation(constraint, *relation, true, condition)?;
                    &constraint.reference.predicate
                }
                Predicate::IntegerArithmetic {
                    operation,
                    inputs,
                    result,
                } => {
                    if ArithmeticOperation::from_name(&constraint.reference.predicate)
                        != Some(*operation)
                    {
                        return Err(invalid(
                            "constraint report predicate does not match its operation",
                        ));
                    }
                    if inputs.len() != operation.input_count() {
                        return Err(invalid("integer arithmetic has the wrong input count"));
                    }
                    for operand in inputs.iter().chain(std::iter::once(result)) {
                        self.validate_operand(operand, ValueType::Integer)?;
                    }
                    if operation.has_fixed_exponent() {
                        let Operand::Literal(Value::Integer(exponent)) = inputs[1] else {
                            return Err(invalid(
                                "power exponent must be a fixed integer parameter",
                            ));
                        };
                        operation.validate_exponent(exponent).map_err(invalid)?;
                    }
                    &constraint.reference.predicate
                }
                Predicate::SetIn { .. }
                | Predicate::SetEqual { .. }
                | Predicate::SetSubset { .. }
                | Predicate::SetCardinality { .. }
                | Predicate::SetUnion { .. }
                | Predicate::SetIntersection { .. }
                | Predicate::SetRelation { .. }
                | Predicate::SetMembership { .. }
                | Predicate::SetOperation { .. } => {
                    self.validate_set(constraint)?;
                    &constraint.reference.predicate
                }
                Predicate::DisjunctiveStrict { starts, durations } => {
                    if starts.len() != durations.len() {
                        return Err(invalid("strict disjunctive arrays must have equal lengths"));
                    }
                    for operand in starts.iter().chain(durations) {
                        self.validate_operand(operand, ValueType::Integer)?;
                    }
                    if !durations.iter().all(|operand| matches!(operand, Operand::Literal(Value::Integer(value)) if *value >= 0)) {
                        return Err(invalid("strict disjunctive durations must be fixed nonnegative integers"));
                    }
                    "fzn_disjunctive_strict"
                }
                Predicate::Linear {
                    coefficients,
                    terms,
                    equality,
                    ..
                } => {
                    self.validate_linear(coefficients, terms)?;
                    let relation = if *equality {
                        IntegerRelation::Equal
                    } else {
                        IntegerRelation::LessEqual
                    };
                    self.validate_relation(constraint, relation, true, &RelationCondition::Plain)?;
                    &constraint.reference.predicate
                }
                Predicate::ReifiedLessEqual {
                    left,
                    right,
                    result,
                } => {
                    self.validate_operand(left, ValueType::Integer)?;
                    self.validate_operand(right, ValueType::Integer)?;
                    self.validate_operand(result, ValueType::Boolean)?;
                    if RelationName::from_name(&constraint.reference.predicate)
                        != Some(RelationName {
                            relation: IntegerRelation::LessEqual,
                            linear: false,
                            mode: RelationMode::Reified,
                        })
                    {
                        return Err(invalid(
                            "constraint report predicate does not match its operation",
                        ));
                    }
                    &constraint.reference.predicate
                }
            };
            if constraint.reference.predicate != predicate_name {
                return Err(invalid(
                    "constraint report predicate does not match its operation",
                ));
            }
        }
        Ok(())
    }

    fn validate_ordering(
        &self,
        constraint: &Constraint,
        ordering: &Ordering,
    ) -> Result<(), Diagnostic> {
        use ordering::DistinctMode;
        if OrderingName::from_name(&constraint.reference.predicate) != Some(ordering.name()) {
            return Err(invalid(
                "ordering predicate name does not match its executable contract",
            ));
        }
        ordering.validate_shape().map_err(invalid)?;
        let validate = |operands: &[Operand], kind| -> Result<(), Diagnostic> {
            for operand in operands {
                self.validate_operand(operand, kind)?;
            }
            Ok(())
        };
        let element_kind = |boolean| {
            if boolean {
                ValueType::Boolean
            } else {
                ValueType::Integer
            }
        };
        match ordering {
            Ordering::Distinct { entries, mode } => {
                validate(entries, ValueType::Integer)?;
                if let DistinctMode::Optional(presence) = mode {
                    validate(presence, ValueType::Boolean)?;
                }
            }
            Ordering::Equal { entries, .. } | Ordering::Precede { entries, .. } => {
                validate(entries, ValueType::Integer)?
            }
            Ordering::Lex {
                left,
                right,
                boolean,
                ..
            }
            | Ordering::NotEqual {
                left,
                right,
                boolean,
            } => {
                validate(left, element_kind(*boolean))?;
                validate(right, element_kind(*boolean))?;
            }
            Ordering::Monotone {
                entries, boolean, ..
            } => validate(entries, element_kind(*boolean))?,
            Ordering::Extremum {
                entries, result, ..
            } => {
                validate(entries, ValueType::Integer)?;
                self.validate_operand(result, ValueType::Integer)?;
            }
            Ordering::ArgExtremum {
                entries,
                result,
                boolean,
                ..
            } => {
                validate(entries, element_kind(*boolean))?;
                self.validate_operand(result, ValueType::Integer)?;
            }
            Ordering::Sort {
                entries,
                sorted,
                permutation,
                ..
            } => {
                validate(entries, ValueType::Integer)?;
                validate(sorted, ValueType::Integer)?;
                if let Some(permutation) = permutation {
                    validate(permutation, ValueType::Integer)?;
                }
            }
            Ordering::Inverse {
                entries, inverse, ..
            } => {
                validate(entries, ValueType::Integer)?;
                validate(inverse, ValueType::Integer)?;
            }
            Ordering::Channel { entries, value, .. } => {
                validate(entries, ValueType::Boolean)?;
                self.validate_operand(value, ValueType::Integer)?;
            }
            Ordering::Member {
                entries,
                needle,
                result,
                boolean,
            } => {
                validate(entries, element_kind(*boolean))?;
                self.validate_operand(needle, element_kind(*boolean))?;
                if let Some(result) = result {
                    self.validate_operand(result, ValueType::Boolean)?;
                }
            }
        }
        Ok(())
    }

    fn validate_table(&self, constraint: &Constraint, table: &Table) -> Result<(), Diagnostic> {
        let name = TableName::from_name(&constraint.reference.predicate).ok_or_else(|| {
            invalid("table predicate name does not match its executable contract")
        })?;
        if name.kind() != table.table.kind() || name.mode != table.condition.mode() {
            return Err(invalid(
                "table predicate name does not match its executable contract",
            ));
        }
        table.validate_shape().map_err(invalid)?;
        for entry in &table.tuple {
            self.validate_operand(entry, name.kind())?;
        }
        if let Some(operand) = table.condition.operand() {
            self.validate_operand(operand, ValueType::Boolean)?;
        }
        Ok(())
    }

    fn validate_counting(
        &self,
        constraint: &Constraint,
        counting: &Counting,
    ) -> Result<(), Diagnostic> {
        use counting::{CardinalityCounts, CountSelector};
        let mismatch = || invalid("counting predicate name does not match its executable contract");
        let name = CountingName::from_name(&constraint.reference.predicate).ok_or_else(mismatch)?;
        counting.validate_shape().map_err(invalid)?;
        let actual = match counting {
            Counting::Count {
                entries,
                selector,
                count,
                relation,
                condition,
                code,
            } => {
                for entry in entries {
                    self.validate_operand(entry, ValueType::Integer)?;
                }
                self.validate_operand(count, ValueType::Integer)?;
                if let Some(operand) = condition.operand() {
                    self.validate_operand(operand, ValueType::Boolean)?;
                }
                if let CountSelector::Needle(needle) = selector {
                    self.validate_operand(needle, ValueType::Integer)?;
                }
                if let Some(code) = code {
                    if relations::coded_relation(*code) != Some(*relation)
                        || condition.mode() != RelationMode::Plain
                    {
                        return Err(mismatch());
                    }
                    match selector {
                        CountSelector::Needle(_) => CountingName::Coded { matches: false },
                        CountSelector::Matches(_) => CountingName::Coded { matches: true },
                        _ => return Err(mismatch()),
                    }
                } else {
                    match selector {
                        CountSelector::Needle(needle) => match relation {
                            IntegerRelation::Equal => CountingName::Count(condition.mode()),
                            IntegerRelation::GreaterEqual | IntegerRelation::LessEqual
                                if condition.mode() == RelationMode::Plain
                                    && matches!(needle, Operand::Literal(_))
                                    && matches!(count, Operand::Literal(_)) =>
                            {
                                if *relation == IntegerRelation::GreaterEqual {
                                    CountingName::AtLeast
                                } else {
                                    CountingName::AtMost
                                }
                            }
                            _ => return Err(mismatch()),
                        },
                        CountSelector::Members(_) | CountSelector::Distinct
                            if *relation == IntegerRelation::Equal
                                && condition.mode() == RelationMode::Plain =>
                        {
                            if matches!(selector, CountSelector::Distinct) {
                                CountingName::Nvalue
                            } else {
                                CountingName::Among
                            }
                        }
                        _ => return Err(mismatch()),
                    }
                }
            }
            Counting::Cardinality {
                entries,
                counts,
                closed,
                ..
            } => {
                for entry in entries {
                    self.validate_operand(entry, ValueType::Integer)?;
                }
                if let CardinalityCounts::Exact(counts) = counts {
                    for count in counts {
                        self.validate_operand(count, ValueType::Integer)?;
                    }
                }
                CountingName::Cardinality {
                    closed: *closed,
                    bounded: matches!(counts, CardinalityCounts::Bounds { .. }),
                }
            }
            Counting::Window {
                entries,
                membership,
                ..
            } => {
                let boolean = match membership {
                    Value::Boolean(_) => true,
                    Value::IntegerSet(_) => false,
                    _ => return Err(mismatch()),
                };
                for entry in entries {
                    self.validate_operand(
                        entry,
                        if boolean {
                            ValueType::Boolean
                        } else {
                            ValueType::Integer
                        },
                    )?;
                }
                CountingName::Window { boolean }
            }
        };
        if actual != name {
            return Err(mismatch());
        }
        Ok(())
    }

    fn validate_selection(&self, constraint: &Constraint) -> Result<(), Diagnostic> {
        let mismatch =
            || invalid("constraint report predicate does not match its selection operation");
        let name =
            SelectionName::from_name(&constraint.reference.predicate).ok_or_else(mismatch)?;
        match &constraint.predicate {
            Predicate::Element {
                indices,
                elements,
                result,
                layout,
            } => {
                if name.form != layout.form() || indices.len() != layout.index_count() {
                    return Err(mismatch());
                }
                layout.validate(elements.len()).map_err(invalid)?;
                for index in indices {
                    self.validate_operand(index, ValueType::Integer)?;
                }
                for element in elements {
                    self.validate_operand(element, name.kind)?;
                    if !name.allows_variables && !matches!(element, Operand::Literal(_)) {
                        return Err(invalid("element array requires fixed parameters"));
                    }
                }
                self.validate_operand(result, name.kind)?;
            }
            Predicate::Conditional {
                condition,
                then_value,
                else_value,
                result,
            } => {
                if name.form != SelectionForm::Conditional {
                    return Err(mismatch());
                }
                self.validate_operand(condition, ValueType::Boolean)?;
                for value in [then_value, else_value, result] {
                    self.validate_operand(value, name.kind)?;
                }
            }
            _ => return Err(mismatch()),
        }
        Ok(())
    }

    fn validate_set_channel(
        &self,
        constraint: &Constraint,
        channel: &SetChannel,
    ) -> Result<(), Diagnostic> {
        if SetChannelName::from_name(&constraint.reference.predicate) != Some(channel.name()) {
            return Err(invalid(
                "set-channel predicate name does not match its executable operation",
            ));
        }
        channel.validate_shape().map_err(invalid)?;
        for (operand, kind) in channel.typed_operands() {
            self.validate_operand(operand, kind)?;
        }
        Ok(())
    }

    fn validate_set_global(
        &self,
        constraint: &Constraint,
        global: &SetGlobal,
    ) -> Result<(), Diagnostic> {
        if SetGlobalName::from_name(&constraint.reference.predicate) != Some(global.name()) {
            return Err(invalid(
                "set-global predicate name does not match its executable operation or mode",
            ));
        }
        global.validate_shape().map_err(invalid)?;
        match global {
            SetGlobal::Aggregate {
                entries, result, ..
            } => {
                for operand in entries.iter().chain(std::iter::once(result)) {
                    self.validate_operand(operand, ValueType::IntegerSet)?;
                }
            }
            SetGlobal::Convex { set } => self.validate_operand(set, ValueType::IntegerSet)?,
            SetGlobal::Hull { set, result } => {
                self.validate_operand(set, ValueType::IntegerSet)?;
                self.validate_operand(result, ValueType::IntegerSet)?;
            }
            SetGlobal::Sequence { entries } | SetGlobal::AtMostOne { entries, .. } => {
                for operand in entries {
                    self.validate_operand(operand, ValueType::IntegerSet)?;
                }
            }
            SetGlobal::Weights { set, result, .. } => {
                self.validate_operand(set, ValueType::IntegerSet)?;
                self.validate_operand(result, ValueType::Integer)?;
            }
            SetGlobal::Extremum {
                set,
                integer,
                condition,
                ..
            } => {
                self.validate_operand(set, ValueType::IntegerSet)?;
                self.validate_operand(integer, ValueType::Integer)?;
                if let Some(operand) = condition.operand() {
                    self.validate_operand(operand, ValueType::Boolean)?;
                }
            }
        }
        Ok(())
    }

    fn validate_set(&self, constraint: &Constraint) -> Result<(), Diagnostic> {
        let mismatch = || {
            invalid("constraint report predicate does not match its set operation, mode or code")
        };
        let name = SetName::from_name(&constraint.reference.predicate).ok_or_else(mismatch)?;
        let (kind, condition) = match &constraint.predicate {
            Predicate::SetIn { element, set } | Predicate::SetMembership { element, set, .. } => {
                let element_type = match element {
                    Operand::Literal(value) => value.value_type(),
                    Operand::Variable(slot) => {
                        self.slots
                            .get(*slot)
                            .ok_or_else(|| invalid("membership element slot is out of bounds"))?
                            .kind
                    }
                };
                if !matches!(element_type, ValueType::Integer | ValueType::Boolean) {
                    return Err(invalid("membership element must be integer or Boolean"));
                }
                self.validate_operand(element, element_type)?;
                self.validate_operand(set, ValueType::IntegerSet)?;
                let SetKind::Membership { fixed } = name.kind else {
                    return Err(mismatch());
                };
                if (fixed || element_type == ValueType::Boolean)
                    && !matches!(set, Operand::Literal(_))
                {
                    return Err(invalid("this membership form requires a fixed integer set"));
                }
                let condition = match &constraint.predicate {
                    Predicate::SetMembership {
                        condition: RelationCondition::Plain,
                        ..
                    } => return Err(mismatch()),
                    Predicate::SetMembership { condition, .. } => Some(condition),
                    _ => None,
                };
                (name.kind, condition)
            }
            Predicate::SetEqual { left, right }
            | Predicate::SetSubset { left, right }
            | Predicate::SetRelation { left, right, .. } => {
                self.validate_operand(left, ValueType::IntegerSet)?;
                self.validate_operand(right, ValueType::IntegerSet)?;
                match &constraint.predicate {
                    Predicate::SetEqual { .. } => (SetKind::Relation(SetRelation::Equal), None),
                    Predicate::SetSubset { .. } => (SetKind::Relation(SetRelation::Subset), None),
                    Predicate::SetRelation {
                        relation,
                        condition,
                        native_code,
                        ..
                    } => {
                        if native_code.is_none()
                            && matches!(condition, RelationCondition::Plain)
                            && matches!(relation, SetRelation::Equal | SetRelation::Subset)
                        {
                            return Err(mismatch());
                        }
                        let kind = if let Some(code) = native_code {
                            if SetRelation::from_code(*code) != Some(*relation) {
                                return Err(mismatch());
                            }
                            SetKind::NativeRelation
                        } else {
                            SetKind::Relation(*relation)
                        };
                        (kind, Some(condition))
                    }
                    _ => unreachable!(),
                }
            }
            Predicate::SetCardinality { set, cardinality } => {
                self.validate_operand(set, ValueType::IntegerSet)?;
                self.validate_operand(cardinality, ValueType::Integer)?;
                (SetKind::Cardinality, None)
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
            }
            | Predicate::SetOperation {
                left,
                right,
                result,
                ..
            } => {
                for operand in [left, right, result] {
                    self.validate_operand(operand, ValueType::IntegerSet)?;
                }
                let operation = match &constraint.predicate {
                    Predicate::SetUnion { .. } => SetOperation::Union,
                    Predicate::SetIntersection { .. } => SetOperation::Intersection,
                    Predicate::SetOperation {
                        operation: SetOperation::Union | SetOperation::Intersection,
                        ..
                    } => return Err(mismatch()),
                    Predicate::SetOperation { operation, .. } => *operation,
                    _ => unreachable!(),
                };
                (SetKind::Operation(operation), None)
            }
            _ => return Err(mismatch()),
        };
        let mode = condition.map_or(RelationMode::Plain, RelationCondition::mode);
        if kind != name.kind || mode != name.mode {
            return Err(mismatch());
        }
        if let Some(operand) = condition.and_then(RelationCondition::operand) {
            self.validate_operand(operand, ValueType::Boolean)?;
        }
        Ok(())
    }

    fn validate_float(&self, constraint: &Constraint) -> Result<(), Diagnostic> {
        let mismatch =
            || invalid("constraint report predicate does not match its float operation or mode");
        let name = FloatName::from_name(&constraint.reference.predicate).ok_or_else(mismatch)?;
        let expected = match &constraint.predicate {
            Predicate::FloatRelation {
                relation,
                left,
                right,
                condition,
                code,
            } => {
                self.validate_operand(left, ValueType::Float)?;
                self.validate_operand(right, ValueType::Float)?;
                self.validate_float_condition(condition)?;
                if let Some(code) = code {
                    if relations::coded_relation(*code) != Some(*relation)
                        || condition.mode() != RelationMode::Implied
                    {
                        return Err(invalid("invalid coded float implication"));
                    }
                    FloatName::CodedImplication
                } else {
                    FloatName::Relation {
                        relation: *relation,
                        linear: false,
                        mode: condition.mode(),
                    }
                }
            }
            Predicate::FloatLinear {
                relation,
                coefficients,
                terms,
                condition,
                ..
            } => {
                if coefficients.len() != terms.len() {
                    return Err(invalid(
                        "float linear coefficient and term arrays must have equal lengths",
                    ));
                }
                for term in terms {
                    self.validate_operand(term, ValueType::Float)?;
                }
                self.validate_float_condition(condition)?;
                FloatName::Relation {
                    relation: *relation,
                    linear: true,
                    mode: condition.mode(),
                }
            }
            Predicate::FloatArithmetic {
                operation,
                inputs,
                result,
            } => {
                if inputs.len() != operation.input_count() {
                    return Err(invalid("float arithmetic has the wrong input count"));
                }
                for operand in inputs.iter().chain(std::iter::once(result)) {
                    self.validate_operand(operand, ValueType::Float)?;
                }
                if *operation == FloatOperation::FixedBasePower
                    && !matches!(&inputs[0], Operand::Literal(Value::Float(base)) if base.get() > 0.0)
                {
                    return Err(invalid("fixed-base power requires a fixed positive base"));
                }
                FloatName::Arithmetic(*operation)
            }
            Predicate::FloatIntegerPower {
                base,
                exponent,
                result,
            } => {
                self.validate_operand(base, ValueType::Float)?;
                self.validate_operand(result, ValueType::Float)?;
                floats::validate_integer_exponent(*exponent).map_err(invalid)?;
                FloatName::IntegerPower
            }
            Predicate::FloatArrayExtremum {
                extremum,
                inputs,
                result,
            } => {
                if inputs.is_empty() {
                    return Err(invalid("float array extremum needs a nonempty array"));
                }
                for operand in inputs.iter().chain(std::iter::once(result)) {
                    self.validate_operand(operand, ValueType::Float)?;
                }
                let FloatName::ArrayExtremum {
                    extremum: named, ..
                } = name
                else {
                    return Err(mismatch());
                };
                if named != *extremum {
                    return Err(mismatch());
                }
                name
            }
            Predicate::FloatConversion {
                operation,
                input,
                result,
            } => {
                self.validate_operand(input, operation.input_type())?;
                self.validate_operand(result, operation.output_type())?;
                FloatName::Conversion(*operation)
            }
            Predicate::FloatMembership {
                membership,
                element,
                intervals,
            } => {
                self.validate_operand(element, ValueType::Float)?;
                match membership {
                    FloatMembership::Interval if intervals.len() != 1 => {
                        return Err(invalid("float interval membership needs one endpoint pair"));
                    }
                    FloatMembership::Set
                        if FloatSet::from_ranges(intervals.clone()).ranges() != intervals =>
                    {
                        return Err(invalid("float set ranges must be canonical"));
                    }
                    _ => {}
                }
                FloatName::Membership(*membership)
            }
            _ => return Err(mismatch()),
        };
        if name != expected {
            return Err(mismatch());
        }
        Ok(())
    }

    fn validate_float_condition(&self, condition: &RelationCondition) -> Result<(), Diagnostic> {
        if let Some(operand) = condition.operand() {
            self.validate_operand(operand, ValueType::Boolean)?;
        }
        Ok(())
    }

    fn validate_boolean(&self, constraint: &Constraint) -> Result<(), Diagnostic> {
        let mismatch =
            || invalid("constraint report predicate does not match its Boolean operation or mode");
        let name = BooleanName::from_name(&constraint.reference.predicate).ok_or_else(mismatch)?;
        let (kind, arity, condition) = match &constraint.predicate {
            Predicate::BooleanRelation {
                relation,
                left,
                right,
                condition,
            } => {
                self.validate_operand(left, ValueType::Boolean)?;
                self.validate_operand(right, ValueType::Boolean)?;
                (
                    BooleanKind::Relation(*relation),
                    2 + usize::from(condition.operand().is_some()),
                    Some(condition),
                )
            }
            Predicate::BooleanLinear {
                relation,
                coefficients,
                terms,
                bound,
                condition,
            } => {
                if coefficients.len() != terms.len() {
                    return Err(invalid(
                        "Boolean linear coefficient and term arrays must have equal lengths",
                    ));
                }
                for term in terms {
                    self.validate_operand(term, ValueType::Boolean)?;
                }
                self.validate_operand(bound, ValueType::Integer)?;
                (
                    BooleanKind::Linear(*relation),
                    3 + usize::from(condition.operand().is_some()),
                    Some(condition),
                )
            }
            Predicate::BooleanOperation {
                operation,
                inputs,
                condition,
            } => {
                let (kind, count) = match name.kind {
                    BooleanKind::Scalar(_) => {
                        if inputs.len() != operation.input_count() {
                            return Err(invalid(
                                "scalar Boolean operation has the wrong input count",
                            ));
                        }
                        (BooleanKind::Scalar(*operation), inputs.len())
                    }
                    BooleanKind::Reduction(_) => (BooleanKind::Reduction(*operation), 1),
                    _ => return Err(mismatch()),
                };
                for input in inputs {
                    self.validate_operand(input, ValueType::Boolean)?;
                }
                (
                    kind,
                    count + usize::from(condition.operand().is_some()),
                    Some(condition),
                )
            }
            Predicate::BooleanClause {
                positive,
                negative,
                condition,
            } => {
                for input in positive.iter().chain(negative) {
                    self.validate_operand(input, ValueType::Boolean)?;
                }
                (
                    BooleanKind::Clause,
                    2 + usize::from(condition.operand().is_some()),
                    Some(condition),
                )
            }
            Predicate::BooleanToInteger { boolean, integer } => {
                self.validate_operand(boolean, ValueType::Boolean)?;
                self.validate_operand(integer, ValueType::Integer)?;
                (BooleanKind::ToInteger, 2, None)
            }
            _ => unreachable!("Boolean predicate selected by validation"),
        };
        let mode = condition.map_or(RelationMode::Plain, RelationCondition::mode);
        if name.kind != kind || name.mode_for_arity(arity) != Some(mode) {
            return Err(mismatch());
        }
        if let Some(operand) = condition.and_then(RelationCondition::operand) {
            self.validate_operand(operand, ValueType::Boolean)?;
        }
        Ok(())
    }

    fn validate_relation(
        &self,
        constraint: &Constraint,
        relation: IntegerRelation,
        linear: bool,
        condition: &RelationCondition,
    ) -> Result<(), Diagnostic> {
        if RelationName::from_name(&constraint.reference.predicate)
            != Some(RelationName {
                relation,
                linear,
                mode: condition.mode(),
            })
        {
            return Err(invalid(
                "constraint report predicate does not match its operation",
            ));
        }
        if let Some(operand) = condition.operand() {
            self.validate_operand(operand, ValueType::Boolean)?;
        }
        Ok(())
    }

    fn validate_linear(&self, coefficients: &[i64], terms: &[Operand]) -> Result<(), Diagnostic> {
        if coefficients.len() != terms.len() {
            return Err(invalid(
                "linear coefficient and term arrays must have equal lengths",
            ));
        }
        for term in terms {
            self.validate_operand(term, ValueType::Integer)?;
        }
        Ok(())
    }

    fn validate_operand(&self, operand: &Operand, expected: ValueType) -> Result<(), Diagnostic> {
        let kind = match operand {
            Operand::Literal(value) => value.value_type(),
            Operand::Variable(slot) => {
                self.slots
                    .get(*slot)
                    .ok_or_else(|| invalid("operand refers to an unknown slot"))?
                    .kind
            }
        };
        if kind != expected {
            return Err(invalid(
                "operand type does not match its operation or binding",
            ));
        }
        Ok(())
    }

    fn validate_reference(&self, reference: &VariableRef, slot: usize) -> Result<(), Diagnostic> {
        if slot >= self.slots.len() {
            return Err(invalid("variable refers to an unknown slot"));
        }
        validate_location(&reference.location)?;
        let binding = self
            .bindings
            .get(&reference.name)
            .ok_or_else(|| invalid("variable reference names an unknown binding"))?;
        let index = match (binding.is_array, reference.position) {
            (false, None) => 0,
            (true, Some(position)) if position > 0 => position - 1,
            _ => {
                return Err(invalid(
                    "variable reference shape does not match its binding",
                ));
            }
        };
        if !binding.assignable
            || !matches!(binding.elements.get(index), Some(Operand::Variable(bound)) if *bound == slot)
        {
            return Err(invalid(
                "variable reference does not match its binding slot",
            ));
        }
        Ok(())
    }
}

fn validate_location(location: &Location) -> Result<(), Diagnostic> {
    if location.start > location.end || location.line == 0 || location.column == 0 {
        return Err(invalid("invalid report location"));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        location: None,
        message: message.into(),
    }
}
