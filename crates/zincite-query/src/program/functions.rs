//! Request-local function preparation and native type admission.
use super::*;
use zirql::{DeclaredType as D, ProjectionHost, ValueExpression as E, ValueExpressionKind as K};

const NATIVE_FUNCTION_NAMES: &[&str] = &[
    "direct_children",
    "node_kind",
    "written_call_name",
    "same_declaration",
    "reference_info",
];
pub(super) fn is_native_function(name: &str) -> bool {
    NATIVE_FUNCTION_NAMES.contains(&name)
}

fn native_function<'c, 'a, 'h, 'q>(
    name: &str,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<
    Option<zirql::NativeFunction<Context<'c, 'a>, composition::CompositionHost<'h, 'q>>>,
    zirql::QueryError,
> {
    let Some(index) = NATIVE_FUNCTION_NAMES
        .iter()
        .position(|candidate| *candidate == name)
    else {
        return Ok(None);
    };
    // Every definition below has at most two parameters and one nested result.
    budget.charge_results(8, zirql::Location::Query(range.clone()))?;
    budget.charge(
        8 * std::mem::size_of::<D<Kind>>(),
        zirql::Location::Query(range.clone()),
    )?;
    let (parameters, result, invoke): (
        _,
        _,
        zirql::NativeFunctionImplementation<Context<'c, 'a>, composition::CompositionHost<'h, 'q>>,
    ) = match index {
        0 => (
            vec![D::Native(Kind::Node)],
            D::Sequence(Box::new(D::Native(Kind::Node))),
            direct_children,
        ),
        1 => (vec![D::Native(Kind::Node)], D::String, node_kind),
        2 => (
            vec![D::Native(Kind::Node)],
            D::Optional(Box::new(D::String)),
            written_call_name,
        ),
        3 => (
            vec![D::Native(Kind::Declaration), D::Native(Kind::Declaration)],
            D::Boolean,
            same_declaration,
        ),
        4 => {
            budget.charge("targets".len(), zirql::Location::Query(range.clone()))?;
            (
                vec![D::Native(Kind::Declaration)],
                D::Record(vec![(
                    "targets".into(),
                    D::Sequence(Box::new(D::Native(Kind::Declaration))),
                )]),
                reference_info,
            )
        }
        _ => unreachable!("native definition index"),
    };
    Ok(Some(zirql::NativeFunction {
        name: NATIVE_FUNCTION_NAMES[index],
        signature: zirql::OperationSignature { parameters, result },
        invoke,
    }))
}

impl Parser<'_> {
    pub(super) fn admit_program_functions(
        &mut self,
        functions: &mut [zirql::UnresolvedPatternFunctionDeclaration<Expression>],
        budget: &mut Budget,
    ) -> Result<(), QueryError> {
        for function in functions.iter() {
            check_signature(&function.result, budget, self.limits.nesting)?;
            for parameter in &function.parameters {
                check_signature(&parameter.value.declaration, budget, self.limits.nesting)?;
            }
            check_value(&function.value, budget, self.limits.nesting)?;
            self.report_model_demand |= signature_needs_model(&function.result.value)
                || function
                    .parameters
                    .iter()
                    .any(|parameter| signature_needs_model(&parameter.value.declaration.value));
        }
        self.constructed_functions = functions
            .iter()
            .filter(|function| constructed_signature(&function.result.value))
            .map(|function| function.name.value.clone())
            .collect();
        for function in functions {
            normalize_value(
                &mut function.value,
                &BTreeMap::new(),
                &self.queries,
                &self.constructed_functions,
            );
        }
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
        normalize_value(
            &mut value,
            &self.names,
            &self.queries,
            &self.constructed_functions,
        );
        Ok(value)
    }
}

pub(super) fn normalize_value(
    expression: &mut E<Expression>,
    names: &BTreeMap<String, Kind>,
    queries: &[ProgramQuery],
    constructed_functions: &std::collections::BTreeSet<String>,
) {
    recover_call_value(expression, queries, constructed_functions);
    lower_recovered_value(expression, names, queries, constructed_functions);
}

// The caller already applied late-known function result facts at this root.
// Children still use the complete recovery/lowering entry point.
pub(super) fn lower_recovered_value(
    expression: &mut E<Expression>,
    names: &BTreeMap<String, Kind>,
    queries: &[ProgramQuery],
    constructed_functions: &std::collections::BTreeSet<String>,
) {
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
        normalize_value(child, names, queries, constructed_functions);
    });
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
// Payload mapping stays with the host. Spelling, terminal classification and
// lowering use the same suffix kinds as the reader.
fn value_suffix_kind(
    stage: &ExpressionStep<Stage, Emitter, zirql::NativeSetOperator>,
    queries: &[ProgramQuery],
) -> Option<zirql::ValueSuffixKind> {
    if matches!(stage, ExpressionStep::Ordering(order) if order.order == zirql::NativeOrder::Sort && order.selector.is_none())
    {
        return Some(zirql::ValueSuffixKind::Sort);
    }
    if let ExpressionStep::Collection(operation) = stage {
        return Some(match operation {
            zirql::Stage::Count => zirql::ValueSuffixKind::Count,
            zirql::Stage::Unique => zirql::ValueSuffixKind::Unique,
            zirql::Stage::Tally => zirql::ValueSuffixKind::Tally,
            _ => return None,
        });
    }
    let ExpressionStep::Stage(Stage::Query(index, _)) = stage else {
        return None;
    };
    let [stage] = queries[*index].query.stages.as_slice() else {
        return None;
    };
    Some(match stage.kind {
        language::StageKind::Count => zirql::ValueSuffixKind::Count,
        language::StageKind::Unique => zirql::ValueSuffixKind::Unique,
        language::StageKind::Tally => zirql::ValueSuffixKind::Tally,
        _ => return None,
    })
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
                    value: ExpressionStep::Value(value),
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
                    terminal = value_suffix_kind(&step.value, queries)?.is_terminal();
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
                    Some(ExpressionStep::Value(_))
                ) {
                    let step = steps.remove(0);
                    let ExpressionStep::Value(value) = step.value else {
                        unreachable!()
                    };
                    Arc::try_unwrap(value).expect("parser-owned value")
                } else {
                    take(input, queries)?
                };
                for step in std::mem::take(steps) {
                    let suffix = value_suffix_kind(&step.value, queries)
                        .expect("suffixes checked above")
                        .without_arguments()
                        .expect("host suffix has no arguments");
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
            if steps.len() == 1 && matches!(steps[0].value, ExpressionStep::Value(_)) =>
        {
            let step = steps.pop().unwrap();
            let ExpressionStep::Value(value) = step.value else {
                unreachable!()
            };
            Some(Arc::try_unwrap(value).expect("parser-owned value"))
        }
        _ => None,
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
impl<'c, 'a> zirql::FunctionHost<Context<'c, 'a>> for composition::CompositionHost<'_, '_> {
    fn native_function(
        &self,
        name: &str,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Option<zirql::NativeFunction<Context<'c, 'a>, Self>>, zirql::QueryError> {
        native_function(name, budget, range)
    }
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
        zirql::is_common_operation_name(name)
            || composition::domain_stage(name)
            || matches!(
                name,
                "source"
                    | "create"
                    | "replace"
                    | "fn"
                    | "enum_member"
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

fn native_node<'a>(value: &ReportValue<'a>) -> &'a zincite_syntax::SyntaxNode {
    let ReportValue::Query(SemanticStream::Inspection(result)) = unretained(value) else {
        unreachable!("admitted node")
    };
    match result.as_ref() {
        QueryResult::Nodes(selection) => selection.nodes[0].node,
        QueryResult::Selection(selection) => selection.items[0].node,
        _ => unreachable!("admitted node"),
    }
}
fn direct_children<'a>(
    _: &composition::CompositionHost<'_, '_>,
    values: Vec<Located<ReportValue<'a>>>,
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
    let node = native_node(&values[0].value);
    let input = context.input;
    let count = node.child_nodes().count();
    budget.charge(count, zirql::Location::Query(range.clone()))?;
    budget.charge_results(count + 1, zirql::Location::Query(range.clone()))?;
    budget.check_items(count, zirql::Location::Query(range.clone()))?;
    Ok(ReportValue::Query(SemanticStream::Inspection(Box::new(
        QueryResult::Nodes(crate::NodeSelection {
            input,
            nodes: node
                .child_nodes()
                .map(|node| crate::SelectedNode {
                    node,
                    file: input.file(),
                    range: input.original_range(node.range()),
                })
                .collect(),
        }),
    ))))
}
fn node_kind<'a>(
    _: &composition::CompositionHost<'_, '_>,
    values: Vec<Located<ReportValue<'a>>>,
    _: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
    let name = inspection::kind_name(native_node(&values[0].value).kind());
    budget.charge(name.len(), zirql::Location::Query(range.clone()))?;
    Ok(ReportValue::Constructed(zirql::ConstructedValue::String(
        name,
    )))
}
fn written_call_name<'a>(
    _: &composition::CompositionHost<'_, '_>,
    values: Vec<Located<ReportValue<'a>>>,
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
    Ok(ReportValue::Constructed(
        match inspection::call_name(context.input, native_node(&values[0].value)) {
            Some(name) => {
                budget.charge(name.len(), zirql::Location::Query(range.clone()))?;
                zirql::ConstructedValue::String(name.to_owned())
            }
            None => zirql::ConstructedValue::Absent,
        },
    ))
}
fn same_declaration<'a>(
    host: &composition::CompositionHost<'_, '_>,
    values: Vec<Located<ReportValue<'a>>>,
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
    Ok(ReportValue::Constructed(zirql::ConstructedValue::Boolean(
        host.identity(&values[0].value, &Kind::Declaration, context, budget, range)?
            == host.identity(&values[1].value, &Kind::Declaration, context, budget, range)?,
    )))
}
fn reference_info<'a>(
    host: &composition::CompositionHost<'_, '_>,
    values: Vec<Located<ReportValue<'a>>>,
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
    let value = &values[0].value;
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
    Ok(ReportValue::Capture(zirql::NativeCapture {
        fields: vec![(
            "targets".to_owned(),
            ReportValue::EntitySequence {
                declarations: true,
                owner,
                value: Box::new(ReportValue::Query(SemanticStream::Entities(rows))),
            },
        )],
    }))
}

pub(super) fn into_value(mut expression: Expression) -> E<Expression> {
    if let ExpressionKind::Sequence { steps, .. } = &mut expression.value
        && steps.len() == 1
        && matches!(steps[0].value, ExpressionStep::Value(_))
    {
        let step = steps.pop().unwrap();
        let ExpressionStep::Value(value) = step.value else {
            unreachable!()
        };
        Arc::try_unwrap(value).expect("owned value expression")
    } else {
        let range = expression.range.clone();
        Located::new(K::NativeValue(Box::new(expression)), range)
    }
}

pub(super) fn expression_kind(
    expression: &Expression,
    host: &composition::CompositionHost<'_, '_>,
    input: &Kind,
    scope: &BTreeMap<String, Kind>,
    budget: &mut Budget,
) -> Result<Kind, zirql::QueryError> {
    zirql::pattern_function_expression_kind_borrowed(
        host.0
            .declarations
            .iter()
            .zip(host.0.signatures)
            .map(|(body, signature)| body.with_signature(signature)),
        host,
        expression,
        scope,
        Some(input.clone()),
        false,
        budget,
    )
}
pub(super) fn value_kind(
    expression: &E<Expression>,
    host: &composition::CompositionHost<'_, '_>,
    input: &Kind,
    scope: &BTreeMap<String, Kind>,
    budget: &mut Budget,
) -> Result<Kind, zirql::QueryError> {
    budget.charge(1, zirql::Location::Query(expression.range.clone()))?;
    match &expression.value {
        K::Name(name) => scope.get(name).cloned().ok_or_else(|| {
            zirql::QueryError::query(
                expression.range.clone(),
                format!("undefined binding: {name}"),
            )
        }),
        K::Call { .. } | K::SubjectScope { .. } | K::Field { .. } | K::TupleAccess { .. } => {
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
            expression_kind(query, host, input, scope, budget)
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
                if let ExpressionStep::Value(value) = &step.value {
                    kind = match &value.value {
                        K::NativeValue(query) | K::NativeQuery(query) => {
                            parsed_native_kind(query, names, queries)
                        }
                        K::Name(name) => names.get(name).cloned().unwrap_or(Kind::Constructed),
                        _ => Kind::Value(Rc::new(vec![zirql::ValueType::Unestablished])),
                    };
                } else if matches!(&step.value, ExpressionStep::Projection(_)) {
                    kind = Kind::Value(Rc::new(vec![zirql::ValueType::Unestablished]));
                }
                kind = match &step.value {
                    ExpressionStep::Match(matching) => {
                        Kind::Captures(Rc::new(zirql::NativeCapture {
                            fields: matching
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
                        }))
                    }
                    ExpressionStep::Paths(_) => Kind::Captures(Rc::new(zirql::NativeCapture {
                        fields: vec![
                            ("source".into(), Kind::Declaration),
                            ("sink".into(), Kind::Declaration),
                            ("steps".into(), Kind::Declarations),
                        ],
                    })),
                    ExpressionStep::WhereExists(_) => {
                        native::set_kind(&kind, &step.range).unwrap_or(Kind::Constructed)
                    }
                    ExpressionStep::Mapping(_) => {
                        Kind::Dictionary(Rc::new(vec![Kind::Constructed]))
                    }
                    ExpressionStep::Collection(operation) => {
                        let stage = match operation {
                            zirql::Stage::Count => Some(language::StageKind::Count),
                            zirql::Stage::Unique => Some(language::StageKind::Unique),
                            zirql::Stage::Tally => Some(language::StageKind::Tally),
                            zirql::Stage::Head(n) => Some(language::StageKind::Head(*n)),
                            zirql::Stage::Tail(n) => Some(language::StageKind::Tail(*n)),
                            zirql::Stage::Reverse => Some(language::StageKind::Reverse),
                            zirql::Stage::Check { expected, message } => {
                                Some(language::StageKind::Check {
                                    expected: *expected,
                                    message: message.clone(),
                                })
                            }
                            _ => None,
                        };
                        stage
                            .and_then(|stage| {
                                validate_stages(
                                    &Query {
                                        stages: vec![language::Stage {
                                            kind: stage,
                                            range: step.range.clone(),
                                        }],
                                        range: step.range.clone(),
                                        nesting_limit: 64,
                                    },
                                    kind.clone(),
                                )
                                .ok()
                            })
                            .unwrap_or(kind)
                    }
                    ExpressionStep::Stage(Stage::Query(index, _)) => {
                        validate_stages(&queries[*index].query, kind).unwrap_or(Kind::Constructed)
                    }
                    ExpressionStep::Stage(_) => Kind::Constructed,
                    _ => kind,
                };
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
        ) || matches!(&value.value, K::Call { function, .. } if is_native_function(&function.value));
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
