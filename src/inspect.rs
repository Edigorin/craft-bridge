//! Inspect: what an artifact contains. JSON in, summary out; nothing in the
//! artifact is executed.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::profile::{self, Source, WarningsCapture, RECIPE, STEP};
use crate::recipe::{self, Parameters, StepSummary};
use crate::zipio;

/// The machine-readable summary of an artifact. Later stages of the bridge
/// consume this shape; it is pinned by golden tests.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InspectReport {
    pub artifact: String,
    pub profile: String,
    pub container: String,
    pub name: String,
    pub created_by: String,
    pub source: Source,
    pub warnings: WarningsCapture,
    pub fallback_mode: String,
    pub steps: Vec<StepSummary>,
    pub parameters: Parameters,
    pub geometry: GeometryView,
    pub members: Vec<MemberView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeometryView {
    pub member: String,
    pub bytes: u64,
    pub schema: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemberView {
    pub path: String,
    pub bytes: u64,
}

/// Inspect an artifact with the given limits.
pub fn inspect(path: &Path, limits: &Limits) -> Result<InspectReport> {
    let artifact_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let mut archive = zipio::open_archive(path, limits)?;
    let mut budget = limits.max_total_bytes;

    let manifest_bytes = zipio::read_member(&mut archive, profile::MANIFEST, limits, &mut budget)?;
    let manifest = profile::parse_manifest(&manifest_bytes, limits)?;

    let recipe_bytes = zipio::read_member(&mut archive, RECIPE, limits, &mut budget)?;
    let recipe_value = recipe::validate(&recipe_bytes, limits)?;
    let (steps, parameters) = recipe::summarize(&recipe_value);

    let step_bytes = zipio::read_member(&mut archive, STEP, limits, &mut budget)?;
    let schema = step_schema(&step_bytes);

    // The inventory must match the payload members exactly.
    for entry in &manifest.members {
        let actual = match entry.path.as_str() {
            RECIPE => recipe_bytes.len() as u64,
            STEP => step_bytes.len() as u64,
            other => {
                return Err(Error::InvalidArtifact(format!(
                    "inventory lists {other}, which this profile does not carry"
                )))
            }
        };
        if actual != entry.bytes {
            return Err(Error::InvalidArtifact(format!(
                "inventory mismatch for {}: manifest says {} bytes, member is {actual}",
                entry.path, entry.bytes
            )));
        }
    }

    Ok(InspectReport {
        artifact: artifact_name,
        profile: manifest.profile,
        container: manifest.container,
        name: manifest.name,
        created_by: manifest.created_by,
        source: manifest.source,
        warnings: manifest.warnings,
        fallback_mode: manifest.fallback_mode,
        steps,
        parameters,
        geometry: GeometryView {
            member: STEP.to_string(),
            bytes: step_bytes.len() as u64,
            schema,
        },
        members: vec![
            MemberView {
                path: RECIPE.to_string(),
                bytes: recipe_bytes.len() as u64,
            },
            MemberView {
                path: STEP.to_string(),
                bytes: step_bytes.len() as u64,
            },
        ],
    })
}

/// The STEP schema identifier: the first quoted string on the FILE_SCHEMA
/// line, when present.
fn step_schema(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let line = text.lines().find(|l| l.contains("FILE_SCHEMA"))?;
    let rest = &line[line.find("FILE_SCHEMA")? + "FILE_SCHEMA".len()..];
    let start = rest.find('\'')? + 1;
    let end = start + rest[start..].find('\'')?;
    let inner = &rest[start..end];
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

/// Human-readable rendering of a report. Pinned by golden tests.
pub fn render_human(report: &InspectReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("craftx artifact: {}\n", report.artifact));
    out.push_str(&format!(
        "profile: {}   container: {}\n",
        report.profile, report.container
    ));
    out.push_str(&format!(
        "name: {}   created by: {}\n",
        report.name, report.created_by
    ));
    let warnings_file = report.source.warnings_file.as_deref().unwrap_or("(none)");
    out.push_str(&format!(
        "source: {} + {}   warnings sidecar: {}\n",
        report.source.recipe_file, report.source.step_file, warnings_file
    ));
    out.push_str(&format!(
        "omission warnings: {}\n",
        report.warnings.describe()
    ));
    if !report.warnings.items.is_empty() {
        for item in &report.warnings.items {
            out.push_str(&format!("  - {item}\n"));
        }
    }
    out.push_str(&format!("fallback: {}\n", report.fallback_mode));
    out.push_str(&format!("\nsteps ({}):\n", report.steps.len()));
    for step in &report.steps {
        match &step.detail {
            Some(detail) => out.push_str(&format!(
                "  {:>3}  {}   {}\n",
                step.index, step.command, detail
            )),
            None => out.push_str(&format!("  {:>3}  {}\n", step.index, step.command)),
        }
    }
    out.push_str(&format!(
        "\nparameters ({} user, {} model):\n",
        report.parameters.user.len(),
        report.parameters.model.len()
    ));
    for param in &report.parameters.user {
        out.push_str(&format!(
            "  user   {} = {}\n",
            param.name,
            compact(&param.raw)
        ));
    }
    for param in &report.parameters.model {
        out.push_str(&format!(
            "  model  {} = {}\n",
            param.name,
            compact(&param.raw)
        ));
    }
    match &report.geometry.schema {
        Some(schema) => out.push_str(&format!(
            "\ngeometry: {} ({} bytes, schema {})\n",
            report.geometry.member, report.geometry.bytes, schema
        )),
        None => out.push_str(&format!(
            "\ngeometry: {} ({} bytes, no FILE_SCHEMA line)\n",
            report.geometry.member, report.geometry.bytes
        )),
    }
    out.push_str("members:\n");
    for member in &report.members {
        out.push_str(&format!("  {}  {} bytes\n", member.path, member.bytes));
    }
    out
}

fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unreadable>".to_string())
}
