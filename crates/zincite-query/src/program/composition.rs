//! Concrete native composition adapter; Zirql reads, validates and executes clauses.
use super::*;
use zirql::{ProjectionHost, ValueReaderHost};

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
        if let Some(suffix) = self.markdown_suffix(&range)? {
            let (mut expression, kind, emits) =
                self.composition_value(range.start..suffix.start, native_record, binding)?;
            if emits {
                zirql::dispose_expression(expression);
                return Err(QueryError::query(
                    suffix,
                    "markdown cannot follow another terminal emitter",
                ));
            }
            self.preflight
                .charge(3, zirql::Location::Query(suffix.clone()))
                .map_err(pipeline::core_error)?;
            self.preflight
                .charge_results(3, zirql::Location::Query(suffix.clone()))
                .map_err(pipeline::core_error)?;
            let step = Located::new(
                ExpressionStep::Emit {
                    emitter: Emitter::Markdown,
                    policy: EmissionPolicy::Terminal,
                },
                suffix,
            );
            if let ExpressionKind::Sequence { steps, .. } = &mut expression.value {
                steps.push(step);
            } else {
                expression = Located::new(
                    ExpressionKind::Sequence {
                        input: Box::new(expression),
                        steps: vec![step],
                    },
                    range.clone(),
                );
            }
            expression.range = range.clone();
            check_edit_order(&expression, &self.queries, &mut false)?;
            self.cursor.position = range.end;
            return Ok((expression, kind, true));
        }
        self.composition_value(range, native_record, binding)
    }

    fn markdown_suffix(
        &mut self,
        range: &Range<usize>,
    ) -> Result<Option<Range<usize>>, QueryError> {
        use zincite_syntax::TokenKind as T;
        let tokens = self.lexed.tokens();
        let end = tokens.partition_point(|token| token.range.end <= range.end);
        let mut scanned = 0;
        let mut tail = tokens[..end]
            .iter()
            .rev()
            .take_while(|token| token.range.start >= range.start)
            .inspect(|_| scanned += 1)
            .filter(|token| {
                !matches!(token.kind, T::Whitespace | T::LineComment | T::BlockComment)
            });
        let suffix = match (tail.next(), tail.next()) {
            (Some(name), Some(pipe))
                if &self.cursor.source[name.range.clone()] == "markdown"
                    && pipe.kind == T::Pipe =>
            {
                Some(pipe.range.start..name.range.end)
            }
            _ => None,
        };
        self.preflight
            .charge(scanned, zirql::Location::Query(range.clone()))
            .map_err(pipeline::core_error)?;
        Ok(suffix)
    }

    fn composition_value(
        &mut self,
        range: Range<usize>,
        native_record: bool,
        binding: bool,
    ) -> Result<(Expression, Kind, bool), QueryError> {
        self.begin_pure(range.clone())?;
        let mut budget =
            std::mem::replace(&mut self.preflight, Budget::for_program(self.limits.core()));
        let mut reader = zirql::ArgumentReader::new(&self.cursor.source[..range.end], range.start)
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
        for query in &natives {
            zirql::walk_expression(
                query,
                &mut self.preflight,
                self.reader_nesting,
                |query, _| {
                    if let ExpressionKind::Sequence { steps, .. } = &query.value {
                        self.prepared_values |= steps.iter().any(|step| {
                            !matches!(
                                step.value,
                                ExpressionStep::Stage(_) | ExpressionStep::Emit { .. }
                            )
                        });
                    }
                    Ok(())
                },
            )
            .map_err(pipeline::core_error)?;
        }
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
        // Functions are collected before their result declarations are known.
        // Recover that late semantic fact before choosing native/value lowering.
        functions::recover_call_value(&mut value, &self.queries, &self.constructed_functions);
        let native = matches!(
            value.value,
            zirql::ValueExpressionKind::NativeQuery(_) | zirql::ValueExpressionKind::NativeValue(_)
        );
        if !native && !retained_record {
            functions::lower_recovered_value(
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
            return Ok((core_value(value, range), kind, false));
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
                return Ok((core_value(value, range), Kind::Constructed, false));
            }
        };
        check_edit_order(&expression, &self.queries, &mut false)?;
        let host = NativeHost {
            project_reports: false,
            functions: self.prepared || self.prepared_values || !self.function_names.is_empty(),
            signatures: &[],
            declarations: &[],
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
            functions::expression_kind(
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
                    match &step.value {
                        ExpressionStep::Value(value) => {
                            self.admit_data_value(value, budget, true)?
                        }
                        ExpressionStep::Stage(stage) => {
                            self.admit_data_stage(stage, &step.range, budget)?
                        }
                        ExpressionStep::Filter(_) => {
                            let mut result = Ok(());
                            step.value.for_each_payload(|guard| {
                                if result.is_ok() {
                                    result = self.admit_data_stage(guard, &step.range, budget);
                                }
                            });
                            result?;
                        }
                        ExpressionStep::Head(value) => {
                            self.admit_data_value(value, budget, false)?
                        }
                        ExpressionStep::Collection(
                            zirql::Stage::Head(_) | zirql::Stage::Count | zirql::Stage::Unique,
                        ) => {}
                        ExpressionStep::Emit { .. } => {
                            return Err(QueryError::query(
                                step.range.clone(),
                                "data expressions cannot emit",
                            ));
                        }
                        _ => {
                            return Err(QueryError::query(
                                step.range.clone(),
                                "report-only native expression is unavailable in data programs",
                            ));
                        }
                    }
                }
                Ok(())
            }
            _ => Err(QueryError::query(
                expression.range.clone(),
                "report-only native expression is unavailable in data programs",
            )),
        }
    }
    pub(super) fn admit_data_value(
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
            Stage::Copy(_) | Stage::Data(..) | Stage::Member(_) => Ok(()),
            Stage::Guard(guard) => {
                if semantic::requests_facts(&guard.stage) {
                    return Err(QueryError::query(
                        range.clone(),
                        "semantic predicate is unavailable in data programs",
                    ));
                }
                for value in guard.values() {
                    self.admit_data_value(value, budget, false)?;
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
            | "markdown"
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

    fn is_stage(&self, name: &str) -> bool {
        matches!(name, "clone_members" | "clone_elements" | "repeat_elements")
            || domain_stage(name)
            || self.function_names.contains(name)
    }
    fn parse_stage_in_scope(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        locals: &[&str],
        budget: &mut Budget,
        nesting: usize,
    ) -> Result<ExpressionStep<Stage, Emitter, zirql::NativeSetOperator>, zirql::QueryError> {
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
    ) -> Result<Stage, zirql::QueryError> {
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
    ) -> Result<ExpressionStep<Stage, Emitter, zirql::NativeSetOperator>, zirql::QueryError> {
        if reader.keyword("markdown") {
            self.cursor.position = reader.position();
            return Ok(ExpressionStep::Emit {
                emitter: Emitter::Markdown,
                policy: EmissionPolicy::Terminal,
            });
        }
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
            self.prepared_values = true;
            return Ok(ExpressionStep::Paths(paths.into()));
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
            let locals = self.reader_locals();
            let mut value = zirql::read_function_call_in_scope(
                reader,
                function,
                self,
                &locals.iter().map(String::as_str).collect::<Vec<_>>(),
                budget,
                self.reader_nesting,
            )?;
            functions::normalize_value(
                &mut value,
                &self.names,
                &self.queries,
                &self.constructed_functions,
            );
            return Ok(ExpressionStep::Value(value.into()));
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
            language::StageKind::Head(count) => {
                self.prepared_values = true;
                match arguments.pop() {
                    Some(argument) => ExpressionStep::Head(argument.expression.into()),
                    None => ExpressionStep::Collection(zirql::Stage::Head(count)),
                }
            }
            language::StageKind::Tail(count) => {
                self.prepared_values = true;
                match arguments.pop() {
                    Some(argument) => ExpressionStep::Tail(argument.expression.into()),
                    None => ExpressionStep::Collection(zirql::Stage::Tail(count)),
                }
            }
            language::StageKind::Check { expected, message } => {
                self.prepared_values = true;
                if arguments.is_empty() {
                    ExpressionStep::Collection(zirql::Stage::Check { expected, message })
                } else {
                    let mut operand = None;
                    let mut dynamic_message = None;
                    for argument in arguments {
                        if matches!(argument.kind, language::ArgumentKind::CheckMessage) {
                            dynamic_message = Some(argument.expression.into());
                        } else {
                            operand = Some(argument.expression.into());
                        }
                    }
                    ExpressionStep::Check {
                        operand,
                        message: dynamic_message,
                    }
                }
            }
            language::StageKind::Reverse => {
                self.prepared_values = true;
                ExpressionStep::Collection(zirql::Stage::Reverse)
            }
            language::StageKind::Count => {
                self.prepared_values = true;
                ExpressionStep::Collection(zirql::Stage::Count)
            }
            language::StageKind::Unique => {
                self.prepared_values = true;
                ExpressionStep::Collection(zirql::Stage::Unique)
            }
            language::StageKind::Tally => {
                self.prepared_values = true;
                ExpressionStep::Collection(zirql::Stage::Tally)
            }
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
    ) -> Result<Stage, zirql::QueryError> {
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
            return Ok(Stage::Guard(Box::new(Guard {
                boolean: None,
                operand: None,
                prepared: Some(Located::new(
                    zirql::ValueExpressionKind::NativeValue(Box::new(operand)),
                    range,
                )),
                argument: Some((argument, value)),
                stage,
            })));
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
        Ok(Stage::Guard(Box::new(guard)))
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
    ) -> Result<Stage, zirql::QueryError> {
        let input = Located::new(
            zirql::ValueExpressionKind::NativeValue(Box::new(Located::new(
                ExpressionKind::Input,
                range.clone(),
            ))),
            range.clone(),
        );
        Ok(Stage::Guard(Box::new(Guard {
            boolean: Some(predicate.bind(input)),
            operand: None,
            prepared: None,
            argument: None,
            stage: language::Stage {
                kind: language::StageKind::Names,
                range,
            },
        })))
    }
    fn parse_predicate_leaf_in_argument(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        argument: Option<&zirql::Pattern>,
        locals: &[&str],
        budget: &mut Budget,
        nesting: usize,
    ) -> Result<Stage, zirql::QueryError> {
        let mut stage = self.parse_predicate_leaf_in_scope(reader, locals, budget, nesting)?;
        let Stage::Guard(guard) = &mut stage else {
            unreachable!("native guard leaf")
        };
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
        Ok(stage)
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
impl<'c, 'a> ProgramHost<Context<'c, 'a>> for CompositionHost<'_, '_> {
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
    fn transition(
        &self,
        input: Option<Self::Value>,
        context: &mut Context<'c, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
        run: impl FnOnce(
            Option<Self::Value>,
            &mut Context<'c, 'a>,
            &mut Budget,
        ) -> Result<Self::Value, zirql::EvaluationError<QueryError>>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        let partial = input.as_ref().and_then(native::selection_error).cloned();
        if context.query.complete_observations
            && let Some(input) = &input
        {
            native::require_complete(input, budget, range)?;
        }
        let previous = context.query.incomplete_observation.take();
        let result = run(input, context, budget);
        let observed = std::mem::replace(&mut context.query.incomplete_observation, previous);
        let value = result?;
        native::incomplete(value, partial.or(observed), budget, range)
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
        if let Stage::Guard(guard) = stage {
            return Ok(guard.values().map(|value| (value, Kind::Nodes)).collect());
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
        let _ = scope;
        let values = self.value_arguments(stage, budget, range)?;
        Ok(values
            .into_iter()
            .map(|(expression, kind)| zirql::ScopedNativeArgument {
                expression,
                relative: Some(if matches!(stage, Stage::Guard(_)) {
                    input.clone()
                } else {
                    kind
                }),
                locals: Vec::new(),
                allow_effects: false,
            })
            .collect())
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
            ReportValue::Dictionary(value) => Kind::GeneralDictionary(Rc::clone(&value.schema)),
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
    fn record_kind(
        &self,
        fields: Vec<(String, Kind)>,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        Ok(Kind::Capture(Rc::new(zirql::NativeCapture { fields })))
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
            Kind::GeneralDictionary(schema) => Some(vec![T::Dictionary {
                keys: if schema.0 == Kind::Strings {
                    vec![T::String]
                } else {
                    self.scalar_types(&schema.0, budget, range)?
                        .unwrap_or_else(|| vec![T::Unestablished])
                },
                values: self
                    .scalar_types(&schema.1, budget, range)?
                    .unwrap_or_else(|| vec![T::Unestablished]),
            }]),
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
    fn expression_kind(
        &self,
        expression: &Expression,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
    ) -> Result<Kind, zirql::QueryError> {
        functions::expression_kind(expression, self, input, scope, budget)
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
            Stage::Data(..) => Ok(input.clone()),
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
                if self.0.queries[*index].query.is_editing() {
                    Ok(input.clone())
                } else {
                    validate_stages(&self.0.queries[*index].query, input.clone())
                        .map_err(query_error)
                }
            }
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
        matches!(stage, Stage::Member(_))
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
            return Ok(Kind::Value(Rc::new(zirql::collection_subject_types(
                types,
                budget,
                zirql::Location::Query(range.clone()),
            )?)));
        }
        match input {
            Kind::GeneralDictionary(schema) => Ok(schema.1.clone()),
            Kind::Dictionary(values) => {
                if let Some(first) = values.first()
                    && values.iter().all(|value| value == first)
                {
                    return Ok(first.clone());
                }
                let mut types = Vec::new();
                for value in values.iter() {
                    let Some(facts) = self.scalar_types(value, budget, range)? else {
                        return Err(zirql::QueryError::query(
                            range.clone(),
                            "dictionary values need one admitted subject kind",
                        ));
                    };
                    types.extend(facts);
                }
                if types.is_empty() {
                    types.push(zirql::ValueType::Unestablished);
                }
                Ok(Kind::Value(Rc::new(types)))
            }
            Kind::Tally => Ok(Kind::Count),
            Kind::Record => Ok(Kind::Constructed),
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
    fn selection_kind(
        &self,
        input: &Kind,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        native::set_kind(input, range)
    }
    fn is_constructed_kind(&self, kind: &Kind) -> bool {
        matches!(kind, Kind::Constructed | Kind::Value(_) | Kind::Scalar(_))
    }
    fn selection_items(
        &self,
        input: &Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<Self::Value>, zirql::EvaluationError<QueryError>> {
        let value = self.copy_value(input, context, budget, range)?;
        self.rows(value, context, budget, range)
            .map(|(_, rows)| rows)
    }
    fn restore_selection(
        &self,
        input: Self::Value,
        items: Vec<Self::Value>,
        policy: zirql::SelectionShape,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        let mut shape = native::shape(&input);
        if policy == zirql::SelectionShape::Filter
            && let native::Shape::Stream { item, .. } = &mut shape
        {
            *item = false;
        }
        let restored = shape.restore(items, context, budget, range)?;
        match restored {
            ReportValue::Constructed(selected) => {
                match zirql::RuntimeTypeHost::constructed_value(self, &input) {
                    Some(original) => Ok(ReportValue::Constructed(
                        zirql::restore_collection_declaration(
                            original,
                            selected,
                            policy == zirql::SelectionShape::Filter,
                            budget,
                            zirql::Location::Query(range.clone()),
                        )?,
                    )),
                    None => Ok(ReportValue::Constructed(selected)),
                }
            }
            restored => Ok(restored),
        }
    }
    fn admit_selection(
        &self,
        value: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        self.admitted_copy(&value, context, budget, range)
    }
    fn same_selection(
        &self,
        left: &Self::Value,
        right: &Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<bool, zirql::EvaluationError<QueryError>> {
        self.same_native_selection(left, right, context, budget, range)
    }
    fn ordering_key_kind(
        &self,
        kind: &Kind,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::QueryError> {
        if native::supported_key(kind) {
            Ok(())
        } else {
            Err(zirql::QueryError::query(
                range.clone(),
                "native ordering needs an explicit supported data key",
            ))
        }
    }
    fn ordering_key(
        &self,
        value: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::NativeOrderKey, zirql::EvaluationError<QueryError>> {
        native::ordering_key(value, context, budget, range, self.0.nesting)
    }
    fn mapping_kind(
        &self,
        key: &Kind,
        value: &Kind,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        if !native::string_key_kind(key) {
            return Err(zirql::QueryError::query(
                range.clone(),
                "map_by key must produce exactly one string",
            ));
        }
        Ok(Kind::Dictionary(Rc::new(vec![value.clone()])))
    }
    fn mapping_keys(
        &self,
        value: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Option<Vec<String>>, zirql::EvaluationError<QueryError>> {
        native::string_keys(value, context, budget, range, self.0.nesting)
    }
    fn map_sort_kind(
        &self,
        input: &Kind,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        if matches!(input, Kind::GeneralDictionary(_)) {
            Ok(input.clone())
        } else {
            native::map_sort_kind(input, range)
        }
    }
    fn is_map_kind(&self, kind: &Kind) -> bool {
        matches!(
            kind,
            Kind::Dictionary(_) | Kind::GeneralDictionary(_) | Kind::Record | Kind::Tally
        )
    }
    fn map_entries(
        &self,
        value: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<(String, Self::Value)>, zirql::EvaluationError<QueryError>> {
        native::map_entries(value, context, budget, range)
    }
    fn construct_map(
        &self,
        entries: Vec<(String, Self::Value)>,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        Ok(ReportValue::Record(entries))
    }
    fn restore_map(
        &self,
        input: Self::Value,
        entries: Vec<(String, Self::Value)>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        native::restore_map(input, entries, context, budget, range)
    }
    fn capture_selection_kind(
        &self,
        schema: zirql::NativeCapture<Kind>,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        Ok(Kind::Captures(Rc::new(schema)))
    }
    fn construct_captures(
        &self,
        rows: Vec<Self::Value>,
        schema: zirql::NativeCapture<Kind>,
        _: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        budget.charge(rows.len() + 1, zirql::Location::Query(range.clone()))?;
        budget.charge_results(rows.len() + 1, zirql::Location::Query(range.clone()))?;
        let rows = rows
            .into_iter()
            .map(|row| match row {
                ReportValue::Capture(capture) => capture,
                _ => unreachable!("native capture row"),
            })
            .collect();
        Ok(ReportValue::Captures(CaptureMatches {
            rows,
            schema: Rc::new(schema),
        }))
    }
    fn filter_truth(
        &self,
        truth: zirql::Truth,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<bool, zirql::EvaluationError<QueryError>> {
        native::finish_filter(truth, context, budget, range)
            .map(|truth| truth == zirql::Truth::True)
    }
    fn require_guard(
        &self,
        truth: zirql::Truth,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
        message: &str,
    ) -> Result<bool, zirql::EvaluationError<QueryError>> {
        if truth == zirql::Truth::Unknown {
            return Err(predicate_error(
                zirql::QueryError::query(range.clone(), message)
                    .with_category(zirql::QueryErrorCategory::UnknownPredicate)
                    .into(),
                context.guard_unknown.take(),
            ));
        }
        context.guard_unknown = None;
        let _ = budget;
        Ok(truth == zirql::Truth::True)
    }
    fn collection_kind(
        &self,
        operation: &zirql::Stage<()>,
        input: &Kind,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        if let Kind::GeneralDictionary(_) = input {
            return match operation {
                zirql::Stage::Count => Ok(Kind::Count),
                zirql::Stage::Head(_)
                | zirql::Stage::Tail(_)
                | zirql::Stage::Reverse
                | zirql::Stage::Check { .. } => Ok(input.clone()),
                _ => Err(zirql::QueryError::query(
                    range.clone(),
                    "unique/tally need a positional selection",
                )),
            };
        }
        if matches!(operation, zirql::Stage::Unique | zirql::Stage::Tally) {
            let key = self.subject_kind(input, budget, range)?;
            return if matches!(operation, zirql::Stage::Unique) {
                Ok(input.clone())
            } else {
                Ok(Kind::GeneralDictionary(Rc::new((
                    key,
                    Kind::Value(Rc::new(vec![zirql::ValueType::Integer])),
                ))))
            };
        }
        let stage = language::Stage {
            kind: match operation {
                zirql::Stage::Head(n) => language::StageKind::Head(*n),
                zirql::Stage::Tail(n) => language::StageKind::Tail(*n),
                zirql::Stage::Reverse => language::StageKind::Reverse,
                zirql::Stage::Count => language::StageKind::Count,
                zirql::Stage::Unique => language::StageKind::Unique,
                zirql::Stage::Tally => language::StageKind::Tally,
                zirql::Stage::Check { expected, message } => language::StageKind::Check {
                    expected: *expected,
                    message: message.clone(),
                },
                _ => {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "unsupported collection tag",
                    ));
                }
            },
            range: range.clone(),
        };
        validate_stages(
            &Query {
                stages: vec![stage],
                range: range.clone(),
                nesting_limit: self.0.nesting,
            },
            input.clone(),
        )
        .map_err(query_error)
    }
    fn collection_unique_tally(
        &self,
        operation: &zirql::Stage<()>,
        input: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        zirql::evaluate_native_unique_tally(operation, input, self, context, budget, range)
    }
    fn dictionary_view<'v>(
        &self,
        value: &'v Self::Value,
    ) -> Option<&'v [(Self::Value, Self::Value)]> {
        match functions::unretained(value) {
            ReportValue::Dictionary(value) => Some(value.entries()),
            _ => None,
        }
    }
    fn construct_dictionary(
        &self,
        entries: Vec<(Self::Value, Self::Value)>,
        _: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        let mut schema = None;
        for (key, value) in &entries {
            let entry = (
                self.kind(key, budget, range)?,
                self.kind(value, budget, range)?,
            );
            match &mut schema {
                None => schema = Some(entry),
                Some((key, value)) => {
                    if *key != entry.0 {
                        *key = Kind::Constructed;
                    }
                    if *value != entry.1 {
                        *value = Kind::Constructed;
                    }
                }
            }
        }
        let (key, value) = schema.unwrap_or((Kind::Constructed, Kind::Constructed));
        Ok(ReportValue::Dictionary(NativeDictionary {
            entries: Rc::new(entries),
            schema: Rc::new((key, value)),
        }))
    }
    fn restore_dictionary(
        &self,
        input: Self::Value,
        entries: Vec<(Self::Value, Self::Value)>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        let tally = !matches!(functions::unretained(&input), ReportValue::Dictionary(_));
        let entries = if tally {
            entries
                .into_iter()
                .map(|(key, value)| {
                    let value = if let Some(zirql::ConstructedValue::Integer(count)) = self
                        .pattern_constructed(&value)
                        .map(zirql::ConstructedValue::data)
                    {
                        let count = usize::try_from(*count).map_err(|_| {
                            zirql::QueryError::query(range.clone(), "tally count overflow")
                        })?;
                        ReportValue::Query(SemanticStream::Inspection(Box::new(
                            QueryResult::Count(count),
                        )))
                    } else {
                        value
                    };
                    Ok((key, value))
                })
                .collect::<Result<Vec<_>, zirql::QueryError>>()?
        } else {
            entries
        };
        let schema = match functions::unretained(&input) {
            ReportValue::Dictionary(value) => Rc::clone(&value.schema),
            _ => Rc::new((
                self.subject_kind(&self.kind(&input, budget, range)?, budget, range)?,
                Kind::Count,
            )),
        };
        let _ = context;
        Ok(ReportValue::Dictionary(NativeDictionary {
            entries: Rc::new(entries),
            schema,
        }))
    }
    fn restore_collection(
        &self,
        input: Self::Value,
        result: zirql::NativeValue<Self::Value>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        match result {
            zirql::NativeValue::Stream(items) => self.restore_selection(
                input,
                items,
                zirql::SelectionShape::Preserve,
                context,
                budget,
                range,
            ),
            zirql::NativeValue::Count(count) => Ok(ReportValue::Query(SemanticStream::Inspection(
                Box::new(QueryResult::Count(count)),
            ))),
            zirql::NativeValue::Tally(entries) => Ok(ReportValue::Query(
                SemanticStream::Inspection(Box::new(QueryResult::Tally(entries))),
            )),
        }
    }

    fn validate_guard_leaf(
        &self,
        guard: &Stage,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::QueryError> {
        let Stage::Guard(guard) = guard else {
            return Err(zirql::QueryError::query(
                range.clone(),
                "expected native guard payload",
            ));
        };
        if guard.boolean.is_some() {
            return Ok(());
        }
        if let Some((kind, expression)) = &guard.argument {
            functions::check_argument(expression, *kind, self, input, scope, budget)?;
        }
        let kind = if let Some(operand) = &guard.prepared {
            functions::value_kind(operand, self, input, scope, budget)?
        } else {
            functions::expression_kind(guard.operand(), self, input, scope, budget)?
        };
        guard_subject_kind(&kind, range)?;
        Ok(())
    }
    fn evaluate_guard_leaf(
        &self,
        guard: &Stage,
        input: &Self::Value,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<Self::Value>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        let Stage::Guard(guard) = guard else {
            return Err(
                zirql::QueryError::query(range.clone(), "expected native guard payload").into(),
            );
        };
        if guard.boolean.is_some() || guard.prepared.is_some() || guard.argument.is_some() {
            return Err(zirql::QueryError::query(
                range.clone(),
                "value guards require the active prepared argument runner",
            )
            .into());
        }
        let relative = self.copy_value(input, context, budget, range)?;
        let value =
            zirql::evaluate_expression(guard.operand(), self, relative, context, bindings, budget)?;
        self.guard_value(
            value,
            guard.predicate(),
            &guard.stage,
            context,
            budget,
            range,
        )
    }
    fn evaluate_guard_leaf_with_arguments(
        &self,
        guard: &Stage,
        input: &Self::Value,
        bindings: &Bindings<Self::Value>,
        arguments: &mut dyn zirql::NativeArguments<Context<'_, 'a>, Self>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        let Stage::Guard(guard) = guard else {
            return Err(
                zirql::QueryError::query(range.clone(), "expected native guard payload").into(),
            );
        };
        self.guard_with_arguments(guard, input, Some(bindings), arguments, budget, range)
    }
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
