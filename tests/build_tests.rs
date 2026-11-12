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
    replay::plan(&value, &Limits::default()).unwrap()
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
    assert!(macro_text.contains("no translation yet"));
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
    let err = replay::plan(&value, &Limits::default()).unwrap_err();
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
        macro_text.contains("Front Plane"),
        "the origin mirror plane is not selected"
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
