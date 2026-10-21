//! Original-source copies and final literal obligations for explicit append edits.
use super::*;
use crate::data::charge;
use crate::{LiteralKind as L, LiteralValue, reduction, structured};
use std::collections::{BTreeSet, HashSet};
use zincite_lint::{
    DeclarationId, DeclarationRole, Domain, EditPart, Instantiation, TextEdit, TypeKind,
};
use zincite_syntax::{NodeKind as N, SyntaxNode, TokenKind as T};

#[derive(Debug)]
pub(super) struct ScalarArgument {
    source: String,
    range: Range<usize>,
    names: Vec<Located<String>>,
}

#[derive(Debug)]
pub(super) enum CopyStage {
    Members(ScalarArgument, ScalarArgument),
    Elements(Located<String>),
    Repeat(ScalarArgument),
}

/// Opaque source fragments tied to their original input and model-fact owner.
/// Values are created by clone/repeat stages and consumed by append statements.
#[derive(Debug)]
pub struct SourceCopies<'a> {
    input: &'a Input,
    owner: Rc<ModelFacts<'a>>,
    parts: Vec<EditPart>,
    kind: CopyKind,
}
#[derive(Debug)]
enum CopyKind {
    Members {
        declaration: DeclarationId,
        maps: Vec<BTreeMap<String, String>>,
    },
    Elements {
        array: bool,
    },
}
enum CopyRequest<'a, 'v> {
    Members { count: usize, prefix: String },
    Elements(&'v SourceCopies<'a>),
    Repeat(usize),
}
impl Parser<'_> {
    pub(super) fn parse_copy_stage(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        budget: &mut Budget,
    ) -> Result<Stage, zirql::QueryError> {
        let name = reader.word()?;
        reader.expect('(')?;
        let stage = if name.value == "clone_elements" {
            let binding = reader.word()?;
            if self.names.get(binding.value) != Some(&Kind::MemberCopies) {
                return Err(zirql::QueryError::query(
                    binding.range,
                    "clone_elements requires a member-copy binding",
                ));
            }
            CopyStage::Elements(Located::new(binding.value.to_owned(), binding.range))
        } else {
            let count = self.copy_argument(reader, budget)?;
            if name.value == "clone_members" {
                reader.expect(',')?;
                CopyStage::Members(count, self.copy_argument(reader, budget)?)
            } else {
                CopyStage::Repeat(count)
            }
        };
        reader.expect(')')?;
        self.cursor.position = reader.position();
        self.report_model_demand = true;
        self.source_copy_range = Some(name.range.start..reader.position());
        Ok(Stage::Copy(stage))
    }
    fn copy_argument(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        budget: &mut Budget,
    ) -> Result<ScalarArgument, zirql::QueryError> {
        self.cursor.position = reader.position();
        let range = self
            .cursor
            .scalar_range()
            .map_err(composition::query_error)?;
        reader.advance(self.cursor.position - reader.position())?;
        std::mem::swap(&mut self.preflight, budget);
        let result = self.pure_value::<()>(range.clone());
        std::mem::swap(&mut self.preflight, budget);
        let (_, names) = result.map_err(composition::query_error)?;
        budget.charge(range.len() + 1, zirql::Location::Query(range.clone()))?;
        budget.charge_results(1, zirql::Location::Query(range.clone()))?;
        Ok(ScalarArgument {
            source: self.cursor.source[range.clone()].to_owned(),
            range,
            names,
        })
    }
}
impl NativeHost<'_> {
    pub(super) fn source_copy<'a>(
        &self,
        stage: &CopyStage,
        value: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        if !self.editing {
            return Err(zirql::QueryError::query(
                range.clone(),
                "source-copy stages require an editing program ending with emit",
            )
            .into());
        }
        let facts = context
            .report_facts(budget, range)
            .map_err(zirql::EvaluationError::Host)?
            .ok_or_else(|| {
                zirql::QueryError::query(
                    range.clone(),
                    "source copying requires read-only model context (--model)",
                )
            })?;
        let mut scalar = |arg: &ScalarArgument, budget: &mut Budget| {
            let expression =
                comprehension::read_data(&arg.source, arg.range.clone(), budget, self.nesting)
                    .map_err(zirql::EvaluationError::Host)?;
            self.constructed(
                &expression,
                &arg.names,
                context,
                bindings,
                budget,
                &arg.range,
            )
        };
        let request = match stage {
            CopyStage::Members(count, prefix) => {
                let n = scalar(count, budget)?;
                let p = scalar(prefix, budget)?;
                let zirql::ConstructedValue::String(prefix) = p.data() else {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "clone_members prefix must be a string",
                    )
                    .into());
                };
                charge(budget, prefix.len(), range).map_err(zirql::EvaluationError::Host)?;
                CopyRequest::Members {
                    count: copy_count(&n, range)?,
                    prefix: prefix.clone(),
                }
            }
            CopyStage::Repeat(count) => {
                CopyRequest::Repeat(copy_count(&scalar(count, budget)?, range)?)
            }
            CopyStage::Elements(name) => {
                let ReportValue::SourceCopies(mapping) =
                    functions::unretained(bindings.get(&name.value, name.range.clone())?)
                else {
                    return Err(zirql::QueryError::query(
                        name.range.clone(),
                        "clone_elements requires member copies",
                    )
                    .into());
                };
                let CopyKind::Members { .. } = &mapping.kind else {
                    return Err(zirql::QueryError::query(
                        name.range.clone(),
                        "clone_elements requires member copies",
                    )
                    .into());
                };
                admit_owner(mapping, context.input, &facts, range)
                    .map_err(zirql::EvaluationError::Host)?;
                CopyRequest::Elements(mapping.as_ref())
            }
        };
        let Some(SemanticStream::Inspection(selection)) = value.stream() else {
            return Err(zirql::QueryError::query(
                range.clone(),
                "source copying requires one literal assignment",
            )
            .into());
        };
        let QueryResult::Literals(selection) = selection.as_ref() else {
            return Err(zirql::QueryError::query(
                range.clone(),
                "source copying requires one literal assignment",
            )
            .into());
        };
        if selection.values.len() != 1 || !std::ptr::eq(selection.input, context.input) {
            return Err(zirql::QueryError::query(
                range.clone(),
                "source copying requires one original literal assignment",
            )
            .into());
        }
        let copies = make_copies(
            context.input,
            facts,
            &selection.values[0],
            request,
            budget,
            self.limits,
            range,
        )
        .map_err(zirql::EvaluationError::Host)?;
        Ok(ReportValue::SourceCopies(Rc::new(copies)))
    }
}
fn copy_count(
    value: &zirql::ConstructedValue,
    range: &Range<usize>,
) -> Result<usize, zirql::QueryError> {
    match value.data() {
        zirql::ConstructedValue::Integer(n) => zirql::nonnegative_integer_to_usize(
            *n,
            zirql::Location::Query(range.clone()),
            "additional copies must be a nonnegative integer",
        ),
        _ => Err(zirql::QueryError::query(
            range.clone(),
            "additional copies must be a nonnegative integer",
        )),
    }
}
fn admit_owner<'a>(
    copies: &SourceCopies<'a>,
    input: &Input,
    facts: &ModelFacts<'a>,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    if !std::ptr::eq(copies.input, input) || !std::ptr::eq(copies.owner.as_ref(), facts) {
        return Err(QueryError::query(
            range.clone(),
            "source copies belong to a different input or model owner",
        ));
    }
    Ok(())
}
fn assignment<'a>(
    input: &'a Input,
    name: &str,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<&'a SyntaxNode, QueryError> {
    let mut result = None;
    for node in input.syntax().tree().child_nodes() {
        charge(budget, 1, range)?;
        if node.kind() == N::Assignment
            && structured::direct_token(input, node).is_some_and(|token| {
                crate::identifier_identity(std::str::from_utf8(input.token_bytes(token)).unwrap())
                    == name
            })
            && result.replace(node).is_some()
        {
            return Err(QueryError::input(
                input.original_range(node.range()),
                "source edits require exactly one original assignment",
            ));
        }
    }
    result.ok_or_else(|| {
        QueryError::query(range.clone(), format!("missing data assignment for {name}"))
    })
}
fn source_assignment<'a>(
    input: &'a Input,
    value: &LiteralValue<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(&'a str, &'a SyntaxNode), QueryError> {
    for node in input.syntax().tree().child_nodes() {
        charge(budget, 1, range)?;
        if let Some(rhs) = crate::assignment_rhs(node)
            && (input.original_range(rhs.range()) == value.range
                || input.original_range(structured::unwrapped(rhs).range()) == value.range)
        {
            let token = structured::direct_token(input, node).unwrap();
            return Ok((
                crate::identifier_identity(std::str::from_utf8(input.token_bytes(token)).unwrap()),
                structured::unwrapped(rhs),
            ));
        }
    }
    Err(QueryError::input(
        value.range.clone(),
        "source copying requires a complete assignment RHS",
    ))
}
fn uninitialized(
    facts: &ModelFacts<'_>,
    name: &str,
    role: DeclarationRole,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<DeclarationId, QueryError> {
    let declaration = comprehension::unique_declaration(facts, name, role, budget, range)?;
    let node = comprehension::declaration_node(facts, declaration.id, budget, range)?.unwrap();
    let children = node
        .child_nodes()
        .filter(|n| n.kind() != N::Annotation)
        .count();
    if children != usize::from(role == DeclarationRole::Value) {
        return Err(QueryError::query(
            range.clone(),
            "source edits require an uninitialized model declaration",
        ));
    }
    Ok(declaration.id)
}
fn interior(input: &Input, node: &SyntaxNode) -> Result<(Range<usize>, bool), QueryError> {
    let array = node.kind() == N::ArrayLiteral;
    if !array && node.kind() != N::SetLiteral {
        return Err(QueryError::input(
            input.original_range(node.range()),
            "source copying supports direct sets and rank-one arrays",
        ));
    }
    let r = input.original_range(node.range());
    Ok((r.start + 1..r.end - 1, array))
}
fn trailing_comma(input: &Input, interior: &Range<usize>) -> bool {
    input
        .syntax()
        .tokens()
        .iter()
        .enumerate()
        .rev()
        .find(|(i, t)| {
            let r = input.token_range(*i);
            interior.start <= r.start
                && r.end <= interior.end
                && !matches!(t.kind, T::Whitespace | T::LineComment | T::BlockComment)
        })
        .is_some_and(|(_, t)| t.kind == T::Comma)
}
fn make_copies<'a>(
    input: &'a Input,
    owner: Rc<ModelFacts<'a>>,
    value: &LiteralValue<'a>,
    request: CopyRequest<'a, '_>,
    budget: &mut Budget,
    limits: Limits,
    range: &Range<usize>,
) -> Result<SourceCopies<'a>, QueryError> {
    let (count, prefix, mapping) = match &request {
        CopyRequest::Members { count, prefix } => (*count, Some(prefix.as_str()), None),
        CopyRequest::Repeat(count) => (*count, None, None),
        CopyRequest::Elements(mapping) => {
            let CopyKind::Members { maps, .. } = &mapping.kind else {
                unreachable!()
            };
            (maps.len(), None, Some(*mapping))
        }
    };
    let (name, node) = source_assignment(input, value, budget, range)?;
    let (inside, array) = interior(input, node)?;
    let units = count
        .checked_mul(inside.len().max(1))
        .ok_or_else(|| QueryError::query(range.clone(), "source copy size overflow"))?;
    charge(budget, units, range)?;
    budget
        .charge_results(count, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    if count > limits.collection {
        return Err(QueryError::query(
            range.clone(),
            "query collection limit exceeded",
        ));
    }
    for (i, token) in input.syntax().tokens().iter().enumerate() {
        charge(budget, 1, range)?;
        let r = input.token_range(i);
        if inside.start <= r.start
            && r.end <= inside.end
            && matches!(token.kind, T::LineComment | T::BlockComment)
            && crate::directive(input.token_bytes(i)).is_some()
        {
            return Err(QueryError::input(r, "cannot copy source directives"));
        }
    }
    let mut checker = Validator::new(input, &owner, budget, limits, range);
    let mut replacements = Vec::new();
    let kind = if let Some(prefix) = prefix {
        if prefix.is_empty()
            || !prefix.as_bytes()[0].is_ascii_alphabetic()
            || !prefix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(QueryError::query(
                range.clone(),
                "member prefix must be a plain identifier",
            ));
        }
        let id = uninitialized(&owner, name, DeclarationRole::Enum, checker.budget, range)?;
        let members = checker.members(id)?;
        if members.is_empty() {
            return Err(QueryError::input(
                value.range.clone(),
                "clone_members requires a nonempty enum",
            ));
        }
        collect_member_tokens(
            input,
            value,
            &members,
            &mut replacements,
            checker.budget,
            range,
        )?;
        let mut maps = Vec::with_capacity(count);
        let mut names = existing_names(input, &owner, checker.budget, range)?;
        for copy in 1..=count {
            let mut map = BTreeMap::new();
            for (position, member) in members.iter().enumerate() {
                charge(checker.budget, prefix.len() + member.len() + 48, range)?;
                checker
                    .budget
                    .charge_results(1, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                let generated = format!("{prefix}_{copy}_{}", position + 1);
                charge(checker.budget, generated.len() + 1, range)?;
                checker
                    .budget
                    .charge_results(1, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                if !names.insert(generated.clone()) {
                    return Err(QueryError::query(
                        range.clone(),
                        format!("generated member collision: {generated}"),
                    ));
                }
                map.insert(member.clone(), generated);
            }
            maps.push(map);
        }
        CopyKind::Members {
            declaration: id,
            maps,
        }
    } else {
        let id = uninitialized(&owner, name, DeclarationRole::Value, checker.budget, range)?;
        let shape = reduction::declaration_shape(
            &owner,
            id,
            &mut Work {
                budget: checker.budget,
            },
            range,
            limits,
        )?;
        if let Some(mapping) = mapping {
            let CopyKind::Members { declaration, .. } = &mapping.kind else {
                unreachable!()
            };
            checker.remap = Some(*declaration);
        }
        let metadata = comprehension::declared_value(&owner, name, checker.budget, range)?.unwrap();
        checker.validate(value, &shape, metadata, 0)?;
        replacements = std::mem::take(&mut checker.replacements);
        CopyKind::Elements { array }
    };
    let maps = match &kind {
        CopyKind::Members { maps, .. } => Some(maps),
        _ => mapping.and_then(|m| match &m.kind {
            CopyKind::Members { maps, .. } => Some(maps),
            _ => None,
        }),
    };
    let entries = match &value.kind {
        L::Set(v) => v.len(),
        L::Array { indexing, entries } if *indexing != structured::ArrayIndexing::StartingKey => {
            entries.len()
        }
        _ => {
            return Err(QueryError::input(
                value.range.clone(),
                "source copying requires a direct rank-one collection",
            ));
        }
    };
    let literal_count = literal_copy_units(value, checker.budget, range)?;
    let copied_literals = count
        .checked_mul(literal_count)
        .ok_or_else(|| QueryError::query(range.clone(), "source copy size overflow"))?;
    checker
        .budget
        .charge_results(copied_literals, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    let part_count = count
        .checked_mul(
            replacements
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(2))
                .ok_or_else(|| QueryError::query(range.clone(), "source copy size overflow"))?,
        )
        .ok_or_else(|| QueryError::query(range.clone(), "source copy size overflow"))?;
    charge(checker.budget, part_count, range)?;
    checker
        .budget
        .charge_results(part_count, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    let mut parts = Vec::with_capacity(if entries > 0 { part_count } else { 0 });
    if entries > 0 {
        for copy in 0..count {
            if copy > 0 && !trailing_comma(input, &inside) {
                parts.push(EditPart::Text(",".into()));
            }
            let mut start = inside.start;
            for (token, member) in &replacements {
                let Some(map) = maps.map(|maps| &maps[copy]) else {
                    break;
                };
                parts.push(EditPart::Original(start..token.start));
                charge(checker.budget, map[member].len(), range)?;
                parts.push(EditPart::Text(map[member].clone()));
                start = token.end;
            }
            parts.push(EditPart::Original(start..inside.end));
        }
    }
    Ok(SourceCopies {
        input,
        owner,
        parts,
        kind,
    })
}
fn literal_copy_units(
    value: &LiteralValue<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<usize, QueryError> {
    charge(budget, 1, range)?;
    let mut units = 1;
    match &value.kind {
        L::Set(values) | L::Tuple(values) => {
            for value in values {
                units += literal_copy_units(value, budget, range)?;
            }
        }
        L::Record(fields) => {
            for field in fields {
                units += literal_copy_units(&field.value, budget, range)?;
            }
        }
        L::Array { entries, .. } => {
            for entry in entries {
                if let Some(key) = &entry.key {
                    units += literal_copy_units(key, budget, range)?;
                }
                units += literal_copy_units(&entry.value, budget, range)?;
            }
        }
        _ => {}
    }
    Ok(units)
}

fn collect_member_tokens(
    input: &Input,
    value: &LiteralValue<'_>,
    members: &[String],
    output: &mut Vec<(Range<usize>, String)>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    charge(budget, 1 + members.len(), range)?;
    match &value.kind {
        L::Member(name) if members.iter().any(|m| m == name) => {
            charge(budget, name.len() + 1, range)?;
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            output.push((
                member_token_range(input, value, budget, range)?,
                (*name).to_owned(),
            ));
        }
        L::Set(v) | L::Tuple(v) => {
            for v in v {
                collect_member_tokens(input, v, members, output, budget, range)?;
            }
        }
        L::Record(fields) => {
            for f in fields {
                collect_member_tokens(input, &f.value, members, output, budget, range)?;
            }
        }
        L::Array { entries, .. } => {
            for e in entries {
                if let Some(key) = &e.key {
                    collect_member_tokens(input, key, members, output, budget, range)?;
                }
                collect_member_tokens(input, &e.value, members, output, budget, range)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn member_token_range(
    input: &Input,
    value: &LiteralValue<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Range<usize>, QueryError> {
    let tokens = input.syntax().tokens();
    let start =
        tokens.partition_point(|t| t.range.end + input.syntax_offset() <= value.range.start);
    for (i, token) in tokens.iter().enumerate().skip(start) {
        charge(budget, 1, range)?;
        if token.range.start + input.syntax_offset() >= value.range.end {
            break;
        }
        if matches!(token.kind, T::Identifier | T::QuotedIdentifier) {
            return Ok(input.token_range(i));
        }
    }
    Err(QueryError::input(
        value.range.clone(),
        "member has no identifier token",
    ))
}

fn existing_names(
    input: &Input,
    facts: &ModelFacts<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<BTreeSet<String>, QueryError> {
    let mut names = BTreeSet::new();
    for (i, token) in input.syntax().tokens().iter().enumerate() {
        charge(budget, 1 + token.range.len(), range)?;
        if matches!(token.kind, T::Identifier | T::QuotedIdentifier) {
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            names.insert(
                crate::identifier_identity(std::str::from_utf8(input.token_bytes(i)).unwrap())
                    .to_owned(),
            );
        }
    }
    for declaration in &facts.bindings().declarations {
        charge(budget, declaration.name.len() + 1, range)?;
        budget
            .charge_results(1, zirql::Location::Query(range.clone()))
            .map_err(pipeline::core_error)?;
        names.insert(declaration.name.clone());
    }
    Ok(names)
}

pub(crate) struct EnumScale {
    declaration: DeclarationId,
    pub(crate) edit: Option<TextEdit>,
}

/// A constructor domain supplies a count, never synthesized member identities.
struct ConstructorDomain<'a> {
    name: &'a str,
    name_range: Range<usize>,
    bound_range: Range<usize>,
    count: i64,
}

fn constructor_domain<'a>(
    input: &'a Input,
    rhs: &SyntaxNode,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ConstructorDomain<'a>, QueryError> {
    let rhs = structured::unwrapped(rhs);
    let invalid = || {
        QueryError::input(
            input.original_range(rhs.range()),
            "scale_enum requires a named constructor C(1..N) with a positive literal integer N",
        )
    };
    let mut significant = Vec::new();
    let tokens = input.syntax().tokens();
    let start = tokens.partition_point(|token| token.range.start < rhs.range().start);
    for (offset, token) in tokens[start..]
        .iter()
        .enumerate()
        .take_while(|(_, t)| t.range.end <= rhs.range().end)
    {
        charge(budget, 1, range)?;
        if !matches!(token.kind, T::Whitespace | T::LineComment | T::BlockComment) {
            if significant.len() == 6 {
                return Err(invalid());
            }
            significant.push(start + offset);
        }
    }
    let [name, left, low, dots, high, right] = significant.as_slice() else {
        return Err(invalid());
    };
    if !matches!(tokens[*name].kind, T::Identifier | T::QuotedIdentifier)
        || tokens[*low].kind != T::IntegerLiteral
        || tokens[*high].kind != T::IntegerLiteral
        || input.token_bytes(*left) != b"("
        || input.token_bytes(*dots) != b".."
        || input.token_bytes(*right) != b")"
    {
        return Err(invalid());
    }
    let integer = |index, budget: &mut Budget| {
        let literal = LiteralValue {
            file: "",
            range: input.token_range(index),
            spelling: input.token_bytes(index),
            kind: L::Integer,
        };
        comprehension::written_scalar(&literal, input, budget)
    };
    let zirql::ConstructedValue::Integer(count) = integer(*high, budget)? else {
        return Err(invalid());
    };
    if integer(*low, budget)? != zirql::ConstructedValue::Integer(1) || count <= 0 {
        return Err(invalid());
    }
    let name_text =
        crate::identifier_identity(std::str::from_utf8(input.token_bytes(*name)).unwrap());
    if name_text == "anon_enum" {
        return Err(invalid());
    }
    Ok(ConstructorDomain {
        name: name_text,
        name_range: input.token_range(*name),
        bound_range: input.token_range(*high),
        count,
    })
}

pub(crate) fn prepare_scale(
    input: &Input,
    facts: &ModelFacts<'_>,
    edit: &data::Edit,
    factor: &zirql::ConstructedValue,
    budget: &mut Budget,
) -> Result<EnumScale, QueryError> {
    let zirql::ConstructedValue::Integer(factor) = factor.data() else {
        return Err(QueryError::query(
            edit.range.clone(),
            "scale_enum factor must be a positive integer",
        ));
    };
    if *factor <= 0 {
        return Err(QueryError::query(
            edit.range.clone(),
            "scale_enum factor must be a positive integer",
        ));
    }
    let declaration = uninitialized(
        facts,
        &edit.name.value,
        DeclarationRole::Enum,
        budget,
        &edit.name.range,
    )?;
    let rhs = crate::assignment_rhs(assignment(
        input,
        &edit.name.value,
        budget,
        &edit.name.range,
    )?)
    .unwrap();
    let domain = constructor_domain(input, rhs, budget, &edit.range)?;
    let count = domain.count.checked_mul(*factor).ok_or_else(|| {
        QueryError::query(
            edit.range.clone(),
            "scale_enum bound multiplication overflow",
        )
    })?;
    for (i, token) in input.syntax().tokens().iter().enumerate() {
        charge(budget, 1, &edit.range)?;
        if matches!(token.kind, T::Identifier | T::QuotedIdentifier)
            && input.token_range(i) != domain.name_range
            && crate::identifier_identity(std::str::from_utf8(input.token_bytes(i)).unwrap())
                == domain.name
        {
            return Err(QueryError::input(
                input.token_range(i),
                "scale_enum cannot remap constructor references outside its defining assignment",
            ));
        }
    }
    charge(budget, 22, &edit.range)?;
    Ok(EnumScale {
        declaration,
        edit: (count != domain.count).then(|| TextEdit {
            range: domain.bound_range,
            replacement: vec![EditPart::Text(count.to_string())],
        }),
    })
}

// Keep the three edit inputs explicit alongside their shared validation context.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_appends<'a>(
    input: &Input,
    facts: &ModelFacts<'a>,
    copies: &[(&Located<String>, Rc<SourceCopies<'a>>)],
    edits: &[&data::Edit],
    scales: &[EnumScale],
    budget: &mut Budget,
    limits: Limits,
    range: &Range<usize>,
) -> Result<Vec<TextEdit>, QueryError> {
    if copies.is_empty() && scales.is_empty() {
        return Ok(Vec::new());
    }
    let mut names = existing_names(input, facts, budget, range)?;
    let mut changed: Vec<_> = scales.iter().map(|scale| scale.declaration).collect();
    for (target, copy) in copies {
        admit_owner(copy, input, facts, range)?;
        if let CopyKind::Members { declaration, maps } = &copy.kind {
            if facts.bindings().declarations[declaration.0].name != target.value {
                return Err(QueryError::query(
                    target.range.clone(),
                    "member copies must append to their own enum",
                ));
            }
            if !maps.is_empty() {
                changed.push(*declaration);
            }
            for map in maps {
                for name in map.values() {
                    charge(budget, name.len() + 1, range)?;
                    budget
                        .charge_results(1, zirql::Location::Query(range.clone()))
                        .map_err(pipeline::core_error)?;
                    if !names.insert(name.clone()) {
                        return Err(QueryError::query(
                            target.range.clone(),
                            format!("generated member collision: {name}"),
                        ));
                    }
                }
            }
        }
    }
    for edit in edits {
        let id = comprehension::unique_declaration(
            facts,
            &edit.name.value,
            DeclarationRole::Value,
            budget,
            &edit.name.range,
        )?
        .id;
        let shape = reduction::declaration_shape(facts, id, &mut Work { budget }, range, limits)?;
        if changed.iter().any(|id| shape.contains_enum(Some(*id))) {
            return Err(QueryError::query(
                edit.name.range.clone(),
                "constructed edits depending on a changed enum cannot share a source-copy batch",
            ));
        }
    }
    let mut result = Vec::new();
    for (target, copy) in copies {
        let rhs = structured::unwrapped(
            crate::assignment_rhs(assignment(input, &target.value, budget, &target.range)?)
                .unwrap(),
        );
        let (inside, array) = interior(input, rhs)?;
        match &copy.kind {
            CopyKind::Members { .. } if array => {
                return Err(QueryError::query(
                    target.range.clone(),
                    "enum append requires a member set",
                ));
            }
            CopyKind::Elements { array: expected } => {
                uninitialized(
                    facts,
                    &target.value,
                    DeclarationRole::Value,
                    budget,
                    &target.range,
                )?;
                if array != *expected {
                    return Err(QueryError::query(
                        target.range.clone(),
                        "append collection kinds differ",
                    ));
                }
            }
            _ => {}
        }
        if copy.parts.is_empty() {
            continue;
        }
        let mut replacement = Vec::new();
        charge(budget, inside.len() + copy.parts.len(), range)?;
        let nonempty = rhs.child_nodes().next().is_some();
        if nonempty && !trailing_comma(input, &inside) {
            replacement.push(EditPart::Text(",".into()));
        }
        budget
            .charge_results(copy.parts.len() + 2, zirql::Location::Query(range.clone()))
            .map_err(pipeline::core_error)?;
        for part in &copy.parts {
            charge(
                budget,
                match part {
                    EditPart::Text(s) => s.len(),
                    EditPart::Original(r) => r.len(),
                },
                range,
            )?;
            replacement.push(part.clone());
        }
        result.push(TextEdit {
            range: inside.end..inside.end,
            replacement,
        });
    }
    Ok(result)
}

pub(crate) fn validate_candidate(
    candidate: &Input,
    facts: &ModelFacts<'_>,
    copies: &[(&Located<String>, Rc<SourceCopies<'_>>)],
    scales: &[EnumScale],
    budget: &mut Budget,
    limits: Limits,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    if copies.is_empty() && scales.is_empty() {
        return Ok(());
    }
    charge(budget, copies.len() + scales.len(), range)?;
    budget
        .charge_results(copies.len(), zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    let changed: Vec<_> = copies
        .iter()
        .filter_map(|(_, c)| match &c.kind {
            CopyKind::Members { declaration, maps } if !maps.is_empty() => Some(*declaration),
            _ => None,
        })
        .chain(scales.iter().map(|scale| scale.declaration))
        .collect();
    let mut checker = Validator::new(candidate, facts, budget, limits, range);
    for node in candidate.syntax().tree().child_nodes() {
        charge(checker.budget, 1, range)?;
        let Some(rhs) = crate::assignment_rhs(node) else {
            continue;
        };
        let token = structured::direct_token(candidate, node).unwrap();
        let name =
            crate::identifier_identity(std::str::from_utf8(candidate.token_bytes(token)).unwrap());
        let targeted = copies.iter().any(|(target, _)| target.value == name);
        charge(checker.budget, facts.bindings().declarations.len(), range)?;
        let mut declarations = facts
            .bindings()
            .declarations
            .iter()
            .filter(|d| d.top_level && d.name == name);
        let declaration = declarations.next();
        if declaration.is_none() || declarations.next().is_some() {
            return Err(QueryError::input(
                candidate.original_range(rhs.range()),
                "data declaration identity is unavailable or ambiguous",
            ));
        }
        let declaration = declaration.unwrap();
        if declaration.role == DeclarationRole::Enum {
            if targeted {
                checker.members(declaration.id)?;
            }
            continue;
        }
        if declaration.role != DeclarationRole::Value {
            continue;
        }
        let shape = reduction::declaration_shape(
            facts,
            declaration.id,
            &mut Work {
                budget: checker.budget,
            },
            range,
            limits,
        )?;
        if targeted || changed.iter().any(|id| shape.contains_enum(Some(*id))) {
            assignment(candidate, name, checker.budget, range)?;
            let value = structured::read_literal(
                candidate,
                rhs,
                &mut Work {
                    budget: checker.budget,
                },
                range,
                limits,
            )?;
            let metadata =
                comprehension::declared_value(facts, name, checker.budget, range)?.unwrap();
            checker.validate(&value, &shape, metadata, 0)?;
        }
    }
    Ok(())
}
enum Axis {
    Keys(Vec<String>),
    Cardinality(usize),
}
impl Axis {
    fn len(&self) -> usize {
        match self {
            Self::Keys(keys) => keys.len(),
            Self::Cardinality(n) => *n,
        }
    }
}
struct Validator<'a, 'm, 'model, 'b> {
    input: &'a Input,
    facts: &'m ModelFacts<'model>,
    budget: &'b mut Budget,
    limits: Limits,
    range: &'b Range<usize>,
    enums: BTreeMap<usize, Vec<String>>,
    remap: Option<DeclarationId>,
    replacements: Vec<(Range<usize>, String)>,
}
impl<'a, 'm, 'model, 'b> Validator<'a, 'm, 'model, 'b> {
    fn new(
        input: &'a Input,
        facts: &'m ModelFacts<'model>,
        budget: &'b mut Budget,
        limits: Limits,
        range: &'b Range<usize>,
    ) -> Self {
        Self {
            input,
            facts,
            budget,
            limits,
            range,
            enums: BTreeMap::new(),
            remap: None,
            replacements: Vec::new(),
        }
    }
    fn members(&mut self, id: DeclarationId) -> Result<Vec<String>, QueryError> {
        if let Some(members) = self.enums.get(&id.0) {
            charge(
                self.budget,
                members.iter().map(|m| m.len() + 1).sum(),
                self.range,
            )?;
            self.budget
                .charge_results(members.len(), zirql::Location::Query(self.range.clone()))
                .map_err(pipeline::core_error)?;
            return Ok(members.clone());
        }
        let declaration = &self.facts.bindings().declarations[id.0];
        comprehension::unique_declaration(
            self.facts,
            &declaration.name,
            DeclarationRole::Enum,
            self.budget,
            self.range,
        )?;
        let node =
            comprehension::declaration_node(self.facts, id, self.budget, self.range)?.unwrap();
        let members = if node.child_nodes().any(|n| n.kind() != N::Annotation) {
            comprehension::enum_cases(self.facts, declaration, self.budget, self.range)?
                .iter()
                .map(|case| case.name.clone())
                .collect::<Vec<_>>()
        } else {
            let rhs = crate::assignment_rhs(assignment(
                self.input,
                &declaration.name,
                self.budget,
                self.range,
            )?)
            .unwrap();
            let value = structured::read_literal(
                self.input,
                rhs,
                &mut Work {
                    budget: self.budget,
                },
                self.range,
                self.limits,
            )?;
            let L::Set(values) = value.kind else {
                return Err(QueryError::input(
                    value.range,
                    "enum membership requires a literal member set",
                ));
            };
            let mut members = Vec::new();
            let mut seen = HashSet::new();
            for value in values {
                charge(self.budget, value.spelling.len() + 1, self.range)?;
                let L::Member(name) = value.kind else {
                    return Err(QueryError::input(
                        value.range,
                        "enum set requires member identifiers",
                    ));
                };
                if !seen.insert(name) {
                    return Err(QueryError::input(value.range, "duplicate enum member"));
                }
                members.push(name.to_owned());
            }
            members
        };
        charge(
            self.budget,
            members.iter().map(|m| m.len() + 1).sum(),
            self.range,
        )?;
        self.budget
            .charge_results(members.len(), zirql::Location::Query(self.range.clone()))
            .map_err(pipeline::core_error)?;
        self.enums.insert(id.0, members.clone());
        Ok(members)
    }
    fn member_token(
        &mut self,
        value: &LiteralValue<'_>,
        id: DeclarationId,
        name: &str,
    ) -> Result<(), QueryError> {
        if self.remap == Some(id) {
            charge(self.budget, name.len() + 1, self.range)?;
            self.budget
                .charge_results(1, zirql::Location::Query(self.range.clone()))
                .map_err(pipeline::core_error)?;
            self.replacements.push((
                member_token_range(self.input, value, self.budget, self.range)?,
                name.to_owned(),
            ));
        }
        Ok(())
    }
    fn enum_axis(&mut self, id: DeclarationId) -> Result<Axis, QueryError> {
        let declaration = &self.facts.bindings().declarations[id.0];
        let node =
            comprehension::declaration_node(self.facts, id, self.budget, self.range)?.unwrap();
        if !node.child_nodes().any(|n| n.kind() != N::Annotation) {
            uninitialized(
                self.facts,
                &declaration.name,
                DeclarationRole::Enum,
                self.budget,
                self.range,
            )?;
            let rhs = crate::assignment_rhs(assignment(
                self.input,
                &declaration.name,
                self.budget,
                self.range,
            )?)
            .unwrap();
            if structured::unwrapped(rhs).kind() != N::SetLiteral {
                let domain = constructor_domain(self.input, rhs, self.budget, self.range)?;
                let count = usize::try_from(domain.count).map_err(|_| {
                    QueryError::input(domain.bound_range, "constructor cardinality overflow")
                })?;
                return Ok(Axis::Cardinality(count));
            }
        }
        self.members(id).map(Axis::Keys)
    }
    fn axis(&mut self, domain: &Domain) -> Result<Option<Axis>, QueryError> {
        charge(self.budget, 1, self.range)?;
        match domain {
            Domain::Enum(id) => self.enum_axis(*id).map(Some),
            Domain::Named {
                declaration,
                domain,
            } => {
                if self.facts.bindings().declarations[declaration.0].role == DeclarationRole::Enum {
                    self.enum_axis(*declaration).map(Some)
                } else {
                    self.axis(domain)
                }
            }
            Domain::UnconstrainedInt => Ok(None),
            Domain::Range { lower, upper } => {
                let low = comprehension::integer_bound(
                    lower,
                    self.input,
                    &BTreeMap::new(),
                    self.facts,
                    self.budget,
                    self.range,
                )?;
                let high = comprehension::integer_bound(
                    upper,
                    self.input,
                    &BTreeMap::new(),
                    self.facts,
                    self.budget,
                    self.range,
                )?;
                let n = high
                    .checked_sub(low)
                    .and_then(|n| n.checked_add(1))
                    .unwrap_or(i64::MAX)
                    .max(0);
                let n = usize::try_from(n)
                    .map_err(|_| QueryError::query(self.range.clone(), "axis size overflow"))?;
                charge(self.budget, n.saturating_mul(21), self.range)?;
                self.budget
                    .charge_results(n, zirql::Location::Query(self.range.clone()))
                    .map_err(pipeline::core_error)?;
                if n > self.limits.collection {
                    return Err(QueryError::query(
                        self.range.clone(),
                        "query collection limit exceeded",
                    ));
                }
                Ok(Some(Axis::Keys(
                    (low..=high).map(|n| n.to_string()).collect(),
                )))
            }
            _ => Err(QueryError::query(
                self.range.clone(),
                "source-copy validation requires established literal array domains",
            )),
        }
    }
    fn validate(
        &mut self,
        value: &LiteralValue<'_>,
        shape: &reduction::Shape,
        metadata: comprehension::LiteralMetadata<'_>,
        depth: usize,
    ) -> Result<(), QueryError> {
        charge(self.budget, 1, self.range)?;
        let metadata = comprehension::resolve_metadata(
            metadata,
            self.facts,
            self.budget,
            self.range,
            self.limits.nesting,
        )?;
        let invalid = || {
            QueryError::input(
                value.range.clone(),
                "literal does not satisfy its retained model type or final domain",
            )
        };
        if depth >= self.limits.nesting.min(64)
            || shape.ty.optional
            || shape.ty.instantiation != Instantiation::Parameter
        {
            return Err(invalid());
        }
        match (&value.kind, &shape.ty.kind) {
            (L::Integer, TypeKind::Int | TypeKind::Float) | (L::Float, TypeKind::Float) => {
                let number = comprehension::written_scalar(value, self.input, self.budget)?;
                data::validate_numeric_domain(
                    &number,
                    metadata,
                    self.input,
                    &BTreeMap::new(),
                    self.facts,
                    self.budget,
                    self.range,
                )
                .map_err(|mut error| {
                    if matches!(error.location, crate::ErrorLocation::Query) {
                        error.location = crate::ErrorLocation::Input;
                        error.range = value.range.clone();
                    }
                    error
                })
            }
            (L::Boolean(_), TypeKind::Bool) | (L::String, TypeKind::String) => Ok(()),
            (L::Member(name), TypeKind::Enum(id)) => {
                // Expanded enum types also describe subset domains. Require the
                // retained domain itself to name this enum after alias resolution.
                let mut domain = metadata
                    .node
                    .filter(|node| node.kind() == N::DomainType)
                    .and_then(|node| node.child_nodes().next());
                while let Some(node) = domain
                    && node.kind() == N::ParenthesizedExpression
                {
                    charge(self.budget, 1 + node.children().len(), self.range)?;
                    domain = node.child_nodes().next();
                }
                let location = domain
                    .filter(|node| node.kind() == N::Expression)
                    .map(|node| self.facts.model().files[metadata.file].location(node.range()));
                let mut full_enum = false;
                for reference in &self.facts.bindings().references {
                    charge(self.budget, 1, self.range)?;
                    if reference.file == metadata.file
                        && location.as_ref().is_some_and(|location| {
                            location.range.start <= reference.location.range.start
                                && reference.location.range.end <= location.range.end
                        })
                        && reference.resolution == zincite_lint::BindingResolution::Resolved(*id)
                    {
                        full_enum = true;
                        break;
                    }
                }
                if !full_enum {
                    return Err(QueryError::input(
                        value.range.clone(),
                        "source-copy validation requires a full enum domain; enum subsets are unsupported",
                    ));
                }
                if !self.members(*id)?.iter().any(|m| m == name) {
                    return Err(invalid());
                }
                self.member_token(value, *id, name)?;
                Ok(())
            }
            (L::Set(values), TypeKind::Set(_)) => {
                for v in values {
                    self.validate(
                        v,
                        &shape.components[0].1,
                        comprehension::child_metadata(
                            Some(metadata),
                            &shape.components[0].1.ty,
                            None,
                        ),
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            (L::Tuple(values), TypeKind::Tuple(_)) if values.len() == shape.components.len() => {
                for (i, (v, (_, s))) in values.iter().zip(&shape.components).enumerate() {
                    self.validate(
                        v,
                        s,
                        comprehension::child_metadata(Some(metadata), &s.ty, Some(i)),
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            (L::Record(fields), TypeKind::Record(_)) if fields.len() == shape.components.len() => {
                let mut seen = HashSet::new();
                for f in fields {
                    charge(
                        self.budget,
                        shape.components.len() + f.label.name.len(),
                        self.range,
                    )?;
                    self.budget
                        .charge_results(1, zirql::Location::Query(self.range.clone()))
                        .map_err(pipeline::core_error)?;
                    let Some((_, s)) = shape
                        .components
                        .iter()
                        .find(|(name, _)| name.as_deref() == Some(f.label.name))
                    else {
                        return Err(invalid());
                    };
                    if !seen.insert(f.label.name) {
                        return Err(invalid());
                    }
                    let child = data::record_metadata(
                        metadata,
                        f.label.name,
                        &s.ty,
                        self.facts,
                        self.budget,
                        self.range,
                    )?;
                    self.validate(&f.value, s, child, depth + 1)?;
                }
                Ok(())
            }
            (L::Array { indexing, entries }, TypeKind::Array { indices, .. })
                if indices.len() == 1 && *indexing != structured::ArrayIndexing::StartingKey =>
            {
                let axis = self.axis(&shape.indices[0])?;
                if let Some(axis) = &axis
                    && axis.len() != entries.len()
                {
                    return Err(QueryError::input(
                        value.range.clone(),
                        "array coverage differs from the final domain; append every affected assignment",
                    ));
                }
                let mut keys = BTreeSet::new();
                for entry in entries {
                    if let Some(key) = &entry.key {
                        if matches!(axis, Some(Axis::Cardinality(_))) {
                            return Err(QueryError::input(
                                key.range.clone(),
                                "constructor-indexed arrays require positional entries",
                            ));
                        }
                        charge(
                            self.budget,
                            key.spelling.len().saturating_mul(2)
                                + axis.as_ref().map_or(0, Axis::len)
                                + 1,
                            self.range,
                        )?;
                        self.budget
                            .charge_results(2, zirql::Location::Query(self.range.clone()))
                            .map_err(pipeline::core_error)?;
                        if let (L::Member(name), TypeKind::Enum(id)) = (&key.kind, &indices[0].kind)
                        {
                            self.member_token(key, *id, name)?;
                        }
                        let key = match &key.kind {
                            L::Member(name) if matches!(indices[0].kind, TypeKind::Enum(_)) => {
                                (*name).to_owned()
                            }
                            L::Integer if indices[0].kind == TypeKind::Int => {
                                match comprehension::written_scalar(key, self.input, self.budget)? {
                                    zirql::ConstructedValue::Integer(n) => n.to_string(),
                                    _ => return Err(invalid()),
                                }
                            }
                            _ => return Err(invalid()),
                        };
                        if !keys.insert(key.clone())
                            || axis.as_ref().is_some_and(
                                |axis| matches!(axis, Axis::Keys(keys) if !keys.contains(&key)),
                            )
                        {
                            return Err(invalid());
                        }
                    }
                    self.validate(
                        &entry.value,
                        &shape.components[0].1,
                        comprehension::child_metadata(
                            Some(metadata),
                            &shape.components[0].1.ty,
                            None,
                        ),
                        depth + 1,
                    )?;
                }
                if *indexing == structured::ArrayIndexing::Explicit && axis.is_none() {
                    charge(
                        self.budget,
                        keys.len()
                            .saturating_mul(keys.len().max(1).ilog2() as usize + 1),
                        self.range,
                    )?;
                    self.budget
                        .charge_results(keys.len(), zirql::Location::Query(self.range.clone()))
                        .map_err(pipeline::core_error)?;
                    let mut integers = keys
                        .iter()
                        .map(|key| key.parse::<i64>().map_err(|_| invalid()))
                        .collect::<Result<Vec<_>, _>>()?;
                    integers.sort_unstable();
                    if integers
                        .windows(2)
                        .any(|pair| pair[0].checked_add(1) != Some(pair[1]))
                    {
                        return Err(invalid());
                    }
                }
                Ok(())
            }
            _ => Err(invalid()),
        }
    }
}

impl SourceCopies<'_> {
    pub(super) fn kind(&self) -> Kind {
        match self.kind {
            CopyKind::Members { .. } => Kind::MemberCopies,
            CopyKind::Elements { .. } => Kind::SourceCopies,
        }
    }
}
