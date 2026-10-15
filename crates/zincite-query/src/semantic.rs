//! Read-only navigation over the existing public model facts.
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;

use serde_json::{Value, json};
use zincite_lint::{
    BindingFacts, BindingResolution, CallOutcome, CallableFacts, DeclarationId, FileId,
    Instantiation, InstantiationFacts, ModelContext, ModelFacts, ReferenceKind, SourceDiagnostic,
    SourceKind, SourceLocation, TypeInst, TypeKind,
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
    facts: Rc<ModelFacts<'a>>,
    bindings_requested: bool,
    callables_requested: bool,
    instantiations_requested: bool,
    body_callables: Option<CallableFacts>,
    reference_order: Option<Vec<usize>>,
    pub(crate) stream: SemanticStream<'a>,
    limitations: Vec<SourceDiagnostic>,
    json: bool,
    pub(crate) incomplete_observation: Option<std::sync::Arc<QueryError>>,
}

enum ReferenceEndpoint {
    Declaration(DeclarationId),
    Intrinsic,
    Uncertain(SourceLocation, String),
}

impl<'a> SemanticResult<'a> {
    pub fn model(&self) -> &'a ModelContext {
        self.model
    }
    pub fn bindings(&self) -> Option<&BindingFacts> {
        self.bindings_requested.then(|| self.facts.bindings())
    }
    pub fn callables(&self) -> Option<&CallableFacts> {
        self.callables_requested.then(|| {
            self.body_callables
                .as_ref()
                .unwrap_or_else(|| self.facts.source_callables())
        })
    }
    pub fn instantiation_facts(&self) -> Option<&InstantiationFacts> {
        self.instantiations_requested
            .then(|| self.facts.instantiations())
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
                &self.bindings().unwrap().declarations[id.0].location
            }
            SemanticEntity::Reference(id) => &self.bindings().unwrap().references[id].location,
        }
    }
    pub fn file(&self, entity: SemanticEntity) -> FileId {
        match entity {
            SemanticEntity::Declaration(id) => self.bindings().unwrap().declarations[id.0].file,
            SemanticEntity::Reference(id) => self.bindings().unwrap().references[id].file,
        }
    }
    /// Source of a declaration's owning node, or an exact written reference token.
    pub fn source_bytes(&self, entity: SemanticEntity) -> &[u8] {
        let file = &self.model.files[self.file(entity)];
        let range = match entity {
            SemanticEntity::Declaration(id) => self.bindings().unwrap().declarations[id.0]
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
            SemanticEntity::Declaration(id) => &self.bindings().unwrap().declarations[id.0].name,
            SemanticEntity::Reference(id) => &self.bindings().unwrap().references[id].name,
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
    pub(crate) fn subject_bytes(&self, subject: &SemanticSubject<'_>) -> &[u8] {
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
        work.produce(1, &stage.range)?;
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
        self.render_stream(&self.stream)
    }

    pub(crate) fn source_output_bytes(&self, stream: &SemanticStream<'a>) -> usize {
        match stream {
            SemanticStream::Entities(rows) => {
                rows.iter().map(|&row| self.source_bytes(row).len()).sum()
            }
            SemanticStream::Types(rows) => rows
                .iter()
                .map(|row| {
                    row.ty
                        .as_ref()
                        .map_or("unknown", |ty| type_kind(&ty.kind))
                        .len()
                        + 1
                })
                .sum(),
            SemanticStream::Instantiations(rows) => rows
                .iter()
                .map(|row| instantiation_name(row.instantiation).len() + 1)
                .sum(),
            SemanticStream::Inspection(_) => {
                unreachable!("inspection renderer accounts its own leaves")
            }
        }
    }

    pub(crate) fn render_stream(&self, stream: &SemanticStream<'a>) -> Vec<u8> {
        match stream {
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

    pub(crate) fn admit_program_json(
        &self,
        stream: &SemanticStream<'a>,
        work: &mut Work,
        range: &Range<usize>,
    ) -> Result<(), QueryError> {
        match stream {
            SemanticStream::Inspection(value) => crate::inspection::admit_json(value, work, range)?,
            SemanticStream::Entities(rows) => {
                work.produce(1, range)?;
                for &row in rows {
                    self.admit_subject_program_json(&SemanticSubject::Entity(row), work, range)?;
                }
            }
            SemanticStream::Types(rows) => {
                work.produce(1, range)?;
                for row in rows {
                    let (units, bytes) = row.ty.as_ref().map_or((1, 0), type_allocation);
                    work.produce(units * 16 + 2, range)?;
                    work.spend(bytes + units * 64, range)?;
                    self.admit_subject_program_json(&row.subject, work, range)?;
                }
            }
            SemanticStream::Instantiations(rows) => {
                work.produce(1, range)?;
                for row in rows {
                    self.admit_subject_program_json(&row.subject, work, range)?;
                    work.produce(4, range)?;
                    work.spend(row.reason.as_ref().map_or(0, String::len), range)?;
                }
            }
        }
        Ok(())
    }

    fn admit_subject_program_json(
        &self,
        subject: &SemanticSubject<'_>,
        work: &mut Work,
        range: &Range<usize>,
    ) -> Result<(), QueryError> {
        work.produce(36, range)?;
        work.spend(
            256 + self.model.files[self.subject_file(subject)]
                .path
                .as_os_str()
                .len()
                + self.subject_bytes(subject).len(),
            range,
        )?;
        if let SemanticSubject::Entity(entity) = subject {
            work.spend(self.name(*entity).len(), range)?;
            if let SemanticEntity::Reference(id) = entity {
                let reference = &self.bindings().unwrap().references[*id];
                if let BindingResolution::Overloads(ids) | BindingResolution::Ambiguous(ids) =
                    &reference.resolution
                {
                    work.produce(ids.len(), range)?;
                }
                if reference.kind == ReferenceKind::Callable
                    && let Some(callables) = self.callables()
                {
                    for call in &callables.calls {
                        work.visit(range)?;
                        if call.file == reference.file
                            && call.location.range.start == reference.location.range.start
                        {
                            match &call.outcome {
                                CallOutcome::Ambiguous { candidates } => {
                                    work.produce(candidates.len(), range)?
                                }
                                CallOutcome::Unsupported { candidates, reason } => {
                                    work.produce(candidates.len(), range)?;
                                    work.spend(reason.len(), range)?;
                                }
                                CallOutcome::NoMatch { reason }
                                | CallOutcome::Unresolved { reason } => {
                                    work.spend(reason.len(), range)?
                                }
                                CallOutcome::Intrinsic { name, .. } => {
                                    work.spend(name.len(), range)?
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
                        let declaration = &self.bindings().unwrap().declarations[id.0];
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
                        let reference = &self.bindings().unwrap().references[*id];
                        value["kind"] = json!("reference");
                        value["reference_id"] = json!(id);
                        value["reference_kind"] = json!(reference_kind(reference.kind));
                        value["binding"] = binding_json(&reference.resolution);
                        if let Some(callables) = self.callables()
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
        self.json_stream(&self.stream)
    }

    pub(crate) fn json_stream(&self, stream: &SemanticStream<'a>) -> Result<Value, QueryError> {
        match stream {
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
            | StageKind::ReferencedDeclarations
            | StageKind::Types
            | StageKind::Instantiations
    ) || matches!(&stage.kind, StageKind::Filter(predicate) if predicate_facts(predicate))
}
pub(crate) fn predicate_facts(predicate: &Predicate) -> bool {
    match &predicate.kind {
        PredicateKind::Type(_) | PredicateKind::Instantiation(_) | PredicateKind::SourceKind(_) => {
            true
        }
        PredicateKind::Not(p) | PredicateKind::Group(p) => predicate_facts(p),
        PredicateKind::And(ps) | PredicateKind::Or(ps) => ps.iter().any(predicate_facts),
        _ => false,
    }
}

pub(crate) fn prepare<'a>(
    query: &Query,
    input: &'a Input,
    facts: Rc<ModelFacts<'a>>,
    work: &mut Work,
    limits: Limits,
) -> Result<SemanticResult<'a>, QueryError> {
    let model = facts.model();
    let requesting = query.stages.iter().find(|s| requests_facts(s)).unwrap();
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
    let selection = Selection {
        input,
        items: crate::collect_items(input, limits.collection, work, &query.range, true)?,
        document: true,
    };
    let mut report = SemanticResult {
        model,
        facts,
        bindings_requested: false,
        callables_requested: false,
        instantiations_requested: false,
        body_callables: None,
        reference_order: None,
        stream: SemanticStream::Inspection(Box::new(QueryResult::Selection(selection))),
        limitations: Vec::new(),
        json: false,
        incomplete_observation: None,
    };
    for limitation in &model.limitations {
        report.limit(
            limitation.location.clone(),
            &limitation.message,
            work,
            requesting,
            limits,
        )?;
    }
    Ok(report)
}

impl<'a> SemanticResult<'a> {
    pub(crate) fn evaluate_stage(
        &mut self,
        input: &'a Input,
        stream: SemanticStream<'a>,
        stage: &Stage,
        work: &mut Work,
        limits: Limits,
        callable_references: &mut Vec<usize>,
    ) -> Result<SemanticStream<'a>, QueryError> {
        let model = self.model;
        Ok(match &stage.kind {
            StageKind::References => {
                self.ensure_bindings(work, stage)?;
                let subjects = subjects(stream, model, stage, limits, work)?;
                SemanticStream::Entities(self.references(
                    &subjects,
                    work,
                    stage,
                    limits,
                    callable_references,
                )?)
            }
            StageKind::Declarations => {
                self.ensure_bindings(work, stage)?;
                let subjects = subjects(stream, model, stage, limits, work)?;
                SemanticStream::Entities(self.declarations(&subjects, work, stage, limits)?)
            }
            StageKind::Uses => {
                self.ensure_bindings(work, stage)?;
                let subjects = subjects(stream, model, stage, limits, work)?;
                let declarations = self.declarations(&subjects, work, stage, limits)?;
                SemanticStream::Entities(self.uses(&declarations, work, stage, limits)?)
            }
            StageKind::ReferencedDeclarations => {
                self.ensure_bindings(work, stage)?;
                let SemanticStream::Entities(declarations) = stream else {
                    return Err(type_error(
                        &stage.range,
                        "referenced_declarations requires declarations",
                    ));
                };
                SemanticStream::Entities(self.referenced_declarations(
                    &declarations,
                    work,
                    stage,
                    limits,
                )?)
            }
            StageKind::TransitiveReferences(depth) => {
                self.ensure_bindings(work, stage)?;
                let subjects = subjects(stream, model, stage, limits, work)?;
                let declarations = self.declarations(&subjects, work, stage, limits)?;
                SemanticStream::Entities(self.transitive(
                    &declarations,
                    *depth,
                    work,
                    stage,
                    limits,
                )?)
            }
            StageKind::Types => {
                let subjects = subjects(stream, model, stage, limits, work)?;
                let subjects = self.typed_subjects(subjects, work, stage, limits)?;
                let mut rows = Vec::new();
                for subject in subjects {
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    work.produce(1, &stage.range)?;
                    let ty = self.type_of(&subject, work, stage, limits)?;
                    rows.push(SemanticType { subject, ty });
                }
                SemanticStream::Types(rows)
            }
            StageKind::Instantiations => {
                let subjects = subjects(stream, model, stage, limits, work)?;
                let subjects = self.typed_subjects(subjects, work, stage, limits)?;
                let mut rows = Vec::new();
                for subject in subjects {
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    work.produce(1, &stage.range)?;
                    let (instantiation, reason) =
                        self.instantiation_of(&subject, work, stage, limits)?;
                    rows.push(SemanticInstantiation {
                        subject,
                        instantiation,
                        reason,
                    });
                }
                SemanticStream::Instantiations(rows)
            }
            StageKind::Filter(_)
            | StageKind::Head(_)
            | StageKind::Tail(_)
            | StageKind::Reverse
            | StageKind::Check { .. }
            | StageKind::Count
            | StageKind::Unique
            | StageKind::Tally => unreachable!("the core evaluates common stages"),
            StageKind::Names | StageKind::Text
                if !matches!(stream, SemanticStream::Inspection(_)) =>
            {
                let subjects = subjects(stream, model, stage, limits, work)?;
                let mut values = Vec::new();
                for subject in subjects {
                    let names = if matches!(stage.kind, StageKind::Text) {
                        let location = self.subject_location(&subject);
                        vec![
                            std::str::from_utf8(self.subject_bytes(&subject)).map_err(|_| {
                                model_error(location, "text inspection requires UTF-8 source text")
                            })?,
                        ]
                    } else {
                        match &subject {
                            SemanticSubject::Entity(entity) => vec![self.name(*entity)],
                            SemanticSubject::Node { file, node, .. } => {
                                work.spend(node.children().len(), &stage.range)?;
                                crate::inspection::direct_names(&model.files[*file].parsed, node)
                            }
                        }
                    };
                    for name in names {
                        check_collection(values.len(), limits.collection, &stage.range)?;
                        work.produce(1, &stage.range)?;
                        work.spend(name.len().max(1), &stage.range)?;
                        values.push(name.to_owned());
                    }
                }
                SemanticStream::Inspection(Box::new(QueryResult::Strings(values)))
            }
            StageKind::Emit if !matches!(stream, SemanticStream::Inspection(_)) => stream,
            StageKind::Json => {
                self.json = true;
                stream
            }
            _ => match stream {
                SemanticStream::Inspection(result) => SemanticStream::Inspection(Box::new(
                    crate::evaluate_stage(input, Some(model), *result, stage, work, limits)?,
                )),
                _ => {
                    return Err(type_error(
                        &stage.range,
                        "stage does not accept this semantic stream",
                    ));
                }
            },
        })
    }

    pub(crate) fn finish(
        &mut self,
        callable_references: Vec<usize>,
        work: &mut Work,
        requesting: &Stage,
        range: &Range<usize>,
        limits: Limits,
    ) -> Result<(), QueryError> {
        self.finish_reference_limitations(callable_references, work, requesting, limits)?;
        self.charge_output(work, range, limits)?;
        if self.json {
            self.validate_json(work, range)?;
        }
        Ok(())
    }
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
                work.produce(1, &stage.range)?;
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
            let nodes = crate::inspection::into_nodes(*result, &stage.range, work)?;
            for selected in nodes.nodes {
                work.visit(&stage.range)?;
                check_collection(subjects.len(), limits.collection, &stage.range)?;
                work.produce(1, &stage.range)?;
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

/// Admit source tokens and CST nodes before any public fact producer runs.
pub(crate) fn admit_model_facts(
    model: &ModelContext,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    if let Some(error) = model.errors.first() {
        return Err(model_error(error.location.clone(), &error.message));
    }
    for source in &model.files {
        work.spend(source.parsed.tokens().len(), range)?;
        work.visit(range)?;
        let mut pending = vec![source.parsed.tree()];
        while let Some(node) = pending.pop() {
            for child in node.child_nodes() {
                work.visit(range)?;
                pending.push(child);
            }
        }
    }
    Ok(())
}

impl SemanticResult<'_> {
    // Admission is charged per request, including a warm shared owner.
    pub(crate) fn admit_fact_owner(
        &mut self,
        work: &mut Work,
        range: &Range<usize>,
    ) -> Result<(), QueryError> {
        if !self.bindings_requested {
            admit_model_facts(self.model, work, range)?;
            self.facts.bindings();
            self.bindings_requested = true;
        }
        Ok(())
    }
    fn ensure_bindings(&mut self, work: &mut Work, stage: &Stage) -> Result<(), QueryError> {
        self.admit_fact_owner(work, &stage.range)
    }
    fn ensure_callables(&mut self, work: &mut Work, stage: &Stage) -> Result<(), QueryError> {
        self.ensure_bindings(work, stage)?;
        self.facts.source_callables();
        self.callables_requested = true;
        Ok(())
    }
    fn ensure_occurrence_callables(
        &mut self,
        subject: &SemanticSubject<'_>,
        work: &mut Work,
        stage: &Stage,
    ) -> Result<(), QueryError> {
        self.ensure_callables(work, stage)?;
        let file = self.subject_file(subject);
        if self.model.files[file].kind != SourceKind::StandardLibrary
            || self.declared_subject(subject, work, stage)?.is_some()
        {
            return Ok(());
        }
        let location = self.subject_location(subject);
        let facts = self.callables().unwrap();
        for fact in &facts.expressions {
            work.visit(&stage.range)?;
            if fact.file == file && fact.location.range == location.range {
                return Ok(());
            }
        }
        if let SemanticSubject::Entity(SemanticEntity::Reference(id)) = subject {
            let reference = &self.bindings().unwrap().references[*id];
            if reference.kind == ReferenceKind::Callable {
                for call in &facts.calls {
                    work.visit(&stage.range)?;
                    if call.file == file && call.location.range.start == location.range.start {
                        return Ok(());
                    }
                }
            }
        }
        let offset = self.model.files[file].byte_offset;
        let mut owner = None;
        for declaration in &self.bindings().unwrap().declarations {
            work.visit(&stage.range)?;
            if declaration.file == file
                && matches!(
                    declaration.role,
                    zincite_lint::DeclarationRole::Function
                        | zincite_lint::DeclarationRole::Predicate
                        | zincite_lint::DeclarationRole::Test
                        | zincite_lint::DeclarationRole::Annotation
                )
                && declaration.syntax_range.start + offset <= location.range.start
                && location.range.end <= declaration.syntax_range.end + offset
            {
                owner = Some(declaration.id);
                break;
            }
        }
        if let Some(owner) = owner {
            self.body_callables = Some(self.facts.source_callable_body(facts, owner));
        }
        Ok(())
    }
    fn ensure_instantiations(&mut self, work: &mut Work, stage: &Stage) -> Result<(), QueryError> {
        self.ensure_callables(work, stage)?;
        self.facts.instantiations();
        self.instantiations_requested = true;
        Ok(())
    }
    fn owning_range(&self, subject: &SemanticSubject<'_>) -> Range<usize> {
        match subject {
            SemanticSubject::Entity(SemanticEntity::Declaration(id)) => {
                let declaration = &self.bindings().unwrap().declarations[id.0];
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
            for id in 0..self.bindings().unwrap().references.len() {
                work.visit(&stage.range)?;
                let reference = &self.bindings().unwrap().references[id];
                if reference.file == file && contains(&region, &reference.location.range) {
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    work.produce(1, &stage.range)?;
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
            if self.callables_requested {
                // target records actual unknown outcomes and treats resolved
                // declarations and intrinsics as known endpoints.
                self.target(id, work, stage, limits)?;
            } else {
                let reference = &self.bindings().unwrap().references[id];
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
                    work.produce(1, &stage.range)?;
                    work.visit(&stage.range)?;
                    rows.push(SemanticEntity::Declaration(*id));
                }
                SemanticSubject::Entity(SemanticEntity::Reference(id)) => {
                    if let Some(target) = self.target(*id, work, stage, limits)? {
                        check_collection(rows.len(), limits.collection, &stage.range)?;
                        work.produce(1, &stage.range)?;
                        rows.push(SemanticEntity::Declaration(target));
                    }
                }
                SemanticSubject::Node { file, node, .. } => {
                    for declaration in &self.bindings().unwrap().declarations {
                        work.visit(&stage.range)?;
                        if declaration.file == *file && declaration.syntax_range == node.range() {
                            check_collection(rows.len(), limits.collection, &stage.range)?;
                            work.produce(1, &stage.range)?;
                            rows.push(SemanticEntity::Declaration(declaration.id));
                        }
                    }
                }
            }
        }
        Ok(rows)
    }
    fn endpoint(
        &mut self,
        id: usize,
        work: &mut Work,
        stage: &Stage,
        _limits: Limits,
    ) -> Result<ReferenceEndpoint, QueryError> {
        work.visit(&stage.range)?;
        if self.bindings().unwrap().references[id].kind == ReferenceKind::Callable {
            self.ensure_occurrence_callables(
                &SemanticSubject::Entity(SemanticEntity::Reference(id)),
                work,
                stage,
            )?;
        }
        let reference = &self.bindings().unwrap().references[id];
        let location = reference.location.clone();
        let mut reason = format!(
            "reference '{}' has no proven declaration target",
            reference.name
        );
        if reference.kind == ReferenceKind::Callable {
            for call in &self.callables().unwrap().calls {
                work.visit(&stage.range)?;
                if call.file != reference.file || call.location.range.start != location.range.start
                {
                    continue;
                }
                match &call.outcome {
                    CallOutcome::Resolved { declaration, .. } => {
                        return Ok(ReferenceEndpoint::Declaration(*declaration));
                    }
                    CallOutcome::Intrinsic { .. } => return Ok(ReferenceEndpoint::Intrinsic),
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
            return Ok(ReferenceEndpoint::Declaration(declaration));
        }
        Ok(ReferenceEndpoint::Uncertain(location, reason))
    }
    fn target(
        &mut self,
        id: usize,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Option<DeclarationId>, QueryError> {
        match self.endpoint(id, work, stage, limits)? {
            ReferenceEndpoint::Declaration(id) => Ok(Some(id)),
            ReferenceEndpoint::Intrinsic => Ok(None),
            ReferenceEndpoint::Uncertain(location, reason) => {
                if self.incomplete_observation.is_none() {
                    work.spend(
                        location.path.as_os_str().len() + reason.len() + 1,
                        &stage.range,
                    )?;
                    work.produce(1, &stage.range)?;
                    self.incomplete_observation = Some(std::sync::Arc::new(QueryError {
                        range: location.range.clone(),
                        location: crate::ErrorLocation::Model(location.clone()),
                        message: reason.clone(),
                    }));
                }
                self.limit(location, reason, work, stage, limits)?;
                Ok(None)
            }
        }
    }

    /// Complete adjacency in the legacy edge order, refusing any uncertain endpoint.
    /// The request-local index bounds each lookup to its owning file and region.
    pub(crate) fn complete_targets(
        &mut self,
        id: DeclarationId,
        work: &mut Work,
        range: &Range<usize>,
        limits: Limits,
    ) -> Result<Vec<SemanticEntity>, QueryError> {
        let stage = Stage {
            kind: StageKind::ReferencedDeclarations,
            range: range.clone(),
        };
        self.ensure_bindings(work, &stage)?;
        let declaration = self
            .bindings()
            .unwrap()
            .declarations
            .get(id.0)
            .ok_or_else(|| type_error(range, "unavailable declaration identity"))?;
        let file = declaration.file;
        let offset = self.model.files[file].byte_offset;
        let region = declaration.syntax_range.start + offset..declaration.syntax_range.end + offset;
        if self.reference_order.is_none() {
            work.produce(self.bindings().unwrap().references.len(), range)?;
            self.reference_order = Some(self.ordered_references(work, &stage, limits)?);
        }
        let references = &self.bindings().unwrap().references;
        let order = self.reference_order.as_ref().unwrap();
        work.spend(order.len().max(1).ilog2() as usize + 1, range)?;
        let first = order.partition_point(|&i| {
            (references[i].file, references[i].location.range.start) < (file, region.start)
        });
        let mut selected = HashSet::new();
        let mut rows = Vec::new();
        let mut index = first;
        while let Some(&reference) = self.reference_order.as_ref().unwrap().get(index) {
            index += 1;
            work.visit(range)?;
            let occurrence = &self.bindings().unwrap().references[reference];
            if occurrence.file != file || occurrence.location.range.start >= region.end {
                break;
            }
            if !contains(&region, &occurrence.location.range) {
                continue;
            }
            match self.endpoint(reference, work, &stage, limits)? {
                ReferenceEndpoint::Declaration(target) => {
                    if selected.contains(&target.0) {
                        continue;
                    }
                    check_collection(rows.len(), limits.collection, range)?;
                    work.produce(2, range)?;
                    selected.insert(target.0);
                    rows.push(SemanticEntity::Declaration(target));
                }
                ReferenceEndpoint::Intrinsic => {}
                ReferenceEndpoint::Uncertain(location, reason) => {
                    return Err(model_error(
                        location,
                        format!("reference_info cannot provide complete adjacency: {reason}"),
                    ));
                }
            }
        }
        Ok(rows)
    }
    fn uses(
        &mut self,
        declarations: &[SemanticEntity],
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Vec<SemanticEntity>, QueryError> {
        let mut rows = Vec::new();
        let mut selected = HashSet::new();
        let mut names = HashSet::new();
        for declaration in declarations {
            let SemanticEntity::Declaration(id) = declaration else {
                unreachable!()
            };
            work.visit(&stage.range)?;
            check_collection(selected.len(), limits.collection, &stage.range)?;
            work.produce(1, &stage.range)?;
            selected.insert(id.0);
            let name = &self.bindings().unwrap().declarations[id.0].name;
            work.spend(name.len(), &stage.range)?;
            check_collection(names.len(), limits.collection, &stage.range)?;
            work.produce(1, &stage.range)?;
            names.insert(name.clone());
        }
        // Uncertain occurrences matter even when the requested declaration set is empty.
        let mut targets = Vec::new();
        for id in 0..self.bindings().unwrap().references.len() {
            work.visit(&stage.range)?;
            let reference = &self.bindings().unwrap().references[id];
            if !selected.is_empty() {
                // Binding names are already normalized. Candidate identity also
                // rules out shadowed declarations with the same written name.
                let relevant = match &reference.resolution {
                    BindingResolution::Resolved(id) => selected.contains(&id.0),
                    BindingResolution::Overloads(ids) | BindingResolution::Ambiguous(ids) => {
                        work.spend(ids.len(), &stage.range)?;
                        ids.iter().any(|id| selected.contains(&id.0))
                    }
                    BindingResolution::Unresolved => {
                        work.spend(reference.name.len(), &stage.range)?;
                        names.contains(&reference.name)
                    }
                };
                if !relevant {
                    continue;
                }
            }
            check_collection(targets.len(), limits.collection, &stage.range)?;
            work.produce(1, &stage.range)?;
            targets.push((id, self.target(id, work, stage, limits)?));
        }
        for declaration in declarations {
            let SemanticEntity::Declaration(declaration) = declaration else {
                unreachable!()
            };
            for (id, target) in &targets {
                work.visit(&stage.range)?;
                if *target == Some(*declaration) {
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    work.produce(1, &stage.range)?;
                    rows.push(SemanticEntity::Reference(*id));
                }
            }
        }
        Ok(rows)
    }
    fn ordered_references(
        &self,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Vec<usize>, QueryError> {
        let references = &self.bindings().unwrap().references;
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
        Ok(order)
    }

    fn referenced_declarations(
        &mut self,
        declarations: &[SemanticEntity],
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Vec<SemanticEntity>, QueryError> {
        let order = self.ordered_references(work, stage, limits)?;
        let mut rows = Vec::new();
        for declaration in declarations {
            let SemanticEntity::Declaration(id) = declaration else {
                return Err(type_error(
                    &stage.range,
                    "referenced_declarations requires declarations",
                ));
            };
            work.visit(&stage.range)?;
            let declaration = &self.bindings().unwrap().declarations[id.0];
            let file = declaration.file;
            let offset = self.model.files[file].byte_offset;
            let region =
                declaration.syntax_range.start + offset..declaration.syntax_range.end + offset;
            let mut selected = Vec::new();
            for &reference in &order {
                work.visit(&stage.range)?;
                let occurrence = &self.bindings().unwrap().references[reference];
                if occurrence.file != file || !contains(&region, &occurrence.location.range) {
                    continue;
                }
                if let Some(target) = self.target(reference, work, stage, limits)? {
                    let mut repeated = false;
                    for previous in &selected {
                        work.visit(&stage.range)?;
                        if *previous == target {
                            repeated = true;
                            break;
                        }
                    }
                    if repeated {
                        continue;
                    }
                    check_collection(selected.len(), limits.collection, &stage.range)?;
                    check_collection(rows.len(), limits.collection, &stage.range)?;
                    work.produce(2, &stage.range)?;
                    selected.push(target);
                    rows.push(SemanticEntity::Declaration(target));
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
        let order = self.ordered_references(work, stage, limits)?;
        let mut emitted = HashSet::new();
        let mut rows = Vec::new();
        for _ in 0..depth {
            if frontier.is_empty() {
                break;
            }
            let mut next = Vec::new();
            for declaration in frontier {
                work.visit(&stage.range)?;
                let declaration = &self.bindings().unwrap().declarations[declaration.0];
                let file = declaration.file;
                let offset = self.model.files[file].byte_offset;
                let region =
                    declaration.syntax_range.start + offset..declaration.syntax_range.end + offset;
                for &id in &order {
                    work.visit(&stage.range)?;
                    let reference = &self.bindings().unwrap().references[id];
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
                        work.produce(1, &stage.range)?;
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
        self.ensure_bindings(work, stage)?;
        let mut rows = Vec::new();
        for subject in subjects {
            let before = rows.len();
            if let SemanticSubject::Node { file, node, .. } = &subject {
                for declaration in &self.bindings().unwrap().declarations {
                    work.visit(&stage.range)?;
                    if declaration.file == *file && declaration.syntax_range == node.range() {
                        check_collection(rows.len(), limits.collection, &stage.range)?;
                        work.produce(1, &stage.range)?;
                        rows.push(SemanticSubject::Entity(SemanticEntity::Declaration(
                            declaration.id,
                        )));
                    }
                }
            }
            if rows.len() == before {
                check_collection(rows.len(), limits.collection, &stage.range)?;
                work.produce(1, &stage.range)?;
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
        self.ensure_occurrence_callables(subject, work, stage)?;
        let declaration = self.declared_subject(subject, work, stage)?;
        let facts = self.callables().unwrap();
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
                && self.bindings().unwrap().references[*id].kind == ReferenceKind::Callable
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
        if work.budget.remaining_results().is_some()
            && let Some(ty) = ty
        {
            let (units, bytes) = type_allocation(ty);
            work.produce(units, &stage.range)?;
            work.spend(bytes, &stage.range)?;
        }
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
                for declaration in &self.bindings().unwrap().declarations {
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
        self.ensure_bindings(work, stage)?;
        if let Some(id) = self.declared_subject(subject, work, stage)? {
            work.visit(&stage.range)?;
            let instantiation = self.bindings().unwrap().declarations[id.0].instantiation;
            outcome = (
                instantiation,
                (instantiation == Instantiation::Unknown)
                    .then(|| "declared instantiation is unknown".to_owned()),
            );
        } else {
            self.ensure_occurrence_callables(subject, work, stage)?;
            self.ensure_instantiations(work, stage)?;
            let location = self.subject_location(subject);
            let file = self.subject_file(subject);
            for fact in &self.instantiation_facts().unwrap().expressions {
                work.visit(&stage.range)?;
                if fact.file == file && fact.location.range == location.range {
                    outcome = (fact.instantiation, fact.reason.clone());
                    break;
                }
            }
            if outcome.0 == Instantiation::Unknown
                && self.model.files[file].kind == SourceKind::StandardLibrary
            {
                // The ordinary instantiation producer walks user sources. An
                // explicitly navigated library occurrence uses its source type.
                for fact in &self.callables().unwrap().expressions {
                    work.visit(&stage.range)?;
                    if fact.file == file
                        && fact.location.range == location.range
                        && fact.ty.instantiation != Instantiation::Unknown
                    {
                        outcome = (fact.ty.instantiation, None);
                        break;
                    }
                }
            }
            if outcome.0 == Instantiation::Unknown
                && let SemanticSubject::Entity(SemanticEntity::Reference(id)) = subject
                && self.bindings().unwrap().references[*id].kind == ReferenceKind::Callable
            {
                for fact in &self.callables().unwrap().calls {
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

    pub(crate) fn guard_leaf(
        &mut self,
        predicate: &Predicate,
        subject: &SemanticSubject<'_>,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<(Truth, Option<SourceDiagnostic>), QueryError> {
        let truth = self.predicate_leaf(predicate, subject, work, stage, limits)?;
        let diagnostic = if truth == Truth::Unknown {
            let location = match subject {
                SemanticSubject::Entity(entity) => self.location(*entity),
                SemanticSubject::Node { location, .. } => location,
            };
            let mut existing = None;
            for diagnostic in self.limitations.iter().rev() {
                work.visit(&predicate.range)?;
                if diagnostic.location == *location {
                    existing = Some(diagnostic);
                    break;
                }
            }
            let message = existing.map_or("semantic guard fact is unknown", |d| d.message.as_str());
            work.spend(
                location.path.as_os_str().len() + message.len() + 1,
                &predicate.range,
            )?;
            work.produce(1, &predicate.range)?;
            Some(SourceDiagnostic {
                location: location.clone(),
                message: message.to_owned(),
            })
        } else {
            None
        };
        Ok((truth, diagnostic))
    }
    pub(crate) fn predicate_leaf(
        &mut self,
        predicate: &Predicate,
        subject: &SemanticSubject<'_>,
        work: &mut Work,
        stage: &Stage,
        limits: Limits,
    ) -> Result<Truth, QueryError> {
        Ok(match &predicate.kind {
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
            PredicateKind::Group(_)
            | PredicateKind::Not(_)
            | PredicateKind::And(_)
            | PredicateKind::Or(_) => {
                unreachable!("Boolean predicates are evaluated by the pipeline")
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
                let reference = &self.bindings().unwrap().references[*id];
                if let BindingResolution::Overloads(ids) | BindingResolution::Ambiguous(ids) =
                    &reference.resolution
                {
                    work.spend(ids.len(), range)?;
                }
                if reference.kind == ReferenceKind::Callable
                    && let Some(callables) = self.callables()
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
                    for (value, _) in values {
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
pub(crate) enum Truth {
    True,
    False,
    Unknown,
}
impl Truth {
    fn from(value: bool) -> Self {
        if value { Self::True } else { Self::False }
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

/// Size of owned native type components, before cloning retained facts.
pub(crate) fn type_allocation(ty: &TypeInst) -> (usize, usize) {
    let mut total = (1, 0);
    let mut add = |ty| {
        let (units, bytes) = type_allocation(ty);
        total.0 += units;
        total.1 += bytes;
    };
    match &ty.kind {
        TypeKind::Set(element) => add(element),
        TypeKind::Array { indices, element } => {
            for index in indices {
                add(index);
            }
            add(element);
        }
        TypeKind::Tuple(fields) => {
            for field in fields {
                add(field);
            }
        }
        TypeKind::Record(fields) => {
            for (_, field) in fields {
                add(field);
            }
            total.0 += fields.len();
            total.1 += fields.iter().map(|(name, _)| name.len()).sum::<usize>();
        }
        TypeKind::Variable { name, .. } | TypeKind::Unknown(name) => total.1 += name.len(),
        _ => {}
    }
    total
}
