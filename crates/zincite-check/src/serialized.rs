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
        if let Some((_, operand)) = self.objective {
            self.validate_operand(operand, ValueType::Integer)?;
        }
        Ok(())
    }

    fn validate_bindings(&self) -> Result<(), Diagnostic> {
        for (name, binding) in &self.bindings {
            if name.is_empty() || (!binding.is_array && binding.elements.len() != 1) {
                return Err(invalid("invalid binding name or scalar shape"));
            }
            for &operand in &binding.elements {
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
                    Domain::Boolean => ValueType::Boolean,
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
        let defined_slots: HashSet<_> = self
            .constraints
            .iter()
            .filter_map(|constraint| {
                let name = constraint.reference.metadata.defines.as_ref()?;
                let binding = self.bindings.get(name)?;
                match binding.elements.as_slice() {
                    [Operand::Variable(slot)] if !binding.is_array && binding.assignable => {
                        Some(*slot)
                    }
                    _ => None,
                }
            })
            .collect();
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
                match slot.default {
                    Some(value)
                        if value.value_type() == slot.kind && seeds[index].contains(&value) => {}
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
                Predicate::Linear {
                    coefficients,
                    terms,
                    equality,
                    ..
                } => {
                    if coefficients.len() != terms.len() {
                        return Err(invalid(
                            "linear coefficient and term arrays must have equal lengths",
                        ));
                    }
                    for &term in terms {
                        self.validate_operand(term, ValueType::Integer)?;
                    }
                    if *equality {
                        "int_lin_eq"
                    } else {
                        "int_lin_le"
                    }
                }
                Predicate::ReifiedLessEqual {
                    left,
                    right,
                    result,
                } => {
                    self.validate_operand(*left, ValueType::Integer)?;
                    self.validate_operand(*right, ValueType::Integer)?;
                    self.validate_operand(*result, ValueType::Boolean)?;
                    "int_le_reif"
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

    fn validate_operand(&self, operand: Operand, expected: ValueType) -> Result<(), Diagnostic> {
        let kind = match operand {
            Operand::Literal(value) => value.value_type(),
            Operand::Variable(slot) => {
                self.slots
                    .get(slot)
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
