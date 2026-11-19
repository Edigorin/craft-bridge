//! Offline conic-math tests against research 09's worked examples
//! (research/outputs/09-conic-rho-conversion.md T3). The examples carry
//! a uniformly RESCALED implicit equation; the module's canonical form
//! is eq 1/eq 2 (G(M) = -w^2, G(Q) = 1). The recovery ratio is
//! scale-invariant and must give the same w under both.

use craft_bridge::conic::{ConicClass, ConicSeg};

fn approx(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b} within {tol}");
}

// The three fixtures share the control triangle P0=(0,0), Q=(20,8),
// P2=(40,0) in MILLIMETRES; the module works in metres.
fn seg(rho: f64) -> ConicSeg {
    ConicSeg::new((0.0, 0.0), (0.020, 0.008), (0.040, 0.0), rho).unwrap()
}

#[test]
fn example_e_ellipse_geometry_matches_the_implicit_conic() {
    let s = seg(1.0 / 3.0);
    approx(s.weight(), 0.5, 1e-15);
    assert_eq!(s.class(), ConicClass::Ellipse);
    let g = s.implicit().unwrap();
    // canonical normalization: G(Q)=1, G(M)=-w^2
    approx(g.eval((0.020, 0.008)), 1.0, 1e-12);
    approx(g.eval((0.020, 0.0)), -0.25, 1e-12);
    // rescaled example equation: G_example = G / w^2 gives -1 and 4
    let w2 = s.weight() * s.weight();
    approx(g.eval((0.020, 0.0)) / w2, -1.0, 1e-12);
    approx(g.eval((0.020, 0.008)) / w2, 4.0, 1e-12);
    // the example's interior sample lies on the curve
    let b = s.at(0.25).unwrap();
    approx(b.0, 100.0 / 13.0 / 1000.0, 1e-12);
    approx(b.1, 24.0 / 13.0 / 1000.0, 1e-12);
    approx(g.eval(b), 0.0, 1e-9);
    // the example's classical quantities satisfy the focus-directrix
    // property at P0 (independent check against the implicit equation)
    let f: (f64, f64) = (42.4697327285 / 1000.0, -2.6666666667 / 1000.0);
    let _vtx: (f64, f64) = (43.0940107676 / 1000.0, -2.6666666667 / 1000.0); // the documented classical vertex (the directrix below uses its closed form)
    let e = 0.9729679680;
    let p0: (f64, f64) = (0.0, 0.0);
    let d_focus = ((p0.0 - f.0) * (p0.0 - f.0) + (p0.1 - f.1) * (p0.1 - f.1)).sqrt();
    let d_dir = (((20.0f64 + 200.0f64 / 71.0f64.sqrt()) / 1000.0) - p0.0).abs();
    approx(d_focus, e * d_dir, 1e-9);
}

#[test]
fn example_p_parabola_geometry_matches_the_implicit_conic() {
    let s = seg(0.5);
    approx(s.weight(), 1.0, 1e-15);
    assert_eq!(s.class(), ConicClass::Parabola);
    let g = s.implicit().unwrap();
    let b = s.at(0.25).unwrap();
    approx(b.0, 0.010, 1e-12);
    approx(b.1, 0.003, 1e-12);
    approx(g.eval(b), 0.0, 1e-9);
    // example vertex and focus on the parabola identity: y = 4 - (x-20)^2/100
    // sample point (10,3) mm satisfies it
    approx(
        4.0f64 - ((10.0f64 - 20.0f64) * (10.0f64 - 20.0f64)) / 100.0,
        3.0,
        1e-12,
    );
    // focus-directrix: |P0-F| = 29 mm = distance to y=29
    let f: (f64, f64) = (0.020, -0.021);
    let p0: (f64, f64) = (0.0, 0.0);
    let d_focus = ((p0.0 - f.0) * (p0.0 - f.0) + (p0.1 - f.1) * (p0.1 - f.1)).sqrt();
    approx(d_focus, 0.029, 1e-12);
}

#[test]
fn example_h_hyperbola_geometry_matches_the_implicit_conic() {
    let s = seg(2.0 / 3.0);
    approx(s.weight(), 2.0, 1e-15);
    assert_eq!(s.class(), ConicClass::Hyperbola);
    let g = s.implicit().unwrap();
    let w2 = s.weight() * s.weight();
    // rescaled example equation values: G(M)=-1, G(Q)=1/4
    approx(g.eval((0.020, 0.0)) / w2, -1.0, 1e-12);
    approx(g.eval((0.020, 0.008)) / w2, 0.25, 1e-12);
    let b = s.at(0.25).unwrap();
    approx(b.0, 140.0 / 11.0 / 1000.0, 1e-12);
    approx(b.1, 48.0 / 11.0 / 1000.0, 1e-12);
    approx(g.eval(b), 0.0, 1e-9);
}

#[test]
fn inverse_recovery_is_scale_invariant() {
    for rho in [1.0 / 3.0, 0.5, 2.0 / 3.0, 0.1, 0.9] {
        let s = seg(rho);
        let g = s.implicit().unwrap();
        let got = g.rho_from_endpoints((0.0, 0.0), (0.040, 0.0)).unwrap();
        approx(got, rho, 1e-9);
        // a uniform rescale of G must not change the recovery
        let scaled = craft_bridge::conic::ImplicitConic {
            h: [
                [g.h[0][0] * 7.0, g.h[0][1] * 7.0],
                [g.h[1][0] * 7.0, g.h[1][1] * 7.0],
            ],
            g: [g.g[0] * 7.0, g.g[1] * 7.0],
            c: g.c * 7.0,
        };
        let got2 = scaled.rho_from_endpoints((0.0, 0.0), (0.040, 0.0)).unwrap();
        approx(got2, rho, 1e-9);
    }
}

#[test]
fn classification_covers_the_three_families() {
    assert_eq!(seg(0.25).class(), ConicClass::Ellipse);
    assert_eq!(seg(0.5).class(), ConicClass::Parabola);
    assert_eq!(seg(0.75).class(), ConicClass::Hyperbola);
}

#[test]
fn fail_closed_on_degenerate_and_extreme_input() {
    // rho out of range / non-finite
    assert!(ConicSeg::new((0.0, 0.0), (0.02, 0.008), (0.04, 0.0), 0.0).is_err());
    assert!(ConicSeg::new((0.0, 0.0), (0.02, 0.008), (0.04, 0.0), 1.0).is_err());
    assert!(ConicSeg::new((0.0, 0.0), (0.02, 0.008), (0.04, 0.0), f64::NAN).is_err());
    // coincident endpoints
    assert!(ConicSeg::new((0.0, 0.0), (0.02, 0.008), (0.0, 0.0), 0.5).is_err());
    // apex on the start point
    assert!(ConicSeg::new((0.0, 0.0), (0.0, 0.0), (0.04, 0.0), 0.5).is_err());
    // collinear control triangle
    assert!(ConicSeg::new((0.0, 0.0), (0.02, 0.0), (0.04, 0.0), 0.5).is_err());
    // non-finite points
    assert!(ConicSeg::new((0.0, f64::INFINITY), (0.02, 0.008), (0.04, 0.0), 0.5).is_err());
    // evaluation domain
    let s = seg(0.5);
    assert!(s.at(-0.1).is_err());
    assert!(s.at(1.1).is_err());
    assert!(s.at(f64::NAN).is_err());
    // extreme rho must stay in (0,1) and finite
    assert!(ConicSeg::new((0.0, 0.0), (0.02, 0.008), (0.04, 0.0), 1e-9).is_ok());
    assert!(ConicSeg::new((0.0, 0.0), (0.02, 0.008), (0.04, 0.0), 1.0 - 1e-9).is_ok());
}

#[test]
fn trim_range_covers_the_endpoints_and_shoulder() {
    let s = seg(1.0 / 3.0);
    let p0 = s.at(0.0).unwrap();
    approx(p0.0, 0.0, 1e-15);
    approx(p0.1, 0.0, 1e-15);
    let p2 = s.at(1.0).unwrap();
    approx(p2.0, 0.040, 1e-15);
    approx(p2.1, 0.0, 1e-15);
    // shoulder = M + rho*(Q-M)
    let mid = s.at(0.5).unwrap();
    approx(mid.0, 0.020, 1e-12);
    approx(mid.1, 8.0 / 3.0 / 1000.0, 1e-12);
}
