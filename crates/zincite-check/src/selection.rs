//! Exact selection signatures and flat-array layouts shared by admission and checking.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionForm {
    OneBased,
    Offset,
    Matrix,
    SetMatrix,
    Conditional,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectionName {
    pub kind: ValueType,
    pub form: SelectionForm,
    pub allows_variables: bool,
}

impl SelectionName {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "array_bool_element" | "gecode_array_bool_element" | "fzn_array_bool_element" => Self {
                kind: ValueType::Boolean,
                form: SelectionForm::OneBased,
                allows_variables: false,
            },
            "array_var_bool_element"
            | "gecode_array_var_bool_element"
            | "fzn_array_var_bool_element" => Self {
                kind: ValueType::Boolean,
                form: SelectionForm::OneBased,
                allows_variables: true,
            },
            "array_var_bool_element_nonshifted" => Self {
                kind: ValueType::Boolean,
                form: SelectionForm::OneBased,
                allows_variables: true,
            },
            "array_int_element" | "gecode_array_int_element" | "fzn_array_int_element" => Self {
                kind: ValueType::Integer,
                form: SelectionForm::OneBased,
                allows_variables: false,
            },
            "array_var_int_element"
            | "gecode_array_var_int_element"
            | "fzn_array_var_int_element" => Self {
                kind: ValueType::Integer,
                form: SelectionForm::OneBased,
                allows_variables: true,
            },
            "array_var_int_element_nonshifted" => Self {
                kind: ValueType::Integer,
                form: SelectionForm::OneBased,
                allows_variables: true,
            },
            "array_set_element" | "gecode_array_set_element" | "fzn_array_set_element" => Self {
                kind: ValueType::IntegerSet,
                form: SelectionForm::OneBased,
                allows_variables: false,
            },
            "array_var_set_element"
            | "gecode_array_var_set_element"
            | "fzn_array_var_set_element" => Self {
                kind: ValueType::IntegerSet,
                form: SelectionForm::OneBased,
                allows_variables: true,
            },
            "array_var_set_element_nonshifted" => Self {
                kind: ValueType::IntegerSet,
                form: SelectionForm::OneBased,
                allows_variables: true,
            },
            "array_float_element" => Self {
                kind: ValueType::Float,
                form: SelectionForm::OneBased,
                allows_variables: false,
            },
            "array_var_float_element" => Self {
                kind: ValueType::Float,
                form: SelectionForm::OneBased,
                allows_variables: true,
            },
            "array_var_float_element_nonshifted" => Self {
                kind: ValueType::Float,
                form: SelectionForm::OneBased,
                allows_variables: true,
            },
            "gecode_int_element" | "gecode_gecode_int_element" | "fzn_gecode_int_element" => Self {
                kind: ValueType::Integer,
                form: SelectionForm::Offset,
                allows_variables: true,
            },
            "gecode_var_int_element"
            | "gecode_gecode_var_int_element"
            | "fzn_gecode_var_int_element" => Self {
                kind: ValueType::Integer,
                form: SelectionForm::Offset,
                allows_variables: true,
            },
            "gecode_int_element2d" | "gecode_gecode_int_element2d" | "fzn_gecode_int_element2d" => {
                Self {
                    kind: ValueType::Integer,
                    form: SelectionForm::Matrix,
                    allows_variables: true,
                }
            }
            "gecode_bool_element" | "gecode_gecode_bool_element" | "fzn_gecode_bool_element" => {
                Self {
                    kind: ValueType::Boolean,
                    form: SelectionForm::Offset,
                    allows_variables: true,
                }
            }
            "gecode_var_bool_element"
            | "gecode_gecode_var_bool_element"
            | "fzn_gecode_var_bool_element" => Self {
                kind: ValueType::Boolean,
                form: SelectionForm::Offset,
                allows_variables: true,
            },
            "gecode_bool_element2d"
            | "gecode_gecode_bool_element2d"
            | "fzn_gecode_bool_element2d" => Self {
                kind: ValueType::Boolean,
                form: SelectionForm::Matrix,
                allows_variables: true,
            },
            "gecode_set_element2d" | "gecode_gecode_set_element2d" | "fzn_gecode_set_element2d" => {
                Self {
                    kind: ValueType::IntegerSet,
                    form: SelectionForm::SetMatrix,
                    allows_variables: true,
                }
            }
            "gecode_bool_ite" | "gecode_gecode_bool_ite" | "fzn_gecode_bool_ite" => Self {
                kind: ValueType::Boolean,
                form: SelectionForm::Conditional,
                allows_variables: true,
            },
            "gecode_int_ite" | "gecode_gecode_int_ite" | "fzn_gecode_int_ite" => Self {
                kind: ValueType::Integer,
                form: SelectionForm::Conditional,
                allows_variables: true,
            },
            "gecode_set_ite" | "gecode_gecode_set_ite" | "fzn_gecode_set_ite" => Self {
                kind: ValueType::IntegerSet,
                form: SelectionForm::Conditional,
                allows_variables: true,
            },
            "gecode_float_ite" | "gecode_gecode_float_ite" | "fzn_gecode_float_ite" => Self {
                kind: ValueType::Float,
                form: SelectionForm::Conditional,
                allows_variables: true,
            },
            _ => return None,
        })
    }

    #[cfg(feature = "prepare")]
    pub fn arity(self) -> usize {
        match self.form {
            SelectionForm::OneBased => 3,
            SelectionForm::Offset | SelectionForm::Conditional => 4,
            SelectionForm::Matrix | SelectionForm::SetMatrix => 6,
        }
    }

    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<prepare::ArgumentType> {
        let scalar = |kind, is_var| prepare::ArgumentType {
            kind,
            is_var,
            is_array: false,
        };
        let array = prepare::ArgumentType {
            kind: self.kind,
            is_var: self.allows_variables,
            is_array: true,
        };
        let index = scalar(ValueType::Integer, true);
        let fixed_int = scalar(ValueType::Integer, false);
        let result = scalar(self.kind, true);
        match self.form {
            SelectionForm::OneBased => vec![index, array, result],
            SelectionForm::Offset => vec![index, fixed_int, array, result],
            SelectionForm::Matrix => vec![
                index.clone(),
                index,
                array,
                scalar(ValueType::IntegerSet, false),
                scalar(ValueType::IntegerSet, false),
                result,
            ],
            SelectionForm::SetMatrix => vec![
                array,
                index.clone(),
                fixed_int.clone(),
                index,
                fixed_int,
                result,
            ],
            SelectionForm::Conditional => {
                vec![
                    scalar(ValueType::Boolean, true),
                    result.clone(),
                    result.clone(),
                    result,
                ]
            }
        }
    }
}

/// The fixed indexing contract of a flattened element relation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ElementLayout {
    OneBased,
    Offset {
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        offset: i64,
    },
    Matrix {
        rows: IntegerSet,
        columns: IntegerSet,
    },
    SetMatrix {
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        width: i64,
        #[serde(
            serialize_with = "serialize_integer",
            deserialize_with = "deserialize_integer"
        )]
        height: i64,
    },
}

impl std::fmt::Display for ElementLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OneBased => f.write_str("one-based positions"),
            Self::Offset { offset } => write!(f, "offset {offset}"),
            Self::Matrix { rows, columns } => write!(f, "rows {rows}, columns {columns}"),
            Self::SetMatrix { width, height } => {
                write!(f, "zero-based matrix with width {width}, height {height}")
            }
        }
    }
}

pub(crate) enum SelectionPosition {
    Missing,
    OutOfRange,
    Selected(usize),
}

impl ElementLayout {
    pub(crate) fn form(&self) -> SelectionForm {
        match self {
            Self::OneBased => SelectionForm::OneBased,
            Self::Offset { .. } => SelectionForm::Offset,
            Self::Matrix { .. } => SelectionForm::Matrix,
            Self::SetMatrix { .. } => SelectionForm::SetMatrix,
        }
    }

    pub(crate) fn index_count(&self) -> usize {
        match self {
            Self::OneBased | Self::Offset { .. } => 1,
            Self::Matrix { .. } | Self::SetMatrix { .. } => 2,
        }
    }

    pub(crate) fn validate(&self, length: usize) -> Result<(), &'static str> {
        let dimensions = match self {
            Self::OneBased | Self::Offset { .. } => return Ok(()),
            Self::Matrix { rows, columns } => {
                let bounds = |set: &IntegerSet| {
                    let (lower, upper) = set
                        .contiguous_bounds()
                        .ok_or("element index sets must be nonempty and contiguous")?;
                    usize::try_from(i128::from(upper) - i128::from(lower) + 1)
                        .map_err(|_| "element dimensions exceed this target's addressable size")
                };
                (bounds(rows)?, bounds(columns)?)
            }
            Self::SetMatrix { width, height } => {
                if *width <= 0 || *height <= 0 {
                    return Err("element width and height must be positive");
                }
                let size = |value| {
                    usize::try_from(value)
                        .map_err(|_| "element dimensions exceed this target's addressable size")
                };
                (size(*height)?, size(*width)?)
            }
        };
        if dimensions.0.checked_mul(dimensions.1) != Some(length) {
            return Err("element matrix dimensions must match the flat array length");
        }
        Ok(())
    }

    pub(crate) fn position(
        &self,
        indices: &[Operand],
        length: usize,
        values: &[Option<Value>],
    ) -> SelectionPosition {
        let axes = match self {
            Self::OneBased => vec![(1, length as i128)],
            Self::Offset { offset } => vec![(i128::from(*offset), length as i128)],
            Self::Matrix { rows, columns } => [rows, columns]
                .map(|set| {
                    let (lower, upper) =
                        set.contiguous_bounds().expect("admitted matrix index sets");
                    (i128::from(lower), i128::from(upper) - i128::from(lower) + 1)
                })
                .to_vec(),
            Self::SetMatrix { width, height } => {
                vec![(0, i128::from(*height)), (0, i128::from(*width))]
            }
        };
        let mut has_missing_index = false;
        let mut position = 0;
        for (index, (lower, size)) in indices.iter().zip(axes) {
            let Some(value) = index.value(values) else {
                has_missing_index = true;
                continue;
            };
            let relative = i128::from(value.integer()) - lower;
            if relative < 0 || relative >= size {
                return SelectionPosition::OutOfRange;
            }
            // Admission checks the rectangular product; each axis is checked before flattening.
            position = position * size as usize + relative as usize;
        }
        if has_missing_index {
            SelectionPosition::Missing
        } else {
            SelectionPosition::Selected(position)
        }
    }
}

impl Predicate {
    pub(crate) fn selected_operand<'a>(&'a self, values: &[Option<Value>]) -> Option<&'a Operand> {
        match self {
            Self::Element {
                indices,
                elements,
                layout,
                ..
            } => match layout.position(indices, elements.len(), values) {
                SelectionPosition::Selected(position) => Some(&elements[position]),
                _ => None,
            },
            Self::Conditional {
                condition,
                then_value,
                else_value,
                ..
            } => Some(if condition.value(values)?.boolean() {
                then_value
            } else {
                else_value
            }),
            _ => None,
        }
    }
}
