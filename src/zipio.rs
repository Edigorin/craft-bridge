//! Bounded, allowlisting I/O for artifacts and their inputs.
//!
//! Artifacts are hostile. Members are read directly out of the archive, never
//! extracted to disk, and only the exact profile member names are accepted —
//! no path separators beyond the one in `geometry/final.step`, no alternate
//! spellings, no directories, no duplicates. Output is published atomically,
//! refuses symlinks, and lands only at the requested path.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::result;

use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::profile::{is_snapshot_member, KNOWN_MEMBERS, SNAPSHOT_PREFIX};

/// Read a whole input file, bounded by `limit` both by metadata and during the
/// read, so a lying size cannot cause an unbounded allocation.
pub fn read_file_bounded(path: &Path, limit: u64, what: &str) -> Result<Vec<u8>> {
    let meta =
        fs::metadata(path).map_err(|e| Error::io(format!("{what} {}", path.display()), e))?;
    if !meta.is_file() {
        return Err(Error::InvalidArtifact(format!(
            "{what} {} is not a regular file",
            path.display()
        )));
    }
    if meta.len() > limit {
        return Err(Error::limit(
            format!("{what} {}", path.display()),
            limit,
            meta.len(),
        ));
    }
    let file = File::open(path).map_err(|e| Error::io(format!("{what} {}", path.display()), e))?;
    let mut buf = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| Error::io(format!("{what} {}", path.display()), e))?;
    if buf.len() as u64 > limit {
        return Err(Error::limit(
            format!("{what} {}", path.display()),
            limit,
            buf.len() as u64,
        ));
    }
    Ok(buf)
}

/// Open an artifact for reading: file-size cap first (this bounds the ZIP
/// central directory too), then the member-count cap, then full member-name
/// validation from the central directory — all before any member is touched.
///
/// The name census reads the central directory directly because the ZIP
/// library collapses duplicate names to one entry at construction; without
/// the census, a duplicated member would be invisible. The census is a
/// bounded validation scan (names only), not a parser: the library still
/// reads all content.
pub fn open_archive(path: &Path, limits: &Limits) -> Result<ZipArchive<File>> {
    let meta =
        fs::metadata(path).map_err(|e| Error::io(format!("artifact {}", path.display()), e))?;
    if !meta.is_file() {
        return Err(Error::InvalidArtifact(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    if meta.len() > limits.max_archive_bytes {
        return Err(Error::limit(
            format!("artifact {}", path.display()),
            limits.max_archive_bytes,
            meta.len(),
        ));
    }
    let mut file =
        File::open(path).map_err(|e| Error::io(format!("artifact {}", path.display()), e))?;
    let names = census_names(&mut file, limits)?;
    check_names(&names)?;
    let archive = ZipArchive::new(file).map_err(|e| {
        Error::InvalidArtifact(format!("{} is not a readable ZIP: {e}", path.display()))
    })?;
    if archive.len() != names.len() {
        return Err(Error::InvalidArtifact(format!(
            "the ZIP library sees {} members but the central directory lists {}",
            archive.len(),
            names.len()
        )));
    }
    Ok(archive)
}

const EOCD_SIG: u32 = 0x0605_4b50;
const CD_HEADER_SIG: u32 = 0x0201_4b50;
const ZIP64_SENTINEL: u32 = 0xFFFF_FFFF;

/// Enumerate member names from the ZIP central directory. Bounded by the
/// member-count limit and by reads of at most 46 bytes plus one name per
/// entry. ZIP64 archives are refused: the profile's size caps are far below
/// ZIP64 thresholds.
fn census_names(file: &mut File, limits: &Limits) -> Result<Vec<String>> {
    let len = file
        .metadata()
        .map_err(|e| Error::io("artifact metadata", e))?
        .len();
    // The end-of-central-directory record is 22 bytes plus up to 65535 bytes
    // of comment, at the very end of the file.
    let tail_len = 22u64 + 65535;
    file.seek(SeekFrom::Start(len.saturating_sub(tail_len)))
        .map_err(|e| Error::io("artifact seek", e))?;
    let mut tail = Vec::new();
    (&mut *file)
        .take(tail_len)
        .read_to_end(&mut tail)
        .map_err(|e| Error::io("artifact tail", e))?;
    let mut eocd = None;
    if tail.len() >= 22 {
        for i in (0..=tail.len() - 22).rev() {
            if u32_at(&tail, i) == EOCD_SIG {
                eocd = Some(i);
                break;
            }
        }
    }
    let eocd = eocd
        .ok_or_else(|| Error::InvalidArtifact("no end-of-central-directory record".to_string()))?;
    let total = u16_at(&tail, eocd + 10) as usize;
    let cd_size = u32_at(&tail, eocd + 12);
    let cd_offset = u32_at(&tail, eocd + 16);
    if total as u32 == u16::MAX as u32 || cd_size == ZIP64_SENTINEL || cd_offset == ZIP64_SENTINEL {
        return Err(Error::InvalidArtifact(
            "ZIP64 archives are not supported".to_string(),
        ));
    }
    if total > limits.max_members {
        return Err(Error::limit(
            "artifact member count",
            limits.max_members as u64,
            total as u64,
        ));
    }
    let cd_end = u64::from(cd_offset).saturating_add(u64::from(cd_size));
    if cd_end > len {
        return Err(Error::InvalidArtifact(
            "central directory lies outside the file".to_string(),
        ));
    }
    file.seek(SeekFrom::Start(u64::from(cd_offset)))
        .map_err(|e| Error::io("artifact seek", e))?;
    let mut names = Vec::with_capacity(total);
    for index in 0..total {
        let mut header = [0u8; 46];
        file.read_exact(&mut header)
            .map_err(|e| Error::io(format!("central directory entry {index}"), e))?;
        if u32_at(&header, 0) != CD_HEADER_SIG {
            return Err(Error::InvalidArtifact(format!(
                "central directory entry {index} has a bad signature"
            )));
        }
        let name_len = u16_at(&header, 28) as usize;
        let extra_len = u16_at(&header, 30) as usize;
        let comment_len = u16_at(&header, 32) as usize;
        let mut name = vec![0u8; name_len];
        file.read_exact(&mut name)
            .map_err(|e| Error::io(format!("central directory entry {index} name"), e))?;
        file.seek(SeekFrom::Current((extra_len + comment_len) as i64))
            .map_err(|e| Error::io("artifact seek", e))?;
        let name = String::from_utf8(name).map_err(|_| {
            Error::InvalidArtifact(format!("central directory entry {index} name is not UTF-8"))
        })?;
        names.push(name);
    }
    Ok(names)
}

/// Every member name must be a known profile member, exactly once, as a
/// regular file. Anything else is refused before reading.
fn check_names(names: &[String]) -> Result<()> {
    let mut seen: Vec<&String> = Vec::new();
    for name in names {
        if !KNOWN_MEMBERS.contains(&name.as_str()) && !is_snapshot_member(name) {
            return Err(Error::InvalidArtifact(format!(
                "unknown member {name:?} (allowed: {KNOWN_MEMBERS:?} plus {SNAPSHOT_PREFIX}step_NNNN.step)"
            )));
        }
        if name.ends_with('/') {
            return Err(Error::InvalidArtifact(format!(
                "member {name} is not a regular file"
            )));
        }
        if seen.contains(&name) {
            return Err(Error::InvalidArtifact(format!("duplicate member {name}")));
        }
        seen.push(name);
    }
    Ok(())
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    bytes
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .unwrap_or(0)
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0)
}

/// Read one known member, bounded per member and by the shared total budget.
/// Declared size and actual decompressed size must agree.
pub fn read_member(
    archive: &mut ZipArchive<File>,
    name: &str,
    limits: &Limits,
    budget: &mut u64,
) -> Result<Vec<u8>> {
    if !KNOWN_MEMBERS.contains(&name) && !is_snapshot_member(name) {
        return Err(Error::InvalidArtifact(format!("unknown member {name}")));
    }
    let cap = limits.max_member_bytes.min(*budget);
    let mut buf = Vec::new();
    let declared;
    {
        let mut entry = archive
            .by_name(name)
            .map_err(|_| Error::InvalidArtifact(format!("artifact has no member {name}")))?;
        declared = entry.size();
        if declared > cap {
            return Err(Error::limit(format!("member {name}"), cap, declared));
        }
        entry
            .by_ref()
            .take(cap.saturating_add(1))
            .read_to_end(&mut buf)
            .map_err(|e| Error::io(format!("member {name}"), e))?;
    }
    let actual = buf.len() as u64;
    if actual > cap {
        return Err(Error::limit(format!("member {name}"), cap, actual));
    }
    if actual != declared {
        return Err(Error::InvalidArtifact(format!(
            "member {name} unpacked to {actual} bytes but declares {declared}"
        )));
    }
    *budget = budget.saturating_sub(actual);
    Ok(buf)
}

/// Write an artifact atomically at `out`.
///
/// Refuses symlink destinations always; refuses to replace an existing file
/// unless `overwrite` is set. The archive is staged in the destination
/// directory with exclusive creation and renamed into place; failures remove
/// the staging file and leave any existing destination unchanged.
pub fn write_archive_atomic(
    out: &Path,
    overwrite: bool,
    limits: &Limits,
    members: &[(&str, &[u8])],
) -> Result<()> {
    let mut seen: Vec<&str> = Vec::new();
    for (name, bytes) in members {
        if !KNOWN_MEMBERS.contains(name) && !is_snapshot_member(name) {
            return Err(Error::output(
                out,
                format!("refusing to write unknown member {name}"),
            ));
        }
        if seen.contains(name) {
            return Err(Error::output(out, format!("duplicate member {name}")));
        }
        seen.push(name);
        if bytes.len() as u64 > limits.max_member_bytes {
            return Err(Error::limit(
                format!("member {name}"),
                limits.max_member_bytes,
                bytes.len() as u64,
            ));
        }
    }

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
        .unwrap_or_else(|| "artifact".to_string());
    let staging = dir.join(format!(".{stem}.tmp-{}", std::process::id()));

    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .map_err(|e| Error::io(format!("staging {}", staging.display()), e))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    let written: result::Result<_, Error> = (|| {
        for (name, bytes) in members {
            writer
                .start_file(*name, options)
                .map_err(|e| Error::output(out, format!("zip: {e}")))?;
            writer
                .write_all(bytes)
                .map_err(|e| Error::io(format!("staging {}", staging.display()), e))?;
        }
        let file = writer
            .finish()
            .map_err(|e| Error::output(out, format!("zip: {e}")))?;
        file.sync_all()
            .map_err(|e| Error::io(format!("staging {}", staging.display()), e))?;
        Ok(())
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&staging);
        return Err(e);
    }
    if let Err(e) = fs::rename(&staging, out) {
        let _ = fs::remove_file(&staging);
        return Err(Error::io(format!("output {}", out.display()), e));
    }
    Ok(())
}
