//! Read-only navigation over the existing public model facts.
use std::collections::HashSet;
use std::ops::Range;

use serde_json::{Value, json};
use zincite_lint::{
    BindingFacts, BindingResolution, CallOutcome, CallableFacts, DeclarationId, FileId,
    Instantiation, InstantiationFacts, ModelContext, ReferenceKind, SourceDiagnostic, SourceKind,
    SourceLocation, TypeInst, TypeKind, resolve_bindings, resolve_callables,
    resolve_instantiations,
};
use zincite_syntax::{FileMode, SyntaxNode};

use crate::inspection::{check_collection, kind_name, type_error};
use crate::language::{Predicate, PredicateKind, Stage, StageKind};
use crate::{ErrorLocation, Input, Limits, Query, QueryError, QueryResult, Selection, Work};

/// Identities belong to `SemanticResult::bindings`, independently of CST identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticEntity {
    Declaration(DeclarationId),
    /// Index into the retained binding facts' written references.
    Reference(usize),
}

/// A type or instantiation belongs to this exact declaration, occurrence or node.
#[derive(Clone, Debug)]
pub enum SemanticSubject<'a> {
    Entity(SemanticEntity),
    Node {
        file: FileId,
        node: &'a SyntaxNode,
        location: SourceLocation,
    },
}

#[derive(Clone, Debug)]
pub struct SemanticType<'a> {
    pub subject: SemanticSubject<'a>,
    /// None means the producer has no fact for this exact subject.
    pub ty: Option<TypeInst>,
}

#[derive(Clone, Debug)]
pub struct SemanticInstantiation<'a> {
    pub subject: SemanticSubject<'a>,
    pub instantiation: Instantiation,
    pub reason: Option<String>,
}

#[derive(Debug)]
pub enum SemanticStream<'a> {
    /// An ordinary projection, count or JSON emitter; limitations remain outside it.
    Inspection(Box<QueryResult<'a>>),
    Entities(Vec<SemanticEntity>),
    Types(Vec<SemanticType<'a>>),
    Instantiations(Vec<SemanticInstantiation<'a>>),
}

/// Native inspection with immutable source owners and only demanded fact families.
/// The input and model must outlive this result. Locations include each file's BOM;
/// declaration owning CST ranges remain in the fact API's BOM-stripped coordinates.
#[derive(Debug)]
pub struct SemanticResult<'a> {
    model: &'a ModelContext,
    bindings: Option<BindingFacts>,
    callables: Option<CallableFacts>,
    instantiations: Option<InstantiationFacts>,
    stream: SemanticStream<'a>,
    limitations: Vec<SourceDiagnostic>,
    json: bool,
}

impl<'a> SemanticResult<'a> {
    pub fn model(&self) -> &'a ModelContext {
        self.model
    }
    pub fn bindings(&self) -> Option<&BindingFacts> {
        self.bindings.as_ref()
    }
    pub fn callables(&self) -> Option<&CallableFacts> {
        self.callables.as_ref()
    }
    pub fn instantiation_facts(&self) -> Option<&InstantiationFacts> {
        self.instantiations.as_ref()
    }
    pub fn stream(&self) -> &SemanticStream<'a> {
        &self.stream
    }
    pub fn limitations(&self) -> &[SourceDiagnostic] {
        &self.limitations
    }
    pub fn is_complete(&self) -> bool {
        self.limitations.is_empty()
    }

    /// Locate a semantic identity using its actual retained file owner.
    pub fn location(&self, entity: SemanticEntity) -> &SourceLocation {
        match entity {
            SemanticEntity::Declaration(id) => {
                &self.bindings.as_ref().unwrap().declarations[id.0].location
            }
            SemanticEntity::Reference(id) => {
                &self.bindings.as_ref().unwrap().references[id].location
            }
        }
    }
    pub fn file(&self, entity: SemanticEntity) -> FileId {
        match entity {
            SemanticEntity::Declaration(id) => {
                self.bindings.as_ref().unwrap().declarations[id.0].file
            }
            SemanticEntity::Reference(id) => self.bindings.as_ref().unwrap().references[id].file,
        }
    }
    /// Source of a declaration's owning node, or an exact written reference token.
    pub fn source_bytes(&self, entity: SemanticEntity) -> &[u8] {
        let file = &self.model.files[self.file(entity)];
        let range = match entity {
            SemanticEntity::Declaration(id) => self.bindings.as_ref().unwrap().declarations[id.0]
                .syntax_range
                .clone(),
            SemanticEntity::Reference(_) => {
                let range = &self.location(entity).range;
                range.start - file.byte_offset..range.end - file.byte_offset
            }
        };
        &file.source_bytes()[range]
    }
    fn name(&self, entity: SemanticEntity) -> &str {
        match entity {
            SemanticEntity::Declaration(id) => {
                &self.bindings.as_ref().unwrap().declarations[id.0].name
            }
            SemanticEntity::Reference(id) => &self.bindings.as_ref().unwrap().references[id].name,
        }
    }
    fn subject_location(&self, subject: &SemanticSubject<'_>) -> SourceLocation {
        match subject {
            SemanticSubject::Entity(entity) => self.location(*entity).clone(),
            SemanticSubject::Node { location, .. } => location.clone(),
        }
    }
    fn subject_file(&self, subject: &SemanticSubject<'_>) -> FileId {
        match subject {
            SemanticSubject::Entity(entity) => self.file(*entity),
            SemanticSubject::Node { file, .. } => *file,
        }
    }
    fn subject_bytes(&self, subject: &SemanticSubject<'_>) -> &[u8] {
        match subject {
            SemanticSubject::Entity(entity) => self.source_bytes(*entity),
            SemanticSubject::Node { file, node, .. } => {
                &self.model.files[*file].source_bytes()[node.range()]
            }
        }
    }
    fn limit(
        &mut self,
        location: SourceLocation,
        message: impl Into<String>,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<(), QueryError> {
        let message = message.into();
        for existing in &self.limitations {
            work.visit(&stage.range)?;
            if existing.location == location && existing.message == message {
                return Ok(());
            }
        }
        check_collection(self.limitations.len(), limits.collection, &stage.range)?;
        work.spend(message.len().max(1), &stage.range)?;
        self.limitations
            .push(SourceDiagnostic { location, message });
        Ok(())
    }

    pub(crate) fn render(&self) -> Vec<u8> {
        if self.json {
            let value = json!({
                "complete": self.is_complete(),
                "limitations": self.limitations.iter().map(|d| json!({"file": d.location.path.to_string_lossy(), "range": range_json(&d.location.range), "line": d.location.line, "column": d.location.column, "message": d.message})).collect::<Vec<_>>(),
                "result": self.json_value().expect("semantic JSON was validated"),
            });
            let mut bytes = serde_json::to_vec(&value).expect("inspection values serialize");
            bytes.push(b'\n');
            return bytes;
        }
        match &self.stream {
            SemanticStream::Inspection(result) => result.render(),
            SemanticStream::Entities(entities) => entities
                .iter()
                .flat_map(|e| self.source_bytes(*e))
                .copied()
                .collect(),
            SemanticStream::Types(rows) => rows
                .iter()
                .flat_map(|row| {
                    format!(
                        "{}\n",
                        row.ty.as_ref().map_or("unknown", |ty| type_kind(&ty.kind))
                    )
                    .into_bytes()
                })
                .collect(),
            SemanticStream::Instantiations(rows) => rows
                .iter()
                .flat_map(|row| format!("{}\n", instantiation_name(row.instantiation)).into_bytes())
                .collect(),
        }
    }

    fn subject_json(&self, subject: &SemanticSubject<'_>) -> Result<Value, QueryError> {
        let file_id = self.subject_file(subject);
        let file = &self.model.files[file_id];
        let location = self.subject_location(subject);
        let text = std::str::from_utf8(self.subject_bytes(subject)).map_err(|_| {
            model_error(
                location.clone(),
                "text and JSON inspection require UTF-8 source text",
            )
        })?;
        let mut value = json!({"file": file.path.to_string_lossy(), "file_id": file_id, "range": range_json(&location.range), "source_kind": source_kind(file.kind), "text": text});
        match subject {
            SemanticSubject::Node { node, .. } => {
                value["kind"] = json!(kind_name(node.kind()));
            }
            SemanticSubject::Entity(entity) => {
                value["name"] = json!(self.name(*entity));
                match entity {
                    SemanticEntity::Declaration(id) => {
                        let declaration = &self.bindings.as_ref().unwrap().declarations[id.0];
                        value["kind"] = json!("declaration");
                        value["declaration_id"] = json!(id.0);
                        value["role"] =
                            json!(format!("{:?}", declaration.role).to_ascii_lowercase());
                        value["syntax_range"] = range_json(
                            &(declaration.syntax_range.start + file.byte_offset
                                ..declaration.syntax_range.end + file.byte_offset),
                        );
                    }
                    SemanticEntity::Reference(id) => {
                        let reference = &self.bindings.as_ref().unwrap().references[*id];
                        value["kind"] = json!("reference");
                        value["reference_id"] = json!(id);
                        value["reference_kind"] = json!(reference_kind(reference.kind));
                        value["binding"] = binding_json(&reference.resolution);
                        if let Some(callables) = &self.callables
                            && reference.kind == ReferenceKind::Callable
                        {
                            value["call"] = callables
                                .calls
                                .iter()
                                .find(|call| {
                                    call.file == reference.file
                                        && call.location.range.start
                                            == reference.location.range.start
                                })
                                .map_or(Value::Null, |call| call_json(&call.outcome));
                        }
                    }
                }
            }
        }
        Ok(value)
    }
    fn validate_json(&self, work: &mut Work, range: &Range<usize>) -> Result<(), QueryError> {
        let validate = |subject: &SemanticSubject<'_>| {
            std::str::from_utf8(self.subject_bytes(subject))
                .map(|_| ())
                .map_err(|_| {
                    model_error(
                        self.subject_location(subject),
                        "text and JSON inspection require UTF-8 source text",
                    )
                })
        };
        match &self.stream {
            SemanticStream::Inspection(result) => {
                crate::inspection::validate_json(result, work, range)?
            }
            SemanticStream::Entities(rows) => {
                for entity in rows {
                    validate(&SemanticSubject::Entity(*entity))?;
                }
            }
            SemanticStream::Types(rows) => {
                for row in rows {
                    validate(&row.subject)?;
                }
            }
            SemanticStream::Instantiations(rows) => {
                for row in rows {
                    validate(&row.subject)?;
                }
            }
        }
        Ok(())
    }

    fn json_value(&self) -> Result<Value, QueryError> {
        match &self.stream {
            SemanticStream::Inspection(result) => crate::inspection::json_value(result),
            SemanticStream::Entities(entities) => entities
                .iter()
                .map(|e| self.subject_json(&SemanticSubject::Entity(*e)))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            SemanticStream::Types(rows) => rows
                .iter()
                .map(|row| {
                    let mut value = self.subject_json(&row.subject)?;
                    value["type"] = row.ty.as_ref().map_or(Value::Null, type_json);
                    Ok(value)
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            SemanticStream::Instantiations(rows) => rows
                .iter()
                .map(|row| {
                    let mut value = self.subject_json(&row.subject)?;
                    value["instantiation"] = json!(instantiation_name(row.instantiation));
                    value["reason"] = json!(row.reason);
                    Ok(value)
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
        }
    }
}

pub(crate) fn requests_facts(stage: &Stage) -> bool {
    matches!(
        stage.kind,
        StageKind::References
            | StageKind::Declarations
            | StageKind::Uses
            | StageKind::TransitiveReferences(_)
            | StageKind::Types
            | StageKind::Instantiations
    ) || matches!(&stage.kind, StageKind::Filter(predicate) if predicate_facts(predicate))
}
fn predicate_facts(predicate: &Predicate) -> bool {
    match &predicate.kind {
        PredicateKind::Type(_) | PredicateKind::Instantiation(_) | PredicateKind::SourceKind(_) => {
            true
        }
        PredicateKind::Not(p) | PredicateKind::Group(p) => predicate_facts(p),
        PredicateKind::And(ps) | PredicateKind::Or(ps) => ps.iter().any(predicate_facts),
        _ => false,
    }
}

pub(crate) fn evaluate<'a>(
    query: &Query,
    input: &'a Input,
    model: Option<&'a ModelContext>,
    limits: Limits,
) -> Result<QueryResult<'a>, QueryError> {
    let requesting = query.stages.iter().find(|s| requests_facts(s)).unwrap();
    let model = model.ok_or_else(|| {
        QueryError::query(
            requesting.range.clone(),
            "semantic inspection requires a retained model context (--model for stdin)",
        )
    })?;
    if let Some(error) = model.errors.first() {
        return Err(model_error(error.location.clone(), &error.message));
    }
    if input.mode != FileMode::Model {
        return Err(QueryError::query(
            requesting.range.clone(),
            "semantic inspection requires model-file mode (.mzn)",
        ));
    }
    let root = model
        .root_file
        .and_then(|id| model.files.get(id))
        .ok_or_else(|| {
            QueryError::query(
                requesting.range.clone(),
                "semantic inspection requires a retained root file",
            )
        })?;
    if input.syntax_offset != root.byte_offset
        || input.bytes[input.syntax_offset..] != *root.source_bytes()
    {
        return Err(QueryError::input(
            0..input.bytes.len().min(3),
            "semantic input must exactly match the retained model root's original bytes, including its BOM",
        ));
    }
    let mut work = Work {
        remaining: limits.work,
    };
    let selection = Selection {
        input,
        items: crate::collect_items(input, limits.collection, &mut work, &query.range, true)?,
        document: true,
    };
    let mut report = SemanticResult {
        model,
        bindings: None,
        callables: None,
        instantiations: None,
        stream: SemanticStream::Inspection(Box::new(QueryResult::Selection(selection))),
        limitations: Vec::new(),
        json: false,
    };
    let mut callable_references = Vec::new();
    for limitation in &model.limitations {
        report.limit(
            limitation.location.clone(),
            &limitation.message,
            &mut work,
            requesting,
            limits,
        )?;
    }
    for stage in &query.stages {
        work.visit(&stage.range)?;
        let stream = std::mem::replace(&mut report.stream, SemanticStream::Entities(Vec::new()));
        report.stream = match &stage.kind {
            StageKind::References => {
                report.ensure_bindings(&mut work, stage, limits)?;
                let subjects = subjects(stream, model, stage, limits, &mut work)?;
                SemanticStream::Entities(report.references(
                    &subjects,
                    &mut work,
                    stage,
                    limits,
                    &mut callable_references,
                )?)
            }
            StageKind::Declarations => {
                report.ensure_bindings(&mut work, stage, limits)?;
                let subjects = subjects(stream, model, stage, limits, &mut work)?;
                SemanticStream::Entities(report.declarations(&subjects, &mut work, stage, limits)?)
            }
            StageKind::Uses => {
                report.ensure_bindings(&mut work, stage, limits)?;
                let subjects = subjects(stream, model, stage, limits, &mut work)?;
                let declarations = report.declarations(&subjects, &mut work, stage, limits)?;
                SemanticStream::Entities(report.uses(&declarations, &mut work, stage, limits)?)
            }
            StageKind::TransitiveReferences(depth) => {
                report.ensure_bindings(&mut work, stage, limits)?;
                let subjects = subjects(stream, model, stage, limits, &mut work)?;
                let declarations = report.declarations(&subjects, &mut work, stage, limits)?;
                SemanticStream::Entities(report.transitive(
                    &declarations,
                    *depth,
                    &mut work,
                    stage,
                    limits,
                )?)
            }
            StageKind::Types => {
                let subjects = subjects(stream, model, stage, limits, &mut work)?;
                let subjects = report.typed_subjects(subjects, &mut work, stage, limits)?;
                let mut rows = Vec::new();
                for subject in subjects {
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    let ty = report.type_of(&subject, &mut work, stage, limits)?;
                    rows.push(SemanticType { subject, ty });
                }
                SemanticStream::Types(rows)
            }
            StageKind::Instantiations => {
                let subjects = subjects(stream, model, stage, limits, &mut work)?;
                let subjects = report.typed_subjects(subjects, &mut work, stage, limits)?;
                let mut rows = Vec::new();
                for subject in subjects {
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    let (instantiation, reason) =
                        report.instantiation_of(&subject, &mut work, stage, limits)?;
                    rows.push(SemanticInstantiation {
                        subject,
                        instantiation,
                        reason,
                    });
                }
                SemanticStream::Instantiations(rows)
            }
            StageKind::Filter(predicate)
                if predicate_facts(predicate)
                    || !matches!(stream, SemanticStream::Inspection(_)) =>
            {
                report.filter(stream, predicate, &mut work, stage, limits)?
            }
            StageKind::Head(count) => match stream {
                SemanticStream::Entities(mut rows) => {
                    rows.truncate(*count);
                    SemanticStream::Entities(rows)
                }
                SemanticStream::Types(mut rows) => {
                    rows.truncate(*count);
                    SemanticStream::Types(rows)
                }
                SemanticStream::Instantiations(mut rows) => {
                    rows.truncate(*count);
                    SemanticStream::Instantiations(rows)
                }
                SemanticStream::Inspection(result) => SemanticStream::Inspection(Box::new(
                    crate::evaluate_stage(input, Some(model), *result, stage, &mut work, limits)?,
                )),
            },
            StageKind::Count => match stream {
                SemanticStream::Inspection(result) => SemanticStream::Inspection(Box::new(
                    crate::evaluate_stage(input, Some(model), *result, stage, &mut work, limits)?,
                )),
                other => {
                    SemanticStream::Inspection(Box::new(QueryResult::Count(stream_len(&other))))
                }
            },
            StageKind::Unique if !matches!(stream, SemanticStream::Inspection(_)) => {
                unique(stream, &mut work, stage)?
            }
            StageKind::Names | StageKind::Text
                if !matches!(stream, SemanticStream::Inspection(_)) =>
            {
                let subjects = subjects(stream, model, stage, limits, &mut work)?;
                let mut values = Vec::new();
                for subject in subjects {
                    let names = if matches!(stage.kind, StageKind::Text) {
                        let location = report.subject_location(&subject);
                        vec![
                            std::str::from_utf8(report.subject_bytes(&subject)).map_err(|_| {
                                model_error(location, "text inspection requires UTF-8 source text")
                            })?,
                        ]
                    } else {
                        match &subject {
                            SemanticSubject::Entity(entity) => vec![report.name(*entity)],
                            SemanticSubject::Node { file, node, .. } => {
                                work.spend(node.children().len(), &stage.range)?;
                                crate::inspection::direct_names(&model.files[*file].parsed, node)
                            }
                        }
                    };
                    for name in names {
                        check_collection(values.len(), limits.collection, &stage.range)?;
                        work.spend(name.len().max(1), &stage.range)?;
                        values.push(name.to_owned());
                    }
                }
                SemanticStream::Inspection(Box::new(QueryResult::Strings(values)))
            }
            StageKind::Emit if !matches!(stream, SemanticStream::Inspection(_)) => stream,
            StageKind::Json => {
                report.json = true;
                stream
            }
            _ => match stream {
                SemanticStream::Inspection(result) => SemanticStream::Inspection(Box::new(
                    crate::evaluate_stage(input, Some(model), *result, stage, &mut work, limits)?,
                )),
                _ => {
                    return Err(type_error(
                        &stage.range,
                        "stage does not accept this semantic stream",
                    ));
                }
            },
        };
    }
    if let SemanticStream::Inspection(result) = &report.stream
        && let QueryResult::Selection(selection) = result.as_ref()
        && !selection.document
    {
        crate::validate_fragment_directives(input, &selection.items)?;
    }
    report.finish_reference_limitations(callable_references, &mut work, requesting, limits)?;
    // Charge text/JSON output before constructing the rendered value.
    report.charge_output(&mut work, &query.range, limits)?;
    if report.json {
        report.validate_json(&mut work, &query.range)?;
    }
    Ok(QueryResult::Semantic(report))
}

fn subjects<'a>(
    stream: SemanticStream<'a>,
    model: &ModelContext,
    stage: &Stage,
    limits: Limits,
    work: &mut Work,
) -> Result<Vec<SemanticSubject<'a>>, QueryError> {
    let mut subjects = Vec::new();
    match stream {
        SemanticStream::Entities(rows) => {
            for entity in rows {
                check_collection(subjects.len(), limits.collection, &stage.range)?;
                work.visit(&stage.range)?;
                subjects.push(SemanticSubject::Entity(entity));
            }
        }
        SemanticStream::Types(rows) => {
            for row in rows {
                work.visit(&stage.range)?;
                subjects.push(row.subject);
            }
        }
        SemanticStream::Instantiations(rows) => {
            for row in rows {
                work.visit(&stage.range)?;
                subjects.push(row.subject);
            }
        }
        SemanticStream::Inspection(result) => {
            let nodes = crate::inspection::into_nodes(*result, &stage.range)?;
            for selected in nodes.nodes {
                work.visit(&stage.range)?;
                check_collection(subjects.len(), limits.collection, &stage.range)?;
                subjects.push(SemanticSubject::Node {
                    file: model.root_file.unwrap(),
                    node: selected.node,
                    location: model.files[model.root_file.unwrap()].location(selected.node.range()),
                });
            }
        }
    }
    Ok(subjects)
}

impl SemanticResult<'_> {
    // Public producers operate on a whole context. Bound that scan before asking
    // them to allocate facts; no second resolver or partial producer is introduced.
    fn fact_scan(&self, work: &mut Work, stage: &Stage, limits: Limits) -> Result<(), QueryError> {
        let mut entries = 0usize;
        for source in &self.model.files {
            work.spend(source.parsed.tokens().len(), &stage.range)?;
            entries = entries.saturating_add(source.parsed.tokens().len());
            if entries >= limits.collection {
                return Err(type_error(
                    &stage.range,
                    "query collection limit exceeded while scanning model facts",
                ));
            }
            let mut pending = vec![source.parsed.tree()];
            while let Some(node) = pending.pop() {
                work.visit(&stage.range)?;
                entries = entries.saturating_add(1);
                if entries > limits.collection {
                    return Err(type_error(
                        &stage.range,
                        "query collection limit exceeded while scanning model facts",
                    ));
                }
                for child in node.child_nodes() {
                    check_collection(pending.len(), limits.collection, &stage.range)?;
                    pending.push(child);
                }
            }
        }
        Ok(())
    }
    fn ensure_bindings(
        &mut self,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<(), QueryError> {
        if self.bindings.is_none() {
            self.fact_scan(work, stage, limits)?;
            self.bindings = Some(resolve_bindings(self.model));
        }
        Ok(())
    }
    fn ensure_callables(
        &mut self,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<(), QueryError> {
        self.ensure_bindings(work, stage, limits)?;
        if self.callables.is_none() {
            self.fact_scan(work, stage, limits)?;
            self.callables = Some(resolve_callables(
                self.model,
                self.bindings.as_ref().unwrap(),
            ));
        }
        Ok(())
    }
    fn ensure_instantiations(
        &mut self,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<(), QueryError> {
        self.ensure_callables(work, stage, limits)?;
        if self.instantiations.is_none() {
            self.fact_scan(work, stage, limits)?;
            self.instantiations = Some(resolve_instantiations(
                self.model,
                self.bindings.as_ref().unwrap(),
                self.callables.as_ref().unwrap(),
            ));
        }
        Ok(())
    }
    fn owning_range(&self, subject: &SemanticSubject<'_>) -> Range<usize> {
        match subject {
            SemanticSubject::Entity(SemanticEntity::Declaration(id)) => {
                let declaration = &self.bindings.as_ref().unwrap().declarations[id.0];
                let offset = self.model.files[declaration.file].byte_offset;
                declaration.syntax_range.start + offset..declaration.syntax_range.end + offset
            }
            _ => self.subject_location(subject).range,
        }
    }
    fn references(
        &mut self,
        subjects: &[SemanticSubject<'_>],
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
        callable_references: &mut Vec<usize>,
    ) -> Result<Vec<SemanticEntity>, QueryError> {
        let mut rows = Vec::new();
        for subject in subjects {
            let file = self.subject_file(subject);
            let region = self.owning_range(subject);
            for id in 0..self.bindings.as_ref().unwrap().references.len() {
                work.visit(&stage.range)?;
                let reference = &self.bindings.as_ref().unwrap().references[id];
                if reference.file == file && contains(&region, &reference.location.range) {
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    rows.push(SemanticEntity::Reference(id));
                    if reference.kind == ReferenceKind::Callable {
                        // Lexical lookup cannot distinguish an unresolved call
                        // from a language intrinsic. Use any callable facts the
                        // pipeline requests before deciding report uncertainty.
                        work.spend(callable_references.len(), &stage.range)?;
                        if !callable_references.contains(&id) {
                            check_collection(
                                callable_references.len(),
                                limits.collection,
                                &stage.range,
                            )?;
                            callable_references.push(id);
                        }
                    } else if matches!(
                        reference.resolution,
                        BindingResolution::Unresolved | BindingResolution::Ambiguous(_)
                    ) {
                        self.limit(
                            reference.location.clone(),
                            format!(
                                "reference '{}' has an unresolved or ambiguous binding",
                                reference.name
                            ),
                            work,
                            stage,
                            limits,
                        )?;
                    }
                }
            }
        }
        Ok(rows)
    }
    fn finish_reference_limitations(
        &mut self,
        callable_references: Vec<usize>,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<(), QueryError> {
        for id in callable_references {
            work.visit(&stage.range)?;
            if self.callables.is_some() {
                // target records actual unknown outcomes and treats resolved
                // declarations and intrinsics as known endpoints.
                self.target(id, work, stage, limits)?;
            } else {
                let reference = &self.bindings.as_ref().unwrap().references[id];
                if matches!(
                    reference.resolution,
                    BindingResolution::Unresolved | BindingResolution::Ambiguous(_)
                ) {
                    self.limit(
                        reference.location.clone(),
                        format!(
                            "reference '{}' has an unresolved or ambiguous binding",
                            reference.name
                        ),
                        work,
                        stage,
                        limits,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn declarations(
        &mut self,
        subjects: &[SemanticSubject<'_>],
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Vec<SemanticEntity>, QueryError> {
        let mut rows = Vec::new();
        for subject in subjects {
            match subject {
                SemanticSubject::Entity(SemanticEntity::Declaration(id)) => {
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    work.visit(&stage.range)?;
                    rows.push(SemanticEntity::Declaration(*id));
                }
                SemanticSubject::Entity(SemanticEntity::Reference(id)) => {
                    if let Some(target) = self.target(*id, work, stage, limits)? {
                        check_collection(rows.len(), limits.collection, &stage.range)?;
                        rows.push(SemanticEntity::Declaration(target));
                    }
                }
                SemanticSubject::Node { file, node, .. } => {
                    for declaration in &self.bindings.as_ref().unwrap().declarations {
                        work.visit(&stage.range)?;
                        if declaration.file == *file && declaration.syntax_range == node.range() {
                            check_collection(rows.len(), limits.collection, &stage.range)?;
                            rows.push(SemanticEntity::Declaration(declaration.id));
                        }
                    }
                }
            }
        }
        Ok(rows)
    }
    fn target(
        &mut self,
        id: usize,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Option<DeclarationId>, QueryError> {
        work.visit(&stage.range)?;
        if self.bindings.as_ref().unwrap().references[id].kind == ReferenceKind::Callable {
            self.ensure_callables(work, stage, limits)?;
        }
        let reference = &self.bindings.as_ref().unwrap().references[id];
        let location = reference.location.clone();
        let mut reason = format!(
            "reference '{}' has no proven declaration target",
            reference.name
        );
        if reference.kind == ReferenceKind::Callable {
            for call in &self.callables.as_ref().unwrap().calls {
                work.visit(&stage.range)?;
                if call.file != reference.file || call.location.range.start != location.range.start
                {
                    continue;
                }
                match &call.outcome {
                    CallOutcome::Resolved { declaration, .. } => return Ok(Some(*declaration)),
                    CallOutcome::Intrinsic { .. } => return Ok(None),
                    CallOutcome::NoMatch { reason: r }
                    | CallOutcome::Unresolved { reason: r }
                    | CallOutcome::Unsupported { reason: r, .. } => reason = r.clone(),
                    CallOutcome::Ambiguous { .. } => {
                        reason = format!("call '{}' is ambiguous", reference.name)
                    }
                }
                break;
            }
        } else if let BindingResolution::Resolved(declaration) = reference.resolution {
            return Ok(Some(declaration));
        }
        self.limit(location, reason, work, stage, limits)?;
        Ok(None)
    }
    fn uses(
        &mut self,
        declarations: &[SemanticEntity],
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Vec<SemanticEntity>, QueryError> {
        let mut rows = Vec::new();
        // Uncertain occurrences matter even when the requested declaration set is empty.
        let mut targets = Vec::new();
        for id in 0..self.bindings.as_ref().unwrap().references.len() {
            check_collection(targets.len(), limits.collection, &stage.range)?;
            targets.push(self.target(id, work, stage, limits)?);
        }
        for declaration in declarations {
            let SemanticEntity::Declaration(declaration) = declaration else {
                unreachable!()
            };
            for (id, target) in targets.iter().enumerate() {
                work.visit(&stage.range)?;
                if *target == Some(*declaration) {
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    rows.push(SemanticEntity::Reference(id));
                }
            }
        }
        Ok(rows)
    }
    fn transitive(
        &mut self,
        declarations: &[SemanticEntity],
        depth: usize,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Vec<SemanticEntity>, QueryError> {
        if depth == 0 {
            return Ok(Vec::new());
        }
        let mut frontier = Vec::new();
        let mut visited = HashSet::new();
        for entity in declarations {
            let SemanticEntity::Declaration(id) = entity else {
                unreachable!()
            };
            work.visit(&stage.range)?;
            if !visited.contains(&id.0) {
                check_collection(visited.len(), limits.collection, &stage.range)?;
                visited.insert(id.0);
                frontier.push(*id);
            }
        }
        let references = &self.bindings.as_ref().unwrap().references;
        work.spend(
            references
                .len()
                .saturating_mul(references.len().max(1).ilog2() as usize + 1),
            &stage.range,
        )?;
        if references.len() > limits.collection {
            return Err(type_error(&stage.range, "query collection limit exceeded"));
        }
        let mut order: Vec<_> = (0..references.len()).collect();
        order.sort_by_key(|&id| {
            let r = &references[id];
            (
                r.file,
                r.location.range.start,
                r.location.range.end,
                reference_order(r.kind),
            )
        });
        let mut emitted = HashSet::new();
        let mut rows = Vec::new();
        for _ in 0..depth {
            if frontier.is_empty() {
                break;
            }
            let mut next = Vec::new();
            for declaration in frontier {
                work.visit(&stage.range)?;
                let declaration = &self.bindings.as_ref().unwrap().declarations[declaration.0];
                let file = declaration.file;
                let offset = self.model.files[file].byte_offset;
                let region =
                    declaration.syntax_range.start + offset..declaration.syntax_range.end + offset;
                for &id in &order {
                    work.visit(&stage.range)?;
                    let reference = &self.bindings.as_ref().unwrap().references[id];
                    if reference.file != file || !contains(&region, &reference.location.range) {
                        continue;
                    }
                    let identity = (
                        file,
                        reference.location.range.start,
                        reference.location.range.end,
                        reference_order(reference.kind),
                    );
                    if !emitted.contains(&identity) {
                        check_collection(rows.len(), limits.collection, &stage.range)?;
                        emitted.insert(identity);
                        rows.push(SemanticEntity::Reference(id));
                    }
                    if let Some(target) = self.target(id, work, stage, limits)?
                        && !visited.contains(&target.0)
                    {
                        check_collection(visited.len(), limits.collection, &stage.range)?;
                        visited.insert(target.0);
                        next.push(target);
                    }
                }
            }
            frontier = next;
        }
        Ok(rows)
    }

    fn typed_subjects<'a>(
        &mut self,
        subjects: Vec<SemanticSubject<'a>>,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Vec<SemanticSubject<'a>>, QueryError> {
        self.ensure_bindings(work, stage, limits)?;
        let mut rows = Vec::new();
        for subject in subjects {
            let before = rows.len();
            if let SemanticSubject::Node { file, node, .. } = &subject {
                for declaration in &self.bindings.as_ref().unwrap().declarations {
                    work.visit(&stage.range)?;
                    if declaration.file == *file && declaration.syntax_range == node.range() {
                        check_collection(rows.len(), limits.collection, &stage.range)?;
                        rows.push(SemanticSubject::Entity(SemanticEntity::Declaration(
                            declaration.id,
                        )));
                    }
                }
            }
            if rows.len() == before {
                check_collection(rows.len(), limits.collection, &stage.range)?;
                rows.push(subject);
            }
        }
        Ok(rows)
    }

    fn type_of(
        &mut self,
        subject: &SemanticSubject<'_>,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Option<TypeInst>, QueryError> {
        self.ensure_callables(work, stage, limits)?;
        let declaration = self.declared_subject(subject, work, stage)?;
        let facts = self.callables.as_ref().unwrap();
        let mut ty = None;
        if let Some(id) = declaration {
            for fact in &facts.declarations {
                work.visit(&stage.range)?;
                if fact.declaration == id {
                    ty = Some(&fact.ty);
                    break;
                }
            }
        } else {
            let file = self.subject_file(subject);
            let location = self.subject_location(subject);
            // A different expression never inherits its target declaration's type.
            for fact in &facts.expressions {
                work.visit(&stage.range)?;
                if fact.file == file && fact.location.range == location.range {
                    ty = Some(&fact.ty);
                    break;
                }
            }
            if ty.is_none()
                && let SemanticSubject::Entity(SemanticEntity::Reference(id)) = subject
                && self.bindings.as_ref().unwrap().references[*id].kind == ReferenceKind::Callable
            {
                for fact in &facts.calls {
                    work.visit(&stage.range)?;
                    if fact.file == file && fact.location.range.start == location.range.start {
                        ty = match &fact.outcome {
                            CallOutcome::Resolved { return_type, .. }
                            | CallOutcome::Intrinsic { return_type, .. } => Some(return_type),
                            _ => None,
                        };
                        break;
                    }
                }
            }
        }
        let reason = if let Some(ty) = ty {
            type_unknown(ty, work, &stage.range, limits)?
        } else {
            Some("type fact unavailable for this exact occurrence".to_owned())
        };
        let ty = ty.cloned();
        if let Some(reason) = reason {
            self.limit(self.subject_location(subject), reason, work, stage, limits)?;
        }
        Ok(ty)
    }

    fn declared_subject(
        &self,
        subject: &SemanticSubject<'_>,
        work: &mut Work,
        stage: &Stage,
    ) -> Result<Option<DeclarationId>, QueryError> {
        match subject {
            SemanticSubject::Entity(SemanticEntity::Declaration(id)) => Ok(Some(*id)),
            SemanticSubject::Node { file, node, .. } => {
                // This is the declaration's own node, never an expression with
                // a coincident name. Shared nodes are expanded by typed stages.
                for declaration in &self.bindings.as_ref().unwrap().declarations {
                    work.visit(&stage.range)?;
                    if declaration.file == *file && declaration.syntax_range == node.range() {
                        return Ok(Some(declaration.id));
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn instantiation_of(
        &mut self,
        subject: &SemanticSubject<'_>,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<(Instantiation, Option<String>), QueryError> {
        let mut outcome = (
            Instantiation::Unknown,
            Some("instantiation fact unavailable for this exact occurrence".to_owned()),
        );
        self.ensure_bindings(work, stage, limits)?;
        if let Some(id) = self.declared_subject(subject, work, stage)? {
            work.visit(&stage.range)?;
            let instantiation = self.bindings.as_ref().unwrap().declarations[id.0].instantiation;
            outcome = (
                instantiation,
                (instantiation == Instantiation::Unknown)
                    .then(|| "declared instantiation is unknown".to_owned()),
            );
        } else {
            self.ensure_instantiations(work, stage, limits)?;
            let location = self.subject_location(subject);
            let file = self.subject_file(subject);
            for fact in &self.instantiations.as_ref().unwrap().expressions {
                work.visit(&stage.range)?;
                if fact.file == file && fact.location.range == location.range {
                    outcome = (fact.instantiation, fact.reason.clone());
                    break;
                }
            }
            if outcome.0 == Instantiation::Unknown
                && let SemanticSubject::Entity(SemanticEntity::Reference(id)) = subject
                && self.bindings.as_ref().unwrap().references[*id].kind == ReferenceKind::Callable
            {
                for fact in &self.callables.as_ref().unwrap().calls {
                    work.visit(&stage.range)?;
                    if fact.file == file && fact.location.range.start == location.range.start {
                        match &fact.outcome {
                            CallOutcome::Resolved { return_type, .. }
                            | CallOutcome::Intrinsic { return_type, .. } => {
                                outcome = (
                                    return_type.instantiation,
                                    (return_type.instantiation == Instantiation::Unknown)
                                        .then(|| "call return instantiation is unknown".to_owned()),
                                );
                            }
                            _ => {}
                        }
                        break;
                    }
                }
            }
        }
        if let Some(reason) = &outcome.1 {
            self.limit(self.subject_location(subject), reason, work, stage, limits)?;
        }
        Ok(outcome)
    }

    fn predicate(
        &mut self,
        predicate: &Predicate,
        subject: &SemanticSubject<'_>,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Truth, QueryError> {
        work.visit(&predicate.range)?;
        Ok(match &predicate.kind {
            PredicateKind::Group(p) => self.predicate(p, subject, work, stage, limits)?,
            PredicateKind::Not(p) => self.predicate(p, subject, work, stage, limits)?.not(),
            PredicateKind::And(ps) | PredicateKind::Or(ps) => {
                let conjunction = matches!(predicate.kind, PredicateKind::And(_));
                let mut unknown = false;
                for p in ps {
                    let truth = self.predicate(p, subject, work, stage, limits)?;
                    if conjunction && truth == Truth::False {
                        return Ok(Truth::False);
                    }
                    if !conjunction && truth == Truth::True {
                        return Ok(Truth::True);
                    }
                    unknown |= truth == Truth::Unknown;
                }
                if unknown {
                    Truth::Unknown
                } else {
                    Truth::from(conjunction)
                }
            }
            PredicateKind::Type(expected) => match self.type_of(subject, work, stage, limits)? {
                Some(ty) if type_unknown(&ty, work, &predicate.range, limits)?.is_none() => {
                    Truth::from(type_kind(&ty.kind) == expected)
                }
                _ => Truth::Unknown,
            },
            PredicateKind::Instantiation(expected) => {
                let (instantiation, _) = self.instantiation_of(subject, work, stage, limits)?;
                if instantiation == Instantiation::Unknown {
                    Truth::Unknown
                } else {
                    Truth::from(instantiation_name(instantiation) == expected)
                }
            }
            PredicateKind::SourceKind(expected) => Truth::from(
                source_kind(self.model.files[self.subject_file(subject)].kind) == expected,
            ),
            PredicateKind::Name(expected) => match subject {
                SemanticSubject::Entity(entity) => {
                    Truth::from(self.name(*entity) == crate::identifier_identity(expected))
                }
                SemanticSubject::Node { file, node, .. } => {
                    work.spend(node.children().len(), &predicate.range)?;
                    Truth::from(
                        crate::inspection::direct_names(&self.model.files[*file].parsed, node)
                            .first()
                            .copied()
                            == Some(crate::identifier_identity(expected)),
                    )
                }
            },
            PredicateKind::Kind(expected) => match subject {
                SemanticSubject::Entity(SemanticEntity::Reference(_)) => {
                    Truth::from(expected == "reference")
                }
                SemanticSubject::Entity(SemanticEntity::Declaration(_)) => {
                    Truth::from(expected == "declaration")
                }
                SemanticSubject::Node { node, .. } => Truth::from(
                    kind_name(node.kind()) == *expected
                        || crate::ItemKind::parse(expected)
                            .is_some_and(|kind| kind.as_str() == item_kind_name(node.kind())),
                ),
            },
        })
    }
    fn filter<'a>(
        &mut self,
        stream: SemanticStream<'a>,
        predicate: &Predicate,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<SemanticStream<'a>, QueryError> {
        // Keep item attachments for syntax streams, and row facts for typed streams.
        macro_rules! retain_rows {
            ($rows:expr, $subject:expr, $variant:ident) => {{
                let mut retained = Vec::new();
                for row in $rows {
                    work.visit(&stage.range)?;
                    let subject = $subject(&row);
                    if self.predicate(predicate, &subject, work, stage, limits)? == Truth::True {
                        retained.push(row);
                    }
                }
                SemanticStream::$variant(retained)
            }};
        }
        Ok(match stream {
            SemanticStream::Entities(rows) => retain_rows!(
                rows,
                |row: &SemanticEntity| SemanticSubject::Entity(*row),
                Entities
            ),
            SemanticStream::Types(rows) => {
                retain_rows!(rows, |row: &SemanticType<'a>| row.subject.clone(), Types)
            }
            SemanticStream::Instantiations(rows) => retain_rows!(
                rows,
                |row: &SemanticInstantiation<'a>| row.subject.clone(),
                Instantiations
            ),
            SemanticStream::Inspection(result) => {
                let root = self.model.root_file.unwrap();
                let file = &self.model.files[root];
                let result = match *result {
                    QueryResult::Selection(mut selection) => {
                        let mut retained = Vec::new();
                        for item in selection.items {
                            let subject = SemanticSubject::Node {
                                file: root,
                                node: item.node,
                                location: file.location(item.node.range()),
                            };
                            if self.predicate(predicate, &subject, work, stage, limits)?
                                == Truth::True
                            {
                                retained.push(item);
                            }
                        }
                        selection.items = retained;
                        selection.document = false;
                        QueryResult::Selection(selection)
                    }
                    QueryResult::Nodes(mut selection) => {
                        let mut retained = Vec::new();
                        for node in selection.nodes {
                            let subject = SemanticSubject::Node {
                                file: root,
                                node: node.node,
                                location: file.location(node.node.range()),
                            };
                            if self.predicate(predicate, &subject, work, stage, limits)?
                                == Truth::True
                            {
                                retained.push(node);
                            }
                        }
                        selection.nodes = retained;
                        QueryResult::Nodes(selection)
                    }
                    _ => {
                        return Err(type_error(
                            &stage.range,
                            "filter requires selected nodes or semantic rows",
                        ));
                    }
                };
                SemanticStream::Inspection(Box::new(result))
            }
        })
    }
    fn charge_subject_json(
        &self,
        subject: &SemanticSubject<'_>,
        work: &mut Work,
        range: &Range<usize>,
    ) -> Result<(), QueryError> {
        if !self.json {
            return Ok(());
        }
        let file = &self.model.files[self.subject_file(subject)];
        work.spend(file.path.as_os_str().len(), range)?;
        if let SemanticSubject::Entity(entity) = subject {
            work.spend(self.name(*entity).len(), range)?;
            if let SemanticEntity::Reference(id) = entity {
                let reference = &self.bindings.as_ref().unwrap().references[*id];
                if let BindingResolution::Overloads(ids) | BindingResolution::Ambiguous(ids) =
                    &reference.resolution
                {
                    work.spend(ids.len(), range)?;
                }
                if reference.kind == ReferenceKind::Callable
                    && let Some(callables) = &self.callables
                {
                    for call in &callables.calls {
                        work.visit(range)?;
                        if call.file == reference.file
                            && call.location.range.start == reference.location.range.start
                        {
                            match &call.outcome {
                                CallOutcome::Ambiguous { candidates }
                                | CallOutcome::Unsupported { candidates, .. } => {
                                    work.spend(candidates.len(), range)?
                                }
                                _ => {}
                            }
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn charge_output(
        &self,
        work: &mut Work,
        range: &Range<usize>,
        limits: Limits,
    ) -> Result<(), QueryError> {
        if self.json {
            for limitation in &self.limitations {
                work.visit(range)?;
                work.spend(
                    limitation.location.path.as_os_str().len() + limitation.message.len(),
                    range,
                )?;
            }
        }
        match &self.stream {
            SemanticStream::Inspection(result) => match result.as_ref() {
                QueryResult::Selection(selection) => {
                    for item in &selection.items {
                        work.spend(item.range.len().max(1), range)?;
                        if self.json {
                            work.spend(selection.input.file().len(), range)?;
                        }
                    }
                }
                QueryResult::Nodes(selection) => {
                    for node in &selection.nodes {
                        work.spend(node.range.len().max(1), range)?;
                        if self.json {
                            work.spend(selection.input.file().len(), range)?;
                        }
                    }
                }
                QueryResult::Strings(values) => {
                    for value in values {
                        work.spend(value.len().max(1), range)?;
                    }
                }
                QueryResult::Tally(values) => {
                    for value in values.keys() {
                        work.spend(value.len().max(1), range)?;
                    }
                }
                QueryResult::Count(_) => {}
                QueryResult::Literals(selection) => {
                    for value in selection.values() {
                        work.spend(value.range.len(), range)?;
                    }
                }
                _ => {
                    return Err(type_error(
                        range,
                        "semantic inspection cannot produce an edit candidate",
                    ));
                }
            },
            SemanticStream::Entities(rows) => {
                for entity in rows {
                    work.spend(self.source_bytes(*entity).len().max(1), range)?;
                    self.charge_subject_json(&SemanticSubject::Entity(*entity), work, range)?;
                }
            }
            SemanticStream::Types(rows) => {
                for row in rows {
                    work.spend(self.subject_bytes(&row.subject).len().max(1), range)?;
                    self.charge_subject_json(&row.subject, work, range)?;
                    if let Some(ty) = &row.ty {
                        type_unknown(ty, work, range, limits)?;
                    }
                }
            }
            SemanticStream::Instantiations(rows) => {
                for row in rows {
                    work.spend(self.subject_bytes(&row.subject).len().max(1), range)?;
                    self.charge_subject_json(&row.subject, work, range)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    False,
    Unknown,
}
impl Truth {
    fn from(value: bool) -> Self {
        if value { Self::True } else { Self::False }
    }
    fn not(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }
}
fn item_kind_name(kind: zincite_syntax::NodeKind) -> &'static str {
    use zincite_syntax::NodeKind::*;
    match kind {
        Assignment => "assignment",
        Declaration => "declaration",
        EnumDeclaration => "enum",
        TypeAlias => "type_alias",
        FunctionDeclaration => "function",
        PredicateDeclaration => "predicate",
        TestDeclaration => "test",
        AnnotationDeclaration => "annotation",
        Constraint => "constraint",
        Include => "include",
        Output => "output",
        Solve | SolveMinimize | SolveMaximize => "solve",
        _ => "",
    }
}
fn model_error(location: SourceLocation, message: impl Into<String>) -> QueryError {
    QueryError {
        range: location.range.clone(),
        location: ErrorLocation::Model(location),
        message: message.into(),
    }
}
fn contains(region: &Range<usize>, range: &Range<usize>) -> bool {
    region.start <= range.start && range.end <= region.end
}
fn reference_order(kind: ReferenceKind) -> u8 {
    match kind {
        ReferenceKind::Value => 0,
        ReferenceKind::Callable => 1,
        ReferenceKind::Type => 2,
    }
}
fn reference_kind(kind: ReferenceKind) -> &'static str {
    match kind {
        ReferenceKind::Value => "value",
        ReferenceKind::Callable => "callable",
        ReferenceKind::Type => "type",
    }
}
fn source_kind(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::User => "user",
        SourceKind::StandardLibrary => "standard_library",
    }
}
fn instantiation_name(instantiation: Instantiation) -> &'static str {
    match instantiation {
        Instantiation::Parameter => "par",
        Instantiation::Decision => "var",
        Instantiation::Unknown => "unknown",
    }
}
fn stream_len(stream: &SemanticStream<'_>) -> usize {
    match stream {
        SemanticStream::Entities(rows) => rows.len(),
        SemanticStream::Types(rows) => rows.len(),
        SemanticStream::Instantiations(rows) => rows.len(),
        _ => unreachable!(),
    }
}
fn unique<'a>(
    stream: SemanticStream<'a>,
    work: &mut Work,
    stage: &Stage,
) -> Result<SemanticStream<'a>, QueryError> {
    let mut seen = HashSet::new();
    let mut retain = |subject: &SemanticSubject<'_>| -> Result<bool, QueryError> {
        work.visit(&stage.range)?;
        let key = match subject {
            SemanticSubject::Entity(SemanticEntity::Declaration(id)) => (0, id.0, 0, 0),
            SemanticSubject::Entity(SemanticEntity::Reference(id)) => (1, *id, 0, 0),
            SemanticSubject::Node { file, location, .. } => {
                (2, *file, location.range.start, location.range.end)
            }
        };
        Ok(seen.insert(key))
    };
    Ok(match stream {
        SemanticStream::Entities(rows) => {
            let mut result = Vec::new();
            for row in rows {
                if retain(&SemanticSubject::Entity(row))? {
                    result.push(row);
                }
            }
            SemanticStream::Entities(result)
        }
        SemanticStream::Types(rows) => {
            let mut result = Vec::new();
            for row in rows {
                if retain(&row.subject)? {
                    result.push(row);
                }
            }
            SemanticStream::Types(result)
        }
        SemanticStream::Instantiations(rows) => {
            let mut result = Vec::new();
            for row in rows {
                if retain(&row.subject)? {
                    result.push(row);
                }
            }
            SemanticStream::Instantiations(result)
        }
        _ => unreachable!(),
    })
}
pub(crate) fn known_type_kind(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "int"
            | "float"
            | "string"
            | "annotation"
            | "enum"
            | "set"
            | "array"
            | "tuple"
            | "record"
            | "bottom"
    )
}
fn type_kind(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Bool => "bool",
        TypeKind::Int => "int",
        TypeKind::Float => "float",
        TypeKind::String => "string",
        TypeKind::Annotation => "annotation",
        TypeKind::Enum(_) => "enum",
        TypeKind::Set(_) => "set",
        TypeKind::Array { .. } => "array",
        TypeKind::Tuple(_) => "tuple",
        TypeKind::Record(_) => "record",
        TypeKind::Bottom => "bottom",
        TypeKind::Variable { .. } => "variable",
        TypeKind::Unknown(_) => "unknown",
    }
}
fn type_unknown(
    ty: &TypeInst,
    work: &mut Work,
    range: &Range<usize>,
    limits: Limits,
) -> Result<Option<String>, QueryError> {
    check_collection(0, limits.collection, range)?;
    let mut pending = vec![(ty, 0)];
    let mut reason = None;
    while let Some((ty, depth)) = pending.pop() {
        work.visit(range)?;
        if depth > limits.nesting {
            return Err(type_error(range, "query type nesting limit exceeded"));
        }
        let unknown = match &ty.kind {
            TypeKind::Unknown(message) => {
                work.spend(message.len(), range)?;
                Some(message.clone())
            }
            TypeKind::Variable { name, .. } => {
                work.spend(name.len(), range)?;
                Some(format!("type variable '{name}' is unresolved"))
            }
            _ if ty.instantiation == Instantiation::Unknown => {
                Some("type instantiation is unknown".to_owned())
            }
            _ => None,
        };
        if reason.is_none() {
            reason = unknown;
        }
        let mut push = |component| -> Result<(), QueryError> {
            check_collection(pending.len(), limits.collection, range)?;
            pending.push((component, depth + 1));
            Ok(())
        };
        match &ty.kind {
            TypeKind::Set(element) => push(element.as_ref())?,
            TypeKind::Array { indices, element } => {
                push(element.as_ref())?;
                for index in indices.iter().rev() {
                    push(index)?;
                }
            }
            TypeKind::Tuple(fields) => {
                for field in fields.iter().rev() {
                    push(field)?;
                }
            }
            TypeKind::Record(fields) => {
                for (name, field) in fields.iter().rev() {
                    work.spend(name.len(), range)?;
                    push(field)?;
                }
            }
            _ => {}
        }
    }
    Ok(reason)
}

fn range_json(range: &Range<usize>) -> Value {
    json!({"start": range.start, "end": range.end})
}
fn binding_json(binding: &BindingResolution) -> Value {
    match binding {
        BindingResolution::Resolved(id) => json!({"outcome": "resolved", "declaration_id": id.0}),
        BindingResolution::Overloads(ids) => {
            json!({"outcome": "overloads", "candidates": ids.iter().map(|id| id.0).collect::<Vec<_>>()})
        }
        BindingResolution::Ambiguous(ids) => {
            json!({"outcome": "ambiguous", "candidates": ids.iter().map(|id| id.0).collect::<Vec<_>>()})
        }
        BindingResolution::Unresolved => json!({"outcome": "unresolved"}),
    }
}
fn call_json(call: &CallOutcome) -> Value {
    match call {
        CallOutcome::Resolved { declaration, .. } => {
            json!({"outcome": "resolved", "declaration_id": declaration.0})
        }
        CallOutcome::Intrinsic { name, .. } => json!({"outcome": "intrinsic", "name": name}),
        CallOutcome::NoMatch { reason } => json!({"outcome": "no_match", "reason": reason}),
        CallOutcome::Unresolved { reason } => json!({"outcome": "unresolved", "reason": reason}),
        CallOutcome::Ambiguous { candidates } => {
            json!({"outcome": "ambiguous", "candidates": candidates.iter().map(|id| id.0).collect::<Vec<_>>()})
        }
        CallOutcome::Unsupported { reason, candidates } => {
            json!({"outcome": "unsupported", "reason": reason, "candidates": candidates.iter().map(|id| id.0).collect::<Vec<_>>()})
        }
    }
}
fn type_json(ty: &TypeInst) -> Value {
    let mut value = json!({"kind": type_kind(&ty.kind), "instantiation": instantiation_name(ty.instantiation), "optional": ty.optional});
    match &ty.kind {
        TypeKind::Enum(id) => value["declaration_id"] = json!(id.0),
        TypeKind::Set(element) => value["element"] = type_json(element),
        TypeKind::Array { indices, element } => {
            value["indices"] = json!(indices.iter().map(type_json).collect::<Vec<_>>());
            value["element"] = type_json(element);
        }
        TypeKind::Tuple(fields) => {
            value["fields"] = json!(fields.iter().map(type_json).collect::<Vec<_>>())
        }
        TypeKind::Record(fields) => {
            value["fields"] = json!(
                fields
                    .iter()
                    .map(|(name, ty)| json!({"name": name, "type": type_json(ty)}))
                    .collect::<Vec<_>>()
            )
        }
        TypeKind::Variable {
            name,
            enum_only,
            any,
        } => {
            value["name"] = json!(name);
            value["enum_only"] = json!(enum_only);
            value["any"] = json!(any);
        }
        TypeKind::Unknown(reason) => value["reason"] = json!(reason),
        _ => {}
    }
    value
}
