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

// ---- literal expressions ----

#[test]
fn literals_parse_to_si_and_expressions_do_not() {
    let cases = [
        ("12 mm", 0.012, false),
        ("12", 0.012, false),
        ("-0.5 in", -0.0127, false),
        ("3 deg", 3.0 * std::f64::consts::PI / 180.0, true),
        ("0.25 rad", 0.25, true),
        ("2 cm", 0.02, false),
        ("1 ft", 0.3048, false),
    ];
    for (text, si, angle) in cases {
        let (v, a) = replay::parse_literal(text).expect(text);
        assert!((v - si).abs() < 1e-12, "{text}: {v} != {si}");
        assert_eq!(a, angle, "{text}");
    }
    for text in ["width", "2 * width", "12 mm + 1", "mm", "", "1..2", "1e3"] {
        assert!(
            replay::parse_literal(text).is_none(),
            "{text} must not parse"
        );
    }
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
