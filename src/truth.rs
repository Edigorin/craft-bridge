//! The extrude/revolve direction truth table (Phase 1 batch 2).
//!
//! Every constant below is MEASURED ON THE SEAT from a direction probe's
//! runtime report (`corpus/compat-probe/probe-extrudedir`, `-twosided`,
//! `-throughall`, `-revolvedir`). `0` is the UNMEASURED sentinel: every
//! decision that consults an unmeasured cell refuses with a named reason and
//! the step never reports `created` (docs/mapping.md, "Direction truth
//! table"). Filling a constant is a one-line change here, followed by golden
//! and corpus regeneration; the emitted macro carries the value in its
//! `TT_*` VBA constants and names its probe in every refusal.
//!
//! Structural law gate: the probes run one frame and one directed axis, so
//! the revolve rows below assume the right-hand-rule law
//! (`TT_REV_DIR_LAW`) about the drawn axis direction. That law is a
//! SEPARATE acceptance: filling the sense constants alone unlocks nothing
//! until the law value is set from seat evidence (the sweep-direction
//! acceptance check or a discriminating probe).

/// `FeatureExtrusion2`'s `Dir=True` flag sends material toward this many
/// times the profile sketch normal `n_sk` (`+1` = toward `n_sk`). The
/// two-sided and blind call shapes are measured separately below.
///
/// SLOT: probe-extrudedir cases `dir-true` / `dir-false` (the box's X extent
/// on the verified Front frame names the physical side; D1 identity
/// confirms the dimension).
pub const TT_EX_DIR_TRUE_SIDE: i32 = 0;

/// With the emitter's two-sided `FeatureExtrusion2` call shape (`Dir=False`,
/// first depth in argument 1), the FIRST depth's end sits toward this many
/// times `n_sk`. A source first side that resolves elsewhere swaps the two
/// depth arguments (and the D1/D2 dimension indices).
///
/// SLOT: probe-twosided (the box's unequal X extents name the assignment;
/// both D1/D2 identities confirm the arguments).
pub const TT_EX_FIRST_SIDE: i32 = 0;

/// The through-all call shape's `Dir=True` reading (same form as
/// `TT_EX_DIR_TRUE_SIDE`). probe-throughall's own interpretation contract
/// allows an INCONCLUSIVE result (a fresh part offers no bounded material
/// for "through all"; the side is never inferred from the blind cases), in
/// which case this stays `0` and through-all extrudes refuse with a named
/// reason.
///
/// SLOT: probe-throughall cases `dir-true` / `dir-false`.
pub const TT_EX_THROUGH_ALL_DIR: i32 = 0;

/// With the emitter's blind revolve call shape (`ReverseDir=False`), the
/// sweep is the right-hand rotation about the drawn axis direction (the
/// construction line, start to end) by `+angle` when this is `+1`, and
/// about the opposite direction when `-1`.
///
/// SLOT: probe-revolvedir cases `blind-270-normal` / `blind-270-reverse`
/// (the mass-property centroid names the sweep side; the axis endpoint
/// order is pinned as evidence).
pub const TT_REV_SENSE: i32 = 0;

/// With the emitter's two-sided revolve call shape (`ReverseDir=False`), the
/// FIRST angle sweeps with right-hand sense `+1` or `-1` about the drawn
/// axis direction (start to end).
///
/// SLOT: probe-revolvedir case `twosided-100-150` (the centroid names which
/// side received the 100 deg first angle; D1/D2 identities confirm the
/// arguments).
pub const TT_REV_TWO_FIRST_SIDE: i32 = 0;

/// The structural law the two rows above are read under. `0` =
/// UNESTABLISHED: directional revolves (blind partial and two-sided) refuse
/// with a named reason even when the sense constants are filled, because the
/// probes measure one frame and one directed axis only. `1` = the
/// right-hand-rule law about the drawn axis direction (start to end) has
/// been accepted from seat evidence.
///
/// SLOT: not filled from the four probes alone; set from the sweep-direction
/// seat-acceptance check ("sweep direction and volume of partial revolves
/// are acceptance checks") or a discriminating probe.
pub const TT_REV_DIR_LAW: i32 = 0;

/// Tolerance for profile-side classification, in millimetres (the recipe's
/// sketch coordinates).
pub const SIDE_TOL_MM: f64 = 1e-6;

/// The blind/through-all extrude `Dir` flag: the material must reach
/// `want_side * n_sk` (`+1` = toward the profile sketch normal) and the
/// call's `Dir=True` reaches `tt_dir_true_side * n_sk`. `None` = the
/// semantics are unestablished (the caller names the probe and refuses).
pub fn extrude_dir_flag(want_side: i32, tt_dir_true_side: i32) -> Option<bool> {
    if tt_dir_true_side == 0 {
        return None;
    }
    Some(want_side == tt_dir_true_side)
}

/// The two-sided extrude argument swap: `want_first_side` is where the
/// source's first depth's end must sit, in units of `n_sk`; `tt_first_side`
/// is [`TT_EX_FIRST_SIDE`]. `true` = the two depth arguments (and their
/// D1/D2 dimension indices) are swapped. `None` = unestablished.
pub fn extrude_two_sided_swap(want_first_side: i32, tt_first_side: i32) -> Option<bool> {
    if tt_first_side == 0 {
        return None;
    }
    Some(want_first_side != tt_first_side)
}

/// The blind revolve `ReverseDir` flag: the sweep must be the right-hand
/// rotation by `want_sense * angle` about the drawn axis direction and the
/// call's `ReverseDir=False` sweeps with sense `tt_rev_sense`. `None` =
/// unestablished.
pub fn revolve_reverse_flag(want_sense: i32, tt_rev_sense: i32) -> Option<bool> {
    if tt_rev_sense == 0 {
        return None;
    }
    Some(want_sense != tt_rev_sense)
}

/// The two-sided revolve argument swap: `want_first_sense` is the right-hand
/// sense the first (source) angle must sweep about the drawn axis direction;
/// `tt_two_first_sense` is [`TT_REV_TWO_FIRST_SIDE`]. `true` = the two angle
/// arguments (and their D1/D2 dimension indices) are swapped. `None` =
/// unestablished.
pub fn revolve_two_sided_swap(want_first_sense: i32, tt_two_first_sense: i32) -> Option<bool> {
    if tt_two_first_sense == 0 {
        return None;
    }
    Some(want_first_sense != tt_two_first_sense)
}

/// Which side of the revolve axis the profile lies on, in source sketch
/// millimetres: `+1` / `-1`, or a named refusal.
///
/// This is the SolidCraft kernel's own canonical factor
/// (`kernel/src/build.rs` `canonical_revolve_axis` / `revolved`: partial
/// turns go toward the side the sketch normal points to, and the rotation
/// axis is flipped by `sign(axis x radial . normal)`). The classification
/// uses only the NON-CONSTRUCTION profile geometry - line endpoints and
/// circle centres with their radii - so construction axes, unused points
/// and a circle's extent beyond its centre are all accounted for. The axis
/// direction is normalized first. A profile that straddles the axis, or
/// whose side cannot be established (no profile geometry, or everything on
/// the axis), refuses with the reason named.
pub fn profile_side(
    points: &[[f64; 2]],
    lines: &[(usize, usize, bool)],
    circles: &[(usize, f64, bool)],
    origin: [f64; 2],
    dir: [f64; 2],
) -> Result<i32, String> {
    let norm = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    if !norm.is_finite() || norm < 1e-12 {
        return Err("the revolve axis direction is zero".to_string());
    }
    let d = [dir[0] / norm, dir[1] / norm];
    // The signed perpendicular distance of a point from the axis line.
    let cross = |p: [f64; 2]| d[0] * (p[1] - origin[1]) - d[1] * (p[0] - origin[0]);
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let mut seen = false;
    let mut grow = |a: f64, b: f64| {
        lo = lo.min(a).min(b);
        hi = hi.max(a).max(b);
        seen = true;
    };
    for (a, b, construction) in lines {
        if *construction {
            continue;
        }
        let pa = points
            .get(*a)
            .copied()
            .ok_or("the line endpoint is not in the point list")?;
        let pb = points
            .get(*b)
            .copied()
            .ok_or("the line endpoint is not in the point list")?;
        grow(cross(pa), cross(pb));
    }
    for (c, r, construction) in circles {
        if *construction {
            continue;
        }
        let pc = points
            .get(*c)
            .copied()
            .ok_or("the circle centre is not in the point list")?;
        if !r.is_finite() || *r < 0.0 {
            return Err("the circle radius is not a finite length".to_string());
        }
        let sc = cross(pc);
        grow(sc - r, sc + r);
    }
    if !seen {
        return Err("the sketch carries no profile geometry beside the revolve axis".to_string());
    }
    if lo < -SIDE_TOL_MM && hi > SIDE_TOL_MM {
        return Err("the profile crosses the revolve axis".to_string());
    }
    if hi > SIDE_TOL_MM {
        return Ok(1);
    }
    if lo < -SIDE_TOL_MM {
        return Ok(-1);
    }
    Err("the profile lies on the revolve axis; its side cannot be established".to_string())
}
