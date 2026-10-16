//! Shared expression reading and native MiniZinc projection.
use crate::{Input, QueryError, QueryResult, SemanticStream, pipeline, structured};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
};
use zincite_lint::{
    Declaration, DeclarationId, DeclarationRole, Domain, ModelFacts, NumericBound, TypeInst,
    TypeKind,
};
use zincite_syntax::{NodeKind as N, SyntaxElement, SyntaxNode, TokenKind as T};
use zirql::{
    Budget, ConstructedValue as V, Located, ValueBinaryOperator as B, ValueExpression as E,
    ValueExpressionKind as K, ValueGenerator, ValueUnaryOperator as U,
};

#[derive(Debug)]
pub(crate) struct LoweredValue<Q = ()> {
    pub expression: E<Q>,
    pub members: Vec<MemberRequest>,
}
#[derive(Clone, Debug)]
pub(crate) struct MemberRequest {
    pub slot: String,
    pub enum_name: Located<String>,
    pub member_name: Located<String>,
    pub range: Range<usize>,
}

fn charge(budget: &mut Budget, units: usize, range: &Range<usize>) -> Result<(), QueryError> {
    budget
        .charge(units, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)
}
fn slots<T>(budget: &mut Budget, len: usize, range: &Range<usize>) -> Result<Vec<T>, QueryError> {
    if len
        .checked_mul(std::mem::size_of::<T>())
        .is_none_or(|n| n > isize::MAX as usize)
    {
        return Err(QueryError::query(range.clone(), "allocation size overflow"));
    }
    charge(budget, len, range)?;
    budget
        .charge_results(len, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    Ok(Vec::with_capacity(len))
}

/// Read the closed constructed Data subset with the common query reader.
pub(crate) fn read_data<Q>(
    source: &str,
    range: Range<usize>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<LoweredValue<Q>, QueryError> {
    charge(budget, range.start + source.len(), &range)?;
    let text = format!("{}{source}", " ".repeat(range.start));
    let mut reader =
        zirql::ArgumentReader::new(&text, range.start).map_err(pipeline::core_error)?;
    let mut host = DataReader {
        members: Vec::new(),
    };
    let value = zirql::read_value_expression(&mut reader, &mut host, budget, nesting)
        .map_err(pipeline::core_error)?;
    reader.skip_whitespace();
    if reader.position() != range.end {
        return Err(QueryError::query(
            reader.position()..range.end,
            "expected one data expression",
        ));
    }
    admit_data(&value, &text, budget, &mut |_, range, _| {
        Err(QueryError::query(
            range.clone(),
            "native expression is unavailable here",
        ))
    })?;
    let expression = crate::program::builder::map_queries(value, &mut |_| {
        unreachable!("Data reader has no native values")
    });
    Ok(LoweredValue {
        expression,
        members: host.members,
    })
}
struct DataReader {
    members: Vec<MemberRequest>,
}
impl zirql::ValueReaderHost for DataReader {
    type Stage = ();
    type SetOperator = ();
    type Emitter = ();
    type Predicate = ();
    fn is_binding(&self, _: &str) -> bool {
        false
    }
    fn is_stage(&self, _: &str) -> bool {
        false
    }
    fn parse_stage(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        _: &mut Budget,
    ) -> Result<zirql::ExpressionStep<(), ()>, zirql::QueryError> {
        Err(zirql::QueryError::query(
            reader.position()..reader.position(),
            "native stages are unavailable here",
        ))
    }
    fn parse_predicate_leaf(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        _: &mut Budget,
    ) -> Result<(), zirql::QueryError> {
        Err(zirql::QueryError::query(
            reader.position()..reader.position(),
            "native predicates are unavailable here",
        ))
    }
    fn filter(&mut self, _: zirql::Predicate<()>, _: Range<usize>) {}
    fn projection(&mut self, _: zirql::Projection<zirql::Expression<(), (), ()>>, _: Range<usize>) {
    }
    fn set_operator(&self, _: &str) -> Option<()> {
        None
    }
    fn parse_domain_call(
        &mut self,
        name: Located<&str>,
        reader: &mut zirql::ArgumentReader<'_>,
        _: &[&str],
        budget: &mut Budget,
        _: usize,
    ) -> Result<Option<E<zirql::Expression<(), (), ()>>>, zirql::QueryError> {
        let Some(mut member) = read_member(name, reader, budget)? else {
            return Ok(None);
        };
        member.slot = format!("\0member{}", self.members.len());
        let value = Located::new(K::Name(member.slot.clone()), member.range.clone());
        self.members.push(member);
        Ok(Some(value))
    }
}
/// Read only the native enum/member signature, leaving other calls untouched.
pub(crate) fn read_member(
    name: Located<&str>,
    reader: &mut zirql::ArgumentReader<'_>,
    budget: &mut Budget,
) -> Result<Option<MemberRequest>, zirql::QueryError> {
    if name.value != "enum_member" {
        return Ok(None);
    }
    reader.expect('(')?;
    let enum_name = reader.word()?;
    let enum_name = Located::new(enum_name.value.to_owned(), enum_name.range);
    reader.expect(',')?;
    let member_name = reader.quoted_string()?;
    reader.expect(')')?;
    let range = name.range.start..reader.position();
    budget.charge(
        enum_name.value.len() + member_name.value.len() + 1,
        zirql::Location::Query(range.clone()),
    )?;
    budget.charge_results(1, zirql::Location::Query(range.clone()))?;
    Ok(Some(MemberRequest {
        slot: String::new(),
        enum_name,
        member_name,
        range,
    }))
}
/// Check every written branch before a Data request can perform an effect.
pub(crate) fn admit_data<Q>(
    expression: &E<Q>,
    source: &str,
    budget: &mut Budget,
    native: &mut impl FnMut(&Q, &Range<usize>, &mut Budget) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    let mut pending = vec![expression];
    while let Some(value) = pending.pop() {
        charge(budget, 1, &value.range)?;
        let refused = || {
            QueryError::query(
                value.range.clone(),
                "report-only form is unavailable in data expressions",
            )
        };
        match &value.value {
            K::Literal(V::Absent) => return Err(refused()),
            K::Literal(_) | K::Name(_) | K::PredicateSubject => {}
            K::NativeQuery(query) | K::NativeValue(query) => native(query, &value.range, budget)?,
            K::Array(values) | K::Tuple(values) | K::Set(values) => pending.extend(values),
            K::Record(fields) => {
                if source
                    .get(value.range.clone())
                    .is_some_and(|text| text.trim_start().starts_with('{'))
                {
                    return Err(refused());
                }
                pending.extend(fields.iter().map(|(_, value)| value));
            }
            K::KeyedArray {
                declared_axis_sources,
                entries,
                ..
            } => {
                pending.extend(declared_axis_sources.iter().flatten());
                pending.extend(entries.iter().flat_map(|(key, value)| [key, value]));
            }
            K::Comprehension { clauses, head, .. }
            | K::KeyedComprehension { clauses, head, .. } => {
                pending.push(head);
                if let K::KeyedComprehension {
                    key,
                    declared_axis_sources,
                    empty_axis_sources,
                    ..
                } = &value.value
                {
                    pending.push(key);
                    pending.extend(declared_axis_sources.iter().flatten());
                    pending.extend(empty_axis_sources.iter().flatten());
                }
                for clause in clauses {
                    match &clause.value {
                        ValueGenerator::In { source, filter, .. }
                        | ValueGenerator::Bind {
                            value: source,
                            filter,
                            ..
                        } => {
                            pending.push(source);
                            pending.extend(filter);
                        }
                    }
                }
            }
            K::One(operand) if matches!(operand.value, K::NativeQuery(_) | K::NativeValue(_)) => {
                pending.push(operand)
            }
            K::Unary { operand, .. }
            | K::Field { value: operand, .. }
            | K::TupleAccess { value: operand, .. }
            | K::IndexSet(operand) => pending.push(operand),
            K::Binary {
                operator,
                left,
                right,
            } => {
                if matches!(operator, B::Contains | B::StartsWith | B::EndsWith) {
                    return Err(refused());
                }
                pending.extend([left.as_ref(), right.as_ref()]);
            }
            K::Range {
                lower: left,
                upper: right,
            }
            | K::ArrayAccess {
                array: left,
                index: right,
            } => pending.extend([left.as_ref(), right.as_ref()]),
            K::Conditional {
                branches,
                otherwise,
            } => {
                pending.extend(
                    branches
                        .iter()
                        .flat_map(|(condition, value)| [condition, value]),
                );
                pending.push(otherwise);
            }
            K::SubjectScope { subject, argument } => {
                pending.extend([subject.as_ref(), argument.body.as_ref()])
            }
            K::PredicateScope { subject, predicate } => {
                pending.extend([subject.as_ref(), predicate.body.as_ref()])
            }
            _ => return Err(refused()),
        }
    }
    Ok(())
}
fn number(text: &str, range: &Range<usize>) -> Result<V, QueryError> {
    match structured::parse_number(text).map_err(|m| QueryError::query(range.clone(), m))? {
        structured::Scalar::Integer(v) => Ok(V::Integer(v)),
        structured::Scalar::Float(v) => Ok(V::Float(v)),
        _ => unreachable!(),
    }
}

pub(crate) fn literal_source(
    input: &Input,
    name: &str,
    facts: Option<&ModelFacts<'_>>,
    budget: &mut Budget,
    range: &Range<usize>,
    limits: crate::Limits,
) -> Result<V, QueryError> {
    let mut found = None;
    for node in input.syntax().tree().child_nodes() {
        charge(budget, 1 + node.children().len(), range)?;
        if node.kind() != N::Assignment {
            continue;
        }
        let i = structured::direct_token(input, node).unwrap();
        charge(budget, input.token_bytes(i).len(), range)?;
        if crate::identifier_identity(std::str::from_utf8(input.token_bytes(i)).unwrap()) == name {
            if found.is_some() {
                return Err(QueryError::input(
                    input.original_range(node.range()),
                    "literal source requires exactly one original assignment",
                ));
            }
            found = crate::assignment_rhs(node);
        }
    }
    let node = found.ok_or_else(|| {
        QueryError::query(
            range.clone(),
            "literal source requires exactly one original assignment",
        )
    })?;
    let source_range = input.original_range(node.range());
    let literal = structured::read_literal(
        input,
        node,
        &mut crate::Work { budget },
        &source_range,
        limits,
    )
    .map_err(input_error)?;
    let metadata = facts
        .map(|facts| declared_value(facts, name, budget, &source_range))
        .transpose()
        .map_err(input_error)?
        .flatten();
    evaluate_literal(&literal, input, facts, metadata, budget, limits.nesting)
}
fn input_error(mut error: QueryError) -> QueryError {
    error.location = crate::ErrorLocation::Input;
    error
}
fn position(i: usize, range: &Range<usize>) -> Result<i64, QueryError> {
    i.checked_add(1)
        .and_then(|n| i64::try_from(n).ok())
        .ok_or_else(|| QueryError::query(range.clone(), "integer key overflow"))
}
/// Bindings outside default fallbacks retain the host's ordinary projection preflight.
pub(crate) fn eager_names<Q>(
    expression: &E<Q>,
    names: &mut BTreeSet<String>,
    budget: &mut Budget,
) -> Result<(), QueryError> {
    charge(budget, 1, &expression.range)?;
    match &expression.value {
        K::Block { .. }
        | K::ValueMatch { .. }
        | K::Queue { .. }
        | K::PopFront(_)
        | K::PushBackAll { .. }
        | K::IdentitySet { .. }
        | K::Insert { .. }
        | K::Fold { .. }
        | K::Scan { .. }
        | K::Loop { .. }
        | K::Break(_)
        | K::Continue(_) => {
            return Err(QueryError::query(
                expression.range.clone(),
                "control values require a prepared report program",
            ));
        }
        K::SubjectScope { subject, argument } => {
            eager_names(subject, names, budget)?;
            eager_names(&argument.body, names, budget)?;
        }
        K::PredicateScope { subject, predicate } => {
            eager_names(subject, names, budget)?;
            eager_names(&predicate.body, names, budget)?;
        }
        K::Filter { array, predicate } => {
            eager_names(array, names, budget)?;
            eager_names(&predicate.body, names, budget)?;
        }
        K::Name(name) => {
            charge(budget, name.len() + 1, &expression.range)?;
            budget
                .charge_results(1, zirql::Location::Query(expression.range.clone()))
                .map_err(pipeline::core_error)?;
            names.insert(name.clone());
        }
        K::CurrentInput
        | K::Literal(_)
        | K::NativeQuery(_)
        | K::NativeValue(_)
        | K::PredicateSubject => {}
        K::Call { arguments, .. } => {
            for argument in arguments {
                eager_names(argument, names, budget)?;
            }
        }
        K::Default { value, .. } => eager_names(value, names, budget)?,
        K::Array(values) | K::Tuple(values) | K::Set(values) => {
            for value in values {
                eager_names(value, names, budget)?;
            }
        }
        K::Record(fields) => {
            for (_, value) in fields {
                eager_names(value, names, budget)?;
            }
        }
        K::Dictionary(entries) | K::KeyedArray { entries, .. } => {
            if let K::KeyedArray {
                declared_axis_sources,
                ..
            } = &expression.value
            {
                for source in declared_axis_sources.iter().flatten() {
                    eager_names(source, names, budget)?;
                }
            }
            for (key, value) in entries {
                eager_names(key, names, budget)?;
                eager_names(value, names, budget)?;
            }
        }
        K::Unique(value)
        | K::Tally(value)
        | K::Occurs(value)
        | K::Absent(value)
        | K::One(value)
        | K::IndexSet(value)
        | K::Entries(value)
        | K::Unary { operand: value, .. }
        | K::Field { value, .. }
        | K::TupleAccess { value, .. }
        | K::Reduce { array: value, .. }
        | K::Sort { array: value, .. }
        | K::Extremum { array: value, .. }
        | K::GroupBy { rows: value, .. } => eager_names(value, names, budget)?,
        K::Lookup {
            value: left,
            key: right,
        }
        | K::HasKey {
            value: left,
            key: right,
        }
        | K::Binary { left, right, .. }
        | K::ZipExact { left, right }
        | K::Range {
            lower: left,
            upper: right,
        }
        | K::ArrayAccess {
            array: left,
            index: right,
        }
        | K::Require {
            condition: left,
            value: right,
            ..
        } => {
            eager_names(left, names, budget)?;
            eager_names(right, names, budget)?;
        }
        K::Conditional {
            branches,
            otherwise,
        } => {
            for (condition, value) in branches {
                eager_names(condition, names, budget)?;
                eager_names(value, names, budget)?;
            }
            eager_names(otherwise, names, budget)?;
        }
        K::Comprehension { clauses, head, .. }
        | K::DictionaryComprehension { clauses, head, .. }
        | K::KeyedComprehension { clauses, head, .. } => {
            if let K::KeyedComprehension {
                declared_axis_sources,
                ..
            } = &expression.value
            {
                for source in declared_axis_sources.iter().flatten() {
                    eager_names(source, names, budget)?;
                }
            }
            eager_names(head, names, budget)?;
            if let K::DictionaryComprehension { key, .. } = &expression.value {
                eager_names(key, names, budget)?;
            }
            if let K::KeyedComprehension {
                key,
                empty_axis_sources,
                ..
            } = &expression.value
            {
                eager_names(key, names, budget)?;
                for value in empty_axis_sources.iter().flatten() {
                    eager_names(value, names, budget)?;
                }
            }
            for clause in clauses {
                match &clause.value {
                    ValueGenerator::In { source, filter, .. }
                    | ValueGenerator::Bind {
                        value: source,
                        filter,
                        ..
                    } => {
                        eager_names(source, names, budget)?;
                        if let Some(filter) = filter {
                            eager_names(filter, names, budget)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn model_error(location: zincite_lint::SourceLocation, message: impl Into<String>) -> QueryError {
    QueryError {
        range: location.range.clone(),
        location: crate::ErrorLocation::Model(location),
        message: message.into(),
    }
}
pub(crate) fn nominal_id(id: DeclarationId, range: &Range<usize>) -> Result<u64, QueryError> {
    u64::try_from(id.0).map_err(|_| QueryError::query(range.clone(), "nominal identity overflow"))
}
pub(crate) fn unique_declaration<'a>(
    facts: &'a ModelFacts<'_>,
    name: &str,
    role: DeclarationRole,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<&'a Declaration, QueryError> {
    let mut found = None;
    for declaration in &facts.bindings().declarations {
        charge(budget, declaration.name.len() + name.len() + 1, range)?;
        if declaration.top_level && declaration.name == name {
            if declaration.role != role || found.is_some() {
                return Err(QueryError::query(
                    range.clone(),
                    "unavailable or ambiguous declaration identity",
                ));
            }
            found = Some(declaration);
        }
    }
    found.ok_or_else(|| QueryError::query(range.clone(), "unavailable declaration identity"))
}
pub(crate) fn declaration_type<'a>(
    facts: &'a ModelFacts<'_>,
    id: DeclarationId,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<&'a TypeInst, QueryError> {
    let mut found = None;
    for ty in &facts.source_callables().declarations {
        charge(budget, 1, range)?;
        if ty.declaration == id {
            if found.is_some() {
                return Err(QueryError::query(
                    range.clone(),
                    "ambiguous declaration type",
                ));
            }
            found = Some(&ty.ty);
        }
    }
    found.ok_or_else(|| QueryError::query(range.clone(), "unavailable declaration type"))
}
/// Explicit cases and their retained declaration coordinates prove membership/order.
pub(crate) fn enum_cases<'a>(
    facts: &'a ModelFacts<'_>,
    owner: &'a Declaration,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Vec<&'a Declaration>, QueryError> {
    let owner = unique_declaration(facts, &owner.name, DeclarationRole::Enum, budget, range)?;
    let file = &facts.model().files[owner.file];
    let mut root = None;
    for (i, node) in file.parsed.tree().child_nodes().enumerate() {
        charge(budget, 1, range)?;
        if i == owner.item
            && node.kind() == N::EnumDeclaration
            && node.range() == owner.syntax_range
        {
            root = Some(node);
        }
    }
    let root = root.ok_or_else(|| {
        model_error(
            owner.location.clone(),
            "enum owner has no retained declaration",
        )
    })?;
    let definition = root
        .child_nodes()
        .find(|n| n.kind() == N::EnumDefinition)
        .ok_or_else(|| {
            model_error(
                owner.location.clone(),
                "enum identity/order requires explicit cases",
            )
        })?;
    let count = definition
        .child_nodes()
        .map(|n| n.child_nodes().count())
        .try_fold(0usize, |n, count| n.checked_add(count))
        .ok_or_else(|| QueryError::query(range.clone(), "enum case count overflow"))?;
    let mut members = slots(budget, count, range)?;
    for group in definition.child_nodes() {
        charge(budget, 1 + group.children().len(), range)?;
        if group.kind() != N::EnumCases {
            return Err(model_error(
                file.location(group.range()),
                "computed or constructed enum identity/order is unavailable",
            ));
        }
        for case in group.child_nodes() {
            charge(budget, 1 + case.children().len(), range)?;
            let mut member = None;
            for declaration in &facts.bindings().declarations {
                charge(budget, 1, range)?;
                if declaration.file == owner.file
                    && declaration.item == owner.item
                    && declaration.syntax_range == case.range()
                    && declaration.role == DeclarationRole::EnumMember
                {
                    if member.is_some() {
                        return Err(model_error(
                            file.location(case.range()),
                            "ambiguous enum case identity",
                        ));
                    }
                    member = Some(declaration);
                }
            }
            let member = member.ok_or_else(|| {
                model_error(
                    file.location(case.range()),
                    "enum case has no public declaration identity",
                )
            })?;
            if !matches!(declaration_type(facts, member.id, budget, range)?.kind, TypeKind::Enum(id) if id == owner.id)
            {
                return Err(model_error(
                    member.location.clone(),
                    "enum member has an uncertain owner",
                ));
            }
            unique_declaration(
                facts,
                &member.name,
                DeclarationRole::EnumMember,
                budget,
                range,
            )?;
            members.push(member);
        }
    }
    Ok(members)
}
pub(crate) fn resolve_member(
    facts: Option<&ModelFacts<'_>>,
    enum_name: Option<&str>,
    member_name: &str,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<zirql::NominalMember, QueryError> {
    let facts = facts.ok_or_else(|| {
        QueryError::query(
            range.clone(),
            "enum member identity requires model context (--model)",
        )
    })?;
    let member = unique_declaration(
        facts,
        member_name,
        DeclarationRole::EnumMember,
        budget,
        range,
    )?;
    let TypeKind::Enum(owner_id) = declaration_type(facts, member.id, budget, range)?.kind else {
        return Err(QueryError::query(
            range.clone(),
            "enum member has an uncertain owner",
        ));
    };
    let owner = &facts.bindings().declarations[owner_id.0];
    if enum_name.is_some_and(|name| name != owner.name) {
        return Err(QueryError::query(
            range.clone(),
            "enum member belongs to another enum",
        ));
    }
    let cases = enum_cases(facts, owner, budget, range)?;
    let order = cases
        .iter()
        .position(|case| case.id == member.id)
        .ok_or_else(|| QueryError::query(range.clone(), "enum member order is unavailable"))?;
    charge(
        budget,
        cases.len() + owner.name.len() + member.name.len() + 1,
        range,
    )?;
    budget
        .charge_results(1, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    Ok(zirql::NominalMember {
        type_id: zirql::NominalTypeId(nominal_id(owner.id, range)?),
        member_id: zirql::NominalMemberId(nominal_id(member.id, range)?),
        declaration_order: order,
        type_name: owner.name.to_owned(),
        member_name: member.name.to_owned(),
    })
}
/// The written type accompanies public expanded types; enum types do not prove index subsets.
#[derive(Clone, Copy)]
pub(crate) struct LiteralMetadata<'a> {
    pub ty: &'a TypeInst,
    pub file: usize,
    pub node: Option<&'a SyntaxNode>,
}
pub(crate) fn declaration_node<'a>(
    facts: &'a ModelFacts<'_>,
    id: DeclarationId,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Option<&'a SyntaxNode>, QueryError> {
    let declaration = &facts.bindings().declarations[id.0];
    for (i, node) in facts.model().files[declaration.file]
        .parsed
        .tree()
        .child_nodes()
        .enumerate()
    {
        charge(budget, 1, range)?;
        if i == declaration.item && node.range() == declaration.syntax_range {
            return Ok(Some(node));
        }
    }
    Ok(None)
}
pub(crate) fn resolve_metadata<'a, 'model: 'a>(
    mut metadata: LiteralMetadata<'a>,
    facts: &ModelFacts<'model>,
    budget: &mut Budget,
    range: &Range<usize>,
    nesting: usize,
) -> Result<LiteralMetadata<'a>, QueryError> {
    for _ in 0..nesting.min(64) {
        let Some(node) = metadata.node else {
            return Ok(metadata);
        };
        charge(budget, 1 + node.children().len(), range)?;
        if node.kind() != N::DomainType {
            return Ok(metadata);
        }
        let file = &facts.model().files[metadata.file];
        let location = file.location(node.range());
        let mut alias = None;
        for reference in &facts.bindings().references {
            charge(budget, 1, range)?;
            if reference.file == metadata.file
                && location.range.start <= reference.location.range.start
                && reference.location.range.end <= location.range.end
                && let zincite_lint::BindingResolution::Resolved(id) = reference.resolution
            {
                let declaration = &facts.bindings().declarations[id.0];
                if declaration.role == DeclarationRole::TypeAlias {
                    alias = Some(declaration);
                }
            }
        }
        let Some(alias) = alias else {
            return Ok(metadata);
        };
        unique_declaration(
            facts,
            &alias.name,
            DeclarationRole::TypeAlias,
            budget,
            range,
        )?;
        // Model CST storage is owned by the retained context, not the fact vector.
        let source = &facts.model().files[alias.file];
        let mut node = None;
        for (i, item) in source.parsed.tree().child_nodes().enumerate() {
            charge(budget, 1, range)?;
            if i == alias.item && item.range() == alias.syntax_range {
                node = item.child_nodes().find(|n| n.kind() != N::Annotation);
            }
        }
        metadata = LiteralMetadata {
            ty: metadata.ty,
            file: alias.file,
            node,
        };
    }
    Err(QueryError::query(
        range.clone(),
        "model type nesting limit exceeded",
    ))
}
pub(crate) fn child_metadata<'a>(
    metadata: Option<LiteralMetadata<'a>>,
    ty: &'a TypeInst,
    position: Option<usize>,
) -> LiteralMetadata<'a> {
    let node = metadata
        .and_then(|metadata| metadata.node)
        .and_then(|node| {
            if let Some(i) = position {
                node.child_nodes().nth(i)
            } else {
                node.child_nodes().last()
            }
        });
    LiteralMetadata {
        ty,
        file: metadata.map_or(0, |metadata| metadata.file),
        node,
    }
}
pub(crate) fn declared_value<'a>(
    facts: &'a ModelFacts<'_>,
    name: &str,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Option<LiteralMetadata<'a>>, QueryError> {
    let mut owner = None;
    for declaration in &facts.bindings().declarations {
        charge(budget, 1 + name.len() + declaration.name.len(), range)?;
        if declaration.top_level && declaration.name == name {
            if declaration.role != DeclarationRole::Value || owner.is_some() {
                return Err(QueryError::query(
                    range.clone(),
                    "source declaration identity is ambiguous",
                ));
            }
            owner = Some(declaration);
        }
    }
    let Some(owner) = owner else {
        return Ok(None);
    };
    let ty = declaration_type(facts, owner.id, budget, range)?;
    if ty.instantiation != zincite_lint::Instantiation::Parameter {
        return Err(QueryError::query(
            range.clone(),
            "source requires a fixed declaration",
        ));
    }
    let node = declaration_node(facts, owner.id, budget, range)?
        .and_then(|node| node.child_nodes().find(|node| node.kind() != N::Annotation));
    Ok(Some(LiteralMetadata {
        ty,
        file: owner.file,
        node,
    }))
}
/// Resolve only literal fixed scalar dependencies; arithmetic itself runs in Zirql.
pub(crate) fn integer_bound(
    bound: &NumericBound,
    input: &Input,
    scalars: &BTreeMap<usize, i64>,
    facts: &ModelFacts<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<i64, QueryError> {
    fn expression(
        bound: &NumericBound,
        input: &Input,
        scalars: &BTreeMap<usize, i64>,
        facts: &ModelFacts<'_>,
        budget: &mut Budget,
        range: &Range<usize>,
        remaining: usize,
    ) -> Result<E, QueryError> {
        charge(budget, 1, range)?;
        if remaining == 0 {
            return Err(QueryError::query(
                range.clone(),
                "axis bound nesting limit exceeded",
            ));
        }
        let value = match bound {
            NumericBound::Integer(n) => K::Literal(V::Integer(*n)),
            NumericBound::Symbol(id)
            | NumericBound::Defined {
                declaration: id, ..
            } => {
                let declaration = &facts.bindings().declarations[id.0];
                let owner = unique_declaration(
                    facts,
                    &declaration.name,
                    DeclarationRole::Value,
                    budget,
                    range,
                )?;
                let ty = declaration_type(facts, owner.id, budget, range)?;
                if ty.optional
                    || ty.instantiation != zincite_lint::Instantiation::Parameter
                    || ty.kind != TypeKind::Int
                {
                    return Err(model_error(
                        owner.location.clone(),
                        "axis dependency requires one fixed integer scalar declaration",
                    ));
                }
                let n = if let Some(n) = scalars.get(&id.0) {
                    *n
                } else {
                    let mut rhs = None;
                    for node in input.syntax().tree().child_nodes() {
                        charge(budget, 1 + node.children().len(), range)?;
                        if node.kind() == N::Assignment {
                            let token = structured::direct_token(input, node).unwrap();
                            if crate::identifier_identity(
                                std::str::from_utf8(input.token_bytes(token)).unwrap(),
                            ) == owner.name
                            {
                                if rhs.is_some() {
                                    return Err(QueryError::input(
                                        input.original_range(node.range()),
                                        "axis scalar assignment is ambiguous",
                                    ));
                                }
                                rhs = crate::assignment_rhs(node);
                            }
                        }
                    }
                    if let Some(rhs) = rhs {
                        let literal = structured::read_literal(
                            input,
                            rhs,
                            &mut crate::Work { budget },
                            range,
                            crate::Limits::default(),
                        )
                        .map_err(input_error)?;
                        if !matches!(literal.kind, structured::LiteralKind::Integer) {
                            return Err(QueryError::input(
                                literal.range,
                                "axis dependency requires an original integer literal; computed assignments are unsupported",
                            ));
                        }
                        let V::Integer(n) = written_scalar(&literal, input, budget)? else {
                            unreachable!()
                        };
                        n
                    } else if let NumericBound::Defined { value, .. } = bound {
                        let NumericBound::Integer(n) = value.as_ref() else {
                            return Err(model_error(
                                owner.location.clone(),
                                "computed axis scalar dependency is unsupported",
                            ));
                        };
                        *n
                    } else {
                        return Err(model_error(
                            owner.location.clone(),
                            "axis scalar dependency has no original literal assignment",
                        ));
                    }
                };
                K::Literal(V::Integer(n))
            }
            NumericBound::Arithmetic { operator, operands } => {
                let mut values = slots(budget, operands.len(), range)?;
                for operand in operands {
                    values.push(expression(
                        operand,
                        input,
                        scalars,
                        facts,
                        budget,
                        range,
                        remaining - 1,
                    )?);
                }
                match values.len() {
                    1 if matches!(operator, T::Plus | T::Minus) => K::Unary {
                        operator: if *operator == T::Plus {
                            U::Positive
                        } else {
                            U::Negative
                        },
                        operand: Box::new(values.pop().unwrap()),
                    },
                    2 => {
                        let operator = match operator {
                            T::Plus => B::Add,
                            T::Minus => B::Subtract,
                            T::Star => B::Multiply,
                            T::Div => B::IntegerDivide,
                            T::Mod => B::Modulo,
                            _ => {
                                return Err(QueryError::query(
                                    range.clone(),
                                    "unsupported axis arithmetic",
                                ));
                            }
                        };
                        let right = values.pop().unwrap();
                        let left = values.pop().unwrap();
                        K::Binary {
                            operator,
                            left: Box::new(left),
                            right: Box::new(right),
                        }
                    }
                    _ => {
                        return Err(QueryError::query(
                            range.clone(),
                            "unsupported axis arithmetic",
                        ));
                    }
                }
            }
            _ => {
                return Err(QueryError::query(
                    range.clone(),
                    "unknown or unsupported integer axis bound",
                ));
            }
        };
        Ok(Located::new(value, range.clone()))
    }
    let expression = expression(bound, input, scalars, facts, budget, range, 64)?;
    match zirql::evaluate_value(&expression, &zirql::Bindings::default(), budget)
        .map_err(pipeline::core_error)?
    {
        V::Integer(n) => Ok(n),
        _ => Err(QueryError::query(
            range.clone(),
            "axis bound is not an integer",
        )),
    }
}
fn domain_axis(
    domain: &Domain,
    input: &Input,
    scalars: &BTreeMap<usize, i64>,
    facts: &ModelFacts<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<zirql::KeyAxis, QueryError> {
    use zirql::{KeyAxis, ValueKey as Key, ValueKeyType as KeyType};
    match domain {
        Domain::Range { lower, upper } => {
            let lo = integer_bound(lower, input, scalars, facts, budget, range)?;
            let hi = integer_bound(upper, input, scalars, facts, budget, range)?;
            let len = if lo > hi {
                0
            } else {
                usize::try_from(i128::from(hi) - i128::from(lo) + 1)
                    .map_err(|_| QueryError::query(range.clone(), "axis size overflow"))?
            };
            let mut keys = slots(budget, len, range)?;
            for i in 0..len {
                keys.push(Key::Integer(
                    lo.checked_add(
                        i64::try_from(i)
                            .map_err(|_| QueryError::query(range.clone(), "axis key overflow"))?,
                    )
                    .ok_or_else(|| QueryError::query(range.clone(), "axis key overflow"))?,
                ));
            }
            Ok(KeyAxis {
                key_type: KeyType::Integer,
                keys,
            })
        }
        Domain::Named {
            declaration,
            domain,
        } if matches!(domain.as_ref(), Domain::Enum(id) if id == declaration) => {
            domain_axis(domain, input, scalars, facts, budget, range)
        }
        Domain::Enum(id) => {
            let owner = facts.bindings().declarations.get(id.0).ok_or_else(|| {
                QueryError::query(range.clone(), "axis enum identity unavailable")
            })?;
            let cases = enum_cases(facts, owner, budget, range)?;
            let mut keys = slots(budget, cases.len(), range)?;
            for member in cases {
                keys.push(Key::Member(resolve_member(
                    Some(facts),
                    Some(&owner.name),
                    &member.name,
                    budget,
                    range,
                )?));
            }
            Ok(KeyAxis {
                key_type: KeyType::Member(zirql::NominalTypeId(nominal_id(*id, range)?)),
                keys,
            })
        }
        _ => Err(QueryError::query(
            range.clone(),
            "axis domain must be a written integer interval or explicit enum",
        )),
    }
}
pub(crate) fn declared_axes(
    input: &Input,
    scalars: &BTreeMap<usize, i64>,
    metadata: Option<LiteralMetadata<'_>>,
    facts: Option<&ModelFacts<'_>>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Option<Vec<zirql::KeyAxis>>, QueryError> {
    let Some(metadata) = metadata else {
        return Ok(None);
    };
    let TypeKind::Array { indices: types, .. } = &metadata.ty.kind else {
        return Ok(None);
    };
    let facts = facts
        .ok_or_else(|| QueryError::query(range.clone(), "declared axes require model context"))?;
    let mut domains = slots(budget, types.len(), range)?;
    for i in 0..types.len() {
        let node = metadata
            .node
            .and_then(|node| node.child_nodes().nth(i))
            .ok_or_else(|| {
                QueryError::query(range.clone(), "written array index domain is unavailable")
            })?;
        let location = facts.model().files[metadata.file].location(node.range());
        let mut domain = None;
        for index in &facts.source_domains().array_indices {
            charge(budget, 1, range)?;
            if index.file == metadata.file && index.location.range == location.range {
                domain = Some(&index.domain);
            }
        }
        domains.push(domain.ok_or_else(|| {
            QueryError::query(range.clone(), "array index domain is unavailable")
        })?);
    }
    // An unconstrained integer declaration adds no missing rank-one identity.
    if types.len() == 1
        && types[0].kind == TypeKind::Int
        && matches!(domains.as_slice(), [Domain::UnconstrainedInt])
    {
        return Ok(None);
    }
    let mut axes = slots(budget, types.len(), range)?;
    for domain in domains {
        axes.push(domain_axis(domain, input, scalars, facts, budget, range)?);
    }
    Ok(Some(axes))
}
fn literal_key(
    literal: &structured::LiteralValue<'_>,
    input: &Input,
    facts: Option<&ModelFacts<'_>>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<E, QueryError> {
    let expression = project_literal(literal, input, facts, None, budget, nesting, 0)?;
    match &expression.value {
        K::Literal(V::Integer(_) | V::Member(_)) | K::Tuple(_) => Ok(expression),
        _ => Err(QueryError::input(
            literal.range.clone(),
            "array key requires an integer or resolved enum member",
        )),
    }
}
fn array_element<'a>(metadata: Option<LiteralMetadata<'a>>) -> Option<LiteralMetadata<'a>> {
    let metadata = metadata?;
    let TypeKind::Array { element, .. } = &metadata.ty.kind else {
        return None;
    };
    Some(child_metadata(Some(metadata), element, None))
}
fn written_scalar(
    literal: &structured::LiteralValue<'_>,
    input: &Input,
    budget: &mut Budget,
) -> Result<V, QueryError> {
    let range = &literal.range;
    let syntax_range = range.start - input.syntax_offset()..range.end - input.syntax_offset();
    let tokens = input.syntax().tokens();
    let first = tokens.partition_point(|t| t.range.start < syntax_range.start);
    let mut negative = false;
    let mut atom = None;
    for (offset, token) in tokens[first..]
        .iter()
        .enumerate()
        .take_while(|(_, t)| t.range.end <= syntax_range.end)
    {
        charge(budget, 1, range).map_err(input_error)?;
        match token.kind {
            T::Whitespace
            | T::LineComment
            | T::BlockComment
            | T::LeftParen
            | T::RightParen
            | T::Plus => {}
            T::Minus => negative = !negative,
            T::IntegerLiteral | T::FloatLiteral | T::StringLiteral if atom.is_none() => {
                atom = Some(first + offset)
            }
            _ => {
                return Err(QueryError::input(
                    range.clone(),
                    "unsupported written scalar",
                ));
            }
        }
    }
    let atom =
        atom.ok_or_else(|| QueryError::input(range.clone(), "written scalar has no token"))?;
    let text = std::str::from_utf8(input.token_bytes(atom)).unwrap();
    charge(budget, text.len() + usize::from(negative), range).map_err(input_error)?;
    if tokens[atom].kind == T::StringLiteral {
        return structured::decode_string(text)
            .map(V::String)
            .map_err(|m| QueryError::input(range.clone(), m));
    }
    if negative {
        number(&format!("-{text}"), range).map_err(input_error)
    } else {
        number(text, range).map_err(input_error)
    }
}
fn project_literal(
    literal: &structured::LiteralValue<'_>,
    input: &Input,
    facts: Option<&ModelFacts<'_>>,
    metadata: Option<LiteralMetadata<'_>>,
    budget: &mut Budget,
    nesting: usize,
    depth: usize,
) -> Result<E, QueryError> {
    use structured::{ArrayIndexing as Indexing, LiteralKind as L};
    let range = &literal.range;
    if depth >= nesting.min(64) {
        return Err(QueryError::input(
            range.clone(),
            "literal nesting limit exceeded",
        ));
    }
    charge(budget, 1 + literal.spelling.len(), range).map_err(input_error)?;
    let metadata = metadata
        .map(|metadata| {
            resolve_metadata(
                metadata,
                facts.expect("metadata has retained facts"),
                budget,
                range,
                nesting.saturating_sub(depth),
            )
        })
        .transpose()
        .map_err(input_error)?;
    let kind = match &literal.kind {
        L::Absent => K::Literal(V::Absent),
        L::Integer | L::Float | L::String => K::Literal(written_scalar(literal, input, budget)?),
        L::Boolean(v) => K::Literal(V::Boolean(*v)),
        L::Member(name) => {
            let member = resolve_member(facts, None, name, budget, range).map_err(|e| {
                if matches!(e.location, crate::ErrorLocation::Query) {
                    input_error(e)
                } else {
                    e
                }
            })?;
            if let Some(facts) = facts {
                // Reuse a written model reference only when these are the same original bytes.
                for (file_id, file) in facts.model().files.iter().enumerate() {
                    charge(budget, 1, range).map_err(input_error)?;
                    if input.mode == zincite_syntax::FileMode::Model {
                        charge(budget, file.source_bytes().len(), range).map_err(input_error)?;
                    }
                    if input.mode == zincite_syntax::FileMode::Model
                        && input.syntax_offset() == file.byte_offset
                        && input.source_bytes()[file.byte_offset..] == *file.source_bytes()
                    {
                        for reference in &facts.bindings().references {
                            charge(budget, 1, range).map_err(input_error)?;
                            if reference.file == file_id
                                && reference.location.range == *range
                                && !matches!(reference.resolution, zincite_lint::BindingResolution::Resolved(id) if nominal_id(id, range)? == member.member_id.0)
                            {
                                return Err(QueryError::input(
                                    range.clone(),
                                    "written member reference has uncertain identity",
                                ));
                            }
                        }
                    }
                }
            }
            K::Literal(V::Member(member))
        }
        L::Set(values) | L::Tuple(values) => {
            let mut children = slots(budget, values.len(), range).map_err(input_error)?;
            for (i, child) in values.iter().enumerate() {
                let child_meta = match metadata.map(|metadata| &metadata.ty.kind) {
                    Some(TypeKind::Set(ty)) => Some(child_metadata(metadata, ty, None)),
                    Some(TypeKind::Tuple(types)) => {
                        types.get(i).map(|ty| child_metadata(metadata, ty, Some(i)))
                    }
                    _ => None,
                };
                children.push(project_literal(
                    child,
                    input,
                    facts,
                    child_meta,
                    budget,
                    nesting,
                    depth + 1,
                )?);
            }
            if matches!(literal.kind, L::Tuple(_)) {
                K::Tuple(children)
            } else if values.is_empty() {
                let element_type = match metadata.map(|metadata| &metadata.ty.kind) {
                    Some(TypeKind::Set(ty)) => match ty.kind {
                        TypeKind::Int => Some(zirql::SetElementType::Integer),
                        TypeKind::Bool => Some(zirql::SetElementType::Boolean),
                        TypeKind::Float => Some(zirql::SetElementType::Float),
                        TypeKind::Enum(id) => {
                            let facts = facts.ok_or_else(|| {
                                QueryError::input(
                                    range.clone(),
                                    "empty nominal set requires model context",
                                )
                            })?;
                            let owner = &facts.bindings().declarations[id.0];
                            enum_cases(facts, owner, budget, range)?;
                            Some(zirql::SetElementType::Member(zirql::NominalTypeId(
                                nominal_id(id, range)?,
                            )))
                        }
                        TypeKind::String => {
                            return Err(QueryError::input(
                                range.clone(),
                                "string sets are unsupported",
                            ));
                        }
                        _ => None,
                    },
                    _ => None,
                };
                K::Literal(V::Set {
                    element_type,
                    elements: Vec::new(),
                })
            } else {
                K::Set(children)
            }
        }
        L::Record(fields) => {
            let mut children = slots(budget, fields.len(), range).map_err(input_error)?;
            for field in fields {
                charge(budget, field.label.name.len() + 1, range).map_err(input_error)?;
                let child_meta = if let Some(metadata) = metadata {
                    if let TypeKind::Record(types) = &metadata.ty.kind {
                        let mut written = None;
                        for node in metadata.node.into_iter().flat_map(SyntaxNode::child_nodes) {
                            charge(budget, 1 + node.children().len(), range)
                                .map_err(input_error)?;
                            let parsed = &facts.unwrap().model().files[metadata.file].parsed;
                            for token in
                                node.children().iter().filter_map(|element| match element {
                                    SyntaxElement::Token(i) => Some(&parsed.tokens()[*i]),
                                    _ => None,
                                })
                            {
                                if matches!(token.kind, T::Identifier | T::QuotedIdentifier) {
                                    let name = &parsed.source()[token.range.clone()];
                                    charge(budget, name.len(), range).map_err(input_error)?;
                                    if crate::identifier_identity(name) == field.label.name {
                                        written = node.child_nodes().next();
                                    }
                                }
                            }
                        }
                        types
                            .iter()
                            .find(|(name, _)| name == field.label.name)
                            .map(|(_, ty)| LiteralMetadata {
                                ty,
                                file: metadata.file,
                                node: written,
                            })
                    } else {
                        None
                    }
                } else {
                    None
                };
                children.push((
                    Located::new(field.label.name.to_owned(), field.label.range.clone()),
                    project_literal(
                        &field.value,
                        input,
                        facts,
                        child_meta,
                        budget,
                        nesting,
                        depth + 1,
                    )?,
                ));
            }
            K::Record(children)
        }
        L::Array { indexing, entries } => {
            let axes = declared_axes(input, &BTreeMap::new(), metadata, facts, budget, range)
                .map_err(|e| {
                    if matches!(e.location, crate::ErrorLocation::Query) {
                        input_error(e)
                    } else {
                        e
                    }
                })?;
            if *indexing == Indexing::Positional && axes.is_none() {
                let mut children = slots(budget, entries.len(), range).map_err(input_error)?;
                for entry in entries {
                    children.push(project_literal(
                        &entry.value,
                        input,
                        facts,
                        array_element(metadata),
                        budget,
                        nesting,
                        depth + 1,
                    )?);
                }
                K::Array(children)
            } else {
                let mut pairs = slots(budget, entries.len(), range).map_err(input_error)?;
                let mut next = None;
                for (i, entry) in entries.iter().enumerate() {
                    let key = if let Some(key) = &entry.key {
                        let key = literal_key(key, input, facts, budget, nesting)?;
                        next = match &key.value {
                            K::Literal(V::Integer(n)) => Some(*n),
                            _ => None,
                        };
                        key
                    } else if *indexing == Indexing::StartingKey {
                        let n = next.and_then(|n| n.checked_add(1)).ok_or_else(|| QueryError::input(entry.value.range.clone(), "starting-key tail requires a written integer and checked successors"))?;
                        next = Some(n);
                        Located::new(K::Literal(V::Integer(n)), entry.value.range.clone())
                    } else {
                        let axes = axes.as_ref().unwrap();
                        if axes.len() != 1 {
                            return Err(QueryError::input(
                                range.clone(),
                                "positional source cannot establish multidimensional keys",
                            ));
                        }
                        let key = axes[0].keys.get(i).ok_or_else(|| {
                            QueryError::input(
                                range.clone(),
                                "source exceeds declared axis coverage",
                            )
                        })?;
                        key_expression(key, budget, range)?
                    };
                    pairs.push((
                        key,
                        project_literal(
                            &entry.value,
                            input,
                            facts,
                            array_element(metadata),
                            budget,
                            nesting,
                            depth + 1,
                        )?,
                    ));
                }
                K::KeyedArray {
                    axes,
                    declared_axis_sources: None,
                    entries: pairs,
                }
            }
        }
        L::Matrix { column_keys, rows } => {
            let mut axes = declared_axes(input, &BTreeMap::new(), metadata, facts, budget, range)
                .map_err(input_error)?;
            let columns = if column_keys.is_empty() {
                rows.first().map_or(0, |row| row.cells.len())
            } else {
                column_keys.len()
            };
            let count = rows
                .len()
                .checked_mul(columns)
                .ok_or_else(|| QueryError::input(range.clone(), "matrix size overflow"))?;
            let mut pairs = slots(budget, count, range).map_err(input_error)?;
            if axes.as_ref().is_some_and(|axes| axes.len() != 2) {
                return Err(QueryError::input(range.clone(), "matrix requires rank two"));
            }
            for (i, row) in rows.iter().enumerate() {
                if row.cells.len() != columns {
                    return Err(QueryError::input(range.clone(), "matrix has missing cells"));
                }
                for (j, cell) in row.cells.iter().enumerate() {
                    let row_key = if let Some(key) = &row.key {
                        literal_key(key, input, facts, budget, nesting)?
                    } else if let Some(axes) = &axes {
                        key_expression(
                            axes[0].keys.get(i).ok_or_else(|| {
                                QueryError::input(range.clone(), "matrix exceeds declared row axis")
                            })?,
                            budget,
                            range,
                        )?
                    } else {
                        Located::new(K::Literal(V::Integer(position(i, range)?)), range.clone())
                    };
                    let col_key = if let Some(key) = column_keys.get(j) {
                        literal_key(key, input, facts, budget, nesting)?
                    } else if let Some(axes) = &axes {
                        key_expression(
                            axes[1].keys.get(j).ok_or_else(|| {
                                QueryError::input(
                                    range.clone(),
                                    "matrix exceeds declared column axis",
                                )
                            })?,
                            budget,
                            range,
                        )?
                    } else {
                        Located::new(K::Literal(V::Integer(position(j, range)?)), range.clone())
                    };
                    let mut parts = slots(budget, 2, range).map_err(input_error)?;
                    parts.extend([row_key, col_key]);
                    pairs.push((
                        Located::new(K::Tuple(parts), range.clone()),
                        project_literal(
                            cell,
                            input,
                            facts,
                            array_element(metadata),
                            budget,
                            nesting,
                            depth + 1,
                        )?,
                    ));
                }
            }
            if count == 0 && axes.is_none() {
                return Err(QueryError::input(
                    range.clone(),
                    "empty matrix requires established axes",
                ));
            }
            K::KeyedArray {
                declared_axis_sources: None,
                axes: axes.take(),
                entries: pairs,
            }
        }
    };
    Ok(Located::new(kind, range.clone()))
}
pub(crate) fn key_expression(
    key: &zirql::ValueKey,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<E, QueryError> {
    let value = match key {
        zirql::ValueKey::Integer(n) => V::Integer(*n),
        zirql::ValueKey::Member(member) => {
            charge(
                budget,
                member.type_name.len() + member.member_name.len() + 1,
                range,
            )?;
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            V::Member(zirql::NominalMember {
                type_id: member.type_id,
                member_id: member.member_id,
                declaration_order: member.declaration_order,
                type_name: member.type_name.to_owned(),
                member_name: member.member_name.to_owned(),
            })
        }
    };
    Ok(Located::new(K::Literal(value), range.clone()))
}
// Copy only the already available declaration, never the source's stored data.
fn copy_declaration(
    ty: &zirql::DeclaredType<()>,
    budget: &mut Budget,
    range: &Range<usize>,
    depth: usize,
) -> Result<zirql::DeclaredType<()>, zirql::QueryError> {
    use zirql::DeclaredType as D;
    let location = zirql::Location::Query(range.clone());
    budget.charge(1, location.clone())?;
    if depth >= 64 {
        return Err(zirql::QueryError::query(
            range.clone(),
            "declaration nesting limit exceeded",
        ));
    }
    Ok(match ty {
        D::Optional(child) | D::Array(child) | D::Sequence(child) | D::Keyed(child) => {
            budget.charge_results(1, location)?;
            let child = Box::new(copy_declaration(child, budget, range, depth + 1)?);
            match ty {
                D::Optional(_) => D::Optional(child),
                D::Array(_) => D::Array(child),
                D::Sequence(_) => D::Sequence(child),
                _ => D::Keyed(child),
            }
        }
        D::Tuple(types) => {
            budget.charge_results(types.len(), location)?;
            D::Tuple(
                types
                    .iter()
                    .map(|ty| copy_declaration(ty, budget, range, depth + 1))
                    .collect::<Result<_, _>>()?,
            )
        }
        D::Record(fields) => {
            budget.charge_results(fields.len(), location.clone())?;
            let mut output = Vec::with_capacity(fields.len());
            for (name, ty) in fields {
                budget.charge(name.len(), location.clone())?;
                output.push((
                    name.clone(),
                    copy_declaration(ty, budget, range, depth + 1)?,
                ));
            }
            D::Record(output)
        }
        D::Boolean => D::Boolean,
        D::Integer => D::Integer,
        D::Float => D::Float,
        D::String => D::String,
        D::Member(id) => D::Member(*id),
        D::Set(ty) => D::Set(*ty),
        D::Queue(_) | D::IdentitySet(()) => {
            return Err(zirql::QueryError::query(
                range.clone(),
                "computational values are unavailable in Zincite",
            ));
        }
        D::Native(()) => {
            return Err(zirql::QueryError::query(
                range.clone(),
                "native declaration requires its host",
            ));
        }
    })
}

/// Available outer kinds and declared contracts, without inspecting stored data.
pub(crate) fn value_facts(
    value: &V,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<zirql::ValueType, zirql::QueryError> {
    use zirql::ValueType as T;
    budget.charge(1, zirql::Location::Query(range.clone()))?;
    let actual = match value.data() {
        V::Declared { .. } => unreachable!("transparent annotation"),
        V::Absent => T::Absent,
        V::Boolean(_) => T::Boolean,
        V::Integer(_) => T::Integer,
        V::Float(_) => T::Float,
        V::String(_) => T::String,
        V::Member(member) => T::Member(member.type_id),
        V::Record(_) => T::RecordKind,
        V::Tuple(_) => T::TupleKind,
        V::Set { element_type, .. } => T::Set(*element_type),
        V::Array(_) => T::Array(zirql::ArrayType::Elements(Vec::new())),
        V::KeyedArray { .. } => T::KeyedArray(zirql::KeyedArrayType {
            axes: None,
            elements: Vec::new(),
        }),
    };
    if let V::Declared { declaration, .. } = value {
        budget.charge_results(1, zirql::Location::Query(range.clone()))?;
        Ok(T::Declared {
            actual: Box::new(actual),
            declaration: copy_declaration(declaration, budget, range, 0)?,
        })
    } else {
        Ok(actual)
    }
}

// TypeInst owns model meaning. This bounded conversion retains only the supported
// constructed contract; actual data still goes through declare_constructed.
fn source_declaration(
    ty: &TypeInst,
    budget: &mut Budget,
    range: &Range<usize>,
    depth: usize,
) -> Result<zirql::DeclaredType<()>, QueryError> {
    use zirql::DeclaredType as D;
    if depth >= 64 || ty.instantiation != zincite_lint::Instantiation::Parameter {
        return Err(QueryError::input(
            range.clone(),
            "source requires a bounded fixed supported declaration",
        ));
    }
    charge(budget, 1, range).map_err(input_error)?;
    let declaration = match &ty.kind {
        TypeKind::Bool => D::Boolean,
        TypeKind::Int => D::Integer,
        TypeKind::Float => D::Float,
        TypeKind::String => D::String,
        TypeKind::Enum(id) => D::Member(zirql::NominalTypeId(nominal_id(*id, range)?)),
        TypeKind::Set(element)
            if !element.optional
                && element.instantiation == zincite_lint::Instantiation::Parameter =>
        {
            D::Set(match element.kind {
                TypeKind::Bool => zirql::SetElementType::Boolean,
                TypeKind::Int => zirql::SetElementType::Integer,
                TypeKind::Float => zirql::SetElementType::Float,
                TypeKind::Enum(id) => {
                    zirql::SetElementType::Member(zirql::NominalTypeId(nominal_id(id, range)?))
                }
                _ => {
                    return Err(QueryError::input(
                        range.clone(),
                        "unsupported declared set element",
                    ));
                }
            })
        }
        TypeKind::Array { element, .. } => {
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)
                .map_err(input_error)?;
            D::Array(Box::new(source_declaration(
                element,
                budget,
                range,
                depth + 1,
            )?))
        }
        TypeKind::Tuple(types) => {
            let mut output = slots(budget, types.len(), range).map_err(input_error)?;
            for ty in types {
                output.push(source_declaration(ty, budget, range, depth + 1)?);
            }
            D::Tuple(output)
        }
        TypeKind::Record(fields) => {
            let mut output = slots(budget, fields.len(), range).map_err(input_error)?;
            for (label, ty) in fields {
                charge(budget, label.len(), range).map_err(input_error)?;
                output.push((
                    label.clone(),
                    source_declaration(ty, budget, range, depth + 1)?,
                ));
            }
            D::Record(output)
        }
        _ => {
            return Err(QueryError::input(
                range.clone(),
                "unsupported source declaration",
            ));
        }
    };
    if ty.optional {
        budget
            .charge_results(1, zirql::Location::Query(range.clone()))
            .map_err(pipeline::core_error)
            .map_err(input_error)?;
        Ok(D::Optional(Box::new(declaration)))
    } else {
        Ok(declaration)
    }
}

fn evaluate_literal(
    literal: &structured::LiteralValue<'_>,
    input: &Input,
    facts: Option<&ModelFacts<'_>>,
    metadata: Option<LiteralMetadata<'_>>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<V, QueryError> {
    let expression = project_literal(literal, input, facts, metadata, budget, nesting, 0)?;
    let value = zirql::evaluate_value_with_host_validation(
        &expression,
        &zirql::Bindings::default(),
        budget,
        validate_report_value,
    )
    .map_err(pipeline::core_error)
    .map_err(input_error)?;
    validate_report_shape(&value, budget, &literal.range).map_err(input_error)?;
    if let Some(metadata) = metadata {
        let declaration = source_declaration(metadata.ty, budget, &literal.range, 0)?;
        zirql::declare_constructed(
            value,
            declaration,
            budget,
            zirql::Location::Query(literal.range.clone()),
        )
        .map_err(pipeline::core_error)
        .map_err(input_error)
    } else {
        Ok(value)
    }
}

pub(crate) fn project(
    stream: &SemanticStream<'_>,
    facts: Option<&ModelFacts<'_>>,
    budget: &mut Budget,
    range: &Range<usize>,
    nesting: usize,
) -> Result<V, QueryError> {
    let SemanticStream::Inspection(result) = stream else {
        return Err(QueryError::query(
            range.clone(),
            "native selections require an explicit names, count or values projection",
        ));
    };
    match result.as_ref() {
        QueryResult::Count(n) => {
            charge(budget, 1, range)?;
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            Ok(V::Integer(i64::try_from(*n).map_err(|_| {
                QueryError::query(range.clone(), "count exceeds signed 64-bit range")
            })?))
        }
        QueryResult::Strings(strings) => {
            let mut values = slots(budget, strings.len(), range)?;
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            for string in strings {
                charge(budget, string.len() + 1, range)?;
                values.push(V::String(string.to_owned()));
            }
            Ok(V::Array(values))
        }
        QueryResult::Literals(selection) => {
            let mut values = slots(budget, selection.values().len(), range)?;
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            for literal in selection.values() {
                let input = selection.input();
                let mut metadata = None;
                if let Some(facts) = facts {
                    for assignment in input.syntax().tree().child_nodes() {
                        charge(budget, 1 + assignment.children().len(), range)?;
                        if assignment.kind() == N::Assignment
                            && crate::assignment_rhs(assignment).is_some_and(|rhs| {
                                input.original_range(rhs.range()) == literal.range
                            })
                        {
                            let token = structured::direct_token(input, assignment).unwrap();
                            let name = crate::identifier_identity(
                                std::str::from_utf8(input.token_bytes(token)).unwrap(),
                            );
                            metadata = declared_value(facts, name, budget, &literal.range)
                                .map_err(input_error)?;
                        }
                    }
                }
                values.push(evaluate_literal(
                    literal, input, facts, metadata, budget, nesting,
                )?);
            }
            Ok(V::Array(values))
        }
        _ => Err(QueryError::query(
            range.clone(),
            "native result requires an explicit supported scalar projection",
        )),
    }
}

/// Read-only reports retain the generic sparse core value contract.
pub(crate) fn validate_report_shape(
    value: &V,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    validate_report_value(value, budget, &zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)
}
/// The trusted runtime policy used before an inline value reaches its parent.
pub(crate) fn validate_report_value(
    value: &V,
    budget: &mut Budget,
    location: &zirql::Location,
) -> Result<(), zirql::QueryError> {
    zirql::constructed_value_type(value, budget, location.clone())?;
    Ok(())
}

pub(crate) fn validate_data_value(
    value: &V,
    budget: &mut Budget,
    location: &zirql::Location,
) -> Result<(), zirql::QueryError> {
    zirql::constructed_value_type(value, budget, location.clone())?;
    check_shape(value, budget, location)
}

pub(crate) fn validate_data_shape(
    value: &V,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    validate_data_value(value, budget, &zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)
}

fn check_data_declaration(
    ty: &zirql::DeclaredType<()>,
    budget: &mut Budget,
    location: &zirql::Location,
) -> Result<(), zirql::QueryError> {
    use zirql::DeclaredType as D;
    budget.charge(1, location.clone())?;
    match ty {
        D::Optional(_) => {
            return Err(zirql::QueryError {
                location: location.clone(),
                message: "optional values are unavailable in data expressions".into(),
                category: zirql::QueryErrorCategory::Other,
            });
        }
        D::Array(child) | D::Sequence(child) | D::Keyed(child) => {
            check_data_declaration(child, budget, location)?;
        }
        D::Tuple(types) => {
            for ty in types {
                check_data_declaration(ty, budget, location)?;
            }
        }
        D::Record(fields) => {
            for (_, ty) in fields {
                check_data_declaration(ty, budget, location)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_shape(
    value: &V,
    budget: &mut Budget,
    location: &zirql::Location,
) -> Result<(), zirql::QueryError> {
    budget.charge(1, location.clone())?;
    match value {
        V::Declared { value, declaration } => {
            check_data_declaration(declaration, budget, location)?;
            check_shape(value, budget, location)?;
        }
        V::Absent => {
            return Err(zirql::QueryError {
                location: location.clone(),
                message: "absence is unavailable in data expressions".into(),
                category: zirql::QueryErrorCategory::Other,
            });
        }
        V::KeyedArray { axes, entries } => {
            budget.charge(axes.len(), location.clone())?;
            let mut product = usize::from(!axes.iter().any(|axis| axis.keys.is_empty()));
            for axis in axes {
                budget.charge(1 + axis.keys.len(), location.clone())?;
                product =
                    product
                        .checked_mul(axis.keys.len())
                        .ok_or_else(|| zirql::QueryError {
                            location: location.clone(),
                            message: "array axis product overflow".into(),
                            category: zirql::QueryErrorCategory::Other,
                        })?;
                for pair in axis.keys.windows(2) {
                    let contiguous = match (&pair[0], &pair[1]) {
                        (zirql::ValueKey::Integer(a), zirql::ValueKey::Integer(b)) => {
                            a.checked_add(1) == Some(*b)
                        }
                        (zirql::ValueKey::Member(a), zirql::ValueKey::Member(b)) => {
                            a.type_id == b.type_id
                                && a.declaration_order.checked_add(1) == Some(b.declaration_order)
                        }
                        _ => false,
                    };
                    if !contiguous {
                        return Err(zirql::QueryError {
                            location: location.clone(),
                            message: "array axis must be increasing and contiguous".into(),
                            category: zirql::QueryErrorCategory::Other,
                        });
                    }
                }
            }
            // The core has already checked identity, uniqueness and row-major keys.
            // Cardinality therefore proves full-product coverage without expansion.
            budget.charge(entries.len(), location.clone())?;
            if product != entries.len() {
                return Err(zirql::QueryError {
                    location: location.clone(),
                    message: "keyed array has missing cells in its rectangular axis product".into(),
                    category: zirql::QueryErrorCategory::Other,
                });
            }
            for entry in entries {
                check_shape(&entry.value, budget, location)?;
            }
        }
        V::Array(values)
        | V::Tuple(values)
        | V::Set {
            elements: values, ..
        } => {
            for value in values {
                check_shape(value, budget, location)?;
            }
        }
        V::Record(fields) => {
            for (_, value) in fields {
                check_shape(value, budget, location)?;
            }
        }
        V::Integer(_) | V::Float(_) | V::Boolean(_) | V::String(_) | V::Member(_) => {}
    }
    Ok(())
}

pub(crate) fn json_value(
    value: &V,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Value, QueryError> {
    validate_report_shape(value, budget, range)?;
    tagged(value, budget, range)
}
fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
fn tagged(value: &V, budget: &mut Budget, range: &Range<usize>) -> Result<Value, QueryError> {
    charge(budget, 64, range)?;
    budget
        .charge_results(8, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    Ok(match value.data() {
        V::Declared { .. } => unreachable!("transparent annotation"),
        V::Absent => json!({"kind":"absent"}),
        V::Integer(v) => json!({"kind":"integer", "value":v}),
        V::Float(v) => json!({"kind":"float", "value":v}),
        V::Boolean(v) => json!({"kind":"boolean", "value":v}),
        V::String(v) => {
            charge(budget, v.len(), range)?;
            json!({"kind":"string", "value":v})
        }
        V::Member(member) => {
            charge(
                budget,
                member.type_name.len() + member.member_name.len() + 8,
                range,
            )?;
            json!({"kind":"member", "enum":member.type_name, "name":member.member_name})
        }
        V::KeyedArray { axes, entries } => {
            let mut output_axes = slots(budget, axes.len(), range)?;
            for axis in axes {
                let mut keys = slots(budget, axis.keys.len(), range)?;
                for key in &axis.keys {
                    keys.push(tagged_key(key, budget, range)?);
                }
                output_axes.push(Value::Array(keys));
            }
            let mut output_entries = slots(budget, entries.len(), range)?;
            for entry in entries {
                charge(budget, 32, range)?;
                budget
                    .charge_results(5, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                let mut keys = slots(budget, entry.key.len(), range)?;
                for key in &entry.key {
                    keys.push(tagged_key(key, budget, range)?);
                }
                output_entries.push(object([
                    ("key", Value::Array(keys)),
                    ("value", tagged(&entry.value, budget, range)?),
                ]));
            }
            object([
                ("kind", Value::String("array".into())),
                ("indexing", Value::String("explicit".into())),
                ("axes", Value::Array(output_axes)),
                ("entries", Value::Array(output_entries)),
            ])
        }
        V::Record(fields) => {
            budget
                .charge_results(fields.len(), zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            let mut result = serde_json::Map::with_capacity(fields.len());
            for (label, value) in fields {
                charge(budget, label.len() + 16, range)?;
                result.insert(label.to_owned(), tagged(value, budget, range)?);
            }
            Value::Object(result)
        }
        V::Array(values) => {
            let mut axes = slots(budget, values.len(), range)?;
            let mut entries = slots(budget, values.len(), range)?;
            for (i, value) in values.iter().enumerate() {
                let position = i
                    .checked_add(1)
                    .and_then(|n| i64::try_from(n).ok())
                    .ok_or_else(|| QueryError::query(range.clone(), "positional key overflow"))?;
                charge(budget, 96, range)?;
                budget
                    .charge_results(22, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                axes.push(json!({"kind":"integer", "value":position}));
                let key = Value::Array(vec![json!({"kind":"integer", "value":position})]);
                entries.push(object([
                    ("key", key),
                    ("value", tagged(value, budget, range)?),
                ]));
            }
            object([
                ("kind", Value::String("array".into())),
                ("indexing", Value::String("positional".into())),
                ("axes", Value::Array(vec![Value::Array(axes)])),
                ("entries", Value::Array(entries)),
            ])
        }
        V::Tuple(values)
        | V::Set {
            elements: values, ..
        } => {
            let mut elements = slots(budget, values.len(), range)?;
            for value in values {
                elements.push(tagged(value, budget, range)?);
            }
            object([
                (
                    "kind",
                    Value::String(
                        if matches!(value.data(), V::Tuple(_)) {
                            "tuple"
                        } else {
                            "set"
                        }
                        .into(),
                    ),
                ),
                ("elements", Value::Array(elements)),
            ])
        }
    })
}

fn tagged_key(
    key: &zirql::ValueKey,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Value, QueryError> {
    charge(budget, 64, range)?;
    budget
        .charge_results(8, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    Ok(match key {
        zirql::ValueKey::Integer(n) => json!({"kind":"integer", "value":n}),
        zirql::ValueKey::Member(member) => {
            charge(
                budget,
                member.type_name.len() + member.member_name.len(),
                range,
            )?;
            json!({"kind":"member", "enum":member.type_name, "name":member.member_name})
        }
    })
}

pub(crate) fn read_scalar_predicate(
    reader: &mut zirql::ArgumentReader<'_>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<zirql::ValueExpression, QueryError> {
    let mut host = DataReader {
        members: Vec::new(),
    };
    let predicate = zirql::read_predicate_argument(reader, &mut host, &[], budget, nesting)
        .map_err(pipeline::core_error)?;
    admit_data(
        &predicate.body,
        reader.source(),
        budget,
        &mut |_, range, _| {
            Err(QueryError::query(
                range.clone(),
                "native values are unavailable in scalar predicates",
            ))
        },
    )?;
    if let Some(member) = host.members.first() {
        return Err(QueryError::query(
            member.range.clone(),
            "scalar element predicates need established nominal facts; use a direct member comparison",
        ));
    }
    let range = predicate.body.range.clone();
    let body = crate::program::builder::map_queries(
        predicate.bind(Located::new(K::Name("\0scalar".into()), range)),
        &mut |_| unreachable!("scalar predicate has no native query"),
    );
    Ok(body)
}
