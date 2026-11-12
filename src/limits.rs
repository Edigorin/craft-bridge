//! Numerical limits for untrusted input.
//!
//! Every read of an artifact or an input file is bounded *during* the read,
//! before an allocation can grow large. Arithmetic on sizes is checked. The ZIP
//! central directory itself is bounded first by the archive file-size limit and
//! then by the member-count limit, which is enforced before any member is
//! touched.

/// Largest artifact file we open. This bounds the ZIP central directory too,
/// because the directory is part of the file.
pub const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;

/// Most members an artifact may list. Enforced before any member is read.
pub const MAX_MEMBERS: usize = 16;

/// Largest decompressed member we read from an artifact.
pub const MAX_MEMBER_BYTES: u64 = 256 * 1024 * 1024;

/// Largest total decompressed bytes we read from one artifact.
pub const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

/// Largest raw recipe JSON we read.
pub const MAX_RECIPE_BYTES: u64 = 64 * 1024 * 1024;

/// Largest raw STEP file we read.
pub const MAX_STEP_BYTES: u64 = 256 * 1024 * 1024;

/// Largest raw warnings sidecar we read.
pub const MAX_WARNINGS_BYTES: u64 = 4 * 1024 * 1024;

/// Most commands a recipe may contain.
pub const MAX_COMMANDS: usize = 100_000;

/// Most omission warnings a sidecar may contain.
pub const MAX_WARNINGS: usize = 10_000;

// JSON nesting depth is bounded by serde_json's own recursion limit (128).
// The bridge never raises it.

/// The limits, as one value, so tests can exercise boundaries cheaply.
#[derive(Debug, Clone)]
pub struct Limits {
    pub max_archive_bytes: u64,
    pub max_members: usize,
    pub max_member_bytes: u64,
    pub max_total_bytes: u64,
    pub max_recipe_bytes: u64,
    pub max_step_bytes: u64,
    pub max_warnings_bytes: u64,
    pub max_commands: usize,
    pub max_warnings: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_archive_bytes: MAX_ARCHIVE_BYTES,
            max_members: MAX_MEMBERS,
            max_member_bytes: MAX_MEMBER_BYTES,
            max_total_bytes: MAX_TOTAL_BYTES,
            max_recipe_bytes: MAX_RECIPE_BYTES,
            max_step_bytes: MAX_STEP_BYTES,
            max_warnings_bytes: MAX_WARNINGS_BYTES,
            max_commands: MAX_COMMANDS,
            max_warnings: MAX_WARNINGS,
        }
    }
}

impl Limits {
    /// Tiny limits for boundary tests: nothing test-sized can exhaust these.
    pub fn small_for_tests() -> Self {
        Limits {
            max_archive_bytes: 1024 * 1024,
            max_members: 8,
            max_member_bytes: 64 * 1024,
            max_total_bytes: 256 * 1024,
            max_recipe_bytes: 64 * 1024,
            max_step_bytes: 64 * 1024,
            max_warnings_bytes: 16 * 1024,
            max_commands: 64,
            max_warnings: 32,
        }
    }
}
