//! Owned typed values lower directly to the same prepared report program.
use super::*;
use zirql::ValueExpression as E;

/// A typed value whose native leaves are shared expression wrappers around Query.
/// The wrapper permits flat native pipelines to be released iteratively.
pub type ProgramValue = E<zirql::Expression<ProgramNative, zirql::NativeSetOperator, ()>>;
/// A typed pure function with located pattern parameters and unresolved host types.
pub type ProgramFunction = zirql::UnresolvedPatternFunctionDeclaration<
    zirql::Expression<ProgramNative, zirql::NativeSetOperator, ()>,
>;

/// A native MiniZinc query leaf. Common composition uses Zirql expression steps.
#[derive(Debug)]
pub enum ProgramNative {
    Query(Query),
}

struct Inputs {
    functions: Vec<ProgramFunction>,
    bindings: Vec<(zirql::Pattern, ProgramValue)>,
    report: Option<ProgramValue>,
}
impl Drop for Inputs {
    fn drop(&mut self) {
        for mut function in std::mem::take(&mut self.functions) {
            for parameter in &mut function.parameters {
                dispose_signature(std::mem::replace(
                    &mut parameter.value.declaration.value,
                    zirql::SignatureType::Integer,
                ));
            }
            dispose_signature(std::mem::replace(
                &mut function.result.value,
                zirql::SignatureType::Integer,
            ));
            zirql::dispose_unresolved_pattern_functions(vec![function]);
        }
        for (pattern, value) in std::mem::take(&mut self.bindings) {
            zirql::dispose_pattern(pattern);
            zirql::dispose_native_value(value);
        }
        if let Some(value) = self.report.take() {
            zirql::dispose_native_value(value);
        }
    }
}
impl Program {
    /// Construct a pure report without parsing generated source. Native leaves
    /// use shared expressions with `ProgramNative` stages and native set
    /// operators. Query leaves own domain syntax; ordering selectors retain
    /// their singleton input and the caller's query range.
    ///
    /// All input trees transfer to this call, including on failure. Preparation
    /// against the current input/model happens during ordinary evaluation.
    /// Shared syntax inside a native operand must have one owner:
    /// lowering consumes its opaque native payloads. Sharing runtime values does
    /// not impose this syntax-ownership requirement.
    ///
    /// # Errors
    /// Unsupported constructs, duplicate names and exhausted construction bounds
    /// retain caller-supplied ranges. Shared syntax with multiple owners, direct
    /// filter, match and path payloads and emission are refused before lowering.
    /// Evaluation also checks scope, purity,
    /// pattern compatibility and native types before executing statements.
    pub fn from_values(
        functions: Vec<ProgramFunction>,
        bindings: Vec<(zirql::Pattern, ProgramValue)>,
        final_report: ProgramValue,
        limits: ProgramLimits,
    ) -> Result<Self, QueryError> {
        let mut inputs = Inputs {
            functions,
            bindings,
            report: Some(final_report),
        };
        let mut budget = Budget::for_program(limits.core());
        let range = inputs.report.as_ref().unwrap().range.clone();
        let mut names = std::collections::BTreeSet::new();
        for function in &inputs.functions {
            functions::check_signature(&function.result, &mut budget, limits.nesting)?;
            for parameter in &function.parameters {
                functions::check_signature(
                    &parameter.value.declaration,
                    &mut budget,
                    limits.nesting,
                )?;
            }
            for query in functions::check_value(&function.value, &mut budget, limits.nesting)? {
                check_native(query, &mut budget, limits.nesting)?;
            }
        }
        for (pattern, value) in &inputs.bindings {
            let selected =
                zirql::pattern_names(pattern, &mut budget).map_err(pipeline::core_error)?;
            for name in &selected {
                if !names.insert(name.value.clone()) {
                    return Err(QueryError::query(
                        name.range.clone(),
                        "binding is already defined",
                    ));
                }
            }
            for query in functions::check_value(value, &mut budget, limits.nesting)? {
                check_native(query, &mut budget, limits.nesting)?;
            }
        }
        for query in
            functions::check_value(inputs.report.as_ref().unwrap(), &mut budget, limits.nesting)?
        {
            check_native(query, &mut budget, limits.nesting)?;
        }
        let mut queries = Vec::new();
        let mut lower = |query, range: Range<usize>| {
            let index = queries.len();
            queries.push(ProgramQuery {
                query,
                arguments: Rc::new(Vec::new()),
                copy_units: range.len() + 1,
            });
            Stage::Query(index, Rc::new(Vec::new()))
        };
        let mut functions = std::mem::take(&mut inputs.functions)
            .into_iter()
            .map(|function| zirql::UnresolvedPatternFunctionDeclaration {
                name: function.name,
                parameters: function.parameters,
                result: function.result,
                value: zirql::map_value_payloads(function.value, &mut |query| {
                    map_native(query, &mut lower)
                }),
            })
            .collect::<Vec<_>>();
        let mut statements = Vec::new();
        for (pattern, value) in std::mem::take(&mut inputs.bindings) {
            let range = value.range.clone();
            statements.push(Located::new(
                zirql::Statement::BindPattern {
                    pattern: Arc::new(pattern),
                    value: zirql::map_value_payloads(value, &mut |query| {
                        map_native(query, &mut lower)
                    })
                    .into(),
                },
                range,
            ));
        }
        statements.push(Located::new(
            zirql::Statement::Evaluate {
                expression: core_value(
                    zirql::map_value_payloads(inputs.report.take().unwrap(), &mut |query| {
                        map_native(query, &mut lower)
                    }),
                    range.clone(),
                ),
                implicit_output: Some(Emitter::Json),
            },
            range.clone(),
        ));
        for function in &mut functions {
            functions::normalize_value(
                &mut function.value,
                &BTreeMap::new(),
                &queries,
                &Default::default(),
            );
        }
        for statement in &mut statements {
            let expression = match &mut statement.value {
                zirql::Statement::Bind { expression, .. }
                | zirql::Statement::BindFields { expression, .. }
                | zirql::Statement::Evaluate { expression, .. } => expression,
                zirql::Statement::BindPattern { value, .. } => {
                    functions::normalize_value(
                        Arc::get_mut(value).expect("builder-owned value"),
                        &BTreeMap::new(),
                        &queries,
                        &Default::default(),
                    );
                    continue;
                }
                zirql::Statement::Parameter { .. } => continue,
            };
            if let ExpressionKind::Sequence { steps, .. } = &mut expression.value {
                for step in steps {
                    let value = match &mut step.value {
                        ExpressionStep::Value(value) => {
                            Arc::get_mut(value).expect("builder-owned value")
                        }
                        _ => continue,
                    };
                    functions::normalize_value(
                        value,
                        &BTreeMap::new(),
                        &queries,
                        &Default::default(),
                    );
                }
            }
        }
        let report_model_demand = queries
            .iter()
            .any(|query| query.query.requires_model_facts())
            || functions.iter().any(|function| {
                functions::signature_needs_model(&function.result.value)
                    || function.parameters.iter().any(|parameter| {
                        functions::signature_needs_model(&parameter.value.declaration.value)
                    })
            });
        Ok(Self {
            parameters: Vec::new(),
            program: zirql::Program::new(statements, range.clone()),
            preflight_work: limits.work - budget.remaining_work(),
            preflight_results: limits.results - budget.remaining_results().unwrap(),
            preflight_range: Some(range),
            queries,
            functions,
            function_limits: zirql::FunctionLimits { active_depth: 64 },
            report_model_demand,
            editing: false,
            prepared: true,
            prepared_values: false,
        })
    }
}

type TypedExpression = zirql::Expression<ProgramNative, zirql::NativeSetOperator, ()>;

fn map_native(
    query: TypedExpression,
    lower: &mut impl FnMut(Query, Range<usize>) -> Stage,
) -> Expression {
    let range = query.range;
    Located::new(
        match query.value {
            ExpressionKind::Input => ExpressionKind::Input,
            ExpressionKind::Binding(name) => ExpressionKind::Binding(name),
            ExpressionKind::Group(value) => {
                ExpressionKind::Group(Box::new(map_native(*value, lower)))
            }
            ExpressionKind::Field { value, field } => ExpressionKind::Field {
                value: Box::new(map_native(*value, lower)),
                field,
            },
            ExpressionKind::Record(fields) => ExpressionKind::Record(
                fields
                    .into_iter()
                    .map(|(name, value)| (name, map_native(value, lower)))
                    .collect(),
            ),
            ExpressionKind::Set {
                operator,
                left,
                right,
            } => ExpressionKind::Set {
                operator,
                left: Box::new(map_native(*left, lower)),
                right: Box::new(map_native(*right, lower)),
            },
            ExpressionKind::Sequence { input, steps } => ExpressionKind::Sequence {
                input: Box::new(map_native(*input, lower)),
                steps: steps
                    .into_iter()
                    .map(|step| {
                        let range = step.range;
                        Located::new(map_step(step.value, lower, &range), range)
                    })
                    .collect(),
            },
        },
        range,
    )
}

fn map_body(
    body: zirql::NativeBody<TypedExpression>,
    lower: &mut impl FnMut(Query, Range<usize>) -> Stage,
) -> zirql::NativeBody<Expression> {
    let value = body.into_value();
    let zirql::ValueExpressionKind::NativeValue(query) = value.value else {
        unreachable!("native body")
    };
    zirql::NativeBody::new(map_native(*query, lower), value.range)
}

fn map_step(
    step: zirql::ExpressionStep<ProgramNative, (), zirql::NativeSetOperator>,
    lower: &mut impl FnMut(Query, Range<usize>) -> Stage,
    range: &Range<usize>,
) -> ExpressionStep<Stage, Emitter, zirql::NativeSetOperator> {
    let mut map_value =
        |value| zirql::map_value_payloads(value, &mut |query| map_native(query, lower));
    match step {
        ExpressionStep::Value(value) => ExpressionStep::Value(
            map_value(Arc::try_unwrap(value).expect("builder-owned value")).into(),
        ),
        ExpressionStep::Head(value) => ExpressionStep::Head(
            map_value(Arc::try_unwrap(value).expect("builder-owned bound")).into(),
        ),
        ExpressionStep::Tail(value) => ExpressionStep::Tail(
            map_value(Arc::try_unwrap(value).expect("builder-owned bound")).into(),
        ),
        ExpressionStep::Check { operand, message } => ExpressionStep::Check {
            operand: operand.map(|value| {
                map_value(Arc::try_unwrap(value).expect("builder-owned check operand")).into()
            }),
            message: message.map(|value| {
                map_value(Arc::try_unwrap(value).expect("builder-owned check message")).into()
            }),
        },
        ExpressionStep::Projection(projection) => {
            let projection = Arc::try_unwrap(projection).expect("builder-owned projection");
            ExpressionStep::Projection(
                zirql::Projection {
                    subject: projection.subject,
                    fields: projection
                        .fields
                        .into_iter()
                        .map(|(label, value)| (label, map_value(value)))
                        .collect(),
                }
                .into(),
            )
        }
        ExpressionStep::Ordering(ordering) => {
            let ordering = Arc::try_unwrap(ordering).expect("builder-owned ordering");
            ExpressionStep::Ordering(
                zirql::NativeOrdering {
                    order: ordering.order,
                    selector: ordering.selector.map(|body| map_body(body, lower)),
                }
                .into(),
            )
        }
        ExpressionStep::Mapping(mapping) => {
            let mapping = Arc::try_unwrap(mapping).expect("builder-owned mapping");
            ExpressionStep::Mapping(
                zirql::NativeMap {
                    key: map_body(mapping.key, lower),
                    value: map_body(mapping.value, lower),
                }
                .into(),
            )
        }
        ExpressionStep::Group(body) => ExpressionStep::Group(
            map_body(Arc::try_unwrap(body).expect("builder-owned group"), lower).into(),
        ),
        ExpressionStep::WhereExists(body) => ExpressionStep::WhereExists(
            map_body(
                Arc::try_unwrap(body).expect("builder-owned relation"),
                lower,
            )
            .into(),
        ),
        ExpressionStep::FixedPoint(body) => ExpressionStep::FixedPoint(
            map_body(Arc::try_unwrap(body).expect("builder-owned body"), lower).into(),
        ),
        ExpressionStep::MapSorting => ExpressionStep::MapSorting,
        ExpressionStep::Collection(operation) => ExpressionStep::Collection(operation),
        ExpressionStep::Stage(ProgramNative::Query(query)) => {
            ExpressionStep::Stage(lower(query, range.clone()))
        }
        ExpressionStep::Filter(_)
        | ExpressionStep::Match(_)
        | ExpressionStep::Paths(_)
        | ExpressionStep::Emit { .. } => {
            unreachable!("unsupported typed payload rejected before lowering")
        }
    }
}

fn check_native(
    query: &zirql::Expression<ProgramNative, zirql::NativeSetOperator, ()>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<(), QueryError> {
    zirql::walk_expression(query, budget, nesting, |_, _| Ok(())).map_err(pipeline::core_error)?;
    let mut pending = vec![(query, 0)];
    while let Some((query, depth)) = pending.pop() {
        if depth >= nesting.min(64) {
            return Err(QueryError::query(
                query.range.clone(),
                "native expression nesting limit exceeded",
            ));
        }
        budget
            .charge(1, zirql::Location::Query(query.range.clone()))
            .map_err(pipeline::core_error)?;
        match &query.value {
            ExpressionKind::Input | ExpressionKind::Binding(_) => {}
            ExpressionKind::Group(value) | ExpressionKind::Field { value, .. } => {
                pending.push((value, depth + 1))
            }
            ExpressionKind::Record(fields) => {
                let location = zirql::Location::Query(query.range.clone());
                budget
                    .charge(fields.len(), location.clone())
                    .map_err(pipeline::core_error)?;
                budget
                    .charge_results(fields.len(), location)
                    .map_err(pipeline::core_error)?;
                pending.extend(fields.iter().map(|(_, v)| (v, depth + 1)))
            }
            ExpressionKind::Set { left, right, .. } => {
                pending.push((left, depth + 1));
                pending.push((right, depth + 1));
            }
            ExpressionKind::Sequence { input, steps } => {
                pending.push((input, depth + 1));
                for step in steps {
                    budget
                        .charge_results(1, zirql::Location::Query(step.range.clone()))
                        .map_err(pipeline::core_error)?;
                    let owners = match &step.value {
                        ExpressionStep::Value(value)
                        | ExpressionStep::Head(value)
                        | ExpressionStep::Tail(value) => Arc::strong_count(value),
                        ExpressionStep::Check { operand, message } => operand
                            .iter()
                            .chain(message)
                            .map(Arc::strong_count)
                            .max()
                            .unwrap_or(1),
                        ExpressionStep::Projection(value) => Arc::strong_count(value),
                        ExpressionStep::Ordering(value) => Arc::strong_count(value),
                        ExpressionStep::Mapping(value) => Arc::strong_count(value),
                        ExpressionStep::Group(value)
                        | ExpressionStep::WhereExists(value)
                        | ExpressionStep::FixedPoint(value) => Arc::strong_count(value),
                        ExpressionStep::Filter(_)
                        | ExpressionStep::Match(_)
                        | ExpressionStep::Paths(_) => {
                            return Err(QueryError::query(
                                step.range.clone(),
                                "typed native filter, match and path payloads are unsupported",
                            ));
                        }
                        ExpressionStep::Emit { .. } => {
                            return Err(QueryError::query(
                                step.range.clone(),
                                "typed report values must be pure",
                            ));
                        }
                        _ => 1,
                    };
                    if owners != 1 {
                        return Err(QueryError::query(
                            step.range.clone(),
                            "typed native syntax must have one owner before lowering",
                        ));
                    }
                    let mut checked = Ok(());
                    step.value.for_each_value(|value| {
                        if checked.is_ok() {
                            checked = functions::check_value(
                                value,
                                budget,
                                nesting.saturating_sub(depth + 1),
                            )
                            .map(|queries| {
                                pending.extend(queries.into_iter().map(|query| (query, depth + 1)))
                            });
                        }
                    });
                    checked?;
                    let mut checked = Ok(());
                    step.value.for_each_payload(|ProgramNative::Query(query)| {
                        if checked.is_err() {
                            return;
                        }
                        checked = check_query(query, budget, &step.range);
                    });
                    checked?;
                }
            }
        }
    }
    Ok(())
}

fn check_query(query: &Query, budget: &mut Budget, range: &Range<usize>) -> Result<(), QueryError> {
    if query.is_editing()
        || query.stages.iter().any(|stage| {
            matches!(
                stage.kind,
                language::StageKind::Emit
                    | language::StageKind::Json
                    | language::StageKind::EmitDocument
            )
        })
    {
        return Err(QueryError::query(
            range.clone(),
            "typed report values must be pure",
        ));
    }
    let location = zirql::Location::Query(range.clone());
    budget
        .charge(query.range.len() + query.stages.len(), location.clone())
        .map_err(pipeline::core_error)?;
    budget
        .charge_results(query.stages.len(), location)
        .map_err(pipeline::core_error)
}

fn dispose_signature(signature: zirql::SignatureType) {
    use zirql::SignatureType as S;
    let mut pending = vec![signature];
    while let Some(signature) = pending.pop() {
        match signature {
            S::Optional(value)
            | S::Sequence(value)
            | S::Set(value)
            | S::Keyed(value)
            | S::Queue(value)
            | S::IdentitySet(value) => pending.push(value.value),
            S::Dictionary(key, value) => {
                pending.push(key.value);
                pending.push(value.value);
            }
            S::Tuple(values) => pending.extend(values.into_iter().map(|value| value.value)),
            S::Record(fields) => pending.extend(fields.into_iter().map(|(_, value)| value.value)),
            S::Value | S::Boolean | S::Integer | S::Float | S::String | S::Named(_) => {}
        }
    }
}
