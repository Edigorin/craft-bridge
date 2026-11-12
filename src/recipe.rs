//! Recipe handling: the SolidCraft script JSON.
//!
//! A recipe is data. Nothing here executes a command; unknown commands are
//! preserved and summarised so a later stage can turn them into fallback
//! bodies instead of guessing at them.

use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::limits::Limits;

/// Parse and structurally validate a recipe body.
///
/// The shape is the SolidCraft script format: an object with a `commands`
/// array whose entries carry a `command` id and an optional `params` object.
/// Unknown commands are fine here; translation allowlisting is the
/// generator's job.
pub fn validate(bytes: &[u8], limits: &Limits) -> Result<Value> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| Error::json("recipe.json", e))?;
    let commands = commands_of(&value)?;
    if commands.len() > limits.max_commands {
        return Err(Error::limit(
            "recipe command count",
            limits.max_commands as u64,
            commands.len() as u64,
        ));
    }
    for (index, entry) in commands.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| Error::InvalidRecipe(format!("command {index} is not an object")))?;
        match object.get("command") {
            Some(Value::String(id)) if !id.is_empty() => {}
            _ => {
                return Err(Error::InvalidRecipe(format!(
                    "command {index} has no command id"
                )))
            }
        }
        if let Some(params) = object.get("params") {
            if !params.is_object() {
                return Err(Error::InvalidRecipe(format!(
                    "command {index} params are not an object"
                )));
            }
        }
    }
    Ok(value)
}

/// The `commands` array of a validated recipe.
pub fn commands_of(value: &Value) -> Result<&Vec<Value>> {
    let root = value
        .as_object()
        .ok_or_else(|| Error::InvalidRecipe("root is not an object".to_string()))?;
    let commands = root
        .get("commands")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::InvalidRecipe("no commands array".to_string()))?;
    Ok(commands)
}

/// One recipe step, for display.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepSummary {
    pub index: usize,
    pub command: String,
    pub detail: Option<String>,
}

/// One design parameter, preserved verbatim so expressions and units survive.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParamSummary {
    pub name: String,
    pub raw: Value,
}

/// The design parameters a recipe carries: user parameters from
/// `parameters.add`, model parameters from inside `timeline.add`.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Parameters {
    pub user: Vec<ParamSummary>,
    pub model: Vec<ParamSummary>,
}

/// Summarise a validated recipe: its steps and its design parameters.
/// This reads JSON only; no command is interpreted or run.
pub fn summarize(value: &Value) -> (Vec<StepSummary>, Parameters) {
    let mut steps = Vec::new();
    let mut parameters = Parameters::default();
    let no_commands: &[Value] = &[];
    let commands = match commands_of(value) {
        Ok(commands) => commands.as_slice(),
        Err(_) => no_commands,
    };
    for (index, entry) in commands.iter().enumerate() {
        let object = entry.as_object();
        let id = object
            .and_then(|o| o.get("command"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let params = object
            .and_then(|o| o.get("params"))
            .and_then(Value::as_object);
        let detail = params
            .and_then(|p| p.get("feature"))
            .and_then(Value::as_object)
            .and_then(|f| f.get("type"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                params
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
        match id.as_str() {
            "parameters.add" => {
                let name = params
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("(unnamed)")
                    .to_string();
                parameters.user.push(ParamSummary {
                    name,
                    raw: entry.get("params").cloned().unwrap_or(Value::Null),
                });
            }
            "timeline.add" => {
                if let Some(model) = params.and_then(|p| p.get("params")) {
                    push_model_params(&mut parameters.model, model);
                }
            }
            _ => {}
        }
        steps.push(StepSummary {
            index,
            command: id,
            detail,
        });
    }
    (steps, parameters)
}

fn push_model_params(out: &mut Vec<ParamSummary>, model: &Value) {
    match model {
        Value::Object(map) => {
            for (name, raw) in map {
                out.push(ParamSummary {
                    name: name.clone(),
                    raw: raw.clone(),
                });
            }
        }
        Value::Array(items) => {
            for item in items {
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("(unnamed)")
                    .to_string();
                out.push(ParamSummary {
                    name,
                    raw: item.clone(),
                });
            }
        }
        _ => {}
    }
}
