//! Build orchestration: a validated artifact becomes a `.swb` macro and a
//! plan report. All output goes through the same publication policy as
//! packing: refuse symlinks, refuse existing destinations unless asked,
//! stage exclusively and rename into place.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::profile::{self, Manifest, RECIPE};
use crate::recipe;
use crate::replay;
use crate::report::{self, PlanReport};
use crate::swb;
use crate::zipio;

pub struct BuildOptions<'a> {
    pub artifact: &'a Path,
    pub out: &'a Path,
    pub plan: Option<&'a Path>,
    pub plan_text: Option<&'a Path>,
    pub overwrite: bool,
}

#[derive(Debug)]
pub struct BuildOutcome {
    pub macro_bytes: usize,
    pub plan: PlanReport,
}

/// Build the macro and the plan report from an artifact.
pub fn build(opts: &BuildOptions, limits: &Limits) -> Result<BuildOutcome> {
    let artifact_name = opts
        .artifact
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| opts.artifact.display().to_string());
    let macro_name = opts
        .out
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "macro.swb".to_string());

    // Read the artifact through the bounded, allowlisting reader.
    let mut archive = zipio::open_archive(opts.artifact, limits)?;
    let mut budget = limits.max_total_bytes;
    let manifest_bytes = zipio::read_member(&mut archive, profile::MANIFEST, limits, &mut budget)?;
    let _manifest: Manifest = profile::parse_manifest(&manifest_bytes, limits)?; // validated against the profile
    let recipe_bytes = zipio::read_member(&mut archive, RECIPE, limits, &mut budget)?;
    let _step_bytes = zipio::read_member(&mut archive, profile::STEP, limits, &mut budget)?;
    let recipe_value = recipe::validate(&recipe_bytes, limits)?;

    let steps = replay::plan(&recipe_value, limits)?;
    let macro_text = swb::emit(&steps, &artifact_name, env!("CARGO_PKG_VERSION"));
    let macro_bytes = macro_text.as_bytes();
    if macro_bytes.len() as u64 > limits.max_generated_bytes {
        return Err(Error::limit(
            "generated macro",
            limits.max_generated_bytes,
            macro_bytes.len() as u64,
        ));
    }

    let plan = report::build_report(&steps, &artifact_name, &macro_name);
    if let Some(path) = opts.plan {
        let mut bytes =
            serde_json::to_vec_pretty(&plan).map_err(|e| Error::json("plan report", e))?;
        bytes.push(b'\n');
        publish_file(path, opts.overwrite, &bytes)?;
    }
    if let Some(path) = opts.plan_text {
        publish_file(path, opts.overwrite, report::render_text(&plan).as_bytes())?;
    }
    publish_file(opts.out, opts.overwrite, macro_bytes)?;

    Ok(BuildOutcome {
        macro_bytes: macro_bytes.len(),
        plan,
    })
}

/// Publish one generated file with the standard policy: refuse symlink
/// destinations always, refuse existing files unless `overwrite`, stage
/// exclusively in the destination directory and rename into place, removing
/// the staging file on any failure. Mirrors zipio::write_archive_atomic.
fn publish_file(out: &Path, overwrite: bool, bytes: &[u8]) -> Result<()> {
    use std::fs::{self, OpenOptions};
    use std::io::Write;

    match fs::symlink_metadata(out) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(Error::output(
                out,
                "refusing to write through a symbolic link",
            ))
        }
        Ok(meta) if meta.is_file() => {
            if !overwrite {
                return Err(Error::output(
                    out,
                    "already exists (pass --overwrite to replace it)",
                ));
            }
        }
        Ok(_) => return Err(Error::output(out, "exists and is not a regular file")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::io(format!("output {}", out.display()), e)),
    }
    let dir = match out.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let stem = out
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_string());
    let staging = dir.join(format!(".{stem}.tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .map_err(|e| Error::io(format!("staging {}", staging.display()), e))?;
    if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&staging);
        return Err(Error::io(format!("staging {}", staging.display()), e));
    }
    drop(file);
    if let Err(e) = fs::rename(&staging, out) {
        let _ = fs::remove_file(&staging);
        return Err(Error::io(format!("output {}", out.display()), e));
    }
    Ok(())
}
