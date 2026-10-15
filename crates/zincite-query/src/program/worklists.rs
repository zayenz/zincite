//! Native storage and owner checks for the shared computational runtime.
use super::*;
use zirql::{DeclaredType as D, Location, ProgramHost, ProjectionHost};

/// An opaque retained queue or identity set. Its storage is never serialized.
#[derive(Debug)]
pub struct Computational<'a> {
    pub(super) value: Rc<zirql::ComputationalValue<ReportValue<'a>, Kind>>,
    pub(super) range: Range<usize>,
}
impl Computational<'_> {
    pub fn len(&self) -> usize {
        match self.value.as_ref() {
            zirql::ComputationalValue::Queue(value) => value.len(),
            zirql::ComputationalValue::IdentitySet(value) => value.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
}
/// A tuple retaining native field owners and original coordinates.
#[derive(Debug)]
pub struct NativeTuple<'a> {
    pub(super) values: Rc<Vec<ReportValue<'a>>>,
    pub(super) range: Range<usize>,
}
impl<'a> NativeTuple<'a> {
    pub fn values(&self) -> &[ReportValue<'a>] {
        &self.values
    }
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
}
/// An eager native sequence, retaining its element contract when empty.
#[derive(Debug)]
pub struct NativeSequence<'a> {
    pub(super) values: Rc<Vec<ReportValue<'a>>>,
    pub(super) element: Rc<D<Kind>>,
    pub(super) range: Range<usize>,
}
impl<'a> NativeSequence<'a> {
    pub fn values(&self) -> &[ReportValue<'a>] {
        &self.values
    }
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
}
pub(super) fn copy<'a>(value: &ReportValue<'a>) -> Option<ReportValue<'a>> {
    Some(match value {
        ReportValue::Computational(value) => ReportValue::Computational(Computational {
            value: Rc::clone(&value.value),
            range: value.range.clone(),
        }),
        ReportValue::Tuple(value) => ReportValue::Tuple(NativeTuple {
            values: Rc::clone(&value.values),
            range: value.range.clone(),
        }),
        ReportValue::Sequence(value) => ReportValue::Sequence(NativeSequence {
            values: Rc::clone(&value.values),
            element: Rc::clone(&value.element),
            range: value.range.clone(),
        }),
        _ => return None,
    })
}
pub(super) fn owner<'v, 'a>(value: &'v ReportValue<'a>) -> Option<&'v Rc<ModelFacts<'a>>> {
    match value {
        ReportValue::Retained(value) => owner(value),
        ReportValue::Item(value) => owner(value),
        ReportValue::EntitySequence { owner, .. } => owner.as_ref(),
        _ => None,
    }
}
pub(super) fn declaration(
    value: &D<Kind>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Rc<D<Kind>>, zirql::QueryError> {
    let location = Location::Query(range.clone());
    budget.charge_results(1, location.clone())?;
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        budget.charge(1, location.clone())?;
        budget.charge_results(1, location.clone())?;
        match value {
            D::Optional(v) | D::Sequence(v) | D::Array(v) | D::Queue(v) | D::Keyed(v) => {
                pending.push(v)
            }
            D::Tuple(values) => {
                budget.charge_results(values.len(), location.clone())?;
                pending.extend(values);
            }
            D::Record(values) => {
                budget.charge_results(values.len(), location.clone())?;
                for (name, value) in values {
                    budget.charge(name.len() + 1, location.clone())?;
                    pending.push(value);
                }
            }
            _ => {}
        }
    }
    Ok(Rc::new(value.clone()))
}
impl<'a> composition::CompositionHost<'_, '_> {
    pub(super) fn sequence_values(
        &self,
        value: &ReportValue<'a>,
        element: &D<Kind>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<ReportValue<'a>>, zirql::EvaluationError<QueryError>> {
        let location = Location::Query(range.clone());
        if let ReportValue::Sequence(sequence) = functions::unretained(value) {
            if sequence.element.as_ref() != element {
                return Err(zirql::QueryError::query(
                    range.clone(),
                    "native sequence element contract mismatch",
                )
                .into());
            }
            budget.check_items(sequence.values.len(), location.clone())?;
            budget.charge_results(sequence.values.len(), location)?;
            return sequence
                .values
                .iter()
                .map(|value| self.copy_value(value, context, budget, range))
                .collect();
        }
        zirql::admit_value(
            value,
            &D::Sequence(Box::new(element.clone())),
            self,
            context,
            budget,
            location,
        )?;
        let copied = self.copy_value(value, context, budget, range)?;
        self.subjects(copied, context, budget, range)
    }
    pub(super) fn identity(
        &self,
        value: &ReportValue<'a>,
        expected: &Kind,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::NativeIdentity, zirql::EvaluationError<QueryError>> {
        zirql::admit_value(
            value,
            &D::Native(expected.clone()),
            self,
            context,
            budget,
            Location::Query(range.clone()),
        )?;
        match (expected, functions::unretained(value)) {
            (Kind::Declaration, ReportValue::Query(SemanticStream::Entities(rows))) => {
                let semantic::SemanticEntity::Declaration(id) = rows[0] else {
                    unreachable!("admitted declaration")
                };
                let owner = owner(value).expect("admitted declaration owner");
                Ok(zirql::NativeIdentity([
                    owner.bindings() as *const _ as usize as u64,
                    0,
                    id.0 as u64,
                    0,
                ]))
            }
            (Kind::Node, ReportValue::Query(SemanticStream::Inspection(result))) => {
                let (input, node) = match result.as_ref() {
                    QueryResult::Nodes(selection) => (selection.input, selection.nodes[0].node),
                    QueryResult::Selection(selection) => (selection.input, selection.items[0].node),
                    _ => unreachable!("admitted node"),
                };
                Ok(zirql::NativeIdentity([
                    input as *const _ as usize as u64,
                    1,
                    node as *const _ as usize as u64,
                    0,
                ]))
            }
            _ => Err(zirql::QueryError::query(
                range.clone(),
                "identity requires one Declaration or Node",
            )
            .into()),
        }
    }
}

pub(super) fn map_signature(
    value: &mut Located<zirql::SignatureType>,
    map: &impl Fn(Range<usize>) -> Range<usize>,
) {
    use zirql::SignatureType as S;
    value.range = map(value.range.clone());
    match &mut value.value {
        S::Named(name) => name.range = map(name.range.clone()),
        S::Optional(value)
        | S::Sequence(value)
        | S::Set(value)
        | S::Keyed(value)
        | S::Queue(value)
        | S::IdentitySet(value) => map_signature(value, map),
        S::Tuple(values) => {
            for value in values {
                map_signature(value, map);
            }
        }
        S::Record(values) => {
            for (name, value) in values {
                name.range = map(name.range.clone());
                map_signature(value, map);
            }
        }
        S::Boolean | S::Integer | S::Float | S::String => {}
    }
}
