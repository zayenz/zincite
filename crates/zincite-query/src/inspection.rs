//! Direct CST navigation and source projections for the fixed query pipeline.
use std::collections::{BTreeMap, HashSet};
use std::ops::Range;

use serde_json::{Value, json};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind, is_utf8_boundary};

use crate::language::{Predicate, Stage, StageKind};
use crate::{Input, ItemKind, QueryError, QueryResult, Work, identifier_identity};

/// One original CST node, with its original-file byte coordinates.
#[derive(Clone, Debug)]
pub struct SelectedNode<'a> {
    pub node: &'a SyntaxNode,
    pub file: &'a str,
    pub range: Range<usize>,
}

#[derive(Debug)]
pub struct NodeSelection<'a> {
    pub(crate) input: &'a Input,
    pub(crate) nodes: Vec<SelectedNode<'a>>,
}

impl<'a> NodeSelection<'a> {
    pub fn input(&self) -> &'a Input {
        self.input
    }
    pub fn nodes(&self) -> &[SelectedNode<'a>] {
        &self.nodes
    }
    pub(crate) fn truncate(&mut self, count: usize) {
        self.nodes.truncate(count);
    }
}

/// A validated JSON emitter retaining the native result until rendering.
#[derive(Debug)]
pub struct JsonResult<'a> {
    result: Box<QueryResult<'a>>,
}

impl JsonResult<'_> {
    pub fn result(&self) -> &QueryResult<'_> {
        &self.result
    }
    pub(crate) fn render(&self) -> Vec<u8> {
        // Evaluation checks text conversion while the immutable input is borrowed.
        let value = json_value(&self.result).expect("JSON text was validated during evaluation");
        let mut bytes = serde_json::to_vec(&value).expect("inspection values serialize");
        bytes.push(b'\n');
        bytes
    }
}

pub(crate) fn type_error(range: &Range<usize>, message: &str) -> QueryError {
    QueryError::query(range.clone(), message)
}

pub(crate) fn require_nodes(
    result: &QueryResult<'_>,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    if matches!(result, QueryResult::Selection(_) | QueryResult::Nodes(_)) {
        Ok(())
    } else {
        Err(type_error(range, "stage requires selected nodes"))
    }
}

pub(crate) fn into_nodes<'a>(
    result: QueryResult<'a>,
    range: &Range<usize>,
) -> Result<NodeSelection<'a>, QueryError> {
    match result {
        QueryResult::Nodes(nodes) => Ok(nodes),
        QueryResult::Selection(selection) => Ok(NodeSelection {
            input: selection.input,
            nodes: selection
                .items
                .into_iter()
                .map(|item| SelectedNode {
                    node: item.node,
                    file: selection.input.file(),
                    range: item.range,
                })
                .collect(),
        }),
        _ => Err(type_error(range, "stage requires selected nodes")),
    }
}

pub(crate) fn navigate<'a>(
    result: QueryResult<'a>,
    stage: &Stage,
    work: &mut Work,
    limit: usize,
) -> Result<QueryResult<'a>, QueryError> {
    let selection = into_nodes(result, &stage.range)?;
    let input = selection.input;
    if let StageKind::Range(range) = &stage.kind
        && (range.end > input.bytes.len()
            || !is_utf8_boundary(&input.bytes, range.start)
            || !is_utf8_boundary(&input.bytes, range.end))
    {
        return Err(type_error(
            &stage.range,
            "range bounds must be valid original-input byte boundaries",
        ));
    }
    let mut nodes = Vec::new();
    for selected in selection.nodes {
        let mut pending = vec![selected.node];
        while let Some(node) = pending.pop() {
            work.visit(&stage.range)?;
            let original = input.original_range(node.range());
            let retain = match &stage.kind {
                StageKind::Expressions => is_expression(node.kind()),
                StageKind::Children => !std::ptr::eq(node, selected.node),
                StageKind::Subtree => true,
                StageKind::Range(range) => {
                    range.start < range.end
                        && range.start <= original.start
                        && original.end <= range.end
                }
                _ => unreachable!(),
            };
            if retain {
                check_collection(nodes.len(), limit, &stage.range)?;
                nodes.push(SelectedNode {
                    node,
                    file: input.file(),
                    range: original,
                });
            }
            let descend = match stage.kind {
                StageKind::Expressions | StageKind::Subtree => true,
                StageKind::Children => std::ptr::eq(node, selected.node),
                _ => false,
            };
            if descend {
                work.spend(node.children().len(), &stage.range)?;
                for child in node.children().iter().rev() {
                    if let SyntaxElement::Node(child) = child {
                        check_collection(pending.len(), limit, &stage.range)?;
                        pending.push(child);
                    }
                }
            }
        }
    }
    Ok(QueryResult::Nodes(NodeSelection { input, nodes }))
}

pub(crate) fn check_collection(
    length: usize,
    limit: usize,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    if length >= limit {
        Err(type_error(range, "query collection limit exceeded"))
    } else {
        Ok(())
    }
}

pub(crate) fn filter<'a>(
    selection: NodeSelection<'a>,
    predicate: &Predicate,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<QueryResult<'a>, QueryError> {
    let mut nodes = Vec::new();
    for node in selection.nodes {
        work.visit(range)?;
        work.spend(node.node.children().len(), range)?;
        let names = direct_names(selection.input, node.node);
        if predicate.matches_values(&kind_name(node.node.kind()), names.first().copied(), work)? {
            nodes.push(node);
        }
    }
    Ok(QueryResult::Nodes(NodeSelection {
        input: selection.input,
        nodes,
    }))
}

pub(crate) fn project<'a>(
    result: QueryResult<'a>,
    stage: &Stage,
    work: &mut Work,
    limit: usize,
) -> Result<QueryResult<'a>, QueryError> {
    let selection = into_nodes(result, &stage.range)?;
    let mut values = Vec::new();
    for selected in selection.nodes {
        work.visit(&stage.range)?;
        work.spend(selected.node.children().len(), &stage.range)?;
        if matches!(stage.kind, StageKind::Text | StageKind::AnnotationNames) {
            work.spend(selected.range.len(), &stage.range)?;
        }
        let names = match stage.kind {
            StageKind::Names => direct_names(selection.input, selected.node),
            StageKind::CallNames => call_name(selection.input, selected.node)
                .into_iter()
                .collect(),
            StageKind::AnnotationNames => annotation_name(selection.input, selected.node)
                .into_iter()
                .collect(),
            StageKind::Text => vec![original_text(selection.input, &selected.range)?],
            _ => unreachable!(),
        };
        for name in names {
            check_collection(values.len(), limit, &stage.range)?;
            if !matches!(stage.kind, StageKind::Text) {
                work.spend(name.len(), &stage.range)?;
            }
            values.push(name.to_owned());
        }
    }
    Ok(QueryResult::Strings(values))
}

fn direct_names<'a>(input: &'a Input, node: &SyntaxNode) -> Vec<&'a str> {
    node.children()
        .iter()
        .filter_map(|child| {
            let SyntaxElement::Token(index) = child else {
                return None;
            };
            let token = &input.syntax().tokens()[*index];
            matches!(
                token.kind,
                TokenKind::Identifier | TokenKind::QuotedIdentifier
            )
            .then(|| identifier_identity(&input.syntax().source()[token.range.clone()]))
        })
        .collect()
}

fn call_name<'a>(input: &'a Input, node: &SyntaxNode) -> Option<&'a str> {
    if !matches!(
        node.kind(),
        NodeKind::CallExpression | NodeKind::GeneratorCallExpression
    ) {
        return None;
    }
    direct_names(input, node).first().copied().or_else(|| {
        node.children().iter().any(|child| matches!(child, SyntaxElement::Token(index) if input.syntax().tokens()[*index].kind == TokenKind::Anonymous)).then_some("_")
    })
}

fn annotation_name<'a>(input: &'a Input, node: &SyntaxNode) -> Option<&'a str> {
    if node.kind() != NodeKind::Annotation {
        return None;
    }
    let mut head = node.child_nodes().next()?;
    while matches!(
        head.kind(),
        NodeKind::AnnotatedExpression
            | NodeKind::ArrayAccessExpression
            | NodeKind::FieldAccessExpression
            | NodeKind::ParenthesizedExpression
    ) {
        head = head.child_nodes().next()?;
    }
    call_name(input, head).or_else(|| direct_names(input, head).first().copied()).or_else(|| {
        head.children().iter().any(|child| matches!(child, SyntaxElement::Token(index) if input.syntax().tokens()[*index].kind == TokenKind::Output)).then_some("output")
    })
}

pub(crate) fn original_text<'a>(
    input: &'a Input,
    range: &Range<usize>,
) -> Result<&'a str, QueryError> {
    std::str::from_utf8(&input.bytes[range.clone()]).map_err(|_| QueryError::input(range.clone(), "text and JSON inspection require UTF-8 source text; source emission preserves opaque comment bytes"))
}

pub(crate) fn unique<'a>(
    result: QueryResult<'a>,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<QueryResult<'a>, QueryError> {
    match result {
        QueryResult::Selection(mut selection) => {
            let mut seen = HashSet::new();
            let mut items = Vec::new();
            for item in selection.items {
                work.visit(range)?;
                if seen.insert(std::ptr::from_ref(item.node)) {
                    items.push(item);
                }
            }
            selection.items = items;
            Ok(QueryResult::Selection(selection))
        }
        QueryResult::Nodes(mut selection) => {
            let mut seen = HashSet::new();
            let mut nodes = Vec::new();
            for node in selection.nodes {
                work.visit(range)?;
                if seen.insert(std::ptr::from_ref(node.node)) {
                    nodes.push(node);
                }
            }
            selection.nodes = nodes;
            Ok(QueryResult::Nodes(selection))
        }
        QueryResult::Strings(values) => {
            let mut seen = HashSet::new();
            let mut unique = Vec::new();
            for value in values {
                work.spend(value.len().max(1), range)?;
                if seen.insert(value.clone()) {
                    unique.push(value);
                }
            }
            Ok(QueryResult::Strings(unique))
        }
        _ => Err(type_error(range, "unique requires a node or string stream")),
    }
}

pub(crate) fn tally<'a>(
    result: QueryResult<'a>,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<QueryResult<'a>, QueryError> {
    let QueryResult::Strings(values) = result else {
        return Err(type_error(range, "tally requires projected strings"));
    };
    let mut counts = BTreeMap::new();
    for value in values {
        work.spend(value.len().max(1), range)?;
        *counts.entry(value).or_insert(0) += 1;
    }
    Ok(QueryResult::Tally(counts))
}

pub(crate) fn json<'a>(
    result: QueryResult<'a>,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<QueryResult<'a>, QueryError> {
    match &result {
        QueryResult::Selection(selection) => {
            for item in &selection.items {
                work.spend(item.range.len().max(1), range)?;
            }
        }
        QueryResult::Nodes(selection) => {
            for node in &selection.nodes {
                work.spend(node.range.len().max(1), range)?;
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
        QueryResult::Literals(selection) => {
            crate::structured::validate_json(selection, work, range)?
        }
        QueryResult::Count(_) => {}
        _ => return Err(type_error(range, "json requires an inspection result")),
    }
    match &result {
        QueryResult::Selection(selection) => {
            for item in &selection.items {
                original_text(selection.input, &item.range)?;
                work.spend(selection.input.file().len(), range)?;
            }
        }
        QueryResult::Nodes(selection) => {
            for node in &selection.nodes {
                original_text(selection.input, &node.range)?;
                work.spend(selection.input.file().len(), range)?;
            }
        }
        _ => {}
    }
    Ok(QueryResult::Json(JsonResult {
        result: Box::new(result),
    }))
}

fn node_json(input: &Input, node: &SyntaxNode, range: &Range<usize>) -> Result<Value, QueryError> {
    Ok(json!({
        "kind": kind_name(node.kind()),
        "names": direct_names(input, node),
        "call_names": call_name(input, node).into_iter().collect::<Vec<_>>(),
        "annotation_names": annotation_name(input, node).into_iter().collect::<Vec<_>>(),
        "file": input.file(),
        "range": {"start": range.start, "end": range.end},
        "text": original_text(input, range)?,
    }))
}

fn json_value(result: &QueryResult<'_>) -> Result<Value, QueryError> {
    match result {
        QueryResult::Selection(selection) => selection
            .items
            .iter()
            .map(|item| node_json(selection.input, item.node, &item.range))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        QueryResult::Nodes(selection) => selection
            .nodes
            .iter()
            .map(|node| node_json(selection.input, node.node, &node.range))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        QueryResult::Literals(selection) => crate::structured::json_value(selection),
        QueryResult::Strings(values) => Ok(json!(values)),
        QueryResult::Count(count) => Ok(json!(count)),
        QueryResult::Tally(values) => Ok(json!(values)),
        _ => unreachable!("only validated inspection results reach the JSON renderer"),
    }
}

pub(crate) fn kind_name(kind: NodeKind) -> String {
    let written = format!("{kind:?}");
    let mut name = String::new();
    for (index, character) in written.chars().enumerate() {
        if character.is_ascii_uppercase() && index != 0 {
            name.push('_');
        }
        name.push(character.to_ascii_lowercase());
    }
    name
}

pub(crate) fn known_kind(name: &str) -> bool {
    ItemKind::parse(name).is_some() || SYNTAX_KINDS.iter().any(|kind| kind_name(*kind) == name)
}

fn is_expression(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Expression
            | NodeKind::InterpolatedString
            | NodeKind::UnaryExpression
            | NodeKind::BinaryExpression
            | NodeKind::ParenthesizedExpression
            | NodeKind::CallExpression
            | NodeKind::GeneratorCallExpression
            | NodeKind::ConditionalExpression
            | NodeKind::LetExpression
            | NodeKind::SetComprehension
            | NodeKind::ArrayComprehension
            | NodeKind::IndexedArrayComprehension
            | NodeKind::SetLiteral
            | NodeKind::ArrayLiteral
            | NodeKind::MatrixLiteral
            | NodeKind::TupleLiteral
            | NodeKind::RecordLiteral
            | NodeKind::ArrayAccessExpression
            | NodeKind::FieldAccessExpression
            | NodeKind::AnnotatedExpression
            | NodeKind::RangeExpression
    )
}

const SYNTAX_KINDS: &[NodeKind] = &[
    NodeKind::Root,
    NodeKind::EnumDeclaration,
    NodeKind::TypeAlias,
    NodeKind::EnumDefinition,
    NodeKind::EnumCases,
    NodeKind::EnumCase,
    NodeKind::EnumConstructor,
    NodeKind::TypeInstConcatenation,
    NodeKind::Declaration,
    NodeKind::FunctionDeclaration,
    NodeKind::PredicateDeclaration,
    NodeKind::AnnotationCapture,
    NodeKind::TestDeclaration,
    NodeKind::AnnotationDeclaration,
    NodeKind::ParameterList,
    NodeKind::Parameter,
    NodeKind::TypeInstVariable,
    NodeKind::ScalarType,
    NodeKind::TupleType,
    NodeKind::RecordType,
    NodeKind::RecordField,
    NodeKind::TupleLiteral,
    NodeKind::RecordLiteral,
    NodeKind::RecordLiteralField,
    NodeKind::FieldAccessExpression,
    NodeKind::DomainType,
    NodeKind::SetType,
    NodeKind::SetCardinality,
    NodeKind::ArrayType,
    NodeKind::ArrayIndexBinding,
    NodeKind::ListType,
    NodeKind::Assignment,
    NodeKind::Constraint,
    NodeKind::Include,
    NodeKind::Output,
    NodeKind::Solve,
    NodeKind::SolveMinimize,
    NodeKind::SolveMaximize,
    NodeKind::Expression,
    NodeKind::InterpolatedString,
    NodeKind::UnaryExpression,
    NodeKind::BinaryExpression,
    NodeKind::ParenthesizedExpression,
    NodeKind::CallExpression,
    NodeKind::GeneratorCallExpression,
    NodeKind::ConditionalExpression,
    NodeKind::ConditionalBranch,
    NodeKind::ElseBranch,
    NodeKind::LetExpression,
    NodeKind::LetBlock,
    NodeKind::SetComprehension,
    NodeKind::ArrayComprehension,
    NodeKind::IndexedArrayComprehension,
    NodeKind::GeneratorList,
    NodeKind::Generator,
    NodeKind::WhereFilter,
    NodeKind::SetLiteral,
    NodeKind::ArrayLiteral,
    NodeKind::IndexedArrayEntry,
    NodeKind::IndexTuple,
    NodeKind::MatrixLiteral,
    NodeKind::MatrixColumnIndices,
    NodeKind::MatrixRow,
    NodeKind::ArrayAccessExpression,
    NodeKind::NamedArgument,
    NodeKind::AnnotatedExpression,
    NodeKind::Annotation,
    NodeKind::RangeExpression,
    NodeKind::Error,
];
