//! Exact conic-segment math for native `solidcraft/57` `type:"conic"`
//! records (research 09: `research/outputs/09-conic-rho-conversion.md`).
//!
//! A native conic record names three sketch points and a rho:
//! `a` = start point, `apex` = the point indexed as the ENDPOINT-TANGENT
//! INTERSECTION (called Q here - it is NOT a point on the curve and NOT
//! the classical principal vertex), `b` = end point, `rho` in (0,1) the
//! shoulder-height ratio. The exact curve is the rational quadratic
//! Bezier with control triangle (P0, Q, P2) and weight `w = rho/(1-rho)`.
//!
//! Normalization convention (research 09 eq 1/eq 2): the implicit form
//! produced here is `G(X) = s^2 - 4 w^2 r (1-r-s)` in the barycentric
//! coordinates (r, s) of X against the control triangle, so `G(Q) = 1`
//! and `G(M) = -w^2` for M = (P0+P2)/2. The worked examples in research 09
//! present a uniformly RESCALED equation (their `G(M) = -1`); the inverse
//! recovery uses the ratio `-G(M)/G(Q)` and is invariant under any
//! nonzero uniform scaling of G.
//!
//! Units: metres at this boundary (the 57 loader converts its native
//! millimetres explicitly). Numeric contract: every operation checks its
//! outputs for finiteness and refuses named; the degeneracy constants
//! are PROJECT CHOICES, not literature-backed tolerances (research 09 T5
//! found no universal replay tolerance in the literature). The evaluation
//! domain of `at` is the closed trim range t in [0,1].
//!
//! EMISSION DOES NOT LIVE HERE. No SolidWorks call shape is decided in
//! this module: research 09's release gate forbids claiming exactness
//! from `CreateConic` (its XApex semantics are unverified and the
//! official example is self-inconsistent under a classical reading)
//! until a seat probe proves a form by independent geometric readback.

use crate::error::{Error, Result};

/// Project-chosen degeneracy bound (not literature-backed): a control
/// triangle whose |sin(angle)| between its legs falls below this is
/// treated as collinear and refused.
const CONIC_COLLINEAR_EPS: f64 = 1e-12;

/// Project-chosen input floor: |U| or |V| at or below this many metres
/// makes the triangle degenerate regardless of angle.
const CONIC_SEGMENT_FLOOR_M: f64 = 1e-15;

/// The scale used for finiteness overflow guards.
const CONIC_FINITE_BOUND: f64 = 1e100;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConicSeg {
    pub p0: (f64, f64),
    pub q: (f64, f64),
    pub p2: (f64, f64),
    pub rho: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConicClass {
    Ellipse,
    Parabola,
    Hyperbola,
}

/// The implicit quadratic `G(X) = X^T H X + 2 g^T X + c` in the eq 2
/// normalization (see the module note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImplicitConic {
    pub h: [[f64; 2]; 2],
    pub g: [f64; 2],
    pub c: f64,
}

fn finite2(p: (f64, f64)) -> bool {
    p.0.is_finite()
        && p.1.is_finite()
        && p.0.abs() <= CONIC_FINITE_BOUND
        && p.1.abs() <= CONIC_FINITE_BOUND
}

fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

impl ConicSeg {
    /// Validate and construct. Fail-closed with named reasons: missing
    /// or non-finite input, rho outside (0,1), coincident endpoints, and
    /// an effectively collinear control triangle all refuse.
    pub fn new(p0: (f64, f64), q: (f64, f64), p2: (f64, f64), rho: f64) -> Result<Self> {
        if !(finite2(p0) && finite2(q) && finite2(p2)) {
            return Err(Error::InvalidRecipe(
                "conic: control points must be finite".to_string(),
            ));
        }
        if !rho.is_finite() || rho <= 0.0 || rho >= 1.0 {
            return Err(Error::InvalidRecipe(format!(
                "conic: rho {rho} is outside the open interval (0, 1)"
            )));
        }
        let u = sub(p2, p0);
        let v = sub(q, p0);
        let lu = (u.0 * u.0 + u.1 * u.1).sqrt();
        let lv = (v.0 * v.0 + v.1 * v.1).sqrt();
        if !lu.is_finite() || !lv.is_finite() {
            return Err(Error::InvalidRecipe(
                "conic: control-triangle leg lengths are not finite".to_string(),
            ));
        }
        if lu <= CONIC_SEGMENT_FLOOR_M {
            return Err(Error::InvalidRecipe(
                "conic: the endpoints coincide (zero chord)".to_string(),
            ));
        }
        if lv <= CONIC_SEGMENT_FLOOR_M {
            return Err(Error::InvalidRecipe(
                "conic: the apex coincides with the start point".to_string(),
            ));
        }
        let det = cross(u, v);
        if !det.is_finite() {
            return Err(Error::InvalidRecipe(
                "conic: the control-triangle determinant is not finite".to_string(),
            ));
        }
        if (det.abs() / (lu * lv)) < CONIC_COLLINEAR_EPS {
            return Err(Error::InvalidRecipe(
                "conic: the control triangle is effectively collinear".to_string(),
            ));
        }
        Ok(ConicSeg { p0, q, p2, rho })
    }

    pub fn weight(&self) -> f64 {
        self.rho / (1.0 - self.rho)
    }

    /// Classification by the Bezier weight (research 09 table): this is
    /// NOT the focal eccentricity e - rho, w and e stay separate
    /// quantities (conflict C7).
    pub fn class(&self) -> ConicClass {
        let w = self.weight();
        if (w - 1.0).abs() <= f64::EPSILON * 8.0 {
            ConicClass::Parabola
        } else if w < 1.0 {
            ConicClass::Ellipse
        } else {
            ConicClass::Hyperbola
        }
    }

    /// Exact rational quadratic evaluation on the closed trim range
    /// t in [0,1]; anything else refuses named.
    pub fn at(&self, t: f64) -> Result<(f64, f64)> {
        if !t.is_finite() || t < 0.0 || t > 1.0 {
            return Err(Error::InvalidRecipe(format!(
                "conic: evaluation parameter {t} is outside [0, 1]"
            )));
        }
        let w = self.weight();
        let one_minus = 1.0 - t;
        let l0 = one_minus * one_minus;
        let l1 = 2.0 * w * t * one_minus;
        let l2 = t * t;
        let d = l0 + l1 + l2;
        if !d.is_finite() || d <= 0.0 {
            return Err(Error::InvalidRecipe(
                "conic: the rational denominator is not finite and positive".to_string(),
            ));
        }
        let x = (l0 * self.p0.0 + l1 * self.q.0 + l2 * self.p2.0) / d;
        let y = (l0 * self.p0.1 + l1 * self.q.1 + l2 * self.p2.1) / d;
        if !(x.is_finite() && y.is_finite()) {
            return Err(Error::InvalidRecipe(
                "conic: the evaluated point is not finite".to_string(),
            ));
        }
        Ok((x, y))
    }

    /// The implicit form (research 09 eq 1/eq 2), checked for finite
    /// coefficients. The barycentric map is J = [U V] with U = P2-P0 and
    /// V = Q-P0; the construction refuses a non-invertible J.
    pub fn implicit(&self) -> Result<ImplicitConic> {
        let u = sub(self.p2, self.p0);
        let v = sub(self.q, self.p0);
        let det = cross(u, v);
        if !det.is_finite() || det.abs() <= CONIC_SEGMENT_FLOOR_M * CONIC_SEGMENT_FLOOR_M {
            return Err(Error::InvalidRecipe(
                "conic: the barycentric map is not invertible".to_string(),
            ));
        }
        // A = J^-1 = 1/det * [ V.y  -V.x ; -U.y  U.x ] (columns U, V)
        let a = [[v.1 / det, -v.0 / det], [-u.1 / det, u.0 / det]];
        // b = -A * P0
        let b = [
            -(a[0][0] * self.p0.0 + a[0][1] * self.p0.1),
            -(a[1][0] * self.p0.0 + a[1][1] * self.p0.1),
        ];
        let w2 = self.weight() * self.weight();
        // K = [ 4w^2   2w^2 ; 2w^2   1 ], f = (-2w^2, 0)
        let k = [[4.0 * w2, 2.0 * w2], [2.0 * w2, 1.0]];
        let f = [-2.0 * w2, 0.0];
        // H = A^T K A
        let ka = [
            [
                k[0][0] * a[0][0] + k[0][1] * a[1][0],
                k[0][0] * a[0][1] + k[0][1] * a[1][1],
            ],
            [
                k[1][0] * a[0][0] + k[1][1] * a[1][0],
                k[1][0] * a[0][1] + k[1][1] * a[1][1],
            ],
        ];
        let h = [
            [
                a[0][0] * ka[0][0] + a[1][0] * ka[1][0],
                a[0][0] * ka[0][1] + a[1][0] * ka[1][1],
            ],
            [
                a[0][1] * ka[0][0] + a[1][1] * ka[1][0],
                a[0][1] * ka[0][1] + a[1][1] * ka[1][1],
            ],
        ];
        // kb + f
        let kb = [
            ka[0][0] * b[0] + ka[0][1] * b[1],
            ka[1][0] * b[0] + ka[1][1] * b[1],
        ];
        let kbp_f = [kb[0] + f[0], kb[1] + f[1]];
        // g = A^T (K b + f)
        let g = [
            a[0][0] * kbp_f[0] + a[1][0] * kbp_f[1],
            a[0][1] * kbp_f[0] + a[1][1] * kbp_f[1],
        ];
        // c = b^T K b + 2 f^T b
        let c = b[0] * kb[0] + b[1] * kb[1] + 2.0 * (f[0] * b[0] + f[1] * b[1]);
        let all = [h[0][0], h[0][1], h[1][0], h[1][1], g[0], g[1], c];
        if !all.iter().all(|x| x.is_finite()) {
            return Err(Error::InvalidRecipe(
                "conic: the implicit coefficients are not finite".to_string(),
            ));
        }
        Ok(ImplicitConic { h, g, c })
    }
}

impl ImplicitConic {
    pub fn eval(&self, x: (f64, f64)) -> f64 {
        x.0 * (self.h[0][0] * x.0 + self.h[0][1] * x.1)
            + x.1 * (self.h[1][0] * x.0 + self.h[1][1] * x.1)
            + 2.0 * (self.g[0] * x.0 + self.g[1] * x.1)
            + self.c
    }

    /// Inverse recovery (research 09 eq 6): from a verified implicit
    /// conic and its exact endpoints, recover rho. Preconditions checked
    /// named: finite inputs, the endpoints lie on the conic (scale-aware
    /// against the coefficient magnitude), nonzero endpoint gradients
    /// (tangents defined), a conditioned tangent intersection, and a
    /// finite positive recovery ratio. The result is invariant under any
    /// nonzero uniform scaling of G (the ratio cancels it).
    pub fn rho_from_endpoints(&self, p0: (f64, f64), p2: (f64, f64)) -> Result<f64> {
        if !(finite2(p0) && finite2(p2)) {
            return Err(Error::InvalidRecipe(
                "conic: the endpoints must be finite".to_string(),
            ));
        }
        let scale = self.magnitude_scale();
        if !(scale.is_finite()) || scale <= 0.0 {
            return Err(Error::InvalidRecipe(
                "conic: the implicit scale is not finite and positive".to_string(),
            ));
        }
        let tol = CONIC_COLLINEAR_EPS.max(1e-12) * scale * 1e3;
        if self.eval(p0).abs() > tol || self.eval(p2).abs() > tol {
            return Err(Error::InvalidRecipe(
                "conic: an endpoint does not lie on the implicit conic".to_string(),
            ));
        }
        let g0 = self.grad(p0);
        let g2 = self.grad(p2);
        let lg0 = (g0.0 * g0.0 + g0.1 * g0.1).sqrt();
        let lg2 = (g2.0 * g2.0 + g2.1 * g2.1).sqrt();
        if !lg0.is_finite()
            || !lg2.is_finite()
            || lg0 <= CONIC_SEGMENT_FLOOR_M
            || lg2 <= CONIC_SEGMENT_FLOOR_M
        {
            return Err(Error::InvalidRecipe(
                "conic: an endpoint gradient vanishes (no tangent)".to_string(),
            ));
        }
        // Tangent lines: g0 . (X - p0) = 0 and g2 . (X - p2) = 0; their
        // intersection is the Bezier control point Q.
        let d = cross(g0, g2);
        if !d.is_finite() || d.abs() <= CONIC_COLLINEAR_EPS * lg0 * lg2 {
            return Err(Error::InvalidRecipe(
                "conic: the endpoint tangents are parallel (no apex)".to_string(),
            ));
        }
        // Solve g0.(Q-p0)=0 and g2.(Q-p2)=0:
        // [g0x g0y; g2x g2y] Q = [g0.p0; g2.p2]
        let r0 = g0.0 * p0.0 + g0.1 * p0.1;
        let r2 = g2.0 * p2.0 + g2.1 * p2.1;
        let q = ((r0 * g2.1 - r2 * g0.1) / d, (g0.0 * r2 - g2.0 * r0) / d);
        if !finite2(q) {
            return Err(Error::InvalidRecipe(
                "conic: the tangent intersection is not finite".to_string(),
            ));
        }
        let m = ((p0.0 + p2.0) / 2.0, (p0.1 + p2.1) / 2.0);
        let gm = self.eval(m);
        let gq = self.eval(q);
        if !gm.is_finite() || !gq.is_finite() || gq.abs() <= CONIC_COLLINEAR_EPS * scale {
            return Err(Error::InvalidRecipe(
                "conic: the recovery ratio is undefined".to_string(),
            ));
        }
        let w2 = -gm / gq;
        if !w2.is_finite() || w2 <= 0.0 {
            return Err(Error::InvalidRecipe(
                "conic: the recovery ratio is not finite and positive".to_string(),
            ));
        }
        let w = w2.sqrt();
        let rho = w / (1.0 + w);
        if !rho.is_finite() || rho <= 0.0 || rho >= 1.0 {
            return Err(Error::InvalidRecipe(
                "conic: the recovered rho is outside (0, 1)".to_string(),
            ));
        }
        Ok(rho)
    }

    fn grad(&self, x: (f64, f64)) -> (f64, f64) {
        (
            2.0 * (self.h[0][0] * x.0 + self.h[0][1] * x.1) + 2.0 * self.g[0],
            2.0 * (self.h[1][0] * x.0 + self.h[1][1] * x.1) + 2.0 * self.g[1],
        )
    }

    /// A magnitude scale for relative tolerances: the largest
    /// coefficient magnitude (finite-checked by construction).
    fn magnitude_scale(&self) -> f64 {
        let mut s = 0.0f64;
        for v in [
            self.h[0][0],
            self.h[0][1],
            self.h[1][0],
            self.h[1][1],
            self.g[0],
            self.g[1],
            self.c,
        ] {
            if v.abs() > s {
                s = v.abs();
            }
        }
        s
    }
}
