use std::collections::BTreeMap;

use anyhow::Result;
use mlua::{Table, Value as LuaValue};
use serde_json::{Map, Value};

use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) struct OutputDeclaration {
    pub(crate) name: String,
    pub(crate) value: Value,
}

pub(crate) fn output_value_from_lua(value: LuaValue) -> mlua::Result<Value> {
    json_from_lua(value)
}

fn json_from_lua(value: LuaValue) -> mlua::Result<Value> {
    match value {
        LuaValue::Nil => Ok(Value::Null),
        LuaValue::Boolean(value) => Ok(Value::Bool(value)),
        LuaValue::Integer(value) => Ok(Value::Number(value.into())),
        LuaValue::Number(value) => serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| mlua::Error::RuntimeError("output numbers must be finite".to_string())),
        LuaValue::String(value) => Ok(Value::String(value.to_string_lossy())),
        LuaValue::Table(table) => json_from_table(table),
        _ => Err(mlua::Error::RuntimeError(
            "output values must be JSON-compatible".to_string(),
        )),
    }
}

fn json_from_table(table: Table) -> mlua::Result<Value> {
    let mut entries = Vec::new();
    for pair in table.pairs::<LuaValue, LuaValue>() {
        entries.push(pair?);
    }
    if entries.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }

    let array = entries
        .iter()
        .all(|(key, _)| matches!(key, LuaValue::Integer(index) if *index > 0));
    if array {
        let mut values = entries
            .into_iter()
            .map(|(key, value)| {
                let LuaValue::Integer(index) = key else {
                    unreachable!()
                };
                Ok((index, json_from_lua(value)?))
            })
            .collect::<mlua::Result<Vec<_>>>()?;
        values.sort_by_key(|(index, _)| *index);
        if values
            .iter()
            .enumerate()
            .any(|(offset, (index, _))| *index != offset as i64 + 1)
        {
            return Err(mlua::Error::RuntimeError(
                "output arrays must have contiguous indexes".to_string(),
            ));
        }
        return Ok(Value::Array(
            values.into_iter().map(|(_, value)| value).collect(),
        ));
    }

    let mut object = Map::new();
    for (key, value) in entries {
        let LuaValue::String(key) = key else {
            return Err(mlua::Error::RuntimeError(
                "output objects must have string keys".to_string(),
            ));
        };
        object.insert(key.to_string_lossy(), json_from_lua(value)?);
    }
    Ok(Value::Object(object))
}

pub(crate) fn sync_outputs(declarations: &[OutputDeclaration], state: &mut State) -> Result<()> {
    let mut outputs = BTreeMap::new();
    for declaration in declarations {
        outputs.insert(declaration.name.clone(), declaration.value.clone());
    }
    state.outputs = outputs;
    Ok(())
}
