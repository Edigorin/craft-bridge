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
    assert!(
        macro_text.contains("CreateCornerRectangle"),
        "no box sketch"
    );
    assert!(
        macro_text.contains("CreateCircleByRadius"),
        "no cylinder sketch"
    );
    assert!(
        macro_text.contains("regName(0)"),
        "the cross-reference slot is missing"
    );
    assert!(
        macro_text.contains("If regName(0) = \"\" Then GoTo ExFail"),
        "the dead-slot guard is missing"
    );
    assert!(
        macro_text.contains("* 2"),
        "the segmented right-hand side is missing"
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
    assert!(macro_text.contains("FeatureMirror"), "no mirror");
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

    // A uniform scale in element 12 scales the extracted axes, which the
    // probe tolerates by comparing directions after normalization.
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
