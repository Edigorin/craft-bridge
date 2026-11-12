//! The replay profile: the versioned agreement on what a craftx artifact
//! contains and how a rebuild consumes it.
//!
//! The container shape follows SolidCraft's draft RFC 0001 (`*.craftx`), and
//! this profile layers the replay payload over it. Deviations from the draft
//! are listed in `docs/replay-profile.md`. Compatibility with a future
//! CamCraft bridge is an intent, not a claim of established interoperability.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::limits::Limits;

/// The replay profile this build reads and writes.
pub const PROFILE_ID: &str = "craft-bridge/replay/1";

/// The container shape the artifact follows.
pub const CONTAINER_ID: &str = "craftx/1";

/// Manifest member.
pub const MANIFEST: &str = "manifest.json";
/// Recipe member: the SolidCraft script JSON, verbatim.
pub const RECIPE: &str = "recipe.json";
/// Geometry member: the single final-body STEP snapshot.
pub const STEP: &str = "geometry/final.step";

/// Every member name this profile accepts, exactly and case-sensitively.
/// Nothing else may appear in an artifact: no directories, no duplicates, no
/// alternate spellings.
pub const KNOWN_MEMBERS: [&str; 3] = [MANIFEST, RECIPE, STEP];

/// The fallback mode of this profile version: one STEP body from the end of
/// the recipe, replacing everything from the first unmappable step onward.
/// Per-boundary snapshots are a later profile capability.
pub const FALLBACK_MODE: &str = "final-snapshot-coarse";

/// What a packed artifact claims about itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub profile: String,
    pub container: String,
    pub name: String,
    pub created_by: String,
    pub source: Source,
    pub warnings: WarningsCapture,
    pub fallback_mode: String,
    /// Inventory of the payload members. `manifest.json` is excluded to avoid
    /// self-size ambiguity.
    pub members: Vec<MemberEntry>,
}

/// The input files the artifact was packed from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub recipe_file: String,
    pub step_file: String,
    pub warnings_file: Option<String>,
}

/// The exporter's omission warnings, as handed to the packer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarningsCapture {
    pub status: WarningStatus,
    pub items: Vec<String>,
}

/// Whether the exporter's omission warnings were supplied at all. "Provided"
/// with zero items means the exporter reported no omissions; "not-provided"
/// means the omissions are unknown and must be shown as such.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WarningStatus {
    Provided,
    NotProvided,
}

impl WarningsCapture {
    pub fn provided(items: Vec<String>) -> Self {
        WarningsCapture {
            status: WarningStatus::Provided,
            items,
        }
    }

    pub fn not_provided() -> Self {
        WarningsCapture {
            status: WarningStatus::NotProvided,
            items: Vec::new(),
        }
    }

    /// The line inspect shows for this capture.
    pub fn describe(&self) -> String {
        match self.status {
            WarningStatus::Provided if self.items.is_empty() => {
                "provided: exporter reported no omissions".to_string()
            }
            WarningStatus::Provided => {
                format!("provided: {} omission warning(s)", self.items.len())
            }
            WarningStatus::NotProvided => "NOT PROVIDED: exporter omissions unknown".to_string(),
        }
    }
}

/// One payload member and its exact byte size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberEntry {
    pub path: String,
    pub bytes: u64,
}

/// Parse and validate a manifest body against this profile.
pub fn parse_manifest(bytes: &[u8], limits: &Limits) -> Result<Manifest> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::InvalidArtifact("manifest.json is not UTF-8".to_string()))?;
    let manifest: Manifest =
        serde_json::from_str(text).map_err(|e| Error::json("manifest.json", e))?;
    if manifest.profile != PROFILE_ID {
        return Err(Error::UnsupportedProfile {
            found: manifest.profile,
            supported: PROFILE_ID.to_string(),
        });
    }
    if manifest.container != CONTAINER_ID {
        return Err(Error::InvalidArtifact(format!(
            "container is {} (this build writes {})",
            manifest.container, CONTAINER_ID
        )));
    }
    if manifest.fallback_mode != FALLBACK_MODE {
        return Err(Error::InvalidArtifact(format!(
            "fallback mode is {} (this profile is {})",
            manifest.fallback_mode, FALLBACK_MODE
        )));
    }
    if manifest.members.len() > limits.max_members {
        return Err(Error::limit(
            "manifest member inventory",
            limits.max_members as u64,
            manifest.members.len() as u64,
        ));
    }
    let mut seen = Vec::new();
    for entry in &manifest.members {
        if entry.path == MANIFEST {
            return Err(Error::InvalidArtifact(
                "manifest.json must not list itself in the inventory".to_string(),
            ));
        }
        if !KNOWN_MEMBERS.contains(&entry.path.as_str()) {
            return Err(Error::InvalidArtifact(format!(
                "unknown member {} in the inventory",
                entry.path
            )));
        }
        if seen.contains(&entry.path) {
            return Err(Error::InvalidArtifact(format!(
                "duplicate member {} in the inventory",
                entry.path
            )));
        }
        seen.push(entry.path.clone());
    }
    for required in [RECIPE, STEP] {
        if !seen.iter().any(|p| p == required) {
            return Err(Error::InvalidArtifact(format!(
                "inventory is missing {required}"
            )));
        }
    }
    Ok(manifest)
}
