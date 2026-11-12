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
    UserParam {
        name: String,
    },
    Sketch(SketchStep),
    Extrude(ExtrudeStep),
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
    /// Millimetres for lengths, radians for angles.
    pub value: f64,
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

/// A single-direction blind extrude over the whole sketch.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtrudeStep {
    pub name: String,
    /// Depth in metres.
    pub depth_m: f64,
    pub negative: bool,
}

/// Build the plan from a validated recipe (the artifact's `recipe.json`).
pub fn plan(recipe: &Value, limits: &Limits) -> Result<Vec<PlannedStep>> {
    let commands = crate::recipe::commands_of(recipe)?;
    let mut steps = Vec::with_capacity(commands.len());
    let mut stopped = false;
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
                PlannedStep {
                    index,
                    command: id,
                    status: PlanStatus::NotExecuted,
                    detail: "recorded; equations land with expressions".to_string(),
                    work: Work::UserParam { name },
                }
            }
            "timeline.add" => plan_feature(index, &id, params, limits)?,
            _ => PlannedStep {
                index,
                command: id,
                status: PlanStatus::FallbackPlanned,
                detail: "no translation; a fallback body lands with the hybrid-fallback ticket"
                    .to_string(),
                work: Work::Unmappable,
            },
        };
        let stop_here = matches!(planned.work, Work::Unmappable);
        steps.push(planned);
        if stop_here {
            stopped = true;
        }
    }
    Ok(steps)
}

fn plan_feature(
    index: usize,
    id: &str,
    params: Option<&Value>,
    limits: &Limits,
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
    Ok(match kind {
        "sketch" => match parse_sketch(name, feature, limits) {
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
        "extrude" => match parse_extrude(name, feature) {
            Ok(extrude) => {
                let detail = format!(
                    "extrude {} mm{} over the whole sketch",
                    extrude.depth_m * 1000.0,
                    if extrude.negative { " (negative)" } else { "" }
                );
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
        },
        _ => unmappable(format!(
            "feature type {kind:?} has no translation yet; a fallback body lands with the hybrid-fallback ticket"
        )),
    })
}

/// Parse a sketch. Malformed recipe data is InvalidRecipe; content outside
/// the tracer subset is Unsupported (the stop boundary) or a degrade trigger
/// (kept on the sketch and reported).
fn parse_sketch(name: String, feature: &Value, limits: &Limits) -> Result<(SketchStep, bool)> {
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
                    });
                }
                "angle" => dimensions.push(DimensionT {
                    kind: DimKind::Angle,
                    a: line_index("a")?,
                    b: Some(line_index("b")?),
                    value: value()?,
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

    let sketch = SketchStep {
        name,
        plane,
        points,
        lines,
        circles,
        relations,
        dimensions,
        dropped_constraints,
    };
    let degrade = !sketch.dropped_constraints.is_empty();
    Ok((sketch, degrade))
}

/// Single-direction blind extrude over the whole sketch, from a literal
/// distance expression only. Anything else is an Unsupported reason (the
/// stop rule).
fn parse_extrude(name: String, feature: &Value) -> std::result::Result<ExtrudeStep, String> {
    let extent = feature
        .get("extent")
        .ok_or_else(|| "extrude has no extent".to_string())?;
    for forbidden in [
        "distance2",
        "start_offset",
        "taper",
        "to",
        "from",
        "to_offset",
        "through_all",
        "corner",
        "to_body_apart",
    ] {
        if extent.get(forbidden).is_some() {
            return Err(format!(
                "extrude extent has {forbidden}, outside the tracer subset"
            ));
        }
    }
    if feature.get("thin").is_some() {
        return Err("thin extrude is outside the tracer subset".to_string());
    }
    match feature
        .get("profiles")
        .and_then(|p| p.get("type"))
        .and_then(Value::as_str)
    {
        Some("all") => {}
        _ => {
            return Err(
                "extrude profiles other than \"all\" are outside the tracer subset".to_string(),
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
                "extrude operation {other:?} is outside the tracer subset"
            ))
        }
    }
    let negative = match extent
        .get("direction")
        .and_then(Value::as_str)
        .unwrap_or("positive")
    {
        "positive" => false,
        "negative" => true,
        other => {
            return Err(format!(
                "extrude direction {other:?} is outside the tracer subset"
            ))
        }
    };
    let distance = extent
        .get("distance")
        .and_then(Value::as_str)
        .ok_or_else(|| "extrude distance is not a literal expression".to_string())?;
    let (v, is_angle) = parse_literal(distance).ok_or_else(|| {
        "extrude distance is not a literal expression (expressions land later)".to_string()
    })?;
    if is_angle {
        return Err("extrude distance is an angle".to_string());
    }
    Ok(ExtrudeStep {
        name,
        depth_m: v,
        negative,
    })
}

/// Parse a literal expression: a number with an optional unit suffix. Bare
/// numbers are millimetres. Returns (SI value: metres or radians, is_angle).
/// Anything with names or operators is None.
pub fn parse_literal(s: &str) -> Option<(f64, bool)> {
    let t = s.trim();
    let bytes = t.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let start = i;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if i == start {
        return None;
    }
    let number: f64 = t[start..i].parse().ok()?;
    if !number.is_finite() {
        return None;
    }
    let unit = t[i..].trim();
    let (factor, is_angle) = match unit {
        "" => (0.001, false),
        "mm" => (0.001, false),
        "cm" => (0.01, false),
        "m" => (1.0, false),
        "in" => (0.0254, false),
        "ft" => (0.3048, false),
        "deg" => (std::f64::consts::PI / 180.0, true),
        "rad" => (1.0, true),
        _ => return None,
    };
    let sign = if t.starts_with('-') { -1.0 } else { 1.0 };
    Some((sign * number.abs() * factor, is_angle))
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
