//! solidcraft/57 loader tests: format-aware validation, source identity vs
//! dense runtime indices, native coordinate drivers, suppression, the named
//! stop policy and prefix semantics. Fixtures are independently authored
//! synthetic designs - never client geometry.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use craft_bridge::build::{self, BuildOptions};
use craft_bridge::limits::Limits;
use craft_bridge::pack::{self, PackOptions};
use craft_bridge::recipe;
use craft_bridge::replay::{self, DimKind, PlanStatus, Work};

const DESIGN: &str = include_str!("fixtures/design57.json");
const STEP_FIXTURE: &str = include_str!("fixtures/part.step");

fn plan_of(recipe_json: &str) -> Vec<replay::PlannedStep> {
    let limits = Limits::default();
    let value = recipe::validate(recipe_json.as_bytes(), &limits).unwrap();
    replay::plan(&value, &limits, &BTreeMap::new()).unwrap()
}

fn plan_err(recipe_json: &str) -> String {
    let limits = Limits::default();
    match recipe::validate(recipe_json.as_bytes(), &limits) {
        Err(e) => e.to_string(),
        Ok(value) => match replay::plan(&value, &limits, &BTreeMap::new()) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected a refusal"),
        },
    }
}

/// A minimal one-sketch solidcraft/57 design around `points`.
fn sketch_design(id: u64, plane: &str, points: &str) -> String {
    format!(
        r#"{{
  "format": "solidcraft/57",
  "units": "mm",
  "params": [{{ "name": "W", "expr": "50 mm", "unit": "mm" }}],
  "features": [
    {{
      "id": {id},
      "name": "s",
      "suppressed": false,
      "type": "sketch",
      "plane": {plane},
      "sketch": {{
        "points": [{points}],
        "curves": [
          {{ "id": "l1", "type": "line", "a": 0, "b": 1 }},
          {{ "id": "l2", "type": "line", "a": 1, "b": 2 }},
          {{ "id": "l3", "type": "line", "a": 2, "b": 0 }}
        ],
        "constraints": [],
        "counters": {{ "l": 3, "p": 3 }}
      }}
    }}
  ]
}}"#
    )
}

#[test]
fn design57_plans_dense_indices_and_source_ids() {
    let steps = plan_of(DESIGN);
    // 3 parameter equations first, then the 5 features: dense 0..7.
    assert_eq!(steps.len(), 8);
    assert_eq!(steps[0].source_id, "parameter VaneSpan");
    assert_eq!(steps[0].command, "parameter");
    assert!(matches!(steps[0].work, Work::UserParam { .. }));
    assert_eq!(steps[2].source_id, "parameter VaneThickness");
    // Source feature ids travel in source_id; the runtime index is dense.
    assert_eq!(
        (steps[3].index, steps[3].source_id.as_str()),
        (3, "feature 5")
    );
    assert_eq!(
        (steps[4].index, steps[4].source_id.as_str()),
        (4, "feature 9000")
    );
    assert_eq!(
        (steps[7].index, steps[7].source_id.as_str()),
        (7, "feature 100000")
    );
    assert!(matches!(steps[3].work, Work::Sketch(_)));
    assert!(matches!(steps[4].work, Work::Extrude(_)));
    assert_eq!(steps[3].status, PlanStatus::Mapped);
    assert_eq!(steps[4].status, PlanStatus::Mapped);
    assert_eq!(steps[6].status, PlanStatus::Mapped);
    assert_eq!(steps[7].status, PlanStatus::Mapped);
}

#[test]
fn design57_coordinate_drivers_become_native_dimensions() {
    let steps = plan_of(DESIGN);
    let sketch = match &steps[3].work {
        Work::Sketch(s) => s,
        _ => panic!("feature 5 is a sketch"),
    };
    // Five driven coordinates (p2.x, p3.x, p3.y, p4.x, p4.y) each become a
    // Distance* dimension bound by an equation; the two zero coordinates on
    // p1 are recorded notes (no zero-length dimension can land).
    let driver_dims: Vec<_> = sketch
        .dimensions
        .iter()
        .filter(|d| matches!(d.kind, DimKind::DistanceX | DimKind::DistanceY))
        .collect();
    assert_eq!(driver_dims.len(), 5);
    assert_eq!(sketch.dim_equations.len(), 5);
    assert_eq!(sketch.driver_notes.len(), 2);
    for note in &sketch.driver_notes {
        assert!(note.contains("zero delta"), "{note}");
    }
    // The fix constraint is a faithful no-op and must not degrade.
    assert!(sketch.dropped_constraints.is_empty());
    // The fixed point p4 is driven by its expressions (precedence rule).
    let driven_fixed = sketch
        .dim_equations
        .iter()
        .filter(|(_, eq)| eq.rhs.contains("VaneRise"))
        .count();
    assert!(driven_fixed >= 1, "p4.y must follow VaneRise");
}

#[test]
fn sparse_and_huge_feature_ids_resolve_by_map() {
    let steps = plan_of(DESIGN);
    let ex1 = match &steps[4].work {
        Work::Extrude(e) => e,
        _ => panic!(),
    };
    let ex2 = match &steps[7].work {
        Work::Extrude(e) => e,
        _ => panic!(),
    };
    // Feature 100000 references sketch 9002: the reference is a map lookup
    // over source ids, never an index.
    assert_eq!(ex1.sketch_step, 3);
    assert_eq!(ex2.sketch_step, 6);
}

#[test]
fn duplicate_and_out_of_bound_feature_ids_refuse() {
    let dup = DESIGN.replace("\"id\": 9000", "\"id\": 5");
    let err = plan_err(&dup);
    assert!(err.contains("not unique"), "{err}");
    let huge = DESIGN.replace("\"id\": 100000", "\"id\": 4503599627370496");
    let err = plan_err(&huge);
    assert!(err.contains("2^52"), "{err}");
}

#[test]
fn the_rollback_marker_excludes_features_named_without_stopping() {
    // Source-pinned semantics (nested-schema research A6): the marker is a
    // zero-based index into features; the feature AT that index and every
    // feature after it are rolled back - recorded, not executed, never a
    // stop. Parameters always evaluate.
    let design = DESIGN.replace("\"next_id\": 100001", "\"marker\": 1, \"next_id\": 100001");
    let steps = plan_of(&design);
    assert!(
        matches!(steps[3].work, Work::Sketch(_)),
        "feature at index 0 still replays"
    );
    for step in &steps[4..] {
        assert_eq!(step.status, PlanStatus::NotExecuted);
        assert!(matches!(step.work, Work::Note));
        assert!(
            step.detail.contains("past the timeline rollback marker"),
            "{}",
            step.detail
        );
    }
    // A marker past the last feature is a no-op cursor: everything replays.
    let ok = DESIGN.replace("\"next_id\": 100001", "\"marker\": 9, \"next_id\": 100001");
    assert_eq!(plan_of(&ok).len(), 8);
    assert_eq!(plan_of(&ok)[7].status, PlanStatus::Mapped);
    // A malformed marker still refuses.
    for marker in ["-1", "2.5", "\"x\""] {
        let design = DESIGN.replace(
            "\"next_id\": 100001",
            &format!("\"marker\": {marker}, \"next_id\": 100001"),
        );
        let err = plan_err(&design);
        assert!(err.contains("marker"), "{marker}: {err}");
    }
}

#[test]
fn centerline_and_infinite_curves_replay_as_construction_geometry() {
    // The source profile detector excludes construction, centerline and
    // infinite curves from bounding profiles; replay marks them all the
    // same way so a centerline can never split a profile.
    let design = r#"{"format": "solidcraft/57", "units": "mm", "params": [], "features": [
        {"id": 1, "name": "s", "suppressed": false, "type": "sketch",
         "plane": {"type": "origin", "name": "XY"},
         "sketch": {
           "points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true},
                      {"id": "p1", "pos": [10.0, 0.0]},
                      {"id": "p2", "pos": [3.0, 7.0]}],
           "curves": [
             {"id": "l1", "type": "line", "a": 0, "b": 1},
             {"id": "l2", "type": "line", "a": 1, "b": 2, "centerline": true},
             {"id": "l3", "type": "line", "a": 2, "b": 0, "infinite": true}
           ],
           "constraints": [], "counters": {"l": 3, "p": 3}}}]}"#;
    let steps = plan_of(design);
    let sketch = match &steps[0].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    let flags: Vec<bool> = sketch.lines.iter().map(|l| l.construction).collect();
    assert_eq!(flags, vec![false, true, true], "{flags:?}");
}

#[test]
fn angle_constraints_with_flip_are_dropped_not_misreplayed() {
    // flip selects the supplementary branch; replaying the primary angle
    // would be silently wrong, so the constraint degrades named.
    let design = r#"{"format": "solidcraft/57", "units": "mm", "params": [], "features": [
        {"id": 1, "name": "s", "suppressed": false, "type": "sketch",
         "plane": {"type": "origin", "name": "XY"},
         "sketch": {
           "points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true},
                      {"id": "p1", "pos": [10.0, 0.0]},
                      {"id": "p2", "pos": [3.0, 7.0]},
                      {"id": "p3", "pos": [-4.0, 5.0]}],
           "curves": [
             {"id": "l1", "type": "line", "a": 0, "b": 1},
             {"id": "l2", "type": "line", "a": 1, "b": 2},
             {"id": "l3", "type": "line", "a": 2, "b": 3}
           ],
           "constraints": [
             {"id": "k1", "type": "angle", "a": 1, "b": 2, "value": 0.7, "flip": true}
           ], "counters": {"l": 3, "p": 4}}}]}"#;
    let steps = plan_of(design);
    assert_eq!(steps[0].status, PlanStatus::DegradePlanned);
    let sketch = match &steps[0].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert!(
        sketch.dimensions.is_empty(),
        "no angle dimension may be driven"
    );
    assert!(
        sketch
            .dropped_constraints
            .iter()
            .any(|d| d.contains("flip")),
        "{:?}",
        sketch.dropped_constraints
    );
}

#[test]
fn operations_outside_new_body_stop_named() {
    for op in ["join", "cut", "intersect"] {
        let design = DESIGN.replace(
            "\"operation\": \"new_body\"",
            &format!("\"operation\": \"{op}\""),
        );
        let steps = plan_of(&design);
        // The first new_body is feature 9000 at index 4: the replacement
        // hits all three operations in the file, so the first stop is there.
        assert_eq!(steps[4].status, PlanStatus::FallbackPlanned);
        assert!(matches!(steps[4].work, Work::Unmappable));
        assert!(
            steps[4].detail.contains("Phase B body registry"),
            "{op}: {}",
            steps[4].detail
        );
        // Prefix semantics: everything after the stop is not-executed.
        assert_eq!(steps[5].status, PlanStatus::NotExecuted);
        assert!(matches!(steps[5].work, Work::AfterStop));
    }
}

#[test]
fn unknown_feature_types_stop_named() {
    let design = DESIGN.replace(
        "\"id\": 100000,\n      \"name\": \"Tail body\"",
        "\"id\": 100000,\n      \"name\": \"Tail body\",\n      \"teleport\": true",
    );
    let design = design.replace(
        "\"type\": \"extrude\",\n      \"sketch\": 9002",
        "\"type\": \"teleport\",\n      \"sketch\": 9002",
    );
    let steps = plan_of(&design);
    assert_eq!(steps[7].status, PlanStatus::FallbackPlanned);
    assert!(
        steps[7].detail.contains("\"teleport\""),
        "{}",
        steps[7].detail
    );
}

#[test]
fn suppression_skips_without_stopping() {
    // A suppressed feature is a recorded note; the timeline continues, so
    // the sketches around it map normally.
    let steps = plan_of(DESIGN);
    assert_eq!(steps[5].status, PlanStatus::NotExecuted);
    assert!(matches!(steps[5].work, Work::Note));
    assert!(steps[5].detail.contains("suppressed"));
    assert_eq!(steps[6].status, PlanStatus::Mapped);
}

#[test]
fn offset_planes_plan_with_bound_distance_equations() {
    // Inline offset form (the plaque's): the sketch step owns the plane
    // and its distance equation.
    let points = r#"{ "id": "origin", "pos": [0.0, 0.0], "fixed": true },
        { "id": "p1", "pos": [50.0, 0.0], "fixed": true },
        { "id": "p2", "pos": [12.0, 30.0], "fixed": true }"#;
    let plane =
        r#"{ "type": "offset", "base": { "type": "origin", "name": "XY" }, "distance": "W" }"#;
    let steps = plan_of(&sketch_design(7, plane, points));
    let sketch = match &steps[1].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    match &sketch.plane_binding {
        replay::PlaneBinding::Offset { distance } => {
            assert!((distance.expect_si - 0.05).abs() < 1e-12, "{distance:?}");
            assert!(distance.rhs.contains("W"), "{}", distance.rhs);
        }
        other => panic!("expected an owned offset plane: {other:?}"),
    }
    // A zero offset binds to the base plane (defined behavior).
    let zero =
        r#"{ "type": "offset", "base": { "type": "origin", "name": "XY" }, "distance": "0 mm" }"#;
    let steps = plan_of(&sketch_design(7, zero, points));
    let sketch = match &steps[1].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert_eq!(sketch.plane_binding, replay::PlaneBinding::Origin);
    // A tiny positive offset cannot establish its side (below the
    // placement tolerance): named stop.
    let tiny = r#"{ "type": "offset", "base": { "type": "origin", "name": "XY" }, "distance": "0.0000005 m" }"#;
    let steps = plan_of(&sketch_design(7, tiny, points));
    assert!(matches!(steps[1].work, Work::Unmappable));
    assert!(
        steps[1].detail.contains("ambiguous offset"),
        "{}",
        steps[1].detail
    );
    // A negative offset selects the unprobed side: named stop.
    let neg = r#"{ "type": "offset", "base": { "type": "origin", "name": "XY" }, "distance": "0 mm - W" }"#;
    let steps = plan_of(&sketch_design(7, neg, points));
    assert!(matches!(steps[1].work, Work::Unmappable));
    assert!(
        steps[1].detail.contains("negative offset"),
        "{}",
        steps[1].detail
    );
}

#[test]
fn construction_plane_steps_plan_and_sketches_bind_to_them() {
    // Micro-D01 shape: a construction_plane feature (offset form) followed
    // by a sketch that references it; the reference resolves to the plane
    // step's dense index.
    let design = r#"{"format": "solidcraft/57", "units": "mm",
        "params": [{ "name": "W", "expr": "50 mm", "unit": "mm" }],
        "features": [
          {"id": 40, "name": "PlaquePlane", "suppressed": false, "type": "construction_plane",
           "plane": {"type": "offset", "base": {"type": "origin", "name": "XY"}, "distance": "W"}},
          {"id": 41, "name": "S", "suppressed": false, "type": "sketch",
           "plane": {"type": "construction", "feature": 40},
           "sketch": {
             "points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true},
                        {"id": "p1", "pos": [50.0, 0.0], "fixed": true},
                        {"id": "p2", "pos": [12.0, 30.0], "fixed": true}],
             "curves": [
               {"id": "l1", "type": "line", "a": 0, "b": 1},
               {"id": "l2", "type": "line", "a": 1, "b": 2},
               {"id": "l3", "type": "line", "a": 2, "b": 0}],
             "constraints": [], "counters": {"l": 3, "p": 3}}}
        ]}"#;
    let steps = plan_of(design);
    let plane = match &steps[1].work {
        Work::OffsetPlane(p) => p,
        _ => panic!(
            "the construction_plane must plan as a plane step: {:?}",
            steps[1]
        ),
    };
    assert_eq!(plane.name, "CB_Plane_1");
    let expect = plane.distance.as_ref().map(|d| d.expect_si).unwrap_or(0.0);
    assert!((expect - 0.05).abs() < 1e-12);
    let sketch = match &steps[2].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert_eq!(
        sketch.plane_binding,
        replay::PlaneBinding::Constructed {
            step: 1,
            expect_si: 0.05
        }
    );
}

#[test]
fn construction_plane_references_refuse_named_per_case() {
    let points = r#"{ "id": "origin", "pos": [0.0, 0.0], "fixed": true },
        { "id": "p1", "pos": [50.0, 0.0], "fixed": true },
        { "id": "p2", "pos": [12.0, 30.0], "fixed": true }"#;
    let sketch = |plane: &str| sketch_design(7, plane, points);
    // Wrong type: the referenced id EXISTS but is the helper's own sketch
    // (id 7), not a construction_plane.
    let steps = plan_of(&sketch(r#"{ "type": "construction", "feature": 7 }"#));
    assert!(matches!(steps[1].work, Work::Unmappable));
    assert!(
        steps[1].detail.contains("not a construction_plane step"),
        "{}",
        steps[1].detail
    );
    // Malformed reference ids refuse named - never a silent feature 0.
    for bad in [
        r#"{ "type": "construction" }"#,
        r#"{ "type": "construction", "feature": "40" }"#,
        r#"{ "type": "construction", "feature": -3 }"#,
        r#"{ "type": "construction", "feature": 2.5 }"#,
    ] {
        let steps = plan_of(&sketch(bad));
        assert!(matches!(steps[1].work, Work::Unmappable), "{bad}");
        assert!(
            steps[1].detail.contains("feature id"),
            "{bad}: {}",
            steps[1].detail
        );
    }
    // Absent feature.
    let steps = plan_of(&sketch(r#"{ "type": "construction", "feature": 999 }"#));
    assert!(matches!(steps[1].work, Work::Unmappable));
    assert!(
        steps[1].detail.contains("not a construction_plane step"),
        "{}",
        steps[1].detail
    );
    // Suppressed plane step.
    let design = r#"{"format": "solidcraft/57", "units": "mm", "params": [],
        "features": [
          {"id": 40, "name": "P", "suppressed": true, "type": "construction_plane",
           "plane": {"type": "offset", "base": {"type": "origin", "name": "XY"}, "distance": "12 mm"}},
          {"id": 41, "name": "S", "suppressed": false, "type": "sketch",
           "plane": {"type": "construction", "feature": 40},
           "sketch": {"points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true}],
             "curves": [], "constraints": [], "counters": {"l": 0, "p": 1}}}
        ]}"#;
    let steps = plan_of(design);
    assert!(matches!(steps[1].work, Work::Unmappable));
    assert!(
        steps[1].detail.contains("suppressed"),
        "{}",
        steps[1].detail
    );
    // Forward reference: the plane step comes after the sketch.
    let design = r#"{"format": "solidcraft/57", "units": "mm", "params": [],
        "features": [
          {"id": 41, "name": "S", "suppressed": false, "type": "sketch",
           "plane": {"type": "construction", "feature": 40},
           "sketch": {"points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true}],
             "curves": [], "constraints": [], "counters": {"l": 0, "p": 1}}},
          {"id": 40, "name": "P", "suppressed": false, "type": "construction_plane",
           "plane": {"type": "offset", "base": {"type": "origin", "name": "XY"}, "distance": "12 mm"}}
        ]}"#;
    let steps = plan_of(design);
    // params: [] - the sketch is the first step and it stops named.
    assert!(matches!(steps[0].work, Work::Unmappable));
    assert!(
        steps[0].detail.contains("forward reference"),
        "{}",
        steps[0].detail
    );
    // A non-replayed plane form on the construction_plane itself.
    let design = r#"{"format": "solidcraft/57", "units": "mm", "params": [],
        "features": [
          {"id": 40, "name": "P", "suppressed": false, "type": "construction_plane",
           "plane": {"type": "along_path", "path": {"type": "point", "p": [0,0,0]}, "t": "0"}},
          {"id": 41, "name": "S", "suppressed": false, "type": "sketch",
           "plane": {"type": "construction", "feature": 40},
           "sketch": {"points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true}],
             "curves": [], "constraints": [], "counters": {"l": 0, "p": 1}}}
        ]}"#;
    let steps = plan_of(design);
    assert!(matches!(steps[0].work, Work::Unmappable));
    assert!(
        steps[0].detail.contains("only origin-based offset planes"),
        "{}",
        steps[0].detail
    );
    // The stop rule then carries the downstream sketch as not-executed -
    // the plane step's named reason is the record.
    assert!(matches!(steps[1].work, Work::AfterStop));
}

#[test]
fn point_expressions_win_over_stored_positions() {
    // The source engine fixes expression points at their EVALUATED
    // positions; a stored pos that disagrees is a stale cache and loses.
    let points = r#"{ "id": "origin", "pos": [0.0, 0.0], "fixed": true },
        { "id": "p1", "pos": [40.0, 0.0], "exprs": { "x": "W" } },
        { "id": "p2", "pos": [12.0, 30.0] }"#;
    let steps = plan_of(&sketch_design(
        7,
        r#"{ "type": "origin", "name": "XY" }"#,
        points,
    ));
    let sketch = match &steps[1].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert!(
        sketch
            .driver_notes
            .iter()
            .any(|n| n.contains("the expression wins")),
        "{:?}",
        sketch.driver_notes
    );
    assert_eq!(sketch.points[1][0], 50.0);
    // The driver dimension carries the evaluated value.
    let dim = sketch
        .dimensions
        .iter()
        .find(|d| matches!(d.kind, DimKind::DistanceX))
        .unwrap();
    assert_eq!(dim.value, 50.0);
}

#[test]
fn expressions_outside_the_subset_degrade_named() {
    let points = r#"{ "id": "origin", "pos": [0.0, 0.0], "fixed": true },
        { "id": "p1", "pos": [50.0, 0.0], "exprs": { "x": "sin(W)" } },
        { "id": "p2", "pos": [12.0, 30.0] }"#;
    let steps = plan_of(&sketch_design(
        7,
        r#"{ "type": "origin", "name": "XY" }"#,
        points,
    ));
    assert_eq!(steps[1].status, PlanStatus::DegradePlanned);
    let sketch = match &steps[1].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert!(
        sketch
            .dropped_constraints
            .iter()
            .any(|d| d.contains("not replayable as a driver")),
        "{:?}",
        sketch.dropped_constraints
    );
}

#[test]
fn the_57_surface_counts_and_summarizes_in_runtime_order() {
    let limits = Limits::default();
    let value = recipe::validate(DESIGN.as_bytes(), &limits).unwrap();
    // Parameters first, then features: the addressable runtime count.
    assert_eq!(recipe::step_count(&value).unwrap(), 8);
    let (steps, params) = recipe::summarize(&value);
    assert_eq!(steps.len(), 8);
    assert_eq!(steps[0].command, "parameter");
    assert_eq!(steps[3].command, "sketch");
    assert_eq!(steps[4].command, "extrude");
    assert_eq!(params.user.len(), 3);
    assert!(params.model.is_empty());
}

#[test]
fn a_57_artifact_packs_and_builds() {
    let dir = tempfile::tempdir().unwrap();
    let recipe_path = dir.path().join("design.json");
    let step_path = dir.path().join("part.step");
    let artifact = dir.path().join("design.craftx");
    let out: PathBuf = dir.path().join("design.swb");
    fs::write(&recipe_path, DESIGN).unwrap();
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
    build::build(
        &BuildOptions {
            artifact: &artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: true,
        },
        &Limits::default(),
    )
    .unwrap();
    let macro_text = fs::read_to_string(&out).unwrap();
    // The parameter equations land as global variables, and the extrude
    // depth equation follows VaneThickness.
    assert!(macro_text.contains("VaneSpan"), "global equations missing");
    assert!(
        macro_text.contains("VaneThickness"),
        "extent equation missing"
    );
}

#[test]
fn a_valid_id_just_below_the_source_limit_plans() {
    // Near-2^52 ids are legal source identity (the source caps at 2^52).
    let design = DESIGN.replace("\"id\": 100000", "\"id\": 4503599627370495");
    let steps = plan_of(&design);
    assert_eq!(steps[7].source_id, "feature 4503599627370495");
    assert!(matches!(steps[7].work, Work::Extrude(_)));
}

#[test]
fn snapshot_boundaries_address_dense_indices_with_parameter_offsets() {
    // The join extrude sits at dense index 4: three parameter equations
    // come first. The boundary addresses the RUNTIME index (never the
    // source feature id 9000), parameter offset included.
    let design = DESIGN.replace("\"operation\": \"new_body\"", "\"operation\": \"join\"");
    let value = {
        let limits = Limits::default();
        recipe::validate(design.as_bytes(), &limits).unwrap()
    };
    let limits = Limits::default();
    // Without a boundary: the stop rule holds everything after.
    let steps = replay::plan(&value, &limits, &BTreeMap::new()).unwrap();
    assert!(matches!(steps[4].work, Work::Unmappable));
    assert!(matches!(steps[5].work, Work::AfterStop));
    // With a boundary at the stop's dense index: replay continues.
    let mut snaps = BTreeMap::new();
    snaps.insert(4usize, craft_bridge::profile::SnapshotScope::Boundary);
    let steps = replay::plan(&value, &limits, &snaps).unwrap();
    assert!(matches!(steps[4].work, Work::Unmappable));
    assert!(!matches!(steps[5].work, Work::AfterStop));
    assert_eq!(steps[6].status, PlanStatus::Mapped);
    // A boundary at a parameter's own index addresses that parameter step.
    let mut snaps = BTreeMap::new();
    snaps.insert(2usize, craft_bridge::profile::SnapshotScope::Boundary);
    let steps = replay::plan(&value, &limits, &snaps).unwrap();
    assert!(matches!(steps[2].work, Work::UserParam { .. }));
    assert!(matches!(steps[5].work, Work::AfterStop));
}

#[test]
fn pack_validates_snapshot_boundaries_against_the_step_count() {
    let dir = tempfile::tempdir().unwrap();
    let recipe_path = dir.path().join("design.json");
    let step_path = dir.path().join("part.step");
    fs::write(&recipe_path, DESIGN).unwrap();
    fs::write(&step_path, STEP_FIXTURE).unwrap();
    // Index 2 addresses the third parameter equation: accepted.
    let out = dir.path().join("ok.craftx");
    pack::pack(
        &PackOptions {
            recipe: &recipe_path,
            step: &step_path,
            out: &out,
            warnings: None,
            name: None,
            overwrite: false,
            snapshots: vec![(2usize, step_path.as_path())],
            final_snapshot: None,
        },
        &Limits::default(),
    )
    .unwrap();
    // Index 8 == step_count (3 parameters + 5 features): refused.
    let out = dir.path().join("bad.craftx");
    let err = pack::pack(
        &PackOptions {
            recipe: &recipe_path,
            step: &step_path,
            out: &out,
            warnings: None,
            name: None,
            overwrite: false,
            snapshots: vec![(8usize, step_path.as_path())],
            final_snapshot: None,
        },
        &Limits::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("past the last recipe step"), "{err}");
}

#[test]
fn malformed_control_fields_refuse_instead_of_defaulting() {
    // marker: present it must be a non-negative whole number.
    for (marker, want) in [
        ("-1", "not a non-negative whole number"),
        ("2.5", "not a non-negative whole number"),
        ("\"x\"", "not a number"),
    ] {
        let design = DESIGN.replace(
            "\"next_id\": 100001",
            &format!("\"marker\": {marker}, \"next_id\": 100001"),
        );
        let err = plan_err(&design);
        assert!(err.contains(want), "{marker}: {err}");
    }
    // params: present it must be an array.
    let design = sketch_design(
        7,
        r#"{ "type": "origin", "name": "XY" }"#,
        r#"{ "id": "origin", "pos": [0.0, 0.0], "fixed": true }"#,
    )
    .replace(
        "\"params\": [{ \"name\": \"W\", \"expr\": \"50 mm\", \"unit\": \"mm\" }]",
        "\"params\": {}",
    );
    let err = plan_err(&design);
    assert!(err.contains("params is present but not an array"), "{err}");
    // suppressed: present it must be a boolean.
    let design = DESIGN.replace("\"suppressed\": false", "\"suppressed\": \"yes\"");
    let err = plan_err(&design);
    assert!(
        err.contains("suppressed is present but not a boolean"),
        "{err}"
    );
    // operation: present it must be a string (absence alone means new_body).
    let design = DESIGN.replace("\"operation\": \"new_body\"", "\"operation\": 5");
    let steps = plan_of(&design);
    assert!(matches!(steps[4].work, Work::Unmappable));
    assert!(
        steps[4]
            .detail
            .contains("operation is present but not a string"),
        "{}",
        steps[4].detail
    );
}

#[test]
fn geometry_affecting_primitive_semantics_stop_named() {
    // A centred box is not a corner box: rebuilding it from the corner
    // would be silently wrong.
    let design = DESIGN
        .replace("\"suppressed\": true", "\"suppressed\": false")
        .replace(
            "\"height\": \"7 mm\",",
            "\"height\": \"7 mm\", \"centred\": true,",
        );
    let steps = plan_of(&design);
    assert!(matches!(steps[5].work, Work::Unmappable));
    assert!(
        steps[5].detail.contains("centred box placement"),
        "{}",
        steps[5].detail
    );
    // Cylinder placement references are not replayed either.
    let design = r#"{"format": "solidcraft/57", "units": "mm", "params": [], "features": [
            {"id": 1, "name": "peg", "suppressed": false, "type": "cylinder",
              "base": ["0", "0", "0"], "axis": [0, 0, 1], "radius": "5 mm", "height": "9 mm",
              "references": [{"face": 3}], "operation": "new_body"}]}"#;
    let steps = plan_of(design);
    assert!(matches!(steps[0].work, Work::Unmappable));
    assert!(
        steps[0].detail.contains("cylinder references"),
        "{}",
        steps[0].detail
    );
}

#[test]
fn documents_outside_the_single_part_boundary_refuse() {
    let design = DESIGN.replace(
        "\"units\": \"mm\",",
        "\"units\": \"mm\", \"components\": [{ \"id\": 1 }],",
    );
    let err = plan_err(&design);
    assert!(err.contains("outside the v1 replay boundary"), "{err}");
    // Configurations and point clouds are evaluated state too (research
    // 06): a config row can override parameters and suppression, so a
    // replay that ignored it would build the wrong model.
    for (field, value) in [
        ("\"configs\": {\"columns\": [], \"rows\": [], \"row_identities\": {}, \"active\": \"\", \"rules\": []}", "configs"),
        ("\"point_clouds\": [{ \"id\": 1 }]", "point_clouds"),
    ] {
        let design = DESIGN.replace("\"units\": \"mm\",", &format!("\"units\": \"mm\", {field},"));
        let err = plan_err(&design);
        assert!(err.contains(value), "{value}: {err}");
    }
}

#[test]
fn text_generated_outlines_degrade_named() {
    // sketch.links text sources regenerate their outlines on edit; a
    // replay can only freeze the geometry it sees - and must say so.
    let design = sketch_design(
        7,
        r#"{ "type": "origin", "name": "XY" }"#,
        r#"{ "id": "origin", "pos": [0.0, 0.0], "fixed": true },
        { "id": "p1", "pos": [10.0, 0.0], "fixed": true },
        { "id": "p2", "pos": [3.0, 7.0], "fixed": true }"#,
    )
        .replace(
            "\"constraints\": []",
            "\"constraints\": [], \"links\": [{ \"id\": \"t1\", \"source\": { \"type\": \"text\", \"text\": \"HI\" } }]",
        );
    let steps = plan_of(&design);
    assert_eq!(steps[1].status, PlanStatus::DegradePlanned);
    let sketch = match &steps[1].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert!(
        sketch
            .dropped_constraints
            .iter()
            .any(|d| d.contains("text/generated link")),
        "{:?}",
        sketch.dropped_constraints
    );
}

#[test]
fn non_length_coordinate_expressions_are_refused() {
    let points = r#"{ "id": "origin", "pos": [0.0, 0.0], "fixed": true },
        { "id": "p1", "pos": [50.0, 0.0], "exprs": { "x": "3 deg" } },
        { "id": "p2", "pos": [12.0, 30.0] }"#;
    let steps = plan_of(&sketch_design(
        7,
        r#"{ "type": "origin", "name": "XY" }"#,
        points,
    ));
    assert_eq!(steps[1].status, PlanStatus::DegradePlanned);
    let sketch = match &steps[1].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert!(
        sketch
            .dropped_constraints
            .iter()
            .any(|d| d.contains("is not a length")),
        "{:?}",
        sketch.dropped_constraints
    );
}

#[test]
fn the_driver_anchor_is_chosen_after_evaluation_and_must_hold() {
    // A point CACHED at the origin but driven away by its expression is
    // not an anchor: with no other anchor, the drivers degrade named
    // instead of dimensioning against a moving point.
    let points = r#"{ "id": "origin", "pos": [0.0, 0.0], "fixed": true, "exprs": { "x": "W" } },
        { "id": "p1", "pos": [50.0, 0.0], "exprs": { "x": "W" } },
        { "id": "p2", "pos": [12.0, 30.0] }"#;
    let steps = plan_of(&sketch_design(
        7,
        r#"{ "type": "origin", "name": "XY" }"#,
        points,
    ));
    let sketch = match &steps[1].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert!(
        sketch.dropped_constraints.iter().any(|d| d
            .contains("no pinned (fixed or fix-constrained) curve endpoint anchors the sketch")),
        "{:?}",
        sketch.dropped_constraints
    );
    // With a genuine fixed anchor at the origin, the drivers land against
    // it - and the moving point still follows its expression.
    let points = r#"{ "id": "moving", "pos": [0.0, 0.0], "exprs": { "x": "W" } },
        { "id": "anchor", "pos": [0.0, 0.0], "fixed": true },
        { "id": "p1", "pos": [50.0, 0.0], "exprs": { "x": "W" } }"#;
    let steps = plan_of(&sketch_design(
        7,
        r#"{ "type": "origin", "name": "XY" }"#,
        points,
    ));
    let sketch = match &steps[1].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert!(
        sketch.dropped_constraints.is_empty(),
        "{:?}",
        sketch.dropped_constraints
    );
    // moving.x, p1.x = two drivers. Both measure against the anchor's
    // endpoint slot (1: l1's second endpoint), never a point against
    // itself.
    assert_eq!(sketch.dim_equations.len(), 2);
    for (di, _) in &sketch.dim_equations {
        let d = &sketch.dimensions[*di];
        assert_eq!(d.a, 1, "the anchor slot is l1's second endpoint");
        assert_ne!(d.a, d.b.unwrap_or(usize::MAX), "no self-dimension");
    }
}

#[test]
fn the_plan_report_publishes_source_identity_and_driver_notes() {
    let steps = plan_of(DESIGN);
    let report = craft_bridge::report::build_report(&steps, "design.craftx", "design.swb");
    let sketch_step = &report.steps[3];
    assert_eq!(sketch_step.source_id, "feature 5");
    assert_eq!(sketch_step.notes.len(), 2, "{:?}", sketch_step.notes);
    // The parameter steps carry their source names too.
    assert_eq!(report.steps[0].source_id, "parameter VaneSpan");
}

#[test]
fn constraints_on_unoccupied_points_degrade_instead_of_refusing() {
    // The micro designs tie curve endpoints to the sketch origin with a
    // coincident constraint. The origin point occupies no curve endpoint,
    // so the relation cannot be replayed - but the design is valid and
    // the build must proceed with a named degrade, not refuse.
    let design = r#"{"format": "solidcraft/57", "units": "mm", "params": [], "features": [
        {"id": 1, "name": "s", "suppressed": false, "type": "sketch",
         "plane": {"type": "origin", "name": "XY"},
         "sketch": {
           "points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true},
                      {"id": "p1", "pos": [10.0, 0.0], "fixed": true},
                      {"id": "p2", "pos": [3.0, 7.0], "fixed": true}],
           "curves": [
             {"id": "l1", "type": "line", "a": 1, "b": 2},
             {"id": "l2", "type": "line", "a": 2, "b": 1}
           ],
           "constraints": [
             {"id": "k1", "type": "coincident", "p": 0, "q": 1},
             {"id": "k2", "type": "fix", "p": 1}
           ], "counters": {"l": 2, "p": 3}}}]}"#;
    let steps = plan_of(design);
    assert_eq!(steps[0].status, PlanStatus::DegradePlanned);
    let sketch = match &steps[0].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert!(
        sketch
            .dropped_constraints
            .iter()
            .any(|d| d.contains("no curve endpoint occupies")),
        "{:?}",
        sketch.dropped_constraints
    );
    // The fix on a real endpoint becomes a real sgFIXED pin relation.
    assert!(
        sketch
            .relations
            .iter()
            .any(|r| matches!(r, replay::RelationT::Fix(_))),
        "{:?}",
        sketch.relations
    );
}

#[test]
fn fix_constraints_pin_endpoints_as_fix_relations() {
    // D02/D10 shape: the anchor arrives as a source fix constraint on a
    // curve endpoint, and the coordinate drivers anchor on it.
    let points = r#"{ "id": "p0", "pos": [-3.2, 1.3], "fixed": false },
        { "id": "p1", "pos": [34.5, 0.0], "fixed": true, "exprs": { "x": "W - 2.8 mm" } },
        { "id": "p2", "pos": [39.2, 7.6], "fixed": true, "exprs": { "x": "W + 1.9 mm" } }"#;
    let design = sketch_design(7, r#"{ "type": "origin", "name": "XY" }"#, points).replace(
        "\"constraints\": []",
        "\"constraints\": [{ \"id\": \"k1\", \"type\": \"fix\", \"p\": 0 }]",
    );
    let steps = plan_of(&design);
    let sketch = match &steps[1].work {
        Work::Sketch(s) => s,
        _ => panic!(),
    };
    assert!(
        sketch.dropped_constraints.is_empty(),
        "{:?}",
        sketch.dropped_constraints
    );
    // The fix relation exists (from the constraint) and the drivers use
    // the delta form against the pinned point: Width drives x offsets from
    // p0 at -3.2 mm, so the equations carry the anchor correction.
    let fix_count = sketch
        .relations
        .iter()
        .filter(|r| matches!(r, replay::RelationT::Fix(_)))
        .count();
    assert_eq!(fix_count, 1, "{:?}", sketch.relations);
    assert_eq!(sketch.dim_equations.len(), 2);
    for (_, eq) in &sketch.dim_equations {
        assert!(eq.rhs.contains("+ 3.2mm"), "{}", eq.rhs);
    }
}

#[test]
fn offset_plane_emission_uses_the_probe_pinned_shapes() {
    // The generated macro must carry the probe-backed call shapes
    // (docs/source-probe-evidence.md): the exact InsertRefPlane form, the
    // discovered-name dimension binding (never a hardcoded D1), the
    // feature-object sketch opening, the componentwise signed placement
    // check, the offset-aware restoration, and the plane fallback
    // lifecycle. Independently authored synthetic fixture.
    let design = r#"{"format": "solidcraft/57", "units": "mm",
        "params": [{ "name": "W", "expr": "50 mm", "unit": "mm" }],
        "features": [
          {"id": 40, "name": "P", "suppressed": false, "type": "construction_plane",
           "plane": {"type": "offset", "base": {"type": "origin", "name": "XY"}, "distance": "W"}},
          {"id": 41, "name": "OnPlane", "suppressed": false, "type": "sketch",
           "plane": {"type": "construction", "feature": 40},
           "sketch": {
             "points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true},
                        {"id": "p1", "pos": [50.0, 0.0], "fixed": true},
                        {"id": "p2", "pos": [12.0, 30.0], "fixed": true}],
             "curves": [
               {"id": "l1", "type": "line", "a": 0, "b": 1},
               {"id": "l2", "type": "line", "a": 1, "b": 2},
               {"id": "l3", "type": "line", "a": 2, "b": 0}],
             "constraints": [], "counters": {"l": 3, "p": 3}}},
          {"id": 42, "name": "InlinePlane", "suppressed": false, "type": "sketch",
           "plane": {"type": "offset", "base": {"type": "origin", "name": "XY"}, "distance": "12"},
           "sketch": {
             "points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true},
                        {"id": "p1", "pos": [50.0, 0.0], "fixed": true},
                        {"id": "p2", "pos": [12.0, 30.0], "fixed": true}],
             "curves": [
               {"id": "l1", "type": "line", "a": 0, "b": 1},
               {"id": "l2", "type": "line", "a": 1, "b": 2},
               {"id": "l3", "type": "line", "a": 2, "b": 0}],
             "constraints": [], "counters": {"l": 3, "p": 3}}}
        ]}"#;
    let dir = tempfile::tempdir().unwrap();
    let recipe_path = dir.path().join("design.json");
    let step_path = dir.path().join("part.step");
    let artifact = dir.path().join("design.craftx");
    let out: PathBuf = dir.path().join("design.swb");
    fs::write(&recipe_path, design).unwrap();
    fs::write(&step_path, STEP_FIXTURE).unwrap();
    pack::pack(
        &PackOptions {
            recipe: &recipe_path,
            step: &step_path,
            out: &artifact,
            warnings: None,
            name: None,
            overwrite: false,
            snapshots: vec![(2usize, step_path.as_path())],
            final_snapshot: None,
        },
        &Limits::default(),
    )
    .unwrap();
    build::build(
        &BuildOptions {
            artifact: &artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: true,
        },
        &Limits::default(),
    )
    .unwrap();
    let m = fs::read_to_string(&out).unwrap();
    // The probe-pinned creation call shape, exactly.
    assert!(
        m.contains("gFeatMgr.InsertRefPlane(8, "),
        "{:?}",
        "InsertRefPlane form"
    );
    assert!(
        m.contains("0#, 0#, 0#, 0#)"),
        "unused slots must be explicit zeros"
    );
    // Discovered-name binding: the dimension OBJECT, never a literal D1.
    assert!(
        m.contains("AddDimEquation(dm, "),
        "the binding must use the discovered dimension"
    );
    assert!(
        !m.contains("\"D1@"),
        "no hardcoded dimension name may appear"
    );
    // Unit-suffixed equation text (bare SolidWorks numbers are document units).
    assert!(
        m.contains("12mm"),
        "the bare literal distance must be unit-suffixed"
    );
    // Feature-object sketch opening and the offset acceptance twin.
    assert!(m.contains("OpenSketchOnNamedPlane("));
    assert!(m.contains("AcceptAndPublishOffset("));
    // Componentwise signed placement check.
    assert!(m.contains("refusing a mirrored or tangential placement"));
    // Exact generated-statement pins (the double-quoting blocker): the
    // name literal is a single VBA literal, never doubled.
    assert!(
        m.contains("feat.Name = \"CB_Plane_1\""),
        "the rename statement must carry one VBA literal"
    );
    assert!(
        !m.contains("feat.Name = \"\""),
        "no statement may carry a doubled name literal"
    );
    // The exact rename-check statement: the failure message closes its
    // paren INSIDE the string (a seat run died on this line: an unbalanced
    // paren/quote pair is a VBA compile error and the whole macro refuses
    // to load).
    assert!(
        m.contains("If feat.Name <> \"CB_Plane_1\" Then gMsg = \"the plane feature would not take the name CB_Plane_1 (reads \" & feat.Name & \")\": GoTo PlaneFail"),
        "the rename check must carry the balanced message statement"
    );
    // Every generated non-comment line has balanced VBA string quotes -
    // the scan that would have caught that compile error in CI.
    for (ln, line) in m.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('\'') {
            continue;
        }
        assert!(
            line.matches('"').count() % 2 == 0,
            "line {} carries an odd quote count (unbalanced VBA string): {}",
            ln + 1,
            line
        );
    }
    // The probe-shaped capture: the reference-plane return and the tree
    // feature are separate objects.
    assert!(m.contains("Set refPlane = gFeatMgr.InsertRefPlane("));
    // The open helper takes the LIVE captured feature object (the probe's
    // proven selection form); every call site passes it, and every helper
    // exit records its named reason in gProbeLog (silent exits once left
    // a seat run diagnosing nothing).
    assert!(
        m.contains("Function OpenSketchOnNamedPlane(ByVal idx As Long, feat As SldWorks.Feature")
    );
    let mut call_sites = 0;
    for line in m.lines() {
        if line.contains("OpenSketchOnNamedPlane(") && !line.trim_start().starts_with("Function") {
            assert!(
                line.contains("planeFeat"),
                "the open helper must be passed the stored feature object: {line}"
            );
            call_sites += 1;
        }
    }
    assert!(
        call_sites >= 1,
        "at least one constructed/inline sketch opens on its plane"
    );
    let helper_body = {
        let hs = m
            .find("Function OpenSketchOnNamedPlane(")
            .expect("the open helper exists");
        let he = m[hs..].find("End Function").map(|e| hs + e).unwrap();
        &m[hs..he]
    };
    assert!(
        helper_body.matches("gProbeLog = gProbeLog &").count() >= 5,
        "every open-helper exit must record a named reason"
    );
    assert!(m.contains("Set feat = swDoc.Extension.GetLastFeatureAdded"));
    // Exactly one inline builder call in the inline sketch's BuildSketch,
    // and it runs before the Degrade label (creation happens once).
    let bs_idx = m
        .find("Function BuildSketch3()")
        .expect("inline sketch sub");
    let bs_end = m[bs_idx..]
        .find("End Function")
        .map(|e| bs_idx + e)
        .unwrap();
    let bs_body = &m[bs_idx..bs_end];
    assert_eq!(
        bs_body.matches("st = BuildPlane3()").count(),
        1,
        "the inline plane is created exactly once per build"
    );
    let call_pos = bs_body.find("st = BuildPlane3()").unwrap();
    let degrade_pos = bs_body.find("Degrade:").expect("degrade label exists");
    assert!(
        call_pos < degrade_pos,
        "plane creation must precede the degrade path"
    );
    // Restoration: every CleanSketch re-accepts with the offset twin and
    // no CleanSketch recreates a plane or its equation.
    let mut search = 0usize;
    let mut cleans = 0;
    while let Some(c) = m[search..].find("Function CleanSketch") {
        let start = search + c;
        let end = m[start..].find("End Function").map(|e| start + e).unwrap();
        let body = &m[start..end];
        assert!(
            body.contains("AcceptAndPublishOffset("),
            "restoration must use the offset acceptance: {body}"
        );
        assert!(
            !body.contains("BuildPlane"),
            "restoration must not recreate the plane: {body}"
        );
        assert!(
            !body.contains("AddDimEquation"),
            "restoration must not re-add the offset equation: {body}"
        );
        cleans += 1;
        search = end;
    }
    assert_eq!(
        cleans, 2,
        "both offset-bound sketches have restoration paths"
    );
    // Deletion order: the inline sketch's sketch deletion precedes its
    // plane deletion (the plane is owned by the sketch).
    let del_idx = m.find("Sub DeleteCreated()").expect("DeleteCreated exists");
    let del_end = m[del_idx..].find("End Sub").map(|e| del_idx + e).unwrap();
    let del_body = &m[del_idx..del_end];
    let sketch_del = del_body
        .find("skName3")
        .expect("the inline sketch is deleted");
    let plane_del = del_body.find("planeName3").expect("its plane is deleted");
    assert!(
        sketch_del < plane_del,
        "the sketch must be deleted before its owned plane"
    );
    assert!(
        del_body.contains("planeName"),
        "plane bindings must be cleared"
    );
    // Registry sizing covers the plane equations (slots 0 and 1 here).
    assert!(
        m.contains("Dim regName(0 To 1) As String"),
        "registry sizing must include plane slots"
    );
    // The plane evidence reaches the runtime report (never overwritten).
    assert!(
        m.contains(" | plane: "),
        "the sketch messages must carry the plane evidence"
    );
}

#[test]
fn shared_planes_and_multiple_inline_planes_plan_and_emit_distinctly() {
    // Two sketches share ONE construction plane (one plane step, one
    // builder); a third sketch owns its own inline plane. The macro must
    // contain exactly two BuildPlane functions.
    let tri = r#""points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true},
                {"id": "p1", "pos": [50.0, 0.0], "fixed": true},
                {"id": "p2", "pos": [12.0, 30.0], "fixed": true}],
            "curves": [
              {"id": "l1", "type": "line", "a": 0, "b": 1},
              {"id": "l2", "type": "line", "a": 1, "b": 2},
              {"id": "l3", "type": "line", "a": 2, "b": 0}],
            "constraints": [], "counters": {"l": 3, "p": 3}"#;
    let design = format!(
        r#"{{"format": "solidcraft/57", "units": "mm",
        "params": [{{ "name": "W", "expr": "50 mm", "unit": "mm" }}],
        "features": [
          {{"id": 40, "name": "P", "suppressed": false, "type": "construction_plane",
           "plane": {{"type": "offset", "base": {{"type": "origin", "name": "XY"}}, "distance": "W"}}}},
          {{"id": 41, "name": "S1", "suppressed": false, "type": "sketch",
           "plane": {{"type": "construction", "feature": 40}},
           "sketch": {{{tri}}}}},
          {{"id": 42, "name": "S2", "suppressed": false, "type": "sketch",
           "plane": {{"type": "construction", "feature": 40}},
           "sketch": {{{tri}}}}},
          {{"id": 43, "name": "S3", "suppressed": false, "type": "sketch",
           "plane": {{"type": "offset", "base": {{"type": "origin", "name": "XY"}}, "distance": "12"}},
           "sketch": {{{tri}}}}}
        ]}}"#
    );
    let steps = plan_of(&design);
    let plane_idx = steps
        .iter()
        .find(|s| matches!(s.work, Work::OffsetPlane(_)))
        .map(|s| s.index)
        .expect("one plane step");
    for i in [2usize, 3] {
        match &steps[i].work {
            Work::Sketch(sk) => assert_eq!(
                sk.plane_binding,
                replay::PlaneBinding::Constructed {
                    step: plane_idx,
                    expect_si: 0.05
                },
                "sketch {i} must share the one plane step"
            ),
            other => panic!("sketch expected: {other:?}"),
        }
    }
    match &steps[4].work {
        Work::Sketch(sk) => assert!(matches!(
            sk.plane_binding,
            replay::PlaneBinding::Offset { .. }
        )),
        other => panic!("sketch expected: {other:?}"),
    }
    // Emission: one builder for the plane step, one for the inline plane.
    let dir = tempfile::tempdir().unwrap();
    let recipe_path = dir.path().join("design.json");
    let step_path = dir.path().join("part.step");
    let artifact = dir.path().join("design.craftx");
    let out: PathBuf = dir.path().join("design.swb");
    fs::write(&recipe_path, &design).unwrap();
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
    build::build(
        &BuildOptions {
            artifact: &artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: true,
        },
        &Limits::default(),
    )
    .unwrap();
    let m = fs::read_to_string(&out).unwrap();
    let builders = m.matches("Function BuildPlane").count();
    assert_eq!(
        builders, 2,
        "one builder per owned plane, shared planes count once"
    );
}

#[test]
fn boundary_fallback_invalidates_plane_resolution_named() {
    // plane step -> boundary-snapshot-covered unmappable step -> a sketch
    // referencing the plane. The fallback deletes every created plane at
    // runtime, so the planner must refuse the reference named.
    let design = r#"{"format": "solidcraft/57", "units": "mm",
        "params": [{ "name": "W", "expr": "50 mm", "unit": "mm" }],
        "features": [
          {"id": 40, "name": "P", "suppressed": false, "type": "construction_plane",
           "plane": {"type": "offset", "base": {"type": "origin", "name": "XY"}, "distance": "W"}},
          {"id": 41, "name": "Boom", "suppressed": false, "type": "teleport"},
          {"id": 42, "name": "S", "suppressed": false, "type": "sketch",
           "plane": {"type": "construction", "feature": 40},
           "sketch": {"points": [{"id": "origin", "pos": [0.0, 0.0], "fixed": true}],
             "curves": [], "constraints": [], "counters": {"l": 0, "p": 1}}}
        ]}"#;
    let limits = Limits::default();
    let value = recipe::validate(design.as_bytes(), &limits).unwrap();
    let mut snaps = BTreeMap::new();
    // dense indices: 0 = the W parameter, 1 = the plane step, 2 = the
    // unmappable step (the boundary covers it), 3 = the referencing sketch.
    snaps.insert(2usize, craft_bridge::profile::SnapshotScope::Boundary);
    let steps = replay::plan(&value, &limits, &snaps).unwrap();
    assert!(matches!(steps[2].work, Work::Unmappable));
    // Without a boundary the sketch would stop after the unmappable step
    // anyway; WITH the boundary it plans further and must refuse the
    // plane reference named (the fallback deleted the plane at runtime).
    assert_eq!(steps[3].status, PlanStatus::FallbackPlanned);
    assert!(
        steps[3]
            .detail
            .contains("deleted by the boundary snapshot fallback"),
        "{}",
        steps[3].detail
    );
}

/// A minimal one-sketch solidcraft/57 design carrying one conic curve.
fn conic_design(points: &str, conic: &str) -> String {
    format!(
        r#"{{
  "format": "solidcraft/57",
  "units": "mm",
  "params": [{{ "name": "W", "expr": "50 mm", "unit": "mm" }}],
  "features": [
    {{
      "id": 7,
      "name": "s",
      "suppressed": false,
      "type": "sketch",
      "plane": {{ "type": "origin", "name": "XY" }},
      "sketch": {{
        "points": [{points}],
        "curves": [{conic}],
        "constraints": [],
        "counters": {{ "l": 0, "p": 3 }}
      }}
    }},
    {{
      "id": 8, "name": "e", "suppressed": false, "type": "extrude",
      "sketch": 7, "op": "new",
      "extent": {{ "type": "distance", "distance": "10 mm", "direction": "one" }}
    }}
  ]
}}"#
    )
}

const CONIC_PTS: &str = r#"{ "id": "p0", "pos": [0.0, 0.0] },
        { "id": "q", "pos": [20.0, 8.0] },
        { "id": "p2", "pos": [40.0, 0.0] }"#;
const CONIC_OK: &str = r#"{ "id": "c1", "type": "conic", "a": 0, "b": 2, "apex": 1, "rho": 0.5 }"#;

#[test]
fn conic_validates_offline_and_stops_at_the_seat_gate() {
    let steps = plan_of(&conic_design(CONIC_PTS, CONIC_OK));
    // the parameter and then the sketch step; the sketch stops at the
    // research 09 P0 seat gate as a per-step Unmappable stop
    let sketch = &steps[1];
    assert!(
        matches!(sketch.work, Work::Unmappable),
        "the conic sketch must stop as unmappable: {:?}",
        sketch.work
    );
    for needle in [
        "native conic",
        "points a=0 apex=1 b=2",
        "rho 0.5",
        "class Parabola",
        "construction false, reversed false",
        "research 09 P0",
        "no curve is emitted",
    ] {
        assert!(
            sketch.detail.contains(needle),
            "missing {needle}: {}",
            sketch.detail
        );
    }
    // ordinary downstream stopping: the extrude is not executed
    assert!(
        matches!(steps[2].work, Work::Unmappable | Work::Note)
            || steps[2].status == PlanStatus::NotExecuted
    );
}

#[test]
fn conic_wrappers_travel_to_the_gate_message() {
    let conic = r#"{ "id": "c1", "type": "conic", "a": 0, "b": 2, "apex": 1, "rho": 0.5, "construction": true, "reversed": true }"#;
    let steps = plan_of(&conic_design(CONIC_PTS, conic));
    assert!(
        steps[1].detail.contains("construction true, reversed true"),
        "{}",
        steps[1].detail
    );
}

#[test]
fn malformed_conic_data_refuses_as_invalid_recipe() {
    // malformed data stays a whole-plan refusal (the InvalidRecipe
    // semantics) - distinct from the per-step seat-gate stop
    for (conic, needle) in [
        (
            r#"{ "id": "c1", "type": "conic", "a": 0, "b": 2, "apex": 99, "rho": 0.5 }"#,
            "references point 99 which does not exist",
        ),
        (
            r#"{ "id": "c1", "type": "conic", "a": 0, "b": 2, "apex": 1 }"#,
            "has no rho",
        ),
        (
            r#"{ "id": "c1", "type": "conic", "a": 0, "b": 2, "apex": 1, "rho": 1.5 }"#,
            "outside the open interval",
        ),
    ] {
        let msg = plan_err(&conic_design(CONIC_PTS, conic));
        assert!(msg.contains(needle), "{conic}: {msg}");
    }
    // a non-boolean wrapper is tolerated as absent (unwrap_or(false)) and
    // still reaches the seat gate - it is not malformed data
    let steps = plan_of(&conic_design(
        CONIC_PTS,
        r#"{ "id": "c1", "type": "conic", "a": 0, "b": 2, "apex": 1, "rho": 0.5, "construction": 3 }"#,
    ));
    assert!(
        steps[1].detail.contains("research 09 P0"),
        "{}",
        steps[1].detail
    );
    // a collinear control triangle refuses named
    let collinear = r#"{ "id": "p0", "pos": [0.0, 0.0] },
        { "id": "q", "pos": [20.0, 0.0] },
        { "id": "p2", "pos": [40.0, 0.0] }"#;
    let msg = plan_err(&conic_design(collinear, CONIC_OK));
    assert!(msg.contains("effectively collinear"), "{msg}");
}

#[test]
fn conic_validation_uses_evaluated_coordinates() {
    // expressions win over the stored pos (the source-engine rule):
    // a collinear stored triangle that expressions move off the line
    // passes validation and stops only at the seat gate
    let pts_expr_ok = r#"{ "id": "p0", "pos": [0.0, 0.0] },
        { "id": "q", "pos": [20.0, 0.0], "exprs": { "y": "8 mm" } },
        { "id": "p2", "pos": [40.0, 0.0] }"#;
    let steps = plan_of(&conic_design(pts_expr_ok, CONIC_OK));
    assert!(
        matches!(steps[1].work, Work::Unmappable) && steps[1].detail.contains("research 09 P0"),
        "{}",
        steps[1].detail
    );
    // and the reverse: a stored triangle expressions flatten refuses
    let pts_expr_bad = r#"{ "id": "p0", "pos": [0.0, 0.0] },
        { "id": "q", "pos": [20.0, 8.0], "exprs": { "y": "0 mm" } },
        { "id": "p2", "pos": [40.0, 0.0] }"#;
    let msg = plan_err(&conic_design(pts_expr_bad, CONIC_OK));
    assert!(msg.contains("effectively collinear"), "{msg}");
    // an unevaluable driver refuses named
    let pts_bad_expr = r#"{ "id": "p0", "pos": [0.0, 0.0] },
        { "id": "q", "pos": [20.0, 8.0], "exprs": { "y": "NoSuchParam" } },
        { "id": "p2", "pos": [40.0, 0.0] }"#;
    let msg = plan_err(&conic_design(pts_bad_expr, CONIC_OK));
    assert!(msg.contains("cannot be evaluated at plan time"), "{msg}");
}

#[test]
fn conic_conversion_is_pinned_to_metres() {
    // a sub-floor chord in SOURCE millimetres (1e-13 mm) only refuses as
    // coincident once the loader has converted mm -> m (the conic floor
    // is 1e-15 m): this pins the unit boundary independently of the
    // document units field
    let tiny = r#"{ "id": "p0", "pos": [0.0, 0.0] },
        { "id": "q", "pos": [20.0, 8.0] },
        { "id": "p2", "pos": [0.0000000000001, 0.0] }"#;
    let msg = plan_err(&conic_design(tiny, CONIC_OK));
    assert!(msg.contains("the endpoints coincide"), "{msg}");
}

#[test]
fn conic_stops_are_snapshot_survivable_and_emit_nothing() {
    use craft_bridge::profile::SnapshotScope;
    let design = conic_design(CONIC_PTS, CONIC_OK);
    // ordinary planning: the sketch stops at the gate and the extrude
    // downstream does not execute
    let steps = plan_of(&design);
    assert!(matches!(steps[1].work, Work::Unmappable));
    assert_eq!(steps[2].status, PlanStatus::NotExecuted);
    // a boundary snapshot at the stopped step lets planning continue -
    // the conic refusal is a per-step stop, never a global one
    let limits = Limits::default();
    let value = recipe::validate(design.as_bytes(), &limits).unwrap();
    let mut snaps = BTreeMap::new();
    snaps.insert(1usize, SnapshotScope::Boundary);
    let steps = replay::plan(&value, &limits, &snaps).unwrap();
    assert!(matches!(steps[1].work, Work::Unmappable));
    assert_eq!(
        steps[2].status,
        PlanStatus::FallbackPlanned,
        "{:?}",
        steps[2]
    );
    // the built macro carries NO conic or weighted-spline emission of any
    // kind (the research 09 release gate: no form is seat-proven)
    let dir = tempfile::tempdir().unwrap();
    let recipe_path = dir.path().join("design.json");
    let step_path = dir.path().join("part.step");
    let artifact = dir.path().join("design.craftx");
    let out: PathBuf = dir.path().join("design.swb");
    fs::write(&recipe_path, &design).unwrap();
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
    build::build(
        &BuildOptions {
            artifact: &artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: true,
        },
        &Limits::default(),
    )
    .unwrap();
    let m = fs::read_to_string(&out).unwrap();
    for forbidden in [
        "CreateConic",
        "CreateSpline",
        "CreateSplinesByEqnParams",
        "SplineParamData",
    ] {
        assert!(!m.contains(forbidden), "conic emission leaked: {forbidden}");
    }
}

/// A minimal solidcraft/57 design: one Phase B feature, then a sketch as
/// the downstream step (the stop-cascade and snapshot paths read it).
fn phase_design(feature: &str) -> String {
    format!(
        r#"{{
  "format": "solidcraft/57",
  "units": "mm",
  "params": [],
  "features": [
    {feature},
    {{
      "id": 90,
      "name": "s",
      "suppressed": false,
      "type": "sketch",
      "plane": {{ "type": "origin", "name": "XY" }},
      "sketch": {{
        "points": [{{ "id": "p0", "pos": [0.0, 0.0] }}, {{ "id": "p1", "pos": [10.0, 0.0] }}],
        "curves": [{{ "id": "l1", "type": "line", "a": 0, "b": 1 }}],
        "constraints": [],
        "counters": {{ "l": 1, "p": 2 }}
      }}
    }}
  ]
}}"#
    )
}

#[test]
fn phase_b_features_stop_at_named_seat_gates() {
    // fillet: the record validates (defaults noted) and stops naming the
    // research 12 P0 AND P1 gates and the identity gaps
    let fillet = r#"{ "id": 1, "name": "F", "suppressed": false, "type": "fillet", "edges": [[0,0,0],[1,0,0]], "edge_refs": [{"dir":[0,0,1],"min":[0,0,0],"max":[1,1,1]}] }"#;
    let steps = plan_of(&phase_design(fillet));
    assert!(matches!(steps[0].work, Work::Unmappable), "{:?}", steps[0]);
    for needle in [
        "2 edge pick(s), 1 edge_ref(s)",
        "research 12 P0 body-registry and edge-resolver probes",
        "P1 D06 fillet probe",
        "no feature is emitted",
        "BODY bounds, never edge identity (research 12 C05)",
        "no stable edge ordinals exist (C04)",
    ] {
        assert!(
            steps[0].detail.contains(needle),
            "{needle}: {}",
            steps[0].detail
        );
    }
    // move: the transform-split rule and the D04 gate are named
    let mov = r#"{ "id": 1, "name": "M", "suppressed": false, "type": "move", "bodies": ["b1"], "translate": ["10 mm", "0 mm", "0 mm"] }"#;
    let steps = plan_of(&phase_design(mov));
    for needle in [
        "1 bod(y/ies)",
        "P1 D04 move probe",
        "never mixed in one (research 12 C01)",
        "axis-angle conversion is unprobed (C07)",
    ] {
        assert!(
            steps[0].detail.contains(needle),
            "{needle}: {}",
            steps[0].detail
        );
    }
    // remove: the Body-Delete/Keep mapping is named
    let rm = r#"{ "id": 1, "name": "R", "suppressed": false, "type": "remove", "bodies": ["b1"] }"#;
    let steps = plan_of(&phase_design(rm));
    for needle in [
        "P1 D04 remove probe",
        "InsertDeleteBody2(False)",
        "no feature is emitted",
    ] {
        assert!(
            steps[0].detail.contains(needle),
            "{needle}: {}",
            steps[0].detail
        );
    }
    // combine: join/cut/intersect share the merge/scope gate
    for op in ["join", "cut", "intersect"] {
        let c = format!(
            r#"{{ "id": 1, "name": "C", "suppressed": false, "type": "combine", "target": "b1", "tools": ["b2"], "operation": "{op}" }}"#
        );
        let steps = plan_of(&phase_design(&c));
        for needle in [
            "P0 body-registry and merge/scope probes",
            "P1 D03 join and D07 cut probes",
        ] {
            assert!(
                steps[0].detail.contains(needle),
                "{op} {needle}: {}",
                steps[0].detail
            );
        }
    }
}

#[test]
fn combine_new_body_is_named_and_never_join() {
    let c = r#"{ "id": 1, "name": "C", "suppressed": false, "type": "combine", "operation": "new_body" }"#;
    let steps = plan_of(&phase_design(c));
    assert!(matches!(steps[0].work, Work::Unmappable));
    assert!(
        steps[0].detail.contains("new_body is structurally valid but its combine semantics are unsupported - never interpreted as join"),
        "{}",
        steps[0].detail
    );
    // keep_tools=true carries its named refusal inside the gate
    let c = r#"{ "id": 1, "name": "C", "suppressed": false, "type": "combine", "operation": "cut", "keep_tools": true }"#;
    let steps = plan_of(&phase_design(c));
    assert!(
        steps[0].detail.contains("COMBINE_KEEP_TOOLS_UNSUPPORTED"),
        "{}",
        steps[0].detail
    );
}

#[test]
fn phase_b_defaulted_fields_are_never_malformed() {
    // v0.43.0 defaults: edge_refs/edge_names empty, keep_tools false,
    // copy false, copy_frames empty, rotation fields optional
    for feature in [
        r#"{ "id": 1, "name": "F", "suppressed": false, "type": "fillet" }"#,
        r#"{ "id": 1, "name": "C", "suppressed": false, "type": "combine", "operation": "join" }"#,
        r#"{ "id": 1, "name": "M", "suppressed": false, "type": "move", "bodies": ["b1"] }"#,
        r#"{ "id": 1, "name": "R", "suppressed": false, "type": "remove" }"#,
    ] {
        let steps = plan_of(&phase_design(feature));
        assert!(
            matches!(steps[0].work, Work::Unmappable),
            "{feature}: {:?}",
            steps[0]
        );
        // the defaulted fillet still reports its zero counts
        if feature.contains("fillet") {
            assert!(
                steps[0].detail.contains("0 edge pick(s), 0 edge_ref(s)"),
                "{}",
                steps[0].detail
            );
        }
    }
}

#[test]
fn phase_b_malformed_records_refuse_as_invalid_recipe() {
    for (feature, needle) in [
        (
            r#"{ "id": 1, "name": "F", "suppressed": false, "type": "fillet", "edges": 3 }"#,
            "edges is not an array",
        ),
        (
            r#"{ "id": 1, "name": "F", "suppressed": false, "type": "fillet", "edge_refs": [3] }"#,
            "edge_refs[0] is not an object",
        ),
        (
            r#"{ "id": 1, "name": "F", "suppressed": false, "type": "fillet", "edge_names": ["e1", 2] }"#,
            "edge_names[1] is not a string",
        ),
        (
            r#"{ "id": 1, "name": "C", "suppressed": false, "type": "combine" }"#,
            "has no operation",
        ),
        (
            r#"{ "id": 1, "name": "C", "suppressed": false, "type": "combine", "operation": "explode" }"#,
            "outside the known set (new_body, join, cut, intersect)",
        ),
        (
            r#"{ "id": 1, "name": "C", "suppressed": false, "type": "combine", "operation": "join", "keep_tools": 3 }"#,
            "keep_tools is not a boolean",
        ),
        (
            r#"{ "id": 1, "name": "C", "suppressed": false, "type": "combine", "operation": "join", "target": 7 }"#,
            "target is not a body name or name list",
        ),
        (
            r#"{ "id": 1, "name": "M", "suppressed": false, "type": "move", "bodies": "b1" }"#,
            "bodies is not an array",
        ),
        (
            r#"{ "id": 1, "name": "M", "suppressed": false, "type": "move", "translate": ["1 mm", "2 mm"] }"#,
            "translate is not a 3-component expression vector",
        ),
        (
            r#"{ "id": 1, "name": "M", "suppressed": false, "type": "move", "copy": 2 }"#,
            "copy is not a boolean",
        ),
        (
            r#"{ "id": 1, "name": "M", "suppressed": false, "type": "move", "copy_frames": 1 }"#,
            "copy_frames is not an array",
        ),
        (
            r#"{ "id": 1, "name": "M", "suppressed": false, "type": "move", "rotate_axis": [1, 0] }"#,
            "rotate_axis is not a 3-component vector",
        ),
        (
            r#"{ "id": 1, "name": "M", "suppressed": false, "type": "move", "angle": false }"#,
            "angle is not an expression",
        ),
        (
            r#"{ "id": 1, "name": "R", "suppressed": false, "type": "remove", "bodies": 2 }"#,
            "bodies is not an array",
        ),
    ] {
        let msg = plan_err(&phase_design(feature));
        assert!(msg.contains(needle), "{feature}: {msg}");
    }
}

#[test]
fn phase_b_stops_are_snapshot_survivable_and_emit_nothing() {
    use craft_bridge::profile::SnapshotScope;
    let fillet =
        r#"{ "id": 1, "name": "F", "suppressed": false, "type": "fillet", "edges": [[0,0,0]] }"#;
    let design = phase_design(fillet);
    // ordinary planning: the stop cascades downstream exactly like the
    // conic gate (the shared machinery is untouched)
    let steps = plan_of(&design);
    assert!(matches!(steps[0].work, Work::Unmappable));
    assert_eq!(steps[1].status, PlanStatus::NotExecuted);
    // a boundary snapshot at the stopped step lets planning continue:
    // an INDEPENDENT downstream step plans normally (dependent steps go
    // to their fallback builder - the conic test's case)
    let limits = Limits::default();
    let value = recipe::validate(design.as_bytes(), &limits).unwrap();
    let mut snaps = BTreeMap::new();
    snaps.insert(0usize, SnapshotScope::Boundary);
    let steps = replay::plan(&value, &limits, &snaps).unwrap();
    assert!(matches!(steps[0].work, Work::Unmappable));
    assert_eq!(steps[1].status, PlanStatus::Mapped, "{:?}", steps[1]);
    // only Boundary unblocks: a final-scope snapshot keeps the stop
    // cascade (replay termination is the build layer's final-snapshot
    // machinery - coarse_final_snapshots_terminate_replay)
    let mut snaps = BTreeMap::new();
    snaps.insert(0usize, SnapshotScope::Final);
    let steps = replay::plan(&value, &limits, &snaps).unwrap();
    assert!(matches!(steps[0].work, Work::Unmappable));
    assert_eq!(steps[1].status, PlanStatus::NotExecuted, "{:?}", steps[1]);
    // the built macro carries NO executable phase B feature call - the
    // release gate requires P0 AND P1 probes before any emission. The
    // stop text may name the members diagnostically; the assertions are
    // on executable call shapes.
    let dir = tempfile::tempdir().unwrap();
    let recipe_path = dir.path().join("design.json");
    let step_path = dir.path().join("part.step");
    let artifact = dir.path().join("design.craftx");
    let out: PathBuf = dir.path().join("design.swb");
    fs::write(&recipe_path, &design).unwrap();
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
    build::build(
        &BuildOptions {
            artifact: &artifact,
            out: &out,
            plan: None,
            plan_text: None,
            overwrite: true,
        },
        &Limits::default(),
    )
    .unwrap();
    let m = fs::read_to_string(&out).unwrap();
    for forbidden in [
        "FeatureManager.FeatureFillet",
        "CreateDefinition(swFmFillet",
        "CreateFeature(",
        "FeatureManager.InsertCombineFeature",
        "FeatureManager.InsertMoveCopyBody2",
        "FeatureManager.InsertDeleteBody2",
    ] {
        assert!(
            !m.contains(forbidden),
            "phase B emission leaked: {forbidden}"
        );
    }
}

#[test]
fn phase_b_generic_arms_keep_their_texts() {
    // the neighbouring kinds stay on their established stops
    let c = r#"{ "id": 1, "name": "H", "suppressed": false, "type": "chamfer" }"#;
    let steps = plan_of(&phase_design(c));
    assert!(
        steps[0].detail.contains("carries edge/face references"),
        "{}",
        steps[0].detail
    );
    let c = r#"{ "id": 1, "name": "T", "suppressed": false, "type": "body_transfer" }"#;
    let steps = plan_of(&phase_design(c));
    assert!(
        steps[0]
            .detail
            .contains("needs the source-body-to-runtime-body registry and verified transforms"),
        "{}",
        steps[0].detail
    );
}
