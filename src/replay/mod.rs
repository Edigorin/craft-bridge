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
    },
    Sketch(SketchStep),
    Extrude(ExtrudeStep),
    Revolve(RevolveStep),
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

/// A sketch that can be replayed: lines and circles on an origin plane, only
/// the relation and dimension kinds listed in docs/mapping.md.
#[derive(Debug, Clone, PartialEq)]
pub struct SketchStep {
    pub name: String,
    pub plane: OriginPlane,
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
    /// One side, blind depth in metres.
    Blind { depth_m: f64, negative: bool },
    /// Symmetric about the sketch plane; total thickness in metres (the
    /// legacy_per_side convention is folded in at parse time).
    Symmetric { total_m: f64 },
    /// Two blind sides, each in metres.
    TwoSided { first_m: f64, second_m: f64 },
    /// Through everything in one direction.
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

/// An equation that drives a feature dimension, captured at runtime by name.
#[derive(Debug, Clone, PartialEq)]
pub struct DimEquation {
    /// Which dimension of the feature: 1 for D1, 2 for D2.
    pub which: usize,
    /// The equation right-hand side in SolidWorks syntax.
    pub rhs: String,
    /// The evaluated SI value it must produce (verification).
    pub expect_si: f64,
}

/// How far a revolve turns.
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

/// Build the plan from a validated recipe (the artifact's `recipe.json`).
pub fn plan(recipe: &Value, limits: &Limits) -> Result<Vec<PlannedStep>> {
    let commands = crate::recipe::commands_of(recipe)?;
    let mut steps = Vec::with_capacity(commands.len());
    let mut stopped = false;
    let mut env = ParamEnv::default();
    let mut design_factor = 0.001f64;
    let mut design_text: &'static str = "mm";
    // Recipe feature id -> plan index of its sketch step.
    let mut sketch_steps: Vec<(u64, usize)> = Vec::new();
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
                    command: id,
                    status: PlanStatus::NotExecuted,
                    detail: format!("units recorded ({unit}); values replay in millimetres"),
                    work: Work::Units {
                        unit: unit.to_string(),
                    },
                }
            }
            "parameters.add" => {
                let name = params
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("(unnamed)")
                    .to_string();
                let expression = params
                    .and_then(|p| p.get("expression"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let unit = params
                    .and_then(|p| p.get("unit"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let ctx = expr::Context::for_unit(&unit, design_factor, design_text);
                let evaluated = expr::parse(&expression).ok().and_then(|node| {
                    expr::eval(&node, &ctx, &env.lookup())
                        .ok()
                        .map(|q| (expr::translate(&node, &ctx), q))
                });
                let planned = match evaluated {
                    Some((equation_rhs, q)) if unit != "text" => {
                        env.insert_global(&name, q);
                        PlannedStep {
                            index,
                            command: id,
                            status: PlanStatus::Mapped,
                            detail: format!("equation \"{name}\" = {equation_rhs}"),
                            work: Work::UserParam {
                                name,
                                expression,
                                equation_rhs,
                                expect_si: q.value,
                            },
                        }
                    }
                    _ => PlannedStep {
                        index,
                        command: id,
                        status: PlanStatus::NotExecuted,
                        detail:
                            "not an equation: text parameter or an expression outside the supported subset"
                                .to_string(),
                        work: Work::Note,
                    },
                };
                planned
            }
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
            )?,
            _ => PlannedStep {
                index,
                command: id,
                status: PlanStatus::FallbackPlanned,
                detail: "no translation; a fallback body lands with the hybrid-fallback ticket"
                    .to_string(),
                work: Work::Unmappable,
            },
        };
        if let Work::Sketch(_) = &planned.work {
            if let Some(fid) = params
                .and_then(|p| p.get("feature"))
                .and_then(|f| f.get("id"))
                .and_then(Value::as_u64)
            {
                sketch_steps.push((fid, index));
            }
        }
        let stop_here = matches!(planned.work, Work::Unmappable);
        steps.push(planned);
        if stop_here {
            stopped = true;
        }
    }
    Ok(steps)
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
        "sketch" => match parse_sketch(name, feature, limits, &model_exprs, env, design_factor, design_text) {
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
            match parse_extrude(name, sketch_step, feature, env, design_factor, design_text) {
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
                    ExtrudeKind::TwoSided { first_m, second_m } => format!(
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
            match parse_revolve(name, sketch_step, feature, sketch, env, design_factor, design_text)
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
        _ => unmappable(format!(
            "feature type {kind:?} has no translation yet; a fallback body lands with the hybrid-fallback ticket"
        )),
    })
}

/// Parse a sketch. Malformed recipe data is InvalidRecipe; content outside
/// the tracer subset is Unsupported (the stop boundary) or a degrade trigger
/// (kept on the sketch and reported).
fn parse_sketch(
    name: String,
    feature: &Value,
    limits: &Limits,
    model_exprs: &[(String, String, String)],
    env: &ParamEnv,
    design_factor: f64,
    design_text: &'static str,
) -> Result<(SketchStep, bool)> {
    let plane_v = feature
        .get("plane")
        .ok_or_else(|| Error::InvalidRecipe("sketch has no plane".to_string()))?;
    let plane = match (
        plane_v.get("type").and_then(Value::as_str),
        plane_v.get("name").and_then(Value::as_str),
    ) {
        (Some("origin"), Some("XY")) => OriginPlane::Xy,
        (Some("origin"), Some("XZ")) => OriginPlane::Xz,
        (Some("origin"), Some("YZ")) => OriginPlane::Yz,
        _ => {
            return Err(Error::Unsupported(
                "sketch plane is not an origin plane".to_string(),
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
    for (i, p) in points_v.iter().enumerate() {
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
        let construction = c
            .get("construction")
            .and_then(Value::as_bool)
            .unwrap_or(false);
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
                Error::InvalidRecipe(format!(
                    "constraint {i} references point {p}, which no curve endpoint occupies"
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
                "angle" => dimensions.push(DimensionT {
                    kind: DimKind::Angle,
                    a: line_index("a")?,
                    b: Some(line_index("b")?),
                    value: value()?,
                    param: k.get("param").and_then(Value::as_str).map(str::to_string),
                }),
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
        points,
        lines,
        circles,
        relations,
        dimensions,
        dim_equations: Vec::new(),
        dropped_constraints,
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
            let mut used = Vec::new();
            expr::names(&node, &mut used);
            if !used.iter().all(|u| env.is_global(u)) {
                return None;
            }
            let ctx = expr::Context::for_unit(unit, design_factor, design_text);
            expr::eval(&node, &ctx, &env.lookup()).ok()?;
            Some(DimEquation {
                which: 0,
                rhs: expr::translate(&node, &ctx),
                expect_si: d.value,
            })
        });
        match bound {
            Some(equation) => sketch.dim_equations.push((di, equation)),
            None => sketch
                .dropped_constraints
                .push(format!("{param} (equation not expressible)")),
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
fn parse_extrude(
    name: String,
    sketch_step: usize,
    feature: &Value,
    env: &ParamEnv,
    design_factor: f64,
    design_text: &'static str,
) -> std::result::Result<ExtrudeStep, String> {
    let extent = feature
        .get("extent")
        .ok_or_else(|| "extrude has no extent".to_string())?;
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
    // numbers inside non-literal expressions read in the design unit.
    let mut equations = Vec::new();
    let mut eval_len = |text: &str, which: usize| -> std::result::Result<f64, String> {
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
            equations.push(DimEquation {
                which,
                rhs: expr::translate(&node, &ctx),
                expect_si: q.value,
            });
        }
        Ok(q.value)
    };

    let distance = extent
        .get("distance")
        .and_then(Value::as_str)
        .ok_or_else(|| "extrude distance is not an expression string".to_string())?;
    if through_all {
        let negative = match direction {
            "positive" => false,
            "negative" => true,
            other => {
                return Err(format!(
                    "extrude direction {other:?} with through-all is outside the supported subset"
                ))
            }
        };
        return Ok(ExtrudeStep {
            name,
            sketch_step,
            kind: ExtrudeKind::ThroughAll { negative },
            equations,
        });
    }
    let d1 = eval_len(distance, 1)?;

    let kind = match (direction, extent.get("distance2").and_then(Value::as_str)) {
        ("positive", None) => ExtrudeKind::Blind {
            depth_m: d1.abs(),
            negative: false,
        },
        ("negative", None) => ExtrudeKind::Blind {
            depth_m: d1.abs(),
            negative: true,
        },
        ("symmetric", None) => {
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
        ("positive", Some(d2s)) | ("negative", Some(d2s)) => {
            let d2 = eval_len(d2s, 2)?;
            ExtrudeKind::TwoSided {
                first_m: d1.abs(),
                second_m: d2.abs(),
            }
        }
        (other, _) => return Err(format!("extrude direction {other:?} is unknown")),
    };
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
fn parse_revolve(
    name: String,
    sketch_step: usize,
    feature: &Value,
    sketch: &SketchStep,
    env: &ParamEnv,
    design_factor: f64,
    design_text: &'static str,
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
        .ok_or_else(|| "revolve has no axis".to_string())?;
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
                .ok_or_else(|| "the revolve axis names no curve".to_string())?;
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
            // XY: u=X, v=Y; XZ: u=X, v=Z; YZ: u=Y, v=Z.
            let dir = match (sketch.plane, axis_v.get("axis").and_then(Value::as_str)) {
                (OriginPlane::Xy, Some("x")) => [1.0, 0.0],
                (OriginPlane::Xy, Some("y")) => [0.0, 1.0],
                (OriginPlane::Xz, Some("x")) => [1.0, 0.0],
                (OriginPlane::Xz, Some("z")) => [0.0, 1.0],
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
                .ok_or_else(|| "the axis line has no origin".to_string())?;
            if origin_v.len() != 3 {
                return Err("the axis line origin is not three expressions".to_string());
            }
            let mut xyz = [0.0f64; 3];
            for (i, e) in origin_v.iter().enumerate() {
                let text = e
                    .as_str()
                    .ok_or_else(|| "the axis origin is not an expression string".to_string())?;
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
                .ok_or_else(|| "the axis line has no direction".to_string())?;
            let mut d = [0.0f64; 3];
            for (i, e) in dir_v.iter().enumerate().take(3) {
                d[i] = e.as_f64().unwrap_or(0.0);
            }
            // The axis must lie in the sketch plane; its sketch-space frame
            // follows the same world-axis mapping as above.
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
                    ([xyz[0], xyz[2]], [d[0], d[2]])
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
                origin,
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
    let eval_angle = |text: &str,
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
            equations.push(DimEquation {
                which,
                rhs: expr::translate(&node, &ctx),
                expect_si: q.value,
            });
        }
        Ok(q.value)
    };
    let angle_text = feature
        .get("angle")
        .and_then(Value::as_str)
        .ok_or_else(|| "revolve angle is not an expression string".to_string())?;
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
            first_rad: angle.abs(),
            second_rad: a2.abs(),
        },
        (true, Some(_)) => {
            return Err("symmetric revolve with a second angle is contradictory".to_string())
        }
        (false, None) => RevolveKind::Blind {
            angle_rad: angle.abs(),
            reverse: angle < 0.0,
        },
    };
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
