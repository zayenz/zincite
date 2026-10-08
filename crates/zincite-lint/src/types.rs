//! Bounded type-inst ordering used by callable matching, without data evaluation.
use crate::{DeclarationId, Instantiation};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeInst {
    pub instantiation: Instantiation,
    pub optional: bool,
    pub kind: TypeKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    Bool,
    Int,
    Float,
    String,
    Annotation,
    Enum(DeclarationId),
    Set(Box<TypeInst>),
    Array {
        indices: Vec<TypeInst>,
        element: Box<TypeInst>,
    },
    Tuple(Vec<TypeInst>),
    Record(Vec<(String, TypeInst)>),
    Variable {
        name: String,
        enum_only: bool,
        any: bool,
    },
    /// Empty collections/absent values constrain their type at the use site.
    Bottom,
    Unknown(String),
}

impl TypeInst {
    pub(super) fn par(kind: TypeKind) -> Self {
        Self {
            instantiation: Instantiation::Parameter,
            optional: false,
            kind,
        }
    }
    pub(super) fn unknown(reason: &str) -> Self {
        Self {
            instantiation: Instantiation::Unknown,
            optional: false,
            kind: TypeKind::Unknown(reason.into()),
        }
    }
    pub(super) fn with_inst(mut self, instantiation: Instantiation) -> Self {
        self.instantiation = instantiation;
        match &mut self.kind {
            TypeKind::Array { element, .. } => **element = element.clone().with_inst(instantiation),
            TypeKind::Tuple(fields) => {
                for field in fields {
                    *field = field.clone().with_inst(instantiation);
                }
            }
            TypeKind::Record(fields) => {
                for (_, field) in fields {
                    *field = field.clone().with_inst(instantiation);
                }
            }
            _ => {}
        }
        self
    }
    pub(super) fn known(&self) -> bool {
        self.instantiation != Instantiation::Unknown
            && match &self.kind {
                TypeKind::Unknown(_) | TypeKind::Variable { .. } => false,
                TypeKind::Array { indices, element } => {
                    indices.iter().all(Self::known) && element.known()
                }
                TypeKind::Set(element) => element.known(),
                TypeKind::Tuple(fields) => fields.iter().all(Self::known),
                TypeKind::Record(fields) => fields.iter().all(|(_, field)| field.known()),
                _ => true,
            }
    }
}

pub(super) fn aggregate(types: &[TypeInst]) -> Instantiation {
    if types
        .iter()
        .any(|ty| ty.instantiation == Instantiation::Decision)
    {
        Instantiation::Decision
    } else if types
        .iter()
        .any(|ty| ty.instantiation == Instantiation::Unknown)
    {
        Instantiation::Unknown
    } else {
        Instantiation::Parameter
    }
}

/// Componentwise supported coercions; domain cardinalities are never invented.
/// https://docs.minizinc.dev/en/2.10.1/spec.html#types
pub(super) fn coerces(from: &TypeInst, to: &TypeInst) -> bool {
    if !from.known() || !to.known() {
        return false;
    }
    if from.optional && !to.optional {
        return false;
    }
    if from.instantiation == Instantiation::Decision && to.instantiation == Instantiation::Parameter
    {
        return false;
    }
    use TypeKind::*;
    match (&from.kind, &to.kind) {
        (Bottom, _) => true,
        (Bool, Int | Float) | (Int, Float) | (Enum(_), Int | Float) => true,
        (Set(a), Set(b)) => coerces(a, b),
        (
            Array {
                indices: a,
                element: x,
            },
            Array {
                indices: b,
                element: y,
            },
        ) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| coerces(a, b)) && coerces(x, y),
        (Tuple(a), Tuple(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| coerces(a, b)),
        (Record(a), Record(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|((n, a), (m, b))| n == m && coerces(a, b))
        }
        (a, b) => a == b,
    }
}

pub(super) fn join(a: &TypeInst, b: &TypeInst) -> TypeInst {
    if !a.known() || !b.known() {
        return TypeInst::unknown("component type is unknown");
    }
    let instantiation = aggregate(&[a.clone(), b.clone()]);
    let mut left = a.clone();
    let mut right = b.clone();
    left.instantiation = instantiation;
    right.instantiation = instantiation;
    left.optional = a.optional || b.optional;
    right.optional = left.optional;
    if coerces(&left, &right) {
        return right;
    }
    if coerces(&right, &left) {
        return left;
    }
    let kind = match (&a.kind, &b.kind) {
        (TypeKind::Bool, TypeKind::Enum(_)) | (TypeKind::Enum(_), TypeKind::Bool) => TypeKind::Int,
        (TypeKind::Set(x), TypeKind::Set(y)) => TypeKind::Set(Box::new(join(x, y))),
        (
            TypeKind::Array {
                indices: x,
                element: a,
            },
            TypeKind::Array {
                indices: y,
                element: b,
            },
        ) if x == y => TypeKind::Array {
            indices: x.clone(),
            element: Box::new(join(a, b)),
        },
        (TypeKind::Tuple(x), TypeKind::Tuple(y)) if x.len() == y.len() => {
            TypeKind::Tuple(x.iter().zip(y).map(|(a, b)| join(a, b)).collect())
        }
        (TypeKind::Record(x), TypeKind::Record(y))
            if x.iter().map(|(n, _)| n).eq(y.iter().map(|(n, _)| n)) =>
        {
            TypeKind::Record(
                x.iter()
                    .zip(y)
                    .map(|((n, a), (_, b))| (n.clone(), join(a, b)))
                    .collect(),
            )
        }
        _ => return TypeInst::unknown("types have no supported common type"),
    };
    TypeInst {
        instantiation,
        optional: left.optional,
        kind,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Match {
    Yes,
    No,
    Unknown,
}
impl Match {
    fn and(self, other: Self) -> Self {
        if self == Self::No || other == Self::No {
            Self::No
        } else if self == Self::Unknown || other == Self::Unknown {
            Self::Unknown
        } else {
            Self::Yes
        }
    }
}

pub(super) fn match_type(
    actual: &TypeInst,
    formal: &TypeInst,
    variables: &mut BTreeMap<String, TypeInst>,
) -> Match {
    if let TypeKind::Unknown(_) = formal.kind {
        return Match::Unknown;
    }
    if let TypeKind::Unknown(_) = actual.kind {
        return Match::Unknown;
    }
    let any = matches!(formal.kind, TypeKind::Variable { any: true, .. });
    if !any && actual.optional && !formal.optional {
        return Match::No;
    }
    if !any
        && actual.instantiation == Instantiation::Decision
        && formal.instantiation == Instantiation::Parameter
    {
        return Match::No;
    }
    if let TypeKind::Variable {
        name,
        enum_only,
        any,
    } = &formal.kind
    {
        if actual.kind == TypeKind::Bottom {
            return Match::Yes;
        }
        if *enum_only && !matches!(actual.kind, TypeKind::Int | TypeKind::Enum(_)) {
            return if actual.known() {
                Match::No
            } else {
                Match::Unknown
            };
        }
        if !actual.known() {
            return Match::Unknown;
        }
        let mut bound = actual.clone();
        if !any {
            bound = bound.with_inst(Instantiation::Parameter);
            bound.optional = false;
        }
        if let Some(previous) = variables.get(name) {
            if *enum_only
                && matches!((&previous.kind,&bound.kind),(TypeKind::Enum(a),TypeKind::Enum(b)) if a!=b)
            {
                return Match::No;
            }
            bound = join(previous, &bound);
            if !bound.known() {
                return Match::Unknown;
            }
        }
        variables.insert(name.clone(), bound);
        return Match::Yes;
    }
    use TypeKind::*;
    match (&actual.kind, &formal.kind) {
        (Bottom, _) => Match::Yes,
        (Set(a), Set(b)) => match_type(a, b, variables),
        (
            Array {
                indices: a,
                element: x,
            },
            Array {
                indices: b,
                element: y,
            },
        ) if a.len() == b.len() => {
            let mut state = match_type(x, y, variables);
            for (a, b) in a.iter().zip(b) {
                state = state.and(match_type(a, b, variables));
            }
            state
        }
        (Tuple(a), Tuple(b)) if a.len() == b.len() => {
            a.iter().zip(b).fold(Match::Yes, |state, (a, b)| {
                state.and(match_type(a, b, variables))
            })
        }
        (Record(a), Record(b))
            if a.len() == b.len() && a.iter().map(|(n, _)| n).eq(b.iter().map(|(n, _)| n)) =>
        {
            a.iter().zip(b).fold(Match::Yes, |state, ((_, a), (_, b))| {
                state.and(match_type(a, b, variables))
            })
        }
        _ if actual.known() && formal.known() => {
            if coerces(actual, formal) {
                Match::Yes
            } else {
                Match::No
            }
        }
        _ if matches!(actual.kind, Array { .. } | Set(_) | Tuple(_) | Record(_))
            || matches!(formal.kind, Array { .. } | Set(_) | Tuple(_) | Record(_)) =>
        {
            Match::No
        }
        _ => Match::Unknown,
    }
}

pub(super) fn substitute(ty: &TypeInst, variables: &BTreeMap<String, TypeInst>) -> TypeInst {
    let mut result = ty.clone();
    result.kind = match &ty.kind {
        TypeKind::Variable { name, any, .. } => {
            let Some(mut bound) = variables.get(name).cloned() else {
                return TypeInst::unknown("type-inst variable is unconstrained");
            };
            if !any {
                bound = bound.with_inst(ty.instantiation);
                bound.optional = ty.optional;
            }
            return bound;
        }
        TypeKind::Set(e) => TypeKind::Set(Box::new(substitute(e, variables))),
        TypeKind::Array { indices, element } => TypeKind::Array {
            indices: indices.iter().map(|ty| substitute(ty, variables)).collect(),
            element: Box::new(substitute(element, variables)),
        },
        TypeKind::Tuple(fields) => {
            TypeKind::Tuple(fields.iter().map(|ty| substitute(ty, variables)).collect())
        }
        TypeKind::Record(fields) => TypeKind::Record(
            fields
                .iter()
                .map(|(n, ty)| (n.clone(), substitute(ty, variables)))
                .collect(),
        ),
        other => other.clone(),
    };
    if let TypeKind::Array { element, .. } = &result.kind {
        result.instantiation = element.instantiation;
    }
    result
}

pub(super) fn pattern_narrower(a: &TypeInst, b: &TypeInst) -> bool {
    match (&a.kind, &b.kind) {
        (
            TypeKind::Variable {
                enum_only: a,
                any: x,
                ..
            },
            TypeKind::Variable {
                enum_only: b,
                any: y,
                ..
            },
        ) => (!b || *a) && (!x || *y),
        (_, TypeKind::Variable { .. }) => true,
        (TypeKind::Variable { .. }, _) => false,
        (TypeKind::Set(a), TypeKind::Set(b)) => pattern_narrower(a, b),
        (
            TypeKind::Array {
                indices: a,
                element: x,
            },
            TypeKind::Array {
                indices: b,
                element: y,
            },
        ) => {
            a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| pattern_narrower(a, b))
                && pattern_narrower(x, y)
        }
        (TypeKind::Tuple(a), TypeKind::Tuple(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| pattern_narrower(a, b))
        }
        (TypeKind::Record(a), TypeKind::Record(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|((n, a), (m, b))| n == m && pattern_narrower(a, b))
        }
        _ => true,
    }
}
