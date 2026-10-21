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

/// Typed native stages use the same selection and collection execution as text.
#[derive(Debug)]
pub enum ProgramNative {
    Query(Query),
    Ordering(zirql::NativeOrdering<zirql::Expression<Self, zirql::NativeSetOperator, ()>>),
    /// Construct an insertion-ordered dictionary from singleton-relative selectors.
    Mapping(zirql::NativeMap<zirql::Expression<Self, zirql::NativeSetOperator, ()>>),
    /// Stably sort dictionary values with the native typed scalar key contract.
    MapSorting,
    /// Keep original subjects whose pure native relation is nonempty.
    Exists(Box<zirql::Expression<Self, zirql::NativeSetOperator, ()>>),
    /// Replace the native selection until its ordered identities are unchanged.
    FixedPoint(Box<zirql::Expression<Self, zirql::NativeSetOperator, ()>>),
}

impl ProgramNative {
    fn take_selectors(
        &mut self,
        pending: &mut Vec<zirql::Expression<Self, zirql::NativeSetOperator, ()>>,
    ) {
        match self {
            Self::Ordering(ordering) => pending.extend(ordering.selector.take()),
            Self::Mapping(mapping) => {
                pending.push(std::mem::replace(
                    &mut mapping.key,
                    Located::new(ExpressionKind::Input, 0..0),
                ));
                pending.push(std::mem::replace(
                    &mut mapping.value,
                    Located::new(ExpressionKind::Input, 0..0),
                ));
            }
            Self::Exists(body) | Self::FixedPoint(body) => pending.push(*std::mem::replace(
                body,
                Box::new(Located::new(ExpressionKind::Input, 0..0)),
            )),
            Self::Query(_) | Self::MapSorting => {}
        }
    }
}

impl Drop for ProgramNative {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        self.take_selectors(&mut pending);
        while let Some(expression) = pending.pop() {
            match expression.value {
                ExpressionKind::Input | ExpressionKind::Binding(_) => {}
                ExpressionKind::Group(value) | ExpressionKind::Field { value, .. } => {
                    pending.push(*value)
                }
                ExpressionKind::Record(fields) => {
                    pending.extend(fields.into_iter().map(|(_, value)| value))
                }
                ExpressionKind::Set { left, right, .. } => {
                    pending.push(*left);
                    pending.push(*right);
                }
                ExpressionKind::Sequence { input, steps } => {
                    pending.push(*input);
                    for mut step in steps {
                        if let ExpressionStep::Stage(stage) = &mut step.value {
                            stage.take_selectors(&mut pending);
                        }
                    }
                }
            }
        }
    }
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
    ///
    /// # Errors
    /// Unsupported constructs, duplicate names and exhausted construction bounds
    /// retain caller-supplied ranges. Evaluation also checks scope, purity,
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
        let mut binding_names = Vec::new();
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
            binding_names.push(selected.into_iter().cloned().collect());
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
            value_stage(Stage::Query(index, Rc::new(Vec::new())), range)
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
        for ((pattern, value), names) in std::mem::take(&mut inputs.bindings)
            .into_iter()
            .zip(binding_names)
        {
            let range = value.range.clone();
            statements.push(Located::new(
                zirql::Statement::BindFields {
                    names,
                    expression: value_stage(
                        Stage::PatternBinding(
                            pattern,
                            zirql::map_value_payloads(value, &mut |query| {
                                map_native(query, &mut lower)
                            }),
                        ),
                        range.clone(),
                    ),
                },
                range,
            ));
        }
        statements.push(Located::new(
            zirql::Statement::Evaluate {
                expression: value_stage(
                    Stage::FunctionValue(zirql::map_value_payloads(
                        inputs.report.take().unwrap(),
                        &mut |query| map_native(query, &mut lower),
                    )),
                    range.clone(),
                ),
                implicit_output: Some(Emitter::Json),
            },
            range.clone(),
        ));
        for function in &mut functions {
            functions::lower_primitives(
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
            };
            if let ExpressionKind::Sequence { steps, .. } = &mut expression.value {
                for step in steps {
                    if let ExpressionStep::Stage(
                        Stage::FunctionValue(value) | Stage::PatternBinding(_, value),
                    ) = &mut step.value
                    {
                        functions::lower_primitives(
                            value,
                            &BTreeMap::new(),
                            &queries,
                            &Default::default(),
                        );
                    }
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

fn map_native(
    query: zirql::Expression<ProgramNative, zirql::NativeSetOperator, ()>,
    lower: &mut impl FnMut(Query, Range<usize>) -> Expression,
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
            ExpressionKind::Sequence { input, steps } => {
                let mut input = map_native(*input, lower);
                for step in steps {
                    match step.value {
                        ExpressionStep::Stage(mut stage) => {
                            let mut leaf = match &mut stage {
                                ProgramNative::Query(query) => lower(
                                    std::mem::replace(
                                        query,
                                        Query {
                                            stages: Vec::new(),
                                            range: 0..0,
                                            nesting_limit: 1,
                                        },
                                    ),
                                    step.range.clone(),
                                ),
                                ProgramNative::Mapping(mapping) => value_stage(
                                    Stage::Mapping(native::MappingStage::new(zirql::NativeMap {
                                        key: map_native(
                                            std::mem::replace(
                                                &mut mapping.key,
                                                Located::new(ExpressionKind::Input, 0..0),
                                            ),
                                            lower,
                                        ),
                                        value: map_native(
                                            std::mem::replace(
                                                &mut mapping.value,
                                                Located::new(ExpressionKind::Input, 0..0),
                                            ),
                                            lower,
                                        ),
                                    })),
                                    step.range.clone(),
                                ),
                                ProgramNative::Exists(body) | ProgramNative::FixedPoint(body) => {
                                    let body = map_native(
                                        *std::mem::replace(
                                            body,
                                            Box::new(Located::new(ExpressionKind::Input, 0..0)),
                                        ),
                                        lower,
                                    );
                                    let body = native::BodyStage::new(body);
                                    let stage = if matches!(stage, ProgramNative::Exists(_)) {
                                        Stage::WhereExists(body)
                                    } else {
                                        Stage::FixedPoint(body)
                                    };
                                    value_stage(stage, step.range.clone())
                                }
                                ProgramNative::MapSorting => {
                                    value_stage(Stage::MapSorting, step.range.clone())
                                }
                                ProgramNative::Ordering(ordering) => value_stage(
                                    Stage::Ordering(native::OrderingStage::new(
                                        zirql::NativeOrdering {
                                            order: ordering.order,
                                            selector: ordering
                                                .selector
                                                .take()
                                                .map(|selector| map_native(selector, lower)),
                                        },
                                    )),
                                    step.range.clone(),
                                ),
                            };
                            let ExpressionKind::Sequence { steps, .. } = &mut leaf.value else {
                                unreachable!()
                            };
                            input = Located::new(
                                ExpressionKind::Sequence {
                                    input: Box::new(input),
                                    steps: std::mem::take(steps),
                                },
                                step.range,
                            );
                        }
                        ExpressionStep::Emit { .. } => {
                            unreachable!("typed emission rejected before lowering")
                        }
                    }
                }
                return input;
            }
        },
        range,
    )
}

fn check_native(
    query: &zirql::Expression<ProgramNative, zirql::NativeSetOperator, ()>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<(), QueryError> {
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
                    match &step.value {
                        ExpressionStep::Stage(ProgramNative::Query(query))
                            if !query.is_editing()
                                && !query.stages.iter().any(|stage| {
                                    matches!(
                                        stage.kind,
                                        language::StageKind::Emit
                                            | language::StageKind::Json
                                            | language::StageKind::EmitDocument
                                    )
                                }) =>
                        {
                            budget
                                .charge(
                                    query.range.len() + query.stages.len(),
                                    zirql::Location::Query(step.range.clone()),
                                )
                                .map_err(pipeline::core_error)?;
                            budget
                                .charge_results(
                                    query.stages.len(),
                                    zirql::Location::Query(step.range.clone()),
                                )
                                .map_err(pipeline::core_error)?;
                        }
                        ExpressionStep::Stage(ProgramNative::Mapping(mapping)) => {
                            pending.push((&mapping.key, depth + 1));
                            pending.push((&mapping.value, depth + 1));
                        }
                        ExpressionStep::Stage(
                            ProgramNative::Exists(body) | ProgramNative::FixedPoint(body),
                        ) => pending.push((body, depth + 1)),
                        ExpressionStep::Stage(ProgramNative::MapSorting) => {}
                        ExpressionStep::Stage(ProgramNative::Ordering(ordering)) => {
                            if let Some(selector) = &ordering.selector {
                                pending.push((selector, depth + 1));
                            }
                        }
                        ExpressionStep::Stage(_) | ExpressionStep::Emit { .. } => {
                            return Err(QueryError::query(
                                step.range.clone(),
                                "typed report values must be pure",
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
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
