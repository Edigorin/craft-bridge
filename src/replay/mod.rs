//! The typed replay plan: a validated artifact's recipe, translated into the
//! steps a SolidWorks macro will perform.
//!
//! One allowlist lives here. Reports are rendered from this plan, so they
//! cannot diverge from what the macro does. The stop rule is the profile's
//! semantics: the first step without a translation stops model replay, that
//! step is planned as `fallback-planned` (fallback bodies land with the
//! hybrid-fallback ticket) and every later step is `not-executed`.
//!
//! The recipe shapes are pinned in docs/mapping.md from SolidCraft's own
//! serialization: constraints reference entities by array index into
//! `points`/`curves`, coordinates and constraint values are plain numbers in
//! millimetres and radians, and anything parametric is an expression string.
//!
//! References in this module:
//! - **point index** – into the recipe's `points` array;
//! - **curve index** – into the recipe's `curves` array (original order);
//! - **slot** – a selectable endpoint in the rebuild: line `i` has slots
//!   `2*i` (start) and `2*i + 1` (end); circle `j` has slot
//!   `2*lines_total + j` (centre). Recipe points shared between curves are one
//!   point, so every slot beyond the first at a point index gets a coincident
//!   relation to its representative.

use serde::Serialize;
use serde_json::Value;

pub mod expr;

use crate::error::{Error, Result};
use crate::limits::Limits;

/// The evaluated parameter environment: names to SI quantities, filled in
/// recipe order (the exporter emits dependency order).
#[derive(Debug, Default, Clone)]
pub struct ParamEnv {
    values: Vec<(String, expr::Quantity, bool)>,
}

impl ParamEnv {
    pub fn insert_global(&mut self, name: &str, quantity: expr::Quantity) {
        self.values.push((name.to_string(), quantity, true));
    }

    pub fn insert_model(&mut self, name: &str, quantity: expr::Quantity) {
        self.values.push((name.to_string(), quantity, false));
    }

    /// True when the name is a global (user) parameter: the only kind an
    /// equation may reference in the current subset.
    pub fn is_global(&self, name: &str) -> bool {
        self.values
            .iter()
            .any(|(n, _, global)| n == name && *global)
    }

    pub fn get(&self, name: &str) -> Option<expr::Quantity> {
        self.values
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, q, _)| *q)
    }

    fn lookup(&self) -> impl Fn(&str) -> Option<expr::Quantity> + '_ {
        move |name| self.get(name)
    }
}

/// The design length unit factor from `document.units` (metres per bare unit).
fn design_length_factor(unit: Option<&str>) -> f64 {
    match unit {
        Some("cm") => 0.01,
        Some("m") => 1.0,
        Some("in") => 0.0254,
        Some("ft") => 0.3048,
        _ => 0.001,
    }
}

/// What the plan says about one step before anything runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanStatus {
    Mapped,
    DegradePlanned,
    FallbackPlanned,
    NotExecuted,
}

/// One planned step: the recipe step in order, and what will happen to it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlannedStep {
    pub index: usize,
    /// Source-side identity (e.g. a solidcraft/57 feature id+name). Never
    /// used as a runtime index: the runtime index above is dense 0..n and
    /// bounds arrays and snapshot addressing.
    pub source_id: String,
    pub command: String,
    pub status: PlanStatus,
    pub detail: String,
    /// The typed work, for the emitter. Not part of the plan report.
    #[serde(skip)]
    pub work: Work,
}

/// The typed translation of one step.
#[derive(Debug, Clone, PartialEq)]
pub enum Work {
    Units {
        unit: String,
    },
    /// A parameter becomes a SolidWorks global-variable equation.
    UserParam {
        name: String,
        /// The expression as written (for the plan report).
        expression: String,
        /// The equation right-hand side in SolidWorks syntax.
        equation_rhs: String,
        /// The evaluated SI value the equation must produce (verification).
        expect_si: f64,
        /// The evaluated dimension (verification lives in evaluator space).
        dim: expr::Dim,
    },
    Sketch(SketchStep),
    OffsetPlane(PlaneStep),
    Extrude(ExtrudeStep),
    Revolve(RevolveStep),
    Primitive(PrimStep),
    Modifier(ModStep),
    Pattern(PatStep),
    /// Recorded but not executed and not model work (a text parameter, an
    /// expression outside the supported subset): never triggers the stop rule.
    Note,
    /// The stop boundary: no translation exists yet.
    Unmappable,
    /// After the stop boundary.
    AfterStop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginPlane {
    Xy,
    Xz,
    Yz,
}

/// Who owns the physical plane a sketch opens on. Mutually exclusive:
/// the source frame stays on `SketchStep.plane` (the base origin plane's
/// (u,v) semantics); this binding says who CREATES the plane.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaneBinding {
    /// A SolidWorks origin plane carries the sketch directly.
    Origin,
    /// The sketch step owns an inline offset plane: created with its step,
    /// its distance driven by this equation (a signed distance along the
    /// base normal; zero binds to the base plane instead).
    Offset {
        /// The distance equation (rhs/segs/slot/expect_si in metres,
        /// signed). Participates in registry sizing like any equation.
        distance: DimEquation,
    },
    /// An earlier plan step owns the plane (dense index of a
    /// `Work::OffsetPlane` step); the sketch only selects it. expect_si is
    /// the signed distance for frame validation (0 = the base plane).
    Constructed { step: usize, expect_si: f64 },
}

/// A construction-plane step: `construction_plane` with an offset plane.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaneStep {
    /// The stable feature name the emitter renames the plane to.
    pub name: String,
    /// The base origin plane (frame semantics and base normal).
    pub base: OriginPlane,
    /// The signed distance along the base normal. None = zero offset: the
    /// plane IS the base plane (no creation).
    pub distance: Option<DimEquation>,
}

/// Resolution context for `plane: {type: "construction", feature: N}`:
/// earlier successfully planned plane steps, and why the others are not
/// available (named refusals, never a guess).
pub struct PlaneResolver<'a> {
    /// Earlier planned planes: (source feature id, dense step index, base,
    /// signed distance).
    pub planned: &'a [(u64, usize, OriginPlane, f64)],
    /// Source ids whose plane step did not plan (suppressed, rolled back,
    /// stopped): named reasons, never a guess.
    pub unavailable: &'a [(u64, String)],
    /// Every construction_plane source id with its feature position (the
    /// forward-reference check).
    pub universe: &'a [(u64, usize)],
    /// The referencing feature's own position in the features array.
    pub position: usize,
}

/// Resolve a `plane: {type: "construction", feature: N}` reference to the
/// plane step that owns it. Only earlier, successfully planned plane
/// steps resolve; everything else refuses with its named reason.
pub fn resolve_construction_plane<'a>(
    resolver: &'a PlaneResolver<'a>,
    fid: u64,
) -> std::result::Result<(usize, OriginPlane, f64), String> {
    if let Some((step, base, expect)) = resolver
        .planned
        .iter()
        .find(|(id, _, _, _)| *id == fid)
        .map(|(_, step, base, expect)| (*step, *base, *expect))
    {
        return Ok((step, base, expect));
    }
    if let Some((_, reason)) = resolver.unavailable.iter().find(|(id, _)| *id == fid) {
        return Err(reason.clone());
    }
    if let Some((_, pos)) = resolver.universe.iter().find(|(id, _)| *id == fid) {
        return Err(format!(
            "the referenced construction_plane feature {fid} at feature position {pos} is not an earlier planned plane step (forward reference or unprocessed)"
        ));
    }
    Err(format!(
        "feature {fid} is not a construction_plane step in this design"
    ))
}

/// The signed offset distance of a plane: a `distance` expression along
/// the base normal. Zero binds to the base plane (defined behavior); a
/// negative value selects the unprobed side of the base plane and refuses
/// named until the flip probe pins it.
fn plane_distance_binding(
    plane_v: &Value,
    env: &ParamEnv,
    design_factor: f64,
    design_text: &'static str,
    model_slots: &mut [(String, usize)],
    next_slot: &mut usize,
) -> std::result::Result<PlaneBinding, Error> {
    let distance = plane_v
        .get("distance")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            Error::Unsupported("the offset plane has no distance expression".to_string())
        })?;
    let node = expr::parse(distance)
        .map_err(|e| Error::Unsupported(format!("the offset distance does not parse: {e}")))?;
    let ctx = expr::Context::for_unit("", design_factor, design_text);
    let q = expr::eval(&node, &ctx, &env.lookup())
        .map_err(|e| Error::Unsupported(format!("the offset distance does not evaluate: {e}")))?;
    if q.dim != expr::Dim::Length {
        return Err(Error::Unsupported(format!(
            "the offset distance is not a length (its dimension is {:?})",
            q.dim
        )));
    }
    if q.value.abs() < 1e-12 {
        // A zero offset IS the base plane: no creation, no equation.
        return Ok(PlaneBinding::Origin);
    }
    if q.value < 0.0 {
        return Err(Error::Unsupported(format!(
            "the offset distance {distance} evaluates to {} m: a negative offset selects the unprobed side of the base plane; refusing until the flip probe pins the side",
            q.value
        )));
    }
    let segs =
        expr::translate_with(&node, &ctx, &|n| resolve_name(n, env, model_slots)).map_err(|e| {
            Error::Unsupported(format!(
                "the offset distance is not expressible as an equation: {e}"
            ))
        })?;
    let rhs = expr::translate(&node, &ctx);
    let slot = *next_slot;
    *next_slot += 1;
    Ok(PlaneBinding::Offset {
        distance: DimEquation {
            which: 0,
            rhs,
            segs: coalesce(segs.into_iter().map(seg_from).collect()),
            expect_si: q.value,
            slot,
        },
    })
}

/// A sketch that can be replayed: lines and circles on an origin plane, only
/// the relation and dimension kinds listed in docs/mapping.md.
#[derive(Debug, Clone, PartialEq)]
pub struct SketchStep {
    pub name: String,
    pub plane: OriginPlane,
    /// Who owns the physical plane (origin plane, inline offset, or an
    /// earlier construction-plane step). Mutually exclusive with `plane`,
    /// which stays the SOURCE frame the coordinates live in.
    pub plane_binding: PlaneBinding,
    /// Millimetres, recipe order.
    pub points: Vec<[f64; 2]>,
    pub lines: Vec<LineT>,
    pub circles: Vec<CircleT>,
    pub relations: Vec<RelationT>,
    pub dimensions: Vec<DimensionT>,
    /// Dimension equations: (index into `dimensions`, equation). A dimension
    /// bound to a model parameter becomes an equation when the parameter's
    /// expression is expressible (global references only); otherwise the
    /// dimension keeps its solved value and the binding is a degrade trigger.
    pub dim_equations: Vec<(usize, DimEquation)>,
    /// Constraint kinds the tracer cannot replay: degrade triggers, reported
    /// in both reports and never silently dropped.
    pub dropped_constraints: Vec<String>,
    /// Coordinate-driver notes that are not degrade triggers (a zero
    /// coordinate cannot carry a zero-length dimension): reported, never
    /// silent, never a stop.
    pub driver_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineT {
    pub id: String,
    /// Point indices into `points`.
    pub a: usize,
    pub b: usize,
    pub construction: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CircleT {
    pub id: String,
    /// Centre point index into `points`.
    pub c: usize,
    /// Radius in millimetres.
    pub r: f64,
    pub construction: bool,
}

/// Relations reference endpoint slots (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationT {
    /// Pin one sketch point (sgFix). The slot is a point handle.
    Fix(usize),
    PointsCoincident(usize, usize),
    PointsHorizontal(usize, usize),
    PointsVertical(usize, usize),
    LineHorizontal(usize),
    LineVertical(usize),
    LinesParallel(usize, usize),
    LinesPerpendicular(usize, usize),
    LinesTangent(usize, usize),
    LinesEqual(usize, usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DimensionT {
    pub kind: DimKind,
    /// Line index, circle index, or endpoint slots depending on `kind`:
    /// Length/angle hold curve indices into `lines`; Radius/Diameter hold an
    /// index into `circles`; Distance* hold two slots.
    pub a: usize,
    pub b: Option<usize>,
    /// Millimetres for lengths, radians for angles (the solved value the
    /// recipe stores: ground truth for verification).
    pub value: f64,
    /// The model parameter this dimension is bound to (constraint `param`):
    /// it becomes a dimension equation instead of a fixed value.
    pub param: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimKind {
    Length,
    Distance,
    DistanceX,
    DistanceY,
    Radius,
    Diameter,
    Angle,
}

/// How far a blind extrude reaches.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtrudeKind {
    /// One side, blind depth in metres. `negative` = the material goes
    /// against the source normal (direction: negative, a negative distance,
    /// or both - a negative distance turns its own side over).
    Blind { depth_m: f64, negative: bool },
    /// Symmetric about the sketch plane; total thickness in metres (the
    /// legacy_per_side convention is folded in at parse time).
    Symmetric { total_m: f64 },
    /// Two blind sides, each in metres, with the side each END sits on:
    /// `first_negative` / `second_negative` are the signs of the two ends
    /// (e1 = direction-sign * distance, e2 = -direction-sign * distance2,
    /// source-pinned from SolidCraft's #2700 two-sided semantics). The body
    /// spans between the two ends; ends on the same side of the sketch plane
    /// are refused at replay (no supported call shape spans them).
    TwoSided {
        first_m: f64,
        second_m: f64,
        first_negative: bool,
        second_negative: bool,
    },
    /// Through everything in one direction (its saved distance only names
    /// the side; it carries no depth dimension and no equation).
    ThroughAll { negative: bool },
}

/// A single-direction blind extrude over the whole sketch.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtrudeStep {
    pub name: String,
    /// The plan index of the sketch step this feature extrudes.
    pub sketch_step: usize,
    pub kind: ExtrudeKind,
    /// Non-literal depth expressions become dimension equations; the
    /// equation right-hand side and the evaluated expectation travel here.
    pub equations: Vec<DimEquation>,
}

/// One piece of an equation right-hand side: literal syntax, or the runtime
/// name of a registered dimension (a model-parameter cross-reference).
#[derive(Debug, Clone, PartialEq)]
pub enum RhsSeg {
    Text(String),
    /// The registry slot of a dimension created elsewhere; rendered at
    /// runtime as the captured name. Empty after that step falls back.
    Slot(usize),
}

/// An equation that drives a feature dimension, captured at runtime by name.
#[derive(Debug, Clone, PartialEq)]
pub struct DimEquation {
    /// Which dimension of the feature: 1 for D1, 2 for D2.
    pub which: usize,
    /// The equation right-hand side in SolidWorks syntax (static text only).
    pub rhs: String,
    /// Segmented form when the expression references model parameters.
    pub segs: Vec<RhsSeg>,
    /// The evaluated SI value it must produce (verification).
    pub expect_si: f64,
    /// The registry slot where the macro records this dimension's runtime
    /// name after verification; later equations may then reference it.
    pub slot: usize,
}

/// A rectangular feature pattern or a mirror about an origin plane. Seeds
/// bind to verified native features (never fallback bodies).
#[derive(Debug, Clone, PartialEq)]
pub struct PatStep {
    pub name: String,
    pub kind: PatKind,
    pub equations: Vec<DimEquation>,
    pub degraded_variants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatKind {
    Rectangular {
        /// Plan indices of the seed features.
        seeds: Vec<usize>,
        /// Pattern direction in sketch space (unit vector).
        dir: [f64; 2],
        count: u32,
        spacing: f64,
        reversed: bool,
    },
    Mirror {
        seeds: Vec<usize>,
        /// The origin plane the mirror is about.
        plane: OriginPlane,
    },
}

/// A modification feature: fillet, chamfer, shell, draft or a simple hole.
#[derive(Debug, Clone, PartialEq)]
pub struct ModStep {
    pub name: String,
    pub kind: ModKind,
    pub equations: Vec<DimEquation>,
    /// Variant flags that could not be replayed (reported as degrade).
    pub degraded_variants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModKind {
    Fillet {
        /// Edge picks in model millimetres (recipe coordinates, identity map).
        edges: Vec<[f64; 3]>,
        radius: f64,
        tangent_chain: bool,
    },
    Chamfer {
        edges: Vec<[f64; 3]>,
        distance: f64,
        flip: bool,
        tangent_chain: bool,
    },
    Shell {
        faces: Vec<[f64; 3]>,
        thickness: f64,
        direction: String,
        tangent_chain: bool,
    },
    Draft {
        faces: Vec<[f64; 3]>,
        angle_rad: f64,
    },
    Hole {
        at: [f64; 3],
        diameter: f64,
        /// None = through all.
        depth: Option<f64>,
    },
}

/// Primitives replay as generated sketches plus features.
#[derive(Debug, Clone, PartialEq)]
pub enum PrimKind {
    Box,
    Cylinder,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimStep {
    pub name: String,
    pub kind: PrimKind,
    /// Placement in sketch millimetres (literal only in this phase).
    pub origin: [f64; 2],
    /// Sizes in millimetres, in feature order (length/width/height, or
    /// radius/height), each optionally an equation.
    pub sizes: Vec<PrimSize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimSize {
    pub value: f64,
    pub equation: Option<DimEquation>,
}

/// How far a revolve turns. A blind angle keeps its sign as `reverse` (a
/// negative angle turns its own sweep over); the two-sided angles keep
/// their SIGNS because the swept interval is [-second, +first] (SolidCraft
/// `revolve_extent_for_handle`): a non-positive angle is an interval offset
/// from the sketch plane and is refused at replay, never silently folded.
#[derive(Debug, Clone, PartialEq)]
pub enum RevolveKind {
    Blind { angle_rad: f64, reverse: bool },
    Symmetric { total_rad: f64 },
    TwoSided { first_rad: f64, second_rad: f64 },
}

/// The revolve axis, in sketch space.
#[derive(Debug, Clone, PartialEq)]
pub enum AxisSel {
    /// A line that already exists in the profile sketch (its line index).
    Line(usize),
    /// A helper construction line the macro draws through `origin` along
    /// `dir` (millimetres, unit vector), anchored with a fix relation.
    Emitted { origin: [f64; 2], dir: [f64; 2] },
}

/// A revolve over the whole profile sketch.
#[derive(Debug, Clone, PartialEq)]
pub struct RevolveStep {
    pub name: String,
    /// The plan index of the profile sketch step.
    pub sketch_step: usize,
    pub kind: RevolveKind,
    pub axis: AxisSel,
    pub equations: Vec<DimEquation>,
}

/// One design parameter as a global-variable equation. Used by both recipe
/// shapes: the script's `parameters.add` step and a solidcraft/57 `params`
/// entry. A text parameter or an expression outside the supported subset is
/// a recorded note, never a stop.
#[allow(clippy::too_many_arguments)]
fn plan_user_param(
    index: usize,
    source_id: String,
    command: String,
    name: &str,
    expression: &str,
    unit: &str,
    env: &mut ParamEnv,
    design_factor: f64,
    design_text: &'static str,
) -> PlannedStep {
    let name = name.to_string();
    let expression = expression.to_string();
    let unit = unit.to_string();
    let ctx = expr::Context::for_unit(&unit, design_factor, design_text);
    let evaluated = expr::parse(&expression).ok().and_then(|node| {
        expr::eval(&node, &ctx, &env.lookup())
            .ok()
            .map(|q| (expr::translate(&node, &ctx), q))
    });
    match evaluated {
        Some((equation_rhs, q)) if unit != "text" => {
            env.insert_global(&name, q);
            PlannedStep {
                index,
                source_id,
                command,
                status: PlanStatus::Mapped,
                detail: format!("equation \"{name}\" = {equation_rhs}"),
                work: Work::UserParam {
                    name,
                    expression,
                    equation_rhs,
                    expect_si: q.value,
                    dim: q.dim,
                },
            }
        }
        _ => PlannedStep {
            index,
            source_id,
            command,
            status: PlanStatus::NotExecuted,
            detail: "not an equation: text parameter or an expression outside the supported subset"
                .to_string(),
            work: Work::Note,
        },
    }
}

/// Build the plan from a validated recipe (the artifact's `recipe.json`).
pub fn plan(
    recipe: &Value,
    limits: &Limits,
    snapshots: &std::collections::BTreeMap<usize, crate::profile::SnapshotScope>,
) -> Result<Vec<PlannedStep>> {
    if matches!(
        crate::recipe::shape_of(recipe)?,
        crate::recipe::Shape::Design57
    ) {
        return plan57(recipe, limits, snapshots);
    }
    let commands = crate::recipe::commands_of(recipe)?;
    let mut steps = Vec::with_capacity(commands.len());
    let mut stopped = false;
    let mut env = ParamEnv::default();
    let mut design_factor = 0.001f64;
    let mut design_text: &'static str = "mm";
    // Recipe feature id -> plan index of its sketch step.
    let mut sketch_steps: Vec<(u64, usize)> = Vec::new();
    // Model parameter name -> registry slot of the dimension that carries it.
    let mut model_slots: Vec<(String, usize)> = Vec::new();
    let mut next_slot: usize = 0;
    for (index, entry) in commands.iter().enumerate() {
        let id = entry
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let params = entry.get("params");
        if stopped {
            steps.push(PlannedStep {
                index,
                source_id: String::new(),
                command: id,
                status: PlanStatus::NotExecuted,
                detail: "after the first unmapped step".to_string(),
                work: Work::AfterStop,
            });
            continue;
        }
        let planned = match id.as_str() {
            "document.units" => {
                let unit = params
                    .and_then(|p| p.get("units"))
                    .and_then(Value::as_str)
                    .unwrap_or("mm");
                design_factor = design_length_factor(Some(unit));
                design_text = match unit {
                    "cm" => "cm",
                    "m" => "m",
                    "in" => "in",
                    "ft" => "ft",
                    _ => "mm",
                };
                PlannedStep {
                    index,
                    source_id: String::new(),
                    command: id,
                    status: PlanStatus::NotExecuted,
                    detail: format!("units recorded ({unit}); values replay in millimetres"),
                    work: Work::Units {
                        unit: unit.to_string(),
                    },
                }
            }
            "parameters.add" => plan_user_param(
                index,
                String::new(),
                id,
                params
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("(unnamed)"),
                params
                    .and_then(|p| p.get("expression"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                params
                    .and_then(|p| p.get("unit"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                &mut env,
                design_factor,
                design_text,
            ),
            "timeline.add" => plan_feature(
                index,
                &id,
                params,
                limits,
                &mut env,
                design_factor,
                design_text,
                &steps,
                &sketch_steps,
                &mut model_slots,
                &mut next_slot,
                &PlaneResolver {
                    planned: &[],
                    unavailable: &[],
                    universe: &[],
                    position: 0,
                },
            )?,
            _ => PlannedStep {
                index,
                source_id: String::new(),
                command: id,
                status: PlanStatus::FallbackPlanned,
                detail: "no translation; a fallback body lands with the hybrid-fallback ticket"
                    .to_string(),
                work: Work::Unmappable,
            },
        };
        if matches!(
            planned.work,
            Work::Sketch(_)
                | Work::Extrude(_)
                | Work::Revolve(_)
                | Work::Primitive(_)
                | Work::Modifier(_)
        ) {
            if let Some(fid) = params
                .and_then(|p| p.get("feature"))
                .and_then(|f| f.get("id"))
                .and_then(Value::as_u64)
            {
                sketch_steps.push((fid, index));
            }
        }
        // The stop rule pauses model replay at the first unmappable step,
        // except when a boundary snapshot covers it: the fallback body keeps
        // replay going. A final (coarse) snapshot or nothing stops for good.
        let stop_here = matches!(planned.work, Work::Unmappable)
            && snapshots.get(&index) != Some(&crate::profile::SnapshotScope::Boundary);
        steps.push(planned);
        if stop_here {
            stopped = true;
        }
    }
    Ok(steps)
}

/// Plan a `solidcraft/57` design: parameter equations first (the features
/// reference them), then the features in file order. Dense runtime indices
/// run over that sequence; the source feature id travels in `source_id` and
/// never becomes an index (docs/replay-profile.md).
fn plan57(
    recipe: &Value,
    limits: &Limits,
    snapshots: &std::collections::BTreeMap<usize, crate::profile::SnapshotScope>,
) -> Result<Vec<PlannedStep>> {
    let mut steps = Vec::new();
    let root = recipe.as_object().expect("validated root");
    let unit = root.get("units").and_then(Value::as_str).unwrap_or("mm");
    let design_factor = design_length_factor(Some(unit));
    let design_text: &'static str = match unit {
        "cm" => "cm",
        "m" => "m",
        "in" => "in",
        "ft" => "ft",
        _ => "mm",
    };
    let mut env = ParamEnv::default();
    // Recipe feature id -> plan index of its sketch step (source ids, map
    // lookup: sparse and huge ids are safe).
    let mut sketch_steps: Vec<(u64, usize)> = Vec::new();
    let mut model_slots: Vec<(String, usize)> = Vec::new();
    let mut next_slot: usize = 0;
    let mut stopped = false;
    let mut index = 0usize;
    // Timeline rollback marker (source-pinned): features at this index and
    // after are rolled back - recorded, not executed, never a stop.
    let marker = root
        .get("marker")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX) as usize;
    // Construction-plane reference universe: every construction_plane
    // feature id with its position (the forward-reference check).
    let plane_universe: Vec<(u64, usize)> = crate::recipe::features_of(recipe)?
        .iter()
        .enumerate()
        .filter(|(_, f)| f.get("type").and_then(Value::as_str) == Some("construction_plane"))
        .map(|(pos, f)| (f.get("id").and_then(Value::as_u64).unwrap_or(0), pos))
        .collect();
    let mut planned_planes: Vec<(u64, usize, OriginPlane, f64)> = Vec::new();
    let mut plane_unavailable: Vec<(u64, String)> = Vec::new();
    for p in crate::recipe::params57_of(recipe)? {
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let expression = p
            .get("expr")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let unit = p
            .get("unit")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        steps.push(plan_user_param(
            index,
            format!("parameter {name}"),
            "parameter".to_string(),
            &name,
            &expression,
            &unit,
            &mut env,
            design_factor,
            design_text,
        ));
        index += 1;
    }
    for (position, f) in crate::recipe::features_of(recipe)?.iter().enumerate() {
        let kind = f.get("type").and_then(Value::as_str).unwrap_or("");
        let fid = f.get("id").and_then(Value::as_u64).unwrap_or(0);
        let planned = if position >= marker {
            PlannedStep {
                index,
                source_id: format!("feature {fid}"),
                command: kind.to_string(),
                status: PlanStatus::NotExecuted,
                detail: format!(
                    "{kind} is past the timeline rollback marker (feature index {marker}); the source evaluator has rolled it back and the evaluated model excludes it"
                ),
                work: Work::Note,
            }
        } else if stopped {
            PlannedStep {
                index,
                source_id: format!("feature {fid}"),
                command: kind.to_string(),
                status: PlanStatus::NotExecuted,
                detail: "after the first unmapped step".to_string(),
                work: Work::AfterStop,
            }
        } else {
            let resolver = PlaneResolver {
                planned: &planned_planes,
                unavailable: &plane_unavailable,
                universe: &plane_universe,
                position,
            };
            plan_feature57(
                index,
                f,
                limits,
                &mut env,
                design_factor,
                design_text,
                &steps,
                &sketch_steps,
                &mut model_slots,
                &mut next_slot,
                &resolver,
            )?
        };
        if matches!(
            planned.work,
            Work::Sketch(_)
                | Work::Extrude(_)
                | Work::Revolve(_)
                | Work::Primitive(_)
                | Work::Modifier(_)
        ) {
            sketch_steps.push((fid, index));
        }
        // Construction-plane steps: record what resolved so later sketches
        // can bind to exactly the planes that planned.
        if f.get("type").and_then(Value::as_str) == Some("construction_plane") {
            match &planned.work {
                Work::OffsetPlane(pl) => {
                    let expect = pl.distance.as_ref().map(|d| d.expect_si).unwrap_or(0.0);
                    planned_planes.push((fid, index, pl.base, expect));
                }
                _ => {
                    let reason = match planned.work {
                        Work::Note => planned.detail.clone(),
                        _ => planned.detail.clone(),
                    };
                    plane_unavailable.push((fid, reason));
                }
            }
        }
        let stop_here = matches!(planned.work, Work::Unmappable)
            && snapshots.get(&index) != Some(&crate::profile::SnapshotScope::Boundary);
        steps.push(planned);
        if stop_here {
            stopped = true;
        }
        index += 1;
    }
    Ok(steps)
}

/// One solidcraft/57 feature. The sketch/extrude/box/cylinder payloads are
/// shape-identical to the script's `feature` object, so those feed the
/// shared typed planning unchanged. Everything else stops NAMED with its
/// type and the rollout phase that will carry it - never a guess.
#[allow(clippy::too_many_arguments)]
fn plan_feature57(
    index: usize,
    feature: &Value,
    limits: &Limits,
    env: &mut ParamEnv,
    design_factor: f64,
    design_text: &'static str,
    steps: &[PlannedStep],
    sketch_steps: &[(u64, usize)],
    model_slots: &mut Vec<(String, usize)>,
    next_slot: &mut usize,
    plane_resolver: &PlaneResolver<'_>,
) -> Result<PlannedStep> {
    let kind = feature.get("type").and_then(Value::as_str).unwrap_or("");
    let fid = feature.get("id").and_then(Value::as_u64).unwrap_or(0);
    let name = feature.get("name").and_then(Value::as_str).unwrap_or("");
    let source_id = format!("feature {fid}");
    let unmappable = |detail: String| PlannedStep {
        index,
        source_id: source_id.clone(),
        command: kind.to_string(),
        status: PlanStatus::FallbackPlanned,
        detail,
        work: Work::Unmappable,
    };
    // Suppression is source-level: the timeline keeps the record and skips
    // the geometry. Replay skips it the same way and continues - a
    // suppressed feature never triggers the stop rule.
    if feature
        .get("suppressed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(PlannedStep {
            index,
            source_id,
            command: kind.to_string(),
            status: PlanStatus::NotExecuted,
            detail: format!(
                "{kind} {name:?} is suppressed in the source design; the source timeline keeps the record and skips the geometry"
            ),
            work: Work::Note,
        });
    }
    match kind {
        "sketch" | "extrude" | "box" | "cylinder" => {
            let mut wrapped = serde_json::Map::new();
            wrapped.insert("feature".to_string(), feature.clone());
            let mut planned = plan_feature(
                index,
                kind,
                Some(&Value::Object(wrapped)),
                limits,
                env,
                design_factor,
                design_text,
                steps,
                sketch_steps,
                model_slots,
                next_slot,
                plane_resolver,
            )?;
            planned.source_id = source_id;
            // Named-stop contract: the refusal keeps the feature's name as
            // well as its id (source_id) and type (command).
            if matches!(planned.work, Work::Unmappable | Work::AfterStop)
                && !name.is_empty()
                && !planned.detail.contains(name)
            {
                planned.detail = format!("{kind} {name:?}: {}", planned.detail);
            }
            Ok(planned)
        }
        "fillet" | "chamfer" | "shell" | "draft" | "hole" | "thread" => Ok(unmappable(format!(
            "{kind} {name:?} carries edge/face references; reference resolution is unpinned until the edge-resolver probe lands (Phase B)"
        ))),
        "move" | "remove" | "body_transfer" | "align" | "scale" => Ok(unmappable(format!(
            "{kind} {name:?} needs the source-body-to-runtime-body registry and verified transforms (Phase B)"
        ))),
        "combine" => Ok(unmappable(format!(
            "combine {name:?} needs the source-body-to-runtime-body registry and the merge/scope probe (Phase B)"
        ))),
        "construction_plane" => {
            // The replayed variant is an offset plane from an origin base
            // (its creation and equation chain are seat-probe-backed).
            // Every other plane construction stops named with its form.
            let plane_v = match feature.get("plane") {
                Some(v) => v,
                None => {
                    return Ok(unmappable(format!(
                        "construction_plane {name:?} has no plane definition"
                    )))
                }
            };
            if plane_v.get("type").and_then(Value::as_str) != Some("offset") {
                let form = plane_v.get("type").and_then(Value::as_str).unwrap_or("?");
                return Ok(unmappable(format!(
                    "construction_plane {name:?} is a {form:?} plane; only origin-based offset planes replay (Phase C for the rest)"
                )));
            }
            let base = match (
                plane_v.get("base").and_then(|b| b.get("type")).and_then(Value::as_str),
                plane_v.get("base").and_then(|b| b.get("name")).and_then(Value::as_str),
            ) {
                (Some("origin"), Some("XY")) => OriginPlane::Xy,
                (Some("origin"), Some("XZ")) => OriginPlane::Xz,
                (Some("origin"), Some("YZ")) => OriginPlane::Yz,
                (bt, bn) => {
                    return Ok(unmappable(format!(
                        "construction_plane {name:?} is offset from {bt:?}/{bn:?}: only origin-plane bases replay"
                    )))
                }
            };
            match plane_distance_binding(plane_v, env, design_factor, design_text, model_slots, next_slot) {
                Ok(PlaneBinding::Offset { distance }) => Ok(PlannedStep {
                    index,
                    source_id,
                    command: kind.to_string(),
                    status: PlanStatus::Mapped,
                    detail: format!(
                        "construction_plane {name:?}: offset plane from {base:?}, the distance driven by its equation"
                    ),
                    work: Work::OffsetPlane(PlaneStep {
                        name: format!("CB_Plane_{index}"),
                        base,
                        distance: Some(distance),
                    }),
                }),
                Ok(_) => Ok(PlannedStep {
                    index,
                    source_id,
                    command: kind.to_string(),
                    status: PlanStatus::Mapped,
                    detail: format!(
                        "construction_plane {name:?}: a zero offset - the plane is the {base:?} base plane itself"
                    ),
                    work: Work::OffsetPlane(PlaneStep {
                        name: format!("CB_Plane_{index}"),
                        base,
                        distance: None,
                    }),
                }),
                Err(Error::Unsupported(reason)) => {
                    Ok(unmappable(format!("construction_plane {name:?}: {reason}")))
                }
                Err(e) => Err(e),
            }
        }
        "construction_axis" | "construction_point" | "sweep" | "loft"
        | "torus" | "sphere" | "cone" | "coil" | "pipe" | "pattern" | "mirror" => {
            Ok(unmappable(format!(
                "{kind} {name:?} is a feature class without a replay mapping yet (Phase C)"
            )))
        }
        "revolve" => Ok(unmappable(format!(
            "revolve {name:?}: the solidcraft/57 axis reference shape is unpinned (Phase C)"
        ))),
        other => Ok(unmappable(format!(
            "solidcraft/57 feature type {other:?} ({name:?}) has no replay mapping"
        ))),
    }
}

#[allow(clippy::too_many_arguments)] // the plan context is five call-scoped borrows
fn plan_feature(
    index: usize,
    id: &str,
    params: Option<&Value>,
    limits: &Limits,
    env: &mut ParamEnv,
    design_factor: f64,
    design_text: &'static str,
    steps: &[PlannedStep],
    sketch_steps: &[(u64, usize)],
    model_slots: &mut Vec<(String, usize)>,
    next_slot: &mut usize,
    plane_resolver: &PlaneResolver<'_>,
) -> Result<PlannedStep> {
    let feature = params
        .and_then(|p| p.get("feature"))
        .ok_or_else(|| Error::InvalidRecipe("timeline.add has no feature".to_string()))?;
    let kind = feature.get("type").and_then(Value::as_str).unwrap_or("");
    let name = feature
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let unmappable = |detail: String| PlannedStep {
        index,
        source_id: String::new(),
        command: id.to_string(),
        status: PlanStatus::FallbackPlanned,
        detail,
        work: Work::Unmappable,
    };
    // Model parameters (feature inputs, in the step's params array) evaluate
    // in array order and enter the environment as model entries.
    let mut model_exprs: Vec<(String, String, String)> = Vec::new();
    if let Some(items) = params
        .and_then(|p| p.get("params"))
        .and_then(Value::as_array)
    {
        for item in items {
            let n = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let e = item
                .get("expression")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let u = item
                .get("unit")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if n.is_empty() {
                continue;
            }
            let ctx = expr::Context::for_unit(&u, design_factor, design_text);
            let evaluated = expr::parse(&e)
                .ok()
                .and_then(|node| expr::eval(&node, &ctx, &env.lookup()).ok());
            if let Some(q) = evaluated {
                env.insert_model(&n, q);
            }
            model_exprs.push((n, e, u));
        }
    }
    Ok(match kind {
        "sketch" => match parse_sketch(name, feature, limits, &model_exprs, env, design_factor, design_text, model_slots, next_slot, plane_resolver) {
            Ok((sketch, degrade)) => {
                let dropped = sketch.dropped_constraints.len();
                let detail = if degrade {
                    format!(
                        "sketch on {:?}: {} constraint kind(s) not replayed (degrade)",
                        sketch.plane, dropped
                    )
                } else {
                    format!(
                        "sketch on {:?}: {} lines, {} circles, {} relations, {} dimensions",
                        sketch.plane,
                        sketch.lines.len(),
                        sketch.circles.len(),
                        sketch.relations.len(),
                        sketch.dimensions.len()
                    )
                };
                PlannedStep {
                    index,
                    source_id: String::new(),
                    command: id.to_string(),
                    status: if degrade {
                        PlanStatus::DegradePlanned
                    } else {
                        PlanStatus::Mapped
                    },
                    detail,
                    work: Work::Sketch(sketch),
                }
            }
            Err(Error::Unsupported(reason)) => unmappable(reason),
            Err(e) => return Err(e),
        },
        "extrude" => {
            let sketch_step = match resolve_sketch(feature, sketch_steps, steps) {
                Some(i) => i,
                None => return Ok(unmappable("the extrude's sketch is not in this recipe".to_string())),
            };
            match parse_extrude(name, sketch_step, feature, env, design_factor, design_text, model_slots, next_slot) {
            Ok(extrude) => {
                let detail = match &extrude.kind {
                    ExtrudeKind::Blind { depth_m, negative } => format!(
                        "extrude {} mm{} over the whole sketch",
                        depth_m * 1000.0,
                        if *negative { " (negative)" } else { "" }
                    ),
                    ExtrudeKind::Symmetric { total_m } => {
                        format!("symmetric extrude {} mm total", total_m * 1000.0)
                    }
                    ExtrudeKind::TwoSided {
                        first_m,
                        second_m,
                        ..
                    } => format!(
                        "two-sided extrude {} + {} mm",
                        first_m * 1000.0,
                        second_m * 1000.0
                    ),
                    ExtrudeKind::ThroughAll { negative } => format!(
                        "extrude through all{}",
                        if *negative { " (negative)" } else { "" }
                    ),
                };
                PlannedStep {
                    index,
                    source_id: String::new(),
                    command: id.to_string(),
                    status: PlanStatus::Mapped,
                    detail,
                    work: Work::Extrude(extrude),
                }
            }
            Err(reason) => unmappable(format!(
                "{reason}; a fallback body lands with the hybrid-fallback ticket"
            )),
            }
        }
        "revolve" => {
            let sketch_step = match resolve_sketch(feature, sketch_steps, steps) {
                Some(i) => i,
                None => return Ok(unmappable("the revolve's sketch is not in this recipe".to_string())),
            };
            let sketch = steps.iter().find_map(|s| match &s.work {
                Work::Sketch(sk) if s.index == sketch_step => Some(sk),
                _ => None,
            });
            let sketch = match sketch {
                Some(sk) => sk,
                None => return Ok(unmappable("the revolve's sketch did not replay".to_string())),
            };
            match parse_revolve(name, sketch_step, feature, sketch, env, design_factor, design_text, model_slots, next_slot)
            {
                Ok(revolve) => {
                    let detail = match &revolve.kind {
                        RevolveKind::Blind { angle_rad, .. } => {
                            format!("revolve {} deg", angle_rad.to_degrees())
                        }
                        RevolveKind::Symmetric { total_rad } => {
                            format!("symmetric revolve {} deg total", total_rad.to_degrees())
                        }
                        RevolveKind::TwoSided { first_rad, second_rad } => format!(
                            "two-sided revolve {} + {} deg",
                            first_rad.to_degrees(),
                            second_rad.to_degrees()
                        ),
                    };
                    let axis = match &revolve.axis {
                        AxisSel::Line(_) => "about its sketch line",
                        AxisSel::Emitted { .. } => "about a helper axis",
                    };
                    PlannedStep {
                        index,
                        source_id: String::new(),
                        command: id.to_string(),
                        status: PlanStatus::Mapped,
                        detail: format!("{detail} {axis}"),
                        work: Work::Revolve(revolve),
                    }
                }
                Err(reason) => unmappable(format!(
                    "{reason}; a fallback body lands with the hybrid-fallback ticket"
                )),
            }
        }
        "pattern" | "mirror" => {
            match parse_pattern(
                kind,
                name,
                feature,
                env,
                design_factor,
                design_text,
                steps,
                sketch_steps,
                model_slots,
                next_slot,
            ) {
                Ok(pattern) => {
                    let degrade = !pattern.degraded_variants.is_empty();
                    PlannedStep {
                        index,
                        source_id: String::new(),
                        command: id.to_string(),
                        status: if degrade {
                            PlanStatus::DegradePlanned
                        } else {
                            PlanStatus::Mapped
                        },
                        detail: format!("{kind} replays natively over its seed features"),
                        work: Work::Pattern(pattern),
                    }
                }
                Err(reason) => unmappable(format!(
                    "{reason}; a fallback body lands with the hybrid-fallback ticket"
                )),
            }
        }
        "fillet" | "chamfer" | "shell" | "draft" | "hole" => {
            match parse_mod(kind, name, feature, env, design_factor, design_text, model_slots, next_slot)
            {
                Ok(modifier) => {
                    let degrade = !modifier.degraded_variants.is_empty();
                    PlannedStep {
                        index,
                        source_id: String::new(),
                        command: id.to_string(),
                        status: if degrade {
                            PlanStatus::DegradePlanned
                        } else {
                            PlanStatus::Mapped
                        },
                        detail: format!(
                            "{kind} replays natively{}",
                            if degrade {
                                format!(
                                    " ({} variant(s) not replayed)",
                                    modifier.degraded_variants.len()
                                )
                            } else {
                                String::new()
                            }
                        ),
                        work: Work::Modifier(modifier),
                    }
                }
                Err(reason) => unmappable(format!(
                    "{reason}; a fallback body lands with the hybrid-fallback ticket"
                )),
            }
        }
        "box" | "cylinder" => {
            let prim_kind = if kind == "box" {
                PrimKind::Box
            } else {
                PrimKind::Cylinder
            };
            match parse_prim(name, prim_kind, feature, env, design_factor, design_text, model_slots, next_slot)
            {
                Ok(primitive) => {
                    let what = match primitive.kind {
                        PrimKind::Box => "box",
                        PrimKind::Cylinder => "cylinder",
                    };
                    PlannedStep {
                        index,
                        source_id: String::new(),
                        command: id.to_string(),
                        status: PlanStatus::Mapped,
                        detail: format!(
                        "{what} replayed as a mapped generated sketch and a blind extrude through the direction truth table"
                    ),
                        work: Work::Primitive(primitive),
                    }
                }
                Err(reason) => unmappable(format!(
                    "{reason}; a fallback body lands with the hybrid-fallback ticket"
                )),
            }
        }
        _ => unmappable(format!(
            "feature type {kind:?} has no translation yet; a fallback body lands with the hybrid-fallback ticket"
        )),
    })
}

/// Parse a sketch. Malformed recipe data is InvalidRecipe; content outside
/// the tracer subset is Unsupported (the stop boundary) or a degrade trigger
/// (kept on the sketch and reported).
#[allow(clippy::too_many_arguments)]
fn parse_sketch(
    name: String,
    feature: &Value,
    limits: &Limits,
    model_exprs: &[(String, String, String)],
    env: &ParamEnv,
    design_factor: f64,
    design_text: &'static str,
    model_slots: &mut Vec<(String, usize)>,
    next_slot: &mut usize,
    plane_resolver: &PlaneResolver<'_>,
) -> Result<(SketchStep, bool)> {
    let plane_v = feature
        .get("plane")
        .ok_or_else(|| Error::InvalidRecipe("sketch has no plane".to_string()))?;
    let (plane, plane_binding) = match (
        plane_v.get("type").and_then(Value::as_str),
        plane_v.get("name").and_then(Value::as_str),
    ) {
        (Some("origin"), Some("XY")) => (OriginPlane::Xy, PlaneBinding::Origin),
        (Some("origin"), Some("XZ")) => (OriginPlane::Xz, PlaneBinding::Origin),
        (Some("origin"), Some("YZ")) => (OriginPlane::Yz, PlaneBinding::Origin),
        (Some("offset"), _) => {
            // Signed distance along the base normal (source semantics:
            // "offset by signed expression distance along base normal").
            let base = match (
                plane_v.get("base").and_then(|b| b.get("type")).and_then(Value::as_str),
                plane_v.get("base").and_then(|b| b.get("name")).and_then(Value::as_str),
            ) {
                (Some("origin"), Some("XY")) => OriginPlane::Xy,
                (Some("origin"), Some("XZ")) => OriginPlane::Xz,
                (Some("origin"), Some("YZ")) => OriginPlane::Yz,
                (bt, bn) => {
                    return Err(Error::Unsupported(format!(
                        "sketch plane is offset from {bt:?}/{bn:?}: only origin-plane bases are replayed"
                    )))
                }
            };
            let binding = plane_distance_binding(
                plane_v,
                env,
                design_factor,
                design_text,
                model_slots,
                next_slot,
            )?;
            (base, binding)
        }
        (Some("construction"), _) => {
            let fid = plane_v.get("feature").and_then(Value::as_u64).unwrap_or(0);
            match resolve_construction_plane(plane_resolver, fid) {
                Ok((step, base, expect_si)) => {
                    if expect_si.abs() < 1e-12 {
                        (base, PlaneBinding::Origin)
                    } else {
                        (base, PlaneBinding::Constructed { step, expect_si })
                    }
                }
                Err(reason) => {
                    return Err(Error::Unsupported(format!(
                        "sketch plane references construction_plane feature {fid}: {reason}"
                    )))
                }
            }
        }
        (Some(other), _) => {
            return Err(Error::Unsupported(format!(
                "sketch plane type {other:?} is outside the replayed subset (origin planes)"
            )))
        }
        (None, _) => {
            return Err(Error::Unsupported(
                "sketch plane has no plane type".to_string(),
            ))
        }
    };
    let sketch = feature
        .get("sketch")
        .ok_or_else(|| Error::InvalidRecipe("sketch feature has no sketch".to_string()))?;
    let points_v = sketch
        .get("points")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::InvalidRecipe("sketch has no points".to_string()))?;
    if points_v.len() > limits.max_entities {
        return Err(Error::limit(
            "sketch points",
            limits.max_entities as u64,
            points_v.len() as u64,
        ));
    }
    let mut points = Vec::with_capacity(points_v.len());
    // solidcraft/57 coordinates may carry per-axis driver expressions.
    let mut point_exprs: Vec<[Option<String>; 2]> = Vec::with_capacity(points_v.len());
    let mut point_fixed: Vec<bool> = Vec::with_capacity(points_v.len());
    for (i, p) in points_v.iter().enumerate() {
        let mut exprs: [Option<String>; 2] = [None, None];
        match p.get("exprs") {
            None | Some(Value::Null) => {}
            Some(Value::Object(map)) => {
                for (axis, key) in ["x", "y"].iter().enumerate() {
                    match map.get(*key) {
                        None | Some(Value::Null) => {}
                        Some(Value::String(t)) => exprs[axis] = Some(t.clone()),
                        Some(_) => {
                            return Err(Error::InvalidRecipe(format!(
                                "point {i} {key} expression is not a string"
                            )))
                        }
                    }
                }
            }
            Some(_) => {
                return Err(Error::InvalidRecipe(format!(
                    "point {i} exprs are not an object"
                )))
            }
        }
        point_exprs.push(exprs);
        point_fixed.push(p.get("fixed").and_then(Value::as_bool).unwrap_or(false));
        let pos = p
            .get("pos")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::InvalidRecipe(format!("point {i} has no pos")))?;
        let x = pos
            .first()
            .and_then(Value::as_f64)
            .ok_or_else(|| Error::InvalidRecipe(format!("point {i} x is not a number")))?;
        let y = pos
            .get(1)
            .and_then(Value::as_f64)
            .ok_or_else(|| Error::InvalidRecipe(format!("point {i} y is not a number")))?;
        if !x.is_finite() || !y.is_finite() {
            return Err(Error::InvalidRecipe(format!(
                "point {i} coordinates are not finite"
            )));
        }
        points.push([x, y]);
    }

    let empty = Vec::new();
    let curves_v = sketch
        .get("curves")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    if curves_v.len() > limits.max_entities {
        return Err(Error::limit(
            "sketch curves",
            limits.max_entities as u64,
            curves_v.len() as u64,
        ));
    }
    // curve_map: recipe curve index -> (is_line, index into lines/circles).
    let mut curve_map: Vec<(bool, usize)> = Vec::with_capacity(curves_v.len());
    let mut lines = Vec::new();
    let mut circles = Vec::new();
    for (i, c) in curves_v.iter().enumerate() {
        let id = c
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        // Construction, centerline and infinite curves all sit outside
        // profile detection (the source profile detector excludes them),
        // so they replay as construction geometry.
        let construction = ["construction", "centerline", "infinite"]
            .iter()
            .any(|k| c.get(*k).and_then(Value::as_bool).unwrap_or(false));
        let point_index = |key: &str| -> Result<usize> {
            let v = c
                .get(key)
                .and_then(Value::as_u64)
                .ok_or_else(|| Error::InvalidRecipe(format!("curve {i} has no {key}")))?;
            if v as usize >= points.len() {
                return Err(Error::InvalidRecipe(format!(
                    "curve {i} references point {v} which does not exist"
                )));
            }
            Ok(v as usize)
        };
        match c.get("type").and_then(Value::as_str) {
            Some("line") => {
                let a = point_index("a")?;
                let b = point_index("b")?;
                curve_map.push((true, lines.len()));
                lines.push(LineT {
                    id,
                    a,
                    b,
                    construction,
                });
            }
            Some("circle") => {
                let cidx = point_index("c")?;
                let r = c
                    .get("r")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| Error::InvalidRecipe(format!("circle {i} has no radius")))?;
                if !r.is_finite() || r <= 0.0 {
                    return Err(Error::InvalidRecipe(format!(
                        "circle {i} radius is not a positive number"
                    )));
                }
                curve_map.push((false, circles.len()));
                circles.push(CircleT {
                    id,
                    c: cidx,
                    r,
                    construction,
                });
            }
            _ => {
                return Err(Error::Unsupported(format!(
                    "sketch curve {i} is outside the tracer subset (line, circle)"
                )))
            }
        }
    }

    // Endpoint slots and identity coincidences for shared recipe points.
    let slot_point = |slot: usize| -> usize {
        if (slot / 2) < lines.len() {
            if slot.is_multiple_of(2) {
                lines[slot / 2].a
            } else {
                lines[slot / 2].b
            }
        } else {
            circles[slot - 2 * lines.len()].c
        }
    };
    let slot_count = 2 * lines.len() + circles.len();
    let mut rep: Vec<Option<usize>> = vec![None; points.len()];
    let mut relations = Vec::new();
    for slot in 0..slot_count {
        let p = slot_point(slot);
        match rep[p] {
            Some(first) => relations.push(RelationT::PointsCoincident(first, slot)),
            None => rep[p] = Some(slot),
        }
    }

    let empty_c = Vec::new();
    let constraints_v = sketch
        .get("constraints")
        .and_then(Value::as_array)
        .unwrap_or(&empty_c);
    if constraints_v.len() > limits.max_constraints {
        return Err(Error::limit(
            "sketch constraints",
            limits.max_constraints as u64,
            constraints_v.len() as u64,
        ));
    }
    let mut dimensions = Vec::new();
    let mut dropped_constraints: Vec<String> = Vec::new();
    let mut fix_constrained: Vec<bool> = vec![false; points.len()];
    // Text links generate outline geometry (nested-schema research T4):
    // the outlines replay as frozen curves, but the text parametrics that
    // would regenerate them are not replayed - named, never silent.
    if let Some(Value::Array(links)) = sketch.get("links") {
        if !links.is_empty() {
            dropped_constraints.push(format!(
                "sketch carries {} text/generated link(s): outline geometry replays frozen, its parametric regeneration does not",
                links.len()
            ));
        }
    }
    for (i, k) in constraints_v.iter().enumerate() {
        let ctype = k
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let driven = k.get("driven").and_then(Value::as_bool).unwrap_or(false);
        let number = |key: &str| -> Result<usize> {
            k.get(key)
                .and_then(Value::as_u64)
                .map(|v| v as usize)
                .ok_or_else(|| Error::InvalidRecipe(format!("constraint {i} has no {key}")))
        };
        let point_slot = |key: &str| -> Result<usize> {
            let p = number(key)?;
            if p >= points.len() {
                return Err(Error::InvalidRecipe(format!(
                    "constraint {i} references point {p} which does not exist"
                )));
            }
            rep[p].ok_or_else(|| {
                // Valid source data (the sketch origin is a legitimate
                // reference) with no selectable entity in the replayed
                // subset: degrade the constraint named, never refuse the
                // build.
                Error::Unsupported(format!(
                    "constraint {i} ({ctype}) references point {p}, which no curve endpoint occupies (no entity to constrain)"
                ))
            })
        };
        let line_index = |key: &str| -> Result<usize> {
            let c = number(key)?;
            match curve_map.get(c) {
                Some((true, li)) => Ok(*li),
                Some(_) => Err(Error::Unsupported(format!(
                    "constraint {i} ({ctype}) refers to curve {c}, which is not a line"
                ))),
                None => Err(Error::InvalidRecipe(format!(
                    "constraint {i} references curve {c} which does not exist"
                ))),
            }
        };
        let circle_index = |key: &str| -> Result<usize> {
            let c = number(key)?;
            match curve_map.get(c) {
                Some((false, ci)) => Ok(*ci),
                Some(_) => Err(Error::Unsupported(format!(
                    "constraint {i} ({ctype}) refers to curve {c}, which is not a circle"
                ))),
                None => Err(Error::InvalidRecipe(format!(
                    "constraint {i} references curve {c} which does not exist"
                ))),
            }
        };
        let value = || -> Result<f64> {
            let v = k
                .get("value")
                .and_then(Value::as_f64)
                .ok_or_else(|| Error::InvalidRecipe(format!("constraint {i} has no value")))?;
            if !v.is_finite() {
                return Err(Error::InvalidRecipe(format!(
                    "constraint {i} value is not finite"
                )));
            }
            Ok(v)
        };
        let result: Result<Option<()>> = (|| {
            if driven {
                dropped_constraints.push(format!("{ctype} (driven)"));
                return Ok(Some(()));
            }
            match ctype.as_str() {
                "fix" => {
                    // A point pin: becomes a real sgFix relation so the
                    // SolidWorks solver holds the point like the source
                    // solver does (it is also the natural driver anchor).
                    // The sketch origin is fixed by construction; a free
                    // non-origin point has no entity to pin and degrades.
                    let p = number("p")?;
                    if p >= points.len() {
                        return Err(Error::InvalidRecipe(format!(
                            "constraint {i} references point {p} which does not exist"
                        )));
                    }
                    match rep[p] {
                        Some(slot) => {
                            fix_constrained[p] = true;
                            relations.push(RelationT::Fix(slot));
                        }
                        None if p == 0 => {}
                        None => dropped_constraints.push(format!(
                            "fix (point {p}): no curve endpoint occupies it, there is no entity to pin"
                        )),
                    }
                }
                "coincident" => relations.push(RelationT::PointsCoincident(
                    point_slot("p")?,
                    point_slot("q")?,
                )),
                "horizontal_points" => relations.push(RelationT::PointsHorizontal(
                    point_slot("p")?,
                    point_slot("q")?,
                )),
                "vertical_points" => relations.push(RelationT::PointsVertical(
                    point_slot("p")?,
                    point_slot("q")?,
                )),
                "horizontal" => relations.push(RelationT::LineHorizontal(line_index("l")?)),
                "vertical" => relations.push(RelationT::LineVertical(line_index("l")?)),
                "parallel" => {
                    relations.push(RelationT::LinesParallel(line_index("a")?, line_index("b")?))
                }
                "perpendicular" => relations.push(RelationT::LinesPerpendicular(
                    line_index("a")?,
                    line_index("b")?,
                )),
                "tangent" => {
                    relations.push(RelationT::LinesTangent(line_index("a")?, line_index("b")?))
                }
                "equal" => {
                    relations.push(RelationT::LinesEqual(line_index("a")?, line_index("b")?))
                }
                "length" => dimensions.push(DimensionT {
                    kind: DimKind::Length,
                    a: line_index("l")?,
                    b: None,
                    value: value()?,
                    param: k.get("param").and_then(Value::as_str).map(str::to_string),
                }),
                "distance" | "distance_x" | "distance_y" => {
                    let kind = match ctype.as_str() {
                        "distance" => DimKind::Distance,
                        "distance_x" => DimKind::DistanceX,
                        _ => DimKind::DistanceY,
                    };
                    dimensions.push(DimensionT {
                        kind,
                        a: point_slot("p")?,
                        b: Some(point_slot("q")?),
                        value: value()?,
                        param: k.get("param").and_then(Value::as_str).map(str::to_string),
                    });
                }
                "radius" | "diameter" => {
                    let kind = if ctype == "radius" {
                        DimKind::Radius
                    } else {
                        DimKind::Diameter
                    };
                    dimensions.push(DimensionT {
                        kind,
                        a: circle_index("c")?,
                        b: None,
                        value: value()?,
                        param: k.get("param").and_then(Value::as_str).map(str::to_string),
                    });
                }
                "angle" => {
                    if k.get("flip").and_then(Value::as_bool).unwrap_or(false) {
                        // flip selects the supplementary branch; replaying
                        // the primary branch would be silently wrong.
                        dropped_constraints.push(
                            "angle (flip: the supplementary branch is not replayed)".to_string(),
                        );
                    } else {
                        dimensions.push(DimensionT {
                            kind: DimKind::Angle,
                            a: line_index("a")?,
                            b: Some(line_index("b")?),
                            value: value()?,
                            param: k.get("param").and_then(Value::as_str).map(str::to_string),
                        });
                    }
                }
                other => dropped_constraints.push(other.to_string()),
            }
            Ok(Some(()))
        })();
        match result {
            Ok(_) => {}
            Err(Error::Unsupported(reason)) => dropped_constraints.push(reason),
            Err(e) => return Err(e),
        }
    }

    let mut sketch = SketchStep {
        name,
        plane,
        plane_binding,
        points,
        lines,
        circles,
        relations,
        dimensions,
        dim_equations: Vec::new(),
        dropped_constraints,
        driver_notes: Vec::new(),
    };
    // Bind dimensions to their model parameter's expression, as equations
    // that reference globals only; anything else loses the equation (reported
    // as a degrade trigger) and keeps the solved value.
    for (di, d) in sketch.dimensions.iter().enumerate() {
        let param = match &d.param {
            Some(p) => p.clone(),
            None => continue,
        };
        let found = model_exprs.iter().find(|(n, _, _)| *n == param);
        let bound = found.and_then(|(_, expression, unit)| {
            let node = expr::parse(expression).ok()?;
            let ctx = expr::Context::for_unit(unit, design_factor, design_text);
            expr::eval(&node, &ctx, &env.lookup()).ok()?;
            let segs =
                expr::translate_with(&node, &ctx, &|n| resolve_name(n, env, model_slots)).ok()?;
            let rhs = expr::translate(&node, &ctx);
            let slot = *next_slot;
            *next_slot += 1;
            model_slots.push((param.clone(), slot));
            Some(DimEquation {
                which: 0,
                rhs,
                segs: coalesce(segs.into_iter().map(seg_from).collect()),
                expect_si: match d.kind {
                    DimKind::Angle => d.value,
                    _ => d.value / 1000.0,
                },
                slot,
            })
        });
        match bound {
            Some(equation) => sketch.dim_equations.push((di, equation)),
            None => sketch
                .dropped_constraints
                .push(format!("{param} (equation not expressible)")),
        }
    }
    // Coordinate drivers (solidcraft/57): a point coordinate that carries
    // an expression becomes a NATIVE driver - a DistanceX/DistanceY
    // dimension to the anchored sketch origin, bound by an equation to the
    // expression, so parameter edits carry the sketch geometry. Precedence:
    // the expression drives even when the point is `fixed` (the driving
    // dimension both pins and parametrizes it); `fixed` without an
    // expression stays placement. The expression is the source of truth
    // (the engine fixes expression points at their evaluated positions
    // before solve); pos is a cache. Only lengths drive coordinates. The
    // anchor is chosen AFTER evaluation and must be nonmoving: a curve
    // endpoint at the sketch origin that no parameter edit can move, or
    // the drivers degrade named. A zero coordinate cannot carry a zero-
    // length dimension: position-verified and recorded as a driver note.
    if point_exprs
        .iter()
        .any(|pe| pe[0].is_some() || pe[1].is_some())
    {
        let ctx = expr::Context::for_unit("", design_factor, design_text);
        // Phase 1: evaluate every expressed coordinate. expr-wins: the
        // evaluated value replaces the stored position.
        let mut evals: Vec<[Option<(expr::Node, expr::Quantity)>; 2]> =
            vec![[None, None]; sketch.points.len()];
        for (pi, pe) in point_exprs.iter().enumerate() {
            for axis in 0..2 {
                let text = match &pe[axis] {
                    Some(t) => t.clone(),
                    None => continue,
                };
                let axis_name = if axis == 0 { "x" } else { "y" };
                let node = match expr::parse(&text) {
                    Ok(n) => n,
                    Err(e) => {
                        sketch.dropped_constraints.push(format!(
                            "point {pi} {axis_name} expression not replayable as a driver ({e})"
                        ));
                        continue;
                    }
                };
                let q = match expr::eval(&node, &ctx, &env.lookup()) {
                    Ok(q) => q,
                    Err(e) => {
                        sketch.dropped_constraints.push(format!(
                            "point {pi} {axis_name} expression not replayable as a driver ({e})"
                        ));
                        continue;
                    }
                };
                if q.dim != expr::Dim::Length {
                    // An angle (or count) must never become a coordinate.
                    sketch.dropped_constraints.push(format!(
                        "point {pi} {axis_name} expression is not a length (its dimension is {:?}); the coordinate binding is refused",
                        q.dim
                    ));
                    continue;
                }
                // The expression wins over the stored position; a
                // disagreement is recorded, never silent.
                let eval_mm = q.value * 1000.0;
                let stored_mm = sketch.points[pi][axis];
                if (eval_mm - stored_mm).abs() > 1e-3 {
                    sketch.driver_notes.push(format!(
                        "point {pi} {axis_name}: pos {stored_mm} mm disagrees with the expression ({eval_mm} mm); the expression wins (source-engine rule)"
                    ));
                }
                sketch.points[pi][axis] = eval_mm;
                evals[pi][axis] = Some((node, q));
            }
        }
        // Phase 2: the anchor. A curve endpoint sitting at the sketch
        // origin AFTER evaluation, nonmoving under parameter edits: either
        // it carries no expressions at all, or every expressed coordinate
        // is constant zero (checked by re-evaluating with every parameter
        // doubled - a parametric value would move).
        let doubled = |name: &str| {
            env.get(name).map(|q| expr::Quantity {
                value: q.value * 2.0,
                dim: q.dim,
            })
        };
        let at_origin = |pi: usize| sketch.points[pi][0] == 0.0 && sketch.points[pi][1] == 0.0;
        let nonmoving = |pi: usize| {
            [0usize, 1].iter().all(|&axis| match &evals[pi][axis] {
                None => true,
                Some((node, _)) => match expr::eval(node, &ctx, &doubled) {
                    Ok(q) => q.value.abs() < 1e-9,
                    Err(_) => false,
                },
            })
        };
        let pinned = |pi: usize| (point_fixed[pi] || fix_constrained[pi]) && nonmoving(pi);
        let base_point = (0..sketch.points.len())
            .filter(|&pi| rep[pi].is_some() && pinned(pi))
            .min_by_key(|&pi| {
                (
                    !at_origin(pi),
                    evals[pi].iter().filter(|e| e.is_some()).count(),
                )
            });
        // The anchor becomes a real sgFix pin (or already carries one from
        // its source fix constraint), so the SolidWorks solver holds it
        // where the source solver holds it.
        if let Some(bp) = base_point {
            if !fix_constrained[bp] {
                if let Some(slot) = rep[bp] {
                    sketch.relations.push(RelationT::Fix(slot));
                    fix_constrained[bp] = true;
                }
            }
        }
        // Phase 3: the drivers.
        for (pi, pe) in point_exprs.iter().enumerate() {
            for axis in 0..2 {
                if pe[axis].is_none() {
                    continue;
                }
                let axis_name = if axis == 0 { "x" } else { "y" };
                let (node, _) = match &evals[pi][axis] {
                    Some(e) => e,
                    None => continue, // phase 1 recorded the degrade already
                };
                let base_axis = match base_point {
                    Some(bp) => sketch.points[bp][axis],
                    None => 0.0,
                };
                let delta_mm = sketch.points[pi][axis] - base_axis;
                if delta_mm.abs() < 1e-9 {
                    sketch.driver_notes.push(format!(
                        "point {pi} {axis_name}: zero delta from the anchor carries no zero-length dimension (position verified)"
                    ));
                    continue;
                }
                let target_slot = match rep[pi] {
                    Some(slot) => slot,
                    None => {
                        sketch.dropped_constraints.push(format!(
                            "point {pi} {axis_name}: not a curve endpoint, the coordinate binding cannot become a dimension"
                        ));
                        continue;
                    }
                };
                let base_slot = match base_point.and_then(|bp| rep[bp]) {
                    Some(slot) => slot,
                    None => {
                        sketch.dropped_constraints.push(format!(
                            "point {pi} {axis_name}: no pinned (fixed or fix-constrained) curve endpoint anchors the sketch, the coordinate binding cannot become a dimension"
                        ));
                        continue;
                    }
                };
                if Some(pi) == base_point {
                    sketch.driver_notes.push(format!(
                        "point {pi} {axis_name}: the anchor point carries no self-dimension"
                    ));
                    continue;
                }
                let segs = match expr::translate_with(node, &ctx, &|n| {
                    resolve_name(n, env, model_slots)
                }) {
                    Ok(segs) => segs,
                    Err(e) => {
                        sketch.dropped_constraints.push(format!(
                            "point {pi} {axis_name} (equation not expressible: {e})"
                        ));
                        continue;
                    }
                };
                // The dimension measures the delta from the anchor, so the
                // equation is the expression minus the anchor's fixed
                // offset (both the report text and the emitted segments).
                let mut mapped: Vec<RhsSeg> = Vec::new();
                let mut rhs = expr::translate(node, &ctx);
                if base_axis != 0.0 {
                    let correction = if base_axis > 0.0 {
                        format!(" - {base_axis}mm")
                    } else {
                        format!(" + {}mm", -base_axis)
                    };
                    rhs = format!("({rhs}){correction}");
                    mapped.push(RhsSeg::Text("(".to_string()));
                    mapped.extend(segs.into_iter().map(seg_from));
                    mapped.push(RhsSeg::Text(")".to_string()));
                    mapped.push(RhsSeg::Text(correction));
                } else {
                    mapped.extend(segs.into_iter().map(seg_from));
                }
                let di = sketch.dimensions.len();
                sketch.dimensions.push(DimensionT {
                    kind: if axis == 0 {
                        DimKind::DistanceX
                    } else {
                        DimKind::DistanceY
                    },
                    a: base_slot,
                    b: Some(target_slot),
                    value: delta_mm,
                    param: None,
                });
                let slot = *next_slot;
                *next_slot += 1;
                sketch.dim_equations.push((
                    di,
                    DimEquation {
                        which: 0,
                        rhs,
                        segs: coalesce(mapped),
                        expect_si: delta_mm / 1000.0,
                        slot,
                    },
                ));
            }
        }
    }
    let degrade = !sketch.dropped_constraints.is_empty();
    Ok((sketch, degrade))
}

/// Blind, symmetric, two-sided or through-all extrude over the whole sketch.
/// Extent semantics follow the recipe's convention: `total_distance` is the
/// finished thickness, `legacy_per_side` doubles symmetric distances. A
/// second side with `second_follows_first` is the legacy signed rule and is
/// outside this subset. Expressions evaluate through the shared engine;
/// non-literal expressions travel as dimension equations.
#[allow(clippy::too_many_arguments)]
fn parse_extrude(
    name: String,
    sketch_step: usize,
    feature: &Value,
    env: &ParamEnv,
    design_factor: f64,
    design_text: &'static str,
    model_slots: &mut Vec<(String, usize)>,
    next_slot: &mut usize,
) -> std::result::Result<ExtrudeStep, String> {
    // The emitted FeatureExtrusion2 merge/scope flags are hardcoded and
    // only proven for new_body (the whole corpus). join/subtract/intersect
    // carry target-body semantics that need the Phase B body registry and
    // the merge/scope probe - refusing beats operating on whichever bodies
    // SolidWorks happens to select.
    let operation = match feature.get("operation") {
        None | Some(Value::Null) => "new_body",
        Some(Value::String(s)) => s.as_str(),
        Some(_) => return Err("extrude operation is present but not a string".to_string()),
    };
    match operation {
        "new_body" => {}
        other => {
            return Err(format!(
                "extrude operation {other:?} is outside the supported subset; targeted merge and feature scope await the Phase B body registry and the merge/scope probe (the emitted flags are proven for new_body only)"
            ))
        }
    }
    let extent = feature
        .get("extent")
        .ok_or("extrude has no extent".to_string())?;
    for forbidden in [
        "start_offset",
        "taper",
        "to",
        "from",
        "to_offset",
        "corner",
        "to_body_apart",
    ] {
        if extent.get(forbidden).is_some() {
            return Err(format!(
                "extrude extent has {forbidden}, outside the supported subset"
            ));
        }
    }
    if feature.get("thin").is_some() {
        return Err("thin extrude is outside the supported subset".to_string());
    }
    match feature
        .get("profiles")
        .and_then(|p| p.get("type"))
        .and_then(Value::as_str)
    {
        Some("all") => {}
        _ => {
            return Err(
                "extrude profiles other than \"all\" are outside the supported subset".to_string(),
            )
        }
    }
    match feature
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("new_body")
    {
        "new_body" | "join" => {}
        other => {
            return Err(format!(
                "extrude operation {other:?} is outside the supported subset"
            ))
        }
    }
    if extent
        .get("second_follows_first")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(
            "two-sided extrude with second_follows_first is the legacy signed rule".to_string(),
        );
    }
    let total_distance = match extent
        .get("convention")
        .and_then(Value::as_str)
        .unwrap_or("legacy_per_side")
    {
        "total_distance" => true,
        "legacy_per_side" => false,
        other => return Err(format!("extrude convention {other:?} is unknown")),
    };
    let direction = extent
        .get("direction")
        .and_then(Value::as_str)
        .unwrap_or("positive");
    let through_all = extent
        .get("through_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // Evaluate one expression: literal numbers read as millimetres, bare
    // numbers inside non-literal expressions read in the design unit. The
    // result carries the evaluated value and whether the expression was a
    // literal (legacy symmetric scaling refuses non-literals).
    let mut equations = Vec::new();
    let mut eval_len = |text: &str, which: usize| -> std::result::Result<(f64, bool), String> {
        let node = expr::parse(text).map_err(|e| format!("extrude distance: {e}"))?;
        let literal = matches!(node, expr::Node::Number { .. });
        let ctx = expr::Context {
            bare_length: if literal { 0.001 } else { design_factor },
            bare_angle: 1.0,
            bare_dim: expr::Dim::Length,
            bare_text: design_text,
        };
        let q =
            expr::eval(&node, &ctx, &env.lookup()).map_err(|e| format!("extrude distance: {e}"))?;
        if q.dim != expr::Dim::Length {
            return Err("extrude distance is not a length".to_string());
        }
        if !literal {
            let segs = expr::translate_with(&node, &ctx, &|n| resolve_name(n, env, model_slots))
                .map_err(|e| format!("extrude distance: {e}"))?;
            let slot = *next_slot;
            *next_slot += 1;
            equations.push(DimEquation {
                which,
                rhs: expr::translate(&node, &ctx),
                segs: coalesce(segs.into_iter().map(seg_from).collect()),
                expect_si: q.value,
                slot,
            });
        }
        Ok((q.value, literal))
    };

    if through_all {
        // Two Sides with a through-all first side is a different call shape
        // than the probed one: never silently dropped (it used to be).
        if extent.get("distance2").is_some() {
            return Err(
                "two-sided through-all (through_all with distance2) is outside the supported subset"
                    .to_string(),
            );
        }
        let dir_sign = match direction {
            "positive" => 1.0,
            "negative" => -1.0,
            other => {
                return Err(format!(
                    "extrude direction {other:?} with through-all is outside the supported subset"
                ))
            }
        };
        // The saved distance only names the SIDE (#10415: through all keeps
        // the side a saved negative distance points to). Evaluate it for its
        // sign only - through-all carries no depth dimension, so no depth
        // equation and no live slot may be allocated for it.
        let side = match extent.get("distance").and_then(Value::as_str) {
            None => 1.0,
            Some(text) => {
                let node = expr::parse(text).map_err(|e| format!("extrude distance: {e}"))?;
                let literal = matches!(node, expr::Node::Number { .. });
                let ctx = expr::Context {
                    bare_length: if literal { 0.001 } else { design_factor },
                    bare_angle: 1.0,
                    bare_dim: expr::Dim::Length,
                    bare_text: design_text,
                };
                let q = expr::eval(&node, &ctx, &env.lookup())
                    .map_err(|e| format!("extrude distance: {e}"))?;
                if q.dim != expr::Dim::Length {
                    return Err("extrude distance is not a length".to_string());
                }
                if q.value < 0.0 {
                    -1.0
                } else {
                    1.0
                }
            }
        };
        return Ok(ExtrudeStep {
            name,
            sketch_step,
            kind: ExtrudeKind::ThroughAll {
                negative: dir_sign * side < 0.0,
            },
            equations: Vec::new(),
        });
    }
    let distance = extent
        .get("distance")
        .and_then(Value::as_str)
        .ok_or("extrude distance is not an expression string".to_string())?;
    let (d1, d1_literal) = eval_len(distance, 1)?;

    let kind = match (direction, extent.get("distance2").and_then(Value::as_str)) {
        ("symmetric", None) => {
            if !total_distance && !d1_literal {
                // legacy_per_side doubles the thickness while the equation
                // would drive the doubled dimension from the unscaled
                // expression (and downstream references carry no scale):
                // refused, never silently misbuilt.
                return Err(
                    "a legacy_per_side symmetric extrude with a non-literal distance is outside the supported subset (its doubled dimension cannot track the unscaled expression)"
                        .to_string(),
                );
            }
            let total = if total_distance {
                d1.abs()
            } else {
                2.0 * d1.abs()
            };
            ExtrudeKind::Symmetric { total_m: total }
        }
        (_, Some(_)) if direction == "symmetric" => {
            return Err("symmetric extrude with a second side is contradictory".to_string())
        }
        ("positive" | "negative", d2s) => {
            // The first side's end is e1 = dir_sign * distance and the
            // second side's end is e2 = -dir_sign * distance2 (#2700); a
            // negative distance turns its own side over.
            let dir_sign = if direction == "positive" { 1.0 } else { -1.0 };
            let first_negative = dir_sign * d1 < 0.0;
            match d2s {
                None => ExtrudeKind::Blind {
                    depth_m: d1.abs(),
                    negative: first_negative,
                },
                Some(text2) => {
                    let (d2, _) = eval_len(text2, 2)?;
                    ExtrudeKind::TwoSided {
                        first_m: d1.abs(),
                        second_m: d2.abs(),
                        first_negative,
                        second_negative: -dir_sign * d2 < 0.0,
                    }
                }
            }
        }
        (other, _) => return Err(format!("extrude direction {other:?} is unknown")),
    };
    if let Some(names) = feature.get("param_names").and_then(Value::as_array) {
        for eq in &equations {
            if let Some(n) = names.get(eq.which - 1).and_then(Value::as_str) {
                model_slots.push((n.to_string(), eq.slot));
            }
        }
    }
    Ok(ExtrudeStep {
        name,
        sketch_step,
        kind,
        equations,
    })
}

/// The plan index of the sketch a feature profiles over, resolved by the
/// recipe's feature id (never "the most recent sketch").
fn resolve_sketch(
    feature: &Value,
    sketch_steps: &[(u64, usize)],
    steps: &[PlannedStep],
) -> Option<usize> {
    let sid = feature.get("sketch").and_then(Value::as_u64)?;
    let index = sketch_steps
        .iter()
        .find(|(f, _)| *f == sid)
        .map(|(_, i)| *i)?;
    steps
        .iter()
        .find(|s| s.index == index)
        .filter(|s| matches!(s.work, Work::Sketch(_)))
        .map(|s| s.index)
}

/// A revolve over the whole profile sketch. Axis forms: a line of the profile
/// sketch, the sketch axes, world axes that lie in the sketch plane, or an
/// explicit in-plane line (replayed as an anchored helper construction line).
/// Model and construction axes stay outside the subset. Partial angles
/// require the sketch-normal convention; a full turn is convention-free.
#[allow(clippy::too_many_arguments)]
fn parse_revolve(
    name: String,
    sketch_step: usize,
    feature: &Value,
    sketch: &SketchStep,
    env: &ParamEnv,
    design_factor: f64,
    design_text: &'static str,
    model_slots: &mut Vec<(String, usize)>,
    next_slot: &mut usize,
) -> std::result::Result<RevolveStep, String> {
    if let Some(v) = feature.get("to") {
        let _ = v;
        return Err("revolve to-object is outside the supported subset".to_string());
    }
    match feature
        .get("profiles")
        .and_then(|p| p.get("type"))
        .and_then(Value::as_str)
    {
        Some("all") => {}
        _ => {
            return Err(
                "revolve profiles other than \"all\" are outside the supported subset".to_string(),
            )
        }
    }
    match feature
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("new_body")
    {
        "new_body" | "join" => {}
        other => {
            return Err(format!(
                "revolve operation {other:?} is outside the supported subset"
            ))
        }
    }

    // Axis.
    let axis_v = feature
        .get("axis")
        .ok_or("revolve has no axis".to_string())?;
    let axis_kind = axis_v.get("type").and_then(Value::as_str).unwrap_or("");
    let axis = match axis_kind {
        "sketch_line" => {
            if axis_v
                .get("sketch")
                .and_then(Value::as_u64)
                .map(|s| s != feature.get("id").and_then(Value::as_u64).unwrap_or(0))
                .unwrap_or(false)
            {
                return Err("the revolve axis lives in another sketch".to_string());
            }
            let curve = axis_v
                .get("curve")
                .and_then(Value::as_str)
                .ok_or("the revolve axis names no curve".to_string())?;
            let li = sketch
                .lines
                .iter()
                .position(|l| l.id == curve)
                .ok_or_else(|| format!("the revolve axis {curve} is not a line of the sketch"))?;
            AxisSel::Line(li)
        }
        "sketch_axis" => match axis_v.get("axis").and_then(Value::as_str) {
            Some("x") => AxisSel::Emitted {
                origin: [0.0, 0.0],
                dir: [1.0, 0.0],
            },
            Some("y") => AxisSel::Emitted {
                origin: [0.0, 0.0],
                dir: [0.0, 1.0],
            },
            _ => return Err("the sketch axis is unknown".to_string()),
        },
        "world" => {
            // World axes mapped into the sketch frame (docs/mapping.md):
            // XY: u=X, v=Y; XZ: u=X, v=-Z; YZ: u=Y, v=Z (SolidCraft plane.rs:29-34).
            let dir = match (sketch.plane, axis_v.get("axis").and_then(Value::as_str)) {
                (OriginPlane::Xy, Some("x")) => [1.0, 0.0],
                (OriginPlane::Xy, Some("y")) => [0.0, 1.0],
                (OriginPlane::Xz, Some("x")) => [1.0, 0.0],
                (OriginPlane::Xz, Some("z")) => [0.0, -1.0],
                (OriginPlane::Yz, Some("y")) => [1.0, 0.0],
                (OriginPlane::Yz, Some("z")) => [0.0, 1.0],
                _ => return Err("the world axis does not lie in the sketch plane".to_string()),
            };
            AxisSel::Emitted {
                origin: [0.0, 0.0],
                dir,
            }
        }
        "line" => {
            let origin_v = axis_v
                .get("origin")
                .and_then(Value::as_array)
                .ok_or("the axis line has no origin".to_string())?;
            if origin_v.len() != 3 {
                return Err("the axis line origin is not three expressions".to_string());
            }
            let mut xyz = [0.0f64; 3];
            for (i, e) in origin_v.iter().enumerate() {
                let text = e
                    .as_str()
                    .ok_or("the axis origin is not an expression string".to_string())?;
                let node = expr::parse(text).map_err(|e| format!("the axis origin: {e}"))?;
                if !matches!(node, expr::Node::Number { .. }) {
                    return Err("the axis origin is not a literal in this phase".to_string());
                }
                let ctx = expr::Context {
                    bare_length: 0.001,
                    bare_angle: 1.0,
                    bare_dim: expr::Dim::Length,
                    bare_text: design_text,
                };
                let q = expr::eval(&node, &ctx, &env.lookup())
                    .map_err(|e| format!("the axis origin: {e}"))?;
                xyz[i] = q.value;
            }
            let dir_v = axis_v
                .get("dir")
                .and_then(Value::as_array)
                .ok_or("the axis line has no direction".to_string())?;
            let mut d = [0.0f64; 3];
            for (i, e) in dir_v.iter().enumerate().take(3) {
                d[i] = e.as_f64().unwrap_or(0.0);
            }
            // The axis must lie in the sketch plane; its sketch-space frame
            // follows the same world-axis mapping as above. The evaluated
            // origin is SI; AxisSel::Emitted carries millimetres.
            let (origin, dir) = match sketch.plane {
                OriginPlane::Xy => {
                    if d[2].abs() > 1e-9 || xyz[2].abs() > 1e-6 {
                        return Err("the axis line is not in the XY sketch plane".to_string());
                    }
                    ([xyz[0], xyz[1]], [d[0], d[1]])
                }
                OriginPlane::Xz => {
                    if d[1].abs() > 1e-9 || xyz[1].abs() > 1e-6 {
                        return Err("the axis line is not in the XZ sketch plane".to_string());
                    }
                    ([xyz[0], -xyz[2]], [d[0], -d[2]])
                }
                OriginPlane::Yz => {
                    if d[0].abs() > 1e-9 || xyz[0].abs() > 1e-6 {
                        return Err("the axis line is not in the YZ sketch plane".to_string());
                    }
                    ([xyz[1], xyz[2]], [d[1], d[2]])
                }
            };
            let norm = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
            if !norm.is_finite() || norm < 1e-12 {
                return Err("the axis line direction is zero".to_string());
            }
            AxisSel::Emitted {
                origin: [origin[0] * 1000.0, origin[1] * 1000.0],
                dir: [dir[0] / norm, dir[1] / norm],
            }
        }
        other => {
            return Err(format!(
                "revolve axis {other:?} is outside the supported subset"
            ))
        }
    };

    // Angle.
    let mut equations = Vec::new();
    let mut eval_angle = |text: &str,
                          which: usize,
                          equations: &mut Vec<DimEquation>|
     -> std::result::Result<f64, String> {
        let node = expr::parse(text).map_err(|e| format!("revolve angle: {e}"))?;
        let literal = matches!(node, expr::Node::Number { .. });
        let ctx = expr::Context {
            bare_length: design_factor,
            bare_angle: 1.0,
            bare_dim: expr::Dim::Angle,
            bare_text: design_text,
        };
        let q =
            expr::eval(&node, &ctx, &env.lookup()).map_err(|e| format!("revolve angle: {e}"))?;
        if q.dim != expr::Dim::Angle {
            return Err("revolve angle is not an angle".to_string());
        }
        if !literal {
            let segs = expr::translate_with(&node, &ctx, &|n| resolve_name(n, env, model_slots))
                .map_err(|e| format!("revolve angle: {e}"))?;
            let slot = *next_slot;
            *next_slot += 1;
            equations.push(DimEquation {
                which,
                rhs: expr::translate(&node, &ctx),
                segs: coalesce(segs.into_iter().map(seg_from).collect()),
                expect_si: q.value,
                slot,
            });
        }
        Ok(q.value)
    };
    let angle_text = feature
        .get("angle")
        .and_then(Value::as_str)
        .ok_or("revolve angle is not an expression string".to_string())?;
    let angle = eval_angle(angle_text, 1, &mut equations)?;
    let symmetric = feature
        .get("symmetric")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let angle2 = feature
        .get("angle2")
        .and_then(Value::as_str)
        .map(|t| eval_angle(t, 2, &mut equations))
        .transpose()?;

    let full = (angle - 2.0 * std::f64::consts::PI).abs() < 1e-9 && angle2.is_none() && !symmetric;
    if !full {
        match feature
            .get("convention")
            .and_then(Value::as_str)
            .unwrap_or("legacy_axis_start")
        {
            "sketch_normal" => {}
            other => {
                return Err(format!(
                "a partial revolve under convention {other:?} needs the sketch-normal convention"
            ))
            }
        }
    }

    let kind = match (symmetric, angle2) {
        (true, None) => RevolveKind::Symmetric {
            total_rad: angle.abs(),
        },
        (false, Some(a2)) => RevolveKind::TwoSided {
            first_rad: angle,
            second_rad: a2,
        },
        (true, Some(_)) => {
            return Err("symmetric revolve with a second angle is contradictory".to_string())
        }
        (false, None) => RevolveKind::Blind {
            angle_rad: angle.abs(),
            reverse: angle < 0.0,
        },
    };
    if let Some(names) = feature.get("param_names").and_then(Value::as_array) {
        for eq in &equations {
            if let Some(n) = names.get(eq.which - 1).and_then(Value::as_str) {
                model_slots.push((n.to_string(), eq.slot));
            }
        }
    }
    Ok(RevolveStep {
        name,
        sketch_step,
        kind,
        axis,
        equations,
    })
}

impl SketchStep {
    /// The point index behind an endpoint slot.
    pub fn slot_point(&self, slot: usize) -> usize {
        if (slot / 2) < self.lines.len() {
            if slot.is_multiple_of(2) {
                self.lines[slot / 2].a
            } else {
                self.lines[slot / 2].b
            }
        } else {
            self.circles[slot - 2 * self.lines.len()].c
        }
    }

    /// Millimetre coordinates of an endpoint slot.
    pub fn slot_coord(&self, slot: usize) -> [f64; 2] {
        self.points[self.slot_point(slot)]
    }

    pub fn slot_count(&self) -> usize {
        2 * self.lines.len() + self.circles.len()
    }
}

/// Resolve an expression name for translation: globals become quoted text,
/// model parameters become registry slots, anything else is unresolvable.
fn resolve_name(name: &str, env: &ParamEnv, model_slots: &[(String, usize)]) -> expr::NameRef {
    if env.is_global(name) {
        expr::NameRef::Global
    } else if let Some((_, slot)) = model_slots.iter().find(|(n, _)| n == name) {
        expr::NameRef::Slot(*slot)
    } else {
        expr::NameRef::Unknown
    }
}

fn seg_from(seg: (String, Option<usize>)) -> RhsSeg {
    match seg {
        (text, None) => RhsSeg::Text(text),
        (_, Some(slot)) => RhsSeg::Slot(slot),
    }
}

/// Merge adjacent literal pieces so the generated equation is readable and
/// testable (" * 2" stays one piece).
fn coalesce(segs: Vec<RhsSeg>) -> Vec<RhsSeg> {
    let mut out: Vec<RhsSeg> = Vec::with_capacity(segs.len());
    for seg in segs {
        match (out.last_mut(), &seg) {
            (Some(RhsSeg::Text(acc)), RhsSeg::Text(t)) => acc.push_str(t),
            _ => out.push(seg),
        }
    }
    out
}

/// A scalar in a feature payload: an expression string, or a bare JSON
/// number read as the same kind of literal (solidcraft/57 writes raw
/// numbers where the script shape writes strings).
fn scalar_text(v: &Value) -> Option<String> {
    match v {
        Value::String(t) => Some(t.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Primitives replay as a mapped generated sketch plus a blind extrude
/// through the direction truth table. Placement (corner/base) must be
/// literal and on the base plane; the axis must be Z; sizes may be
/// parametric and become dimension equations. A cylinder radius at or below
/// 2e-6 m is refused: at that scale the circle-dimension kind (radius vs
/// diameter reading) cannot be established. Everything else stops
/// (off-plane placement needs extrude start-offset machinery).
#[allow(clippy::too_many_arguments)]
fn parse_prim(
    name: String,
    kind: PrimKind,
    feature: &Value,
    env: &ParamEnv,
    design_factor: f64,
    design_text: &'static str,
    model_slots: &mut Vec<(String, usize)>,
    next_slot: &mut usize,
) -> std::result::Result<PrimStep, String> {
    let operation = match feature.get("operation") {
        None | Some(Value::Null) => "new_body",
        Some(Value::String(s)) => s.as_str(),
        Some(_) => return Err("primitive operation is present but not a string".to_string()),
    };
    match operation {
        "new_body" => {}
        other => {
            return Err(format!(
                "primitive operation {other:?} is outside the supported subset; targeted merge and feature scope await the Phase B body registry and the merge/scope probe (the emitted flags are proven for new_body only)"
            ))
        }
    }
    // Geometry-affecting primitive semantics the shared parser does not
    // replay must stop named, never be rebuilt from the wrong reference
    // (a centred box is not a corner box).
    match feature.get("centred") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => {}
        Some(Value::Bool(true)) => {
            return Err(
                "the centred box placement is outside the supported subset (corner placement only; Phase B)"
                    .to_string(),
            )
        }
        Some(_) => return Err("primitive centred is present but not a boolean".to_string()),
    }
    if let Some(references) = feature.get("references") {
        let occupied = match references {
            Value::Null => false,
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
            _ => true,
        };
        if occupied {
            return Err(
                "the cylinder references field is outside the supported subset (placement references are not replayed; Phase B)"
                    .to_string(),
            );
        }
    }
    if let Some(axis) = feature.get("axis") {
        let ok = axis
            .as_array()
            .map(|a| {
                a.len() == 3
                    && a[0].as_f64().unwrap_or(1.0).abs() < 1e-9
                    && a[1].as_f64().unwrap_or(1.0).abs() < 1e-9
                    && (a[2].as_f64().unwrap_or(0.0) - 1.0).abs() < 1e-9
            })
            .unwrap_or(false);
        if !ok {
            return Err(
                "a primitive axis other than Z is outside the supported subset".to_string(),
            );
        }
    }
    let place_key = match kind {
        PrimKind::Box => "corner",
        PrimKind::Cylinder => "base",
    };
    let place_v = feature
        .get(place_key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("the primitive has no {place_key}"))?;
    if place_v.len() != 3 {
        return Err(format!(
            "the primitive {place_key} is not three expressions"
        ));
    }
    let mut xyz = [0.0f64; 3];
    for (i, e) in place_v.iter().enumerate() {
        let text = scalar_text(e)
            .ok_or_else(|| format!("the primitive {place_key} is not an expression string"))?;
        let node = expr::parse(&text).map_err(|e| format!("the primitive {place_key}: {e}"))?;
        if !matches!(node, expr::Node::Number { .. }) {
            return Err(format!(
                "the primitive {place_key} is not a literal in this phase"
            ));
        }
        let ctx = expr::Context {
            bare_length: 0.001,
            bare_angle: 1.0,
            bare_dim: expr::Dim::Length,
            bare_text: design_text,
        };
        let q = expr::eval(&node, &ctx, &env.lookup())
            .map_err(|e| format!("the primitive {place_key}: {e}"))?;
        xyz[i] = q.value;
    }
    if xyz[2].abs() > 1e-6 {
        return Err(
            "the primitive sits off the base plane (extrude start-offset is later work)"
                .to_string(),
        );
    }

    // Sizes: each may be a literal or an equation.
    let size_keys: &[&str] = match kind {
        PrimKind::Box => &["length", "width", "height"],
        PrimKind::Cylinder => &["radius", "height"],
    };
    let mut sizes = Vec::new();
    for key in size_keys {
        let text = feature
            .get(key)
            .and_then(scalar_text)
            .ok_or_else(|| format!("the primitive has no {key}"))?;
        let node = expr::parse(&text).map_err(|e| format!("the primitive {key}: {e}"))?;
        let literal = matches!(node, expr::Node::Number { .. });
        let ctx = expr::Context {
            bare_length: if literal { 0.001 } else { design_factor },
            bare_angle: 1.0,
            bare_dim: expr::Dim::Length,
            bare_text: design_text,
        };
        let q = expr::eval(&node, &ctx, &env.lookup())
            .map_err(|e| format!("the primitive {key}: {e}"))?;
        if q.dim != expr::Dim::Length {
            return Err(format!("the primitive {key} is not a length"));
        }
        let value_mm = q.value * 1000.0;
        if !(value_mm.is_finite() && value_mm > 0.0) {
            return Err(format!("the primitive {key} is not a positive length"));
        }
        // The circle-size dimension kind (radius- vs diameter-kind reading)
        // is established against the drawn radius with a 1e-6 m tolerance;
        // at or below 2e-6 m the two candidate readings coincide within that
        // tolerance (and a halved driven radius would pass the geometry
        // check), so the kind can never be established here and the size is
        // refused fail-closed (docs/mapping.md, Primitives).
        if kind == PrimKind::Cylinder && *key == "radius" && q.value <= 2e-6 {
            return Err(format!(
                "the cylinder radius {} m is below twice the 1e-6 m dimension-kind verification tolerance: a radius-kind circle dimension cannot be distinguished from a diameter-kind reading at this scale",
                q.value
            ));
        }
        let equation = if literal {
            None
        } else {
            let segs = expr::translate_with(&node, &ctx, &|n| resolve_name(n, env, model_slots))
                .map_err(|e| format!("the primitive {key}: {e}"))?;
            let slot = *next_slot;
            *next_slot += 1;
            Some(DimEquation {
                which: 0,
                rhs: expr::translate(&node, &ctx),
                segs: coalesce(segs.into_iter().map(seg_from).collect()),
                expect_si: q.value,
                slot,
            })
        };
        sizes.push(PrimSize {
            value: value_mm,
            equation,
        });
    }
    if let Some(names) = feature.get("param_names").and_then(Value::as_array) {
        for (i, size) in sizes.iter().enumerate() {
            if let (Some(n), Some(eq)) =
                (names.get(i).and_then(Value::as_str), size.equation.as_ref())
            {
                model_slots.push((n.to_string(), eq.slot));
            }
        }
    }
    Ok(PrimStep {
        name,
        kind,
        origin: [xyz[0] * 1000.0, xyz[1] * 1000.0],
        sizes,
    })
}

/// Pick points: plain [x,y,z] arrays (FacePick) or objects with an `at`
/// array. Model millimetres; the replay policy maps them identically into
/// the rebuilt model and selections are fail-closed.
fn pick_coords(v: &Value) -> std::result::Result<[f64; 3], String> {
    let arr = match v {
        Value::Array(a) => Some(a),
        other => other.get("at").and_then(Value::as_array),
    }
    .ok_or("a pick is neither [x,y,z] nor {at: [x,y,z]}".to_string())?;
    if arr.len() != 3 {
        return Err("a pick is not three numbers".to_string());
    }
    let mut out = [0.0f64; 3];
    for (i, e) in arr.iter().enumerate() {
        out[i] = e.as_f64().ok_or("a pick coordinate is not a number")?;
        if !out[i].is_finite() {
            return Err("a pick coordinate is not finite".to_string());
        }
    }
    Ok(out)
}

/// Fillet, chamfer, shell, draft and simple holes. Variant-rich forms keep
/// the constant/default essence and report the variants as degrade triggers;
/// forms where the variation is the essence stop.
#[allow(clippy::too_many_arguments)]
fn parse_mod(
    kind: &str,
    name: String,
    feature: &Value,
    env: &ParamEnv,
    design_factor: f64,
    design_text: &'static str,
    model_slots: &[(String, usize)],
    next_slot: &mut usize,
) -> std::result::Result<ModStep, String> {
    let mut degraded_variants = Vec::new();
    let mut equations = Vec::new();
    let mut eval_len = |text: &str,
                        which: usize,
                        equations: &mut Vec<DimEquation>|
     -> std::result::Result<f64, String> {
        let node = expr::parse(text).map_err(|e| format!("{kind}: {e}"))?;
        let literal = matches!(node, expr::Node::Number { .. });
        let ctx = expr::Context {
            bare_length: if literal { 0.001 } else { design_factor },
            bare_angle: 1.0,
            bare_dim: expr::Dim::Length,
            bare_text: design_text,
        };
        let q = expr::eval(&node, &ctx, &env.lookup()).map_err(|e| format!("{kind}: {e}"))?;
        if !literal {
            let segs = expr::translate_with(&node, &ctx, &|n| resolve_name(n, env, model_slots))
                .map_err(|e| format!("{kind}: {e}"))?;
            let slot = *next_slot;
            *next_slot += 1;
            equations.push(DimEquation {
                which,
                rhs: expr::translate(&node, &ctx),
                segs: coalesce(segs.into_iter().map(seg_from).collect()),
                expect_si: q.value,
                slot,
            });
        }
        Ok(q.value)
    };
    let picks = |key: &str| -> std::result::Result<Vec<[f64; 3]>, String> {
        let arr = feature
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{kind} has no {key}"))?;
        if arr.is_empty() {
            return Err(format!("{kind} has no picks in {key}"));
        }
        arr.iter().map(pick_coords).collect()
    };

    let mod_kind = match kind {
        "fillet" => {
            for (key, why) in [
                ("style", "fillet style"),
                ("setbacks", "corner setbacks"),
                ("turn", "fillet turn"),
            ] {
                if feature.get(key).is_some() {
                    degraded_variants.push(why.to_string());
                }
            }
            if feature
                .get("fit")
                .and_then(Value::as_str)
                .map(|f| f != "exact")
                .unwrap_or(false)
            {
                degraded_variants.push("fillet fit".to_string());
            }
            if feature
                .get("corner")
                .and_then(Value::as_str)
                .map(|c| c != "rolling_ball")
                .unwrap_or(false)
            {
                degraded_variants.push("fillet corner".to_string());
            }
            let radius = eval_len(
                feature
                    .get("radius")
                    .and_then(Value::as_str)
                    .ok_or("fillet has no radius expression")?,
                1,
                &mut equations,
            )?;
            ModKind::Fillet {
                edges: picks("edges")?,
                radius: radius.abs(),
                tangent_chain: feature
                    .get("tangent_chain")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            }
        }
        "chamfer" => {
            if feature.get("distance2").is_some() || feature.get("angle").is_some() {
                return Err("an unequal chamfer is outside the supported subset".to_string());
            }
            let distance = eval_len(
                feature
                    .get("distance")
                    .and_then(Value::as_str)
                    .ok_or("chamfer has no distance expression")?,
                1,
                &mut equations,
            )?;
            ModKind::Chamfer {
                edges: picks("edges")?,
                distance: distance.abs(),
                flip: feature
                    .get("flip")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                tangent_chain: feature
                    .get("tangent_chain")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            }
        }
        "shell" => {
            let thickness = eval_len(
                feature
                    .get("thickness")
                    .and_then(Value::as_str)
                    .ok_or("shell has no thickness expression")?,
                1,
                &mut equations,
            )?;
            let direction = feature
                .get("direction")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !matches!(direction.as_str(), "" | "inside" | "outside" | "both") {
                return Err(format!("shell direction {direction:?} is unknown"));
            }
            ModKind::Shell {
                faces: picks("faces")?,
                thickness: thickness.abs(),
                direction,
                tangent_chain: feature
                    .get("tangent_chain")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }
        }
        "draft" => {
            // The neutral plane must be an origin plane in this phase.
            let neutral = feature
                .get("neutral")
                .and_then(|n| n.get("type"))
                .and_then(Value::as_str);
            if neutral != Some("origin") {
                return Err("a draft neutral plane outside the origin planes is outside the supported subset".to_string());
            }
            let angle_text = feature
                .get("angle")
                .and_then(Value::as_str)
                .ok_or("draft has no angle expression")?;
            let node = expr::parse(angle_text).map_err(|e| format!("draft: {e}"))?;
            let literal = matches!(node, expr::Node::Number { .. });
            let ctx = expr::Context {
                bare_length: design_factor,
                bare_angle: 1.0,
                bare_dim: expr::Dim::Angle,
                bare_text: design_text,
            };
            let q = expr::eval(&node, &ctx, &env.lookup()).map_err(|e| format!("draft: {e}"))?;
            if q.dim != expr::Dim::Angle {
                return Err("draft angle is not an angle".to_string());
            }
            if !literal {
                let segs =
                    expr::translate_with(&node, &ctx, &|n| resolve_name(n, env, model_slots))
                        .map_err(|e| format!("draft: {e}"))?;
                let slot = *next_slot;
                *next_slot += 1;
                equations.push(DimEquation {
                    which: 1,
                    rhs: expr::translate(&node, &ctx),
                    segs: coalesce(segs.into_iter().map(seg_from).collect()),
                    expect_si: q.value,
                    slot,
                });
            }
            ModKind::Draft {
                faces: picks("faces")?,
                angle_rad: q.value.abs(),
            }
        }
        "hole" => {
            match feature
                .get("hole")
                .and_then(Value::as_str)
                .unwrap_or("simple")
            {
                "simple" => {}
                "drilled" => degraded_variants.push("drill point".to_string()),
                other => {
                    return Err(format!(
                        "hole kind {other:?} is outside the supported subset"
                    ))
                }
            }
            if feature.get("thread").is_some()
                || feature.get("to").is_some()
                || feature.get("points").is_some()
                || feature
                    .get("modeled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                return Err(
                    "a threaded, to-object or sketch-point hole is outside the supported subset"
                        .to_string(),
                );
            }
            let at = pick_coords(&Value::Array(
                feature
                    .get("position")
                    .and_then(Value::as_array)
                    .cloned()
                    .ok_or("hole has no position")?,
            ))?;
            let diameter = eval_len(
                feature
                    .get("diameter")
                    .and_then(Value::as_str)
                    .ok_or("hole has no diameter expression")?,
                1,
                &mut equations,
            )?;
            let depth = feature
                .get("depth")
                .and_then(Value::as_str)
                .map(|t| eval_len(t, 2, &mut equations))
                .transpose()?;
            ModKind::Hole {
                at,
                diameter: diameter.abs(),
                depth: depth.map(f64::abs),
            }
        }
        other => return Err(format!("{other} is not a modification feature")),
    };
    Ok(ModStep {
        name,
        kind: mod_kind,
        equations,
        degraded_variants,
    })
}

/// Rectangular feature patterns and mirrors about origin planes. Seeds must
/// name mapped features earlier in the recipe (verified native features at
/// runtime); body patterns, second directions and non-origin mirror planes
/// stay outside the subset.
#[allow(clippy::too_many_arguments)]
fn parse_pattern(
    kind: &str,
    name: String,
    feature: &Value,
    _env: &ParamEnv,
    _design_factor: f64,
    _design_text: &'static str,
    steps: &[PlannedStep],
    sketch_steps: &[(u64, usize)],
    _model_slots: &[(String, usize)],
    _next_slot: &mut usize,
) -> std::result::Result<PatStep, String> {
    let mut degraded_variants = Vec::new();
    let equations = Vec::new();
    if feature
        .get("bodies")
        .and_then(Value::as_array)
        .map(|b| !b.is_empty())
        .unwrap_or(false)
    {
        return Err("a body pattern is outside the supported subset".to_string());
    }
    let seeds_v = feature
        .get("features")
        .and_then(Value::as_array)
        .ok_or("the pattern names no features")?;
    if seeds_v.is_empty() {
        return Err("the pattern names no features".to_string());
    }
    let mut seeds = Vec::new();
    for id in seeds_v {
        let fid = id.as_u64().ok_or("a seed is not a feature id")?;
        let step_index = sketch_steps
            .iter()
            .find(|(f, _)| *f == fid)
            .map(|(_, i)| *i);
        let step_index = match step_index {
            Some(i) => i,
            None => return Err(format!("seed feature {fid} is not in this recipe")),
        };
        let live = steps.iter().any(|s| {
            s.index == step_index
                && matches!(
                    s.work,
                    Work::Extrude(_) | Work::Revolve(_) | Work::Primitive(_) | Work::Modifier(_)
                )
        });
        if !live {
            return Err(format!(
                "seed feature {fid} does not replay natively (fallback bodies cannot seed a pattern)"
            ));
        }
        seeds.push(step_index);
    }

    let pat_kind = match kind {
        "pattern" => {
            // Direction and axis selection for feature patterns awaits API
            // pinning; the parser and emitter seam is in place (see #6).
            return Err(
                "a feature pattern's direction selection awaits API pinning (ticket #6 remainder)"
                    .to_string(),
            );
        }
        "mirror" => {
            let plane_v = feature.get("plane").ok_or("the mirror has no plane")?;
            let plane =
                match (
                    plane_v.get("type").and_then(Value::as_str),
                    plane_v.get("name").and_then(Value::as_str),
                ) {
                    (Some("origin"), Some("XY")) => OriginPlane::Xy,
                    (Some("origin"), Some("XZ")) => OriginPlane::Xz,
                    (Some("origin"), Some("YZ")) => OriginPlane::Yz,
                    _ => return Err(
                        "a mirror plane outside the origin planes is outside the supported subset"
                            .to_string(),
                    ),
                };
            if feature
                .get("combine")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                degraded_variants.push("combine".to_string());
            }
            PatKind::Mirror { seeds, plane }
        }
        other => return Err(format!("{other} is not a pattern feature")),
    };
    Ok(PatStep {
        name,
        kind: pat_kind,
        equations,
        degraded_variants,
    })
}
