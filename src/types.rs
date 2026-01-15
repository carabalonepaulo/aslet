use godot::{
    builtin::{Array, PackedByteArray, Variant, VariantArray, VariantType},
    meta::ToGodot,
    prelude::*,
};
use rusqlite::{
    ToSql,
    types::{ToSqlOutput, ValueRef},
};

use crate::error::InternalError;

#[derive(Debug)]
pub struct Columns(Vec<String>);

impl From<Vec<String>> for Columns {
    fn from(value: Vec<String>) -> Self {
        Self(value)
    }
}

impl GodotConvert for Columns {
    type Via = PackedStringArray;
}

impl ToGodot for Columns {
    type ToVia<'v> = PackedStringArray;

    fn to_godot(&self) -> Self::ToVia<'_> {
        let mut array = PackedStringArray::new();
        array.resize(self.0.len());
        for (i, value) in self.0.iter().enumerate() {
            array[i] = value.to_godot();
        }
        array
    }
}

#[derive(Debug)]
pub enum Value {
    Int(i64),
    Number(f64),
    String(String),
    Blob(Vec<u8>),
    Null,
}

impl GodotConvert for Value {
    type Via = Variant;
}

impl ToGodot for Value {
    type ToVia<'v> = Variant;

    fn to_godot(&self) -> Self::ToVia<'_> {
        match self {
            Value::Int(v) => Variant::from(*v),
            Value::Number(v) => Variant::from(*v),
            Value::String(v) => Variant::from(v.as_str()),
            Value::Blob(v) => Variant::from(PackedByteArray::from(v.as_slice())),
            Value::Null => Variant::nil(),
        }
    }
}

impl ToSql for Value {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(match self {
            Value::Int(v) => ToSqlOutput::from(*v),
            Value::Number(v) => ToSqlOutput::from(*v),
            Value::String(v) => ToSqlOutput::from(v.as_str()),
            Value::Blob(v) => ToSqlOutput::from(v.as_slice()),
            Value::Null => ToSqlOutput::Owned(rusqlite::types::Value::Null),
        })
    }
}

impl<'a> From<ValueRef<'a>> for Value {
    fn from(value: ValueRef<'a>) -> Self {
        match value {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(v) => Value::Int(v),
            ValueRef::Real(v) => Value::Number(v),
            ValueRef::Text(v) => Value::String(String::from_utf8_lossy(v).into_owned()),
            ValueRef::Blob(v) => Value::Blob(v.to_vec()),
        }
    }
}

impl TryFrom<Variant> for Value {
    type Error = InternalError;

    fn try_from(value: Variant) -> Result<Self, Self::Error> {
        match value.get_type() {
            VariantType::INT => Ok(Value::Int(value.to())),
            VariantType::FLOAT => Ok(Value::Number(value.to())),
            VariantType::STRING => Ok(Value::String(value.to())),
            VariantType::PACKED_BYTE_ARRAY => Ok(Value::Blob(value.to())),
            VariantType::NIL => Ok(Value::Null),
            ty => Err(InternalError::UnsupportedVariantType(ty)),
        }
    }
}

#[derive(Debug)]
pub struct Row(Vec<Value>);

impl AsRef<[Value]> for Row {
    fn as_ref(&self) -> &[Value] {
        &self.0
    }
}

impl GodotConvert for Row {
    type Via = Array<Variant>;
}

impl ToGodot for Row {
    type ToVia<'v> = Array<Variant>;

    fn to_godot(&self) -> Self::ToVia<'_> {
        let mut row = VariantArray::new();
        for value in self.0.iter() {
            row.push(&value.to_variant());
        }
        row
    }
}

impl From<Array<Variant>> for Row {
    fn from(array: Array<Variant>) -> Self {
        let values = array
            .iter_shared()
            .filter_map(|v| Value::try_from(v).ok())
            .collect();
        Self(values)
    }
}

impl From<Vec<Value>> for Row {
    fn from(value: Vec<Value>) -> Self {
        Self(value)
    }
}

impl<'a> From<&'a rusqlite::Row<'_>> for Row {
    fn from(value: &'a rusqlite::Row) -> Self {
        Row((0..value.as_ref().column_count())
            .map(|i| Value::from(value.get_ref(i).unwrap()))
            .collect())
    }
}

#[derive(Debug)]
pub struct Rows(Vec<Row>);

impl AsRef<[Row]> for Rows {
    fn as_ref(&self) -> &[Row] {
        &self.0
    }
}

impl GodotConvert for Rows {
    type Via = Array<Variant>;
}

impl ToGodot for Rows {
    type ToVia<'v> = Array<Variant>;

    fn to_godot(&self) -> Self::ToVia<'_> {
        let mut rows = VariantArray::new();
        for row in self.0.iter() {
            rows.push(&row.to_variant());
        }
        rows
    }
}

impl From<Vec<Vec<Value>>> for Rows {
    fn from(value: Vec<Vec<Value>>) -> Self {
        Self(value.into_iter().map(Row).collect())
    }
}

impl From<Vec<Row>> for Rows {
    fn from(value: Vec<Row>) -> Self {
        Self(value)
    }
}

impl From<Array<Array<Variant>>> for Rows {
    fn from(rows: Array<Array<Variant>>) -> Self {
        Self(rows.iter_shared().map(Row::from).collect())
    }
}
