//! Concrete native composition adapter; Zirql reads, validates and executes clauses.
use super::*;
use zirql::{MatchHost, ProjectionHost, ValueReaderHost};

type Matching = zirql::NativeMatch<Expression, Guard>;
type Paths = zirql::NativePaths<Stage, Guard>;

#[derive(Debug)]
pub(super) struct PathStage(Option<Box<Paths>>);
impl PathStage {
    pub(super) fn get(&self) -> &Paths {
        self.0.as_ref().expect("retained paths")
    }
}
impl Drop for PathStage {
    fn drop(&mut self) {
        if let Some(paths) = self.0.take() {
            zirql::dispose_paths(*paths);
        }
    }
}

#[derive(Debug)]
pub(super) struct ProjectionStage(Option<zirql::Projection<Expression>>);
impl ProjectionStage {
    pub(super) fn get(&self) -> &zirql::Projection<Expression> {
        self.0.as_ref().expect("retained projection")
    }
}
impl Drop for ProjectionStage {
    fn drop(&mut self) {
        if let Some(projection) = self.0.take() {
            for (_, value) in projection.fields {
                zirql::dispose_native_value(value);
            }
        }
    }
}
#[derive(Debug)]
pub(super) struct MatchStage(Option<Matching>);
impl MatchStage {
    pub(super) fn get(&self) -> &Matching {
        self.0.as_ref().expect("retained match")
    }
}
impl Drop for MatchStage {
    fn drop(&mut self) {
        if let Some(matching) = self.0.take() {
            zirql::dispose_match(matching);
        }
    }
}
#[derive(Debug)]
pub(super) struct FilterStage(Rc<FilterBody>);
#[derive(Debug)]
struct FilterBody(Option<zirql::Predicate<Guard>>);
impl FilterStage {
    pub(super) fn new(predicate: zirql::Predicate<Guard>) -> Self {
        Self(Rc::new(FilterBody(Some(predicate))))
    }
    pub(super) fn get(&self) -> &zirql::Predicate<Guard> {
        self.0.0.as_ref().expect("retained filter")
    }
}
impl Clone for FilterStage {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}
impl Drop for FilterBody {
    fn drop(&mut self) {
        if let Some(predicate) = self.0.take() {
            let mut pending = vec![predicate];
            while let Some(predicate) = pending.pop() {
                match predicate.value {
                    zirql::PredicateKind::Leaf(_) => {}
                    zirql::PredicateKind::Not(child) | zirql::PredicateKind::Group(child) => {
                        pending.push(*child)
                    }
                    zirql::PredicateKind::And(children) | zirql::PredicateKind::Or(children) => {
                        pending.extend(children)
                    }
                }
            }
        }
    }
}
#[derive(Debug)]
pub(super) struct Guard {
    pub(super) boolean: Option<zirql::ValueExpression<Expression>>,
    operand: Option<Expression>,
    prepared: Option<zirql::ValueExpression<Expression>>,
    argument: Option<(language::ArgumentKind, zirql::ValueExpression<Expression>)>,
    stage: language::Stage,
}
impl Guard {
    fn operand(&self) -> &Expression {
        if let Some(value) = &self.prepared {
            let zirql::ValueExpressionKind::NativeValue(query) = &value.value else {
                unreachable!()
            };
            query
        } else {
            self.operand.as_ref().expect("guard operand")
        }
    }
    pub(super) fn values(&self) -> impl Iterator<Item = &zirql::ValueExpression<Expression>> {
        self.boolean
            .iter()
            .chain(self.prepared.iter())
            .chain(self.argument.iter().map(|(_, value)| value))
    }
    fn predicate(&self) -> &language::Predicate {
        let language::StageKind::Filter(predicate) = &self.stage.kind else {
            unreachable!("guard leaf")
        };
        predicate
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(value) = self.boolean.take() {
            zirql::dispose_native_value(value);
        }
        if let Some(value) = self.prepared.take() {
            zirql::dispose_native_value(value);
        }
        if let Some((_, value)) = self.argument.take() {
            zirql::dispose_native_value(value);
        }
        if let Some(operand) = self.operand.take() {
            zirql::dispose_expression(operand);
        }
    }
}

impl Parser<'_> {
    fn guard_argument_end(&self, start: usize) -> usize {
        use zincite_syntax::TokenKind as T;
        let mut depth = 0usize;
        for token in self
            .lexed
            .tokens()
            .iter()
            .filter(|token| token.range.start >= start)
        {
            match token.kind {
                T::Comma | T::RightParen if depth == 0 => return token.range.start,
                T::LeftParen | T::LeftBracket | T::LeftBrace | T::MatrixStart => depth += 1,
                T::RightParen | T::RightBracket | T::RightBrace | T::MatrixEnd => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
        }
        self.cursor.source.len()
    }
    pub(super) fn composition_expression(
        &mut self,
        range: Range<usize>,
        native_record: bool,
        binding: bool,
    ) -> Result<(Expression, Kind, bool), QueryError> {
        self.begin_pure(range.clone())?;
        let mut budget =
            std::mem::replace(&mut self.preflight, Budget::for_program(self.limits.core()));
        let mut reader = zirql::ArgumentReader::new(self.cursor.source, range.start)
            .map_err(pipeline::core_error)?;
        let locals = self.reader_locals();
        let parsed = zirql::read_value_expression_in_scope(
            &mut reader,
            self,
            &locals.iter().map(String::as_str).collect::<Vec<_>>(),
            &mut budget,
            self.reader_nesting,
        );
        self.preflight = budget;
        let mut value = parsed.map_err(pipeline::core_error)?;
        let (natives, needs_runtime) =
            functions::written_value_natives(&value, &mut self.preflight, self.reader_nesting)?;
        if self.function_names.is_empty() && natives.is_empty() && !needs_runtime {
            let facts = self
                .scope_types(&range)?
                .into_iter()
                .map(|(name, ty)| (name, vec![ty]))
                .collect();
            zirql::query_value_types(&value, &facts, &mut ConstructedTypes, &mut self.preflight)
                .map_err(pipeline::core_error)?;
        }
        if let zirql::ValueExpressionKind::Name(name) = &value.value
            && self.names.get(name).is_some_and(|kind| {
                !matches!(kind, Kind::Constructed | Kind::Value(_) | Kind::Scalar(_))
                    && (!binding || !matches!(kind, Kind::Count | Kind::Strings | Kind::Literals))
            })
        {
            value = Located::new(
                zirql::ValueExpressionKind::NativeValue(Box::new(Located::new(
                    ExpressionKind::Binding(name.clone()),
                    value.range.clone(),
                ))),
                value.range,
            );
        }
        let retained_record = native_record
            && functions::retain_native_record(&mut value, &self.names, &self.queries);
        functions::recover_call_value(&mut value, &self.queries, &self.constructed_functions);
        let native = matches!(
            value.value,
            zirql::ValueExpressionKind::NativeQuery(_) | zirql::ValueExpressionKind::NativeValue(_)
        );
        if !native && !retained_record {
            functions::lower_primitives(
                &mut value,
                &self.names,
                &self.queries,
                &self.constructed_functions,
            );
        }
        if let Some(error) = self.composition_error.take() {
            zirql::dispose_native_value(value);
            return Err(error);
        }
        self.cursor.position = reader.position();
        self.cursor.space();
        if self.cursor.position != range.end {
            zirql::dispose_native_value(value);
            return Err(self
                .cursor
                .error("expected one native or projected value expression"));
        }
        if retained_record {
            let zirql::ValueExpressionKind::NativeValue(query) = value.value else {
                unreachable!()
            };
            return Ok((*query, Kind::Record, false));
        }
        if !native {
            self.prepared_values = true;
            self.cursor.position = range.end;
            let kind = match &value.value {
                zirql::ValueExpressionKind::Name(name) => {
                    self.names.get(name).cloned().unwrap_or(Kind::Constructed)
                }
                _ => Kind::Value(Rc::new(vec![zirql::ValueType::Unestablished])),
            };
            return Ok((value_stage(Stage::FunctionValue(value), range), kind, false));
        }
        let mut expression = match value.value {
            zirql::ValueExpressionKind::NativeQuery(query)
            | zirql::ValueExpressionKind::NativeValue(query) => *query,
            kind => {
                self.prepared_values = true;
                self.report_model_demand |= self.queries.iter().any(|query| {
                    query
                        .query
                        .stages
                        .iter()
                        .any(|stage| matches!(stage.kind, language::StageKind::Values))
                });
                let value = Located::new(kind, value.range);
                return Ok((
                    value_stage(Stage::FunctionValue(value), range),
                    Kind::Constructed,
                    false,
                ));
            }
        };
        check_edit_order(&expression, &self.queries, &mut false)?;
        let host = NativeHost {
            functions: self.prepared || !self.function_names.is_empty(),
            signatures: &[],
            declarations: &[],
            parameters: &[],
            queries: &self.queries,
            pipelines: Vec::new(),
            editing: false,
            nesting: self.limits.nesting,
            limits: self.limits.query(),
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
        let emits = terminal.is_some();
        let result = if !self.function_names.is_empty()
            || self
                .names
                .values()
                .any(|kind| matches!(kind, Kind::Constructed))
            || self.queries.iter().any(|query| query.query.is_editing())
        {
            Ok(functions::parsed_native_kind(
                &expression,
                &self.names,
                &self.queries,
            ))
        } else {
            zirql::native_expression_kind(
                &expression,
                &CompositionHost(&host),
                &Kind::Nodes,
                &self.names,
                &mut self.preflight,
            )
        };
        if let (ExpressionKind::Sequence { steps, .. }, Some(terminal)) =
            (&mut expression.value, terminal)
        {
            steps.push(terminal);
        }
        match result {
            Ok(kind) => Ok((expression, kind, emits)),
            Err(error) => {
                zirql::dispose_expression(expression);
                Err(pipeline::core_error(error))
            }
        }
    }
    pub(super) fn admit_data_expression(
        &self,
        expression: &Expression,
        budget: &mut Budget,
        terminal: bool,
    ) -> Result<(), QueryError> {
        budget
            .charge(1, zirql::Location::Query(expression.range.clone()))
            .map_err(pipeline::core_error)?;
        match &expression.value {
            ExpressionKind::Input | ExpressionKind::Binding(_) => Ok(()),
            ExpressionKind::Group(value) | ExpressionKind::Field { value, .. } => {
                self.admit_data_expression(value, budget, false)
            }
            ExpressionKind::Record(fields) => {
                for (_, value) in fields {
                    self.admit_data_expression(value, budget, false)?;
                }
                Ok(())
            }
            ExpressionKind::Sequence { input, steps } => {
                self.admit_data_expression(input, budget, false)?;
                for (index, step) in steps.iter().enumerate() {
                    if terminal
                        && index + 1 == steps.len()
                        && matches!(
                            step.value,
                            ExpressionStep::Emit {
                                emitter: Emitter::Source,
                                ..
                            }
                        )
                    {
                        continue;
                    }
                    let ExpressionStep::Stage(stage) = &step.value else {
                        return Err(QueryError::query(
                            step.range.clone(),
                            "emitters are unavailable in data expressions",
                        ));
                    };
                    self.admit_data_stage(stage, &step.range, budget)?;
                }
                Ok(())
            }
            _ => Err(QueryError::query(
                expression.range.clone(),
                "report-only native expression is unavailable in data programs",
            )),
        }
    }
    fn admit_data_value(
        &self,
        value: &zirql::ValueExpression<Expression>,
        budget: &mut Budget,
        input: bool,
    ) -> Result<(), QueryError> {
        let root = if input {
            match &value.value {
                zirql::ValueExpressionKind::NativeQuery(query)
                | zirql::ValueExpressionKind::NativeValue(query) => Some(query.as_ref()),
                zirql::ValueExpressionKind::One(value) => match &value.value {
                    zirql::ValueExpressionKind::NativeQuery(query)
                    | zirql::ValueExpressionKind::NativeValue(query) => Some(query.as_ref()),
                    _ => None,
                },
                _ => None,
            }
        } else {
            None
        };
        comprehension::admit_data(
            value,
            self.cursor.source,
            budget,
            &mut |query, range, budget| {
                let atom = matches!(query.value, ExpressionKind::Input)
                    || matches!(&query.value, ExpressionKind::Sequence { steps, .. } if steps.iter().all(|step| matches!(step.value, ExpressionStep::Stage(Stage::Member(_)))));
                if atom || root.is_some_and(|root| std::ptr::eq(root, query)) {
                    self.admit_data_expression(query, budget, false)
                } else {
                    Err(QueryError::query(
                        range.clone(),
                        "report-only native expression is unavailable in data expressions",
                    ))
                }
            },
        )
    }

    fn admit_data_stage(
        &self,
        stage: &Stage,
        range: &Range<usize>,
        budget: &mut Budget,
    ) -> Result<(), QueryError> {
        match stage {
            Stage::Copy(_) | Stage::Data(..) | Stage::Parameter(_) | Stage::Member(_) => Ok(()),
            Stage::FunctionValue(value)
            | Stage::FunctionSource(value)
            | Stage::PatternBinding(_, value) => self.admit_data_value(value, budget, true),
            Stage::RecordValue(fields) => {
                if self
                    .cursor
                    .source
                    .get(range.clone())
                    .is_some_and(|text| text.trim_start().starts_with('{'))
                {
                    return Err(QueryError::query(
                        range.clone(),
                        "report-only form is unavailable in data expressions",
                    ));
                }
                for (_, value) in fields {
                    self.admit_data_value(value, budget, false)?;
                }
                Ok(())
            }
            Stage::Filter(predicate) => {
                for guard in guard_leaves(predicate.get()) {
                    if semantic::requests_facts(&guard.stage) {
                        return Err(QueryError::query(
                            range.clone(),
                            "semantic predicate is unavailable in data programs",
                        ));
                    }
                    for value in guard.values() {
                        self.admit_data_value(value, budget, false)?;
                    }
                }
                Ok(())
            }
            Stage::Query(index, arguments) => {
                for stage in &self.queries[*index].query.stages {
                    if !matches!(
                        stage.kind,
                        language::StageKind::FilterElements(_)
                            | language::StageKind::Items
                            | language::StageKind::Filter(_)
                            | language::StageKind::Head(_)
                            | language::StageKind::Range(_)
                            | language::StageKind::Values
                            | language::StageKind::Names
                            | language::StageKind::Count
                            | language::StageKind::Unique
                            | language::StageKind::Fields
                            | language::StageKind::Elements
                            | language::StageKind::Keys
                    ) || semantic::requests_facts(stage)
                    {
                        return Err(QueryError::query(
                            stage.range.clone(),
                            "report-only native expression is unavailable in data programs",
                        ));
                    }
                }
                for argument in arguments.iter() {
                    self.admit_data_value(&argument.expression, budget, false)?;
                }
                Ok(())
            }
            _ => Err(QueryError::query(
                range.clone(),
                "report-only native expression is unavailable in data programs",
            )),
        }
    }
    fn register(&mut self, stage: language::Stage) -> usize {
        let index = self.queries.len();
        let range = stage.range.clone();
        self.queries.push(ProgramQuery {
            query: Query {
                stages: vec![stage],
                range: range.clone(),
                nesting_limit: self.limits.nesting,
            },
            arguments: Rc::new(Vec::new()),
            copy_units: range.len() + 1,
        });
        index
    }
}
fn check_edit_order(
    expression: &Expression,
    queries: &[ProgramQuery],
    edited: &mut bool,
) -> Result<(), QueryError> {
    match &expression.value {
        ExpressionKind::Group(value) => check_edit_order(value, queries, edited)?,
        ExpressionKind::Sequence { input, steps } => {
            check_edit_order(input, queries, edited)?;
            for step in steps {
                if *edited
                    && !matches!(
                        step.value,
                        ExpressionStep::Emit {
                            emitter: Emitter::Source,
                            ..
                        }
                    )
                {
                    return Err(QueryError::query(
                        step.range.clone(),
                        "only emit may follow a transformation stage",
                    ));
                }
                if let ExpressionStep::Stage(Stage::Query(index, _)) = &step.value {
                    *edited |= queries[*index].query.is_editing();
                }
            }
        }
        _ => {}
    }
    Ok(())
}
pub(super) fn domain_stage(name: &str) -> bool {
    matches!(
        name,
        "where_exists"
            | "fixpoint"
            | "items"
            | "subtree"
            | "children"
            | "expressions"
            | "references"
            | "declarations"
            | "uses"
            | "transitive_references"
            | "referenced_declarations"
            | "paths"
            | "types"
            | "instantiations"
            | "names"
            | "call_names"
            | "annotation_names"
            | "text"
            | "values"
            | "fields"
            | "elements"
            | "keys"
            | "range"
            | "head"
            | "tail"
            | "reverse"
            | "check"
            | "count"
            | "unique"
            | "tally"
            | "filter"
            | "filter_elements"
            | "project"
            | "json"
            | "emit"
            | "set_value"
            | "remove"
            | "reduce_enum"
            | "emit_document"
    )
}
impl ValueReaderHost for Parser<'_> {
    type Stage = Stage;
    type SetOperator = zirql::NativeSetOperator;
    type Emitter = Emitter;
    type Predicate = Guard;
    fn is_binding(&self, name: &str) -> bool {
        self.function_scope
            .as_ref()
            .unwrap_or(&self.names)
            .contains_key(name)
    }
    fn retains_native_values(&self) -> bool {
        true
    }
    fn field_pipeline_source(&self) -> zirql::FieldPipelineSource {
        zirql::FieldPipelineSource::ValueArgument
    }
    fn value_source(
        &mut self,
        value: zirql::ValueExpression<Expression>,
        _: Range<usize>,
    ) -> Result<Stage, zirql::QueryError> {
        Ok(Stage::FunctionSource(value))
    }
    fn is_stage(&self, name: &str) -> bool {
        matches!(name, "clone_members" | "clone_elements" | "repeat_elements")
            || domain_stage(name)
            || self.function_names.contains(name)
            || functions::Primitive::named(name).is_some()
    }
    fn parse_stage_in_scope(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        locals: &[&str],
        budget: &mut Budget,
        nesting: usize,
    ) -> Result<ExpressionStep<Stage, Emitter>, zirql::QueryError> {
        let scope = self.function_scope.replace(
            locals
                .iter()
                .map(|name| {
                    (
                        (*name).to_owned(),
                        self.names.get(*name).cloned().unwrap_or(Kind::Constructed),
                    )
                })
                .collect(),
        );
        let previous = std::mem::replace(&mut self.reader_nesting, nesting);
        let result = self.parse_stage(reader, budget);
        self.reader_nesting = previous;
        self.function_scope = scope;
        result
    }
    fn parse_predicate_leaf_in_scope(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        locals: &[&str],
        budget: &mut Budget,
        nesting: usize,
    ) -> Result<Guard, zirql::QueryError> {
        let scope = self.function_scope.replace(
            locals
                .iter()
                .map(|name| {
                    (
                        (*name).to_owned(),
                        self.names.get(*name).cloned().unwrap_or(Kind::Constructed),
                    )
                })
                .collect(),
        );
        let previous = std::mem::replace(&mut self.reader_nesting, nesting);
        let result = self.parse_predicate_leaf(reader, budget);
        self.reader_nesting = previous;
        self.function_scope = scope;
        result
    }
    fn parse_stage(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        budget: &mut Budget,
    ) -> Result<ExpressionStep<Stage, Emitter>, zirql::QueryError> {
        let mut copy_probe = zirql::ArgumentReader::new(reader.source(), reader.position())?;
        if let Ok(name) = copy_probe.word()
            && matches!(
                name.value,
                "clone_members" | "clone_elements" | "repeat_elements"
            )
        {
            return self
                .parse_copy_stage(reader, budget)
                .map(ExpressionStep::Stage);
        }
        let mut probe = zirql::ArgumentReader::new(reader.source(), reader.position())?;
        if probe.keyword("paths") {
            let paths = zirql::read_paths(reader, self, budget, self.reader_nesting)?;
            self.cursor.position = reader.position();
            return Ok(ExpressionStep::Stage(Stage::Paths(PathStage(Some(
                Box::new(paths),
            )))));
        }
        let mut element_probe = zirql::ArgumentReader::new(reader.source(), reader.position())?;
        if element_probe.keyword("filter_elements") && element_probe.consume('(') {
            let start = reader.position();
            let mut direct = zirql::ArgumentReader::new(reader.source(), element_probe.position())?;
            let comparison = direct
                .word()
                .is_ok_and(|name| matches!(name.value, "eq" | "lt" | "le" | "gt" | "ge"))
                && direct.consume('(');
            if !comparison {
                reader.word()?;
                reader.expect('(')?;
                let predicate =
                    comprehension::read_scalar_predicate(reader, budget, self.reader_nesting)
                        .map_err(query_error)?;
                reader.expect(')')?;
                let range = start..reader.position();
                let index = self.register(language::Stage {
                    kind: language::StageKind::FilterElements(
                        crate::structured::Comparison::shared(predicate, 0),
                    ),
                    range,
                });
                return Ok(ExpressionStep::Stage(Stage::Query(
                    index,
                    Rc::clone(&self.queries[index].arguments),
                )));
            }
        }
        self.cursor.position = reader.position();
        let start = self.cursor.position;
        let mut probe = zirql::ArgumentReader::new(reader.source(), start)?;
        if let Ok(name) = probe.word()
            && !domain_stage(name.value)
        {
            let name = reader.word()?;
            let function = Located::new(name.value.to_owned(), name.range);
            reader.expect('(')?;
            let mut arguments = Vec::new();
            if !reader.consume(')') {
                loop {
                    reader.skip_whitespace();
                    let locals = self.reader_locals();
                    let value = zirql::read_value_expression_in_scope(
                        reader,
                        self,
                        &locals.iter().map(String::as_str).collect::<Vec<_>>(),
                        budget,
                        self.reader_nesting,
                    )?;
                    budget.charge_results(1, zirql::Location::Query(value.range.clone()))?;
                    arguments.push(value);
                    if !reader.consume(',') {
                        reader.expect(')')?;
                        break;
                    }
                }
            }
            let range = function.range.start..reader.position();
            let mut value = Located::new(
                zirql::ValueExpressionKind::Call {
                    function,
                    arguments,
                },
                range,
            );
            functions::lower_primitives(
                &mut value,
                &self.names,
                &self.queries,
                &self.constructed_functions,
            );
            return Ok(ExpressionStep::Stage(Stage::FunctionSource(value)));
        }
        let mut arguments = Vec::new();
        let limits = self.limits;
        let source = self.cursor.source;
        std::mem::swap(&mut self.preflight, budget);
        let mut lower = |owner, kind: language::ArgumentKind, range: Range<usize>| {
            let expression = self.read_value(range.clone(), self.reader_nesting)?;
            let eager_members = Vec::new();
            self.preflight
                .charge_results(1, zirql::Location::Query(range))
                .map_err(pipeline::core_error)?;
            arguments.push(QueryArgument {
                owner,
                kind,
                expression,
                eager_members,
            });
            Ok(())
        };
        let mut cursor = language::Parser {
            source,
            position: start,
            nesting_limit: limits.nesting.min(64),
            collection_limit: limits.stream_items,
            arguments: Some(&mut lower),
            shared_budget: None,
        };
        let parsed = cursor.stage();
        let position = cursor.position;
        std::mem::swap(&mut self.preflight, budget);
        let stage = parsed.map_err(query_error)?;
        self.cursor.position = position;
        reader.advance(position - reader.position())?;
        budget.charge(
            stage.range.len() + 1,
            zirql::Location::Query(stage.range.clone()),
        )?;
        budget.charge_results(1, zirql::Location::Query(stage.range.clone()))?;
        Ok(match stage.kind {
            language::StageKind::Json => ExpressionStep::Emit {
                emitter: Emitter::Json,
                policy: EmissionPolicy::Terminal,
            },
            language::StageKind::Emit => ExpressionStep::Emit {
                emitter: Emitter::Source,
                policy: EmissionPolicy::Terminal,
            },
            _ => {
                let index = self.register(stage);
                self.queries[index].arguments = Rc::new(arguments);
                ExpressionStep::Stage(Stage::Query(
                    index,
                    Rc::clone(&self.queries[index].arguments),
                ))
            }
        })
    }

    fn parse_predicate_leaf(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        budget: &mut Budget,
    ) -> Result<Guard, zirql::QueryError> {
        let name = reader.word()?;
        let argument = match name.value {
            "kind" => language::ArgumentKind::Kind,
            "name" => language::ArgumentKind::Name,
            "type" => language::ArgumentKind::Type,
            "instantiation" => language::ArgumentKind::Instantiation,
            "source_kind" => language::ArgumentKind::SourceKind,
            _ => {
                return Err(zirql::QueryError::query(
                    name.range,
                    "unknown native predicate",
                ));
            }
        };
        reader.expect('(')?;
        reader.skip_whitespace();
        let first_end = self.guard_argument_end(reader.position());
        let explicit = reader.source().as_bytes().get(first_end) == Some(&b',');
        let literal = if explicit {
            reader.source()[first_end + 1..]
                .trim_start()
                .starts_with('"')
        } else {
            reader.remaining().starts_with('"')
        };
        if !literal {
            let operand = if explicit {
                let operand =
                    zirql::read_native_expression(reader, self, budget, self.reader_nesting)?;
                reader.expect(',')?;
                operand
            } else {
                Located::new(ExpressionKind::Input, name.range.clone())
            };
            reader.skip_whitespace();
            let start = reader.position();
            let end = self.guard_argument_end(start);
            std::mem::swap(&mut self.preflight, budget);
            let value = self
                .read_value(start..end, self.reader_nesting)
                .map_err(query_error);
            std::mem::swap(&mut self.preflight, budget);
            let value = value?;
            reader.advance(end - reader.position())?;
            reader.expect(')')?;
            let range = name.range.start..reader.position();
            let placeholder = match argument {
                language::ArgumentKind::Kind => language::PredicateKind::Kind("declaration".into()),
                language::ArgumentKind::Type => language::PredicateKind::Type("int".into()),
                language::ArgumentKind::Instantiation => {
                    language::PredicateKind::Instantiation("par".into())
                }
                language::ArgumentKind::SourceKind => {
                    language::PredicateKind::SourceKind("model".into())
                }
                _ => language::PredicateKind::Name(String::new()),
            };
            let stage = language::Stage {
                kind: language::StageKind::Filter(language::Predicate {
                    kind: placeholder,
                    range: range.clone(),
                }),
                range: range.clone(),
            };
            if semantic::requests_facts(&stage) {
                self.register(stage.clone());
            }
            return Ok(Guard {
                boolean: None,
                operand: None,
                prepared: Some(Located::new(
                    zirql::ValueExpressionKind::NativeValue(Box::new(operand)),
                    range,
                )),
                argument: Some((argument, value)),
                stage,
            });
        }
        let operand = if reader.remaining().starts_with('"') {
            Located::new(ExpressionKind::Input, name.range.clone())
        } else {
            let operand = zirql::read_native_expression(reader, self, budget, self.reader_nesting)?;
            if let Err(error) = reader.expect(',') {
                zirql::dispose_expression(operand);
                return Err(error);
            }
            operand
        };
        let mut guard = Guard {
            boolean: None,
            operand: Some(operand),
            prepared: None,
            argument: None,
            stage: language::Stage {
                kind: language::StageKind::Filter(language::Predicate {
                    kind: language::PredicateKind::Name(String::new()),
                    range: name.range.clone(),
                }),
                range: name.range.clone(),
            },
        };
        self.cursor.position = reader.position();
        let (value, value_range) = self.cursor.string().map_err(query_error)?;
        reader.advance(self.cursor.position - reader.position())?;
        reader.expect(')')?;
        budget.charge(value.len() + 1, zirql::Location::Query(value_range.clone()))?;
        let predicate = language::Predicate {
            kind: argument
                .predicate(value, value_range)
                .map_err(query_error)?,
            range: name.range.start..reader.position(),
        };
        guard.stage = language::Stage {
            range: predicate.range.clone(),
            kind: language::StageKind::Filter(predicate),
        };
        // Register semantic demand before the program prepares its one context.
        if semantic::requests_facts(&guard.stage) {
            budget.charge(
                guard.stage.range.len() + 1,
                zirql::Location::Query(guard.stage.range.clone()),
            )?;
            budget.charge_results(1, zirql::Location::Query(guard.stage.range.clone()))?;
            self.register(guard.stage.clone());
        }
        Ok(guard)
    }
    fn is_constructed_binding(&self, name: &str) -> bool {
        matches!(self.names.get(name), Some(Kind::Value(_) | Kind::Scalar(_)))
    }
    fn is_predicate_leaf(&self, name: &str) -> bool {
        matches!(
            name,
            "kind" | "name" | "type" | "instantiation" | "source_kind"
        )
    }
    fn predicate_expression(
        &mut self,
        predicate: zirql::PredicateArgument<Expression>,
        range: Range<usize>,
    ) -> Result<Guard, zirql::QueryError> {
        let input = Located::new(
            zirql::ValueExpressionKind::NativeValue(Box::new(Located::new(
                ExpressionKind::Input,
                range.clone(),
            ))),
            range.clone(),
        );
        Ok(Guard {
            boolean: Some(predicate.bind(input)),
            operand: None,
            prepared: None,
            argument: None,
            stage: language::Stage {
                kind: language::StageKind::Names,
                range,
            },
        })
    }
    fn parse_predicate_leaf_in_argument(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        argument: Option<&zirql::Pattern>,
        locals: &[&str],
        budget: &mut Budget,
        nesting: usize,
    ) -> Result<Guard, zirql::QueryError> {
        let mut guard = self.parse_predicate_leaf_in_scope(reader, locals, budget, nesting)?;
        if argument.is_some() {
            let operand = guard.prepared.take().unwrap_or_else(|| {
                let query = guard.operand.take().expect("native guard operand");
                Located::new(
                    zirql::ValueExpressionKind::NativeValue(Box::new(query)),
                    guard.stage.range.clone(),
                )
            });
            let range = operand.range.clone();
            let zirql::ValueExpressionKind::NativeValue(query) = operand.value else {
                unreachable!("native operand before header binding")
            };
            let body = Located::new(
                zirql::ValueExpressionKind::NativeValue(query),
                range.clone(),
            );
            let pattern = argument
                .map(|pattern| zirql::copy_pattern(pattern, budget))
                .transpose()?;
            guard.prepared = Some(
                zirql::PredicateArgument {
                    argument: pattern,
                    body: Box::new(body),
                }
                .bind_value(Located::new(
                    zirql::ValueExpressionKind::NativeValue(Box::new(Located::new(
                        ExpressionKind::Input,
                        range.clone(),
                    ))),
                    range,
                )),
            );
        }
        if let Some((_, value)) = &mut guard.argument {
            let range = value.range.clone();
            let body = std::mem::replace(
                value,
                Located::new(
                    zirql::ValueExpressionKind::Literal(zirql::ConstructedValue::Absent),
                    range.clone(),
                ),
            );
            let pattern = argument
                .map(|pattern| zirql::copy_pattern(pattern, budget))
                .transpose()?;
            *value = zirql::PredicateArgument {
                argument: pattern,
                body: Box::new(body),
            }
            .bind_value(Located::new(
                zirql::ValueExpressionKind::NativeValue(Box::new(Located::new(
                    ExpressionKind::Input,
                    range.clone(),
                ))),
                range,
            ));
        }
        Ok(guard)
    }
    fn parse_domain_call(
        &mut self,
        name: Located<&str>,
        reader: &mut zirql::ArgumentReader<'_>,
        _: &[&str],
        budget: &mut Budget,
        _: usize,
    ) -> Result<Option<zirql::ValueExpression<Expression>>, zirql::QueryError> {
        let Some(member) = comprehension::read_member(name, reader, budget)? else {
            return Ok(None);
        };
        self.report_model_demand = true;
        let range = member.range.clone();
        Ok(Some(Located::new(
            zirql::ValueExpressionKind::NativeValue(Box::new(value_stage(
                Stage::Member(member),
                range.clone(),
            ))),
            range,
        )))
    }
    fn filter(&mut self, predicate: zirql::Predicate<Guard>, _: Range<usize>) -> Stage {
        Stage::Filter(FilterStage::new(predicate))
    }
    fn projection(
        &mut self,
        projection: zirql::Projection<Expression>,
        _range: Range<usize>,
    ) -> Stage {
        Stage::Projection(ProjectionStage(Some(projection)))
    }

    fn matching(
        &mut self,
        mut matching: Matching,
        _: Range<usize>,
    ) -> Result<Stage, zirql::QueryError> {
        if self.prepared || !self.function_names.is_empty() {
            for clause in &mut matching.clauses {
                if let zirql::MatchClause::From { source, .. }
                | zirql::MatchClause::Bind { value: source, .. } = &mut clause.value
                {
                    let range = source.range.clone();
                    let original = std::mem::replace(
                        source,
                        Located::new(ExpressionKind::Input, range.clone()),
                    );
                    *source = value_stage(
                        Stage::FunctionSource(Located::new(
                            zirql::ValueExpressionKind::NativeValue(Box::new(original)),
                            range.clone(),
                        )),
                        range,
                    );
                }
            }
        }
        Ok(Stage::Match(MatchStage(Some(matching))))
    }
    fn where_exists(
        &mut self,
        query: Expression,
        _: Range<usize>,
    ) -> Result<Stage, zirql::QueryError> {
        self.prepared_values = true;
        Ok(Stage::WhereExists(native::BodyStage::new(query)))
    }
    fn fixed_point(
        &mut self,
        query: Expression,
        _: Range<usize>,
    ) -> Result<Stage, zirql::QueryError> {
        self.prepared_values = true;
        Ok(Stage::FixedPoint(native::BodyStage::new(query)))
    }
    fn ordering(
        &mut self,
        ordering: zirql::NativeOrdering<Expression>,
        _: Range<usize>,
    ) -> Result<Stage, zirql::QueryError> {
        self.prepared_values = true;
        Ok(Stage::Ordering(native::OrderingStage::new(ordering)))
    }
    fn mapping(
        &mut self,
        mapping: zirql::NativeMap<Expression>,
        _: Range<usize>,
    ) -> Result<Stage, zirql::QueryError> {
        self.prepared_values = true;
        Ok(Stage::Mapping(native::MappingStage::new(mapping)))
    }
    fn map_sorting(&mut self, _: Range<usize>) -> Result<Stage, zirql::QueryError> {
        self.prepared_values = true;
        Ok(Stage::MapSorting)
    }
    fn set_operator(&self, keyword: &str) -> Option<zirql::NativeSetOperator> {
        zirql::NativeSetOperator::from_keyword(keyword)
    }
    fn parse_path_edge(
        &mut self,
        edge: Located<&str>,
        budget: &mut Budget,
    ) -> Result<Stage, zirql::QueryError> {
        if edge.value != "referenced_declarations" {
            return Err(zirql::QueryError::query(
                edge.range,
                "unsupported path edge",
            ));
        }
        budget.charge_results(1, zirql::Location::Query(edge.range.clone()))?;
        let index = self.register(language::Stage {
            kind: language::StageKind::ReferencedDeclarations,
            range: edge.range,
        });
        Ok(Stage::Query(
            index,
            Rc::clone(&self.queries[index].arguments),
        ))
    }
}
pub(super) fn query_error(error: QueryError) -> zirql::QueryError {
    zirql::QueryError::query(error.range, error.message)
}

pub(super) struct CompositionHost<'h, 'q>(pub(super) &'h NativeHost<'q>);
impl<'a> ProgramHost<Context<'_, 'a>> for CompositionHost<'_, '_> {
    type Value = ReportValue<'a>;
    type Stage = Stage;
    type SetOperator = zirql::NativeSetOperator;
    type Emitter = Emitter;
    type Error = QueryError;
    fn copy_value(
        &self,
        value: &Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        self.0.copy_value(value, context, budget, range)
    }
    fn evaluate_stage(
        &self,
        stage: &Stage,
        value: Self::Value,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<Self::Value>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        self.0
            .evaluate_stage(stage, value, context, bindings, budget, range)
    }
    fn combine(
        &self,
        operator: &zirql::NativeSetOperator,
        left: Self::Value,
        right: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        self.0
            .combine(operator, left, right, context, budget, range)
    }
    fn construct_record(
        &self,
        fields: Vec<(String, Self::Value)>,
        _: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        if fields.iter().all(|(_, value)| {
            matches!(
                functions::unretained(value),
                ReportValue::Constructed(_) | ReportValue::SharedConstructed(_)
            )
        }) {
            let mut values = Vec::with_capacity(fields.len());
            for (name, value) in fields {
                let value =
                    functions::owned(value, budget, range).map_err(zirql::EvaluationError::Host)?;
                let value = match value {
                    ReportValue::Constructed(value) => value,
                    ReportValue::SharedConstructed(owner) => match Arc::try_unwrap(owner) {
                        Ok(value) => value,
                        Err(owner) => zirql::copy_constructed_value(
                            &owner,
                            budget,
                            zirql::Location::Query(range.clone()),
                        )?,
                    },
                    _ => unreachable!("constructed fields checked above"),
                };
                values.push((name, value));
            }
            Ok(ReportValue::Constructed(zirql::ConstructedValue::Record(
                values,
            )))
        } else {
            Ok(ReportValue::Capture(zirql::NativeCapture { fields }))
        }
    }
    fn tuple_view<'v>(&self, value: &'v Self::Value) -> Option<&'v [Self::Value]> {
        if let ReportValue::Tuple(value) = functions::unretained(value) {
            Some(value.values())
        } else {
            None
        }
    }
    fn positional_view<'v>(&self, value: &'v Self::Value) -> Option<&'v [Self::Value]> {
        if let ReportValue::Sequence(value) = functions::unretained(value) {
            Some(value.values())
        } else {
            None
        }
    }
    fn capture_record<'v>(
        &self,
        value: &'v Self::Value,
    ) -> Option<&'v zirql::NativeCapture<Self::Value>> {
        if let ReportValue::Capture(capture) = functions::unretained(value) {
            Some(capture)
        } else {
            None
        }
    }
    fn emit(
        &self,
        emitter: &Emitter,
        value: &Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::EvaluationError<QueryError>> {
        self.0.emit(emitter, value, context, budget, range)
    }
}
impl<'a> ProjectionHost<Context<'_, 'a>> for CompositionHost<'_, '_> {
    type Kind = Kind;
    fn value_arguments<'s>(
        &self,
        stage: &'s Stage,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::NativeValueArguments<'s, Context<'_, 'a>, Self>, zirql::QueryError> {
        match stage {
            Stage::WhereExists(body) | Stage::FixedPoint(body) => {
                return Ok(vec![(&body.expression, Kind::Nodes)]);
            }
            Stage::Mapping(mapping) => {
                return Ok(vec![
                    (&mapping.key, Kind::Nodes),
                    (&mapping.value, Kind::Nodes),
                ]);
            }
            Stage::Ordering(ordering) => {
                return Ok(ordering
                    .selector
                    .iter()
                    .map(|selector| (selector, Kind::Nodes))
                    .collect());
            }
            Stage::FunctionValue(value) | Stage::FunctionSource(value) => {
                return Ok(vec![(value, Kind::Nodes)]);
            }
            Stage::RecordValue(fields) => {
                return Ok(fields
                    .iter()
                    .map(|(_, value)| (value, Kind::Nodes))
                    .collect());
            }
            Stage::Projection(projection) => {
                return Ok(projection
                    .get()
                    .fields
                    .iter()
                    .map(|(_, expression)| (expression, Kind::Nodes))
                    .collect());
            }
            Stage::Match(matching) => {
                let mut values = Vec::new();
                for clause in &matching.get().clauses {
                    match &clause.value {
                        zirql::MatchClause::From { source, .. }
                        | zirql::MatchClause::Bind { value: source, .. } => {
                            if let Some(value) = source_value(source) {
                                values.push((value, Kind::Nodes));
                            }
                        }
                        zirql::MatchClause::Where { predicate } => {
                            for guard in guard_leaves(predicate) {
                                values.extend(guard.values().map(|value| (value, Kind::Nodes)));
                            }
                        }
                    }
                }
                return Ok(values);
            }
            Stage::Filter(predicate) => {
                return Ok(guard_leaves(predicate.get())
                    .into_iter()
                    .flat_map(|guard| guard.values().map(|value| (value, Kind::Nodes)))
                    .collect());
            }
            Stage::Paths(paths) => {
                return Ok(guard_leaves(&paths.get().target)
                    .into_iter()
                    .flat_map(|guard| guard.values().map(|value| (value, Kind::Declaration)))
                    .collect());
            }
            Stage::Primitive(_, values) => {
                return Ok(values.iter().map(|value| (value, Kind::Record)).collect());
            }
            _ => {}
        }
        let Stage::Query(_, arguments) = stage else {
            return Ok(Vec::new());
        };
        budget.charge(arguments.len(), zirql::Location::Query(range.clone()))?;
        budget.charge_results(arguments.len(), zirql::Location::Query(range.clone()))?;
        Ok(arguments
            .iter()
            .map(|argument| (&argument.expression, Kind::Record))
            .collect())
    }
    fn scoped_value_arguments<'s>(
        &self,
        stage: &'s Stage,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::ScopedNativeValueArguments<'s, Context<'_, 'a>, Self>, zirql::QueryError>
    {
        let relative = matches!(
            input,
            Kind::Nodes
                | Kind::Node
                | Kind::Declaration
                | Kind::Declarations
                | Kind::Entities
                | Kind::Capture(_)
                | Kind::Captures(_)
        )
        .then(|| input.clone());
        let mut result = Vec::new();
        match stage {
            Stage::WhereExists(body) | Stage::FixedPoint(body) => {
                result.push(zirql::ScopedNativeArgument {
                    expression: &body.expression,
                    relative: Some(if matches!(stage, Stage::WhereExists(_)) {
                        self.subject_kind(input, budget, range)?
                    } else {
                        input.clone()
                    }),
                    locals: Vec::new(),
                })
            }
            Stage::Mapping(mapping) => {
                let relative = Some(self.subject_kind(input, budget, range)?);
                for expression in [&mapping.key, &mapping.value] {
                    result.push(zirql::ScopedNativeArgument {
                        expression,
                        relative: relative.clone(),
                        locals: Vec::new(),
                    });
                }
            }
            Stage::Ordering(ordering) => {
                if let Some(expression) = &ordering.selector {
                    result.push(zirql::ScopedNativeArgument {
                        expression,
                        relative: Some(self.subject_kind(input, budget, range)?),
                        locals: Vec::new(),
                    });
                }
            }
            Stage::Projection(projection) => {
                let projection = projection.get();
                let subject = self.subject_kind(input, budget, range)?;
                for (_, expression) in &projection.fields {
                    result.push(zirql::ScopedNativeArgument {
                        expression,
                        relative: Some(subject.clone()),
                        locals: vec![(projection.subject.clone(), subject.clone())],
                    });
                }
            }
            Stage::Match(matching) => {
                let mut current = scope.clone();
                let mut locals = Vec::new();
                for clause in &matching.get().clauses {
                    match &clause.value {
                        zirql::MatchClause::From { name, source }
                        | zirql::MatchClause::Bind {
                            name,
                            value: source,
                        } => {
                            if let Some(expression) = source_value(source) {
                                result.push(zirql::ScopedNativeArgument {
                                    expression,
                                    relative: relative.clone(),
                                    locals: locals.clone(),
                                });
                            }
                            let kind = zirql::native_expression_kind(
                                source, self, input, &current, budget,
                            )?;
                            let kind = self.subject_kind(&kind, budget, &source.range)?;
                            current.insert(name.value.clone(), kind.clone());
                            locals.push((name.clone(), kind));
                        }
                        zirql::MatchClause::Where { predicate } => {
                            for guard in guard_leaves(predicate) {
                                for expression in guard.values() {
                                    result.push(zirql::ScopedNativeArgument {
                                        expression,
                                        relative: relative.clone(),
                                        locals: locals.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Stage::Filter(predicate) => {
                let subject = self.subject_kind(input, budget, range)?;
                for guard in guard_leaves(predicate.get()) {
                    for expression in guard.values() {
                        result.push(zirql::ScopedNativeArgument {
                            expression,
                            relative: Some(subject.clone()),
                            locals: Vec::new(),
                        });
                    }
                }
            }
            Stage::Paths(paths) => {
                for guard in guard_leaves(&paths.get().target) {
                    for expression in guard.values() {
                        result.push(zirql::ScopedNativeArgument {
                            expression,
                            relative: Some(Kind::Declaration),
                            locals: Vec::new(),
                        });
                    }
                }
            }
            Stage::RecordValue(fields) => {
                for (_, expression) in fields {
                    result.push(zirql::ScopedNativeArgument {
                        expression,
                        relative: Some(input.clone()),
                        locals: Vec::new(),
                    });
                }
            }
            Stage::FunctionValue(expression) => result.push(zirql::ScopedNativeArgument {
                expression,
                relative: Some(input.clone()),
                locals: Vec::new(),
            }),
            _ => {
                for (expression, kind) in self.value_arguments(stage, budget, range)? {
                    result.push(zirql::ScopedNativeArgument {
                        expression,
                        relative: (!self.is_explicit_source(stage)).then_some(kind),
                        locals: Vec::new(),
                    });
                }
            }
        }
        budget.charge(result.len(), zirql::Location::Query(range.clone()))?;
        budget.charge_results(result.len(), zirql::Location::Query(range.clone()))?;
        Ok(result)
    }
    fn evaluate_stage_with_arguments(
        &self,
        stage: &Stage,
        value: Self::Value,
        arguments: &mut dyn zirql::NativeArguments<Context<'_, 'a>, Self>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        let partial = native::selection_error(&value).cloned();
        if arguments.parts().0.query.complete_observations {
            native::require_complete(&value, budget, range)?;
        }
        let result = (|| {
            match stage {
                Stage::WhereExists(body) => {
                    return self.exists_with_arguments(body, value, arguments, budget, range);
                }
                Stage::FixedPoint(body) => {
                    return self.fixed_point_with_arguments(body, value, arguments, budget, range);
                }
                Stage::Mapping(mapping) => {
                    return self.map_with_arguments(mapping, value, arguments, budget, range);
                }
                Stage::MapSorting => {
                    return self.sort_map(value, arguments.parts().0, budget, range);
                }
                Stage::Ordering(ordering) => {
                    return self.order_with_arguments(ordering, value, arguments, budget, range);
                }
                Stage::Projection(projection) => {
                    return arguments
                        .evaluate_projection(projection.get(), value, budget)
                        .map(|result| ReportValue::Constructed(result.value));
                }
                Stage::Filter(predicate) => {
                    return self.guard_filter_with_arguments(
                        predicate.get(),
                        value,
                        arguments,
                        budget,
                        range,
                    );
                }
                Stage::Match(matching) => {
                    return self.composed_with_arguments(
                        Some(matching.get()),
                        None,
                        value,
                        arguments,
                        budget,
                        range,
                    );
                }
                Stage::Paths(paths) => {
                    return self.composed_with_arguments(
                        None,
                        Some(paths.get()),
                        value,
                        arguments,
                        budget,
                        range,
                    );
                }
                Stage::RecordValue(fields) => {
                    let mut values = Vec::with_capacity(fields.len());
                    for (name, expression) in fields {
                        let relative =
                            self.copy_value(&value, arguments.parts().0, budget, range)?;
                        let value = arguments.evaluate_native(expression, relative, budget)?;
                        budget.charge(
                            name.value.len() + 1,
                            zirql::Location::Query(name.range.clone()),
                        )?;
                        values.push((
                            name.value.clone(),
                            ReportValue::Retained(value.into_shared()),
                        ));
                    }
                    return self.construct_record(values, arguments.parts().0, budget, range);
                }
                Stage::FunctionValue(expression) | Stage::FunctionSource(expression) => {
                    let selected = arguments.evaluate_native(expression, value, budget)?;
                    return Ok(ReportValue::Retained(selected.into_shared()));
                }
                Stage::Primitive(primitive, values) => {
                    budget.charge_results(
                        values.len().saturating_mul(2),
                        zirql::Location::Query(range.clone()),
                    )?;
                    let mut selected = Vec::with_capacity(values.len());
                    for expression in values {
                        selected.push(arguments.evaluate_native(
                            expression,
                            value_placeholder(),
                            budget,
                        )?);
                    }
                    return functions::primitive(
                        *primitive,
                        &selected
                            .iter()
                            .map(|value| value.value())
                            .collect::<Vec<_>>(),
                        arguments.parts().0,
                        budget,
                        range,
                        self,
                    );
                }
                _ => {}
            }
            if let Stage::Query(index, bodies) = stage
                && !bodies.is_empty()
            {
                let query = self.0.resolve_query_by(
                    &self.0.queries[*index],
                    &mut |argument, budget| {
                        admit_argument_members(argument, arguments.parts().0, budget)?;
                        arguments
                            .evaluate(
                                &argument.expression,
                                ReportValue::Record(Vec::new()),
                                budget,
                            )?
                            .into_owned(budget)
                            .map_err(Into::into)
                    },
                    budget,
                    range,
                )?;
                let declarations =
                    self.0.functions && functions::query_declarations(&query, &value);
                let pipeline = pipeline::lower_pipeline(&query, budget)
                    .map_err(zirql::EvaluationError::Host)?;
                let (context, _) = arguments.parts();
                let result = self
                    .0
                    .execute_pipeline(&pipeline, value, context, budget, range)?;
                return self
                    .0
                    .retain_entity_contract(result, declarations, context, budget, range);
            }
            let (context, bindings) = arguments.parts();
            self.evaluate_stage(stage, value, context, bindings, budget, range)
        })();
        result.and_then(|value| native::incomplete(value, partial, budget, range))
    }
    fn kind(
        &self,
        value: &ReportValue<'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        Ok(match value {
            ReportValue::SourceCopies(value) => value.kind(),
            ReportValue::Computational(value) => Kind::Computational(Rc::new(
                value
                    .value
                    .declaration(budget, &zirql::Location::Query(range.clone()))?,
            )),
            ReportValue::Tuple(value) => Kind::Tuple(Rc::new(
                value
                    .values()
                    .iter()
                    .map(|v| self.kind(v, budget, range))
                    .collect::<Result<_, _>>()?,
            )),
            ReportValue::Sequence(value) => Kind::Sequence(Rc::clone(&value.element)),
            ReportValue::Retained(value) => return self.kind(value, budget, range),
            ReportValue::IncompleteSelection { value, .. } => {
                return self.kind(value, budget, range);
            }
            ReportValue::EntitySequence { declarations, .. } => {
                if *declarations {
                    Kind::Declarations
                } else {
                    Kind::Entities
                }
            }
            ReportValue::Item(value) => {
                return match self.kind(value, budget, range)? {
                    Kind::Nodes => Ok(Kind::Node),
                    Kind::Declarations => Ok(Kind::Declaration),
                    _ => Err(zirql::QueryError::query(
                        range.clone(),
                        "invalid native singleton",
                    )),
                };
            }
            ReportValue::SharedConstructed(value) => {
                Kind::Value(Rc::new(vec![comprehension::value_facts(
                    value, budget, range,
                )?]))
            }
            ReportValue::Constructed(value) => {
                Kind::Value(Rc::new(vec![comprehension::value_facts(
                    value, budget, range,
                )?]))
            }
            ReportValue::Record(fields) => {
                let location = zirql::Location::Query(range.clone());
                budget.charge(fields.len(), location.clone())?;
                budget.charge_results(fields.len(), location)?;
                Kind::Dictionary(Rc::new(
                    fields
                        .iter()
                        .map(|(_, value)| self.kind(value, budget, range))
                        .collect::<Result<_, _>>()?,
                ))
            }
            ReportValue::Capture(capture) => {
                let mut fields = Vec::new();
                for (name, value) in &capture.fields {
                    budget.charge(name.len() + 1, zirql::Location::Query(range.clone()))?;
                    fields.push((name.clone(), self.kind(value, budget, range)?));
                }
                Kind::Capture(Rc::new(zirql::NativeCapture { fields }))
            }
            ReportValue::Captures(captures) => {
                Kind::Captures(copy_schema(&captures.schema, budget, range)?)
            }
            ReportValue::Query(stream) => match stream {
                SemanticStream::Entities(rows) => {
                    budget.charge(rows.len(), zirql::Location::Query(range.clone()))?;
                    if rows
                        .iter()
                        .all(|entity| matches!(entity, semantic::SemanticEntity::Declaration(_)))
                    {
                        Kind::Declarations
                    } else {
                        Kind::Entities
                    }
                }
                SemanticStream::Types(_) => Kind::Types,
                SemanticStream::Instantiations(_) => Kind::Instantiations,
                SemanticStream::Inspection(result) => match result.as_ref() {
                    QueryResult::Selection(_) | QueryResult::Nodes(_) => Kind::Nodes,
                    QueryResult::Strings(_) => Kind::Strings,
                    QueryResult::Literals(_) => Kind::Literals,
                    QueryResult::Count(_) => Kind::Count,
                    QueryResult::Tally(_) => Kind::Tally,
                    _ => unreachable!("read-only native values"),
                },
            },
        })
    }
    fn scalar_types(
        &self,
        kind: &Kind,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Option<Vec<zirql::ValueType>>, zirql::QueryError> {
        use zirql::ValueType as T;
        budget.charge(1, zirql::Location::Query(range.clone()))?;
        Ok(match kind {
            Kind::Value(types) => Some(zirql::copy_value_types(
                types,
                budget,
                zirql::Location::Query(range.clone()),
            )?),
            Kind::Scalar(ty) => Some(vec![(*ty).into()]),
            Kind::Constructed => Some(vec![T::Unestablished]),
            Kind::Count => Some(vec![T::Integer]),
            Kind::Strings => Some(vec![T::Array(zirql::ArrayType::Elements(vec![T::String]))]),
            Kind::Literals => Some(vec![T::Array(zirql::ArrayType::Elements(vec![
                T::Unestablished,
            ]))]),
            _ => None,
        })
    }
    fn constructed_kind(&self, types: Vec<zirql::ValueType>) -> Kind {
        Kind::Value(Rc::new(types))
    }
    fn stage_kind(
        &self,
        stage: &Stage,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        match stage {
            Stage::Filter(predicate) => {
                let subject = self.subject_kind(input, budget, range)?;
                for guard in guard_leaves(predicate.get()) {
                    self.validate_guard_leaf(guard, &subject, scope, budget, range)?;
                }
                Ok(input.clone())
            }
            Stage::WhereExists(body) => self.body_kind(body, input, scope, budget, range, true),
            Stage::FixedPoint(body) => self.body_kind(body, input, scope, budget, range, false),
            Stage::Ordering(ordering) => self.order_kind(ordering, input, scope, budget, range),
            Stage::Mapping(mapping) => self.map_kind(mapping, input, scope, budget, range),
            Stage::MapSorting => native::map_sort_kind(input, range),
            Stage::RetainPayload => Ok(Kind::Payload(Rc::new(input.clone()))),
            Stage::RecordValue(fields) => {
                let mut kinds = Vec::with_capacity(fields.len());
                let mut scalar_fields = Some(Vec::with_capacity(fields.len()));
                for (name, value) in fields {
                    let kind = zirql::pattern_function_value_kind_borrowed(
                        self.0
                            .declarations
                            .iter()
                            .zip(self.0.signatures)
                            .map(|(body, signature)| body.with_signature(signature)),
                        self,
                        value,
                        scope,
                        Some(input.clone()),
                        budget,
                    )?;
                    let kind = match kind {
                        Kind::Payload(kind) => (*kind).clone(),
                        kind => kind,
                    };
                    match self.scalar_types(&kind, budget, &value.range)? {
                        Some(types) => {
                            if let Some(fields) = &mut scalar_fields {
                                fields.push((name, types));
                            }
                        }
                        None => scalar_fields = None,
                    }
                    kinds.push((name.value.clone(), kind));
                }
                let Some(fields) = scalar_fields else {
                    return Ok(Kind::Capture(Rc::new(zirql::NativeCapture {
                        fields: kinds,
                    })));
                };
                let location = zirql::Location::Query(range.clone());
                let mut records = vec![zirql::ValueType::Record(Vec::new())];
                for (name, types) in fields {
                    let mut next = Vec::new();
                    for record in &records {
                        for ty in &types {
                            budget.charge(name.value.len() + 1, location.clone())?;
                            budget.charge_results(1, location.clone())?;
                            let mut copied = zirql::copy_value_types(
                                std::slice::from_ref(record),
                                budget,
                                location.clone(),
                            )?;
                            let zirql::ValueType::Record(mut fields) = copied.pop().unwrap() else {
                                unreachable!()
                            };
                            let ty = zirql::copy_value_types(
                                std::slice::from_ref(ty),
                                budget,
                                location.clone(),
                            )?
                            .pop()
                            .unwrap();
                            fields.push((name.value.clone(), ty));
                            next.push(zirql::ValueType::Record(fields));
                        }
                    }
                    records = next;
                }
                Ok(Kind::Value(Rc::new(records)))
            }
            Stage::FunctionValue(value) | Stage::FunctionSource(value) => {
                functions::value_kind(value, self, input, scope, budget)
            }
            Stage::Primitive(primitive, values) => {
                let expected = if matches!(
                    primitive,
                    functions::Primitive::SameDeclaration | functions::Primitive::ReferenceInfo
                ) {
                    Kind::Declaration
                } else {
                    Kind::Node
                };
                let arity = if matches!(primitive, functions::Primitive::SameDeclaration) {
                    2
                } else {
                    1
                };
                if values.len() != arity {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "native primitive argument count mismatch",
                    ));
                }
                for value in values {
                    let kind = functions::value_kind(value, self, input, scope, budget)?;
                    let unknown = matches!(kind, Kind::Constructed)
                        || matches!(&kind, Kind::Value(types) if types.iter().all(|ty| matches!(ty, zirql::ValueType::Unestablished)));
                    let constructed = matches!(
                        value.value,
                        zirql::ValueExpressionKind::Literal(_)
                            | zirql::ValueExpressionKind::Array(_)
                            | zirql::ValueExpressionKind::Tuple(_)
                            | zirql::ValueExpressionKind::Record(_)
                            | zirql::ValueExpressionKind::Dictionary(_)
                            | zirql::ValueExpressionKind::DictionaryComprehension { .. }
                            | zirql::ValueExpressionKind::Set(_)
                            | zirql::ValueExpressionKind::Unary { .. }
                            | zirql::ValueExpressionKind::Binary { .. }
                            | zirql::ValueExpressionKind::KeyedArray { .. }
                            | zirql::ValueExpressionKind::KeyedComprehension { .. }
                            | zirql::ValueExpressionKind::Comprehension { .. }
                    );
                    if kind != expected && (!unknown || constructed) {
                        return Err(zirql::QueryError::query(
                            value.range.clone(),
                            "native primitive argument type mismatch",
                        ));
                    }
                }
                Ok(match primitive {
                    functions::Primitive::DirectChildren => Kind::Nodes,
                    functions::Primitive::NodeKind => Kind::Scalar(zirql::ParameterType::String),
                    functions::Primitive::WrittenCallName => {
                        Kind::Value(Rc::new(vec![zirql::ValueType::Unestablished]))
                    }
                    functions::Primitive::SameDeclaration => {
                        Kind::Scalar(zirql::ParameterType::Bool)
                    }
                    functions::Primitive::ReferenceInfo => {
                        Kind::Capture(Rc::new(zirql::NativeCapture {
                            fields: vec![("targets".to_owned(), Kind::Declarations)],
                        }))
                    }
                })
            }
            Stage::Copy(stage) => {
                if !matches!(input, Kind::Literals) {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "source copying requires a native literal stream",
                    ));
                }
                Ok(if matches!(stage, source_copy::CopyStage::Members(..)) {
                    Kind::MemberCopies
                } else {
                    Kind::SourceCopies
                })
            }
            Stage::Parameter(index) => Ok(Kind::Scalar(self.0.parameters[*index].ty.value)),
            Stage::Member(_) => Ok(Kind::Value(Rc::new(vec![zirql::ValueType::Unestablished]))),
            Stage::Query(index, arguments) => {
                for argument in arguments.iter() {
                    functions::check_argument(
                        &argument.expression,
                        argument.kind,
                        self,
                        input,
                        scope,
                        budget,
                    )?;
                }
                validate_stages(&self.0.queries[*index].query, input.clone()).map_err(query_error)
            }
            Stage::Projection(projection) => {
                for name in scope.keys() {
                    budget.charge(name.len() + 1, zirql::Location::Query(range.clone()))?;
                }
                if self.0.functions {
                    let projection = projection.get();
                    let range = &projection.subject.range;
                    if scope.contains_key(&projection.subject.value) {
                        return Err(zirql::QueryError::query(
                            range.clone(),
                            "projection subject shadows an existing binding",
                        ));
                    }
                    let subject = self.subject_kind(input, budget, range)?;
                    let mut scope = scope.clone();
                    budget.charge(
                        projection.subject.value.len() + 1,
                        zirql::Location::Query(range.clone()),
                    )?;
                    scope.insert(projection.subject.value.clone(), subject.clone());
                    let mut facts = BTreeMap::new();
                    let mut fields = Vec::new();
                    budget.charge_results(
                        projection.fields.len(),
                        zirql::Location::Query(range.clone()),
                    )?;
                    for (label, expression) in &projection.fields {
                        let kind =
                            functions::value_kind(expression, self, &subject, &scope, budget)?;
                        let types = self
                            .scalar_types(&kind, budget, &expression.range)?
                            .ok_or_else(|| {
                                zirql::QueryError::query(
                                    expression.range.clone(),
                                    "native handles require an explicit scalar projection",
                                )
                            })?;
                        budget.charge(
                            label.value.len().saturating_mul(3) + 1,
                            zirql::Location::Query(label.range.clone()),
                        )?;
                        facts.insert(label.value.clone(), types);
                        fields.push((
                            label.clone(),
                            Located::new(
                                zirql::ValueExpressionKind::<()>::Name(label.value.clone()),
                                expression.range.clone(),
                            ),
                        ));
                    }
                    // Shared record analysis retains every field-type combination, including empty reports.
                    let record =
                        Located::new(zirql::ValueExpressionKind::Record(fields), range.clone());
                    let records =
                        zirql::query_value_types(&record, &facts, &mut ConstructedTypes, budget)?;
                    return Ok(Kind::Value(Rc::new(vec![zirql::ValueType::Array(
                        zirql::ArrayType::Elements(records),
                    )])));
                }
                zirql::projection_types(
                    projection.get(),
                    self,
                    input.clone(),
                    scope.clone(),
                    budget,
                )
                .map(|types| Kind::Value(Rc::new(types)))
            }
            Stage::Match(matching) => {
                zirql::match_types(matching.get(), self, input, scope, budget)
                    .map(|schema| Kind::Captures(Rc::new(schema)))
            }
            Stage::Paths(paths) => zirql::path_types(paths.get(), self, input, scope, budget)
                .map(|schema| Kind::Captures(Rc::new(schema))),
            _ => Err(zirql::QueryError::query(
                range.clone(),
                "stage is unavailable in native composition",
            )),
        }
    }
    fn set_kind(
        &self,
        left: &Kind,
        right: &Kind,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        let left = native::set_kind(left, range)?;
        let right = native::set_kind(right, range)?;
        if left != right {
            return Err(zirql::QueryError::query(
                range.clone(),
                "set operands require the same native kind",
            ));
        }
        Ok(left)
    }
    fn is_explicit_source(&self, stage: &Stage) -> bool {
        match stage {
            Stage::FunctionSource(_)
            | Stage::RecordValue(_)
            | Stage::Primitive(..)
            | Stage::Member(_) => true,
            Stage::Match(matching) => {
                matching
                    .get()
                    .clauses
                    .iter()
                    .all(|clause| match &clause.value {
                        zirql::MatchClause::From { source, .. }
                        | zirql::MatchClause::Bind { value: source, .. } => {
                            explicit_expression(source)
                        }
                        zirql::MatchClause::Where { predicate } => {
                            guard_leaves(predicate).into_iter().all(|guard| {
                                guard.boolean.is_none()
                                    && guard.prepared.is_none()
                                    && explicit_expression(guard.operand())
                            })
                        }
                    })
            }
            _ => false,
        }
    }
    fn is_pure(&self, stage: &Stage) -> bool {
        match stage {
            Stage::Data(..) => false,
            Stage::Query(index, _) => {
                !self.0.queries[*index].query.is_editing()
                    && !self.0.queries[*index].query.stages.iter().any(|s| {
                        matches!(
                            s.kind,
                            language::StageKind::Emit
                                | language::StageKind::Json
                                | language::StageKind::EmitDocument
                        )
                    })
            }
            _ => true,
        }
    }
    fn nested_queries(
        &self,
        _: &Stage,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<zirql::NativeOperands<Context<'_, 'a>, Self>, zirql::QueryError> {
        Ok(Vec::new())
    }
    fn subjects(
        &self,
        input: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<Self::Value>, zirql::EvaluationError<QueryError>> {
        let owner = worklists::owner(&input).cloned();
        let entities = functions::entity_contract(&input);
        let input = functions::owned(input, budget, range).map_err(zirql::EvaluationError::Host)?;
        if let ReportValue::Sequence(sequence) = input {
            budget.check_items(
                sequence.values().len(),
                zirql::Location::Query(range.clone()),
            )?;
            budget.charge_results(
                sequence.values().len(),
                zirql::Location::Query(range.clone()),
            )?;
            return sequence
                .values()
                .iter()
                .map(|value| self.copy_value(value, context, budget, range))
                .collect();
        }
        if let ReportValue::Captures(captures) = input {
            budget.charge(captures.rows.len(), zirql::Location::Query(range.clone()))?;
            budget.charge_results(captures.rows.len(), zirql::Location::Query(range.clone()))?;
            return Ok(captures
                .rows
                .into_iter()
                .map(ReportValue::Capture)
                .collect());
        }
        if matches!(input, ReportValue::Capture(_)) {
            budget.charge(1, zirql::Location::Query(range.clone()))?;
            budget.charge_results(1, zirql::Location::Query(range.clone()))?;
            return Ok(vec![input]);
        }
        let ReportValue::Query(stream) = input else {
            return Err(zirql::QueryError::query(
                range.clone(),
                "native subjects require a selection",
            )
            .into());
        };
        let len = stream_len(&stream);
        budget.check_items(len, zirql::Location::Query(range.clone()))?;
        let units = len.checked_mul(4).ok_or_else(|| {
            zirql::QueryError::query(range.clone(), "subject allocation size overflow")
        })?;
        budget.charge(units, zirql::Location::Query(range.clone()))?;
        budget.charge_results(units, zirql::Location::Query(range.clone()))?;
        let rows = context.query.take_rows(stream);
        context.query.document = false;
        Ok(rows
            .into_iter()
            .map(|row| {
                let value = ReportValue::Query(context.query.restore_stream(vec![row]));
                let value = if let Some(declarations) = entities {
                    ReportValue::EntitySequence {
                        declarations,
                        owner: owner.clone(),
                        value: Box::new(value),
                    }
                } else {
                    value
                };
                if self.0.functions {
                    ReportValue::Item(Box::new(value))
                } else {
                    value
                }
            })
            .collect())
    }
    fn subject_kind(
        &self,
        input: &Kind,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        if !self.0.functions && matches!(input, Kind::Nodes | Kind::Declarations) {
            return Ok(input.clone());
        }
        if let Kind::Value(types) = input {
            let mut elements = Vec::new();
            for ty in types.iter() {
                let ty = if let zirql::ValueType::Declared { actual, .. } = ty {
                    actual.as_ref()
                } else {
                    ty
                };
                let values = match ty {
                    zirql::ValueType::Array(
                        zirql::ArrayType::Positions(values) | zirql::ArrayType::Elements(values),
                    )
                    | zirql::ValueType::ArrayKind(values) => values,
                    _ => {
                        return Err(zirql::QueryError::query(
                            range.clone(),
                            "filter needs a positional list or native selection",
                        ));
                    }
                };
                elements.extend(zirql::copy_value_types(
                    values,
                    budget,
                    zirql::Location::Query(range.clone()),
                )?);
            }
            if elements.is_empty() {
                elements.push(zirql::ValueType::Unestablished);
            }
            return Ok(Kind::Value(Rc::new(elements)));
        }
        match input {
            Kind::Sequence(element) => {
                use zirql::FunctionHost;
                self.declaration_kind(element, budget, range)
            }
            Kind::Nodes => Ok(Kind::Node),
            Kind::Declarations => Ok(Kind::Declaration),
            Kind::Node | Kind::Declaration => Ok(input.clone()),
            Kind::Captures(schema) => Ok(Kind::Capture(copy_schema(schema, budget, range)?)),
            Kind::Capture(_)
            | Kind::Entities
            | Kind::Types
            | Kind::Instantiations
            | Kind::Strings
            | Kind::Literals => Ok(input.clone()),
            _ => Err(zirql::QueryError::query(
                range.clone(),
                "native subjects require a selection",
            )),
        }
    }
    fn pattern_constructed<'v>(
        &self,
        value: &'v Self::Value,
    ) -> Option<&'v zirql::ConstructedValue> {
        match functions::unretained(value) {
            ReportValue::Constructed(value) => Some(value),
            ReportValue::SharedConstructed(value) => Some(value),
            _ => None,
        }
    }
    fn positional_view_kind(
        &self,
        kind: &Kind,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Option<Kind>, zirql::QueryError> {
        if let Kind::Sequence(element) = kind {
            use zirql::FunctionHost;
            self.declaration_kind(element, budget, range).map(Some)
        } else {
            Ok(None)
        }
    }
    fn tuple_view_kind<'k>(&self, kind: &'k Kind) -> Option<&'k [Kind]> {
        if let Kind::Tuple(values) = kind {
            Some(values)
        } else {
            None
        }
    }
    fn capture_record_kind<'k>(&self, kind: &'k Kind) -> Option<&'k zirql::NativeCapture<Kind>> {
        if let Kind::Capture(schema) = kind {
            Some(schema)
        } else {
            None
        }
    }
    fn project_value(
        &self,
        value: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::ConstructedValue, zirql::EvaluationError<QueryError>> {
        let value = functions::owned(value, budget, range).map_err(zirql::EvaluationError::Host)?;
        let value = match value {
            ReportValue::Constructed(value) => value,
            ReportValue::SharedConstructed(value) => zirql::copy_constructed_value(
                &value,
                budget,
                zirql::Location::Query(range.clone()),
            )?,
            ReportValue::Query(stream) => {
                let facts = if matches!(&stream, SemanticStream::Inspection(value) if matches!(value.as_ref(), QueryResult::Literals(_)))
                {
                    context
                        .report_facts(budget, range)
                        .map_err(zirql::EvaluationError::Host)?
                } else {
                    context.facts.clone()
                };
                comprehension::project(&stream, facts.as_deref(), budget, range, self.0.nesting)
                    .map_err(zirql::EvaluationError::Host)?
            }
            _ => {
                return Err(zirql::QueryError::query(
                    range.clone(),
                    "native handles require an explicit scalar projection",
                )
                .into());
            }
        };
        if self.0.editing {
            comprehension::validate_data_shape(&value, budget, range)
        } else {
            comprehension::validate_report_shape(&value, budget, range)
        }
        .map_err(zirql::EvaluationError::Host)?;
        Ok(value)
    }
    fn share_constructed(
        &self,
        value: &Self::Value,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<Option<Arc<zirql::ConstructedValue>>, zirql::EvaluationError<QueryError>> {
        Ok(match functions::unretained(value) {
            ReportValue::SharedConstructed(value) => Some(value.clone()),
            _ => None,
        })
    }
    fn lift_shared_constructed(
        &self,
        value: Arc<zirql::ConstructedValue>,
        _: Vec<zirql::ValueType>,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        Ok(ReportValue::SharedConstructed(value))
    }
    fn lift_constructed(
        &self,
        value: zirql::ConstructedValue,
        _: Vec<zirql::ValueType>,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        Ok(if self.0.functions {
            ReportValue::SharedConstructed(Arc::new(value))
        } else {
            ReportValue::Constructed(value)
        })
    }
}
pub(super) fn stream_len(stream: &SemanticStream<'_>) -> usize {
    match stream {
        SemanticStream::Entities(rows) => rows.len(),
        SemanticStream::Types(rows) => rows.len(),
        SemanticStream::Instantiations(rows) => rows.len(),
        SemanticStream::Inspection(result) => match result.as_ref() {
            QueryResult::Selection(rows) => rows.items.len(),
            QueryResult::Nodes(rows) => rows.nodes.len(),
            QueryResult::Strings(rows) => rows.len(),
            QueryResult::Literals(rows) => rows.values.len(),
            _ => 0,
        },
    }
}
pub(super) fn copy_schema(
    schema: &zirql::NativeCapture<Kind>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Rc<zirql::NativeCapture<Kind>>, zirql::QueryError> {
    budget.charge(schema.fields.len(), zirql::Location::Query(range.clone()))?;
    budget.charge_results(schema.fields.len(), zirql::Location::Query(range.clone()))?;
    let mut fields = Vec::with_capacity(schema.fields.len());
    for (name, kind) in &schema.fields {
        budget.charge(name.len() + 1, zirql::Location::Query(range.clone()))?;
        let kind = match kind {
            Kind::Captures(schema) => Kind::Captures(copy_schema(schema, budget, range)?),
            Kind::Capture(schema) => Kind::Capture(copy_schema(schema, budget, range)?),
            Kind::Value(types) => Kind::Value(Rc::new(zirql::copy_value_types(
                types,
                budget,
                zirql::Location::Query(range.clone()),
            )?)),
            kind => kind.clone(),
        };
        fields.push((name.clone(), kind));
    }
    Ok(Rc::new(zirql::NativeCapture { fields }))
}
impl<'a> MatchHost<Context<'_, 'a>> for CompositionHost<'_, '_> {
    type Guard = Guard;
    fn validate_guard_leaf(
        &self,
        guard: &Guard,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::QueryError> {
        if guard.boolean.is_some() {
            return Ok(());
        }
        if let Some((kind, expression)) = &guard.argument {
            functions::check_argument(expression, *kind, self, input, scope, budget)?;
        }
        let kind = if let Some(operand) = &guard.prepared {
            functions::value_kind(operand, self, input, scope, budget)?
        } else {
            zirql::native_expression_kind(guard.operand(), self, input, scope, budget)?
        };
        guard_subject_kind(&kind, range)?;
        Ok(())
    }
    fn evaluate_guard_leaf(
        &self,
        guard: &Guard,
        input: &Self::Value,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<Self::Value>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        if let Some(expression) = &guard.boolean {
            let subject = self.copy_value(input, context, budget, range)?;
            let session = context.functions.ok_or_else(|| {
                zirql::QueryError::query(range.clone(), "prepared guard session is unavailable")
            })?;
            let selected = session
                .run(expression, bindings, &subject, context, budget)
                .map_err(zirql::EvaluationError::Host)?;
            let value = self.project_value(selected, context, budget, &expression.range)?;
            return boolean_truth(value, &expression.range);
        }
        let relative = self.copy_value(input, context, budget, range)?;
        let value = if let Some(operand) = &guard.prepared {
            let session = context.functions.ok_or_else(|| {
                zirql::QueryError::query(range.clone(), "prepared guard session is unavailable")
            })?;
            session
                .run(operand, bindings, &relative, context, budget)
                .map_err(zirql::EvaluationError::Host)?
        } else {
            zirql::evaluate_expression(guard.operand(), self, relative, context, bindings, budget)?
        };
        if let Some((kind, expression)) = &guard.argument {
            let session = context.functions.ok_or_else(|| {
                zirql::QueryError::query(range.clone(), "prepared guard session is unavailable")
            })?;
            let selected = session
                .run(expression, bindings, input, context, budget)
                .map_err(zirql::EvaluationError::Host)?;
            let text = self.project_value(selected, context, budget, &expression.range)?;
            let zirql::ConstructedValue::String(text) = text.into_data() else {
                return Err(zirql::QueryError::query(
                    expression.range.clone(),
                    "predicate argument needs a string",
                )
                .into());
            };
            let predicate = language::Predicate {
                kind: kind
                    .predicate(text, expression.range.clone())
                    .map_err(zirql::EvaluationError::Host)?,
                range: guard.stage.range.clone(),
            };
            let stage = language::Stage {
                kind: language::StageKind::Filter(predicate.clone()),
                range: guard.stage.range.clone(),
            };
            self.guard_value(value, &predicate, &stage, context, budget, range)
        } else {
            self.guard_value(
                value,
                guard.predicate(),
                &guard.stage,
                context,
                budget,
                range,
            )
        }
    }

    fn evaluate_match_source_with_arguments(
        &self,
        source: &Expression,
        input: Self::Value,
        bindings: &Bindings<Self::Value>,
        arguments: &mut dyn zirql::NativeArguments<Context<'_, 'a>, Self>,
        budget: &mut Budget,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        if let Some(value) = source_value(source) {
            return Ok(ReportValue::Retained(
                arguments
                    .evaluate_match_value(value, input, bindings, budget)?
                    .into_shared(),
            ));
        }
        zirql::evaluate_expression(source, self, input, arguments.parts().0, bindings, budget)
    }
    fn evaluate_guard_leaf_with_arguments(
        &self,
        guard: &Guard,
        input: &Self::Value,
        bindings: &Bindings<Self::Value>,
        arguments: &mut dyn zirql::NativeArguments<Context<'_, 'a>, Self>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        self.guard_with_arguments(guard, input, Some(bindings), arguments, budget, range)
    }
}
pub(super) fn guard_leaves(predicate: &zirql::Predicate<Guard>) -> Vec<&Guard> {
    let mut pending = vec![predicate];
    let mut result = Vec::new();
    while let Some(predicate) = pending.pop() {
        match &predicate.value {
            zirql::PredicateKind::Leaf(guard) => result.push(guard),
            zirql::PredicateKind::Not(child) | zirql::PredicateKind::Group(child) => {
                pending.push(child)
            }
            zirql::PredicateKind::And(children) | zirql::PredicateKind::Or(children) => {
                pending.extend(children)
            }
        }
    }
    result
}
fn explicit_expression(expression: &Expression) -> bool {
    match &expression.value {
        ExpressionKind::Binding(_) => true,
        ExpressionKind::Group(value) | ExpressionKind::Field { value, .. } => {
            explicit_expression(value)
        }
        ExpressionKind::Sequence { input, steps } => {
            if let Some(step) = steps.first()
                && let ExpressionStep::Stage(Stage::FunctionSource(value)) = &step.value
            {
                if let zirql::ValueExpressionKind::NativeValue(query) = &value.value {
                    return explicit_expression(query);
                }
                return true;
            }
            explicit_expression(input)
        }
        _ => false,
    }
}
fn source_value(source: &Expression) -> Option<&zirql::ValueExpression<Expression>> {
    if let ExpressionKind::Sequence { steps, .. } = &source.value
        && let [step] = steps.as_slice()
        && let ExpressionStep::Stage(Stage::FunctionSource(value)) = &step.value
    {
        Some(value)
    } else {
        None
    }
}
impl<'a> CompositionHost<'_, '_> {
    pub(super) fn guard_with_arguments(
        &self,
        guard: &Guard,
        input: &ReportValue<'a>,
        captures: Option<&Bindings<ReportValue<'a>>>,
        arguments: &mut dyn zirql::NativeArguments<Context<'_, 'a>, Self>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        if let Some(expression) = &guard.boolean {
            let subject = self.copy_value(input, arguments.parts().0, budget, range)?;
            return arguments
                .evaluate_predicate(expression, subject, captures, budget)
                .map(|keep| {
                    if keep {
                        zirql::Truth::True
                    } else {
                        zirql::Truth::False
                    }
                });
        }
        let relative = self.copy_value(input, arguments.parts().0, budget, range)?;
        let value = if let Some(operand) = &guard.prepared {
            let value = if let Some(captures) = captures {
                arguments.evaluate_match_value(operand, relative, captures, budget)?
            } else {
                arguments.evaluate_native(operand, relative, budget)?
            };
            ReportValue::Retained(value.into_shared())
        } else {
            let (context, bindings) = arguments.parts();
            zirql::evaluate_expression(
                guard.operand(),
                self,
                relative,
                context,
                captures.unwrap_or(bindings),
                budget,
            )?
        };
        if let Some((kind, expression)) = &guard.argument {
            let subject = self.copy_value(input, arguments.parts().0, budget, range)?;
            let selected = if let Some(captures) = captures {
                arguments.evaluate_match_value(expression, subject, captures, budget)?
            } else {
                arguments.evaluate_native(expression, subject, budget)?
            };
            let context = arguments.parts().0;
            let text = self.project_value(
                ReportValue::Retained(selected.into_shared()),
                context,
                budget,
                &expression.range,
            )?;
            let zirql::ConstructedValue::String(text) = text.into_data() else {
                return Err(zirql::QueryError::query(
                    expression.range.clone(),
                    "predicate argument needs a string",
                )
                .into());
            };
            let predicate = language::Predicate {
                kind: kind
                    .predicate(text, expression.range.clone())
                    .map_err(zirql::EvaluationError::Host)?,
                range: guard.stage.range.clone(),
            };
            let stage = language::Stage {
                kind: language::StageKind::Filter(predicate.clone()),
                range: guard.stage.range.clone(),
            };
            self.guard_value(value, &predicate, &stage, context, budget, range)
        } else {
            self.guard_value(
                value,
                guard.predicate(),
                &guard.stage,
                arguments.parts().0,
                budget,
                range,
            )
        }
    }
    fn composed_with_arguments(
        &self,
        matching: Option<&Matching>,
        paths: Option<&Paths>,
        input: ReportValue<'a>,
        arguments: &mut dyn zirql::NativeArguments<Context<'_, 'a>, Self>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        let kind = self.kind(&input, budget, range)?;
        let mut scope = BTreeMap::new();
        for (name, value) in arguments.parts().1.iter() {
            scope.insert(name.to_owned(), self.kind(value, budget, range)?);
        }
        let schema = if let Some(matching) = matching {
            zirql::match_types(matching, self, &kind, &scope, budget)?
        } else {
            zirql::path_types(paths.expect("paths"), self, &kind, &scope, budget)?
        };
        let previous = arguments.parts().0.guard_unknown.take();
        let result = if let Some(matching) = matching {
            zirql::evaluate_match_with_arguments(matching, self, input, arguments, budget)
        } else {
            zirql::evaluate_paths_with_arguments(
                paths.expect("paths"),
                self,
                input,
                arguments,
                budget,
            )
        };
        let responsible = std::mem::replace(&mut arguments.parts().0.guard_unknown, previous);
        let rows = result.map_err(|error| predicate_error(error, responsible))?;
        budget.charge(rows.len() + 1, zirql::Location::Query(range.clone()))?;
        budget.charge_results(rows.len() + 1, zirql::Location::Query(range.clone()))?;
        let rows = rows
            .into_iter()
            .map(|row| {
                let ReportValue::Capture(capture) = row else {
                    unreachable!("native capture")
                };
                capture
            })
            .collect();
        Ok(ReportValue::Captures(CaptureMatches {
            rows,
            schema: Rc::new(schema),
        }))
    }
    fn guard_value(
        &self,
        value: ReportValue<'a>,
        predicate: &language::Predicate,
        stage: &language::Stage,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        let value = functions::owned(value, budget, range).map_err(zirql::EvaluationError::Host)?;
        let ReportValue::Query(stream) = value else {
            return Err(zirql::QueryError::query(
                range.clone(),
                "semantic guard requires a native subject",
            )
            .into());
        };
        let len = stream_len(&stream);
        if len != 1 {
            return Err(zirql::QueryError::query(
                range.clone(),
                "required guard needs exactly one subject",
            )
            .into());
        }
        budget.charge(len, zirql::Location::Query(range.clone()))?;
        budget.charge_results(len, zirql::Location::Query(range.clone()))?;
        let row = context
            .query
            .take_rows(stream)
            .pop()
            .expect("one guard subject");
        if context.query.report.is_none() {
            use zirql::Host;
            let leaf = (predicate, stage);
            let stage = Located::new(
                zirql::Stage::Filter(Located::new(
                    zirql::PredicateKind::Leaf(leaf),
                    range.clone(),
                )),
                range.clone(),
            );
            let mut query = &mut *context.query;
            pipeline::NativeHost.prepare_stage(&stage, &mut query, budget)?;
            pipeline::NativeHost.prepare_filter_item(&row, &mut query, budget, range)?;
            return pipeline::NativeHost.evaluate_leaf(&leaf, &row, &mut query, budget, range);
        }
        let subject = context.query.subject(&row);
        let report = context.query.report.as_mut().ok_or_else(|| {
            zirql::QueryError::query(range.clone(), "semantic guard requires model facts")
        })?;
        let (truth, diagnostic) = report
            .guard_leaf(
                predicate,
                &subject,
                &mut Work { budget },
                stage,
                self.0.limits,
            )
            .map_err(zirql::EvaluationError::Host)?;
        if let Some(diagnostic) = diagnostic {
            context.guard_unknown = Some(diagnostic);
        }
        Ok(match truth {
            semantic::Truth::True => zirql::Truth::True,
            semantic::Truth::False => zirql::Truth::False,
            semantic::Truth::Unknown => zirql::Truth::Unknown,
        })
    }
    pub(super) fn paths(
        &self,
        paths: &Paths,
        input: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        let kind = self.kind(&input, budget, &paths.range)?;
        let mut scope = BTreeMap::new();
        for (name, value) in bindings.iter() {
            budget.charge(name.len() + 1, zirql::Location::Query(paths.range.clone()))?;
            scope.insert(name.to_owned(), self.kind(value, budget, &paths.range)?);
        }
        let schema = zirql::path_types(paths, self, &kind, &scope, budget)?;
        let previous = context.guard_unknown.take();
        let result = zirql::evaluate_paths(paths, self, input, context, bindings, budget);
        let responsible = std::mem::replace(&mut context.guard_unknown, previous);
        let rows = result.map_err(|error| predicate_error(error, responsible))?;
        budget.charge(rows.len() + 1, zirql::Location::Query(paths.range.clone()))?;
        budget.charge_results(rows.len() + 1, zirql::Location::Query(paths.range.clone()))?;
        let rows = rows
            .into_iter()
            .map(|row| {
                let ReportValue::Capture(capture) = row else {
                    unreachable!("native path record")
                };
                capture
            })
            .collect();
        Ok(ReportValue::Captures(CaptureMatches {
            rows,
            schema: Rc::new(schema),
        }))
    }
    pub(super) fn matching(
        &self,
        matching: &Matching,
        input: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        let kind = self.kind(&input, budget, &matching.range)?;
        let mut scope = BTreeMap::new();
        for (name, value) in bindings.iter() {
            budget.charge(
                name.len() + 1,
                zirql::Location::Query(matching.range.clone()),
            )?;
            scope.insert(name.to_owned(), self.kind(value, budget, &matching.range)?);
        }
        let schema = zirql::match_types(matching, self, &kind, &scope, budget)?;
        let previous = context.guard_unknown.take();
        let result = zirql::evaluate_match(matching, self, input, context, bindings, budget);
        let responsible = std::mem::replace(&mut context.guard_unknown, previous);
        let rows = result.map_err(|error| predicate_error(error, responsible))?;
        budget.charge(
            rows.len() + 1,
            zirql::Location::Query(matching.range.clone()),
        )?;
        budget.charge_results(
            rows.len() + 1,
            zirql::Location::Query(matching.range.clone()),
        )?;
        let rows = rows
            .into_iter()
            .map(|row| {
                let ReportValue::Capture(capture) = row else {
                    unreachable!("composition host constructs captures")
                };
                capture
            })
            .collect();
        Ok(ReportValue::Captures(CaptureMatches {
            rows,
            schema: Rc::new(schema),
        }))
    }
}

fn path_declaration(
    value: &ReportValue<'_>,
    range: &Range<usize>,
) -> Result<semantic::SemanticEntity, zirql::QueryError> {
    if let ReportValue::Query(SemanticStream::Entities(rows)) = functions::unretained(value)
        && let [entity @ semantic::SemanticEntity::Declaration(_)] = rows.as_slice()
    {
        return Ok(*entity);
    }
    Err(zirql::QueryError::query(
        range.clone(),
        "paths require native declarations",
    ))
}

impl<'a> zirql::PathHost<Context<'_, 'a>> for CompositionHost<'_, '_> {
    type Target = Guard;
    fn validate_path_edge(
        &self,
        edge: &Stage,
        input: &Kind,
        _: &BTreeMap<String, Kind>,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::QueryError> {
        if let Stage::Query(index, _) = edge
            && matches!(input, Kind::Declarations | Kind::Declaration)
            && matches!(
                self.0.queries[*index].query.stages.as_slice(),
                [language::Stage {
                    kind: language::StageKind::ReferencedDeclarations,
                    ..
                }]
            )
        {
            return Ok(());
        }
        Err(zirql::QueryError::query(
            range.clone(),
            "path edge requires declarations",
        ))
    }
    fn validate_target_leaf(
        &self,
        target: &Guard,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::QueryError> {
        self.validate_guard_leaf(target, input, scope, budget, range)
    }
    fn evaluate_target_leaf(
        &self,
        target: &Guard,
        input: &Self::Value,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<Self::Value>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        path_declaration(input, range)?;
        self.evaluate_guard_leaf(target, input, context, bindings, budget, range)
    }
    fn evaluate_target_leaf_with_arguments(
        &self,
        guard: &Guard,
        input: &Self::Value,
        arguments: &mut dyn zirql::NativeArguments<Context<'_, 'a>, Self>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        path_declaration(input, range)?;
        self.guard_with_arguments(guard, input, None, arguments, budget, range)
    }

    fn path_neighbors(
        &self,
        edge: &Stage,
        input: &Self::Value,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<Self::Value>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<Self::Value>, zirql::EvaluationError<QueryError>> {
        path_declaration(input, range)?;
        let input = self.copy_value(input, context, budget, range)?;
        let neighbors = self
            .0
            .evaluate_stage(edge, input, context, bindings, budget, range)?;
        self.subjects(neighbors, context, budget, range)
    }
    fn same_path_identity(
        &self,
        left: &Self::Value,
        right: &Self::Value,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<bool, zirql::EvaluationError<QueryError>> {
        Ok(path_declaration(left, range)? == path_declaration(right, range)?)
    }
    fn path_steps(
        &self,
        steps: Vec<Self::Value>,
        _: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        budget.check_items(steps.len(), zirql::Location::Query(range.clone()))?;
        budget.charge(steps.len(), zirql::Location::Query(range.clone()))?;
        budget.charge_results(steps.len() + 1, zirql::Location::Query(range.clone()))?;
        let mut declarations = Vec::with_capacity(steps.len());
        for step in steps {
            declarations.push(path_declaration(&step, range)?);
        }
        Ok(ReportValue::Query(SemanticStream::Entities(declarations)))
    }
}

pub(super) fn predicate_error(
    error: zirql::EvaluationError<QueryError>,
    responsible: Option<SourceDiagnostic>,
) -> zirql::EvaluationError<QueryError> {
    match error {
        zirql::EvaluationError::Core(error)
            if error.category == zirql::QueryErrorCategory::UnknownPredicate =>
        {
            match responsible {
                Some(diagnostic) => zirql::EvaluationError::Host(QueryError {
                    range: diagnostic.location.range.clone(),
                    location: crate::ErrorLocation::Model(diagnostic.location),
                    message: diagnostic.message,
                }),
                None => zirql::EvaluationError::Core(error),
            }
        }
        error => error,
    }
}

fn value_placeholder<'a>() -> ReportValue<'a> {
    ReportValue::Record(Vec::new())
}

fn guard_subject_kind(kind: &Kind, range: &Range<usize>) -> Result<Kind, zirql::QueryError> {
    if matches!(
        kind,
        Kind::Node
            | Kind::Declaration
            | Kind::Nodes
            | Kind::Entities
            | Kind::Declarations
            | Kind::Types
            | Kind::Instantiations
    ) {
        Ok(kind.clone())
    } else {
        Err(zirql::QueryError::query(
            range.clone(),
            "semantic guard requires a native subject",
        ))
    }
}

fn boolean_truth(
    value: zirql::ConstructedValue,
    range: &Range<usize>,
) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
    match value.data() {
        zirql::ConstructedValue::Boolean(true) => Ok(zirql::Truth::True),
        zirql::ConstructedValue::Boolean(false) => Ok(zirql::Truth::False),
        _ => Err(zirql::QueryError::query(range.clone(), "predicate requires a Boolean").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_category_uses_the_responsible_model_diagnostic_or_retains_core_error() {
        let responsible = SourceDiagnostic {
            location: zincite_lint::SourceLocation {
                path: "included/model.mzn".into(),
                range: 20..27,
                line: 3,
                column: 5,
            },
            message: "model predicate is uncertain".into(),
        };
        let error = zirql::QueryError::query(7..14, "changed Unknown refusal")
            .with_category(zirql::QueryErrorCategory::UnknownPredicate);
        assert_eq!(
            predicate_error(
                zirql::EvaluationError::Core(error.clone()),
                Some(responsible.clone())
            ),
            zirql::EvaluationError::Host(QueryError {
                range: 20..27,
                location: crate::ErrorLocation::Model(responsible.location.clone()),
                message: responsible.message.clone(),
            })
        );
        assert_eq!(
            predicate_error(zirql::EvaluationError::Core(error.clone()), None),
            zirql::EvaluationError::Core(error)
        );
        let other = zirql::QueryError::query(8..15, "path target is Unknown");
        assert_eq!(
            predicate_error(
                zirql::EvaluationError::Core(other.clone()),
                Some(responsible.clone())
            ),
            zirql::EvaluationError::Core(other)
        );
        let native = QueryError::input(30..35, "native source error");
        assert_eq!(
            predicate_error(
                zirql::EvaluationError::Host(native.clone()),
                Some(responsible)
            ),
            zirql::EvaluationError::Host(native)
        );
    }
}
