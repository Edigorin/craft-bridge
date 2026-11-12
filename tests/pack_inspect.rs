//! End-to-end tests for packing and inspecting craftx artifacts: the pinned
//! contract, the hostile-input boundaries, and the output policy. Inputs here
//! are synthetic, exporter-shaped fixtures authored for this repository.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use craft_bridge::inspect;
use craft_bridge::limits::Limits;
use craft_bridge::pack::{self, PackOptions};
use craft_bridge::profile::{
    Manifest, MemberEntry, Source, WarningsCapture, CONTAINER_ID, FALLBACK_MODE, MANIFEST,
    PROFILE_ID, RECIPE, STEP,
};

use common::Entry;

const RECIPE_FIXTURE: &str = include_str!("fixtures/script.json");
const STEP_FIXTURE: &str = include_str!("fixtures/part.step");
const WARNINGS_FIXTURE: &str = include_str!("fixtures/warnings.json");
const GOLDEN_MANIFEST: &str = include_str!("golden/manifest.json");
const GOLDEN_INSPECT_JSON: &str = include_str!("golden/inspect.json");
const GOLDEN_INSPECT_TXT: &str = include_str!("golden/inspect.txt");

struct Case {
    dir: tempfile::TempDir,
    recipe: PathBuf,
    step: PathBuf,
    warnings: PathBuf,
    out: PathBuf,
}

impl Case {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let recipe = dir.path().join("script.json");
        let step = dir.path().join("part.step");
        let warnings = dir.path().join("warnings.json");
        let out = dir.path().join("part.craftx");
        fs::write(&recipe, RECIPE_FIXTURE).unwrap();
        fs::write(&step, STEP_FIXTURE).unwrap();
        fs::write(&warnings, WARNINGS_FIXTURE).unwrap();
        Case {
            dir,
            recipe,
            step,
            warnings,
            out,
        }
    }

    fn pack_with_warnings(
        &self,
        warnings: Option<&Path>,
    ) -> craft_bridge::Result<pack::PackReport> {
        pack::pack(
            &PackOptions {
                recipe: &self.recipe,
                step: &self.step,
                out: &self.out,
                warnings,
                name: None,
                overwrite: false,
            },
            &Limits::default(),
        )
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

fn raw_member(artifact: &Path, name: &str) -> String {
    let file = fs::File::open(artifact).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut entry = archive.by_name(name).unwrap();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn err_text(result: craft_bridge::Result<impl std::fmt::Debug>) -> String {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    }
}

fn write_artifact(path: &Path, entries: &[Entry]) {
    fs::write(path, common::build_zip(entries)).unwrap();
}

fn crafted_manifest(members: &[(&str, usize)]) -> Vec<u8> {
    let manifest = Manifest {
        profile: PROFILE_ID.to_string(),
        container: CONTAINER_ID.to_string(),
        name: "crafted".to_string(),
        created_by: "craft-bridge tests".to_string(),
        source: Source {
            recipe_file: "script.json".to_string(),
            step_file: "part.step".to_string(),
            warnings_file: None,
        },
        warnings: WarningsCapture::provided(vec!["example omitted".to_string()]),
        fallback_mode: FALLBACK_MODE.to_string(),
        members: members
            .iter()
            .map(|(path, bytes)| MemberEntry {
                path: path.to_string(),
                bytes: *bytes as u64,
            })
            .collect(),
    };
    serde_json::to_vec_pretty(&manifest).unwrap()
}

// ---- the pinned contract ----

#[test]
fn manifest_is_pinned() {
    let case = Case::new();
    case.pack_with_warnings(Some(&case.warnings)).unwrap();
    let manifest = raw_member(&case.out, MANIFEST);
    assert_eq!(manifest, GOLDEN_MANIFEST);
}

#[test]
fn inspect_json_is_pinned() {
    let case = Case::new();
    case.pack_with_warnings(Some(&case.warnings)).unwrap();
    let report = inspect::inspect(&case.out, &Limits::default()).unwrap();
    let json = serde_json::to_string_pretty(&report).unwrap();
    assert_eq!(json.trim_end(), GOLDEN_INSPECT_JSON.trim_end());
}

#[test]
fn inspect_human_is_pinned() {
    let case = Case::new();
    case.pack_with_warnings(Some(&case.warnings)).unwrap();
    let report = inspect::inspect(&case.out, &Limits::default()).unwrap();
    assert_eq!(inspect::render_human(&report), GOLDEN_INSPECT_TXT);
}

// ---- warnings semantics ----

#[test]
fn missing_warnings_sidecar_says_oms_unknown() {
    let case = Case::new();
    case.pack_with_warnings(None).unwrap();
    let report = inspect::inspect(&case.out, &Limits::default()).unwrap();
    let human = inspect::render_human(&report);
    assert!(human.contains("NOT PROVIDED: exporter omissions unknown"));
    assert_eq!(report.warnings.items.len(), 0);
}

#[test]
fn provided_empty_warnings_is_distinct() {
    let case = Case::new();
    fs::write(case.path().join("empty.json"), "[]").unwrap();
    case.pack_with_warnings(Some(&case.path().join("empty.json")))
        .unwrap();
    let report = inspect::inspect(&case.out, &Limits::default()).unwrap();
    let human = inspect::render_human(&report);
    assert!(human.contains("provided: exporter reported no omissions"));
    assert!(!human.contains("NOT PROVIDED"));
}

#[test]
fn provided_warnings_are_listed() {
    let case = Case::new();
    case.pack_with_warnings(Some(&case.warnings)).unwrap();
    let report = inspect::inspect(&case.out, &Limits::default()).unwrap();
    let human = inspect::render_human(&report);
    assert!(human.contains("provided: 2 omission warning(s)"));
    assert!(human.contains("- appearances omitted"));
    assert!(human.contains("- canvases omitted"));
}

// ---- recipe semantics ----

#[test]
fn unknown_commands_are_preserved_not_executed() {
    let case = Case::new();
    case.pack_with_warnings(None).unwrap();
    let report = inspect::inspect(&case.out, &Limits::default()).unwrap();
    assert!(report
        .steps
        .iter()
        .any(|s| s.command == "vendor.future_thing"));
}

#[test]
fn parameters_keep_expressions_and_units() {
    let case = Case::new();
    case.pack_with_warnings(None).unwrap();
    let report = inspect::inspect(&case.out, &Limits::default()).unwrap();
    let height = report
        .parameters
        .user
        .iter()
        .find(|p| p.name == "height")
        .unwrap();
    assert_eq!(height.raw["expression"], "2 * width");
    let d2 = report
        .parameters
        .model
        .iter()
        .find(|p| p.name == "d2")
        .unwrap();
    assert_eq!(d2.raw, "12 mm");
}

#[test]
fn recipe_shape_is_validated() {
    let case = Case::new();
    fs::write(&case.recipe, "{}").unwrap();
    assert!(err_text(case.pack_with_warnings(None)).contains("no commands array"));
    fs::write(&case.recipe, r#"{"commands":[{"params":{}}]}"#).unwrap();
    assert!(err_text(case.pack_with_warnings(None)).contains("no command id"));
    fs::write(&case.recipe, r#"{"commands":[{"command":"x","params":1}]}"#).unwrap();
    assert!(err_text(case.pack_with_warnings(None)).contains("params are not an object"));
}

#[test]
fn step_and_warnings_shape_is_validated() {
    let case = Case::new();
    fs::write(&case.step, "solid nonsense").unwrap();
    assert!(err_text(case.pack_with_warnings(None)).contains("ISO-10303-21"));
    fs::write(&case.step, STEP_FIXTURE).unwrap();
    fs::write(&case.warnings, r#"{"not":"an array"}"#).unwrap();
    assert!(err_text(case.pack_with_warnings(Some(&case.warnings))).contains("not an array"));
}

// ---- output policy ----

#[test]
fn pack_refuses_to_replace_without_overwrite() {
    let case = Case::new();
    case.pack_with_warnings(None).unwrap();
    let err = err_text(case.pack_with_warnings(None));
    assert!(err.contains("already exists"));
}

#[test]
fn pack_overwrite_replaces_regular_files() {
    let case = Case::new();
    case.pack_with_warnings(None).unwrap();
    pack::pack(
        &PackOptions {
            recipe: &case.recipe,
            step: &case.step,
            out: &case.out,
            warnings: None,
            name: None,
            overwrite: true,
        },
        &Limits::default(),
    )
    .unwrap();
    inspect::inspect(&case.out, &Limits::default()).unwrap();
}

#[cfg(unix)]
#[test]
fn pack_refuses_symlink_destinations() {
    let case = Case::new();
    let target = case.path().join("elsewhere.craftx");
    std::os::unix::fs::symlink(&target, &case.out).unwrap();
    let err = err_text(case.pack_with_warnings(None));
    assert!(err.contains("symbolic link"));
}

#[test]
fn failed_pack_leaves_existing_file_and_no_litter() {
    let case = Case::new();
    fs::write(&case.out, b"keep me").unwrap();
    fs::write(&case.recipe, "not json").unwrap();
    assert!(case.pack_with_warnings(None).is_err());
    assert_eq!(fs::read(&case.out).unwrap(), b"keep me");
    let litter: Vec<_> = fs::read_dir(case.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(litter.is_empty(), "staging files left behind: {litter:?}");
}

// ---- hostile artifacts ----

#[test]
fn zip_slip_member_names_are_rejected() {
    let case = Case::new();
    let artifact = case.path().join("slip.craftx");
    write_artifact(
        &artifact,
        &[
            Entry::stored(MANIFEST, b"{}"),
            Entry::stored("../evil", b"x"),
        ],
    );
    let err = err_text(inspect::inspect(&artifact, &Limits::default()));
    assert!(err.contains("unknown member"));
}

#[test]
fn case_and_directory_variants_are_rejected() {
    let case = Case::new();
    let artifact = case.path().join("case.craftx");
    write_artifact(
        &artifact,
        &[
            Entry::stored(MANIFEST, b"{}"),
            Entry::stored("Recipe.json", b"x"),
            Entry::stored("geometry/", b""),
        ],
    );
    let err = err_text(inspect::inspect(&artifact, &Limits::default()));
    assert!(err.contains("unknown member"));
}

#[test]
fn duplicate_members_are_rejected() {
    let case = Case::new();
    let artifact = case.path().join("dup.craftx");
    write_artifact(
        &artifact,
        &[
            Entry::stored(MANIFEST, b"{}"),
            Entry::stored(RECIPE, b"{}"),
            Entry::stored(RECIPE, b"{}"),
        ],
    );
    let err = err_text(inspect::inspect(&artifact, &Limits::default()));
    assert!(err.contains("duplicate"), "unexpected error: {err}");
}

#[test]
fn member_count_is_capped_before_reading() {
    let case = Case::new();
    let artifact = case.path().join("many.craftx");
    let filler = [0u8; 8];
    let entries: Vec<Entry> = (0..9)
        .map(|i| Entry::stored(Box::leak(format!("m{i}").into_boxed_str()), &filler))
        .collect();
    write_artifact(&artifact, &entries);
    let err = err_text(inspect::inspect(&artifact, &Limits::small_for_tests()));
    assert!(err.contains("member count"));
}

#[test]
fn oversized_members_are_rejected_during_read() {
    let case = Case::new();
    let artifact = case.path().join("big.craftx");
    let recipe = b"{\"commands\":[]}".to_vec();
    let mut step = b"ISO-10303-21;".to_vec();
    step.resize(70 * 1024, b' ');
    step.extend_from_slice(b"END-ISO-10303-21;");
    let manifest = crafted_manifest(&[(RECIPE, recipe.len()), (STEP, step.len())]);
    write_artifact(
        &artifact,
        &[
            Entry::stored(MANIFEST, &manifest),
            Entry::stored(RECIPE, &recipe),
            Entry::stored(STEP, &step),
        ],
    );
    let err = err_text(inspect::inspect(&artifact, &Limits::small_for_tests()));
    assert!(err.contains("over the limit"));
}

#[test]
fn member_size_boundaries_are_exact() {
    let case = Case::new();
    let limit = Limits::small_for_tests().max_member_bytes as usize;
    let recipe = b"{\"commands\":[]}".to_vec();

    for size in [limit, limit + 1] {
        let artifact = case.path().join(format!("edge-{size}.craftx"));
        let mut step = b"ISO-10303-21;".to_vec();
        step.resize(size - 17, b' ');
        step.extend_from_slice(b"END-ISO-10303-21;");
        assert_eq!(step.len(), size);
        let manifest = crafted_manifest(&[(RECIPE, recipe.len()), (STEP, step.len())]);
        write_artifact(
            &artifact,
            &[
                Entry::stored(MANIFEST, &manifest),
                Entry::stored(RECIPE, &recipe),
                Entry::stored(STEP, &step),
            ],
        );
        let result = inspect::inspect(&artifact, &Limits::small_for_tests());
        if size == limit {
            assert!(result.is_ok(), "exactly at the limit must pass: {result:?}");
        } else {
            assert!(err_text(result).contains("over the limit"));
        }
    }
}

#[test]
fn corrupt_crc_is_rejected() {
    let case = Case::new();
    let artifact = case.path().join("corrupt.craftx");
    let recipe = RECIPE_FIXTURE.as_bytes();
    let manifest = crafted_manifest(&[(RECIPE, recipe.len()), (STEP, STEP_FIXTURE.len())]);
    let mut bytes = common::build_zip(&[
        Entry::stored(MANIFEST, &manifest),
        Entry::stored(RECIPE, recipe),
        Entry::stored(STEP, STEP_FIXTURE.as_bytes()),
    ]);
    let at = common::find(&bytes, b"\"commands\"").unwrap() + 2;
    bytes[at] ^= 0xFF;
    fs::write(&artifact, &bytes).unwrap();
    let err = err_text(inspect::inspect(&artifact, &Limits::default()));
    assert!(err.contains("recipe.json"), "unexpected error: {err}");
}

#[test]
fn truncated_archives_are_rejected() {
    let case = Case::new();
    case.pack_with_warnings(None).unwrap();
    let mut bytes = fs::read(&case.out).unwrap();
    let len = bytes.len();
    bytes.truncate(len - 12);
    let artifact = case.path().join("truncated.craftx");
    fs::write(&artifact, &bytes).unwrap();
    let err = err_text(inspect::inspect(&artifact, &Limits::default()));
    assert!(
        err.contains("not a readable ZIP") || err.contains("end-of-central-directory"),
        "unexpected error: {err}"
    );
}

#[test]
fn unsupported_compression_is_rejected() {
    let case = Case::new();
    let artifact = case.path().join("bz2.craftx");
    let manifest = crafted_manifest(&[(RECIPE, 0), (STEP, STEP_FIXTURE.len())]);
    write_artifact(
        &artifact,
        &[
            Entry::stored(MANIFEST, &manifest),
            Entry::method(RECIPE, b"x", 12), // bzip2: unsupported by design
            Entry::stored(STEP, STEP_FIXTURE.as_bytes()),
        ],
    );
    let err = err_text(inspect::inspect(&artifact, &Limits::default()));
    assert!(
        err.contains("Compression method") || err.contains("recipe.json"),
        "unexpected error: {err}"
    );
}

#[test]
fn broken_manifests_are_rejected() {
    let case = Case::new();
    let artifact = case.path().join("badmanifest.craftx");
    write_artifact(
        &artifact,
        &[
            Entry::stored(MANIFEST, b"not json"),
            Entry::stored(RECIPE, b"{}"),
            Entry::stored(STEP, STEP_FIXTURE.as_bytes()),
        ],
    );
    assert!(err_text(inspect::inspect(&artifact, &Limits::default())).contains("manifest.json"));
}

#[test]
fn wrong_profile_version_is_rejected() {
    let case = Case::new();
    let artifact = case.path().join("oldprofile.craftx");
    let mut manifest =
        String::from_utf8(crafted_manifest(&[(RECIPE, 2), (STEP, STEP_FIXTURE.len())])).unwrap();
    manifest = manifest.replace(PROFILE_ID, "craft-bridge/replay/0");
    write_artifact(
        &artifact,
        &[
            Entry::stored(MANIFEST, manifest.as_bytes()),
            Entry::stored(RECIPE, b"{}"),
            Entry::stored(STEP, STEP_FIXTURE.as_bytes()),
        ],
    );
    let err = err_text(inspect::inspect(&artifact, &Limits::default()));
    assert!(
        err.contains("craft-bridge/replay/1"),
        "unexpected error: {err}"
    );
}

#[test]
fn inventory_mismatches_are_rejected() {
    let case = Case::new();
    let artifact = case.path().join("mismatch.craftx");
    let recipe = b"{\"commands\":[]}";
    let manifest = crafted_manifest(&[(RECIPE, recipe.len()), (STEP, STEP_FIXTURE.len() + 1)]);
    write_artifact(
        &artifact,
        &[
            Entry::stored(MANIFEST, &manifest),
            Entry::stored(RECIPE, recipe),
            Entry::stored(STEP, STEP_FIXTURE.as_bytes()),
        ],
    );
    let err = err_text(inspect::inspect(&artifact, &Limits::default()));
    assert!(err.contains("inventory mismatch"));
}

#[test]
fn inventory_must_cover_payload_and_not_itself() {
    let case = Case::new();

    let missing = case.path().join("missing.craftx");
    let manifest = crafted_manifest(&[(RECIPE, 2)]);
    write_artifact(
        &missing,
        &[
            Entry::stored(MANIFEST, &manifest),
            Entry::stored(RECIPE, b"{}"),
        ],
    );
    assert!(
        err_text(inspect::inspect(&missing, &Limits::default())).contains("inventory is missing")
    );

    let selflisted = case.path().join("selflisted.craftx");
    let manifest = crafted_manifest(&[(MANIFEST, 0), (RECIPE, 2), (STEP, STEP_FIXTURE.len())]);
    write_artifact(
        &selflisted,
        &[
            Entry::stored(MANIFEST, &manifest),
            Entry::stored(RECIPE, b"{}"),
            Entry::stored(STEP, STEP_FIXTURE.as_bytes()),
        ],
    );
    assert!(err_text(inspect::inspect(&selflisted, &Limits::default()))
        .contains("must not list itself"));
}

#[test]
fn missing_members_are_rejected() {
    let case = Case::new();
    let artifact = case.path().join("absent.craftx");
    let recipe = b"{\"commands\":[]}";
    let manifest = crafted_manifest(&[(RECIPE, recipe.len()), (STEP, 3)]);
    write_artifact(
        &artifact,
        &[
            Entry::stored(MANIFEST, &manifest),
            Entry::stored(RECIPE, recipe),
        ],
    );
    let err = err_text(inspect::inspect(&artifact, &Limits::default()));
    assert!(err.contains("has no member"), "unexpected error: {err}");
}

#[test]
fn container_and_fallback_mode_must_match() {
    let case = Case::new();

    let wrong_container = case.path().join("container.craftx");
    let mut manifest =
        String::from_utf8(crafted_manifest(&[(RECIPE, 2), (STEP, STEP_FIXTURE.len())])).unwrap();
    manifest = manifest.replace(CONTAINER_ID, "craftx/9");
    write_artifact(
        &wrong_container,
        &[
            Entry::stored(MANIFEST, manifest.as_bytes()),
            Entry::stored(RECIPE, b"{}"),
            Entry::stored(STEP, STEP_FIXTURE.as_bytes()),
        ],
    );
    assert!(err_text(inspect::inspect(&wrong_container, &Limits::default())).contains("container"));

    let wrong_fallback = case.path().join("fallback.craftx");
    let mut manifest =
        String::from_utf8(crafted_manifest(&[(RECIPE, 2), (STEP, STEP_FIXTURE.len())])).unwrap();
    manifest = manifest.replace(FALLBACK_MODE, "something-else");
    write_artifact(
        &wrong_fallback,
        &[
            Entry::stored(MANIFEST, manifest.as_bytes()),
            Entry::stored(RECIPE, b"{}"),
            Entry::stored(STEP, STEP_FIXTURE.as_bytes()),
        ],
    );
    assert!(err_text(inspect::inspect(&wrong_fallback, &Limits::default())).contains("fallback"));
}
