//! Rectangle dimensions, optional presence and endpoint separation.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RectangleName {
    Mandatory,
    Optional,
    FlexOptional,
}
impl RectangleName {
    pub fn from_name(name: &str) -> Option<Self> {
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            match base {
                "gecode_nooverlap" | "gecode_nooverlap_fzn" => return Some(Self::Mandatory),
                "gecode_nooverlap_optional" => return Some(Self::Optional),
                "gecode_nooverlap_flex_optional" => return Some(Self::FlexOptional),
                _ => {}
            }
        }
        None
    }
    #[cfg(feature = "prepare")]
    pub fn arity(self) -> usize {
        match self {
            Self::Mandatory => 4,
            Self::Optional => 5,
            Self::FlexOptional => 7,
        }
    }
    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<prepare::ArgumentType> {
        let mut args = vec![
            prepare::ArgumentType {
                kind: ValueType::Integer,
                is_array: true,
                is_var: true
            };
            self.arity()
        ];
        if self == Self::Optional {
            args[1].is_var = false;
            args[3].is_var = false;
        }
        if matches!(self, Self::Optional | Self::FlexOptional) {
            args.last_mut().unwrap().kind = ValueType::Boolean;
        }
        args
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Rectangles {
    pub name: RectangleName,
    pub x: Vec<Operand>,
    pub width: Vec<Operand>,
    pub right: Option<Vec<Operand>>,
    pub y: Vec<Operand>,
    pub height: Vec<Operand>,
    pub top: Option<Vec<Operand>>,
    pub present: Option<Vec<Operand>>,
}
#[derive(Clone, Debug, Serialize)]
pub struct RectangleArguments {
    pub position: usize,
    pub x: Option<ArgumentValue>,
    pub width: Option<ArgumentValue>,
    pub right: Option<ArgumentValue>,
    pub y: Option<ArgumentValue>,
    pub height: Option<ArgumentValue>,
    pub top: Option<ArgumentValue>,
    pub present: Option<ArgumentValue>,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RectangleDimension {
    Width,
    Height,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RectangleFault {
    Dimension {
        position: usize,
        dimension: RectangleDimension,
        #[serde(serialize_with = "serialize_integer")]
        value: i64,
    },
    Conflict {
        first: usize,
        second: usize,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct RectanglesFinding {
    pub rectangles: Vec<RectangleArguments>,
    pub faults: Vec<RectangleFault>,
}
struct RectangleBounds {
    x: i64,
    right: i64,
    y: i64,
    top: i64,
}

impl Rectangles {
    pub fn integer_operands(&self) -> Vec<&Operand> {
        self.x
            .iter()
            .chain(&self.width)
            .chain(self.right.iter().flatten())
            .chain(&self.y)
            .chain(&self.height)
            .chain(self.top.iter().flatten())
            .collect()
    }
    pub fn operands(&self) -> Vec<&Operand> {
        let mut operands = self.integer_operands();
        operands.extend(self.present.iter().flatten());
        operands
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        let n = self.x.len();
        if self.width.len() != n
            || self.y.len() != n
            || self.height.len() != n
            || self.right.as_ref().is_some_and(|a| a.len() != n)
            || self.top.as_ref().is_some_and(|a| a.len() != n)
            || self.present.as_ref().is_some_and(|a| a.len() != n)
        {
            return Err("rectangle arrays must have equal lengths");
        }
        let flex = self.name == RectangleName::FlexOptional;
        let optional = matches!(
            self.name,
            RectangleName::Optional | RectangleName::FlexOptional
        );
        if self.right.is_some() != flex
            || self.top.is_some() != flex
            || self.present.is_some() != optional
        {
            return Err("rectangle fields do not match their interface");
        }
        if self.name == RectangleName::Optional && self.width.iter().chain(&self.height)
            .any(|operand| !matches!(operand, Operand::Literal(Value::Integer(value)) if *value >= 0)) {
            return Err("optional rectangle dimensions must be fixed nonnegative integers");
        }
        Ok(())
    }
    fn dimension_faults(&self, values: &[Option<Value>]) -> Vec<RectangleFault> {
        let mut faults = Vec::new();
        for i in 0..self.x.len() {
            for (dimension, operand) in [
                (RectangleDimension::Width, &self.width[i]),
                (RectangleDimension::Height, &self.height[i]),
            ] {
                if let Some(Value::Integer(value)) = operand.value(values)
                    && *value < 0
                {
                    faults.push(RectangleFault::Dimension {
                        position: i + 1,
                        dimension,
                        value: *value,
                    });
                }
            }
        }
        faults
    }
    fn relation_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        let mut operands: Vec<_> = self
            .width
            .iter()
            .chain(&self.height)
            .chain(self.present.iter().flatten())
            .collect();
        for i in 0..self.x.len() {
            if self
                .present
                .as_ref()
                .is_some_and(|a| a[i].value(values) == Some(&Value::Boolean(false)))
            {
                continue;
            }
            operands.extend([&self.x[i], &self.y[i]]);
            operands.extend(self.right.as_ref().map(|a| &a[i]));
            operands.extend(self.top.as_ref().map(|a| &a[i]));
        }
        operands
    }
    pub fn required_operands(&self, values: &[Option<Value>]) -> Vec<&Operand> {
        if self.dimension_faults(values).is_empty() {
            self.relation_operands(values)
        } else {
            Vec::new()
        }
    }
    fn bounds(
        &self,
        i: usize,
        values: &[Option<Value>],
        constraint: &Constraint,
    ) -> Result<Option<RectangleBounds>, CheckError> {
        if let Some(present) = &self.present
            && present[i].value(values) != Some(&Value::Boolean(true))
        {
            return Ok(None);
        }
        let integer = |operand: &Operand| operand.value(values).map(Value::integer);
        let (Some(x), Some(y), Some(width), Some(height)) = (
            integer(&self.x[i]),
            integer(&self.y[i]),
            integer(&self.width[i]),
            integer(&self.height[i]),
        ) else {
            return Ok(None);
        };
        if width < 0 || height < 0 {
            return Ok(None);
        }
        let endpoint = |coordinates: &Option<Vec<Operand>>, start: i64, size: i64| {
            if let Some(coordinates) = coordinates {
                Ok(integer(&coordinates[i]))
            } else {
                start.checked_add(size).map(Some).ok_or_else(|| {
                    constraint.execution_error("checked i64 overflow in rectangle endpoint")
                })
            }
        };
        let (Some(right), Some(top)) = (
            endpoint(&self.right, x, width)?,
            endpoint(&self.top, y, height)?,
        ) else {
            return Ok(None);
        };
        Ok(Some(RectangleBounds { x, right, y, top }))
    }
}
impl PreparedModel {
    pub(crate) fn evaluate_rectangles(
        &self,
        constraint: &Constraint,
        rectangles: &Rectangles,
        values: &[Option<Value>],
    ) -> Result<Option<ViolationKind>, CheckError> {
        let mut faults = rectangles.dimension_faults(values);
        let bounds = (0..rectangles.x.len())
            .map(|i| rectangles.bounds(i, values, constraint))
            .collect::<Result<Vec<_>, _>>()?;
        for (first, a) in bounds.iter().enumerate() {
            let Some(a) = a else {
                continue;
            };
            for (second, b) in bounds.iter().enumerate().skip(first + 1) {
                if let Some(b) = b
                    && !(a.right <= b.x || b.right <= a.x || a.top <= b.y || b.top <= a.y)
                {
                    faults.push(RectangleFault::Conflict {
                        first: first + 1,
                        second: second + 1,
                    });
                }
            }
        }
        Ok((!faults.is_empty()).then(|| ViolationKind::Rectangles {
            finding: RectanglesFinding {
                rectangles: (0..rectangles.x.len())
                    .map(|i| RectangleArguments {
                        position: i + 1,
                        x: self.argument(&rectangles.x[i], values),
                        width: self.argument(&rectangles.width[i], values),
                        right: rectangles
                            .right
                            .as_ref()
                            .and_then(|a| self.argument(&a[i], values)),
                        y: self.argument(&rectangles.y[i], values),
                        height: self.argument(&rectangles.height[i], values),
                        top: rectangles
                            .top
                            .as_ref()
                            .and_then(|a| self.argument(&a[i], values)),
                        present: rectangles
                            .present
                            .as_ref()
                            .and_then(|a| self.argument(&a[i], values)),
                    })
                    .collect(),
                faults,
            },
        }))
    }
}
