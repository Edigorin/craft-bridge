//! The extrude/revolve direction truth table (Phase 1 batch 2), MEASURED
//! ON THE SEAT 2026-11-16 from the four direction probes' runtime reports
//! (`/tmp/opencode/seat-reports/probe-{extrudedir,twosided,throughall,
//! revolvedir}.jsonl`; probes in `corpus/compat-probe/`).
//!
//! `0` remains the UNMEASURED sentinel in the decision functions: every
//! decision that consults an unmeasured cell refuses with a named reason
//! and the step never reports `created` (docs/mapping.md, "Direction truth
//! table"). The emitted macro carries the values in its `TT_*` VBA
//! constants and keeps the sentinel checks at runtime.
//!
//! Revolve evidence scope: the probes run one frame and ONE directed axis,
//! so the sweep rows are read under two candidate laws (the flag selects an
//! absolute side in units of the sketch normal, or a right-hand sense about
//! the drawn axis direction). The two laws give IDENTICAL flags exactly
//! when the configuration chirality matches the probe's
//! (`TT_REV_PROBE_CHIRALITY`): that scope is the executable gate
//! (the plan review's "evidence-scoped restriction") - directional revolve
//! forms run there under either law and refuse with a named reason outside
//! it.

/// `FeatureExtrusion2`'s `Dir=True` flag sends material toward this many
/// times the profile sketch normal `n_sk` (`+1` = toward `n_sk`).
///
/// MEASURED (probe-extrudedir `dir-true`/`dir-false`, 2026-11-16): Dir=True
/// put the blind 0.012 m box at x in [-0.012, 0] (the verified Front frame's
/// `n_sk` = +X) = toward MINUS the normal; Dir=False at [0, 0.012].
/// D1@Boss-Extrude1 read back the depth in SI metres.
pub const TT_EX_DIR_TRUE_SIDE: i32 = -1;

/// With the emitter's two-sided `FeatureExtrusion2` call shape (`Dir=False`,
/// first depth in argument 1), the FIRST depth's end sits toward this many
/// times `n_sk`. A source first side that resolves elsewhere swaps the two
/// depth arguments (and the D1/D2 dimension indices).
///
/// MEASURED (probe-twosided, 2026-11-16): first 0.010 / second 0.030 put the
/// box at x in [-0.03, 0.01] - the first depth's end on the PLUS normal side;
/// D1@ = 0.010 and D2@ = 0.030 confirm argument order to identity.
pub const TT_EX_FIRST_SIDE: i32 = 1;

/// The through-all call shape's `Dir=True` reading (same form as
/// `TT_EX_DIR_TRUE_SIDE`).
///
/// MEASURED (probe-throughall `dir-true`/`dir-false`, 2026-11-16): its own
/// measured cases, never inferred from blind - Dir=True put the body at
/// x in [-1, 0], Dir=False at [0, 1]: through-all obeys the Dir flag like
/// blind. The probe's own contract allowed an inconclusive result; the
/// fresh part produced measurable bodies both ways. No D1 dimension exists
/// on the through-all feature (the informative look came back empty).
pub const TT_EX_THROUGH_ALL_DIR: i32 = -1;

/// With the emitter's blind revolve call shape (`ReverseDir=False`), the
/// sweep is the right-hand rotation about the drawn axis direction (the
/// construction line, start to end) by `+angle` when this is `+1`, and
/// about the opposite direction when `-1`.
///
/// MEASURED (probe-revolvedir `blind-270-normal`/`blind-270-reverse`,
/// 2026-11-16): the drawn axis (0,-0.010)->(0,0.080) = +t with the profile
/// on the +u side. ReverseDir=False put the 270 deg centroid at
/// (u, n) = (-0.0046, -0.0046) = mid-angle -135 deg = toward MINUS the
/// normal first = the right-hand +angle sense about +t (t_hat x u_hat =
/// -n_hat); ReverseDir=True mirrored to mid-angle +135 deg. Readings via
/// MassProperty `centerofmass` (the American spelling; `centreofmass`
/// raises 438).
pub const TT_REV_SENSE: i32 = 1;

/// With the emitter's two-sided revolve call shape (`ReverseDir=False`), the
/// FIRST angle sweeps with right-hand sense `+1` or `-1` about the drawn
/// axis direction (start to end).
///
/// MEASURED (probe-revolvedir `twosided-100-150`, 2026-11-16): D1 = 100 deg
/// and D2 = 150 deg (identities in radians), centroid at mid-angle +24.6 deg
/// -> the swept span [-100, +150]: the FIRST angle occupied the minus-
/// normal side = the same right-hand +angle sense as the blind form above
/// (two independent measurements agreeing). D1 sweeps like ReverseDir=False.
pub const TT_REV_TWO_FIRST_SIDE: i32 = 1;

/// The evidence-scope gate for directional revolve forms: the configuration
/// chirality the probe establishes, `sign((drawn axis direction x profile
/// radial side) . sketch normal)`. The probe's pinned setup (drawn axis
/// (0,-0.010)->(0,0.080), profile on the +u side, n_sk = +X) has chirality
/// (t_hat x u_hat).n_hat = -1.
///
/// A replay's chirality is `map_side * sigma` (the runtime `gMapSide` times
/// the emit-time source-side factor). Where it equals this value the two
/// candidate sweep laws (absolute side in units of the normal; right-hand
/// sense about the drawn axis) produce IDENTICAL flags, so the sweep side
/// is established under either reading; elsewhere the two readings diverge,
/// the probes cannot distinguish them, and the step refuses with a named
/// reason - never `created`. This is the plan review's evidence-scoped
/// restriction; the sweep-direction seat-acceptance check remains the
/// outer guard for the residual structural hypotheses.
pub const TT_REV_PROBE_CHIRALITY: i32 = -1;

/// With the primitive blind extrude call shape (byte-identical to the
/// extrude family's blind form), `Dir=True` sends the primitive's height
/// toward this many times the profile sketch normal `n_sk` (`+1` = toward
/// `n_sk`).
///
/// UNMEASURED (`0`) until `probe-primorient` runs on the seat. The
/// composition it pins: the SolidCraft primitive extends toward +Z (the XY
/// source normal) from the base plane - source-pinned from the SolidCraft
/// kernel (`box_solid(corner, corner + (l, w, h))`,
/// `cylinder(base, +Z, r, h)`, with `parse_prim` forcing a base-plane
/// placement, a +Z axis and positive sizes) - so the height's end sits
/// toward `+1 * gMapSide` in `n_sk` units and the call flag is
/// `dirFlag = (wantSide = TT_PRIM_DIR_TRUE_SIDE)` (the same composition as
/// the extrude family, `truth::extrude_dir_flag`).
///
/// Evidence scope (kept narrow on purpose): `probe-primorient` measures the
/// flag on the XY plane's positive-normal configuration on this seat. That
/// pins the observed configuration and cross-checks
/// [`TT_EX_DIR_TRUE_SIDE`] on a second frame (the call shapes are
/// identical); it does NOT distinguish a transform-relative sketch normal
/// from a host-canonicalized normal when both agree there, and it can never
/// cover an opposing-normal map (`gMapSide = -1` is unreachable on the
/// owner's seat: every origin plane publishes a permutation map with side
/// +1). Filling this cell is evidence for the measured configuration only -
/// never proof of the reflected case, which stays an accepted residual
/// under the seat-acceptance outer guard, exactly like the extrude
/// family's composition on unmeasured frames.
pub const TT_PRIM_DIR_TRUE_SIDE: i32 = 0;

/// The evidence-scope check for a directional revolve: `None` while the
/// chirality cell is unmeasured, `true` inside the probe's scope.
pub fn revolve_in_evidence_scope(
    map_side: i32,
    sigma: i32,
    tt_probe_chirality: i32,
) -> Option<bool> {
    if tt_probe_chirality == 0 {
        return None;
    }
    Some(map_side * sigma == tt_probe_chirality)
}

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
