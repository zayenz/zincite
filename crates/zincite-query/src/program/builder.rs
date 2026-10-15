//! Owned typed values lower directly to the same prepared report program.
use super::*;
use zirql::{ValueExpression as E, ValueExpressionKind as K};

/// A typed value whose native leaves are shared expression wrappers around Query.
/// The wrapper permits flat native pipelines to be released iteratively.
pub type ProgramValue = E<zirql::Expression<Query, (), ()>>;
/// A typed pure function with located pattern parameters and unresolved host types.
pub type ProgramFunction =
    zirql::UnresolvedPatternFunctionDeclaration<zirql::Expression<Query, (), ()>>;

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
    /// use shared `Expression<Query, (), ()>` wrappers; the Query owns domain
    /// syntax and the enclosing value retains the caller's query range.
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
                value: map_queries(function.value, &mut lower),
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
                        Stage::PatternBinding(pattern, map_queries(value, &mut lower)),
                        range.clone(),
                    ),
                },
                range,
            ));
        }
        statements.push(Located::new(
            zirql::Statement::Evaluate {
                expression: value_stage(
                    Stage::FunctionValue(map_queries(inputs.report.take().unwrap(), &mut lower)),
                    range.clone(),
                ),
                implicit_output: Some(Emitter::Json),
            },
            range.clone(),
        ));
        for function in &mut functions {
            functions::lower_primitives(&mut function.value, &BTreeMap::new(), &queries);
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
                        functions::lower_primitives(value, &BTreeMap::new(), &queries);
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
    query: zirql::Expression<Query, (), ()>,
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
            ExpressionKind::Set { .. } => {
                unreachable!("typed set composition rejected before lowering")
            }
            ExpressionKind::Sequence { input, steps } => {
                let mut input = map_native(*input, lower);
                for step in steps {
                    match step.value {
                        ExpressionStep::Stage(query) => {
                            let mut leaf = lower(query, step.range.clone());
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

fn map_queries(
    value: ProgramValue,
    lower: &mut impl FnMut(Query, Range<usize>) -> Expression,
) -> E<Expression> {
    let range = value.range;
    let value = match value.value {
        K::CurrentInput => K::CurrentInput,
        K::Literal(value) => K::Literal(value),
        K::Name(name) => K::Name(name),
        K::NativeQuery(query) => K::NativeQuery(Box::new(map_native(*query, lower))),
        K::NativeValue(query) => K::NativeValue(Box::new(map_native(*query, lower))),
        K::Call {
            function,
            arguments,
        } => K::Call {
            function,
            arguments: arguments
                .into_iter()
                .map(|value| map_queries(value, lower))
                .collect(),
        },
        K::Array(values) => K::Array(values.into_iter().map(|v| map_queries(v, lower)).collect()),
        K::Tuple(values) => K::Tuple(values.into_iter().map(|v| map_queries(v, lower)).collect()),
        K::Set(values) => K::Set(values.into_iter().map(|v| map_queries(v, lower)).collect()),
        K::Record(fields) => K::Record(
            fields
                .into_iter()
                .map(|(n, v)| (n, map_queries(v, lower)))
                .collect(),
        ),
        K::Block { bindings, value } => K::Block {
            bindings: bindings
                .into_iter()
                .map(|(p, v)| (p, map_queries(v, lower)))
                .collect(),
            value: Box::new(map_queries(*value, lower)),
        },
        K::ValueMatch { subject, arms } => K::ValueMatch {
            subject: Box::new(map_queries(*subject, lower)),
            arms: arms
                .into_iter()
                .map(|arm| zirql::ValueMatchArm {
                    pattern: arm.pattern,
                    guard: arm.guard.map(|v| map_queries(v, lower)),
                    value: map_queries(arm.value, lower),
                })
                .collect(),
        },
        K::Conditional {
            branches,
            otherwise,
        } => K::Conditional {
            branches: branches
                .into_iter()
                .map(|(a, b)| (map_queries(a, lower), map_queries(b, lower)))
                .collect(),
            otherwise: Box::new(map_queries(*otherwise, lower)),
        },
        K::KeyedArray { axes, entries } => K::KeyedArray {
            axes,
            entries: entries
                .into_iter()
                .map(|(a, b)| (map_queries(a, lower), map_queries(b, lower)))
                .collect(),
        },
        K::Comprehension {
            constructor,
            clauses,
            head,
        } => K::Comprehension {
            constructor,
            clauses: map_clauses(clauses, lower),
            head: Box::new(map_queries(*head, lower)),
        },
        K::KeyedComprehension {
            clauses,
            key,
            head,
            empty_axis_sources,
        } => K::KeyedComprehension {
            clauses: map_clauses(clauses, lower),
            key: Box::new(map_queries(*key, lower)),
            head: Box::new(map_queries(*head, lower)),
            empty_axis_sources: empty_axis_sources
                .map(|values| values.into_iter().map(|v| map_queries(v, lower)).collect()),
        },
        K::Unique(value) => K::Unique(Box::new(map_queries(*value, lower))),
        K::Tally(value) => K::Tally(Box::new(map_queries(*value, lower))),
        K::Occurs(value) => K::Occurs(Box::new(map_queries(*value, lower))),
        K::Absent(value) => K::Absent(Box::new(map_queries(*value, lower))),
        K::One(value) => K::One(Box::new(map_queries(*value, lower))),
        K::IndexSet(value) => K::IndexSet(Box::new(map_queries(*value, lower))),
        K::Entries(value) => K::Entries(Box::new(map_queries(*value, lower))),
        K::Unary { operator, operand } => K::Unary {
            operator,
            operand: Box::new(map_queries(*operand, lower)),
        },
        K::Binary {
            operator,
            left,
            right,
        } => K::Binary {
            operator,
            left: Box::new(map_queries(*left, lower)),
            right: Box::new(map_queries(*right, lower)),
        },
        K::Field { value, field } => K::Field {
            value: Box::new(map_queries(*value, lower)),
            field,
        },
        K::TupleAccess { value, position } => K::TupleAccess {
            value: Box::new(map_queries(*value, lower)),
            position,
        },
        K::ArrayAccess { array, index } => K::ArrayAccess {
            array: Box::new(map_queries(*array, lower)),
            index: Box::new(map_queries(*index, lower)),
        },
        K::Lookup { value, key } => K::Lookup {
            value: Box::new(map_queries(*value, lower)),
            key: Box::new(map_queries(*key, lower)),
        },
        K::HasKey { value, key } => K::HasKey {
            value: Box::new(map_queries(*value, lower)),
            key: Box::new(map_queries(*key, lower)),
        },
        K::Default { value, fallback } => K::Default {
            value: Box::new(map_queries(*value, lower)),
            fallback: Box::new(map_queries(*fallback, lower)),
        },
        K::ZipExact { left, right } => K::ZipExact {
            left: Box::new(map_queries(*left, lower)),
            right: Box::new(map_queries(*right, lower)),
        },
        K::Reduce { operator, array } => K::Reduce {
            operator,
            array: Box::new(map_queries(*array, lower)),
        },
        K::FilterStartsWith { array, prefix } => K::FilterStartsWith {
            array: Box::new(map_queries(*array, lower)),
            prefix,
        },
        K::Sort {
            array,
            field,
            descending,
        } => K::Sort {
            array: Box::new(map_queries(*array, lower)),
            field,
            descending,
        },
        K::Extremum {
            operator,
            array,
            field,
        } => K::Extremum {
            operator,
            array: Box::new(map_queries(*array, lower)),
            field,
        },
        K::GroupBy { rows, field } => K::GroupBy {
            rows: Box::new(map_queries(*rows, lower)),
            field,
        },
        K::Require {
            condition,
            value,
            message,
        } => K::Require {
            condition: Box::new(map_queries(*condition, lower)),
            value: Box::new(map_queries(*value, lower)),
            message,
        },
        K::Range {
            lower: start,
            upper,
        } => K::Range {
            lower: Box::new(map_queries(*start, lower)),
            upper: Box::new(map_queries(*upper, lower)),
        },
        K::Queue { items, element } => K::Queue {
            items: Box::new(map_queries(*items, lower)),
            element,
        },
        K::PopFront(value) => K::PopFront(Box::new(map_queries(*value, lower))),
        K::Break(value) => K::Break(Box::new(map_queries(*value, lower))),
        K::Continue(values) => {
            K::Continue(values.into_iter().map(|v| map_queries(v, lower)).collect())
        }
        K::PushBackAll { queue, items } => K::PushBackAll {
            queue: Box::new(map_queries(*queue, lower)),
            items: Box::new(map_queries(*items, lower)),
        },
        K::IdentitySet { element } => K::IdentitySet { element },
        K::Insert { set, item } => K::Insert {
            set: Box::new(map_queries(*set, lower)),
            item: Box::new(map_queries(*item, lower)),
        },
        K::Fold {
            items,
            initial,
            step,
        } => K::Fold {
            items: Box::new(map_queries(*items, lower)),
            initial: Box::new(map_queries(*initial, lower)),
            step,
        },
        K::Scan {
            items,
            initial,
            step,
        } => K::Scan {
            items: Box::new(map_queries(*items, lower)),
            initial: Box::new(map_queries(*initial, lower)),
            step,
        },
        K::Loop { states, body } => K::Loop {
            states: states
                .into_iter()
                .map(|(n, v)| (n, map_queries(v, lower)))
                .collect(),
            body: Box::new(map_queries(*body, lower)),
        },
    };
    Located::new(value, range)
}
fn map_clauses(
    clauses: Vec<Located<zirql::ValueGenerator<zirql::Expression<Query, (), ()>>>>,
    lower: &mut impl FnMut(Query, Range<usize>) -> Expression,
) -> Vec<Located<zirql::ValueGenerator<Expression>>> {
    clauses
        .into_iter()
        .map(|clause| {
            Located::new(
                match clause.value {
                    zirql::ValueGenerator::In {
                        binders,
                        source,
                        filter,
                    } => zirql::ValueGenerator::In {
                        binders,
                        source: map_queries(source, lower),
                        filter: filter.map(|v| map_queries(v, lower)),
                    },
                    zirql::ValueGenerator::Bind {
                        binder,
                        value,
                        filter,
                    } => zirql::ValueGenerator::Bind {
                        binder,
                        value: map_queries(value, lower),
                        filter: filter.map(|v| map_queries(v, lower)),
                    },
                },
                clause.range,
            )
        })
        .collect()
}

fn check_native(
    query: &zirql::Expression<Query, (), ()>,
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
            ExpressionKind::Set { .. } => {
                return Err(QueryError::query(
                    query.range.clone(),
                    "set composition is unsupported in Zincite programs",
                ));
            }
            ExpressionKind::Sequence { input, steps } => {
                pending.push((input, depth + 1));
                for step in steps {
                    budget
                        .charge_results(1, zirql::Location::Query(step.range.clone()))
                        .map_err(pipeline::core_error)?;
                    match &step.value {
                        ExpressionStep::Stage(query) if !query.is_editing() => {
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
            S::Tuple(values) => pending.extend(values.into_iter().map(|value| value.value)),
            S::Record(fields) => pending.extend(fields.into_iter().map(|(_, value)| value.value)),
            S::Boolean | S::Integer | S::Float | S::String | S::Named(_) => {}
        }
    }
}
