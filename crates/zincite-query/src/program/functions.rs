//! Request-local function preparation and native type admission.
use super::*;
use zirql::{DeclaredType as D, ProjectionHost, ValueExpression as E, ValueExpressionKind as K};

#[derive(Clone, Copy, Debug)]
pub(super) enum Primitive {
    DirectChildren,
    NodeKind,
    WrittenCallName,
}
impl Primitive {
    pub(super) fn named(name: &str) -> Option<Self> {
        match name {
            "direct_children" => Some(Self::DirectChildren),
            "node_kind" => Some(Self::NodeKind),
            "written_call_name" => Some(Self::WrittenCallName),
            _ => None,
        }
    }
}

impl Parser<'_> {
    pub(super) fn collect_functions(&mut self) -> Result<(), QueryError> {
        use zincite_syntax::TokenKind as T;
        let mut depth = 0usize;
        for (index, token) in self.lexed.tokens().iter().enumerate() {
            if depth == 0 && &self.cursor.source[token.range.clone()] == "fn" {
                let mut following = self.lexed.tokens()[index + 1..].iter().filter(|token| {
                    !matches!(token.kind, T::Whitespace | T::LineComment | T::BlockComment)
                });
                if let (Some(name), Some(open)) = (following.next(), following.next())
                    && open.kind == T::LeftParen
                {
                    self.function_names
                        .insert(self.cursor.source[name.range.clone()].to_owned());
                }
            }
            match token.kind {
                T::LeftParen | T::LeftBracket | T::LeftBrace | T::MatrixStart => depth += 1,
                T::RightParen | T::RightBracket | T::RightBrace | T::MatrixEnd => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
        }
        let mut position = 0;
        let mut depth = 0usize;
        while position < self.lexed.tokens().len() {
            let token = self.lexed.tokens()[position].clone();
            if depth == 0 && &self.cursor.source[token.range.clone()] == "fn" {
                let start = token.range.start;
                let mut reader = zirql::ArgumentReader::new(self.cursor.source, start)
                    .map_err(pipeline::core_error)?;
                let mut budget =
                    std::mem::replace(&mut self.preflight, Budget::for_program(self.limits.core()));
                let result = zirql::read_unresolved_function_declaration(
                    &mut reader,
                    self,
                    &mut budget,
                    self.limits.nesting,
                );
                self.preflight = budget;
                if let Some(mut function) = result.map_err(pipeline::core_error)? {
                    for (_, value) in &mut function.bindings {
                        lower_primitives(value, &BTreeMap::new(), &self.queries);
                    }
                    lower_primitives(&mut function.value, &BTreeMap::new(), &self.queries);
                    self.report_model_demand |= signature_needs_model(&function.result.value)
                        || function
                            .parameters
                            .iter()
                            .any(|parameter| signature_needs_model(&parameter.value.declaration));
                    self.function_ranges.push(start..reader.position());
                    self.functions.push(function);
                    position = self
                        .lexed
                        .tokens()
                        .partition_point(|token| token.range.start < reader.position());
                    continue;
                }
            }
            match token.kind {
                T::LeftParen | T::LeftBracket | T::LeftBrace | T::MatrixStart => depth += 1,
                T::RightParen | T::RightBracket | T::RightBrace | T::MatrixEnd => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
            position += 1;
        }
        self.cursor.position = 0;
        Ok(())
    }
    pub(super) fn function_expression(
        &mut self,
        range: Range<usize>,
    ) -> Result<(Expression, Kind, bool), QueryError> {
        let expression = self.lower_function_value(range.clone())?;
        self.cursor.position = range.end;
        Ok((
            value_stage(Stage::FunctionValue(expression), range),
            Kind::Constructed,
            false,
        ))
    }
    pub(super) fn lower_function_value(
        &mut self,
        range: Range<usize>,
    ) -> Result<E<Expression>, QueryError> {
        self.begin_pure(range.clone())?;
        self.admit_cst(&range)?;
        let mut lowered = comprehension::lower::<Expression>(
            &self.cursor.source[range.clone()],
            range.clone(),
            &mut self.preflight,
            self.limits.nesting,
            comprehension::ExpressionPolicy::Functions,
        )?;
        self.report_model_demand |= !lowered.members.is_empty();
        map_value(
            &mut lowered.expression,
            &|range| range,
            &mut lowered.members,
        );
        lower_primitives(&mut lowered.expression, &self.names, &self.queries);
        Ok(lowered.expression)
    }
    pub(super) fn function_value_end(&self, start: usize) -> usize {
        use zincite_syntax::TokenKind as T;
        let mut depth = 0usize;
        for token in self
            .lexed
            .tokens()
            .iter()
            .filter(|token| token.range.start >= start)
        {
            match token.kind {
                T::Semicolon | T::RightBrace if depth == 0 => return token.range.start,
                T::LeftParen | T::LeftBracket | T::LeftBrace | T::MatrixStart => depth += 1,
                T::RightParen | T::RightBracket | T::RightBrace | T::MatrixEnd => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
        }
        self.cursor.source.len()
    }
}

pub(super) fn lower_primitives(
    expression: &mut E<Expression>,
    names: &BTreeMap<String, Kind>,
    queries: &[ProgramQuery],
) {
    // A function call used as a native clause source has a host stage wrapper.
    // In a common value body recover the call node so tail position stays visible.
    if let K::NativeValue(query) | K::NativeQuery(query) = &mut expression.value
        && let Some(value) = take_source_call(query)
    {
        *expression = value;
    }
    if let K::Name(name) = &expression.value
        && matches!(
            names.get(name),
            Some(Kind::Count | Kind::Strings | Kind::Literals)
        )
    {
        expression.value = K::NativeQuery(Box::new(Located::new(
            ExpressionKind::Binding(name.clone()),
            expression.range.clone(),
        )));
    }
    if let K::NativeValue(query) = &expression.value
        && matches!(
            parsed_native_kind(query, names, queries),
            Kind::Count | Kind::Strings | Kind::Literals
        )
    {
        let K::NativeValue(query) = std::mem::replace(
            &mut expression.value,
            K::Literal(zirql::ConstructedValue::Absent),
        ) else {
            unreachable!()
        };
        expression.value = K::NativeQuery(query);
    }
    // All nodes were admitted by the CST lowering; only registered native calls
    // change representation. Named user calls keep the shared call node.
    match &mut expression.value {
        K::Call {
            function,
            arguments,
        } => {
            for argument in arguments.iter_mut() {
                lower_primitives(argument, names, queries);
            }
            if let Some(primitive) = Primitive::named(&function.value) {
                let arguments = std::mem::take(arguments);
                expression.value = K::NativeValue(Box::new(value_stage(
                    Stage::Primitive(primitive, arguments),
                    expression.range.clone(),
                )));
            }
        }
        K::Array(values) | K::Tuple(values) | K::Set(values) => {
            for value in values {
                lower_primitives(value, names, queries);
            }
        }
        K::Record(fields) => {
            for (_, value) in fields {
                lower_primitives(value, names, queries);
            }
        }
        K::Unary { operand, .. }
        | K::Occurs(operand)
        | K::Absent(operand)
        | K::One(operand)
        | K::IndexSet(operand)
        | K::Entries(operand)
        | K::Reduce { array: operand, .. }
        | K::Sort { array: operand, .. }
        | K::Extremum { array: operand, .. }
        | K::GroupBy { rows: operand, .. }
        | K::Field { value: operand, .. }
        | K::TupleAccess { value: operand, .. } => lower_primitives(operand, names, queries),
        K::Binary { left, right, .. }
        | K::Lookup {
            value: left,
            key: right,
        }
        | K::HasKey {
            value: left,
            key: right,
        }
        | K::Default {
            value: left,
            fallback: right,
        }
        | K::ZipExact { left, right }
        | K::Range {
            lower: left,
            upper: right,
        }
        | K::ArrayAccess {
            array: left,
            index: right,
        } => {
            lower_primitives(left, names, queries);
            lower_primitives(right, names, queries);
        }
        K::Require {
            condition, value, ..
        } => {
            lower_primitives(condition, names, queries);
            lower_primitives(value, names, queries);
        }
        K::Conditional {
            branches,
            otherwise,
        } => {
            for (condition, value) in branches {
                lower_primitives(condition, names, queries);
                lower_primitives(value, names, queries);
            }
            lower_primitives(otherwise, names, queries);
        }
        K::Comprehension { clauses, head, .. } => {
            lower_generators(clauses, names, queries);
            lower_primitives(head, names, queries);
        }
        K::KeyedComprehension {
            clauses,
            key,
            head,
            empty_axis_sources,
        } => {
            lower_generators(clauses, names, queries);
            lower_primitives(key, names, queries);
            lower_primitives(head, names, queries);
            for value in empty_axis_sources.iter_mut().flatten() {
                lower_primitives(value, names, queries);
            }
        }
        K::KeyedArray { entries, .. } => {
            for (key, value) in entries {
                lower_primitives(key, names, queries);
                lower_primitives(value, names, queries);
            }
        }
        K::Literal(_) | K::Name(_) | K::NativeQuery(_) | K::NativeValue(_) => {}
    }
}
fn take_source_call(query: &mut Expression) -> Option<E<Expression>> {
    match &mut query.value {
        ExpressionKind::Group(value) => take_source_call(value),
        ExpressionKind::Sequence { input, steps } if steps.is_empty() => take_source_call(input),
        ExpressionKind::Sequence { steps, .. }
            if steps.len() == 1
                && matches!(
                    steps[0].value,
                    ExpressionStep::Stage(Stage::FunctionSource(_))
                ) =>
        {
            let ExpressionStep::Stage(Stage::FunctionSource(value)) = steps.pop().unwrap().value
            else {
                unreachable!()
            };
            Some(value)
        }
        _ => None,
    }
}
fn lower_generators(
    clauses: &mut [Located<zirql::ValueGenerator<Expression>>],
    names: &BTreeMap<String, Kind>,
    queries: &[ProgramQuery],
) {
    for clause in clauses {
        match &mut clause.value {
            zirql::ValueGenerator::In { source, filter, .. } => {
                lower_primitives(source, names, queries);
                if let Some(value) = filter {
                    lower_primitives(value, names, queries);
                }
            }
            zirql::ValueGenerator::Bind { value, filter, .. } => {
                lower_primitives(value, names, queries);
                if let Some(value) = filter {
                    lower_primitives(value, names, queries);
                }
            }
        }
    }
}

pub(super) struct Session<'r, 'a> {
    unit: zirql::PreparedFunctions<'r, 'r, Context<'r, 'a>, composition::CompositionHost<'r, 'r>>,
    entries: BTreeMap<usize, zirql::PreparedFunctionValue<'r, Expression, Kind>>,
    limits: zirql::FunctionLimits,
}
impl<'r, 'a> Session<'r, 'a> {
    pub(super) fn new(
        declarations: &'r [zirql::UnresolvedFunctionDeclaration<Expression>],
        signatures: &'r [zirql::ResolvedFunctionSignature<Kind>],
        host: &'r composition::CompositionHost<'r, 'r>,
        program: &'r zirql::Program<Stage, (), Emitter>,
        limits: zirql::FunctionLimits,
        budget: &mut Budget,
    ) -> Result<Self, QueryError> {
        let unit = zirql::PreparedFunctions::prepare_borrowed(
            declarations
                .iter()
                .zip(signatures)
                .map(|(body, signature)| body.with_signature(signature)),
            host,
            budget,
        )
        .map_err(error)?;
        let mut session = Self {
            unit,
            entries: BTreeMap::new(),
            limits,
        };
        let mut scope = BTreeMap::new();
        for statement in program.statements() {
            let expression = match &statement.value {
                zirql::Statement::Bind { expression, .. }
                | zirql::Statement::Evaluate { expression, .. } => expression,
            };
            let kind = session.prepare_expression(expression, Kind::Nodes, &scope, host, budget)?;
            if let zirql::Statement::Bind { name, .. } = &statement.value {
                scope.insert(name.value.clone(), kind);
            }
        }
        Ok(session)
    }
    fn prepare_value(
        &mut self,
        value: &'r E<Expression>,
        kind: Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
    ) -> Result<Kind, QueryError> {
        let entry = self
            .unit
            .prepare_value(value, scope, Some(kind), budget)
            .map_err(error)?;
        let kind = entry.kind().clone();
        budget
            .charge_results(1, zirql::Location::Query(value.range.clone()))
            .map_err(pipeline::core_error)?;
        self.entries.insert(value as *const _ as usize, entry);
        Ok(kind)
    }
    fn prepare_expression(
        &mut self,
        expression: &'r Expression,
        input: Kind,
        scope: &BTreeMap<String, Kind>,
        host: &composition::CompositionHost<'_, '_>,
        budget: &mut Budget,
    ) -> Result<Kind, QueryError> {
        match &expression.value {
            ExpressionKind::Input => Ok(input),
            ExpressionKind::Binding(name) => scope.get(name).cloned().ok_or_else(|| {
                QueryError::query(
                    expression.range.clone(),
                    format!("undefined binding: {name}"),
                )
            }),
            ExpressionKind::Group(value) => {
                self.prepare_expression(value, input, scope, host, budget)
            }
            ExpressionKind::Sequence {
                input: source,
                steps,
            } => {
                let mut kind = self.prepare_expression(source, input, scope, host, budget)?;
                for step in steps {
                    let ExpressionStep::Stage(stage) = &step.value else {
                        continue;
                    };
                    kind = match stage {
                        Stage::FunctionValue(value) | Stage::FunctionSource(value) => {
                            self.prepare_value(value, kind, scope, budget)?
                        }
                        _ => {
                            for (value, relative) in host
                                .value_arguments(stage, budget, &step.range)
                                .map_err(pipeline::core_error)?
                            {
                                self.prepare_value(value, relative, scope, budget)?;
                            }
                            host.stage_kind(stage, &kind, scope, budget, &step.range)
                                .map_err(pipeline::core_error)?
                        }
                    };
                }
                Ok(kind)
            }
            _ => zirql::native_expression_kind(expression, host, &input, scope, budget)
                .map_err(pipeline::core_error),
        }
    }
    pub(super) fn run(
        &self,
        value: &E<Expression>,
        bindings: &Bindings<ReportValue<'a>>,
        input: &ReportValue<'a>,
        context: &mut Context<'r, 'a>,
        budget: &mut Budget,
    ) -> Result<ReportValue<'a>, QueryError> {
        let entry = self
            .entries
            .get(&(value as *const _ as usize))
            .ok_or_else(|| {
                QueryError::query(
                    value.range.clone(),
                    "written function value was not prepared in this scope",
                )
            })?;
        self.unit
            .evaluate(entry, bindings, Some(input), context, budget, self.limits)
            .map_err(error)
    }
}
pub(super) fn error(error: zirql::FunctionError<zirql::EvaluationError<QueryError>>) -> QueryError {
    let mut result = pipeline::evaluation_error(error.error);
    for call in error.calls {
        result.message.push_str(&format!(
            "; in {} at {}..{}",
            call.function, call.range.start, call.range.end
        ));
    }
    if error.omitted_calls > 0 {
        result.message.push_str("; further function calls omitted");
    }
    result
}

pub(super) fn resolve_signatures<'a>(
    declarations: &[zirql::UnresolvedFunctionDeclaration<Expression>],
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
) -> Result<Vec<zirql::ResolvedFunctionSignature<Kind>>, QueryError> {
    declarations
        .iter()
        .map(|declaration| {
            declaration
                .resolve_signature(budget, |name, budget| match name.value {
                    "Node" => Ok(D::Native(Kind::Node)),
                    "Declaration" => Ok(D::Native(Kind::Declaration)),
                    _ => {
                        let facts = context
                            .report_facts(budget, &name.range)
                            .map_err(composition::query_error)?
                            .ok_or_else(|| {
                                zirql::QueryError::query(
                                    name.range.clone(),
                                    "nominal signature needs model facts",
                                )
                            })?;
                        let declaration = comprehension::unique_declaration(
                            &facts,
                            name.value,
                            zincite_lint::DeclarationRole::Enum,
                            budget,
                            &name.range,
                        )
                        .map_err(composition::query_error)?;
                        comprehension::enum_cases(&facts, declaration, budget, &name.range)
                            .map_err(composition::query_error)?;
                        let id = comprehension::nominal_id(declaration.id, &name.range)
                            .map_err(composition::query_error)?;
                        Ok(D::Member(zirql::NominalTypeId(id)))
                    }
                })
                .map_err(pipeline::core_error)
        })
        .collect()
}

pub(super) fn unretained<'v, 'a>(mut value: &'v ReportValue<'a>) -> &'v ReportValue<'a> {
    loop {
        match value {
            ReportValue::Retained(owner) => value = owner,
            ReportValue::Item(item) | ReportValue::EntitySequence { value: item, .. } => {
                value = item
            }
            _ => return value,
        }
    }
}
pub(super) fn is_item(value: &ReportValue<'_>) -> bool {
    match value {
        ReportValue::Retained(owner) => is_item(owner),
        ReportValue::EntitySequence { value, .. } => is_item(value),
        ReportValue::Item(_) => true,
        _ => false,
    }
}
pub(super) fn owned<'a>(
    mut value: ReportValue<'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, QueryError> {
    loop {
        value = match value {
            ReportValue::Retained(owner) => match Arc::try_unwrap(owner) {
                Ok(value) => value,
                Err(owner) => copy_value(unretained(&owner), budget, range)?,
            },
            ReportValue::Item(item) | ReportValue::EntitySequence { value: item, .. } => *item,
            _ => return Ok(value),
        }
    }
}
impl<'a> zirql::FunctionHost<Context<'_, 'a>> for composition::CompositionHost<'_, '_> {
    fn retains_native_admission(&self) -> bool {
        true
    }
    fn declaration_kind(
        &self,
        declaration: &D<Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        budget.charge(1, zirql::Location::Query(range.clone()))?;
        Ok(match declaration {
            D::Boolean => Kind::Scalar(zirql::ParameterType::Bool),
            D::Integer => Kind::Scalar(zirql::ParameterType::Int),
            D::String => Kind::Scalar(zirql::ParameterType::String),
            D::Float => Kind::Value(Rc::new(vec![zirql::ValueType::Float])),
            D::Native(kind) => kind.clone(),
            D::Optional(inner) => self.declaration_kind(inner, budget, range)?,
            D::Sequence(inner) | D::Array(inner)
                if matches!(inner.as_ref(), D::Native(Kind::Node)) =>
            {
                Kind::Nodes
            }
            D::Sequence(inner) | D::Array(inner)
                if matches!(inner.as_ref(), D::Native(Kind::Declaration)) =>
            {
                Kind::Declarations
            }
            D::Record(fields) if fields.iter().any(|(_, field)| native_declaration(field)) => {
                Kind::Capture(Rc::new(zirql::NativeCapture {
                    fields: fields
                        .iter()
                        .map(|(name, declaration)| {
                            Ok((
                                name.clone(),
                                self.declaration_kind(declaration, budget, range)?,
                            ))
                        })
                        .collect::<Result<_, zirql::QueryError>>()?,
                }))
            }
            D::Sequence(inner) | D::Array(inner)
                if matches!(inner.as_ref(), D::Record(_)) && native_declaration(inner) =>
            {
                let Kind::Capture(schema) = self.declaration_kind(inner, budget, range)? else {
                    unreachable!()
                };
                Kind::Captures(schema)
            }
            _ => Kind::Constructed,
        })
    }
    fn is_operation_name(&self, name: &str) -> bool {
        composition::domain_stage(name)
            || Primitive::named(name).is_some()
            || matches!(
                name,
                "source" | "literal" | "create" | "replace" | "fn" | "enum_member"
            )
    }
}
impl<'a> zirql::RuntimeTypeHost<Context<'_, 'a>> for composition::CompositionHost<'_, '_> {
    type Kind = Kind;
    fn constructed_value<'v>(
        &self,
        value: &'v ReportValue<'a>,
    ) -> Option<&'v zirql::ConstructedValue> {
        match unretained(value) {
            ReportValue::Constructed(value) => Some(value),
            ReportValue::SharedConstructed(value) => Some(value),
            _ => None,
        }
    }
    fn admit_native(
        &self,
        value: &ReportValue<'a>,
        expected: &D<Kind>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        location: zirql::Location,
    ) -> Result<(), zirql::EvaluationError<QueryError>> {
        let range = match &location {
            zirql::Location::Query(range) | zirql::Location::Source(range) => range,
        };
        budget.charge(1, location.clone())?;
        match (expected, unretained(value)) {
            (D::Record(fields), ReportValue::Capture(capture)) => {
                return admit_capture(capture, fields, self, context, budget, location);
            }
            (D::Sequence(inner) | D::Array(inner), ReportValue::Captures(captures)) => {
                let D::Record(fields) = inner.as_ref() else {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "native capture sequence needs a record contract",
                    )
                    .into());
                };
                if captures.rows.is_empty() {
                    use zirql::FunctionHost;
                    let Kind::Capture(expected) = self.declaration_kind(inner, budget, range)?
                    else {
                        unreachable!()
                    };
                    if *captures.schema != *expected {
                        return Err(zirql::QueryError::query(
                            range.clone(),
                            "empty capture sequence has another element contract",
                        )
                        .into());
                    }
                }
                for capture in &captures.rows {
                    admit_capture(capture, fields, self, context, budget, location.clone())?;
                }
                return Ok(());
            }
            _ => {}
        }
        let (element, singleton) = match expected {
            D::Optional(inner) => {
                return zirql::admit_value(value, inner, self, context, budget, location);
            }
            D::Sequence(inner) | D::Array(inner) => (inner.as_ref(), false),
            other => (other, true),
        };
        if singleton != is_item(value) {
            return Err(zirql::QueryError::query(
                range.clone(),
                "native singleton/sequence contract mismatch",
            )
            .into());
        }
        match (element, unretained(value)) {
            (D::Native(Kind::Node), ReportValue::Query(SemanticStream::Inspection(result))) => {
                match result.as_ref() {
                    QueryResult::Selection(selection) => {
                        budget.charge(selection.items.len(), location.clone())?;
                        check_node_owner(
                            selection.input,
                            context.input,
                            selection.items.len(),
                            singleton,
                            range,
                        )?;
                        for item in &selection.items {
                            check_node_coordinates(
                                selection.input,
                                item.node,
                                selection.input.file(),
                                &item.range,
                                range,
                            )?;
                        }
                    }
                    QueryResult::Nodes(selection) => {
                        budget.charge(selection.nodes.len(), location.clone())?;
                        check_node_owner(
                            selection.input,
                            context.input,
                            selection.nodes.len(),
                            singleton,
                            range,
                        )?;
                        for node in &selection.nodes {
                            check_node_coordinates(
                                selection.input,
                                node.node,
                                node.file,
                                &node.range,
                                range,
                            )?;
                        }
                    }
                    _ => {
                        return Err(zirql::QueryError::query(
                            range.clone(),
                            "expected native syntax nodes",
                        )
                        .into());
                    }
                }
            }
            (D::Native(Kind::Declaration), ReportValue::Query(SemanticStream::Entities(rows))) => {
                if entity_contract(value) == Some(false)
                    || (rows.is_empty() && entity_contract(value) != Some(true))
                {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "native entity sequence has another element contract",
                    )
                    .into());
                }
                let facts = context
                    .report_facts(budget, range)
                    .map_err(zirql::EvaluationError::Host)?
                    .ok_or_else(|| {
                        zirql::QueryError::query(
                            range.clone(),
                            "declaration admission needs model facts",
                        )
                    })?;
                if singleton && rows.len() != 1 {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "expected one declaration",
                    )
                    .into());
                }
                for row in rows {
                    budget.charge(1, location.clone())?;
                    let semantic::SemanticEntity::Declaration(id) = row else {
                        return Err(zirql::QueryError::query(
                            range.clone(),
                            "expected declaration elements",
                        )
                        .into());
                    };
                    let mut owned = false;
                    for declaration in &facts.bindings().declarations {
                        budget.charge(1, location.clone())?;
                        if declaration.id == *id {
                            owned = true;
                            break;
                        }
                    }
                    if !owned {
                        return Err(zirql::QueryError::query(
                            range.clone(),
                            "unavailable declaration owner",
                        )
                        .into());
                    }
                }
            }
            _ => {
                return Err(zirql::QueryError::query(
                    range.clone(),
                    "native value does not satisfy declared type",
                )
                .into());
            }
        }
        Ok(())
    }
}

fn admit_capture<'a>(
    capture: &zirql::NativeCapture<ReportValue<'a>>,
    fields: &[(String, D<Kind>)],
    host: &composition::CompositionHost<'_, '_>,
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    location: zirql::Location,
) -> Result<(), zirql::EvaluationError<QueryError>> {
    let range = match &location {
        zirql::Location::Query(range) | zirql::Location::Source(range) => range,
    };
    for (index, (name, value)) in capture.fields.iter().enumerate() {
        budget.charge(name.len() + 1, location.clone())?;
        if capture.fields[..index]
            .iter()
            .any(|(previous, _)| previous == name)
        {
            return Err(zirql::QueryError::query(range.clone(), "duplicate capture field").into());
        }
        let Some((_, declaration)) = fields.iter().find(|(expected, _)| expected == name) else {
            return Err(zirql::QueryError::query(range.clone(), "unexpected capture field").into());
        };
        zirql::admit_value(value, declaration, host, context, budget, location.clone())?;
    }
    for (name, declaration) in fields {
        if !matches!(declaration, D::Optional(_))
            && !capture.fields.iter().any(|(actual, _)| actual == name)
        {
            return Err(zirql::QueryError::query(range.clone(), "missing capture field").into());
        }
    }
    Ok(())
}

pub(super) fn primitive<'a>(
    primitive: Primitive,
    value: &ReportValue<'a>,
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
    host: &composition::CompositionHost<'_, '_>,
) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
    zirql::admit_value(
        value,
        &D::Native(Kind::Node),
        host,
        context,
        budget,
        zirql::Location::Query(range.clone()),
    )?;
    let ReportValue::Query(SemanticStream::Inspection(result)) = unretained(value) else {
        unreachable!("admitted node")
    };
    let node = match result.as_ref() {
        QueryResult::Nodes(selection) => selection.nodes[0].node,
        QueryResult::Selection(selection) => selection.items[0].node,
        _ => unreachable!("admitted node"),
    };
    let input = context.input;
    let value = match primitive {
        Primitive::DirectChildren => {
            let count = node.child_nodes().count();
            budget.charge(count, zirql::Location::Query(range.clone()))?;
            budget.charge_results(count + 1, zirql::Location::Query(range.clone()))?;
            budget.check_items(count, zirql::Location::Query(range.clone()))?;
            ReportValue::Query(SemanticStream::Inspection(Box::new(QueryResult::Nodes(
                crate::NodeSelection {
                    input,
                    nodes: node
                        .child_nodes()
                        .map(|node| crate::SelectedNode {
                            node,
                            file: input.file(),
                            range: input.original_range(node.range()),
                        })
                        .collect(),
                },
            ))))
        }
        Primitive::NodeKind => ReportValue::Constructed(zirql::ConstructedValue::String(
            inspection::kind_name(node.kind()),
        )),
        Primitive::WrittenCallName => {
            ReportValue::Constructed(match inspection::call_name(input, node) {
                Some(name) => {
                    budget.charge(name.len(), zirql::Location::Query(range.clone()))?;
                    zirql::ConstructedValue::String(name.to_owned())
                }
                None => zirql::ConstructedValue::Absent,
            })
        }
    };
    Ok(value)
}

pub(super) fn wrap_program_entries(
    statements: Vec<Located<zirql::Statement<Stage, (), Emitter>>>,
) -> Vec<Located<zirql::Statement<Stage, (), Emitter>>> {
    statements
        .into_iter()
        .map(|statement| {
            let (mut expression, binding, implicit_output) = match statement.value {
                zirql::Statement::Bind { name, expression } => (expression, Some(name), None),
                zirql::Statement::Evaluate {
                    expression,
                    implicit_output,
                } => (expression, None, implicit_output),
            };
            let terminal = if let ExpressionKind::Sequence { steps, .. } = &mut expression.value {
                if steps
                    .last()
                    .is_some_and(|step| matches!(step.value, ExpressionStep::Emit { .. }))
                {
                    steps.pop()
                } else {
                    None
                }
            } else {
                None
            };
            let range = expression.range.clone();
            expression = value_stage(
                Stage::FunctionValue(Located::new(
                    K::NativeValue(Box::new(expression)),
                    range.clone(),
                )),
                range.clone(),
            );
            if let Some(terminal) = terminal {
                expression = Located::new(
                    ExpressionKind::Sequence {
                        input: Box::new(expression),
                        steps: vec![terminal],
                    },
                    range,
                );
            }
            Located::new(
                match binding {
                    Some(name) => zirql::Statement::Bind { name, expression },
                    None => zirql::Statement::Evaluate {
                        expression,
                        implicit_output,
                    },
                },
                statement.range,
            )
        })
        .collect()
}

pub(super) fn value_kind(
    expression: &E<Expression>,
    host: &composition::CompositionHost<'_, '_>,
    input: &Kind,
    scope: &BTreeMap<String, Kind>,
    budget: &mut Budget,
) -> Result<Kind, zirql::QueryError> {
    use zirql::FunctionHost;
    budget.charge(1, zirql::Location::Query(expression.range.clone()))?;
    match &expression.value {
        K::Name(name) => scope.get(name).cloned().ok_or_else(|| {
            zirql::QueryError::query(
                expression.range.clone(),
                format!("undefined binding: {name}"),
            )
        }),
        K::Call {
            function,
            arguments,
        } => {
            let signature = host
                .0
                .signatures
                .iter()
                .find(|signature| signature.name.value == function.value)
                .ok_or_else(|| {
                    zirql::QueryError::query(
                        function.range.clone(),
                        format!("undefined function: {}", function.value),
                    )
                })?;
            if signature.parameters.len() != arguments.len() {
                return Err(zirql::QueryError::query(
                    expression.range.clone(),
                    "function argument count mismatch",
                ));
            }
            host.declaration_kind(&signature.result.value, budget, &expression.range)
        }
        K::Literal(value) => Ok(Kind::Value(Rc::new(vec![comprehension::value_facts(
            value,
            budget,
            &expression.range,
        )?]))),
        K::NativeValue(query) => zirql::native_expression_kind(query, host, input, scope, budget),
        _ => Ok(Kind::Constructed),
    }
}

fn check_node_owner(
    owner: &Input,
    current: &Input,
    len: usize,
    singleton: bool,
    range: &Range<usize>,
) -> Result<(), zirql::QueryError> {
    if !std::ptr::eq(owner, current) || singleton && len != 1 {
        return Err(zirql::QueryError::query(
            range.clone(),
            "native node belongs to another input",
        ));
    }
    Ok(())
}
fn check_node_coordinates(
    owner: &Input,
    node: &zincite_syntax::SyntaxNode,
    file: &str,
    coordinates: &Range<usize>,
    range: &Range<usize>,
) -> Result<(), zirql::QueryError> {
    if file != owner.file() || *coordinates != owner.original_range(node.range()) {
        return Err(zirql::QueryError::query(
            range.clone(),
            "native node coordinates do not match its input",
        ));
    }
    Ok(())
}

pub(super) fn parsed_native_kind(
    query: &Expression,
    names: &BTreeMap<String, Kind>,
    queries: &[ProgramQuery],
) -> Kind {
    match &query.value {
        ExpressionKind::Input => Kind::Nodes,
        ExpressionKind::Binding(name) => names.get(name).cloned().unwrap_or(Kind::Constructed),
        ExpressionKind::Group(query) => parsed_native_kind(query, names, queries),
        ExpressionKind::Sequence { input, steps } => {
            let mut kind = parsed_native_kind(input, names, queries);
            for step in steps {
                if let ExpressionStep::Stage(stage) = &step.value {
                    kind = match stage {
                        Stage::Query(index, _) => validate_stages(&queries[*index].query, kind)
                            .unwrap_or(Kind::Constructed),
                        _ => Kind::Constructed,
                    };
                }
            }
            kind
        }
        _ => Kind::Constructed,
    }
}

fn signature_needs_model(signature: &zirql::SignatureType) -> bool {
    use zirql::SignatureType as S;
    match signature {
        S::Named(name) => !matches!(name.value.as_str(), "Node" | "Declaration"),
        S::Optional(inner) | S::Sequence(inner) | S::Set(inner) | S::Keyed(inner) => {
            signature_needs_model(&inner.value)
        }
        S::Tuple(fields) => fields
            .iter()
            .any(|field| signature_needs_model(&field.value)),
        S::Record(fields) => fields
            .iter()
            .any(|(_, field)| signature_needs_model(&field.value)),
        _ => false,
    }
}
pub(super) fn check_scalar(
    expression: &E<Expression>,
    expected: zirql::ParameterType,
    host: &composition::CompositionHost<'_, '_>,
    input: &Kind,
    scope: &BTreeMap<String, Kind>,
    budget: &mut Budget,
) -> Result<(), zirql::QueryError> {
    let kind = value_kind(expression, host, input, scope, budget)?;
    if let Some(types) = host.scalar_types(&kind, budget, &expression.range)?
        && types
            .iter()
            .any(|ty| !matches!(ty, zirql::ValueType::Unestablished) && *ty != expected.into())
    {
        return Err(zirql::QueryError::query(
            expression.range.clone(),
            "primitive argument has an incompatible scalar type",
        ));
    }
    Ok(())
}

fn native_declaration(declaration: &D<Kind>) -> bool {
    match declaration {
        D::Native(_) => true,
        D::Optional(inner) | D::Sequence(inner) | D::Array(inner) | D::Keyed(inner) => {
            native_declaration(inner)
        }
        D::Record(fields) => fields.iter().any(|(_, field)| native_declaration(field)),
        D::Tuple(fields) => fields.iter().any(native_declaration),
        _ => false,
    }
}

pub(super) fn entity_contract(value: &ReportValue<'_>) -> Option<bool> {
    match value {
        ReportValue::Retained(value) => entity_contract(value),
        ReportValue::Item(value) => entity_contract(value),
        ReportValue::EntitySequence { declarations, .. } => Some(*declarations),
        ReportValue::Query(SemanticStream::Entities(rows)) if !rows.is_empty() => Some(
            rows.iter()
                .all(|row| matches!(row, semantic::SemanticEntity::Declaration(_))),
        ),
        _ => None,
    }
}
pub(super) fn query_declarations(query: &Query, input: &ReportValue<'_>) -> bool {
    use language::StageKind as S;
    let mut declarations = entity_contract(input) == Some(true);
    for stage in &query.stages {
        match &stage.kind {
            S::Declarations | S::ReferencedDeclarations => declarations = true,
            S::Head(_) | S::Filter(_) | S::Unique | S::Json | S::Emit => {}
            _ => declarations = false,
        }
    }
    declarations
}
