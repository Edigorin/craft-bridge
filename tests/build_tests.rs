//! Tests for `craft-bridge build`: the pinned macro and plan outputs, the
//! stop rule, degradation planning, literal parsing, escaping and the output
//! policy. Everything is offline; the seat proves replay.

mod common;

use std::fs;
use std::path::PathBuf;

use craft_bridge::build::{self, BuildOptions};
use craft_bridge::limits::Limits;
use craft_bridge::pack::{self, PackOptions};
use craft_bridge::replay::{self, PlanStatus, Work};
use craft_bridge::swb;

const TRACER: &str = include_str!("fixtures/tracer.json");
const TRACER_STOP: &str = include_str!("fixtures/tracer-stop.json");
const TRACER_REVOLVE: &str = include_str!("fixtures/tracer-revolve.json");
const TRACER_PRIM: &str = include_str!("fixtures/tracer-prim.json");
const TRACER_MOD: &str = include_str!("fixtures/tracer-mod.json");
const TRACER_PAT: &str = include_str!("fixtures/tracer-pat.json");
const STEP_FIXTURE: &str = include_str!("fixtures/part.step");
const GOLDEN_MACRO: &str = include_str!("golden/tracer.swb");
const GOLDEN_PLAN_JSON: &str = include_str!("golden/tracer-plan.json");
const GOLDEN_PLAN_TXT: &str = include_str!("golden/tracer-plan.txt");

struct Case {
    dir: tempfile::TempDir,
    artifact: PathBuf,
}

impl Case {
    fn new(recipe: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let recipe_path = dir.path().join("tracer.json");
        let step_path = dir.path().join("part.step");
        let artifact = dir.path().join("tracer.craftx");
        fs::write(&recipe_path, recipe).unwrap();
        fs::write(&step_path, STEP_FIXTURE).unwrap();
        pack::pack(
            &PackOptions {
                recipe: &recipe_path,
                step: &step_path,
                out: &artifact,
                warnings: None,
                name: None,
                overwrite: false,
                snapshots: Vec::new(),
                final_snapshot: None,
            },
            &Limits::default(),
        )
        .unwrap();
        Case { dir, artifact }
    }

    fn build_to(
        &self,
        out_name: &str,
        overwrite: bool,
    ) -> craft_bridge::Result<build::BuildOutcome> {
        let out = self.dir.path().join(out_name);
        build::build(
            &BuildOptions {
                artifact: &self.artifact,
                out: &out,
                plan: None,
                plan_text: None,
                overwrite,
            },
            &Limits::default(),
        )
    }
}

fn plan_of(recipe_json: &str) -> Vec<replay::PlannedStep> {
    let value: serde_json::Value = serde_json::from_str(recipe_json).unwrap();
    craft_bridge::recipe::validate(
        serde_json::to_string(&value).unwrap().as_bytes(),
        &Limits::default(),
    )
    .unwrap();
    replay::plan(
        &value,
        &Limits::default(),
        &std::collections::BTreeMap::new(),
    )
    .unwrap()
}

// ---- the pinned contract ----

#[test]
fn tracer_macro_is_pinned() {
    let case = Case::new(TRACER);
    case.build_to("tracer.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("tracer.swb")).unwrap();
    assert_eq!(macro_text, GOLDEN_MACRO);
}

#[test]
fn tracer_plan_reports_are_pinned() {
    let case = Case::new(TRACER);
    let out = case.dir.path().join("tracer.swb");
    let plan_path = case.dir.path().join("tracer-plan.json");
    let text_path = case.dir.path().join("tracer-plan.txt");
    build::build(
        &BuildOptions {
            artifact: &case.artifact,
            out: &out,
            plan: Some(&plan_path),
            plan_text: Some(&text_path),
            overwrite: false,
        },
        &Limits::default(),
    )
    .unwrap();
    let plan_json = fs::read_to_string(&plan_path).unwrap();
    assert_eq!(plan_json.trim_end(), GOLDEN_PLAN_JSON.trim_end());
    let plan_text = fs::read_to_string(&text_path).unwrap();
    assert_eq!(plan_text, GOLDEN_PLAN_TXT);
}

// ---- the stop rule and degradation planning ----

#[test]
fn stop_rule_marks_the_boundary_and_everything_after() {
    let case = Case::new(TRACER_STOP);
    let outcome = case.build_to("stop.swb", false).unwrap();
    let plan = &outcome.plan;
    assert_eq!(plan.steps[0].status, PlanStatus::DegradePlanned); // pattern_point
    assert_eq!(plan.steps[1].status, PlanStatus::FallbackPlanned); // vendor.future_thing
    assert_eq!(plan.steps[2].status, PlanStatus::NotExecuted); // after the boundary
    let macro_text = fs::read_to_string(case.dir.path().join("stop.swb")).unwrap();
    assert!(macro_text.contains("after the first unmapped step"));
    assert!(macro_text.contains("no snapshot"));
    // The degraded sketch still generates its builder, with the degraded status.
    assert!(macro_text.contains("BuildSketch0 = \"degraded\""));
}

#[test]
fn unsupported_plane_stops_model_replay() {
    let recipe = TRACER.replace(
        "\"name\": \"XY\", \"type\": \"origin\"",
        "\"type\": \"custom\"",
    );
    let steps = plan_of(&recipe);
    assert_eq!(steps[2].status, PlanStatus::FallbackPlanned);
    assert!(matches!(steps[2].work, Work::Unmappable));
    assert_eq!(steps[3].status, PlanStatus::NotExecuted);
}

#[test]
fn malformed_sketch_data_is_rejected() {
    // A curve referencing a point that does not exist.
    let recipe = TRACER.replace("\"a\": 0, \"b\": 1", "\"a\": 99, \"b\": 1");
    let value: serde_json::Value = serde_json::from_str(&recipe).unwrap();
    let err = replay::plan(
        &value,
        &Limits::default(),
        &std::collections::BTreeMap::new(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("point 99"), "unexpected: {err}");

    // A non-literal extrude distance is the stop rule, not a hard error.
    let recipe = TRACER.replace("\"distance\": \"12 mm\"", "\"distance\": \"length / 2\"");
    let steps = plan_of(&recipe);
    assert_eq!(steps[3].status, PlanStatus::FallbackPlanned);
}

// ---- expressions (the engine suite is tests/expr_tests.rs) ----

#[test]
fn expressions_outside_the_subset_stop_the_step() {
    // An extrude distance naming an unknown parameter stops model replay.
    let recipe = TRACER.replace("\"distance\": \"12 mm\"", "\"distance\": \"mystery / 2\"");
    let steps = plan_of(&recipe);
    assert_eq!(steps[3].status, PlanStatus::FallbackPlanned);
    // A parameter expression outside the subset becomes a plain note: no
    // equation, no stop.
    let recipe = TRACER.replace("\"expression\": \"40 mm\"", "\"expression\": \"sqrt(40)\"");
    let steps = plan_of(&recipe);
    assert_eq!(steps[1].status, PlanStatus::NotExecuted);
}

// ---- escaping ----

#[test]
fn vba_literals_escape_quotes_and_controls() {
    assert_eq!(swb::vb_lit("plain"), "\"plain\"");
    assert_eq!(swb::vb_lit("a\"b"), "\"a\"\"b\"");
    assert_eq!(swb::vb_lit("a\nb\tc"), "\"a b c\"");
    assert_eq!(swb::json_str_content("a\"b\\c"), "a\\\"b\\\\c");
}

#[test]
fn hostile_names_are_escaped_in_the_macro() {
    let recipe = TRACER.replace("\"name\": \"Profile\"", "\"name\": \"Pr\\\"ofile\\nX\"");
    let case = Case::new(&recipe);
    let outcome = case.build_to("escaped.swb", true).unwrap();
    assert!(outcome.macro_bytes > 0);
    let macro_text = fs::read_to_string(case.dir.path().join("escaped.swb")).unwrap();
    // The name appears in a comment, VBA-literal escaped (doubled quotes,
    // controls flattened) and never as raw newlines inside a literal.
    assert!(macro_text.contains("Pr\"\"ofile X"));
}

// ---- output policy ----

#[test]
fn build_refuses_to_replace_without_overwrite() {
    let case = Case::new(TRACER);
    case.build_to("once.swb", false).unwrap();
    let err = case.build_to("once.swb", false).unwrap_err().to_string();
    assert!(err.contains("already exists"), "unexpected: {err}");
    case.build_to("once.swb", true).unwrap();
}

#[cfg(unix)]
#[test]
fn build_refuses_symlink_outputs() {
    let case = Case::new(TRACER);
    let target = case.dir.path().join("elsewhere.swb");
    let out = case.dir.path().join("link.swb");
    std::os::unix::fs::symlink(&target, &out).unwrap();
    let err = build::build(
        &BuildOptions {
            artifact: &case.artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: true,
        },
        &Limits::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("symbolic link"), "unexpected: {err}");
}

#[test]
fn generated_outputs_are_capped() {
    let case = Case::new(TRACER);
    let out = case.dir.path().join("tiny.swb");
    let tiny = Limits {
        max_generated_bytes: 10,
        ..Limits::default()
    };
    let err = build::build(
        &BuildOptions {
            artifact: &case.artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: false,
        },
        &tiny,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("over the limit"), "unexpected: {err}");
    assert!(!out.exists(), "nothing may be written when the cap trips");
}

#[test]
fn revolve_axis_styles_and_full_turns_replay() {
    let case = Case::new(TRACER_REVOLVE);
    case.build_to("rev.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("rev.swb")).unwrap();
    assert!(macro_text.contains("FeatureRevolve2"), "no revolve emitted");
    assert!(
        macro_text.contains("skName0"),
        "the sketch binding is missing"
    );
    assert!(
        macro_text.contains("sl0(0).Select"),
        "the axis is not its sketch line"
    );
    assert!(
        macro_text.contains("sgFix"),
        "the helper axis is not anchored"
    );
    let outcome = case.build_to("rev2.swb", false).unwrap();
    assert_eq!(outcome.plan.steps[1].status, PlanStatus::Mapped);
    assert_eq!(outcome.plan.steps[2].status, PlanStatus::Mapped);
}

#[test]
fn partial_revolve_needs_the_sketch_normal_convention() {
    let recipe = TRACER_REVOLVE.replace(
        "\"convention\": \"sketch_normal\"",
        "\"convention\": \"legacy_axis_start\"",
    );
    let steps = plan_of(&recipe);
    assert_eq!(steps[1].status, PlanStatus::FallbackPlanned);
    // The stop rule cascades: the full turn after the stopped step is
    // not-executed. Its convention-freedom is covered by the fixture test.
    assert_eq!(steps[2].status, PlanStatus::NotExecuted);
}

#[test]
fn primitives_and_cross_references_replay() {
    let case = Case::new(TRACER_PRIM);
    case.build_to("prim.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("prim.swb")).unwrap();
    // The box profile is four segments between MAPPED corners (sketch
    // geometry's own drawing calls), never CreateCornerRectangle (whose
    // returned segment order is unpinned and whose axis-aligned footprint
    // would not survive a non-permutation map).
    assert!(
        !macro_text.contains("CreateCornerRectangle"),
        "the primitive box still uses the unpinned corner-rectangle call"
    );
    let box_region = builder_region(&macro_text, "BuildPrim2");
    for corner in [(0.0, 0.0), (0.08, 0.0), (0.08, 0.02), (0.0, 0.02)] {
        let (x, y) = corner;
        let u = format!("MapU(2, {}, {})", swb::vba_num(x), swb::vba_num(y));
        let v = format!("MapV(2, {}, {})", swb::vba_num(x), swb::vba_num(y));
        assert!(
            box_region.contains(&u),
            "box: the mapped corner {u} is missing"
        );
        assert!(
            box_region.contains(&v),
            "box: the mapped corner {v} is missing"
        );
    }
    assert!(
        macro_text.contains("CreateCircleByRadius"),
        "no cylinder sketch"
    );
    // The cylinder centre maps like geometry; the radius stays the SI-metre
    // source value (the map is an isometry).
    assert!(
        macro_text.contains("CreateCircleByRadius(MapU(3, 0.0, 0.0), MapV(3, 0.0, 0.0), 0#, 0.04)"),
        "the cylinder profile is not drawn mapped at its SI radius"
    );
    assert!(
        macro_text.contains("regName(0)"),
        "the cross-reference slot is missing"
    );
    assert!(
        macro_text.contains(
            "If regName(0) = \"\" Then gMsg = \"the parameter slot 0 is not bound (its dimension was omitted or failed)\": GoTo ExFail"
        ),
        "the dead-slot guard does not name the unbound slot"
    );
    assert!(
        macro_text.contains("* 2"),
        "the segmented right-hand side is missing"
    );
    // The cross-reference equation keeps the sign contract: the sign is
    // resolved from the dimension readback and travels through
    // AddDimEquation's signVal (regSign -> SlotRef normalization).
    assert!(
        box_region.contains("signVal * 0.08 * dimScale, 1, signVal"),
        "the box length equation lost the sign contract"
    );
    let outcome = case.build_to("prim2.swb", false).unwrap();
    for step in &outcome.plan.steps {
        assert_eq!(step.status, PlanStatus::Mapped, "{}", step.detail);
    }
}

#[test]
fn primitives_off_the_base_plane_stop() {
    let recipe = TRACER_PRIM.replace(
        "\"corner\": [\"0\", \"0\", \"0\"]",
        "\"corner\": [\"0\", \"0\", \"5 mm\"]",
    );
    let steps = plan_of(&recipe);
    assert_eq!(steps[2].status, PlanStatus::FallbackPlanned);
    assert_eq!(steps[3].status, PlanStatus::NotExecuted);
}

#[test]
fn modifications_replay_with_fail_closed_picks() {
    let case = Case::new(TRACER_MOD);
    case.build_to("mod.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("mod.swb")).unwrap();
    assert!(macro_text.contains("FeatureFillet3"), "no fillet");
    assert!(macro_text.contains("FeatureShell"), "no shell");
    assert!(macro_text.contains("SimpleHole2"), "no hole");
    assert!(
        macro_text.contains("SelectByID2(\"\", \"EDGE\""),
        "edge picks are not fail-closed selections"
    );
    assert!(
        macro_text.contains("a EDGE pick did not land"),
        "pick failures are not checked"
    );
    let outcome = case.build_to("mod2.swb", false).unwrap();
    assert_eq!(outcome.plan.steps[0].status, PlanStatus::Mapped);
    assert_eq!(outcome.plan.steps[1].status, PlanStatus::Mapped);
    assert_eq!(outcome.plan.steps[2].status, PlanStatus::Mapped);
}

#[test]
fn unsupported_modification_variants_stop_or_degrade() {
    // An unequal chamfer stops.
    let recipe = TRACER_MOD.replace(
        "\"type\": \"shell\"",
        "\"distance2\": \"2 mm\", \"type\": \"chamfer\"",
    );
    let steps = plan_of(&recipe);
    assert_eq!(steps[1].status, PlanStatus::FallbackPlanned);
    assert_eq!(steps[2].status, PlanStatus::NotExecuted); // the stop cascades
                                                          // A fillet style variant degrades but still replays constant.
    let recipe = TRACER_MOD.replace(
        "\"type\": \"fillet\"",
        "\"style\": \"variable_radius\", \"type\": \"fillet\"",
    );
    let steps = plan_of(&recipe);
    assert_eq!(steps[0].status, PlanStatus::DegradePlanned);
}

#[test]
fn mirrors_replay_over_verified_native_seeds() {
    let case = Case::new(TRACER_PAT);
    case.build_to("pat.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("pat.swb")).unwrap();
    assert!(macro_text.contains("InsertMirrorFeature2"), "no mirror");
    assert!(
        macro_text.contains("If feat1 Is Nothing Then"),
        "the seed binding is not live-checked"
    );
    assert!(
        macro_text.contains("Set feat1 = feat"),
        "the seed never registers"
    );
    assert!(
        macro_text.contains("gPlane = PlaneNameFor(\"XY\")"),
        "the origin mirror plane is not probed"
    );
    assert!(
        macro_text.contains("If gPlane = \"\" Then"),
        "the empty probe result is not guarded"
    );
    assert!(
        macro_text.contains("SelectByID2(gPlane, \"PLANE\""),
        "the probe-read plane name is not selected"
    );
    let outcome = case.build_to("pat2.swb", false).unwrap();
    assert_eq!(outcome.plan.steps[2].status, PlanStatus::Mapped);
}

#[test]
fn body_patterns_and_feature_patterns_are_honest_stops() {
    // A body mirror stops.
    let recipe = TRACER_PAT.replace(
        "\"features\": [2],",
        "\"bodies\": [\"Body1\"], \"features\": [],",
    );
    let steps = plan_of(&recipe);
    assert_eq!(steps[2].status, PlanStatus::FallbackPlanned);
    // A rectangular feature pattern stops until direction selection is pinned.
    let recipe = TRACER_PAT.replace(
        "\"type\": \"mirror\"",
        "\"pattern\": {\"count1\": \"3\", \"dir1\": [1, 0, 0], \"spacing1\": \"20 mm\", \"type\": \"rectangular\"}, \"type\": \"pattern\"",
    );
    let steps = plan_of(&recipe);
    assert_eq!(steps[2].status, PlanStatus::FallbackPlanned);
}

#[test]
fn equation_free_macros_declare_their_shared_state() {
    // A recipe with no equations at all: AddDimEquation and the fallback
    // helpers still reference regName/ClearSlotsAll, so the generated module
    // must declare and define them unconditionally (plan review).
    let recipe = r#"{
      "about": { "design": "Plain", "format": "solidcraft/53", "replay": "craft-bridge build", "version": "0.40.0-rc.4" },
      "commands": [
        { "command": "document.units", "params": { "units": "mm" } },
        {
          "command": "timeline.add",
          "params": {
            "feature": {
              "id": 1,
              "name": "Profile",
              "plane": { "name": "XY", "type": "origin" },
              "sketch": {
                "constraints": [
                  { "id": "k1", "l": 0, "type": "horizontal" },
                  { "id": "k2", "l": 1, "type": "vertical" }
                ],
                "counters": { "k": 2, "l": 4, "p": 4 },
                "curves": [
                  { "a": 0, "b": 1, "construction": false, "id": "l1", "type": "line" },
                  { "a": 1, "b": 2, "construction": false, "id": "l2", "type": "line" },
                  { "a": 2, "b": 3, "construction": false, "id": "l3", "type": "line" },
                  { "a": 3, "b": 0, "construction": false, "id": "l4", "type": "line" }
                ],
                "points": [
                  { "fixed": true, "id": "origin", "pos": [0.0, 0.0] },
                  { "id": "p1", "pos": [40.0, 0.0] },
                  { "id": "p2", "pos": [40.0, 20.0] },
                  { "id": "p3", "pos": [0.0, 20.0] }
                ]
              },
              "suppressed": false,
              "type": "sketch"
            }
          }
        }
      ]
    }"#;
    let case = Case::new(recipe);
    case.build_to("plain.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("plain.swb")).unwrap();
    assert!(
        macro_text.contains("Dim regName(0 To 0) As String"),
        "regName is not declared for an equation-free recipe"
    );
    assert!(
        macro_text.contains("Sub ClearSlotsAll()"),
        "ClearSlotsAll is not defined for an equation-free recipe"
    );
    // Finding 1a: a Function closed by End Sub is a VBA compile error.
    let host_body = macro_text
        .split("Function ReportPathFromHost() As String")
        .nth(1)
        .expect("no ReportPathFromHost")
        .split("Function MacroDirFromHost()")
        .next()
        .unwrap();
    assert!(
        host_body.contains("End Function") && !host_body.contains("End Sub"),
        "ReportPathFromHost is not closed with End Function"
    );
}

#[test]
fn fallback_snapshots_produce_sidecars_and_fallback_builders() {
    let dir = tempfile::tempdir().unwrap();
    let recipe_path = dir.path().join("script.json");
    let step_path = dir.path().join("part.step");
    let artifact = dir.path().join("stop.craftx");
    fs::write(&recipe_path, TRACER_STOP).unwrap();
    fs::write(&step_path, STEP_FIXTURE).unwrap();
    pack::pack(
        &PackOptions {
            recipe: &recipe_path,
            step: &step_path,
            out: &artifact,
            warnings: None,
            name: None,
            overwrite: false,
            snapshots: vec![(1, step_path.as_path())],
            final_snapshot: None,
        },
        &Limits::default(),
    )
    .unwrap();
    let out = dir.path().join("stop.swb");
    let outcome = build::build(
        &BuildOptions {
            artifact: &artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: false,
        },
        &Limits::default(),
    )
    .unwrap();
    assert_eq!(outcome.plan.steps[1].status, PlanStatus::FallbackPlanned);
    assert!(outcome.plan.steps[1].detail.contains("boundary snapshot"));
    let macro_text = fs::read_to_string(&out).unwrap();
    assert!(macro_text.contains("Fallback1()"), "no fallback builder");
    assert!(macro_text.contains("ImportFile"), "no snapshot import");
    assert!(macro_text.contains("DeleteCreated"), "no pre-step restore");
    assert!(
        macro_text.contains("Sub ClearSlotsAll()"),
        "the slot-invalidation helper is not defined"
    );
    assert!(
        macro_text.contains("ClearSlotsAll"),
        "stale bindings are not invalidated"
    );
    assert!(dir.path().join("fallback-step-0001.step").exists());
    // A boundary fallback continues: the step after keeps replaying.
    assert!(
        macro_text.contains("BuildExtrude2()"),
        "replay stopped after a boundary fallback"
    );
}

#[test]
fn coarse_final_snapshots_terminate_replay() {
    let dir = tempfile::tempdir().unwrap();
    let recipe_path = dir.path().join("script.json");
    let step_path = dir.path().join("part.step");
    let artifact = dir.path().join("coarse.craftx");
    fs::write(&recipe_path, TRACER_STOP).unwrap();
    fs::write(&step_path, STEP_FIXTURE).unwrap();
    pack::pack(
        &PackOptions {
            recipe: &recipe_path,
            step: &step_path,
            out: &artifact,
            warnings: None,
            name: None,
            overwrite: false,
            snapshots: Vec::new(),
            final_snapshot: Some(step_path.as_path()),
        },
        &Limits::default(),
    )
    .unwrap();
    let out = dir.path().join("coarse.swb");
    let outcome = build::build(
        &BuildOptions {
            artifact: &artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: false,
        },
        &Limits::default(),
    )
    .unwrap();
    assert!(outcome.plan.steps[1].detail.contains("final snapshot"));
    let macro_text = fs::read_to_string(&out).unwrap();
    assert!(
        macro_text.contains("If st = \"fallback\" Then stopModel = True"),
        "the coarse snapshot did not terminate replay"
    );
}

#[test]
fn artifacts_without_snapshots_report_the_missing_fallback() {
    let case = Case::new(TRACER_STOP);
    let outcome = case.build_to("nosnap.swb", false).unwrap();
    assert!(outcome.plan.steps[1].detail.contains("no snapshot"));
    let macro_text = fs::read_to_string(case.dir.path().join("nosnap.swb")).unwrap();
    assert!(macro_text.contains("replay terminates here"));
    assert!(!macro_text.contains("ImportFile"));
}

// ---- plane frames: the model-to-sketch transform derivation ----

/// Build one `ModelToSketchTransform.ArrayData` from a known sketch frame
/// (u, v, normal in model coordinates) and origin offset. Rows of the
/// rotation are model axes in sketch coordinates, so its columns are the
/// sketch axes in model coordinates: element (i, j) = axis_j[i].
fn array_data_from_frame(u: [f64; 3], v: [f64; 3], n: [f64; 3], o: [f64; 3]) -> [f64; 16] {
    let axes = [u, v, n];
    let mut a = [0.0; 16];
    for i in 0..3 {
        for j in 0..3 {
            a[3 * i + j] = axes[j][i];
        }
    }
    a[9] = o[0];
    a[10] = o[1];
    a[11] = o[2];
    a[12] = 1.0;
    a
}

#[test]
fn sketch_to_model_inverts_a_translated_rotated_frame() {
    // The reviewer's RC2 catch: PublishMap stores the model->sketch
    // translation while SketchToModel needs the sketch origin in model
    // space - a different quantity on any plane that misses the origin.
    // This mirrors the emitted formula exactly and round-trips a
    // translated AND rotated frame (offset-plane shapes).
    fn frame(basis: [[f64; 3]; 3], trans: [f64; 3]) -> [f64; 16] {
        // array layout: columns are the sketch axes (u = a0,a3,a6 etc),
        // a9..11 = the model->sketch translation; 16 entries, scale 1.
        let mut a = [0.0f64; 16];
        for (j, ax) in basis.iter().enumerate() {
            a[j] = ax[0];
            a[3 + j] = ax[1];
            a[6 + j] = ax[2];
        }
        a[9] = trans[0];
        a[10] = trans[1];
        a[11] = trans[2];
        a[12] = 1.0;
        a
    }
    fn sketch_origin(a: &[f64; 16]) -> [f64; 3] {
        // mirrors the emitted gFrame(9..11): O = -(b0*u' + b1*v' + b2*n')
        let (ux, uy, uz) = (a[0], a[3], a[6]);
        let (vx, vy, vz) = (a[1], a[4], a[7]);
        let (nx, ny, nz) = (a[2], a[5], a[8]);
        [
            -(a[9] * ux + a[10] * vx + a[11] * nx),
            -(a[9] * uy + a[10] * vy + a[11] * ny),
            -(a[9] * uz + a[10] * vz + a[11] * nz),
        ]
    }
    fn model_to_sketch(a: &[f64; 16], p: [f64; 3]) -> [f64; 3] {
        let (ux, uy, uz) = (a[0], a[3], a[6]);
        let (vx, vy, vz) = (a[1], a[4], a[7]);
        let (nx, ny, nz) = (a[2], a[5], a[8]);
        [
            p[0] * ux + p[1] * uy + p[2] * uz + a[9],
            p[0] * vx + p[1] * vy + p[2] * vz + a[10],
            p[0] * nx + p[1] * ny + p[2] * nz + a[11],
        ]
    }
    fn sketch_to_model(a: &[f64; 16], s: f64, t: f64) -> [f64; 3] {
        // mirrors the emitted SketchToModelX/Y/Z exactly
        let o = sketch_origin(a);
        [
            o[0] + s * a[0] + t * a[1],
            o[1] + s * a[3] + t * a[4],
            o[2] + s * a[6] + t * a[7],
        ]
    }
    // A translated, rotated (non-axis-aligned) right-handed frame: axes are
    // rows of a rotation about z by 30 degrees, then a second tilt - built
    // orthonormal by hand: u' = (c, s, 0), v' = (-s, c, 0), n' = (0,0,1)
    // rotated again about u by 20deg (approximated values keep the test
    // honest about the algebra, not the trig).
    let (c30, s30) = ((3f64).sqrt() / 2.0, 0.5);
    let u = [c30, s30, 0.0];
    let v = [-s30, c30, 0.0];
    let n = [0.0, 0.0, 1.0];
    let trans = [0.031, -0.017, 0.042]; // a plane missing the origin
    let a = frame([u, v, n], trans);
    let o = sketch_origin(&a);
    // The sketch origin must map to (0,0,0) in sketch space.
    let o_s = model_to_sketch(&a, o);
    for (got, want) in o_s.iter().zip([0.0, 0.0, 0.0]) {
        assert!((got - want).abs() < 1e-12, "origin does not map to zero: {o_s:?}");
    }
    // Round-trip: any sketch point (s,t) -> model -> sketch gives (s,t,0).
    for (s, t) in [(0.0, 0.0), (0.03, -0.02), (-0.11, 0.07)] {
        let p = sketch_to_model(&a, s, t);
        let q = model_to_sketch(&a, p);
        assert!((q[0] - s).abs() < 1e-12, "s does not round-trip: {q:?}");
        assert!((q[1] - t).abs() < 1e-12, "t does not round-trip: {q:?}");
        assert!(q[2].abs() < 1e-12, "z must vanish in sketch space: {q:?}");
    }
    // The raw translation must NOT be the origin (the bug this fixes).
    let raw = [a[9], a[10], a[11]];
    let d2 = (0..3).map(|i| (raw[i] - o[i]).powi(2)).sum::<f64>();
    assert!(d2 > 1e-9, "test frame must separate translation from origin");
}

#[test]
fn sketch_frame_extraction_follows_the_inverse_transform() {
    use swb::{sketch_frame_from_array_data, sketch_origin_from_array_data};

    // Identity: the sketch frame is the model frame.
    let a = array_data_from_frame(
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0],
    );
    assert_eq!(
        sketch_frame_from_array_data(&a),
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    );

    // Non-symmetric axis permutation (u=Y, v=Z, n=X): the cyclic case a
    // misconfigured seat produces. The naive read of the third ROW of the
    // model-to-sketch rotation gives (0, 0, 1) here, not the true normal
    // (1, 0, 0) - the extraction must follow the columns.
    let a = array_data_from_frame(
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
    );
    let frame = sketch_frame_from_array_data(&a);
    assert_eq!(frame, [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]);
    assert_ne!(
        frame[2],
        [a[6], a[7], a[8]],
        "row read is wrong for this frame"
    );

    // Signed axes, the other cyclic permutation (u=Z, v=X, n=Y).
    let a = array_data_from_frame(
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
    );
    assert_eq!(
        sketch_frame_from_array_data(&a),
        [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
    );

    // A frame with sign flips (u=X, v=-Y, n=-Z).
    let a = array_data_from_frame(
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 0.0],
    );
    assert_eq!(
        sketch_frame_from_array_data(&a),
        [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]]
    );

    // The translation is read back as the model origin in sketch
    // coordinates: a nonzero value means verbatim coordinates would land
    // shifted, and the probe fails closed on it.
    let a = array_data_from_frame(
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.01, -0.02, 0.03],
    );
    assert_eq!(sketch_origin_from_array_data(&a), [0.01, -0.02, 0.03]);

    // A uniform scale in element 12 scales the extracted axes. This
    // normalization serves the strict-frame DIAGNOSTIC only: publication is
    // NOT scale-tolerant - PublishMap refuses any scale but 1 (pinned in
    // adaptation_machinery_is_wired_fail_closed).
    let mut a = array_data_from_frame(
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
    );
    for x in a.iter_mut().take(9) {
        *x *= 2.0;
    }
    a[12] = 2.0;
    let frame = sketch_frame_from_array_data(&a);
    let n = frame[2];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    assert_eq!([n[0] / len, n[1] / len, n[2] / len], [1.0, 0.0, 0.0]);
}

#[test]
fn solidcraft_frames_are_right_handed_unit_axes() {
    use replay::OriginPlane;
    use swb::solidcraft_frame;
    for plane in [OriginPlane::Xy, OriginPlane::Xz, OriginPlane::Yz] {
        let [u, v, n] = solidcraft_frame(plane);
        for axis in [u, v, n] {
            let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-12, "axes must be unit length");
        }
        assert!(
            u[0] * v[0] + u[1] * v[1] + u[2] * v[2] == 0.0,
            "u must be perpendicular to v"
        );
        let cross = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        assert_eq!(cross, n, "the normal must be u x v (right-handed)");
    }
    // The source-pinned table (SolidCraft solidcore/crates/solidcore-record/
    // src/plane.rs:29-34; docs/mapping.md, Orientation).
    assert_eq!(
        swb::solidcraft_frame(replay::OriginPlane::Xy),
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    );
    assert_eq!(
        swb::solidcraft_frame(replay::OriginPlane::Xz),
        [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]
    );
    assert_eq!(
        swb::solidcraft_frame(replay::OriginPlane::Yz),
        [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]
    );
}

#[test]
fn plane_probing_is_frame_checked_and_owns_its_probe_sketch() {
    let case = Case::new(TRACER);
    case.build_to("frame.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("frame.swb")).unwrap();
    // The derivation: the sketch axes are the columns of the rotation, the
    // origin is the translation.
    assert!(
        macro_text.contains("VecIs(a(0), a(3), a(6), 1#, 0#, 0#)"),
        "the u axis is not read as the first column"
    );
    assert!(
        macro_text.contains("VecIs(a(2), a(5), a(8), 0#, 0#, 1#)"),
        "the normal is not read as the third column"
    );
    assert!(
        macro_text
            .contains("Abs(a(9)) > 0.000001 Or Abs(a(10)) > 0.000001 Or Abs(a(11)) > 0.000001"),
        "the probe does not reject planes that miss the model origin"
    );
    // Probe ownership: delete exactly the created feature and verify it.
    assert!(
        macro_text.contains("Function DiscardProbeSketch"),
        "the probe sketch is not owned"
    );
    assert!(
        macro_text.contains("Sub CloseSketchIfOpen()"),
        "cleanup is not guarded by active-sketch state"
    );
    // Fail-closed messages name the observed frames.
    assert!(
        macro_text.contains("& gProbeLog &"),
        "the failure report does not name the probed frames"
    );
    // Sketch failure errors are captured before cleanup runs.
    let hard_fail = macro_text.find("HardFail:").expect("no HardFail");
    let region = &macro_text[hard_fail..];
    let capture = region
        .find("errNum = Err.Number")
        .expect("no error capture");
    let cleanup = region
        .find("CloseSketchIfOpen")
        .expect("no guarded cleanup");
    assert!(
        capture < cleanup,
        "cleanup must not run before the error is captured"
    );
}

#[test]
fn generated_macros_bootstrap_the_document_before_the_recipe() {
    let case = Case::new(TRACER);
    case.build_to("boot.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("boot.swb")).unwrap();
    let boot = macro_text
        .find("Set swApp = Application.SldWorks")
        .expect("no bootstrap");
    let first_step = macro_text.find("stepIdx = 0").expect("no recipe step 0");
    assert!(
        boot < first_step,
        "the bootstrap must precede recipe step 0"
    );
    assert!(
        macro_text.contains("swUserPreferenceStringValue_e.swDefaultTemplatePart"),
        "the template enum is not spelled fully qualified"
    );
    assert!(
        macro_text.contains("Set swSketchMgr = swDoc.SketchManager"),
        "the sketch manager is not initialized"
    );
    assert!(
        macro_text.contains("If swSketchMgr Is Nothing Then"),
        "the sketch manager initialization is not validated"
    );
    assert!(
        macro_text.contains("WriteStep stepIdx, \"document.new\", \"created\""),
        "the bootstrap is not reported to the runtime log"
    );
}

#[test]
fn checkpoint_tracks_the_verified_conventions() {
    let checkpoint = include_str!("../corpus/compat-check/minimal.swb");
    // Finding 6: the dimension is captured from the AddDimension2 return,
    // never by a name that probe-sketch creation could shift.
    assert!(
        !checkpoint.contains("D1@Sketch1"),
        "the checkpoint still assumes sketch numbering"
    );
    assert!(
        checkpoint.contains("dd.GetDimension2(0)"),
        "the checkpoint does not capture the created dimension"
    );
    // The checkpoint probes with the same frame contract as the generator.
    assert!(checkpoint.contains("Function SketchFrameIs"));
    assert!(checkpoint.contains("Function DiscardProbeSketch"));
    assert!(checkpoint.contains("VecIs(a(2), a(5), a(8)"));
}

#[test]
fn checkpoint_carries_the_map_and_classification_treatment() {
    // The batch-1 canary contract: parallel-normal acceptance with the
    // fail-closed map published, mapped geometry with solved-position
    // readback, the relation classified and its kind checked, the dimension
    // kind verified with the sign resolved, and edit-and-follow.
    let checkpoint = include_str!("../corpus/compat-check/minimal.swb");
    assert!(
        checkpoint.contains("Function PlaneIsSource"),
        "the checkpoint does not accept via parallel normal"
    );
    assert!(
        checkpoint.contains("Function AcceptAndPublish"),
        "the checkpoint does not publish the adaptation map"
    );
    assert!(
        checkpoint.contains("Function MapU(") && checkpoint.contains("Function MapV("),
        "the checkpoint does not map coordinates"
    );
    // Transactional publication.
    let cleared = checkpoint
        .find("gMapValid = False")
        .expect("no invalidation");
    let published = checkpoint.find("gMapValid = True").expect("no publication");
    assert!(
        cleared < published,
        "checkpoint publication is not transactional"
    );
    // The relation is classified and its kind checked against the geometry.
    assert!(
        checkpoint.contains("Select Case AxisClass(True)"),
        "the relation is not classified"
    );
    assert!(
        checkpoint.contains("relation omitted"),
        "a non-permutation map does not degrade the relation with a named reason"
    );
    assert!(
        checkpoint.contains("did not level the classified axis"),
        "the relation kind is not checked"
    );
    // The dimension kind is verified against the point projections and the
    // sign is resolved into the assigned value.
    assert!(
        checkpoint.contains("but the line length is"),
        "the dimension kind is not verified"
    );
    assert!(
        checkpoint.contains("signVal = 1#") && checkpoint.contains("signVal = -1#"),
        "the sign is not resolved"
    );
    assert!(
        checkpoint.contains("dp.SystemValue = signVal * 0.05"),
        "the resolved sign is not carried by the value"
    );
    // The geometry is diagonal, so every dimension kind measures a
    // different number and the kind check is discriminating; coinciding
    // magnitudes are refused, never guessed.
    assert!(
        checkpoint.contains("the projection magnitudes coincide; the kind cannot be established"),
        "the checkpoint does not refuse ambiguous dimension kinds"
    );
    assert!(
        checkpoint.contains("the value edit did not carry the geometry"),
        "the checkpoint has no behavioural kind evidence"
    );
    // Circle readback through the pinned centre handle and the pinned
    // radius member (ci.Radius raises 438; GetRadius is seat-proven,
    // probe-circlerad).
    assert!(
        checkpoint.contains("ci.GetCenterPoint2"),
        "the checkpoint does not read the circle back through the centre handle"
    );
    assert!(
        checkpoint.contains("ci.GetRadius"),
        "the checkpoint does not use the pinned GetRadius member"
    );
    // Closed-context verification before any success report.
    assert!(
        checkpoint.contains("the sketch context did not close"),
        "the checkpoint does not verify the closed context"
    );
    assert!(
        checkpoint.contains("closed-context verification failed"),
        "the checkpoint has no closed-context geometry readback"
    );
    // The toggle disable is verified by readback.
    assert!(
        checkpoint.contains("the input-dimension-value dialog could not be disabled"),
        "the checkpoint does not verify the dialog disable"
    );
    // Edit-and-follow acceptance check.
    assert!(
        checkpoint.contains("dp.SystemValue = 0.1"),
        "the checkpoint does not edit the dimension"
    );
    assert!(
        checkpoint.contains("the edited dimension did not carry the geometry"),
        "the checkpoint does not verify the edit followed"
    );
    // The blocking dialog toggle is saved, disabled and restored.
    assert!(
        checkpoint.contains("swUserPreferenceToggle_e.swInputDimValOnCreate, False"),
        "the checkpoint does not disable the input-dimension-value dialog"
    );
    assert!(
        checkpoint.contains("swUserPreferenceToggle_e.swInputDimValOnCreate, gDimToggle"),
        "the checkpoint does not restore the toggle"
    );
}

#[test]
fn probe_cleanup_covers_every_discard_path() {
    // The cleanup contract (plan review 2026-11-13): absence is proven by a
    // live tree walk, never inferred from a failed select; the name is
    // validated before any API sees it; every uncertain state fails closed.
    // These are text-level pins of the emitted VBA - the seat proves the
    // execution (docs/spec.md: goldens prove text, not behavior).
    let case = Case::new(TRACER);
    case.build_to("cleanup.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("cleanup.swb")).unwrap();
    let checkpoint = include_str!("../corpus/compat-check/minimal.swb");
    for (name, text) in [
        ("generated", macro_text.as_str()),
        ("checkpoint", checkpoint),
    ] {
        // Name/query errors: an empty or unreadable name never reaches an API,
        // and a failed walk reports uncertainty, not absence.
        assert!(
            text.contains("If nm = \"\" Then Exit Function"),
            "{name}: empty names are not rejected"
        );
        assert!(
            text.contains("uncertain = True"),
            "{name}: a failed query is not flagged uncertain"
        );
        assert!(
            text.contains("If Err.Number = 0 Then uncertain = False"),
            "{name}: only a complete walk proves absence"
        );
        // Auto-removal: closing an empty sketch may already have removed it;
        // that is clean only when absence is proven (Not uncertain).
        assert!(
            text.contains("DiscardProbeSketch = Not uncertain"),
            "{name}: auto-removal is not bounded by the proof"
        );
        // Surviving sketch + deletion failure: the delete runs on the live
        // feature and the result is re-verified, failing closed on anything
        // but proven absence.
        assert!(
            text.contains("If Not live.Select(False) Then Exit Function"),
            "{name}: the delete is not anchored to the live feature"
        );
        assert!(
            text.contains("DiscardProbeSketch = (live Is Nothing) And (Not uncertain)"),
            "{name}: the deletion is not re-verified"
        );
        assert!(
            text.contains("swDoc.ClearSelection2 True"),
            "{name}: the delete does not start from a clean selection"
        );
    }
}

/// One line of VBA must have balanced string literals. Inside a literal,
/// `""` is an escaped quote; a lone `"` closes. An unclosed literal is a
/// silent compile failure: RunMacro just returns False on the seat and no
/// report is ever written - exactly what an unterminated `"\""`  in JsonStr
/// did to every generated macro until 2026-11-13.
fn assert_vba_literals_balance(text: &str, what: &str) {
    for (i, line) in text.lines().enumerate() {
        let mut in_string = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if in_string {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next(); // "" = one literal quote
                    } else {
                        in_string = false;
                    }
                }
            } else if c == '"' {
                in_string = true;
            } else if c == '\'' {
                break; // comment: rest of line is free text
            }
        }
        assert!(
            !in_string,
            "{what}: unterminated VBA string literal at line {}: {}",
            i + 1,
            line
        );
    }
}

#[test]
fn generated_modules_have_balanced_vba_strings() {
    let case = Case::new(TRACER);
    case.build_to("strings.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("strings.swb")).unwrap();
    assert_vba_literals_balance(&macro_text, "generated");
    assert_vba_literals_balance(
        include_str!("../corpus/compat-check/minimal.swb"),
        "checkpoint",
    );
    // The specific trap: JsonStr must produce the 5-char VBA literal "\"".." ,
    // i.e. bytes 22 5c 22 22 22 - one quote more than the unterminated form.
    assert!(
        macro_text.contains("t = Replace(t, \"\"\"\", \"\\\"\"\")"),
        "JsonStr's quote escape is not the proven form"
    );
}

#[test]
fn all_generated_families_have_balanced_vba_strings() {
    for (name, recipe) in [
        ("tracer", TRACER),
        ("stop-rule", TRACER_STOP),
        ("revolve", TRACER_REVOLVE),
        ("primitives", TRACER_PRIM),
        ("modifications", TRACER_MOD),
        ("patterns", TRACER_PAT),
    ] {
        let case = Case::new(recipe);
        case.build_to("strings.swb", false).unwrap();
        let macro_text = fs::read_to_string(case.dir.path().join("strings.swb")).unwrap();
        assert_vba_literals_balance(&macro_text, name);
    }
}

#[test]
fn xz_frame_predicate_is_pinned_with_signs() {
    let case = Case::new(TRACER);
    case.build_to("xzpin.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("xzpin.swb")).unwrap();
    // SolidCraft XZ is u=+X, v=-Z, n=+Y (plane.rs:29-34): the v column
    // reads minus Z and the normal plus Y. Both signs are pinned in the
    // generated module and in the seat checkpoint alike.
    let pinned = "VecIs(a(1), a(4), a(7), 0#, 0#, -1#) And VecIs(a(2), a(5), a(8), 0#, 1#, 0#)";
    assert!(
        macro_text.contains(pinned),
        "the generated XZ predicate is not the source-pinned frame"
    );
    assert!(
        include_str!("../corpus/compat-check/minimal.swb").contains(pinned),
        "the checkpoint XZ predicate is not the source-pinned frame"
    );
    let wrong = "VecIs(a(1), a(4), a(7), 0#, 0#, 1#) And VecIs(a(2), a(5), a(8), 0#, -1#, 0#)";
    assert!(
        !macro_text.contains(wrong),
        "the old wrong XZ frame survived"
    );
    assert!(!include_str!("../corpus/compat-check/minimal.swb").contains(wrong));
}

#[test]
fn xz_revolve_axes_map_world_z_and_lines_with_v_along_minus_z() {
    // World +Z is the sketch -v direction on XZ (v = -Z, plane.rs:29-34)
    // and a world line keeps u = x but takes v = -z: origin (3, 0, 5) lands
    // at (3, -5). Asymmetric on purpose - the old v = +Z mapping is
    // indistinguishable on symmetric inputs.
    let world = TRACER_REVOLVE
        .replace("\"name\": \"XY\"", "\"name\": \"XZ\"")
        .replace("\"curve\": \"l0\"", "\"axis\": \"z\"")
        .replace("\"type\": \"sketch_line\"", "\"type\": \"world\"");
    let steps = plan_of(&world);
    match &steps[1].work {
        replay::Work::Revolve(rv) => assert_eq!(
            rv.axis,
            replay::AxisSel::Emitted {
                origin: [0.0, 0.0],
                dir: [0.0, -1.0]
            }
        ),
        other => panic!("not a revolve: {other:?}"),
    }
    let line = TRACER_REVOLVE
        .replace("\"name\": \"XY\"", "\"name\": \"XZ\"")
        .replace(
            "\"curve\": \"l0\"",
            "\"origin\": [\"3\", \"0\", \"5\"], \"dir\": [0, 0, 1]",
        )
        .replace("\"type\": \"sketch_line\"", "\"type\": \"line\"");
    let steps = plan_of(&line);
    match &steps[1].work {
        replay::Work::Revolve(rv) => assert_eq!(
            rv.axis,
            replay::AxisSel::Emitted {
                origin: [3.0, -5.0],
                dir: [0.0, -1.0]
            }
        ),
        other => panic!("not a revolve: {other:?}"),
    }
}

#[test]
fn equations_use_the_seat_proven_add_shape_and_read_back() {
    let case = Case::new(TRACER);
    case.build_to("eqshape.swb", false).unwrap();
    let macro_text = fs::read_to_string(case.dir.path().join("eqshape.swb")).unwrap();
    // Seat evidence (rev 34.3.2): Add2 with the documented two arguments is
    // rejected at runtime ("Argument not optional"); the two-argument classic
    // Add works and its returned index is validated and read back. Each
    // helper is checked on its own - a whole-module search would let the
    // global helper carry the dimension helper's checks.
    assert!(
        !macro_text.contains("eqMgr.Add2"),
        "the unproven Add2 call survived"
    );
    for helper in ["AddGlobalEquation", "AddDimEquation"] {
        let start = macro_text
            .find(&format!("Function {helper}("))
            .unwrap_or_else(|| panic!("{helper} is missing"));
        let end = macro_text[start..]
            .find("End Function")
            .expect("the helper ends")
            + start;
        let body = &macro_text[start..end];
        assert!(
            body.contains("idx = eqMgr.Add(-1, eqText)"),
            "{helper}: no seat-proven Add call"
        );
        assert!(
            body.contains("If idx < 0 Then"),
            "{helper}: the insert index lower bound is not validated"
        );
        // The Equation(idx) readback below IS the upper-bound check: an
        // out-of-range index raises there and lands in EqFail. The manager's
        // Count member is unsupported on this seat (eqdiag: err 438).
        assert!(
            !body.contains("eqMgr.Count"),
            "{helper}: the unsupported Count member survived"
        );
        assert!(
            body.contains("eqMgr.Equation(idx) <> eqText"),
            "{helper}: the stored equation is not read back"
        );
    }
    let gstart = macro_text.find("Function AddGlobalEquation(").unwrap();
    let gbody = &macro_text[gstart..macro_text[gstart..].find("End Function").unwrap() + gstart];
    assert!(
        gbody.contains("eqMgr.Value(idx)"),
        "the global equation value is not checked"
    );
    assert!(
        gbody.contains("eqMgr.Value(idx) - expect * evalScale"),
        "the global value check does not use the carried evaluator scale"
    );
    assert!(
        gbody.contains("If evalScale <= 0 Then"),
        "an unestablished evaluator scale is not refused"
    );
    let dstart = macro_text.find("Function AddDimEquation(").unwrap();
    let dbody = &macro_text[dstart..macro_text[dstart..].find("End Function").unwrap() + dstart];
    assert!(
        dbody.contains("dm.SystemValue - expect"),
        "the driven dimension value is not checked"
    );
}

#[test]
fn report_path_prefers_the_fresh_path_file_over_the_environment() {
    // A long-lived SolidWorks process can carry a stale CRAFT_BRIDGE_REPORT
    // inherited from whatever launched it; the path file is rewritten before
    // every run and must win. Environment is the fallback only.
    let case = Case::new(TRACER);
    case.build_to("routing.swb", false).unwrap();
    let generated = fs::read_to_string(case.dir.path().join("routing.swb")).unwrap();
    for (name, text) in [
        ("generated", generated.as_str()),
        (
            "checkpoint",
            include_str!("../corpus/compat-check/minimal.swb"),
        ),
    ] {
        let start = text
            .find("Function ReportPathFromHost")
            .expect("the function exists");
        let end = text[start..]
            .find("End Function")
            .expect("the function ends")
            + start;
        let body = &text[start..end];
        let file = body
            .find("craft-bridge-report-path.txt")
            .unwrap_or_else(|| panic!("{name}: no path-file read"));
        let env = body
            .find("CRAFT_BRIDGE_REPORT")
            .unwrap_or_else(|| panic!("{name}: no environment fallback"));
        assert!(
            file < env,
            "{name}: the environment wins over the fresh path file"
        );
    }
}

#[test]
fn runs_close_their_own_document_by_coordinated_sequencing() {
    // The owner rule: runs leave no documents open - by coordinated
    // sequencing, never a close-all. Smoke runs carry a close-after-run
    // authorization (consumed after the run) and the macro closes its own
    // document; harness runs keep the part open and check-part.vbs closes
    // exactly the run-bound handed-off title after the checks.
    let case = Case::new(TRACER);
    case.build_to("closecheck.swb", false).unwrap();
    let generated = fs::read_to_string(case.dir.path().join("closecheck.swb")).unwrap();
    for (name, text) in [
        ("generated", generated.as_str()),
        (
            "checkpoint",
            include_str!("../corpus/compat-check/minimal.swb"),
        ),
    ] {
        assert!(
            text.contains("Sub CloseOwnedDoc(ByRef closedOk As Boolean)"),
            "{name}: no owned-document closer"
        );
        assert!(
            text.contains("Function CloseMode()"),
            "{name}: no close-mode reader"
        );
        assert!(
            text.contains("Function OwnedDocTitle()"),
            "{name}: no owned-title reporter"
        );
        assert!(
            text.contains("If cnt <> 1 Or Not isOurs Then Exit Sub"),
            "{name}: no pre-close ownership check"
        );
    }
    let smoke = include_str!("../tools/seat-smoke.ps1");
    assert!(
        smoke.contains("\"close-after-run\""),
        "the smoke run does not carry the close authorization"
    );
    assert!(
        smoke.contains("Set-Content -Path $pathFile -Value @($reportPath, $macroDir)"),
        "the close authorization is not consumed"
    );
    assert!(
        smoke.contains("} finally {"),
        "the consumption is not on every exit path"
    );
    let checker = include_str!("../tools/check-part.vbs");
    assert!(
        checker.contains("ownedTitle = WScript.Arguments(3)"),
        "the checker takes no run-bound handoff"
    );
    assert!(
        checker.contains("closeTitle = ownedTitle"),
        "the checker does not close the handed-off title"
    );
    assert!(
        checker.contains("the active document is not the handed-off owned document"),
        "the checker does not verify the handoff before checking"
    );
    let harness = include_str!("../tools/seat-harness.ps1");
    assert!(
        harness.contains("\"command\":\"owned-document\""),
        "the harness does not read the handoff"
    );
}

// ---- Phase 1 batch 1: frame adaptation (offline synthetic coverage) ----

/// Mirrors the emitted AxisClass VBA exactly: the SC u direction maps to
/// sketch (s,t) as (gMap00, gMap01) and the SC v direction as (gMap10,
/// gMap11); a signed permutation lands axis-aligned (0 = s axis, 1 = t
/// axis), anything else is -1 and degrades with a named reason.
fn axis_class(m: [f64; 4], is_horiz: bool) -> i32 {
    let (c0, c1) = if is_horiz { (m[0], m[1]) } else { (m[2], m[3]) };
    if (c0.abs() - 1.0).abs() <= 1e-6 && c1.abs() <= 1e-6 {
        0
    } else if (c1.abs() - 1.0).abs() <= 1e-6 && c0.abs() <= 1e-6 {
        1
    } else {
        -1
    }
}

/// Mirrors the emitted AxisSign VBA exactly.
fn axis_sign(m: [f64; 4], is_horiz: bool, ax: i32) -> f64 {
    let c = match (is_horiz, ax) {
        (true, 0) => m[0],
        (true, _) => m[1],
        (false, 0) => m[2],
        (false, _) => m[3],
    };
    if c < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// Mirrors the emitted MapU/MapV VBA exactly: s = u*gMap00 + v*gMap10,
/// t = u*gMap01 + v*gMap11.
fn map_uv(m: [f64; 4], u: f64, v: f64) -> (f64, f64) {
    (u * m[0] + v * m[2], u * m[1] + v * m[3])
}

#[test]
fn map_classifies_signed_permutations_with_signs() {
    // [gMap00, gMap01, gMap10, gMap11]; columns = where SC u and SC v land.
    let cases: [([f64; 4], &str); 6] = [
        ([1.0, 0.0, 0.0, 1.0], "identity"),
        ([0.0, 1.0, -1.0, 0.0], "quarter turn"),
        ([-1.0, 0.0, 0.0, 1.0], "u reflected"),
        ([1.0, 0.0, 0.0, -1.0], "v reflected"),
        ([0.0, -1.0, 1.0, 0.0], "opposite quarter turn"),
        ([-1.0, 0.0, 0.0, -1.0], "half turn"),
    ];
    for (m, name) in cases {
        let (cu, cv) = (axis_class(m, true), axis_class(m, false));
        assert!(cu == 0 || cu == 1, "{name}: u is not axis-aligned");
        assert!(cv == 0 || cv == 1, "{name}: v is not axis-aligned");
        // A permutation maps u and v onto DIFFERENT sketch axes.
        assert_ne!(cu, cv, "{name}: u and v land on the same axis");
        // The sign classification recovers the mapped displacement exactly:
        // whatever the sign, the equation RHS sign times the source value is
        // what the classified-axis projection measures.
        for (u, v) in [(0.037, -0.011), (-0.02, 0.045)] {
            let (s, t) = map_uv(m, u, v);
            let proj = if cu == 0 { s } else { t };
            let sgn = axis_sign(m, true, cu);
            assert!(
                (sgn * u - proj).abs() < 1e-12,
                "{name}: sign-resolved u does not match the classified projection"
            );
            let proj_v = if cv == 0 { s } else { t };
            let sgn_v = axis_sign(m, false, cv);
            assert!(
                (sgn_v * v - proj_v).abs() < 1e-12,
                "{name}: sign-resolved v does not match the classified projection"
            );
        }
    }
}

#[test]
fn map_refuses_non_permutation_for_axis_work_only() {
    // A 45-degree map: geometry still maps (isometry), but no source axis is
    // axis-aligned - every axis-dependent relation and dimension must
    // degrade (class -1), never guess.
    let q = std::f64::consts::FRAC_1_SQRT_2;
    let m = [q, q, -q, q];
    assert_eq!(axis_class(m, true), -1, "u must not classify");
    assert_eq!(axis_class(m, false), -1, "v must not classify");
    // The coordinate map itself stays well-defined for geometry replay.
    let (s, t) = map_uv(m, 1.0, 0.0);
    assert!((s - q).abs() < 1e-12 && (t - q).abs() < 1e-12);
}

#[test]
fn adaptation_machinery_is_wired_fail_closed() {
    let case = Case::new(TRACER);
    case.build_to("adapt.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("adapt.swb")).unwrap();
    // Transactional publication: validity is cleared before any probe and
    // set only after complete validation.
    let cleared = text
        .find("gMapValid(idx) = False")
        .expect("no invalidation");
    let published = text.find("gMapValid(idx) = True").expect("no publication");
    assert!(cleared < published, "publication is not transactional");
    for pin in [
        "scale is ",
        "is not unit",
        "not orthogonal",
        "the 3D frame is not right-handed",
    ] {
        assert!(text.contains(pin), "frame validation lost: {pin}");
    }
    // Acceptance: translation at the model origin and parallel normal.
    assert!(
        text.contains("not the model origin"),
        "acceptance does not refuse translated probe sketches"
    );
    assert!(
        text.contains("Function PlaneIsSource"),
        "no acceptance predicate"
    );
    // Strict equality remains as a diagnostic only.
    assert!(
        text.contains("DIAGNOSTIC ONLY"),
        "the strict predicate lost its retired marking"
    );
    // Per-sketch map state: two sketches keep independent maps (each step
    // publishes and clears its own slot).
    assert!(
        text.contains("Dim gMap00(0 To ") && text.contains("gMap00(idx)"),
        "map state is not indexed per sketch slot"
    );
    // Deferred consumers keep their batch boundaries in their own builders,
    // before any feature is created: after batch 3 primitives run through
    // the adaptation map (validity + signed-permutation class) and the
    // direction truth table's orientation cell (probe-primorient), while
    // extrude and revolve run through the direction truth table on any
    // valid map and guard on map VALIDITY instead. The retired identity
    // guard is gone from the whole macro.
    assert!(
        !text.contains("MapIsIdentity"),
        "the retired identity-map guard survived anywhere in the macro"
    );
    {
        let case = Case::new(TRACER_PRIM);
        case.build_to("guard.swb", false).unwrap();
        let gtext = fs::read_to_string(case.dir.path().join("guard.swb")).unwrap();
        let region = builder_region(&gtext, "BuildPrim2");
        assert!(
            region.contains("gMapValid(2)"),
            "BuildPrim: no map-validity guard"
        );
        assert!(
            region.contains("MapIsPermutation(2)"),
            "BuildPrim: no signed-permutation map-class guard"
        );
        assert!(
            region.contains("TT_PRIM_DIR_TRUE_SIDE = 0"),
            "BuildPrim: no orientation-cell gate"
        );
        assert!(
            region.contains("probe-primorient"),
            "BuildPrim: the orientation refusal does not name its probe"
        );
        assert!(
            !region.contains("MapIsIdentity("),
            "BuildPrim: the retired identity guard survived"
        );
    }
    for (fam, recipe) in [("BuildExtrude", TRACER), ("BuildRevolve", TRACER_REVOLVE)] {
        let case = Case::new(recipe);
        case.build_to("guard.swb", false).unwrap();
        let gtext = fs::read_to_string(case.dir.path().join("guard.swb")).unwrap();
        let i = gtext
            .find(&format!("Function {fam}"))
            .unwrap_or_else(|| panic!("no {fam} builder"));
        let region = &gtext[i..i + 1200.min(gtext.len() - i)];
        assert!(
            region.contains("gMapValid("),
            "{fam}: no map-validity guard"
        );
        assert!(
            !region.contains("MapIsIdentity("),
            "{fam}: the retired identity guard survived"
        );
    }
    // The toggle dialog that blocks AddDimension2 is disabled and restored.
    assert!(
        text.contains("gDimToggleSaved = True")
            && text.contains("swUserPreferenceToggle_e.swInputDimValOnCreate, False"),
        "the input-dimension-value toggle is not saved and disabled"
    );
    assert!(
        text.contains("swUserPreferenceToggle_e.swInputDimValOnCreate, gDimToggle"),
        "the input-dimension-value toggle is not restored at exit"
    );
}

#[test]
fn sketch_recreation_rebinds_and_reprobes() {
    let case = Case::new(TRACER);
    case.build_to("recreate.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("recreate.swb")).unwrap();
    let i = text.find("Function CleanSketch2").expect("no CleanSketch2");
    let region = &text[i..i + 4200.min(text.len() - i)];
    // Invalidate before deletion.
    let clear = region.find("ClearMap 2").expect("no map invalidation");
    let delete = region.find("EditDelete").expect("no deletion");
    assert!(clear < delete, "the map is not invalidated before deletion");
    // Dimension slots are cleared so the redraw rebinds them.
    assert!(
        region.contains("regName(0) = \"\""),
        "slot 0 is not cleared"
    );
    assert!(
        region.contains("regName(1) = \"\""),
        "slot 1 is not cleared"
    );
    // The deletion is verified through the pinned point enumeration, and
    // only a PROVEN-EMPTY collection counts: a failed query never proves
    // emptiness (REJECT regression).
    assert!(
        region.contains("GetSketchPoints2") && region.contains("points remain"),
        "deletion is not verified against the point enumeration"
    );
    assert!(
        region.contains("the point collection could not be read after deletion"),
        "a failed point query is not refused"
    );
    assert!(
        region.contains("unexpected non-array value"),
        "an unexpected non-array point collection is not refused"
    );
    assert!(
        region.contains("If Not IsEmpty(pts) Then"),
        "the seat-proven empty signature is not the only accepted non-array case"
    );
    // The frame is re-probed and republished before any redraw.
    assert!(
        region.contains("AcceptAndPublish(2, \"XY\", gProbeReason)"),
        "the frame is not republished after recreation"
    );
    // The sketch identity is reacquired, never assumed.
    assert!(
        region.contains("skName2 = skFeat2.Name"),
        "the sketch identity is not reacquired"
    );
    // The degrade path fails closed when recreation fails.
    assert!(
        text.contains("If Not CleanSketch2() Then GoTo SketchFailed"),
        "recreation refusal does not fail the sketch"
    );
    assert!(
        text.contains("gMsg = gCause &"),
        "the recreation refusal loses the original failure cause"
    );
    // Omission is terminating: recorded with a named reason and never
    // re-attempted as a failure (the class check is deterministic).
    assert!(
        text.contains("gOmit = gOmit &"),
        "omitted work does not carry a named reason"
    );
}

#[test]
fn dims_verify_kind_and_resolve_signs_in_the_equation() {
    let case = Case::new(TRACER);
    case.build_to("dims.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("dims.swb")).unwrap();
    let i = text.find("Function Dims2").expect("no Dims2");
    let region = &text[i..i + 4000.min(text.len() - i)];
    // Kind verification is observable: the created value is matched against
    // the point projections.
    assert!(
        region.contains("created = dm.SystemValue"),
        "the created value is not read back"
    );
    assert!(
        region.contains("but the line length is"),
        "the length kind is not verified against the projection"
    );
    // Sign resolution covers both signs and fails closed on neither.
    assert!(region.contains("signVal = 1#") && region.contains("signVal = -1#"));
    assert!(region.contains("but the source value is"));
    // The sign lives in the equation RHS and the expected value - never only
    // in the initial SystemValue - so parameter edits keep tracking.
    assert!(
        region.contains("AddDimEquation(dm, \"40mm\", signVal * 0.04 * dimScale, 0, signVal, skName2, dimScale)"),
        "the equation does not carry the runtime sign"
    );
    assert!(
        text.contains("\"=\"") && text.contains("-1 * ("),
        "AddDimEquation does not fold the sign into the stored equation text"
    );
    // Qualified names: the short D1 is ambiguous across sketches (the
    // evidence round: eqMgr.Add rejects it in two-sketch items).
    assert!(
        text.contains("qName = qName &") || text.contains("qName & \"@\""),
        "dimension equations do not qualify the dimension name"
    );
}

#[test]
fn plane_name_for_keeps_its_own_capture_only() {
    // [F6]: PlaneNameFor probes must not disturb persistent sketch state -
    // no map publication inside it, ever.
    let case = Case::new(TRACER);
    case.build_to("pnf.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("pnf.swb")).unwrap();
    let i = text.find("Function PlaneNameFor").expect("no PlaneNameFor");
    let j = text[i..].find("End Function").unwrap() + i;
    let region = &text[i..j];
    assert!(
        !region.contains("AcceptAndPublish"),
        "PlaneNameFor publishes map state"
    );
    assert!(
        !region.contains("gMapValid"),
        "PlaneNameFor touches persistent map state"
    );
    assert!(
        region.contains("DiscardProbeSketch"),
        "PlaneNameFor does not own its probe sketch"
    );
}

#[test]
fn emitted_vba_has_no_executable_residue() {
    // REJECT regression: a stray backslash before a comment is not a VBA
    // comment - it is executable text and a silent compile failure. Any
    // backslash outside a string literal or comment is residue.
    for (name, recipe) in [
        ("tracer", TRACER),
        ("stop-rule", TRACER_STOP),
        ("revolve", TRACER_REVOLVE),
        ("primitives", TRACER_PRIM),
        ("modifications", TRACER_MOD),
        ("patterns", TRACER_PAT),
    ] {
        let case = Case::new(recipe);
        case.build_to("residue.swb", false).unwrap();
        let text = fs::read_to_string(case.dir.path().join("residue.swb")).unwrap();
        assert_no_executable_residue(&text, name);
    }
    assert_no_executable_residue(
        include_str!("../corpus/compat-check/minimal.swb"),
        "checkpoint",
    );
}

fn code_only(line: &str) -> String {
    let mut code = String::new();
    let mut in_string = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_string {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                } else {
                    in_string = false;
                }
            }
        } else if c == '"' {
            in_string = true;
        } else if c == '\'' {
            break;
        } else {
            code.push(c);
        }
    }
    code
}

fn assert_no_executable_residue(text: &str, what: &str) {
    for (i, line) in text.lines().enumerate() {
        let code = code_only(line);
        assert!(
            !code.contains('\\'),
            "{what}: executable residue at line {}: {}",
            i + 1,
            line
        );
    }
}

#[test]
fn module_state_is_declared_before_use() {
    // REJECT regression: gFrame, gOmit, gDimToggle* were used but never
    // declared - an Option Explicit compile failure that substring tests
    // could not see. Every g-prefixed state identifier must be declared.
    for (name, recipe) in [("tracer", TRACER), ("primitives", TRACER_PRIM)] {
        let case = Case::new(recipe);
        case.build_to("decls.swb", false).unwrap();
        let text = fs::read_to_string(case.dir.path().join("decls.swb")).unwrap();
        assert_state_declared(&text, name);
    }
    assert_state_declared(
        include_str!("../corpus/compat-check/minimal.swb"),
        "checkpoint",
    );
}

fn assert_state_declared(text: &str, what: &str) {
    let mut declared: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in text.lines() {
        let t = line.trim();
        let up = t.to_uppercase();
        if up.starts_with("DIM ") || up.starts_with("CONST ") {
            for part in t[4..].split(',') {
                let name = part
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .split('(')
                    .next()
                    .unwrap_or("");
                if !name.is_empty() {
                    declared.insert(name.to_lowercase());
                }
            }
        }
    }
    for (i, line) in text.lines().enumerate() {
        let code = code_only(line);
        let mut ident = String::new();
        for c in code.chars().chain(std::iter::once(' ')) {
            if c.is_ascii_alphanumeric() || c == '_' {
                ident.push(c);
            } else {
                if ident.starts_with('g')
                    && ident.len() > 1
                    && ident.as_bytes()[1].is_ascii_uppercase()
                {
                    assert!(
                        declared.contains(&ident.to_lowercase()),
                        "{what}: {ident} is used at line {} but never declared",
                        i + 1
                    );
                }
                ident.clear();
            }
        }
    }
}

#[test]
fn distance_dims_refuse_ambiguous_kinds_and_verify_behaviour() {
    // A distance_x dimension on the tracer rectangle: the classified-axis
    // kind must refuse coinciding projection magnitudes and prove itself
    // behaviourally - setting the value must move exactly that axis.
    let recipe = TRACER.replace(
        "{ \"id\": \"k5\", \"l\": 0, \"param\": \"d1\", \"type\": \"length\", \"value\": 40.0 }",
        "{ \"id\": \"k5\", \"p\": 0, \"q\": 1, \"param\": \"d1\", \"type\": \"distance_x\", \"value\": 40.0 }",
    );
    let case = Case::new(&recipe);
    case.build_to("distx.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("distx.swb")).unwrap();
    assert!(
        text.contains("the projection magnitudes coincide; the kind cannot be established"),
        "an ambiguous dimension kind is not refused"
    );
    assert!(
        text.contains("the value edit moved the perpendicular axis"),
        "a wrong-kind dimension is not caught behaviourally"
    );
    assert!(
        text.contains("classified-axis"),
        "the classified-axis behavioural check is missing"
    );
    // The omission path for non-permutation maps is named and terminating.
    assert!(
        text.contains("non-permutation map"),
        "the omission path lost its named reason"
    );
}

#[test]
fn extrude_calls_use_the_full_signature_and_construction_geometry() {
    // Seat-proven (2026-11-16): FeatureExtrusion2 takes 23 arguments -
    // T0/StartOffset/FlipStartOffset are NOT optional (err 449 otherwise);
    // and the construction member is ConstructionGeometry (.Construction
    // raises 438).
    for (name, recipe) in [("tracer", TRACER), ("primitives", TRACER_PRIM)] {
        let case = Case::new(recipe);
        case.build_to("arity.swb", false).unwrap();
        let text = fs::read_to_string(case.dir.path().join("arity.swb")).unwrap();
        for line in text.lines() {
            if line.contains("Set feat = gFeatMgr.FeatureExtrusion2") {
                assert!(
                    line.contains(", 0, 0#, False)"),
                    "{name}: FeatureExtrusion2 call is missing T0/StartOffset/FlipStartOffset"
                );
            }
        }
        assert!(
            !text.contains(".Construction = True"),
            "{name}: the construction member is not ConstructionGeometry"
        );
    }
}

#[test]
fn adddimequation_calls_pass_exactly_seven_arguments() {
    // Compile-error regression (twice burned): AddDimEquation gained the
    // qualifier and the dimension scale; any call site left at the old
    // arity is a silent VBA compile failure (RunMacro returns failure with
    // no report at all).
    fn count_args(call: &str) -> usize {
        let inner = &call[call.find('(').unwrap() + 1..call.rfind(')').unwrap()];
        let mut depth = 0usize;
        let mut commas = 0usize;
        for c in inner.chars() {
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => commas += 1,
                _ => {}
            }
        }
        commas + 1
    }
    for (name, recipe) in [
        ("tracer", TRACER),
        ("primitives", TRACER_PRIM),
        ("revolve", TRACER_REVOLVE),
    ] {
        let case = Case::new(recipe);
        case.build_to("arity2.swb", false).unwrap();
        let text = fs::read_to_string(case.dir.path().join("arity2.swb")).unwrap();
        for line in text.lines() {
            if line.contains("AddDimEquation(dm, ") {
                assert_eq!(
                    count_args(line.split("AddDimEquation").nth(1).unwrap()),
                    7,
                    "{name}: AddDimEquation call has the wrong arity: {line}"
                );
            }
        }
    }
    for item in [
        include_str!("../corpus/frame-two-maps/item.swb"),
        include_str!("../corpus/frame-sign-chain/item.swb"),
        include_str!("../corpus/primitives-crossrefs/item.swb"),
    ] {
        for line in item.lines() {
            if line.contains("AddDimEquation(dm, ") {
                assert_eq!(
                    count_args(line.split("AddDimEquation").nth(1).unwrap()),
                    7,
                    "corpus item: AddDimEquation call has the wrong arity: {line}"
                );
            }
        }
    }
}

#[test]
fn no_identifier_shadows_a_vba_intrinsic() {
    // Seat-proven compile rule: a VBA intrinsic name cannot be an
    // identifier - `Dim sgn As Double` is a compile error that RunMacro
    // reports only as a bare failure with no report (bisected 2026-11-16).
    for (name, recipe) in [
        ("tracer", TRACER),
        ("stop-rule", TRACER_STOP),
        ("revolve", TRACER_REVOLVE),
        ("primitives", TRACER_PRIM),
        ("modifications", TRACER_MOD),
        ("patterns", TRACER_PAT),
    ] {
        let case = Case::new(recipe);
        case.build_to("intrinsics.swb", false).unwrap();
        let text = fs::read_to_string(case.dir.path().join("intrinsics.swb")).unwrap();
        assert_no_intrinsic_shadowing(&text, name);
    }
    assert_no_intrinsic_shadowing(
        include_str!("../corpus/compat-check/minimal.swb"),
        "checkpoint",
    );
}

fn assert_no_intrinsic_shadowing(text: &str, what: &str) {
    const INTRINSICS: &[&str] = &[
        "sgn", "abs", "val", "mid", "left", "right", "len", "asc", "chr", "instr", "date", "time",
        "now", "timer", "format", "string", "array", "filter", "split", "join", "lcase", "ucase",
        "trim", "str", "fix", "int", "round", "sin", "cos", "tan", "log", "exp", "hex", "oct",
        "rgb", "environ", "shell", "name", "kill", "width", "spc", "tab", "stop", "line", "open",
        "close", "input", "print",
    ];
    let check = |name: &str, at: usize| {
        assert!(
            !INTRINSICS.contains(&name.to_lowercase().as_str()),
            "{what}: identifier {name} shadows a VBA intrinsic (line {at})"
        );
    };
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        let up = t.to_uppercase();
        if up.starts_with("DIM ") || up.starts_with("CONST ") {
            // Strip the keyword by its own length: a fixed offset is wrong
            // for Const (it would register "T ..." as the name).
            let rest = match t.find(' ') {
                Some(sp) => &t[sp..],
                None => "",
            };
            for part in rest.split(',') {
                if let Some(m) = part.split_whitespace().next() {
                    let name = m.split('(').next().unwrap_or("");
                    if !name.is_empty() {
                        check(name, i + 1);
                    }
                }
            }
        }
        for kw in ["FUNCTION ", "SUB "] {
            if up.starts_with(kw) {
                if let Some(m) = t[kw.len()..].split('(').next() {
                    let name = m.trim();
                    if !name.is_empty() {
                        check(name, i + 1);
                    }
                }
                if let Some(plist) = t.split('(').nth(1) {
                    for p in plist.split(',') {
                        // The parameter NAME is the first non-keyword token;
                        // the type follows "As".
                        for tok in p.split_whitespace() {
                            let low = tok.to_lowercase();
                            if low == "byval"
                                || low == "byref"
                                || low == "optional"
                                || low == "paramarray"
                            {
                                continue;
                            }
                            let name = tok.trim_end_matches(')');
                            if !name.is_empty() {
                                check(name, i + 1);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Structural scan over emitted VBA: substring tests cannot catch a fused
/// statement line (a lost newline turns a block If into a single-line one
/// and orphans the later End If) or a duplicate declaration in a scope.
fn assert_vba_structure(text: &str, what: &str) {
    let mut block_ifs = 0i32;
    let mut end_ifs = 0i32;
    let mut procs = 0i32;
    let mut end_procs = 0i32;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let proc_names: Vec<String> = text
        .lines()
        .filter_map(|l| {
            let u = l.trim().to_uppercase();
            if u.starts_with("FUNCTION ") || u.starts_with("SUB ") {
                Some(
                    l.split_whitespace()
                        .nth(1)
                        .unwrap_or("")
                        .split('(')
                        .next()
                        .unwrap_or("")
                        .to_string(),
                )
            } else {
                None
            }
        })
        .collect();
    for line in text.lines() {
        let mut code = String::new();
        let mut in_string = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if in_string {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                    } else {
                        in_string = false;
                    }
                }
            } else if c == '"' {
                in_string = true;
            } else if c == '\'' {
                break;
            } else {
                code.push(c);
            }
        }
        let t = code.trim();
        let up = t.to_uppercase();
        if up.starts_with("FUNCTION ") || up.starts_with("SUB ") {
            procs += 1;
            seen.clear();
        }
        if up.starts_with("END FUNCTION") || up.starts_with("END SUB") {
            end_procs += 1;
            seen.clear();
        }
        for decl in ["DIM ", "CONST "] {
            if up.starts_with(decl) {
                for part in t[decl.len()..].split(',') {
                    let name = part.split_whitespace().next().unwrap_or("").to_lowercase();
                    if !name.is_empty() && !seen.insert(name.clone()) {
                        panic!("{what}: duplicate declaration of {name}");
                    }
                }
            }
        }
        // Case-insensitive shadowing: VBA identifiers fold case, so a local
        // named like a module procedure makes calls to it unresolvable.
        for proc in &proc_names {
            if seen.contains(&proc.to_lowercase()) {
                panic!("{what}: a local name shadows the procedure {proc}");
            }
        }
        // A bare "Name()" statement is not a valid VBA procedure call:
        // parens require Call or an assignment context. The grammar error
        // is invisible to every balance scan - it failed a seat probe.
        if t.ends_with("()") && !t.contains(' ') && !t.contains('=') {
            panic!("{what}: bare parenthesized Sub call is invalid VBA: {t}");
        }
        if up.starts_with("IF ") && up.ends_with("THEN") {
            block_ifs += 1;
        }
        if up.starts_with("END IF") {
            end_ifs += 1;
        }
    }
    assert_eq!(procs, end_procs, "{what}: unbalanced Function/Sub blocks");
    assert_eq!(
        block_ifs, end_ifs,
        "{what}: block-If/End-If imbalance - fused or orphaned statements"
    );
}

#[test]
fn emitted_modules_are_structurally_sound() {
    for recipe in [
        TRACER,
        TRACER_STOP,
        TRACER_REVOLVE,
        TRACER_PRIM,
        TRACER_MOD,
        TRACER_PAT,
    ] {
        let case = Case::new(recipe);
        case.build_to("struct.swb", false).unwrap();
        let text = fs::read_to_string(case.dir.path().join("struct.swb")).unwrap();
        assert_vba_structure(&text, "generated family");
    }
    assert_vba_structure(
        include_str!("../corpus/compat-check/minimal.swb"),
        "checkpoint",
    );
}

/// Mirrors the emitted CloseMode VBA semantics line by line - the one-line
/// counterexample must never authorize a close, and a clean two-line EOF
/// means keep-open. The emitted-text assertion pins the header guard that
/// makes this mirror true.
fn close_mode_of(lines: &[&str]) -> &'static str {
    if lines.len() < 2 {
        return "unknown";
    }
    if lines.len() == 2 {
        return "keep";
    }
    if lines[2].trim() == "close-after-run" {
        "close"
    } else {
        "unknown"
    }
}

#[test]
fn close_mode_parsing_is_tri_state_and_fail_closed() {
    assert_eq!(
        close_mode_of(&["r", "d"]),
        "keep",
        "a clean two-line file keeps open"
    );
    assert_eq!(close_mode_of(&["r", "d", "close-after-run"]), "close");
    assert_eq!(
        close_mode_of(&["close-after-run"]),
        "unknown",
        "a one-line file must never authorize a close"
    );
    assert_eq!(
        close_mode_of(&["r", "d", "keep-open"]),
        "unknown",
        "unsupported tokens are rejected"
    );
    assert_eq!(close_mode_of(&[]), "unknown");
    let case = Case::new(TRACER);
    case.build_to("modecheck.swb", false).unwrap();
    let generated = fs::read_to_string(case.dir.path().join("modecheck.swb")).unwrap();
    for (name, text) in [
        ("generated", generated.as_str()),
        (
            "checkpoint",
            include_str!("../corpus/compat-check/minimal.swb"),
        ),
    ] {
        assert!(
            text.contains("If Not ok Then Exit Function"),
            "{name}: a failed header read can still authorize a close"
        );
    }
}

// ---- Phase 1 batch 2: extrude/revolve direction truth table ----

use craft_bridge::replay::{ExtrudeKind, RevolveKind};
use craft_bridge::truth;

/// The builder region of an emitted macro procedure (up to its end).
fn builder_region<'a>(text: &'a str, fname: &str) -> &'a str {
    let start = text
        .find(&format!("Function {fname}("))
        .or_else(|| text.find(&format!("Sub {fname}(")))
        .unwrap_or_else(|| panic!("no {fname} builder"));
    let end = text[start..]
        .find("End Function")
        .or_else(|| text[start..].find("End Sub"))
        .expect("the builder ends")
        + start;
    &text[start..end]
}

#[test]
fn truth_table_decisions_refuse_unmeasured_cells() {
    // 0 is the UNMEASURED sentinel: no decision may be taken from it.
    for want in [-1, 1] {
        assert_eq!(truth::extrude_dir_flag(want, 0), None);
        assert_eq!(truth::extrude_two_sided_swap(want, 0), None);
        assert_eq!(truth::revolve_reverse_flag(want, 0), None);
        assert_eq!(truth::revolve_two_sided_swap(want, 0), None);
        assert_eq!(truth::revolve_in_evidence_scope(want, 1, 0), None);
    }
}

#[test]
fn primitive_orientation_cell_carries_the_measured_reading() {
    // probe-primorient box-dir-false/box-dir-true (2026-11-17) on the Top
    // frame: Dir=True put the blind 0.012 m box at z in [-0.012, 0] =
    // toward MINUS the normal; Dir=False at [0, 0.012]. The reading equals
    // the extrude family's cell on a second frame (the cross-check the
    // evidence scope required) and the emitter's composition resolves
    // through it: wantSide = +1 * gMapSide, dirFlag = (wantSide = TT).
    assert_eq!(
        truth::TT_PRIM_DIR_TRUE_SIDE,
        -1,
        "the primitive orientation cell must carry the probe-primorient reading"
    );
    assert_eq!(
        truth::TT_PRIM_DIR_TRUE_SIDE,
        truth::TT_EX_DIR_TRUE_SIDE,
        "the two byte-identical call shapes must not disagree across frames"
    );
    for (want, flag) in [(1, Some(false)), (-1, Some(true))] {
        assert_eq!(
            truth::extrude_dir_flag(want, truth::TT_PRIM_DIR_TRUE_SIDE),
            flag,
            "want {want}: the composed Dir flag is wrong"
        );
    }
}

#[test]
fn revolve_flags_agree_under_both_candidate_laws_in_the_evidence_scope() {
    // The probe records read two ways: the flag selects an ABSOLUTE side in
    // units of the sketch normal (TT = -1), or a right-hand SENSE about the
    // drawn axis direction (TT = +1). The two readings give identical flags
    // exactly where the configuration chirality (map_side * sigma) matches
    // the probe's - which is what makes the evidence scope's decisions
    // established rather than assumed. Outside it the readings diverge and
    // the gate refuses.
    const TT_ABSOLUTE: i32 = -1; // the same records, absolute-in-normal reading
    for w in [-1, 1] {
        for sigma in [-1, 1] {
            // Blind partial: want sense = sigma * direction sign.
            for d in [-1, 1] {
                let sense_law =
                    truth::revolve_reverse_flag(sigma * d, truth::TT_REV_SENSE).unwrap();
                let absolute_law = truth::revolve_reverse_flag(w * d, TT_ABSOLUTE).unwrap();
                let in_scope =
                    truth::revolve_in_evidence_scope(w, sigma, truth::TT_REV_PROBE_CHIRALITY)
                        .unwrap();
                if in_scope {
                    assert_eq!(
                        sense_law, absolute_law,
                        "w={w} sigma={sigma} d={d}: the laws disagree inside the evidence scope"
                    );
                }
            }
            // Two-sided: the swap decision reads the same way.
            let sense_law =
                truth::revolve_two_sided_swap(sigma, truth::TT_REV_TWO_FIRST_SIDE).unwrap();
            let absolute_law = truth::revolve_two_sided_swap(w, TT_ABSOLUTE).unwrap();
            let in_scope =
                truth::revolve_in_evidence_scope(w, sigma, truth::TT_REV_PROBE_CHIRALITY).unwrap();
            if in_scope {
                assert_eq!(
                    sense_law, absolute_law,
                    "w={w} sigma={sigma}: swap laws disagree"
                );
            } else {
                assert_ne!(
                    sense_law, absolute_law,
                    "w={w} sigma={sigma}: outside the scope the laws must diverge (or the gate is vacuous)"
                );
            }
        }
    }
}

#[test]
fn truth_table_decisions_map_sides_and_senses() {
    // Blind: the Dir flag is the equality of the wanted side and the
    // measured side - over both map-side flips and both directions.
    for (want, measured, flag) in [
        (1, 1, Some(true)),
        (1, -1, Some(false)),
        (-1, 1, Some(false)),
        (-1, -1, Some(true)),
    ] {
        assert_eq!(
            truth::extrude_dir_flag(want, measured),
            flag,
            "want {want} measured {measured}"
        );
    }
    // Two-sided: swap exactly when the wanted first side differs.
    for (want, measured, swap) in [
        (1, 1, Some(false)),
        (1, -1, Some(true)),
        (-1, 1, Some(true)),
        (-1, -1, Some(false)),
    ] {
        assert_eq!(truth::extrude_two_sided_swap(want, measured), swap);
    }
    // Revolve: the flag flips exactly when the wanted sense differs.
    for (want, measured, flag) in [
        (1, 1, Some(false)),
        (1, -1, Some(true)),
        (-1, 1, Some(true)),
        (-1, -1, Some(false)),
    ] {
        assert_eq!(truth::revolve_reverse_flag(want, measured), flag);
    }
    for (want, measured, swap) in [
        (1, 1, Some(false)),
        (1, -1, Some(true)),
        (-1, 1, Some(true)),
        (-1, -1, Some(false)),
    ] {
        assert_eq!(truth::revolve_two_sided_swap(want, measured), swap);
    }
}

#[test]
fn profile_side_classifies_real_geometry_only() {
    // Points 0 and 1 are UNUSED (no non-construction curve reaches them)
    // and 2/3 belong to the construction axis; none of them may decide the
    // profile's side. The circle's radius reaches beyond its centre.
    let points = [
        [0.0, 0.0],   // 0: unused
        [-1.0, 0.0],  // 1: unused, on the far side of the axis
        [0.0, -20.0], // 2/3: the construction axis
        [0.0, 20.0],
        [5.0, -10.0], // 4..7: the profile rectangle at u = 5..10
        [10.0, -10.0],
        [10.0, 10.0],
        [5.0, 10.0],
        [3.0, 0.0], // 8: circle centre u = 3, r = 1
    ];
    let lines = [
        (2, 3, true),
        (4, 5, false),
        (5, 6, false),
        (6, 7, false),
        (7, 4, false),
    ];
    let circles = [(8usize, 1.0f64, false)];
    // Axis along +v at u = 0 (normalized internally).
    let side = truth::profile_side(&points, &lines, &circles, [0.0, -20.0], [0.0, 40.0])
        .expect("the profile lies on one side");
    assert_eq!(side, -1, "the perpendicular distance is -u here");
    // A straddling profile refuses with its reason named.
    let mut straddle = points;
    straddle[4] = [-5.0, -10.0];
    let err = truth::profile_side(&straddle, &lines, &circles, [0.0, -20.0], [0.0, 1.0])
        .expect_err("a crossing profile must refuse");
    assert!(err.contains("crosses"), "unexpected reason: {err}");
    // The circle's RADIUS decides, not its centre: at u = 0.5 with r = 1 it
    // crosses the axis although its centre is off it.
    let circles_near = [(8usize, 1.0f64, false)];
    let mut near = points;
    near[8] = [0.5, 0.0];
    let err = truth::profile_side(&near, &lines, &circles_near, [0.0, -20.0], [0.0, 1.0])
        .expect_err("a circle reaching across the axis must refuse");
    assert!(err.contains("crosses"), "unexpected reason: {err}");
    // Everything on the axis: no side exists.
    let on_axis = [(2usize, 3usize, false), (2, 3, false), (3, 2, false)];
    let err = truth::profile_side(&points, &on_axis, &[], [0.0, 0.0], [0.0, 1.0])
        .expect_err("geometry on the axis has no side");
    assert!(
        err.contains("lies on the revolve axis"),
        "unexpected: {err}"
    );
    // No profile geometry at all is a named refusal, not a silent +1.
    let err = truth::profile_side(&points, &[(2, 3, true)], &[], [0.0, -20.0], [0.0, 1.0])
        .expect_err("construction-only geometry has no profile side");
    assert!(err.contains("no profile geometry"), "unexpected: {err}");
}

#[test]
fn mapped_axis_keeps_the_sweep_sense_for_any_valid_map() {
    // The drawn axis direction is the MAPPED source direction ("like
    // geometry"), so the right-hand sense sigma survives every valid map -
    // signed permutations and non-permutation isometries alike: the 2D cross
    // product scales by the map's determinant (its orientation, the runtime
    // gMapSide). This is the "any valid map" algebra behind the emitted
    // decision.
    let maps: [([f64; 4], &str); 7] = [
        ([1.0, 0.0, 0.0, 1.0], "identity"),
        ([0.0, 1.0, -1.0, 0.0], "quarter turn"),
        ([-1.0, 0.0, 0.0, 1.0], "u reflected"),
        ([1.0, 0.0, 0.0, -1.0], "v reflected"),
        ([0.0, -1.0, 1.0, 0.0], "opposite quarter turn"),
        ([-1.0, 0.0, 0.0, -1.0], "half turn"),
        (
            [
                std::f64::consts::FRAC_1_SQRT_2,
                std::f64::consts::FRAC_1_SQRT_2,
                -std::f64::consts::FRAC_1_SQRT_2,
                std::f64::consts::FRAC_1_SQRT_2,
            ],
            "45 degrees (non-permutation)",
        ),
    ];
    let cross = |a: (f64, f64), b: (f64, f64)| a.0 * b.1 - a.1 * b.0;
    for (d, rho) in [((0.0, 1.0), (5.0, -2.0)), ((1.0, 0.0), (-3.0, 7.0))] {
        let src = cross(d, rho);
        for (m, name) in maps {
            let det = m[0] * m[3] - m[2] * m[1];
            // Vectors map through the linear part of MapU/MapV (no offset).
            let map_vec = |v: (f64, f64)| (v.0 * m[0] + v.1 * m[2], v.0 * m[1] + v.1 * m[3]);
            let mapped = cross(map_vec(d), map_vec(rho));
            assert!(
                (mapped - det * src).abs() < 1e-12,
                "{name}: the mapped cross does not carry the orientation"
            );
            // The wanted sense computed in source space equals the one
            // computed in sketch space (through the mapped axis).
            let sigma_src = if src > 0.0 { 1 } else { -1 };
            let sigma_sketch = if mapped > 0.0 { 1 } else { -1 };
            let w = if det > 0.0 { 1 } else { -1 };
            assert_eq!(sigma_src, w * sigma_sketch, "{name}: senses disagree");
        }
    }
}

#[test]
fn extrude_builders_carry_the_direction_truth_table() {
    // The constants block is emitted with slot markers and probe names.
    let case = Case::new(TRACER);
    case.build_to("tt.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("tt.swb")).unwrap();
    for (name, value, probe) in [
        ("TT_EX_DIR_TRUE_SIDE", -1, "probe-extrudedir"),
        ("TT_EX_FIRST_SIDE", 1, "probe-twosided"),
        ("TT_EX_THROUGH_ALL_DIR", -1, "probe-throughall"),
        ("TT_REV_SENSE", 1, "probe-revolvedir"),
        ("TT_REV_TWO_FIRST_SIDE", 1, "probe-revolvedir"),
        ("TT_REV_PROBE_CHIRALITY", -1, "probe-revolvedir"),
    ] {
        assert!(
            text.contains(&format!("Const {name} As Long = {value}")),
            "{name}: the measured value is not emitted"
        );
        let at = text.find(&format!("Const {name}")).expect("the constant");
        let line = &text[at..text[at..].find('\n').unwrap() + at];
        assert!(
            line.contains(probe),
            "{name}: no evidence source named: {line}"
        );
    }
    // Blind: the flag decision runs through the map side and the measured
    // side, with the probe named in the refusal.
    let blind = builder_region(&text, "BuildExtrude3");
    assert!(
        blind.contains("probe-extrudedir has not established"),
        "the blind refusal does not name its probe"
    );
    assert!(
        blind.contains("wantSide = 1 * gMapSide(2)")
            && blind.contains("dirFlag = (wantSide = TT_EX_DIR_TRUE_SIDE)"),
        "the blind side decision is missing"
    );
}

#[test]
fn extrude_directions_fold_signs_and_refuse_unrepresentable_shapes() {
    // A negative distance turns its own side over (#2700).
    let recipe = TRACER.replace("\"distance\": \"12 mm\"", "\"distance\": \"-12 mm\"");
    let steps = plan_of(&recipe);
    match &steps[3].work {
        Work::Extrude(ex) => assert_eq!(
            ex.kind,
            ExtrudeKind::Blind {
                depth_m: 0.012,
                negative: true
            }
        ),
        other => panic!("not an extrude: {other:?}"),
    }
    let case = Case::new(&recipe);
    case.build_to("neg.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("neg.swb")).unwrap();
    let blind = builder_region(&text, "BuildExtrude3");
    assert!(
        blind.contains("wantSide = -1 * gMapSide(2)"),
        "the negative distance did not flip the wanted side"
    );
    // The sign contract on the depth equation: the dimension carries the
    // absolute value, signVal folds the sign into the stored equation text.
    let recipe = TRACER.replace("\"distance\": \"12 mm\"", "\"distance\": \"-width\"");
    let case = Case::new(&recipe);
    case.build_to("neg2.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("neg2.swb")).unwrap();
    let blind = builder_region(&text, "BuildExtrude3");
    assert!(
        blind.contains(
            "-1#, feat.Name, 1#) Then gMsg = \"the extrude depth equation did not evaluate\""
        ),
        "the negative source sign did not travel through signVal"
    );
    assert!(
        blind.contains(", 0.04, "),
        "the driven dimension does not carry the absolute value"
    );
    // Two-sided ends keep their sides; the arguments and the D1/D2
    // dimension indices follow the runtime swap.
    let recipe = TRACER.replace(
        "\"distance\": \"12 mm\"",
        "\"distance\": \"10 mm\", \"distance2\": \"4 mm\"",
    );
    let steps = plan_of(&recipe);
    match &steps[3].work {
        Work::Extrude(ex) => assert_eq!(
            ex.kind,
            ExtrudeKind::TwoSided {
                first_m: 0.01,
                second_m: 0.004,
                first_negative: false,
                second_negative: true,
            }
        ),
        other => panic!("not an extrude: {other:?}"),
    }
    let case = Case::new(&recipe);
    case.build_to("two.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("two.swb")).unwrap();
    let two = builder_region(&text, "BuildExtrude3");
    assert!(
        two.contains("probe-twosided has not established"),
        "the two-sided refusal does not name its probe"
    );
    assert!(
        two.contains("swap2 = (wantFirst <> TT_EX_FIRST_SIDE)")
            && two.contains("wFirst = 2")
            && two.contains("Trim(Str(wFirst))"),
        "the swap-aware dimension binding is missing"
    );
    // Ends on the same side of the sketch plane: refused before the call.
    let recipe = TRACER.replace(
        "\"distance\": \"12 mm\"",
        "\"distance\": \"-10 mm\", \"distance2\": \"4 mm\"",
    );
    let steps = plan_of(&recipe);
    match &steps[3].work {
        Work::Extrude(ex) => assert_eq!(
            ex.kind,
            ExtrudeKind::TwoSided {
                first_m: 0.01,
                second_m: 0.004,
                first_negative: true,
                second_negative: true,
            }
        ),
        other => panic!("not an extrude: {other:?}"),
    }
    let case = Case::new(&recipe);
    case.build_to("same.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("same.swb")).unwrap();
    assert!(
        text.contains("same side of the sketch plane is not representable"),
        "a same-side span is not refused"
    );
    // Through-all: the direction cell is the probe's; while unmeasured the
    // step refuses and names the probe and its inconclusive contract.
    let recipe = TRACER.replace(
        "\"distance\": \"12 mm\"",
        "\"distance\": \"12 mm\", \"through_all\": true",
    );
    let case = Case::new(&recipe);
    case.build_to("thru.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("thru.swb")).unwrap();
    let thru = builder_region(&text, "BuildExtrude3");
    assert!(
        thru.contains("probe-throughall") && thru.contains("TT_EX_THROUGH_ALL_DIR = 0"),
        "the through-all refusal does not name its probe"
    );
    // Symmetric is direction-free: no truth-table cell is consulted.
    let recipe = TRACER.replace(
        "\"direction\": \"positive\"",
        "\"direction\": \"symmetric\"",
    );
    let case = Case::new(&recipe);
    case.build_to("sym.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("sym.swb")).unwrap();
    let sym = builder_region(&text, "BuildExtrude3");
    assert!(
        sym.contains("swEndCondMidPlane"),
        "the symmetric extrude is not mid-plane"
    );
    assert!(
        !sym.contains("TT_EX") && !sym.contains("TT_REV"),
        "a direction-free form consulted the truth table"
    );
}

#[test]
fn through_all_never_allocates_depth_equations() {
    // The saved distance only names the side: a parametric one must not
    // create a depth equation or a live dimension slot (through-all has no
    // depth dimension to bind to).
    let recipe = TRACER.replace(
        "\"distance\": \"12 mm\"",
        "\"distance\": \"width\", \"through_all\": true",
    );
    let steps = plan_of(&recipe);
    match &steps[3].work {
        Work::Extrude(ex) => {
            assert_eq!(ex.kind, ExtrudeKind::ThroughAll { negative: false });
            assert!(ex.equations.is_empty(), "a phantom depth equation appeared");
        }
        other => panic!("not an extrude: {other:?}"),
    }
    let case = Case::new(&recipe);
    case.build_to("thru2.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("thru2.swb")).unwrap();
    let thru = builder_region(&text, "BuildExtrude3");
    assert!(
        !thru.contains("AddDimEquation"),
        "a through-all builder binds a depth equation"
    );
    // two-sided through-all used to drop the second distance silently: it is
    // a named stop now.
    let recipe = TRACER.replace(
        "\"distance\": \"12 mm\"",
        "\"distance\": \"12 mm\", \"distance2\": \"4 mm\", \"through_all\": true",
    );
    let steps = plan_of(&recipe);
    assert_eq!(steps[3].status, PlanStatus::FallbackPlanned);
    assert!(
        steps[3].detail.contains("two-sided through-all"),
        "unexpected detail: {}",
        steps[3].detail
    );
}

#[test]
fn legacy_symmetric_parametric_extrudes_stop_honestly() {
    // legacy_per_side doubles the thickness; a non-literal distance would
    // drive the doubled dimension from the unscaled expression. Literal
    // distances stay supported (folded at parse time).
    let recipe = TRACER.replace(
        "\"convention\": \"total_distance\", \"direction\": \"positive\", \"distance\": \"12 mm\"",
        "\"convention\": \"legacy_per_side\", \"direction\": \"symmetric\", \"distance\": \"12 mm\"",
    );
    let steps = plan_of(&recipe);
    match &steps[3].work {
        Work::Extrude(ex) => {
            assert_eq!(ex.kind, ExtrudeKind::Symmetric { total_m: 0.024 })
        }
        other => panic!("not an extrude: {other:?}"),
    }
    let recipe = TRACER.replace(
        "\"convention\": \"total_distance\", \"direction\": \"positive\", \"distance\": \"12 mm\"",
        "\"convention\": \"legacy_per_side\", \"direction\": \"symmetric\", \"distance\": \"width\"",
    );
    let steps = plan_of(&recipe);
    assert_eq!(steps[3].status, PlanStatus::FallbackPlanned);
    assert!(
        steps[3].detail.contains("legacy_per_side symmetric"),
        "unexpected detail: {}",
        steps[3].detail
    );
}

#[test]
fn revolve_builders_map_the_axis_and_gate_the_sweep_law() {
    let case = Case::new(TRACER_REVOLVE);
    case.build_to("ttrev.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("ttrev.swb")).unwrap();
    // Partial (BuildRevolve1, axis = its sketch line, sigma = -1): the law
    // gate precedes the measured sense, and both refusals name their probe.
    let partial = builder_region(&text, "BuildRevolve1");
    assert!(
        partial.contains("gMapSide(0) * -1 <> TT_REV_PROBE_CHIRALITY")
            && partial.contains("outside it and the candidate sweep laws disagree"),
        "the evidence-scope gate is missing"
    );
    assert!(
        partial.contains("TT_REV_SENSE = 0") && partial.contains("probe-revolvedir"),
        "the sense refusal does not name its probe"
    );
    assert!(
        partial.contains("revFlag = (TT_REV_SENSE <> -1)"),
        "the profile-side factor did not reach the flag: {}",
        partial
    );
    // Full turn (BuildRevolve2, a helper axis): direction-free - no law
    // gate, no sense cell - and the helper axis is drawn THROUGH THE MAP
    // like geometry, with its placement read back.
    let full = builder_region(&text, "BuildRevolve2");
    assert!(
        !full.contains("TT_REV_") && !full.contains("revFlag"),
        "a full turn consulted the direction truth table"
    );
    assert!(
        full.contains("CreateLine(MapU(0, -0.05, 0.0), MapV(0, -0.05, 0.0), 0#, MapU(0, 0.05, 0.0), MapV(0, 0.05, 0.0), 0#)"),
        "the helper axis is not drawn through the map like geometry"
    );
    assert!(
        full.contains("does not sit where the map put it"),
        "the helper axis placement is not read back"
    );
    assert!(
        full.contains("If Not ax.Select(False)"),
        "the helper axis selection is not checked"
    );
}

#[test]
fn revolve_angle_semantics_preserve_signs_and_swap_sides() {
    // A negative angle2 is an interval offset from the sketch plane
    // ([-angle2, +angle]): the signs survive planning and the builder
    // refuses before feature creation - never folded away.
    let recipe = TRACER_REVOLVE.replace(
        "\"angle\": \"270 deg\"",
        "\"angle\": \"270 deg\", \"angle2\": \"-30 deg\"",
    );
    let steps = plan_of(&recipe);
    match &steps[1].work {
        Work::Revolve(rv) => match &rv.kind {
            RevolveKind::TwoSided {
                first_rad,
                second_rad,
            } => {
                assert!(*first_rad > 0.0, "the first angle lost its sign");
                assert!(*second_rad < 0.0, "the second angle lost its sign");
            }
            other => panic!("not two-sided: {other:?}"),
        },
        other => panic!("not a revolve: {other:?}"),
    }
    let case = Case::new(&recipe);
    case.build_to("signedrev.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("signedrev.swb")).unwrap();
    assert!(
        text.contains("signed angles are refused, never folded"),
        "a signed two-sided interval is not refused"
    );
    // A representable two-sided revolve swaps the angle arguments and their
    // dimension indices per the measured first-side cell and the profile
    // side (sigma = -1 for this fixture).
    let recipe = TRACER_REVOLVE.replace(
        "\"angle\": \"270 deg\"",
        "\"angle\": \"100 deg\", \"angle2\": \"50 deg\"",
    );
    let case = Case::new(&recipe);
    case.build_to("tworev.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("tworev.swb")).unwrap();
    let two = builder_region(&text, "BuildRevolve1");
    assert!(
        two.contains("swapR = (TT_REV_TWO_FIRST_SIDE <> -1)")
            && two.contains("wFirst = 2")
            && two.contains("probe-revolvedir"),
        "the two-sided revolve swap is missing: {}",
        two
    );
    assert!(
        two.contains("Trim(Str(wFirst))"),
        "the swap-aware angle binding is missing"
    );
}

#[test]
#[ignore = "regenerates the pinned goldens after intentional emitter changes"]
fn regenerate_goldens() {
    let case = Case::new(TRACER);
    let out = case.dir.path().join("tracer.swb");
    let plan_path = case.dir.path().join("tracer-plan.json");
    let text_path = case.dir.path().join("tracer-plan.txt");
    build::build(
        &BuildOptions {
            artifact: &case.artifact,
            out: &out,
            plan: Some(&plan_path),
            plan_text: Some(&text_path),
            overwrite: true,
        },
        &Limits::default(),
    )
    .unwrap();
    let gold = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    fs::copy(&out, gold.join("tracer.swb")).unwrap();
    fs::copy(&plan_path, gold.join("tracer-plan.json")).unwrap();
    fs::copy(&text_path, gold.join("tracer-plan.txt")).unwrap();
}

/// The `Option Explicit` compile contract, scanned offline: every identifier
/// a procedure uses must be declared there, at module level, or be a known
/// VBA word or procedure name. Same scanner as tests/frame_corpus.rs (the
/// test files carry their own copies of the scans). Regression for the
/// Dims-template defect the 2026-11-16 byte reviews found: the template
/// declared `v2` but the length behaviour assigns `v` - a whole-module
/// compile error the seat reports only as a bare invocation failure with no
/// report (docs/seat-acceptance.md "Runner quirks").
fn assert_locals_declared(text: &str, what: &str) {
    // VBA keywords, intrinsic functions and intrinsic constants: never a
    // user identifier in these macros.
    const VBA_WORDS: &[&str] = &[
        "dim",
        "const",
        "as",
        "set",
        "let",
        "get",
        "call",
        "new",
        "nothing",
        "true",
        "false",
        "empty",
        "null",
        "if",
        "then",
        "else",
        "elseif",
        "end",
        "select",
        "case",
        "for",
        "next",
        "to",
        "step",
        "each",
        "in",
        "do",
        "while",
        "loop",
        "until",
        "wend",
        "with",
        "exit",
        "goto",
        "on",
        "error",
        "resume",
        "function",
        "sub",
        "property",
        "private",
        "public",
        "static",
        "optional",
        "byref",
        "byval",
        "paramarray",
        "not",
        "and",
        "or",
        "is",
        "mod",
        "xor",
        "eqv",
        "imp",
        "boolean",
        "byte",
        "integer",
        "long",
        "single",
        "double",
        "string",
        "variant",
        "object",
        "date",
        "any",
        "sgn",
        "abs",
        "val",
        "mid",
        "left",
        "right",
        "len",
        "asc",
        "chr",
        "instr",
        "time",
        "now",
        "timer",
        "format",
        "array",
        "filter",
        "split",
        "join",
        "lcase",
        "ucase",
        "trim",
        "str",
        "fix",
        "int",
        "round",
        "sin",
        "cos",
        "tan",
        "log",
        "exp",
        "hex",
        "oct",
        "rgb",
        "environ",
        "shell",
        "name",
        "kill",
        "width",
        "spc",
        "tab",
        "stop",
        "line",
        "open",
        "close",
        "input",
        "print",
        "err",
        "ubound",
        "lbound",
        "isarray",
        "isnumeric",
        "isempty",
        "isnull",
        "isobject",
        "cint",
        "clng",
        "cdbl",
        "csng",
        "cstr",
        "cbool",
        "cvar",
        "cdate",
        "vbcrlf",
        "vbcr",
        "vblf",
        "vbtab",
        "vbobjecterror",
        "vbtextcompare",
        "msgbox",
        "inputbox",
        "typename",
        "createobject",
        "getobject",
        "chrw",
        "ascw",
        "sqr",
        "replace",
        "freefile",
        "output",
        "append",
    ];
    let is_word = |t: &str| VBA_WORDS.contains(&t.to_lowercase().as_str());
    let decl_names = |line: &str| -> Vec<String> {
        let t = line.trim();
        // Strip the Dim/Const keyword; a fixed offset is wrong for Const.
        let rest = match t.find(' ') {
            Some(sp) => &t[sp..],
            None => return Vec::new(),
        };
        rest.split(',')
            .filter_map(|part| {
                let name = part
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .split('(')
                    .next()
                    .unwrap_or("");
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_lowercase())
                }
            })
            .collect()
    };

    // Pass 1: procedure spans, names, parameters, labels and declarations.
    let mut proc_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut module_decls: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut owners: Vec<Option<usize>> = Vec::new();
    let mut procs: Vec<(String, std::collections::HashSet<String>)> = Vec::new();
    let mut current: Option<usize> = None;
    for line in text.lines() {
        let t = line.trim();
        let up = t.to_uppercase();
        let code = code_only(line);
        let cu = code.trim().to_uppercase();
        if cu.starts_with("FUNCTION ") || cu.starts_with("SUB ") {
            let name = t
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .split('(')
                .next()
                .unwrap_or("")
                .to_lowercase();
            proc_names.insert(name.clone());
            let mut decls: std::collections::HashSet<String> = std::collections::HashSet::new();
            if let Some(plist) = t.split('(').nth(1) {
                for p in plist.split(',') {
                    for tok in p.split_whitespace() {
                        let low = tok.to_lowercase();
                        if low == "byval"
                            || low == "byref"
                            || low == "optional"
                            || low == "paramarray"
                        {
                            continue;
                        }
                        decls.insert(tok.trim_end_matches(')').to_lowercase());
                        break;
                    }
                }
            }
            procs.push((name, decls));
            current = Some(procs.len() - 1);
            owners.push(current);
            continue;
        }
        if cu.starts_with("END FUNCTION") || cu.starts_with("END SUB") {
            current = None;
            owners.push(None);
            continue;
        }
        owners.push(current);
        if up.starts_with("DIM ") || up.starts_with("CONST ") {
            let names = decl_names(t);
            match current {
                Some(pi) => procs[pi].1.extend(names),
                None => module_decls.extend(names),
            }
            continue;
        }
        if let Some(pi) = current {
            let c = code.trim();
            if let Some(label) = c.split(':').next() {
                if c.contains(':')
                    && label
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    && !label.is_empty()
                {
                    procs[pi].1.insert(label.to_lowercase());
                }
            }
        }
    }

    // Pass 2: every used identifier is declared somewhere legitimate.
    for (i, line) in text.lines().enumerate() {
        let Some(pi) = owners.get(i).copied().flatten() else {
            continue;
        };
        let code = code_only(line);
        let cu = code.trim().to_uppercase();
        if cu.starts_with("FUNCTION ")
            || cu.starts_with("SUB ")
            || cu.starts_with("DIM ")
            || cu.starts_with("CONST ")
            || cu.starts_with("END FUNCTION")
            || cu.starts_with("END SUB")
        {
            continue;
        }
        let chars: Vec<char> = code.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            if chars[j].is_ascii_alphabetic() || chars[j] == '_' {
                let start = j;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                // Word boundaries: "1E+100" must not yield "e".
                if start > 0
                    && (chars[start - 1].is_ascii_alphanumeric() || chars[start - 1] == '_')
                {
                    continue;
                }
                let after_dot = start > 0 && chars[start - 1] == '.';
                let before_dot = j < chars.len() && chars[j] == '.';
                if after_dot || before_dot {
                    continue;
                }
                let token: String = chars[start..j].iter().collect();
                let low = token.to_lowercase();
                if is_word(&low) {
                    continue;
                }
                if procs[pi].1.contains(&low) || module_decls.contains(&low) {
                    continue;
                }
                if proc_names.contains(&low) {
                    continue;
                }
                panic!(
                    "{what}: {token} is used in {} at line {} but declared nowhere",
                    procs[pi].0,
                    i + 1
                );
            } else {
                j += 1;
            }
        }
    }
}

#[test]
fn sketch_selection_uses_the_stored_feature_object() {
    // Seat evidence (2026-11-17): SelectByID2 by SKETCH name returns False
    // once a probe sketch has been created and deleted before the target
    // sketch (the tracer flow) - the name is right and the selection still
    // fails. A LIVE FEATURE OBJECT's Select is the pinned-reliable path
    // (probe-endpoints, DiscardProbeSketch), with the named selection kept
    // as fallback and TreeSummary() in the refusal for the next evidence
    // round.
    let case = Case::new(TRACER);
    case.build_to("sell.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("sell.swb")).unwrap();
    // Capture: the proven tree walk stores the object per sketch step.
    let sketch = builder_region(&text, "BuildSketch2");
    assert!(
        sketch.contains("Set skFeat2 = FindFeatureByName(skName2, uncertain)")
            && sketch.contains("the sketch feature object could not be captured"),
        "the sketch feature object is not captured through the proven walk"
    );
    // Selection: object first, named fallback, TreeSummary on refusal.
    let extrude = builder_region(&text, "BuildExtrude3");
    assert!(
        extrude.contains("ok = skFeat2.Select(False)") && extrude.contains("select-result="),
        "the extrude does not select through the stored feature object"
    );
    assert!(
        extrude.contains("If ok = False Then ok = swDoc.Extension.SelectByID2(skName2,")
            && extrude.contains("could not select the sketch for the extrude: ")
            && extrude.contains("TreeSummary()"),
        "the extrude lost its named fallback or the TreeSummary refusal"
    );
    // SolveSketch re-enters through the same object, keeping the hardened
    // final-state gate and the raise-after-On-Error-GoTo-0 form.
    let solve = builder_region(&text, "SolveSketch");
    assert!(
        solve.contains("Sub SolveSketch(ByVal nm As String, ByVal skObj As SldWorks.Feature)")
            && solve.contains("If Not skObj Is Nothing Then okSel = skObj.Select(False)"),
        "SolveSketch does not re-enter through the stored feature object"
    );
    assert!(
        solve.contains("On Error GoTo 0") && solve.contains("If sk Is Nothing Then Err.Raise"),
        "SolveSketch lost its verified-final-state gate or raise form"
    );
    assert!(
        text.contains("SolveSketch skName2, skFeat2"),
        "a SolveSketch call site does not pass the stored object"
    );
    // The delete path selects through the object and never blind-deletes.
    // DeleteCreated is emitted only for snapshot-carrying artifacts.
    let dir = tempfile::tempdir().unwrap();
    let recipe_path = dir.path().join("script.json");
    let step_path = dir.path().join("part.step");
    let artifact = dir.path().join("stop.craftx");
    fs::write(&recipe_path, TRACER_STOP).unwrap();
    fs::write(&step_path, STEP_FIXTURE).unwrap();
    pack::pack(
        &PackOptions {
            recipe: &recipe_path,
            step: &step_path,
            out: &artifact,
            warnings: None,
            name: None,
            overwrite: false,
            snapshots: vec![(1, step_path.as_path())],
            final_snapshot: None,
        },
        &Limits::default(),
    )
    .unwrap();
    let out = dir.path().join("stop.swb");
    build::build(
        &BuildOptions {
            artifact: &artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: false,
        },
        &Limits::default(),
    )
    .unwrap();
    let stop_text = fs::read_to_string(&out).unwrap();
    let deleted = builder_region(&stop_text, "DeleteCreated");
    assert!(
        deleted.contains("If Not skFeat0 Is Nothing Then selOk = skFeat0.Select(False)")
            && deleted.contains("If selOk Then swDoc.EditDelete")
            && deleted.contains("Set skFeat0 = Nothing"),
        "DeleteCreated still selects the sketch by name alone"
    );
    // Recreate re-captures into the same slot.
    let clean = builder_region(&text, "CleanSketch2");
    assert!(
        clean.contains("Set skFeat2 = FindFeatureByName(skName2, uncertain)")
            && clean.contains("SolveSketch skName2, skFeat2"),
        "CleanSketch does not re-capture the recreated sketch object"
    );
    // The primitive builder follows the same path with its local object.
    let case = Case::new(TRACER_PRIM);
    case.build_to("sell2.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("sell2.swb")).unwrap();
    let prim = builder_region(&text, "BuildPrim2");
    assert!(
        prim.contains("Set skFeat = FindFeatureByName(primName, uncertain)")
            && prim.contains("If Not skFeat Is Nothing Then ok = skFeat.Select(False)")
            && prim.contains("object and name selection both failed")
            && prim.contains("SolveSketch primName, skFeat"),
        "the primitive builder does not select through its stored feature object"
    );
}

#[test]
fn revolve_refusals_carry_the_call_and_refuse_crossing_profiles() {
    // Seat evidence (b10 acceptance): the second revolve of revolve-axes
    // returned Nothing ("the revolve was not created (kind=single-dir)")
    // with no way to see the call. The refusal now carries the FULL CALL
    // TEXT (the probes' "entering:" style) so the next evidence round has
    // the arguments in hand.
    let case = Case::new(TRACER_REVOLVE);
    case.build_to("calltext.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("calltext.swb")).unwrap();
    let partial = builder_region(&text, "BuildRevolve1");
    assert!(
        partial
            .contains("callText = \"FeatureRevolve2(True, True, False, False, \" & CStr(revFlag)")
            && partial.contains("(kind=single-dir): \" & callText"),
        "the single-dir refusal does not carry the full call"
    );
    // The mid-plane and two-sided refusals carry theirs too.
    let recipe = TRACER_REVOLVE.replace(
        "\"angle\": \"270 deg\"",
        "\"angle\": \"270 deg\", \"symmetric\": true",
    );
    let case = Case::new(&recipe);
    case.build_to("calltext2.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("calltext2.swb")).unwrap();
    let mid = builder_region(&text, "BuildRevolve1");
    assert!(
        mid.contains("callText = \"FeatureRevolve2(True, True, False, False, False, True, swEndConditions_e.swEndCondMidPlane")
            && mid.contains("(kind=midplane): \" & callText"),
        "the midplane refusal does not carry the full call"
    );
    let recipe = TRACER_REVOLVE.replace(
        "\"angle\": \"270 deg\"",
        "\"angle\": \"100 deg\", \"angle2\": \"50 deg\"",
    );
    let case = Case::new(&recipe);
    case.build_to("calltext3.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("calltext3.swb")).unwrap();
    let two = builder_region(&text, "BuildRevolve1");
    assert!(
        two.contains("callText = \"FeatureRevolve2(False, True, False, False, False, True, swEndConditions_e.swEndCondBlind")
            && two.contains("Trim(Str(aA))")
            && two.contains("(kind=two-sided): \" & callText"),
        "the two-sided refusal does not carry the full call"
    );
    // A profile that CROSSES its revolve axis is refused named before the
    // call on every form - the source kernel's own unconditional rule
    // ("the profile crosses the revolve axis", any angle) - and the
    // feature call is never emitted. This is what the b10 fixture did:
    // its full turn's profile straddled the axis (v -10..10 about v=0)
    // and FeatureRevolve2 returned Nothing for it.
    let recipe = TRACER_REVOLVE.replace("25.0", "-5.0");
    let case = Case::new(&recipe);
    case.build_to("cross.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("cross.swb")).unwrap();
    let full = builder_region(&text, "BuildRevolve2");
    assert!(
        full.contains("the profile crosses the revolve axis")
            && full.contains("not a valid revolve profile"),
        "a crossing profile is not refused named: {}",
        full
    );
    assert!(
        !full.contains("FeatureRevolve2"),
        "a crossing profile still emits the feature call"
    );
}

#[test]
fn every_emitted_local_is_declared() {
    // Option Explicit turns an undeclared local into a whole-module compile
    // failure that the seat reports only as a bare invocation failure with
    // no report - the offline scan is the only early warning.
    for (name, recipe) in [
        ("tracer", TRACER),
        ("stop-rule", TRACER_STOP),
        ("revolve", TRACER_REVOLVE),
        ("primitives", TRACER_PRIM),
        ("modifications", TRACER_MOD),
        ("patterns", TRACER_PAT),
    ] {
        let case = Case::new(recipe);
        case.build_to("locals.swb", false).unwrap();
        let text = fs::read_to_string(case.dir.path().join("locals.swb")).unwrap();
        assert_locals_declared(&text, name);
    }
    // Negative control: the scan must catch the exact defect class it
    // exists for (the Dims-template `v`).
    let broken = "Sub Build1()\n    Dim ok As Boolean\n    v = 1#\nEnd Sub\n";
    assert!(
        std::panic::catch_unwind(|| assert_locals_declared(broken, "negative-control")).is_err(),
        "the locals scan misses undeclared locals"
    );
}

// ---- Phase 1 batch 3: primitive adaptation (offline synthetic coverage) ----

/// Mirrors the emitted MapIsPermutation VBA exactly: each map entry is 0 or
/// +-1 within tolerance, with exactly one nonzero per row and per column.
fn map_is_permutation(m: [f64; 4]) -> bool {
    let a = [m[0].abs(), m[1].abs(), m[2].abs(), m[3].abs()];
    for e in a {
        if e > 1e-6 && (e - 1.0).abs() > 1e-6 {
            return false;
        }
    }
    (a[0] + a[2] - 1.0).abs() <= 1e-6
        && (a[1] + a[3] - 1.0).abs() <= 1e-6
        && (a[0] + a[1] - 1.0).abs() <= 1e-6
        && (a[2] + a[3] - 1.0).abs() <= 1e-6
}

#[test]
fn primitive_profiles_map_on_all_signed_permutations() {
    // All eight signed permutations of the sketch map, evaluated over the
    // emitted source corners (the fixture box: length 0.08 m, width 0.02 m
    // at the SC origin): the mapped corners form the mapped footprint, the
    // u edge keeps the length and its length-dimension binding, the v edge
    // the width, the circle maps with its radius unchanged (isometry), the
    // classified shape relations land on the right axes, and the extrude
    // side composes wantSide = +1 * gMapSide through the measured cell.
    // A rotation map classifies as a refusal (bounded support).
    let maps: [[f64; 4]; 8] = [
        [1.0, 0.0, 0.0, 1.0],
        [-1.0, 0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0, -1.0],
        [-1.0, 0.0, 0.0, -1.0],
        [0.0, 1.0, -1.0, 0.0],
        [0.0, -1.0, 1.0, 0.0],
        [0.0, 1.0, 1.0, 0.0],
        [0.0, -1.0, -1.0, 0.0],
    ];
    let corners = [[0.0, 0.0], [0.08, 0.0], [0.08, 0.02], [0.0, 0.02]];
    for m in maps {
        assert!(
            map_is_permutation(m),
            "{m:?} must classify as a permutation"
        );
        let image: Vec<(f64, f64)> = corners.iter().map(|c| map_uv(m, c[0], c[1])).collect();
        let dist =
            |a: (f64, f64), b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
        // ln0 carries the length dimension, ln1 the width; the opposite
        // edges follow structurally through the closed contour.
        assert!(
            (dist(image[0], image[1]) - 0.08).abs() < 1e-12,
            "{m:?}: the u edge changed length"
        );
        assert!(
            (dist(image[1], image[2]) - 0.02).abs() < 1e-12,
            "{m:?}: the v edge changed length"
        );
        assert!(
            (dist(image[2], image[3]) - 0.08).abs() < 1e-12,
            "{m:?}: the far u edge changed length"
        );
        assert!(
            (dist(image[3], image[0]) - 0.02).abs() < 1e-12,
            "{m:?}: the far v edge changed length"
        );
        // Each edge is axis-aligned on the axis its classification picks
        // (the u edges on AxisClass(u), the v edges on AxisClass(v)).
        let cu = axis_class(m, true);
        let cv = axis_class(m, false);
        assert!(
            cu >= 0 && cv >= 0 && cu != cv,
            "{m:?}: the axes do not classify apart"
        );
        let aligned = |a: (f64, f64), b: (f64, f64), ax: i32| {
            if ax == 0 {
                (a.1 - b.1).abs() < 1e-12
            } else {
                (a.0 - b.0).abs() < 1e-12
            }
        };
        assert!(
            aligned(image[0], image[1], cu),
            "{m:?}: the u edge is off its classified axis"
        );
        assert!(
            aligned(image[1], image[2], cv),
            "{m:?}: the v edge is off its classified axis"
        );
        // The circle: the centre maps like geometry; the radius is an
        // isometry invariant (both map columns are unit vectors).
        let (cs, ct) = map_uv(m, 0.0, 0.0);
        assert!(
            cs.abs() < 1e-12 && ct.abs() < 1e-12,
            "{m:?}: the base centre moved"
        );
        let col_u = (m[0].powi(2) + m[1].powi(2)).sqrt();
        let col_v = (m[2].powi(2) + m[3].powi(2)).sqrt();
        assert!(
            (col_u - 1.0).abs() < 1e-12 && (col_v - 1.0).abs() < 1e-12,
            "{m:?}: the map is not an isometry"
        );
        // The side composition for both map orientations through the
        // measured cell (probe-primorient, 2026-11-17): wantSide = +1 *
        // gMapSide and dirFlag = (wantSide = TT_PRIM_DIR_TRUE_SIDE). With
        // the seat's reading (-1): side +1 extrudes with Dir=False
        // (material toward +n_sk = the +Z SC height on the measured
        // configuration), side -1 with Dir=True. The gate text for a
        // zeroed cell remains in the macro and fires only if the cell is
        // ever unmeasured again.
        for (side, flag) in [(1, Some(false)), (-1, Some(true))] {
            assert_eq!(
                truth::extrude_dir_flag(side, truth::TT_PRIM_DIR_TRUE_SIDE),
                flag,
                "{m:?} side {side}: the composed Dir flag is wrong"
            );
        }
    }
    let q = std::f64::consts::FRAC_1_SQRT_2;
    assert!(
        !map_is_permutation([q, q, -q, q]),
        "the 45-degree map must classify as a refusal"
    );
}

#[test]
fn primitive_refusals_are_named() {
    let case = Case::new(TRACER_PRIM);
    case.build_to("refuse.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("refuse.swb")).unwrap();
    let box_region = builder_region(&text, "BuildPrim2");
    let cyl_region = builder_region(&text, "BuildPrim3");
    for (region, name) in [(box_region, "box"), (cyl_region, "cylinder")] {
        assert!(
            region.contains("no valid adaptation map"),
            "{name}: the map-validity refusal is unnamed"
        );
        assert!(
            region.contains("signed-permutation sketch map (bounded support"),
            "{name}: the map-class refusal is unnamed"
        );
        assert!(
            region.contains("TT_PRIM_DIR_TRUE_SIDE = 0"),
            "{name}: the orientation gate is missing"
        );
        assert!(
            region.contains("probe-primorient"),
            "{name}: the orientation refusal does not name its probe"
        );
    }
    // The API circle dimension is diameter-valued (seat-established:
    // probe-circlerad and frame-sign-chain): the 2r reading is EXPECTED and
    // any other reading is refused by name BEFORE the value is written and
    // the dimension registered.
    assert!(
        cyl_region.contains("twice the source radius is"),
        "the diameter-valued reading contract is missing"
    );
    let kindcheck = cyl_region
        .find("twice the source radius")
        .expect("no kind check");
    let write = cyl_region
        .find("dm.SystemValue = signVal")
        .expect("no value write");
    let register = cyl_region.find("AddDimEquation").expect("no registration");
    assert!(
        kindcheck < write && write < register,
        "the kind check runs after the write or the registration"
    );
    // Mapped-placement readback refusals name what failed.
    assert!(
        box_region.contains("does not sit where the map put it"),
        "the box placement readback is unnamed"
    );
    assert!(
        cyl_region.contains("does not sit where the map put it"),
        "the circle placement readback is unnamed"
    );
    // The sign resolution refuses a measurement of neither sign by name.
    assert!(
        box_region.contains("but the source value is 0.08"),
        "the sign mismatch refusal is unnamed"
    );
}

#[test]
fn primitive_equations_reenter_before_the_next_dimension() {
    // AddDimEquation's EditRebuild3 EXITS sketch edit (seat-measured): the
    // first box equation must be followed by a SolveSketch re-entry before
    // the width dimension is created, or the later AddDimension2 silently
    // no-ops in a closed sketch. Ordering regression for the sequencing
    // defect the batch-3 analysis found in the old builder (the fixture's
    // first box size carries the equation).
    let case = Case::new(TRACER_PRIM);
    case.build_to("order.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("order.swb")).unwrap();
    let region = builder_region(&text, "BuildPrim2");
    let eq = region
        .find("AddDimEquation")
        .expect("no dimension equation");
    let reenter = region[eq..]
        .find("SolveSketch primName")
        .expect("no sketch re-entry after the equation")
        + eq;
    let second_dim = region[reenter..]
        .find("AddDimension2")
        .expect("no width dimension after the re-entry")
        + reenter;
    assert!(
        eq < reenter && reenter < second_dim,
        "the equation is not followed by a sketch re-entry before the next dimension"
    );
}

#[test]
fn primitive_box_carries_its_shape_contract() {
    // Parameter-edit safety (plan review): the generated rectangle never
    // relies on automatic relations - coincident corners close the contour,
    // classified horizontal/vertical relations pin rectangularity and edge
    // directions, and a fix on the corner anchors the literal placement.
    let case = Case::new(TRACER_PRIM);
    case.build_to("shape.swb", false).unwrap();
    let text = fs::read_to_string(case.dir.path().join("shape.swb")).unwrap();
    let region = builder_region(&text, "BuildPrim2");
    assert_eq!(
        region.matches("\"sgCOINCIDENT\"").count(),
        4,
        "the box does not close its four corners"
    );
    assert!(
        region.contains("axClassU = AxisClass(2, True)")
            && region.contains("axClassV = AxisClass(2, False)"),
        "the shape relations do not classify at runtime"
    );
    assert!(
        region.contains("nameU = \"sgHORIZONTAL2D\" Else nameU = \"sgVERTICAL2D\""),
        "the u edges carry no classified relation kind"
    );
    assert!(
        region.contains("nameV = \"sgHORIZONTAL2D\" Else nameV = \"sgVERTICAL2D\""),
        "the v edges carry no classified relation kind"
    );
    assert_eq!(
        region.matches("swDoc.SketchAddConstraints nameU").count(),
        2,
        "the two u edges do not get the u relation"
    );
    assert_eq!(
        region.matches("swDoc.SketchAddConstraints nameV").count(),
        2,
        "the two v edges do not get the v relation"
    );
    assert!(
        region.contains("\"sgFix\"") && region.contains("ln0.GetStartPoint2"),
        "the literal corner is not anchored"
    );
    let cyl = builder_region(&text, "BuildPrim3");
    assert!(
        cyl.contains("\"sgFix\"") && cyl.contains("ci.GetCenterPoint2"),
        "the cylinder centre is not anchored"
    );
}

#[test]
fn primitive_cylinder_tiny_radius_is_refused() {
    // Sub-micrometre negative control (plan review): at or below 2e-6 m the
    // radius- and diameter-kind circle-dimension readings coincide within
    // the 1e-6 m verification tolerance (and a halved driven radius passes
    // the geometry check), so the kind can never be established and the
    // planner refuses the size fail-closed.
    let tiny = TRACER_PRIM.replace("\"radius\": \"width\"", "\"radius\": \"0.0005 mm\"");
    let steps = plan_of(&tiny);
    assert_eq!(steps[3].status, PlanStatus::FallbackPlanned);
    assert!(
        steps[3].detail.contains("cannot be distinguished"),
        "the tiny-radius refusal does not name its reason: {}",
        steps[3].detail
    );
    // Just above the floor the same size is accepted again.
    let ok = TRACER_PRIM.replace("\"radius\": \"width\"", "\"radius\": \"0.003 mm\"");
    let steps = plan_of(&ok);
    assert_eq!(steps[3].status, PlanStatus::Mapped, "{}", steps[3].detail);
}
