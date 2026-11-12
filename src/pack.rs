//! Packing: a SolidCraft script JSON plus STEP fallback geometry into a
//! craftx artifact under the replay profile.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::profile::{
    Manifest, MemberEntry, Source, WarningsCapture, CONTAINER_ID, FALLBACK_MODE, MANIFEST,
    PROFILE_ID, RECIPE, STEP,
};
use crate::recipe;
use crate::zipio;

/// Where to pack from and to.
pub struct PackOptions<'a> {
    pub recipe: &'a Path,
    pub step: &'a Path,
    pub out: &'a Path,
    pub warnings: Option<&'a Path>,
    pub name: Option<String>,
    pub overwrite: bool,
}

/// What packing produced.
#[derive(Debug)]
pub struct PackReport {
    pub out: PathBuf,
    pub manifest: Manifest,
    pub command_count: usize,
}

/// Pack an artifact. Inputs are validated before anything is written; the
/// artifact is published atomically at the exact output path.
pub fn pack(opts: &PackOptions, limits: &Limits) -> Result<PackReport> {
    let recipe_bytes = zipio::read_file_bounded(opts.recipe, limits.max_recipe_bytes, "recipe")?;
    let recipe_value = recipe::validate(&recipe_bytes, limits)?;
    let command_count = recipe::commands_of(&recipe_value)?.len();

    let step_bytes = zipio::read_file_bounded(opts.step, limits.max_step_bytes, "STEP geometry")?;
    validate_step(&step_bytes)?;

    let warnings = match opts.warnings {
        Some(path) => {
            let bytes =
                zipio::read_file_bounded(path, limits.max_warnings_bytes, "warnings sidecar")?;
            WarningsCapture::provided(parse_warnings(&bytes, limits)?)
        }
        None => WarningsCapture::not_provided(),
    };

    let name = resolve_name(opts);
    let manifest = Manifest {
        profile: PROFILE_ID.to_string(),
        container: CONTAINER_ID.to_string(),
        name,
        created_by: format!("craft-bridge {}", env!("CARGO_PKG_VERSION")),
        source: Source {
            recipe_file: file_name(opts.recipe),
            step_file: file_name(opts.step),
            warnings_file: opts.warnings.map(file_name),
        },
        warnings,
        fallback_mode: FALLBACK_MODE.to_string(),
        members: vec![
            MemberEntry {
                path: RECIPE.to_string(),
                bytes: recipe_bytes.len() as u64,
            },
            MemberEntry {
                path: STEP.to_string(),
                bytes: step_bytes.len() as u64,
            },
        ],
    };
    let mut manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| Error::json("manifest.json", e))?;
    manifest_bytes.push(b'\n');

    zipio::write_archive_atomic(
        opts.out,
        opts.overwrite,
        limits,
        &[
            (MANIFEST, manifest_bytes.as_slice()),
            (RECIPE, recipe_bytes.as_slice()),
            (STEP, step_bytes.as_slice()),
        ],
    )?;

    Ok(PackReport {
        out: opts.out.to_path_buf(),
        manifest,
        command_count,
    })
}

/// A STEP body must be ISO-10303-21 text; that is the whole contract.
fn validate_step(bytes: &[u8]) -> Result<()> {
    let trimmed = trim_start_ascii_whitespace(bytes);
    if !trimmed.starts_with(b"ISO-10303-21;") {
        return Err(Error::InvalidStep(
            "does not start with ISO-10303-21;".to_string(),
        ));
    }
    if trimmed.len() <= b"ISO-10303-21;".len() {
        return Err(Error::InvalidStep("has no content".to_string()));
    }
    Ok(())
}

fn trim_start_ascii_whitespace(mut bytes: &[u8]) -> &[u8] {
    while let Some((first, rest)) = bytes.split_first() {
        if first.is_ascii_whitespace() {
            bytes = rest;
        } else {
            break;
        }
    }
    bytes
}

/// The warnings sidecar is a JSON array of strings.
fn parse_warnings(bytes: &[u8], limits: &Limits) -> Result<Vec<String>> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| Error::json("warnings sidecar", e))?;
    let items = value
        .as_array()
        .ok_or_else(|| Error::InvalidWarnings("root is not an array".to_string()))?;
    if items.len() > limits.max_warnings {
        return Err(Error::limit(
            "warnings count",
            limits.max_warnings as u64,
            items.len() as u64,
        ));
    }
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| Error::InvalidWarnings("an entry is not a string".to_string()))
        })
        .collect()
}

fn resolve_name(opts: &PackOptions) -> String {
    let name = opts
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            opts.out
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "artifact".to_string())
        });
    let mut name: String = name.chars().take(200).collect();
    name.retain(|c| !c.is_control());
    if name.is_empty() {
        name = "artifact".to_string();
    }
    name
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
