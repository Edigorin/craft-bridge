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
fn offset_and_construction_plane_sketches_stop_named() {
    let points = r#"{ "id": "origin", "pos": [0.0, 0.0], "fixed": true },
        { "id": "p1", "pos": [50.0, 0.0] },
        { "id": "p2", "pos": [12.0, 30.0] }"#;
    let offset =
        r#"{ "type": "offset", "base": { "type": "origin", "name": "XY" }, "distance": "W" }"#;
    let steps = plan_of(&sketch_design(7, offset, points));
    assert!(matches!(steps[1].work, Work::Unmappable));
    assert!(
        steps[1]
            .detail
            .contains("offset plane (offset from XY by W)"),
        "{}",
        steps[1].detail
    );
    assert!(steps[1].detail.contains("offset-plane probe"));
    let construction = r#"{ "type": "construction", "feature": 16 }"#;
    let steps = plan_of(&sketch_design(7, construction, points));
    assert!(matches!(steps[1].work, Work::Unmappable));
    assert!(
        steps[1].detail.contains("construction_plane feature 16"),
        "{}",
        steps[1].detail
    );
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
    assert!(err.contains("not a single-part document"), "{err}");
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
    // The fix on a real endpoint becomes a real sgFix pin relation.
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
