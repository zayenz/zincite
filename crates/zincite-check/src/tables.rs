//! Exact table signatures, row-major membership and Boolean completion.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TableName {
    pub boolean: bool,
    pub mode: RelationMode,
}

impl TableName {
    pub fn from_name(name: &str) -> Option<Self> {
        for base in [
            name,
            name.strip_prefix("gecode_").unwrap_or(""),
            name.strip_prefix("fzn_").unwrap_or(""),
        ] {
            let (boolean, mode) = match base {
                "gecode_table_int" => (false, RelationMode::Plain),
                "gecode_table_int_reif" => (false, RelationMode::Reified),
                "gecode_table_int_imp" => (false, RelationMode::Implied),
                "gecode_table_bool" => (true, RelationMode::Plain),
                "gecode_table_bool_reif" => (true, RelationMode::Reified),
                "gecode_table_bool_imp" => (true, RelationMode::Implied),
                _ => continue,
            };
            return Some(Self { boolean, mode });
        }
        None
    }

    pub fn kind(self) -> ValueType {
        if self.boolean {
            ValueType::Boolean
        } else {
            ValueType::Integer
        }
    }

    #[cfg(feature = "prepare")]
    pub fn signature(self) -> Vec<prepare::ArgumentType> {
        let mut args = vec![
            prepare::ArgumentType {
                kind: self.kind(),
                is_var: true,
                is_array: true,
            },
            prepare::ArgumentType {
                kind: self.kind(),
                is_var: false,
                is_array: true,
            },
        ];
        if self.mode != RelationMode::Plain {
            args.push(prepare::ArgumentType {
                kind: ValueType::Boolean,
                is_var: true,
                is_array: false,
            });
        }
        args
    }
}

/// Fixed row-major table values. The candidate tuple supplies the row width.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "values", rename_all = "snake_case")]
pub enum TableData {
    Integer(
        #[serde(
            serialize_with = "serialize_integers",
            deserialize_with = "deserialize_integers"
        )]
        Vec<i64>,
    ),
    Boolean(Vec<bool>),
}

impl TableData {
    fn len(&self) -> usize {
        match self {
            Self::Integer(values) => values.len(),
            Self::Boolean(values) => values.len(),
        }
    }

    pub(crate) fn kind(&self) -> ValueType {
        match self {
            Self::Integer(_) => ValueType::Integer,
            Self::Boolean(_) => ValueType::Boolean,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Table {
    pub tuple: Vec<Operand>,
    pub table: TableData,
    pub condition: RelationCondition,
}

impl Table {
    pub fn operands(&self) -> Vec<&Operand> {
        self.tuple.iter().chain(self.condition.operand()).collect()
    }

    pub fn validate_shape(&self) -> Result<(), &'static str> {
        let width = self.tuple.len();
        if (width == 0 && self.table.len() != 0)
            || (width != 0 && !self.table.len().is_multiple_of(width))
        {
            return Err("table data must contain complete rows of the candidate tuple width");
        }
        Ok(())
    }

    pub fn membership(&self, values: &[Option<Value>]) -> Option<bool> {
        // Resolve the whole tuple even when the table has no rows.
        let tuple: Vec<_> = self
            .tuple
            .iter()
            .map(|entry| entry.value(values))
            .collect::<Option<_>>()?;
        if tuple.is_empty() {
            return Some(false);
        }
        Some(match &self.table {
            TableData::Integer(table) => table.chunks_exact(tuple.len()).any(|row| {
                row.iter()
                    .zip(&tuple)
                    .all(|(entry, value)| *entry == value.integer())
            }),
            TableData::Boolean(table) => table.chunks_exact(tuple.len()).any(|row| {
                row.iter()
                    .zip(&tuple)
                    .all(|(entry, value)| value.boolean() == *entry)
            }),
        })
    }

    pub fn can_define(&self, output: usize) -> bool {
        matches!(&self.condition, RelationCondition::Reified { result: Operand::Variable(slot) } if *slot == output)
            && !self
                .tuple
                .iter()
                .any(|entry| matches!(entry, Operand::Variable(slot) if *slot == output))
    }
}

impl PreparedModel {
    pub(crate) fn evaluate_table(
        &self,
        table: &Table,
        values: &[Option<Value>],
    ) -> Option<ViolationKind> {
        let membership = table.membership(values).expect("available table tuple");
        if table.condition.satisfied(membership, values) {
            return None;
        }
        Some(ViolationKind::Table {
            tuple: table
                .tuple
                .iter()
                .map(|entry| self.argument(entry, values).expect("available table entry"))
                .collect(),
            table: table.table.clone(),
            width: table.tuple.len(),
            membership,
            condition: self.condition_value(&table.condition, values),
        })
    }
}
