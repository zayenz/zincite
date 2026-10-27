//! FlatZinc-JSON 1.0 admission into the text lowerer's expression and type contracts.
//! Raw slices preserve numeric spelling and JSON locations; no evaluator lives here.

use super::*;
use serde::de::{MapAccess, Visitor};
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::ops::Range;

/// Prepare official compiler model JSON, distinct from a saved executable artifact.
pub fn prepare_json(name: &str, text: &str) -> std::result::Result<PreparedModel, Vec<Diagnostic>> {
    let mut json = JsonSource {
        name,
        original: text,
        syntax: String::new(),
        locations: BTreeMap::new(),
        literals: HashMap::new(),
        types: HashMap::new(),
    };
    let normalized = (|| {
        let raw: &RawValue = serde_json::from_str(text).map_err(|error| Diagnostic {
            location: Some(json.offset_location(error.line(), error.column())),
            message: format!("invalid FlatZinc-JSON: {error}"),
        })?;
        json.model(raw)
    })();
    normalized.map_err(|error| vec![error])?;
    let parsed = zincite_syntax::parse_flatzinc(&json.syntax, FileMode::Model);
    let source = Source {
        name,
        parsed: &parsed,
        json: Some(&json),
    };
    source.validate_syntax()?;
    Lowerer::new(&source)
        .and_then(Lowerer::lower)
        .map_err(|error| vec![error])
}

pub(super) struct JsonSource<'a> {
    name: &'a str,
    original: &'a str,
    syntax: String,
    locations: BTreeMap<usize, (usize, &'a RawValue)>,
    literals: HashMap<usize, &'a RawValue>,
    pub(super) types: HashMap<usize, TypeSpec>,
}

impl<'a> JsonSource<'a> {
    fn model(&mut self, raw: &'a RawValue) -> Result<()> {
        let root = self.object(
            raw,
            &[
                "version",
                "variables",
                "arrays",
                "constraints",
                "solve",
                "output",
            ],
        )?;
        let version = self.required(&root, "version", raw)?;
        if self.string(version)? != "1.0" {
            return Err(self.error(version, "unsupported FlatZinc-JSON version; expected 1.0"));
        }
        let variables = self.required(&root, "variables", raw)?;
        let arrays = self.required(&root, "arrays", raw)?;
        let variables = self.members(variables)?;
        let arrays = self.members(arrays)?;
        let mut declarations = BTreeMap::new();
        for (name, declaration) in variables.iter().chain(arrays.iter()) {
            self.identifier(name, declaration)?;
            if declarations.insert(name.clone(), *declaration).is_some() {
                return Err(self.error(declaration, format!("duplicate declaration '{name}'")));
            }
        }
        let output_raw = self.required(&root, "output", raw)?;
        let mut outputs = HashSet::new();
        for output in self.list(output_raw)? {
            let name = self.string(output)?;
            if !declarations.contains_key(&name) || !outputs.insert(name) {
                return Err(self.error(output, "output needs distinct declared identifiers"));
            }
        }
        let mut kinds = HashMap::new();
        for (name, variable) in &variables {
            let object = self.object(
                variable,
                &["type", "domain", "rhs", "introduced", "defined", "ann"],
            )?;
            let ty = self.required(&object, "type", variable)?;
            kinds.insert(name.clone(), self.value_type(ty)?);
        }
        for (name, array) in &arrays {
            self.array_kind(name, array, &arrays, &mut kinds, &mut HashSet::new())?;
        }
        self.variables(&variables, &kinds, &outputs)?;
        self.arrays(&arrays, &declarations, &kinds, &outputs)?;
        self.constraints(self.required(&root, "constraints", raw)?)?;
        self.solve(self.required(&root, "solve", raw)?)
    }

    fn variables(
        &mut self,
        variables: &BTreeMap<String, &'a RawValue>,
        kinds: &HashMap<String, ValueType>,
        outputs: &HashSet<String>,
    ) -> Result<()> {
        for (name, variable) in variables {
            let object = self.object(
                variable,
                &["type", "domain", "rhs", "introduced", "defined", "ann"],
            )?;
            let kind = kinds[name];
            let domain = match object.get("domain") {
                Some(domain) => self.domain(domain, kind)?,
                None => base_domain(kind),
            };
            let start = self.syntax.len();
            self.syntax
                .push_str(&format!("var {}: {name}", type_name(kind)));
            self.annotations(&object)?;
            if outputs.contains(name) {
                self.syntax.push_str(" :: output_var");
            }
            if let Some(rhs) = object.get("rhs") {
                self.syntax.push_str(" = ");
                self.expression(rhs, ExpressionRole::Literal)?;
            }
            self.syntax.push_str(";\n");
            self.types.insert(
                start,
                TypeSpec {
                    kind,
                    domain,
                    is_var: true,
                    length: None,
                    is_array: false,
                },
            );
            self.mark(start, variable);
        }
        Ok(())
    }

    fn arrays(
        &mut self,
        arrays: &BTreeMap<String, &'a RawValue>,
        declarations: &BTreeMap<String, &'a RawValue>,
        kinds: &HashMap<String, ValueType>,
        outputs: &HashSet<String>,
    ) -> Result<()> {
        for (name, array) in arrays {
            let object = self.object(array, &["a", "introduced", "defined", "ann"])?;
            let contents = self.required(&object, "a", array)?;
            let (length, fixed) = self.array_shape(contents, declarations, &mut HashSet::new())?;
            let is_var = !fixed
                || length == 0
                || outputs.contains(name)
                || object
                    .get("defined")
                    .is_some_and(|flag| flag.get() == "true");
            let kind = kinds[name];
            if is_var && kind == ValueType::FloatSet {
                return Err(self.error(array, "assignable float sets are unsupported"));
            }
            let start = self.syntax.len();
            self.syntax.push_str(&format!(
                "array[1..{length}] of {}{}: {name}",
                if is_var { "var " } else { "" },
                type_name(kind)
            ));
            self.annotations(&object)?;
            if outputs.contains(name) {
                self.syntax
                    .push_str(&format!(" :: output_array([1..{length}])"));
            }
            self.syntax.push_str(" = ");
            self.expression(contents, ExpressionRole::Argument)?;
            self.syntax.push_str(";\n");
            self.types.insert(
                start,
                TypeSpec {
                    kind,
                    domain: base_domain(kind),
                    is_var,
                    length: Some(length),
                    is_array: true,
                },
            );
            self.mark(start, array);
        }
        Ok(())
    }

    fn constraints(&mut self, constraints: &'a RawValue) -> Result<()> {
        for constraint in self.list(constraints)? {
            let object = self.object(constraint, &["id", "args", "ann", "defines", "defined"])?;
            let start = self.syntax.len();
            self.syntax.push_str("constraint ");
            self.call(&object, constraint, ExpressionRole::Argument)?;
            self.annotations(&object)?;
            if let Some(defines) = object.get("defines") {
                let target = self.string(defines)?;
                self.identifier(&target, defines)?;
                self.syntax.push_str(&format!(" :: defines_var({target})"));
            }
            self.syntax.push_str(";\n");
            self.mark(start, constraint);
        }
        Ok(())
    }

    fn solve(&mut self, solve: &'a RawValue) -> Result<()> {
        let object = self.object(solve, &["method", "objective", "ann"])?;
        let method_raw = self.required(&object, "method", solve)?;
        let method = self.string(method_raw)?;
        let start = self.syntax.len();
        self.syntax.push_str("solve");
        self.annotations(&object)?;
        match method.as_str() {
            "satisfy" if !object.contains_key("objective") => self.syntax.push_str(" satisfy"),
            "minimize" | "maximize" => {
                let objective = self.required(&object, "objective", solve)?;
                self.syntax.push_str(&format!(" {method} "));
                self.expression(objective, ExpressionRole::Literal)?;
            }
            _ => return Err(self.error(method_raw, "solve needs satisfy without an objective, or minimize/maximize with a scalar objective")),
        }
        self.syntax.push_str(";\n");
        self.mark(start, solve);
        Ok(())
    }

    fn array_kind(
        &self,
        name: &str,
        raw: &'a RawValue,
        arrays: &BTreeMap<String, &'a RawValue>,
        kinds: &mut HashMap<String, ValueType>,
        visiting: &mut HashSet<String>,
    ) -> Result<ValueType> {
        if let Some(kind) = kinds.get(name) {
            return Ok(*kind);
        }
        if !visiting.insert(name.into()) {
            return Err(self.error(raw, "cyclic array reference"));
        }
        let object = self.object(raw, &["a", "ann", "introduced", "defined"])?;
        let contents = self.required(&object, "a", raw)?;
        let kind = if contents.get().starts_with('"') {
            let alias = self.string(contents)?;
            let target = arrays
                .get(&alias)
                .ok_or_else(|| self.error(contents, "array alias needs a declared array"))?;
            self.array_kind(&alias, target, arrays, kinds, visiting)?
        } else {
            let mut kind = None;
            for element in self.list(contents)? {
                let next = if element.get().starts_with('"') {
                    let target = self.string(element)?;
                    if arrays.contains_key(&target) {
                        return Err(self.error(element, "array elements must be scalar"));
                    }
                    *kinds.get(&target).ok_or_else(|| {
                        self.error(element, format!("unknown reference '{target}'"))
                    })?
                } else {
                    self.literal_kind(element)?
                };
                kind = Some(match (kind, next) {
                    (None, next) => next,
                    (Some(previous), next) if previous == next => previous,
                    (
                        Some(ValueType::Integer | ValueType::Float),
                        ValueType::Integer | ValueType::Float,
                    ) => ValueType::Float,
                    (
                        Some(ValueType::IntegerSet | ValueType::FloatSet),
                        ValueType::IntegerSet | ValueType::FloatSet,
                    ) => ValueType::FloatSet,
                    _ => return Err(self.error(element, "array elements have incompatible types")),
                });
            }
            // Empty bindings contain no typed operand. Reads accept every element type.
            kind.unwrap_or(ValueType::Integer)
        };
        visiting.remove(name);
        kinds.insert(name.into(), kind);
        Ok(kind)
    }

    fn array_shape(
        &self,
        raw: &'a RawValue,
        declarations: &BTreeMap<String, &'a RawValue>,
        visiting: &mut HashSet<String>,
    ) -> Result<(usize, bool)> {
        if raw.get().starts_with('"') {
            let name = self.string(raw)?;
            if !visiting.insert(name.clone()) {
                return Err(self.error(raw, "cyclic array reference"));
            }
            let declaration = declarations
                .get(&name)
                .ok_or_else(|| self.error(raw, "unknown array alias"))?;
            let object = self.members(declaration)?;
            let contents = self.required(&object, "a", declaration)?;
            return self.array_shape(contents, declarations, visiting);
        }
        let elements = self.list(raw)?;
        Ok((
            elements.len(),
            elements
                .iter()
                .all(|element| !element.get().starts_with('"')),
        ))
    }

    fn domain(&self, raw: &'a RawValue, kind: ValueType) -> Result<Domain> {
        match kind {
            ValueType::Integer => {
                let values = integer_set(raw).map_err(|message| self.error(raw, message))?;
                Ok(match values.contiguous_bounds() {
                    Some((lower, upper)) => Domain::Interval { lower, upper },
                    None if values.cardinality() == Some(0) => {
                        Domain::Enumeration { values: Vec::new() }
                    }
                    None => Domain::IntegerRanges { values },
                })
            }
            ValueType::IntegerSet => Ok(Domain::IntegerSet {
                upper: Some(integer_set(raw).map_err(|message| self.error(raw, message))?),
            }),
            ValueType::Float => {
                let values = float_set(raw).map_err(|message| self.error(raw, message))?;
                Ok(match values.ranges() {
                    [(lower, upper)] => Domain::FloatInterval {
                        lower: *lower,
                        upper: *upper,
                    },
                    _ => Domain::FloatRanges { values },
                })
            }
            ValueType::Boolean => {
                let ranges: Vec<[bool; 2]> = serde_json::from_str(raw.get()).map_err(|_| {
                    self.error(raw, "Boolean domain needs ordered Boolean range pairs")
                })?;
                let mut values = Vec::new();
                for [lower, upper] in ranges {
                    if (lower && !upper) || values.last().is_some_and(|last| *last || !lower) {
                        return Err(self.error(
                            raw,
                            "domain ranges must be ordered, nonoverlapping and not reversed",
                        ));
                    }
                    values.push(lower);
                    if lower != upper {
                        values.push(upper);
                    }
                }
                Ok(if values == [false, true] {
                    Domain::Boolean
                } else {
                    Domain::BooleanValues { values }
                })
            }
            ValueType::FloatSet => unreachable!("scalar declarations have no float-set type"),
        }
    }

    fn annotations(&mut self, object: &BTreeMap<String, &'a RawValue>) -> Result<()> {
        if let Some(annotations) = object.get("ann") {
            for annotation in self.list(annotations)? {
                self.syntax.push_str(" :: ");
                self.expression(annotation, ExpressionRole::Annotation)?;
            }
        }
        for (field, annotation) in [
            ("introduced", "var_is_introduced"),
            ("defined", "is_defined_var"),
        ] {
            if let Some(value) = object.get(field) {
                let enabled: bool = serde_json::from_str(value.get())
                    .map_err(|_| self.error(value, format!("{field} must be Boolean")))?;
                if enabled {
                    self.syntax.push_str(&format!(" :: {annotation}"));
                }
            }
        }
        Ok(())
    }

    fn call(
        &mut self,
        object: &BTreeMap<String, &'a RawValue>,
        raw: &'a RawValue,
        role: ExpressionRole,
    ) -> Result<()> {
        let id_raw = self.required(object, "id", raw)?;
        let id = self.string(id_raw)?;
        self.identifier(&id, id_raw)?;
        self.syntax.push_str(&id);
        self.syntax.push('(');
        let args = self.required(object, "args", raw)?;
        for (index, argument) in self.list(args)?.into_iter().enumerate() {
            if index != 0 {
                self.syntax.push(',');
            }
            self.expression(argument, role)?;
        }
        self.syntax.push(')');
        Ok(())
    }

    fn expression(&mut self, raw: &'a RawValue, role: ExpressionRole) -> Result<()> {
        let start = self.syntax.len();
        match raw.get().as_bytes().first() {
            Some(b'"') => {
                let name = self.string(raw)?;
                self.identifier(&name, raw)?;
                self.syntax.push_str(&name);
            }
            Some(b'[') if role != ExpressionRole::Literal => {
                self.syntax.push('[');
                for (index, element) in self.list(raw)?.into_iter().enumerate() {
                    if index != 0 {
                        self.syntax.push(',');
                    }
                    self.expression(
                        element,
                        if role == ExpressionRole::Annotation {
                            role
                        } else {
                            ExpressionRole::Literal
                        },
                    )?;
                }
                self.syntax.push(']');
            }
            Some(b'{') => {
                let object = self.members(raw)?;
                if let Some(string) = object.get("string").filter(|_| object.len() == 1) {
                    self.syntax.push_str(&quote(&self.string(string)?));
                } else if let Some(set) = object.get("set").filter(|_| object.len() == 1) {
                    // Validate the range shape now; numeric roles are checked at use.
                    let _: Vec<[&RawValue; 2]> = serde_json::from_str(set.get()).map_err(|_| {
                        self.error(set, "set needs an array of numeric range pairs")
                    })?;
                    self.syntax.push_str("{}");
                    self.literals.insert(start, raw);
                } else if role != ExpressionRole::Literal {
                    self.object(raw, &["id", "args", "ann"])?;
                    self.call(&object, raw, ExpressionRole::Annotation)?;
                    self.annotations(&object)?;
                } else {
                    return Err(self.error(raw, "expected a literal or reference"));
                }
            }
            Some(b't' | b'f' | b'-' | b'0'..=b'9') => {
                // JSON has already validated lexical spelling. Typed admission uses the raw token.
                self.syntax.push_str(raw.get());
                self.literals.insert(start, raw);
            }
            _ => {
                return Err(self.error(
                    raw,
                    "expected a FlatZinc literal, reference, array or annotation",
                ));
            }
        }
        self.mark(start, raw);
        Ok(())
    }

    pub(super) fn literal(&self, node: &SyntaxNode) -> Option<&'a RawValue> {
        self.literals.get(&node.range().start).copied()
    }

    fn literal_kind(&self, raw: &'a RawValue) -> Result<ValueType> {
        match raw.get().as_bytes().first() {
            Some(b't' | b'f') => Ok(ValueType::Boolean),
            Some(b'-' | b'0'..=b'9') if !raw.get().contains(['.', 'e', 'E']) => {
                if integer(raw).is_ok() {
                    Ok(ValueType::Integer)
                } else {
                    float(raw).map_err(|message| self.error(raw, message))?;
                    Ok(ValueType::Float)
                }
            }
            Some(b'-' | b'0'..=b'9') => {
                float(raw).map_err(|message| self.error(raw, message))?;
                Ok(ValueType::Float)
            }
            Some(b'{') => {
                let object = self.object(raw, &["set"])?;
                let set = self.required(&object, "set", raw)?;
                let ranges: Vec<[&RawValue; 2]> = serde_json::from_str(set.get())
                    .map_err(|_| self.error(set, "set needs numeric range pairs"))?;
                Ok(
                    if ranges
                        .iter()
                        .flatten()
                        .any(|number| integer(number).is_err())
                    {
                        ValueType::FloatSet
                    } else {
                        ValueType::IntegerSet
                    },
                )
            }
            _ => Err(self.error(
                raw,
                "array elements must be supported scalar literals or references",
            )),
        }
    }

    fn value_type(&self, raw: &'a RawValue) -> Result<ValueType> {
        match self.string(raw)?.as_str() {
            "bool" => Ok(ValueType::Boolean),
            "int" => Ok(ValueType::Integer),
            "float" => Ok(ValueType::Float),
            "set of int" => Ok(ValueType::IntegerSet),
            _ => Err(self.error(raw, "unsupported FlatZinc-JSON variable type")),
        }
    }

    fn identifier(&self, name: &str, raw: &'a RawValue) -> Result<()> {
        if !name
            .as_bytes()
            .first()
            .is_some_and(|first| first.is_ascii_alphabetic() || *first == b'_')
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(self.error(raw, "expected an unquoted FlatZinc identifier"));
        }
        Ok(())
    }

    fn required(
        &self,
        object: &BTreeMap<String, &'a RawValue>,
        field: &str,
        owner: &'a RawValue,
    ) -> Result<&'a RawValue> {
        object
            .get(field)
            .copied()
            .ok_or_else(|| self.error(owner, format!("missing FlatZinc-JSON field '{field}'")))
    }

    fn string(&self, raw: &'a RawValue) -> Result<String> {
        serde_json::from_str(raw.get()).map_err(|_| self.error(raw, "expected a JSON string"))
    }
    fn list(&self, raw: &'a RawValue) -> Result<Vec<&'a RawValue>> {
        serde_json::from_str(raw.get()).map_err(|_| self.error(raw, "expected a JSON array"))
    }
    fn members(&self, raw: &'a RawValue) -> Result<BTreeMap<String, &'a RawValue>> {
        serde_json::from_str::<Object<'a>>(raw.get())
            .map(|object| object.0)
            .map_err(|error| self.error(raw, format!("invalid JSON object: {error}")))
    }
    fn object(&self, raw: &'a RawValue, fields: &[&str]) -> Result<BTreeMap<String, &'a RawValue>> {
        let object = self.members(raw)?;
        if let Some((field, value)) = object
            .iter()
            .find(|(field, _)| !fields.contains(&field.as_str()))
        {
            return Err(self.error(value, format!("unknown FlatZinc-JSON field '{field}'")));
        }
        Ok(object)
    }
    fn mark(&mut self, start: usize, raw: &'a RawValue) {
        self.locations.insert(start, (self.syntax.len(), raw));
    }
    fn raw_location(&self, raw: &RawValue) -> Location {
        let start = raw.get().as_ptr() as usize - self.original.as_ptr() as usize;
        self.location_at(start, start + raw.get().len())
    }

    fn location_at(&self, start: usize, end: usize) -> Location {
        let mut line = 1;
        let mut column = 1;
        let mut previous_cr = false;
        for character in self.original[..start].chars() {
            match character {
                '\r' => {
                    line += 1;
                    column = 1;
                }
                '\n' => {
                    if !previous_cr {
                        line += 1;
                    }
                    column = 1;
                }
                _ => column += 1,
            }
            previous_cr = character == '\r';
        }
        Location {
            source: self.name.into(),
            start,
            end,
            line,
            column,
        }
    }

    fn offset_location(&self, line: usize, column: usize) -> Location {
        // Serde reports one-based byte columns and counts LF line breaks.
        let mut start = (self
            .original
            .split_inclusive('\n')
            .take(line.saturating_sub(1))
            .map(str::len)
            .sum::<usize>()
            + column.saturating_sub(1))
        .min(self.original.len());
        while !self.original.is_char_boundary(start) {
            start -= 1;
        }
        self.location_at(start, start)
    }

    pub(super) fn location(&self, range: Range<usize>) -> Location {
        self.locations
            .range(..=range.start)
            .rev()
            .find_map(|(_, (end, raw))| (*end >= range.end).then(|| self.raw_location(raw)))
            .unwrap_or_else(|| self.offset_location(1, 1))
    }
    fn error(&self, raw: &RawValue, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            location: Some(self.raw_location(raw)),
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExpressionRole {
    Literal,
    Argument,
    Annotation,
}

fn type_name(kind: ValueType) -> &'static str {
    match kind {
        ValueType::Integer => "int",
        ValueType::Float => "float",
        ValueType::Boolean => "bool",
        ValueType::IntegerSet => "set of int",
        ValueType::FloatSet => "set of float",
    }
}
fn base_domain(kind: ValueType) -> Domain {
    match kind {
        ValueType::Integer => Domain::Integer,
        ValueType::Float => Domain::Float,
        ValueType::Boolean => Domain::Boolean,
        ValueType::IntegerSet => Domain::IntegerSet { upper: None },
        ValueType::FloatSet => Domain::FloatSet,
    }
}

pub(super) fn integer(raw: &RawValue) -> std::result::Result<i64, &'static str> {
    raw.get()
        .parse()
        .map_err(|_| "expected an exact signed 64-bit integer")
}
pub(super) fn float(raw: &RawValue) -> std::result::Result<FiniteFloat, &'static str> {
    let value = raw
        .get()
        .parse()
        .map_err(|_| "expected a finite JSON number")?;
    FiniteFloat::new(value)
}
pub(super) fn literal(raw: &RawValue, kind: ValueType) -> std::result::Result<Value, String> {
    match kind {
        ValueType::Integer => integer(raw).map(Value::Integer).map_err(str::to_owned),
        ValueType::Float => float(raw).map(Value::Float).map_err(str::to_owned),
        ValueType::Boolean => serde_json::from_str(raw.get())
            .map(Value::Boolean)
            .map_err(|_| "expected a Boolean literal".into()),
        ValueType::IntegerSet | ValueType::FloatSet => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Set<'a> {
                #[serde(borrow)]
                set: &'a RawValue,
            }
            let set: Set<'_> = serde_json::from_str(raw.get())
                .map_err(|_| "expected a set literal".to_string())?;
            if kind == ValueType::IntegerSet {
                integer_set(set.set).map(Value::IntegerSet)
            } else {
                float_set(set.set).map(Value::FloatSet)
            }
        }
    }
}
fn integer_set(raw: &RawValue) -> std::result::Result<IntegerSet, String> {
    let ranges: Vec<[&RawValue; 2]> =
        serde_json::from_str(raw.get()).map_err(|_| "expected integer range pairs".to_string())?;
    let mut set = IntegerSet::empty();
    let mut previous = None;
    for [lower, upper] in ranges {
        let lower = integer(lower).map_err(str::to_owned)?;
        let upper = integer(upper).map_err(str::to_owned)?;
        if lower > upper || previous.is_some_and(|previous| previous >= lower) {
            return Err("ranges must be ordered, nonoverlapping and not reversed".into());
        }
        set = set.union(&IntegerSet::interval(lower, upper));
        previous = Some(upper);
    }
    Ok(set)
}
fn float_set(raw: &RawValue) -> std::result::Result<FloatSet, String> {
    let pairs: Vec<[&RawValue; 2]> = serde_json::from_str(raw.get())
        .map_err(|_| "expected finite float range pairs".to_string())?;
    let mut ranges = Vec::new();
    for [lower, upper] in pairs {
        let lower = float(lower).map_err(str::to_owned)?;
        let upper = float(upper).map_err(str::to_owned)?;
        if lower > upper
            || ranges
                .last()
                .is_some_and(|(_, previous)| *previous >= lower)
        {
            return Err("ranges must be ordered, nonoverlapping and not reversed".into());
        }
        ranges.push((lower, upper));
    }
    Ok(FloatSet::from_ranges(ranges))
}
fn quote(text: &str) -> String {
    let mut quoted = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            control if control.is_control() && u32::from(control) < 128 => {
                quoted.push_str(&format!("\\{:03o}", u32::from(control)))
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

// Unlike a generic map, declaration and call objects reject duplicate JSON keys.
struct Object<'a>(BTreeMap<String, &'a RawValue>);
impl<'de> Deserialize<'de> for Object<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = Object<'de>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an object with distinct keys")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut access: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut members = BTreeMap::new();
                while let Some((key, value)) = access.next_entry::<String, &'de RawValue>()? {
                    if members.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!("duplicate key '{key}'")));
                    }
                }
                Ok(Object(members))
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}
