//! Request-local function preparation and native type admission.
use super::*;
use zirql::{DeclaredType as D, ProjectionHost, ValueExpression as E, ValueExpressionKind as K};

#[derive(Clone, Copy, Debug)]
pub(super) enum Primitive {
    DirectChildren,
    NodeKind,
    WrittenCallName,
    SameDeclaration,
    ReferenceInfo,
}
impl Primitive {
    pub(super) fn named(name: &str) -> Option<Self> {
        match name {
            "same_declaration" => Some(Self::SameDeclaration),
            "reference_info" => Some(Self::ReferenceInfo),
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
                let result = zirql::read_unresolved_pattern_function_declaration(
                    &mut reader,
                    self,
                    &mut budget,
                    self.limits.nesting,
                );
                self.preflight = budget;
                if let Some(function) = result.map_err(pipeline::core_error)? {
                    let admission = (|| {
                        check_signature(
                            &function.result,
                            &mut self.preflight,
                            self.limits.nesting,
                        )?;
                        for parameter in &function.parameters {
                            check_signature(
                                &parameter.value.declaration,
                                &mut self.preflight,
                                self.limits.nesting,
                            )?;
                        }
                        check_value(&function.value, &mut self.preflight, self.limits.nesting)?;
                        Ok::<(), QueryError>(())
                    })();
                    if let Err(error) = admission {
                        zirql::dispose_unresolved_pattern_functions(vec![function]);
                        return Err(error);
                    }

                    self.report_model_demand |= signature_needs_model(&function.result.value)
                        || function.parameters.iter().any(|parameter| {
                            signature_needs_model(&parameter.value.declaration.value)
                        });
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
        self.constructed_functions = self
            .functions
            .iter()
            .filter(|function| constructed_signature(&function.result.value))
            .map(|function| function.name.value.clone())
            .collect();
        for function in &mut self.functions {
            lower_primitives(
                &mut function.value,
                &BTreeMap::new(),
                &self.queries,
                &self.constructed_functions,
            );
        }
        self.cursor.position = 0;
        Ok(())
    }
    pub(super) fn reader_locals(&self) -> Vec<String> {
        self.function_scope
            .as_ref()
            .map(|scope| scope.keys().cloned().collect())
            .unwrap_or_default()
    }
    pub(super) fn read_value(
        &mut self,
        range: Range<usize>,
        nesting: usize,
    ) -> Result<E<Expression>, QueryError> {
        self.begin_pure(range.clone())?;
        let locals = self.reader_locals();
        let mut reader = zirql::ArgumentReader::new(&self.cursor.source[..range.end], range.start)
            .map_err(pipeline::core_error)?;
        let mut budget =
            std::mem::replace(&mut self.preflight, Budget::for_program(self.limits.core()));
        let parsed = zirql::read_value_expression_in_scope(
            &mut reader,
            self,
            &locals.iter().map(String::as_str).collect::<Vec<_>>(),
            &mut budget,
            nesting,
        );
        self.preflight = budget;
        let mut value = parsed.map_err(pipeline::core_error)?;
        reader.skip_whitespace();
        if reader.position() != range.end {
            zirql::dispose_native_value(value);
            return Err(QueryError::query(
                reader.position()..range.end,
                "expected one value expression",
            ));
        }
        written_value_natives(&value, &mut self.preflight, nesting)?;
        lower_primitives(
            &mut value,
            &self.names,
            &self.queries,
            &self.constructed_functions,
        );
        Ok(value)
    }
}

pub(super) fn lower_primitives(
    expression: &mut E<Expression>,
    names: &BTreeMap<String, Kind>,
    queries: &[ProgramQuery],
    constructed_functions: &std::collections::BTreeSet<String>,
) {
    recover_call_value(expression, queries, constructed_functions);
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
    // Admission bounded this tree before lowering. Hosts rewrite native leaves;
    // Zirql owns child and generator structure and ordinary record evaluation.
    zirql::for_each_value_child_mut(expression, |child| {
        lower_primitives(child, names, queries, constructed_functions);
    });
    if let K::Call {
        function,
        arguments,
    } = &mut expression.value
        && let Some(primitive) = Primitive::named(&function.value)
    {
        let arguments = std::mem::take(arguments);
        expression.value = K::NativeValue(Box::new(value_stage(
            Stage::Primitive(primitive, arguments),
            expression.range.clone(),
        )));
    }
}

pub(super) fn recover_call_value(
    expression: &mut E<Expression>,
    queries: &[ProgramQuery],
    functions: &std::collections::BTreeSet<String>,
) {
    if let K::NativeValue(query) | K::NativeQuery(query) = &mut expression.value
        && let Some(value) =
            take_constructed_suffix(query, queries, functions).or_else(|| take_source_call(query))
    {
        *expression = value;
    }
}
fn constructed_signature(signature: &zirql::SignatureType) -> bool {
    use zirql::SignatureType as S;
    match signature {
        S::Value | S::Boolean | S::Integer | S::Float | S::String => true,
        S::Dictionary(key, value) => {
            constructed_signature(&key.value) && constructed_signature(&value.value)
        }
        S::Optional(value) | S::Sequence(value) | S::Set(value) | S::Keyed(value) => {
            constructed_signature(&value.value)
        }
        S::Named(name) => !matches!(name.value.as_str(), "Node" | "Declaration"),
        S::Tuple(values) => values
            .iter()
            .all(|value| constructed_signature(&value.value)),
        S::Record(fields) => fields
            .iter()
            .all(|(_, value)| constructed_signature(&value.value)),
        _ => false,
    }
}
fn common_suffix(
    stage: &ExpressionStep<Stage, Emitter>,
    queries: &[ProgramQuery],
) -> Option<zirql::ValueSuffix<Expression>> {
    if matches!(stage, ExpressionStep::Stage(Stage::Ordering(order)) if order.order == zirql::NativeOrder::Sort && order.selector.is_none())
    {
        return Some(zirql::ValueSuffix::Sort);
    }
    let ExpressionStep::Stage(Stage::Query(index, _)) = stage else {
        return None;
    };
    match queries[*index].query.stages.as_slice() {
        [
            language::Stage {
                kind: language::StageKind::Count,
                ..
            },
        ] => Some(zirql::ValueSuffix::Count),
        [
            language::Stage {
                kind: language::StageKind::Unique,
                ..
            },
        ] => Some(zirql::ValueSuffix::Unique),
        [
            language::Stage {
                kind: language::StageKind::Tally,
                ..
            },
        ] => Some(zirql::ValueSuffix::Tally),
        _ => None,
    }
}
fn constructed_call(
    query: &Expression,
    queries: &[ProgramQuery],
    functions: &std::collections::BTreeSet<String>,
) -> bool {
    fn inspect(
        query: &Expression,
        queries: &[ProgramQuery],
        functions: &std::collections::BTreeSet<String>,
    ) -> Option<bool> {
        match &query.value {
            ExpressionKind::Group(value) => inspect(value, queries, functions),
            ExpressionKind::Sequence { input, steps } => {
                let (mut terminal, steps) = if let Some(Located {
                    value: ExpressionStep::Stage(Stage::FunctionSource(value)),
                    ..
                }) = steps.first()
                {
                    if !matches!(&value.value, K::Call { function, .. } if functions.contains(&function.value))
                    {
                        return None;
                    }
                    (false, &steps[1..])
                } else {
                    (inspect(input, queries, functions)?, steps.as_slice())
                };
                for step in steps {
                    if terminal {
                        return None;
                    }
                    terminal = matches!(
                        common_suffix(&step.value, queries)?,
                        zirql::ValueSuffix::Tally
                    );
                }
                Some(terminal)
            }
            _ => None,
        }
    }
    inspect(query, queries, functions).is_some()
}
fn take_constructed_suffix(
    query: &mut Expression,
    queries: &[ProgramQuery],
    functions: &std::collections::BTreeSet<String>,
) -> Option<E<Expression>> {
    if !constructed_call(query, queries, functions) {
        return None;
    }
    fn take(query: &mut Expression, queries: &[ProgramQuery]) -> Option<E<Expression>> {
        match &mut query.value {
            ExpressionKind::Group(value) => take(value, queries),
            ExpressionKind::Sequence { input, steps } => {
                let mut value = if matches!(
                    steps.first().map(|step| &step.value),
                    Some(ExpressionStep::Stage(Stage::FunctionSource(_)))
                ) {
                    let mut step = steps.remove(0);
                    let ExpressionStep::Stage(Stage::FunctionSource(value)) = &mut step.value
                    else {
                        unreachable!()
                    };
                    std::mem::replace(
                        value,
                        Located::new(K::Literal(zirql::ConstructedValue::Absent), 0..0),
                    )
                } else {
                    take(input, queries)?
                };
                for step in std::mem::take(steps) {
                    let suffix =
                        common_suffix(&step.value, queries).expect("suffixes checked above");
                    value = zirql::lower_value_suffix(value, Located::new(suffix, step.range))
                        .expect("the checked suffix chain ends at tally");
                }
                Some(value)
            }
            _ => None,
        }
    }
    take(query, queries)
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
            let mut step = steps.pop().unwrap();
            let ExpressionStep::Stage(Stage::FunctionSource(value)) = &mut step.value else {
                unreachable!()
            };
            Some(std::mem::replace(
                value,
                Located::new(K::Literal(zirql::ConstructedValue::Absent), 0..0),
            ))
        }
        _ => None,
    }
}

pub(super) struct Session<'r, 'a> {
    unit: zirql::PreparedFunctions<'r, 'r, Context<'r, 'a>, composition::CompositionHost<'r, 'r>>,
    entries: BTreeMap<usize, zirql::PreparedFunctionValue<'r, Expression, Kind>>,
    limits: zirql::FunctionLimits,
    bindings: BTreeMap<usize, zirql::PreparedPatternBinding<'r, Expression, Kind>>,
}
impl<'r, 'a> Session<'r, 'a> {
    pub(super) fn new(
        declarations: &'r [zirql::UnresolvedPatternFunctionDeclaration<Expression>],
        signatures: &'r [zirql::ResolvedPatternFunctionSignature<'r, Kind>],
        host: &'r composition::CompositionHost<'r, 'r>,
        program: &'r zirql::Program<Stage, zirql::NativeSetOperator, Emitter>,
        limits: zirql::FunctionLimits,
        input: Kind,
        budget: &mut Budget,
    ) -> Result<Self, QueryError> {
        let unit = zirql::PreparedFunctions::prepare_patterns_borrowed(
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
            bindings: BTreeMap::new(),
        };
        let mut scope = BTreeMap::new();
        for statement in program.statements() {
            let expression = match &statement.value {
                zirql::Statement::Bind { expression, .. }
                | zirql::Statement::BindFields { expression, .. }
                | zirql::Statement::Evaluate { expression, .. } => expression,
            };
            let kind =
                session.prepare_expression(expression, input.clone(), &scope, host, budget)?;
            if let zirql::Statement::BindFields { names, .. } = &statement.value {
                let entry = session
                    .bindings
                    .values()
                    .find(|entry| entry.bindings().map(|(name, _)| name).eq(names.iter()))
                    .expect("prepared pattern stage");
                for (name, kind) in entry.bindings() {
                    scope.insert(name.value.clone(), kind.clone());
                }
            }
            if let zirql::Statement::Bind { name, .. } = &statement.value {
                let kind = if let Kind::Payload(kind) = kind {
                    (*kind).clone()
                } else {
                    kind
                };
                scope.insert(name.value.clone(), kind);
            }
        }
        Ok(session)
    }
    fn prepare_value(
        &mut self,
        value: &'r E<Expression>,
        kind: Option<Kind>,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
    ) -> Result<Kind, QueryError> {
        let entry = self
            .unit
            .prepare_value(value, scope, kind, budget)
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
                        Stage::Data(..) => kind,
                        Stage::Query(index, _) if host.0.queries[*index].query.is_editing() => kind,
                        Stage::PatternBinding(pattern, value) => {
                            let entry = self
                                .unit
                                .prepare_binding(pattern, value, scope, Some(kind), budget)
                                .map_err(error)?;
                            let fields = entry
                                .bindings()
                                .map(|(name, kind)| (name.value.clone(), kind.clone()))
                                .collect();
                            self.bindings.insert(value as *const _ as usize, entry);
                            Kind::Capture(Rc::new(zirql::NativeCapture { fields }))
                        }
                        Stage::FunctionValue(value) | Stage::FunctionSource(value) => {
                            self.prepare_value(value, Some(kind), scope, budget)?
                        }
                        _ => {
                            for argument in host
                                .scoped_value_arguments(stage, &kind, scope, budget, &step.range)
                                .map_err(pipeline::core_error)?
                            {
                                if argument.locals.is_empty() {
                                    self.prepare_value(
                                        argument.expression,
                                        argument.relative,
                                        scope,
                                        budget,
                                    )?;
                                } else {
                                    budget
                                        .charge(
                                            scope.keys().map(|name| name.len() + 1).sum::<usize>()
                                                + argument
                                                    .locals
                                                    .iter()
                                                    .map(|(name, _)| name.value.len() + 1)
                                                    .sum::<usize>(),
                                            zirql::Location::Query(step.range.clone()),
                                        )
                                        .map_err(pipeline::core_error)?;
                                    let mut scope = scope.clone();
                                    for (name, kind) in argument.locals {
                                        scope.insert(name.value, kind);
                                    }
                                    self.prepare_value(
                                        argument.expression,
                                        argument.relative,
                                        &scope,
                                        budget,
                                    )?;
                                }
                            }
                            host.stage_kind(stage, &kind, scope, budget, &step.range)
                                .map_err(pipeline::core_error)?
                        }
                    };
                }
                Ok(kind)
            }
            ExpressionKind::Set { left, right, .. } => {
                let left = self.prepare_expression(left, input.clone(), scope, host, budget)?;
                let right = self.prepare_expression(right, input, scope, host, budget)?;
                host.set_kind(&left, &right, budget, &expression.range)
                    .map_err(pipeline::core_error)
            }
            ExpressionKind::Record(fields) => {
                for (_, value) in fields {
                    self.prepare_expression(value, input.clone(), scope, host, budget)?;
                }
                Ok(Kind::Record)
            }
            _ => zirql::native_expression_kind(expression, host, &input, scope, budget)
                .map_err(pipeline::core_error),
        }
    }
    pub(super) fn bind(
        &self,
        value: &E<Expression>,
        bindings: &Bindings<ReportValue<'a>>,
        input: &ReportValue<'a>,
        context: &mut Context<'r, 'a>,
        budget: &mut Budget,
    ) -> Result<ReportValue<'a>, QueryError> {
        let entry = self
            .bindings
            .get(&(value as *const _ as usize))
            .expect("prepared pattern binding");
        self.unit
            .evaluate_binding(entry, bindings, Some(input), context, budget, self.limits)
            .map(ReportValue::Capture)
            .map_err(error)
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

pub(super) fn resolve_signatures<'r, 'a>(
    declarations: &'r [zirql::UnresolvedPatternFunctionDeclaration<Expression>],
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
) -> Result<Vec<zirql::ResolvedPatternFunctionSignature<'r, Kind>>, QueryError> {
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
            ReportValue::Item(item)
            | ReportValue::EntitySequence { value: item, .. }
            | ReportValue::IncompleteSelection { value: item, .. } => value = item,
            _ => return value,
        }
    }
}
pub(super) fn is_item(value: &ReportValue<'_>) -> bool {
    match value {
        ReportValue::Retained(owner) => is_item(owner),
        ReportValue::EntitySequence { value, .. }
        | ReportValue::IncompleteSelection { value, .. } => is_item(value),
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
            ReportValue::Item(item)
            | ReportValue::EntitySequence { value: item, .. }
            | ReportValue::IncompleteSelection { value: item, .. } => *item,
            _ => return Ok(value),
        }
    }
}
impl<'a> zirql::FunctionHost<Context<'_, 'a>> for composition::CompositionHost<'_, '_> {
    fn retains_native_admission(&self) -> bool {
        true
    }
    fn resolve_type_name(
        &self,
        name: Located<&str>,
        budget: &mut Budget,
    ) -> Result<D<Kind>, zirql::QueryError> {
        budget.charge(
            name.value.len() + 1,
            zirql::Location::Query(name.range.clone()),
        )?;
        match name.value {
            "Node" => Ok(D::Native(Kind::Node)),
            "Declaration" => Ok(D::Native(Kind::Declaration)),
            _ => Err(zirql::QueryError::query(
                name.range,
                "constructor type requires Node or Declaration",
            )),
        }
    }
    fn prepare_computational(
        &self,
        declaration: Option<&D<Kind>>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::QueryError> {
        budget.charge(1, zirql::Location::Query(range.clone()))?;
        if let Some(declaration) = declaration {
            if matches!(declaration, D::IdentitySet(kind) if !matches!(kind, Kind::Node | Kind::Declaration))
            {
                return Err(zirql::QueryError::query(
                    range.clone(),
                    "identity_set requires Node or Declaration",
                ));
            }
            self.declaration_kind(declaration, budget, range)?;
        }
        Ok(())
    }
    fn computational_value<'v>(
        &self,
        value: &'v ReportValue<'a>,
    ) -> Option<&'v zirql::ComputationalValue<ReportValue<'a>, Kind>> {
        if let ReportValue::Computational(value) = unretained(value) {
            Some(&value.value)
        } else {
            None
        }
    }
    fn lift_computational(
        &self,
        value: zirql::ComputationalValue<ReportValue<'a>, Kind>,
        _: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        budget.charge_results(1, zirql::Location::Query(range.clone()))?;
        Ok(ReportValue::Computational(worklists::Computational {
            value: Rc::new(value),
            range: range.clone(),
        }))
    }
    fn construct_tuple(
        &self,
        values: Vec<ReportValue<'a>>,
        _: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        budget.charge_results(values.len() + 1, zirql::Location::Query(range.clone()))?;
        Ok(ReportValue::Tuple(worklists::NativeTuple {
            values: Rc::new(values),
            range: range.clone(),
        }))
    }
    fn sequence_items(
        &self,
        value: &ReportValue<'a>,
        element: &D<Kind>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<ReportValue<'a>>, zirql::EvaluationError<QueryError>> {
        self.sequence_values(value, element, context, budget, range)
    }
    fn construct_sequence(
        &self,
        values: Vec<ReportValue<'a>>,
        element: &D<Kind>,
        _: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        budget.charge_results(values.len() + 1, zirql::Location::Query(range.clone()))?;
        Ok(ReportValue::Sequence(worklists::NativeSequence {
            values: Rc::new(values),
            element: worklists::declaration(element, budget, range)?,
            range: range.clone(),
        }))
    }
    fn worklist_context(
        &self,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::NativeContextId, zirql::EvaluationError<QueryError>> {
        budget.charge(1, zirql::Location::Query(range.clone()))?;
        context
            .report_facts(budget, range)
            .map_err(zirql::EvaluationError::Host)?;
        Ok(zirql::NativeContextId([
            context.input as *const _ as usize as u64,
            context
                .facts
                .as_ref()
                .map_or(0, |f| f.bindings() as *const _ as usize as u64),
            0,
        ]))
    }
    fn native_identity(
        &self,
        value: &ReportValue<'a>,
        expected: &Kind,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::NativeIdentity, zirql::EvaluationError<QueryError>> {
        self.identity(value, expected, context, budget, range)
    }
    fn declaration_kind(
        &self,
        declaration: &D<Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        budget.charge(1, zirql::Location::Query(range.clone()))?;
        Ok(match declaration {
            D::Value => Kind::Value(Rc::new(vec![zirql::ValueType::Unestablished])),
            D::Dictionary(key, value) => {
                if native_declaration(declaration) {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "dictionaries containing native values are unavailable in Zincite",
                    ));
                }
                let key_kind = self.declaration_kind(key, budget, range)?;
                let value_kind = self.declaration_kind(value, budget, range)?;
                let keys = self
                    .scalar_types(&key_kind, budget, range)?
                    .unwrap_or_else(|| vec![zirql::ValueType::Unestablished]);
                let values = self
                    .scalar_types(&value_kind, budget, range)?
                    .unwrap_or_else(|| vec![zirql::ValueType::Unestablished]);
                Kind::Value(Rc::new(vec![zirql::ValueType::Dictionary { keys, values }]))
            }
            D::Boolean => Kind::Scalar(zirql::ParameterType::Bool),
            D::Integer => Kind::Scalar(zirql::ParameterType::Int),
            D::String => Kind::Scalar(zirql::ParameterType::String),
            D::Float => Kind::Value(Rc::new(vec![zirql::ValueType::Float])),
            D::Native(kind) => kind.clone(),
            D::Queue(_) | D::IdentitySet(_) => {
                Kind::Computational(worklists::declaration(declaration, budget, range)?)
            }
            D::Tuple(fields) if native_declaration(declaration) => Kind::Tuple(Rc::new(
                fields
                    .iter()
                    .map(|d| self.declaration_kind(d, budget, range))
                    .collect::<Result<_, _>>()?,
            )),
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
            D::Sequence(inner) | D::Array(inner) if native_declaration(inner) => {
                Kind::Sequence(worklists::declaration(inner, budget, range)?)
            }
            // Preserve known container shape for selector admission on empty inputs.
            D::Member(id) => Kind::Value(Rc::new(vec![zirql::ValueType::Member(*id)])),
            D::Record(_) => Kind::Value(Rc::new(vec![zirql::ValueType::RecordKind])),
            D::Tuple(_) => Kind::Value(Rc::new(vec![zirql::ValueType::TupleKind])),
            D::Set(element) => Kind::Value(Rc::new(vec![zirql::ValueType::Set(Some(*element))])),
            D::Sequence(element) | D::Array(element) | D::Keyed(element) => {
                let element = self.declaration_kind(element, budget, range)?;
                let elements = self
                    .scalar_types(&element, budget, range)?
                    .unwrap_or_else(|| vec![zirql::ValueType::Unestablished]);
                let collection = match declaration {
                    D::Sequence(_) => zirql::ValueType::Array(zirql::ArrayType::Elements(elements)),
                    D::Array(_) => zirql::ValueType::ArrayKind(elements),
                    D::Keyed(_) => zirql::ValueType::KeyedArray(zirql::KeyedArrayType {
                        axes: None,
                        elements,
                    }),
                    _ => unreachable!("collection declaration"),
                };
                Kind::Value(Rc::new(vec![collection]))
            }
        })
    }
    fn is_operation_name(&self, name: &str) -> bool {
        composition::domain_stage(name)
            || Primitive::named(name).is_some()
            || matches!(
                name,
                "source"
                    | "create"
                    | "replace"
                    | "fn"
                    | "enum_member"
                    | "sort"
                    | "sort_by"
                    | "min"
                    | "min_by"
                    | "max"
                    | "max_by"
                    | "min_all"
                    | "min_all_by"
                    | "max_all"
                    | "max_all_by"
                    | "union"
                    | "intersect"
                    | "except"
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
        if let D::Native(Kind::Computational(declaration)) = expected {
            return self.admit_native(value, declaration, context, budget, location);
        }
        if zirql::admit_computational(value, expected, self, context, budget, location.clone())? {
            return Ok(());
        }
        let range = match &location {
            zirql::Location::Query(range) | zirql::Location::Source(range) => range,
        };
        budget.charge(1, location.clone())?;
        match expected {
            D::Native(Kind::Nodes) => {
                return self.admit_native(
                    value,
                    &D::Sequence(Box::new(D::Native(Kind::Node))),
                    context,
                    budget,
                    location,
                );
            }
            D::Native(Kind::Declarations) => {
                return self.admit_native(
                    value,
                    &D::Sequence(Box::new(D::Native(Kind::Declaration))),
                    context,
                    budget,
                    location,
                );
            }
            D::Native(kind @ (Kind::Capture(_) | Kind::Captures(_))) => {
                if &self.kind(value, budget, range)? != kind {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "native capture contract mismatch",
                    )
                    .into());
                }
                let captures: Vec<_> = match unretained(value) {
                    ReportValue::Capture(capture) => vec![capture],
                    ReportValue::Captures(captures) => captures.rows.iter().collect(),
                    _ => {
                        return Err(zirql::QueryError::query(
                            range.clone(),
                            "expected native captures",
                        )
                        .into());
                    }
                };
                for capture in captures {
                    for (_, field) in &capture.fields {
                        if let Some(value) = self.constructed_value(field) {
                            zirql::constructed_value_type(value, budget, location.clone())?;
                        } else {
                            let kind = self.kind(field, budget, range)?;
                            self.admit_native(
                                field,
                                &D::Native(kind),
                                context,
                                budget,
                                location.clone(),
                            )?;
                        }
                    }
                }
                return Ok(());
            }
            _ => {}
        }
        match (expected, unretained(value)) {
            (D::Tuple(fields), ReportValue::Tuple(tuple)) => {
                if fields.len() != tuple.values().len() {
                    return Err(
                        zirql::QueryError::query(range.clone(), "tuple length mismatch").into(),
                    );
                }
                for (value, field) in tuple.values().iter().zip(fields) {
                    zirql::admit_value(value, field, self, context, budget, location.clone())?;
                }
                return Ok(());
            }
            (D::Sequence(inner) | D::Array(inner), ReportValue::Sequence(sequence)) => {
                if sequence.element.as_ref() != inner.as_ref() {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "native sequence element contract mismatch",
                    )
                    .into());
                }
                for value in sequence.values() {
                    zirql::admit_value(value, inner, self, context, budget, location.clone())?;
                }
                return Ok(());
            }
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
                if !worklists::owner(value)
                    .is_some_and(|owner| std::ptr::eq(owner.bindings(), facts.bindings()))
                {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "foreign or unavailable declaration owner",
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
                    if !facts
                        .bindings()
                        .declarations
                        .get(id.0)
                        .is_some_and(|d| d.id == *id)
                    {
                        return Err(zirql::QueryError::query(
                            range.clone(),
                            "unavailable declaration identity",
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
    values: &[&ReportValue<'a>],
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
    host: &composition::CompositionHost<'_, '_>,
) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
    let value = values[0];
    if matches!(primitive, Primitive::SameDeclaration) {
        return Ok(ReportValue::Constructed(zirql::ConstructedValue::Boolean(
            host.identity(value, &Kind::Declaration, context, budget, range)?
                == host.identity(values[1], &Kind::Declaration, context, budget, range)?,
        )));
    }
    if matches!(primitive, Primitive::ReferenceInfo) {
        host.identity(value, &Kind::Declaration, context, budget, range)?;
        let ReportValue::Query(SemanticStream::Entities(rows)) = unretained(value) else {
            unreachable!("admitted declaration")
        };
        let semantic::SemanticEntity::Declaration(id) = rows[0] else {
            unreachable!("admitted declaration")
        };
        let owner = worklists::owner(value).cloned();
        let report = context.query.report.as_mut().ok_or_else(|| {
            zirql::QueryError::query(range.clone(), "reference_info requires model facts")
        })?;
        let rows = report
            .complete_targets(id, &mut Work { budget }, range, host.0.limits)
            .map_err(zirql::EvaluationError::Host)?;
        budget.charge_results(3, zirql::Location::Query(range.clone()))?;
        return Ok(ReportValue::Capture(zirql::NativeCapture {
            fields: vec![(
                "targets".to_owned(),
                ReportValue::EntitySequence {
                    declarations: true,
                    owner,
                    value: Box::new(ReportValue::Query(SemanticStream::Entities(rows))),
                },
            )],
        }));
    }
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
        Primitive::SameDeclaration | Primitive::ReferenceInfo => {
            unreachable!("declaration primitives returned above")
        }
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

pub(super) fn into_value(mut expression: Expression) -> E<Expression> {
    if let ExpressionKind::Sequence { steps, .. } = &mut expression.value
        && steps.len() == 1
        && matches!(
            steps[0].value,
            ExpressionStep::Stage(Stage::FunctionValue(_))
        )
    {
        let mut step = steps.pop().unwrap();
        let ExpressionStep::Stage(Stage::FunctionValue(value)) = &mut step.value else {
            unreachable!()
        };
        std::mem::replace(
            value,
            Located::new(K::Literal(zirql::ConstructedValue::Absent), 0..0),
        )
    } else {
        let range = expression.range.clone();
        Located::new(K::NativeValue(Box::new(expression)), range)
    }
}

fn is_native_record(expression: &Expression) -> bool {
    match &expression.value {
        ExpressionKind::Record(_) => true,
        ExpressionKind::Group(value) | ExpressionKind::Sequence { input: value, .. } => {
            is_native_record(value)
        }
        _ => false,
    }
}

pub(super) fn wrap_program_entries(
    statements: Vec<Located<zirql::Statement<Stage, zirql::NativeSetOperator, Emitter>>>,
    preserve_native: bool,
    queries: &[ProgramQuery],
) -> Vec<Located<zirql::Statement<Stage, zirql::NativeSetOperator, Emitter>>> {
    statements
        .into_iter()
        .map(|statement| {
            let (mut expression, binding, implicit_output) = match statement.value {
                zirql::Statement::BindFields { names, expression } => {
                    return Located::new(
                        zirql::Statement::BindFields { names, expression },
                        statement.range,
                    );
                }
                zirql::Statement::Bind { name, expression } => (expression, Some(name), None),
                zirql::Statement::Evaluate {
                    expression,
                    implicit_output,
                } => (expression, None, implicit_output),
            };
            // Native report fields share the statement input and keep native leaves.
            if is_native_record(&expression) || matches!(&expression.value,
                ExpressionKind::Sequence { steps, .. } if steps.iter().any(|step| matches!(&step.value, ExpressionStep::Stage(Stage::Data(..))) || matches!(&step.value, ExpressionStep::Stage(Stage::Query(index, _)) if queries[*index].query.is_editing()))) {
                let value = match binding {
                    Some(name) => zirql::Statement::Bind { name, expression },
                    None => zirql::Statement::Evaluate { expression, implicit_output },
                };
                return Located::new(value, statement.range);
            }
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
            // Keep existing native pipeline/capture boundaries intact. Control
            // and computational entries need their prepared result contracts.
            let pure = if let ExpressionKind::Sequence { steps, .. } = &expression.value {
                steps.len() == 1 && matches!(&steps[0].value, ExpressionStep::Stage(Stage::FunctionValue(_)))
            } else { false };
            let value = if pure { into_value(expression) } else if preserve_native {
                let retained = Located::new(ExpressionKind::Sequence { input: Box::new(expression),
                    steps: vec![Located::new(ExpressionStep::Stage(Stage::RetainPayload), range.clone())] }, range.clone());
                Located::new(K::NativeValue(Box::new(retained)), range.clone())
            } else { Located::new(K::NativeValue(Box::new(expression)), range.clone()) };
            expression = value_stage(Stage::FunctionValue(value), range.clone());
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
        K::SubjectScope { .. } | K::Field { .. } | K::TupleAccess { .. } => {
            zirql::pattern_function_value_kind_borrowed(
                host.0
                    .declarations
                    .iter()
                    .zip(host.0.signatures)
                    .map(|(body, signature)| body.with_signature(signature)),
                host,
                expression,
                scope,
                Some(input.clone()),
                budget,
            )
        }
        K::Literal(value) => Ok(Kind::Value(Rc::new(vec![comprehension::value_facts(
            value,
            budget,
            &expression.range,
        )?]))),
        K::NativeValue(query) | K::NativeQuery(query) => {
            zirql::native_expression_kind(query, host, input, scope, budget)
        }
        K::One(value) => {
            value_kind(value, host, input, scope, budget)?;
            Ok(Kind::Constructed)
        }
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
                        Stage::Match(matching) => Kind::Captures(Rc::new(zirql::NativeCapture {
                            fields: matching
                                .get()
                                .clauses
                                .iter()
                                .filter_map(|clause| match &clause.value {
                                    zirql::MatchClause::From { name, .. }
                                    | zirql::MatchClause::Bind { name, .. } => {
                                        Some((name.value.clone(), Kind::Constructed))
                                    }
                                    _ => None,
                                })
                                .collect(),
                        })),
                        Stage::Paths(_) => Kind::Captures(Rc::new(zirql::NativeCapture {
                            fields: vec![
                                ("source".into(), Kind::Declaration),
                                ("sink".into(), Kind::Declaration),
                                ("steps".into(), Kind::Declarations),
                            ],
                        })),
                        Stage::Projection(_) => {
                            Kind::Value(Rc::new(vec![zirql::ValueType::Unestablished]))
                        }
                        Stage::FunctionValue(value) | Stage::FunctionSource(value) => match &value
                            .value
                        {
                            K::NativeValue(query) | K::NativeQuery(query) => {
                                parsed_native_kind(query, names, queries)
                            }
                            K::Name(name) => names.get(name).cloned().unwrap_or(Kind::Constructed),
                            _ => Kind::Value(Rc::new(vec![zirql::ValueType::Unestablished])),
                        },
                        Stage::WhereExists(_) => {
                            native::set_kind(&kind, &step.range).unwrap_or(Kind::Constructed)
                        }
                        Stage::FixedPoint(_)
                        | Stage::Ordering(_)
                        | Stage::MapSorting
                        | Stage::Filter(_) => kind,
                        Stage::Mapping(_) => Kind::Dictionary(Rc::new(vec![Kind::Constructed])),
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

pub(super) fn signature_needs_model(signature: &zirql::SignatureType) -> bool {
    use zirql::SignatureType as S;
    match signature {
        S::Named(name) => !matches!(name.value.as_str(), "Node" | "Declaration"),
        S::Optional(inner)
        | S::Sequence(inner)
        | S::Set(inner)
        | S::Keyed(inner)
        | S::Queue(inner)
        | S::IdentitySet(inner) => signature_needs_model(&inner.value),
        S::Dictionary(key, value) => {
            signature_needs_model(&key.value) || signature_needs_model(&value.value)
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
pub(super) fn check_argument(
    expression: &E<Expression>,
    expected: language::ArgumentKind,
    host: &composition::CompositionHost<'_, '_>,
    input: &Kind,
    scope: &BTreeMap<String, Kind>,
    budget: &mut Budget,
) -> Result<(), zirql::QueryError> {
    let kind = value_kind(expression, host, input, scope, budget)?;
    let types = host.scalar_types(&kind, budget, &expression.range)?;
    let Some(types) = types else {
        return Err(zirql::QueryError::query(
            expression.range.clone(),
            "primitive argument has an incompatible scalar type",
        ));
    };
    for ty in &types {
        // Zincite permits unknown facts here, but does not admit ScalarKind.
        let policy = if matches!(ty, zirql::ValueType::Unestablished) {
            zirql::TypeFactPolicy::Deferred
        } else {
            zirql::TypeFactPolicy::Strict
        };
        expected.admit(ty, policy, budget, &expression.range)?;
    }
    Ok(())
}

fn native_declaration(declaration: &D<Kind>) -> bool {
    match declaration {
        D::Native(kind) => !matches!(
            kind,
            Kind::Value(_)
                | Kind::Scalar(_)
                | Kind::Constructed
                | Kind::Count
                | Kind::Strings
                | Kind::Literals
        ),
        D::Queue(_) | D::IdentitySet(_) => true,
        D::Optional(inner) | D::Sequence(inner) | D::Array(inner) | D::Keyed(inner) => {
            native_declaration(inner)
        }
        D::Dictionary(key, value) => native_declaration(key) || native_declaration(value),
        D::Record(fields) => fields.iter().any(|(_, field)| native_declaration(field)),
        D::Tuple(fields) => fields.iter().any(native_declaration),
        _ => false,
    }
}

pub(super) fn entity_contract(value: &ReportValue<'_>) -> Option<bool> {
    match value {
        ReportValue::Retained(value) => entity_contract(value),
        ReportValue::IncompleteSelection { value, .. } => entity_contract(value),
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
            S::Head(_)
            | S::Tail(_)
            | S::Reverse
            | S::Check { .. }
            | S::Filter(_)
            | S::SharedFilter(_)
            | S::Unique
            | S::Json
            | S::Emit => {}
            _ => declarations = false,
        }
    }
    declarations
}

pub(super) fn check_signature(
    signature: &Located<zirql::SignatureType>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<(), QueryError> {
    use zirql::SignatureType as S;
    let mut pending = vec![(signature, 0)];
    while let Some((signature, depth)) = pending.pop() {
        if depth >= nesting.min(64) {
            return Err(QueryError::query(
                signature.range.clone(),
                "signature nesting limit exceeded",
            ));
        }
        let width = match &signature.value {
            S::Optional(_)
            | S::Sequence(_)
            | S::Set(_)
            | S::Keyed(_)
            | S::Queue(_)
            | S::IdentitySet(_) => 1,
            S::Dictionary(_, _) => 2,
            S::Tuple(values) => values.len(),
            S::Record(fields) => fields.len(),
            _ => 0,
        };
        let location = zirql::Location::Query(signature.range.clone());
        budget
            .charge(width.saturating_add(1), location.clone())
            .map_err(pipeline::core_error)?;
        budget
            .charge_results(width.saturating_add(1), location)
            .map_err(pipeline::core_error)?;
        match &signature.value {
            S::Queue(value)
            | S::IdentitySet(value)
            | S::Optional(value)
            | S::Sequence(value)
            | S::Set(value)
            | S::Keyed(value) => pending.push((value, depth + 1)),
            S::Dictionary(key, value) => {
                pending.push((key, depth + 1));
                pending.push((value, depth + 1));
            }
            S::Tuple(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
            S::Record(fields) => pending.extend(fields.iter().map(|(_, value)| (value, depth + 1))),
            S::Value | S::Boolean | S::Integer | S::Float | S::String | S::Named(_) => {}
        }
    }
    Ok(())
}

/// Admit the host's closed value subset, including unselected branches.
pub(super) fn check_value<'v, Q>(
    value: &'v E<Q>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<Vec<&'v Q>, QueryError> {
    check_values(value, budget, nesting, true, false).map(|(natives, _)| natives)
}
pub(super) fn written_value_natives<'v, Q>(
    value: &'v E<Q>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<(Vec<&'v Q>, bool), QueryError> {
    check_values(value, budget, nesting, false, false)
}
pub(super) fn eager_native_values<'v, Q>(
    value: &'v E<Q>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<Vec<&'v Q>, QueryError> {
    check_values(value, budget, nesting, false, true).map(|(natives, _)| natives)
}
fn check_values<'v, Q>(
    value: &'v E<Q>,
    budget: &mut Budget,
    nesting: usize,
    literals: bool,
    eager: bool,
) -> Result<(Vec<&'v Q>, bool), QueryError> {
    let mut pending = vec![(value, 0)];
    let mut natives = Vec::new();
    let mut needs_runtime = false;
    while let Some((value, depth)) = pending.pop() {
        needs_runtime |= matches!(
            &value.value,
            K::Queue { .. }
                | K::IdentitySet { .. }
                | K::PopFront(_)
                | K::PushBackAll { .. }
                | K::Insert { .. }
                | K::Loop { .. }
                | K::Break(_)
                | K::Continue(_)
                | K::Fold { .. }
                | K::Scan { .. }
        ) || matches!(&value.value, K::Call { function, .. } if Primitive::named(&function.value).is_some());
        if depth >= nesting.min(64) {
            return Err(QueryError::query(
                value.range.clone(),
                "expression nesting limit exceeded",
            ));
        }
        budget
            .charge(1, zirql::Location::Query(value.range.clone()))
            .map_err(pipeline::core_error)?;
        // Zirql counts and enumerates every expression/generator form.
        let width = zirql::value_child_count(value);
        let location = zirql::Location::Query(value.range.clone());
        budget
            .charge(width, location.clone())
            .map_err(pipeline::core_error)?;
        budget
            .charge_results(width.saturating_add(1), location)
            .map_err(pipeline::core_error)?;
        let mut children: Vec<&E<Q>> = Vec::with_capacity(width);
        match &value.value {
            K::Queue {
                element: Some(element),
                ..
            }
            | K::IdentitySet { element } => {
                check_signature(element, budget, nesting.saturating_sub(depth + 1))?;
            }
            K::Literal(literal) if literals => {
                zirql::constructed_value_type(
                    literal,
                    budget,
                    zirql::Location::Query(value.range.clone()),
                )
                .map_err(pipeline::core_error)?;
            }
            K::SubjectScope { argument, .. }
            | K::Map { argument, .. }
            | K::FlatMap { argument, .. } => {
                if let Some(pattern) = &argument.argument {
                    zirql::pattern_names(pattern, budget).map_err(pipeline::core_error)?;
                }
            }
            K::PredicateScope { predicate, .. } | K::Filter { predicate, .. } => {
                if let Some(pattern) = &predicate.argument {
                    zirql::pattern_names(pattern, budget).map_err(pipeline::core_error)?;
                }
            }
            K::NativeQuery(query) | K::NativeValue(query) => natives.push(query.as_ref()),
            K::Record(fields) | K::With { fields, .. } => {
                let mut labels = std::collections::BTreeSet::new();
                for (name, _) in fields {
                    budget
                        .charge(
                            name.value.len() + 1,
                            zirql::Location::Query(name.range.clone()),
                        )
                        .map_err(pipeline::core_error)?;
                    if !labels.insert(&name.value) {
                        return Err(QueryError::query(
                            name.range.clone(),
                            "duplicate report label",
                        ));
                    }
                }
            }
            K::Block { bindings, .. } => {
                for (pattern, _) in bindings {
                    zirql::pattern_names(pattern, budget).map_err(pipeline::core_error)?;
                }
            }
            K::ValueMatch { arms, .. } => {
                for arm in arms {
                    zirql::pattern_names(&arm.pattern, budget).map_err(pipeline::core_error)?;
                }
            }
            K::Comprehension { clauses, .. }
            | K::DictionaryComprehension { clauses, .. }
            | K::KeyedComprehension { clauses, .. } => {
                for clause in clauses {
                    match &clause.value {
                        zirql::ValueGenerator::In { binders, .. } => {
                            for pattern in binders {
                                zirql::pattern_names(pattern, budget)
                                    .map_err(pipeline::core_error)?;
                            }
                        }
                        zirql::ValueGenerator::Bind { binder, .. } => {
                            zirql::pattern_names(binder, budget).map_err(pipeline::core_error)?;
                        }
                    }
                }
            }
            _ => {}
        }
        if let K::Default { value, .. } = &value.value
            && eager
        {
            // Native discovery deliberately leaves this deferred fallback alone.
            children.push(value.as_ref());
        } else {
            zirql::for_each_value_child(value, |child| children.push(child));
        }
        pending.extend(children.into_iter().rev().map(|value| (value, depth + 1)));
    }
    Ok((natives, needs_runtime))
}

fn native_record_field(
    value: &E<Expression>,
    names: &BTreeMap<String, Kind>,
    queries: &[ProgramQuery],
) -> bool {
    let kind = match &value.value {
        K::Name(name) => names.get(name).cloned().unwrap_or(Kind::Constructed),
        K::NativeQuery(query) | K::NativeValue(query) => parsed_native_kind(query, names, queries),
        _ => return false,
    };
    !matches!(kind, Kind::Constructed | Kind::Value(_) | Kind::Scalar(_))
}

pub(super) fn retain_native_record(
    expression: &mut E<Expression>,
    names: &BTreeMap<String, Kind>,
    queries: &[ProgramQuery],
) -> bool {
    let K::Record(fields) = &mut expression.value else {
        return false;
    };
    if fields.is_empty()
        || !fields
            .iter()
            .all(|(_, value)| native_record_field(value, names, queries))
    {
        return false;
    }
    let fields = std::mem::take(fields)
        .into_iter()
        .map(|(name, value)| {
            let range = value.range.clone();
            let query = match value.value {
                K::Name(name) => Located::new(ExpressionKind::Binding(name), range),
                K::NativeValue(query) | K::NativeQuery(query) => *query,
                _ => unreachable!("native field checked above"),
            };
            (name, query)
        })
        .collect();
    expression.value = K::NativeValue(Box::new(Located::new(
        ExpressionKind::Record(fields),
        expression.range.clone(),
    )));
    true
}
