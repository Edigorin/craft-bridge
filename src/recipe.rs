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
    match shape_of(&value)? {
        Shape::Script => validate_script(&value, limits)?,
        Shape::Design57 => validate_57(&value, limits)?,
    }
    Ok(value)
}

/// The legacy script contract: a `commands` array of id-bearing steps.
fn validate_script(value: &Value, limits: &Limits) -> Result<()> {
    let commands = commands_of(value)?;
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
    Ok(())
}

/// The solidcraft/57 contract: `features` and `params` at the root.
///
/// Feature ids are source identity only (unique here; sparse or huge values
/// are fine because the runtime index is dense and separate). Parameter
/// names must be unique: they become global-variable equations. Bounds are
/// checked here so malformed or oversized designs refuse before planning.
fn validate_57(value: &Value, limits: &Limits) -> Result<()> {
    let root = value.as_object().unwrap();
    // The source caps feature ids at 2^52; a rollback marker is a timeline
    // cursor meaning features past it are not part of the evaluated model.
    // Its index base is unpinned, so a marker that would change behavior
    // refuses named instead of guessing which features the STEP carries.
    let feature_count = features_of(value)?.len();
    // Single-part boundary (docs/spec.md): document structure that can
    // affect PLACED geometry is outside v1. Present-and-nonempty refuses
    // named instead of being silently ignored.
    for field in [
        "components",
        "occurrences",
        "assembly",
        "parts",
        "links",
        "body_components",
        "body_offsets",
        // Evaluated state, not metadata (nested-schema research T4):
        // configuration rows override parameters and suppression, rules
        // evaluate at runtime, and point clouds may be feature inputs.
        "configs",
        "point_clouds",
    ] {
        let occupied = match root.get(field) {
            None | Some(Value::Null) => false,
            Some(Value::Array(a)) => !a.is_empty(),
            Some(Value::Object(o)) => !o.is_empty(),
            Some(Value::Bool(b)) => *b,
            Some(_) => true,
        };
        if occupied {
            return Err(Error::InvalidRecipe(format!(
                "the design carries evaluated state outside the v1 replay boundary ({field} is present); components, occurrences, parts, links, body placement, configurations and point clouds change what the evaluated model is"
            )));
        }
    }
    // The marker is source-pinned: a zero-based index into features; the
    // feature AT that index and every feature after it are rolled back and
    // not evaluated (nested-schema research A6). Planning implements the
    // cutoff; only a malformed marker refuses here.
    match root.get("marker") {
        None | Some(Value::Null) => {}
        Some(Value::Number(n)) => {
            if n.as_u64().is_none() {
                return Err(Error::InvalidRecipe(
                    "the timeline rollback marker is not a non-negative whole number".to_string(),
                ));
            }
            let _ = feature_count;
        }
        Some(_) => {
            return Err(Error::InvalidRecipe(
                "the timeline rollback marker is not a number".to_string(),
            ))
        }
    }
    // params: absent is fine (no parameters); present it must be an array.
    match root.get("params") {
        None | Some(Value::Null) | Some(Value::Array(_)) => {}
        Some(_) => {
            return Err(Error::InvalidRecipe(
                "params is present but not an array".to_string(),
            ))
        }
    }
    match root.get("units") {
        None | Some(Value::Null) => {}
        Some(Value::String(u)) if matches!(u.as_str(), "mm" | "cm" | "m" | "in" | "ft") => {}
        Some(Value::String(u)) => {
            return Err(Error::InvalidRecipe(format!(
                "design units {u:?} are outside the supported set (mm, cm, m, in, ft)"
            )))
        }
        Some(_) => {
            return Err(Error::InvalidRecipe(
                "design units are not a string".to_string(),
            ))
        }
    }
    let features = features_of(value)?;
    if features.len() > limits.max_commands {
        return Err(Error::limit(
            "recipe feature count",
            limits.max_commands as u64,
            features.len() as u64,
        ));
    }
    let mut seen_ids: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    for (index, entry) in features.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| Error::InvalidRecipe(format!("feature {index} is not an object")))?;
        match object.get("type") {
            Some(Value::String(t)) if !t.is_empty() => {}
            _ => {
                return Err(Error::InvalidRecipe(format!(
                    "feature {index} has no feature type"
                )))
            }
        }
        match object.get("suppressed") {
            None | Some(Value::Null) | Some(Value::Bool(_)) => {}
            Some(_) => {
                return Err(Error::InvalidRecipe(format!(
                    "feature {index} suppressed is present but not a boolean"
                )))
            }
        }
        match object.get("id") {
            Some(Value::Number(n)) => {
                let id = n.as_u64().ok_or_else(|| {
                    Error::InvalidRecipe(format!("feature {index} id is not a whole number"))
                })?;
                if id >= (1u64 << 52) {
                    return Err(Error::InvalidRecipe(format!(
                        "feature id {id} is at or above the source limit of 2^52"
                    )));
                }
                if !seen_ids.insert(id) {
                    return Err(Error::InvalidRecipe(format!(
                        "feature id {id} is not unique; source ids must resolve unambiguously"
                    )));
                }
            }
            _ => {
                return Err(Error::InvalidRecipe(format!(
                    "feature {index} has no numeric id"
                )))
            }
        }
    }
    let params = params57_of(value)?;
    if params.len() > limits.max_commands {
        return Err(Error::limit(
            "recipe parameter count",
            limits.max_commands as u64,
            params.len() as u64,
        ));
    }
    let mut seen_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (index, entry) in params.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| Error::InvalidRecipe(format!("parameter {index} is not an object")))?;
        let name = match object.get("name") {
            Some(Value::String(n)) if !n.is_empty() => n.clone(),
            _ => {
                return Err(Error::InvalidRecipe(format!(
                    "parameter {index} has no name"
                )))
            }
        };
        match object.get("expr") {
            Some(Value::String(e)) if !e.is_empty() => {}
            _ => {
                return Err(Error::InvalidRecipe(format!(
                    "parameter {name} has no expression"
                )))
            }
        }
        if let Some(u) = object.get("unit") {
            if !u.is_string() {
                return Err(Error::InvalidRecipe(format!(
                    "parameter {name} unit is not a string"
                )));
            }
        }
        if !seen_names.insert(name.clone()) {
            return Err(Error::InvalidRecipe(format!(
                "parameter name {name} is not unique"
            )));
        }
    }
    Ok(())
}

/// The `commands` array of a validated script recipe.
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

/// Which recipe shape a validated recipe uses.
///
/// Source identity, dense runtime indices and snapshot addressing are three
/// different things (docs/replay-profile.md): the shape decides only where
/// the step entries live. Runtime indices are always dense 0..n over
/// `entries_of` order (parameters first for a 57 design), and snapshot
/// boundaries address those runtime indices - never source feature ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Legacy SolidCraft script: a `commands` array; feature payloads ride
    /// inside `timeline.add` steps. A missing `format` field means this.
    Script,
    /// `solidcraft/57`: `features` and `params` at the root. The 57 feature
    /// payload is shape-identical to the script's `feature` object, so both
    /// feed the same typed replay planning.
    Design57,
}

/// The recipe shape of a validated recipe.
pub fn shape_of(value: &Value) -> Result<Shape> {
    let root = value
        .as_object()
        .ok_or_else(|| Error::InvalidRecipe("root is not an object".to_string()))?;
    match root.get("format") {
        None => Ok(Shape::Script),
        Some(Value::String(f)) if f == "solidcraft/57" => Ok(Shape::Design57),
        Some(Value::String(f)) => Err(Error::InvalidRecipe(format!(
            "unknown recipe format {f:?} (expected \"solidcraft/57\" or no format field)"
        ))),
        Some(_) => Err(Error::InvalidRecipe(
            "format field is not a string".to_string(),
        )),
    }
}

/// The `features` array of a validated solidcraft/57 recipe.
pub fn features_of(value: &Value) -> Result<&Vec<Value>> {
    let root = value
        .as_object()
        .ok_or_else(|| Error::InvalidRecipe("root is not an object".to_string()))?;
    root.get("features")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::InvalidRecipe("no features array".to_string()))
}

/// The `params` array of a validated solidcraft/57 recipe (may be absent).
pub fn params57_of(value: &Value) -> Result<&[Value]> {
    let root = value
        .as_object()
        .ok_or_else(|| Error::InvalidRecipe("root is not an object".to_string()))?;
    Ok(root
        .get("params")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]))
}

/// The ordered step entries of a validated recipe, in runtime order. For a
/// script these are the `commands`; for a 57 design the parameter equations
/// come first (they must exist before any feature references them), then
/// the features in file order.
pub fn entries_of(value: &Value) -> Result<Vec<Value>> {
    match shape_of(value)? {
        Shape::Script => Ok(commands_of(value)?.clone()),
        Shape::Design57 => {
            let mut out = Vec::new();
            for p in params57_of(value)? {
                out.push(p.clone());
            }
            for f in features_of(value)? {
                out.push(f.clone());
            }
            Ok(out)
        }
    }
}

/// How many runtime steps a validated recipe plans to (the addressable
/// count for snapshot boundaries).
pub fn step_count(value: &Value) -> Result<usize> {
    match shape_of(value)? {
        Shape::Script => Ok(commands_of(value)?.len()),
        Shape::Design57 => Ok(params57_of(value)?.len() + features_of(value)?.len()),
    }
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
    if matches!(shape_of(value), Ok(Shape::Design57)) {
        // 57: parameters are runtime steps first (they are equations the
        // features reference), then features in file order - the same order
        // the planner gives them their dense runtime indices in.
        for p in params57_of(value).unwrap_or(&[]) {
            let name = p
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("(unnamed)")
                .to_string();
            let is_model = p.get("model").and_then(Value::as_bool).unwrap_or(false);
            let summary = ParamSummary {
                name: name.clone(),
                raw: p.clone(),
            };
            if is_model {
                parameters.model.push(summary);
            } else {
                parameters.user.push(summary);
            }
            steps.push(StepSummary {
                index: steps.len(),
                command: "parameter".to_string(),
                detail: Some(name),
            });
        }
        for f in features_of(value).unwrap_or(&Vec::new()) {
            let kind = f
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("(unnamed)")
                .to_string();
            let detail = f
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| kind.clone());
            steps.push(StepSummary {
                index: steps.len(),
                command: kind,
                detail: Some(detail),
            });
        }
        return (steps, parameters);
    }
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
