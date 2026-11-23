# Conic representations and rho conversion 09

**Filed:** 2026-11-18, produced by the owner's research agent, delivered
as a chat attachment and filed verbatim. Answer to
`research/inputs/09-conic-rho-conversion.md`.
Provenance note: the report's labels are its own (Recorded/Community/
Reconstructed/Conflicting/Unverified). Its headline corrections reshape
the 854-conic plan: SolidCraft's `apex` is the TANGENT INTERSECTION (not
a curve point, not the classical vertex), rho = shoulder-height ratio,
w = rho/(1-rho) gives an exact rational quadratic Bezier, and -
critically - the official CreateConic example's own coordinates are
inconsistent with a naive classical focus/vertex interpretation (the two
endpoints imply different eccentricities), so XApex semantics stay
UNVERIFIED and the release gate forbids claiming exactness from a
successful call. The worked examples E/P/H give exact offline test
fixtures for the replay math.

---

# Research 09: conic representations and rho conversion

**Research date:** 18 November 2026  
**Project:** `craft-bridge`, SOLIDWORKS 2026 replay of native `solidcraft/57` sketches  
**Scope:** Research brief `09-conic-rho-conversion.md`, with cross-checks against supplied reports 06/07 and the SolidCraft v0.43.0 schema studies.  
**Execution:** Public documentation and independent symbolic/numeric checks. **No licensed SOLIDWORKS seat test.**  
**Trademark:** SOLIDWORKS is a Dassault Systemes trademark. This research implies no affiliation.

## Evidence labels

- **Recorded:** SOLIDWORKS Help, API Help, a published official example, or a source-confirmed value in the supplied research.
- **Community:** Published external implementation or mathematical discussion. It does not prove SOLIDWORKS behavior.
- **Reconstructed / numerically checked:** Derived here, with formulas checked by symbolic substitution and computed numbers. An exact mathematical identity does **not** establish the corresponding SOLIDWORKS COM behavior.
- **Conflicting:** Two documents differ, or a brief assumption does not match an examined source.
- **Unverified:** No authoritative public statement or seat result establishes the proposed behavior.

## Findings at a glance

| Topic | Finding | Status | Effect on the replay emitter |
|---|---|---|---|
| SOLIDWORKS sketch conic UI | The **top vertex** is the endpoint-tangent intersection; the **shoulder** lies on the curve. UI `Rho` is shoulder height divided by top-vertex height. | **Recorded** [SW1] | Do not treat the top vertex as a point on the conic. |
| SolidCraft conic | Native conic: `type:"conic", a, b, apex, rho`. `apex` indexes the tangent intersection. `rho` is numeric, dimensionless. | **Recorded (supplied source research)** [SC1][SC2] | Read the indexed sketch points, not inline coordinate fields. |
| Weight conversion | `w = rho/(1-rho)`; `rho = w/(1+w)`. A three-control-point rational quadratic Bezier curve represents the conic exactly. | **Reconstructed / checked**, supported by supplied SolidCraft source and mathematical literature [SC2][M1] | Exact canonical geometry is available without guessing a focus. |
| Shape classification | `0<rho<0.5`: ellipse; `rho=0.5`: parabola; `0.5<rho<1`: hyperbola. Circle needs an additional geometric condition. | **Recorded** for SolidCraft; **mathematically checked** [SC2][M1] | Rho is *not* the conic's focus/directrix eccentricity `e`. |
| T3 three-point example | Endpoints `(0,0),(40,0)` and tangent intersection `(20,8)` **do not determine rho**. | **Reconstructed / checked** | Do not manufacture an inferred value. The ellipse example below explicitly chooses `rho=1/3`. |
| `CreateConic` contract | `CreateConic` takes **12 doubles**: `F=(XFocus,YFocus,ZFocus)`, `A=(XApex,YApex,ZApex)`, start XYZ, end XYZ. No rho input. | **Recorded** [SW3][SW4] | A direct SolidCraft `apex` to COM `XApex` mapping is not established. |
| Classical conic conversion | Control polygon + weight uniquely defines a nondegenerate conic. Its implicit equation gives the center, foci, principal vertices and directrices. | **Reconstructed / numerically checked** [M1][M2] | Mathematical conversion can be implemented. COM parameter interpretation still needs a seat probe. |
| Exact geometric fallback | SOLIDWORKS accepts rational B-spline input with four-dimensional weighted control points and order 3. An exact three-control-point candidate exists. | **Recorded** for API capabilities; **unverified** for the proposed seat call [SW5][SW6] | Potentially preserve exact curve geometry as a **spline**, not a native conic sketch entity. |
| Weighted-array conflict | `ISplineParamData.SetControlPoints` requires **homogeneous** `(wx,wy,wz,w)` for an open dimension-4 spline; the older one-argument API page says `(x,y,z,w)` without this qualification. | **Conflicting** [SW5][SW6] | Test the two APIs separately. Do not reuse one encoding blindly. |
| Exactness requirement | `CreateConic` documentation does not specify which classical vertex its `XApex` denotes or how the endpoint pair selects the trimmed branch. | **Unverified** [SW3] | Never mark a generated curve as exact from a successful return alone. |

---

## T1. Rho, the top vertex, the shoulder and the focus

### 1.1 What SOLIDWORKS documents

**Recorded [SW1][SW2].** In the sketch **Conic PropertyManager**:

- **Top vertex** = the intersection of the tangent lines at the two conic endpoints. It is generally **not** a point on the curve.
- **Shoulder** = a point on the curve used to define its height from the chord joining its endpoints.
- **Rho** = the ratio of the shoulder height to the top-vertex height. The heights use the same endpoint chord as their reference.
- The UI sequence is: select endpoints, place the top vertex, set Rho. The top-vertex and shoulder reference points are shown separately.

The UI calls the tangent intersection a *vertex*. In classical conic geometry, a *principal vertex* is an endpoint of a principal axis **on** the conic. These are different definitions. For an asymmetric segment, the shoulder is not generally a principal vertex either.

**Recorded [SW3].** `ISketchManager.CreateConic` labels its second XYZ triplet `XApex, YApex, ZApex` and calls it the conic's *apex*. Its public entry does not establish whether *apex* means UI top vertex, a classical principal vertex, a parabola vertex, or another kernel parameter. Treat the intended meaning as **unverified**.

### 1.2 SolidCraft's exact meaning

**Recorded (supplied, source-pinned) [SC1][SC2].** For a SolidCraft conic:

- `P0` is the sketch point at index `a`.
- `P1` (called `Q` below) is the point at index `apex` and is the intersection of endpoint tangents.
- `P2` is the sketch point at index `b`.
- `rho` is a dimensionless `f64`, with `0<rho<1` in the native conic. MCP input restricts it further to `(1e-6, 1-1e-6)`.
- Source point coordinates are in **millimetres** and must be transformed from the sketch frame to the SOLIDWORKS sketch frame if necessary. Geometry API lengths are in **metres**.

The supplied SolidCraft source study gives

    w = rho / (1 - rho)

and evaluates the rational quadratic Bezier curve

    B(t) = [ (1-t)^2 P0 + 2w t(1-t) Q + t^2 P2 ]
           / [ (1-t)^2 + 2w t(1-t) + t^2 ],  0 <= t <= 1.

The endpoints are `B(0)=P0`, `B(1)=P2`. Its endpoint tangents point along `Q-P0` and `P2-Q`. Therefore, the tangent lines intersect at `Q` when the three points are not collinear.

At `t=1/2`,

    M = (P0 + P2)/2
    S = B(1/2) = M + rho * (Q-M).

The normal height of `S` above the chord divided by the normal height of `Q` is exactly `rho`. Thus SolidCraft's ratio is mathematically compatible with the SOLIDWORKS UI's **documented height-ratio definition**. This does **not** establish that `CreateConic` uses the same internal parameterization.

### 1.3 Classification and different rho conventions

**Reconstructed / checked [M1][M2].** With positive finite weight `w` and non-collinear control points:

| Bezier weight | SolidCraft rho | Conic class | Focal eccentricity |
|---|---|---|---|
| `0<w<1` | `0<rho<0.5` | Ellipse, including the special circle case | `0<=e<1` |
| `w=1` | `rho=0.5` | Parabola | `e=1` |
| `w>1` | `0.5<rho<1` | Hyperbola | `e>1` |

**Do not substitute `e` for `rho`.** The focal eccentricity `e` depends on both the weight and the control triangle. Likewise, the classic circular-arc identity `w=cos(theta/2)` is a special weight formula for a suitably placed circular-arc control polygon. It is **not** the definition `rho=cos(theta/2)`.

For a quarter-circle segment, `w=1/sqrt(2)` and `rho=w/(1+w)=sqrt(2)-1`, approximately `0.4142135624`. An independent CAD application (Creo) also documents `sqrt(2)-1` for a quarter-ellipse/circular-arc construction and places its parabola threshold at `rho=0.5` [PTC1]. This is comparative evidence, **not** a guarantee about the SOLIDWORKS COM method.

**Range distinction.** SOLIDWORKS Help explicitly restricts certain **fillet-profile rho** properties to `[0.05,0.95]` [SW7]. I found **no equivalent numerical clamp documented for the sketch Conic tool** in the examined SOLIDWORKS product pages [SW1][SW2]. Do not apply the fillet limit to sketches without a seat probe. The basic nondegenerate mathematical interval is `0<rho<1`.

---

## T2. Exact mathematical construction

### 2.1 Convert control points + rho to an implicit conic

**Reconstructed / symbolically and numerically checked [M1].** Use two-dimensional coordinates in the sketch's orthonormal frame. Define

    P0 = start point
    Q  = tangent intersection (SolidCraft `apex`)
    P2 = end point
    w  = rho/(1-rho)
    U  = P2-P0
    V  = Q-P0
    J  = [U V]  (2 by 2 matrix with U,V as columns).

Reject if `det(J)=0` within a conditioned, scale-aware collinearity check. For an arbitrary point `X`, calculate

    (r,s)^T = J^(-1) * (X-P0).

The three barycentric coordinates of `X` relative to `(P0,Q,P2)` are

    lambda0 = 1-r-s
    lambda1 = s
    lambda2 = r.

The exact conic equation is

    G(X) = s^2 - 4*w^2 * r*(1-r-s) = 0.                  (1)

Proof: on the rational curve, `lambda0=(1-t)^2/D`, `lambda1=2wt(1-t)/D`, and `lambda2=t^2/D`. Then `lambda1^2 = 4w^2 lambda0 lambda2` identically. This proves the full implicit conic equation, not only a sample-point fit.

For numerical implementation, form the implicit coefficients without a general conic-fit solver. Set

    K = [ 4w^2   2w^2 ]
        [ 2w^2     1  ]
    f = (-2w^2, 0)^T
    A = J^(-1)
    b = -A*P0.

Since `z=(r,s)^T=A*X+b`, the implicit form is

    G(X) = X^T H X + 2*g^T X + c
    H = A^T K A
    g = A^T (K b + f)
    c = b^T K b + 2 f^T b.                               (2)

Scale `X` and the coefficients when possible. Do not make a conic-type decision from a rounded global discriminant alone if the serialized `rho` is available.

### 2.2 Recover the center, vertices, foci and directrices

**Reconstructed / checked [M2][M3].** For an ellipse or hyperbola, `H` is nonsingular. Set

    C = -H^(-1)*g
    qC = G(C)
    H*n_i = lambda_i*n_i,  i=1,2,

with orthonormal eigenvectors `n_i`. Then

    lambda1*z1^2 + lambda2*z2^2 = -qC,
    where X=C+z1*n1+z2*n2.

**Ellipse (`rho<0.5`).** Both eigenvalues have the same sign. The principal semiaxis lengths satisfy

    a_i^2 = -qC/lambda_i.

Let `a>=b>0` be the major and minor semiaxes. The two principal vertices are `C +/- a*n_major`. The foci are `C +/- d*n_major`, where

    d = sqrt(a^2-b^2)
    e = d/a.

For the focus with a `+` sign, the associated directrix is the line

    n_major dot (X-C) = a/e  (= a^2/d),                 (3)

when `e>0`. Use corresponding negative signs for the opposite focus. For a circle, `a=b`, `d=0`, `e=0`, and neither a unique major direction nor a finite focus-directrix line exists. Do not pass an arbitrary ellipse-axis choice to COM as if uniquely determined.

**Hyperbola (`rho>0.5`).** One eigenvalue has the sign of `-qC` and determines the transverse axis `n_t` and `a>0`. The other has the opposite sign and determines `b>0`:

    a^2 = -qC/lambda_t
    b^2 = abs(qC/lambda_other)
    d = sqrt(a^2+b^2)
    e = d/a.

The vertices are `C +/- a*n_t`. The foci are `C +/- d*n_t`. The directrices are `n_t dot (X-C) = +/-a/e`. Choose the vertex/focus on the branch that contains the Bezier segment. That branch can be identified by `B(1/2)`.

**Parabola (`rho=0.5`).** The symmetric matrix `H` has rank one. Let `n` be its unit eigenvector with nonzero eigenvalue `lambda`, and let `t` be a unit null eigenvector. Write `g_n=g dot n`, `g_t=g dot t`, with `g_t!=0`. In coordinates `X=eta*n+xi*t`,

    G = lambda*eta^2 + 2*g_n*eta + 2*g_t*xi + c = 0.

The vertex, signed focal distance and focus are

    eta0 = -g_n/lambda
    xi0  = (g_n^2/lambda - c)/(2*g_t)
    Vtx  = eta0*n + xi0*t
    p    = -g_t/(2*lambda)
    F    = Vtx + p*t
    directrix: t dot (X-Vtx) = -p.                       (4)

Check that `F` is on the axis and that `|X-F|` equals distance to the directrix for the source samples. The direction of `t` may reverse, but `p*t` and the directrix geometry must remain invariant.

**Focus versus top vertex.** The classical foci and principal vertices recovered here are geometric quantities derived from the curve. The SolidCraft `Q` is the tangent intersection. Do not call `Q` the classical focus or principal vertex.

### 2.3 Can these values be passed directly into `CreateConic`?

**Unverified [SW3][SW4].** A candidate call is

```vb
' CANDIDATE ONLY. F and Vtx are derived classical quantities.
' Do not deploy until the SOLIDWORKS 2026 seat proves this interpretation.
Dim seg As SldWorks.SketchSegment
Set seg = swModel.SketchManager.CreateConic( _
    Fx, Fy, Fz, Vx, Vy, Vz, _
    P0x, P0y, P0z, P2x, P2y, P2z)
```

The *signature* is documented. The mapping **classical principal vertex -> `XApex`** is **not**. For ellipses and hyperbolas, there are two choices of focus and principal vertex. For an ellipse, either of the two arcs joining the given endpoints can lie on the same underlying conic. The source's rational Bezier `B(t)` identifies the correct one, but the API does not document how it chooses a branch. A created segment must therefore pass independent curve, tangent, and trim checks before acceptance.

### 2.4 An inconsistency in the published COM example

**Reconstructed / numerically checked against recorded numbers [SW4].** The official sample calls `CreateConic` with

    F  = (0.109436, 0.080614) m
    A  = (0.048742, 0.022907) m
    P0 = (-0.017812, 0) m
    P2 = (-0.006092, -0.069601) m.

If `F` is a classical focus and `A` is a classical principal vertex, define `d=|F-A|` and `n=(F-A)/d`. Each point `P` on the same focus-directrix conic must satisfy

    |P-F| = |e * [n dot (P-A)] + d|.                    (5)

The two endpoints independently imply positive eccentricities of approximately **3.6612781** and **2.6405816**. They do not agree. The six-decimal input rounding is too small to explain the difference.

This test **does not prove that the official example fails**. It shows that simply interpreting the listed focus and apex as one ordinary focus/vertex pair is not consistent with the published sample coordinates. Possible undocumented parameter conventions, shape construction, or treatment of the endpoints need live inspection. This raises the priority of a seat probe: **do not infer the COM geometry from parameter names alone**.

### 2.5 Inverse construction: recover rho from a *verified* conic

**Reconstructed / checked.** Suppose an actual conic equation `G(X)=0` and the exact source start/end points `P0,P2` are known. Calculate the tangent lines at the two endpoints:

    grad G(P) dot (X-P) = 0,
    grad G(X) = 2*H*X + 2*g.

Their intersection gives the Bezier control point `Q`, provided the tangents are not parallel. Let `M=(P0+P2)/2`. For an implicit quadratic `G` with any nonzero uniform scaling,

    w^2 = -G(M)/G(Q),
    w = sqrt(-G(M)/G(Q)),
    rho = w/(1+w).                                      (6)

Choose `w>0`. Reject negative ratios, undefined/tiny denominators, or nearly parallel endpoint tangents. Derivation: in barycentric coordinates, `G` is proportional to `lambda1^2 -4w^2 lambda0 lambda2`; at `Q`, this is `1`, and at `M` it is `-w^2`.

If `F,Vtx` are supplied instead of `G`, first construct a focus-directrix conic **after** resolving the focal axis, directrix side, eccentricity and branch. For example, for `d=|F-Vtx|>0` and `n=(F-Vtx)/d`, the candidate implicit equation is

    G(X) = |X-F|^2 - [e*n dot (X-Vtx)+d]^2 = 0.          (7)

Determine a **single** positive `e` that fits both endpoints. There can be sign/branch choices, and `d=0` makes this construction singular for circles. If no consistent `e` exists, stop. Formula (7) is a mathematical construction, **not** an assertion about the internal SOLIDWORKS API.

### 2.6 Exact rational B-spline alternative

**Recorded API capability; untested replay call [SW5][SW6][SW8].** A rational degree-2 Bezier segment is also a rational B-spline with:

    order = 3                  (degree = 2)
    number_of_controls = 3
    dimension = 4              (x,y,z,weight)
    periodic = False
    knots = [0,0,0,1,1,1]
    weights = [1,w,1].

For the **open/nonperiodic** `ISplineParamData.SetControlPoints` route, the documented control array stores **homogeneous** coordinates:

    [P0x, P0y, P0z, 1,
     w*Qx, w*Qy, w*Qz, w,
     P2x, P2y, P2z, 1].

This is **not** the same literal array as `[P0x,P0y,P0z,1, Qx,Qy,Qz,w, P2x,P2y,P2z,1]`. The second array is the unweighted-coordinate form described without qualification in the older `CreateSplineByEqnParams` page. Verify the chosen interface's format on the seat.

Proposed VBA seat probe using the documented `ISplineParamData` builder:

```vb
' RECONSTRUCTED / UNTESTED VBA. Ellipse fixture, rho=1/3, w=1/2.
' Coordinates are sketch-space metres.
Dim d As SldWorks.SplineParamData
Dim k(0 To 5) As Double
Dim cp(0 To 11) As Double
Dim created As Variant
Dim ok As Boolean

k(0) = 0#: k(1) = 0#: k(2) = 0#
k(3) = 1#: k(4) = 1#: k(5) = 1#

' P0 = (0,0), Q=(0.020,0.008), P2=(0.040,0) metres.
' Open rational spline: homogeneous control-point array.
cp(0) = 0#: cp(1) = 0#: cp(2) = 0#: cp(3) = 1#
cp(4) = 0.01: cp(5) = 0.004: cp(6) = 0#: cp(7) = 0.5
cp(8) = 0.04: cp(9) = 0#: cp(10) = 0#: cp(11) = 1#

Set d = swModel.SketchManager.CreateSplineParamData
With d
    .Dimension = 4
    .Order = 3
    .ControlPointsCount = 3
    .Periodic = False
End With
ok = d.SetKnotPoints(k)
If Not ok Then Err.Raise 5, , "SetKnotPoints failed"
ok = d.SetControlPoints(cp)
If Not ok Then Err.Raise 5, , "SetControlPoints failed"
created = swModel.SketchManager.CreateSplinesByEqnParams2(d)
' The seat probe must inspect the Variant, segment count,
' control-point readback, curve samples, and trim range.
```

Method return and `SplineParamData` class binding are drawn from published signatures; confirm the exact 2026 VBA type binding and marshaling. If either setter's return type differs on the seat, update the macro to use the installed type library. `CreateSplineByEqnParams(Array(...))` is a second probe with **distinct** packed-array and weight semantics [SW6].

This route can produce **mathematically exact conic geometry** if the seat preserves the weighted control points. It does **not** preserve the source's native **conic sketch-entity type**, Rho dimension, or necessarily its constraint behavior. Record `entity_kind="spline"` and `geometry_exact=true` only after checks pass. If native entity identity is required, this is not a substitute.

---

## T3. Three worked examples, including intermediate quantities

All example input values are in **millimetres**, and all numerical results in this section use millimetres. Convert all XYZ values to **metres** for a COM call. All examples use

    P0 = (0,0),  P2 = (40,0),  Q = (20,8).
    M = (20,0), chord half-width a=20, control height h=8.

For this symmetric triangle, let `x'=x-20` and `w=rho/(1-rho)`. Equation (1) simplifies to

    x'^2/400 + [(1-w^2)/w^2]*y^2/64 + y/4 - 1 = 0.   (8)

The tangent at `P0` lies on the line `P0-Q`, and the tangent at `P2` lies on `P2-Q`. Their intersection is always `Q=(20,8)`. The source shoulder is `S=B(1/2)=(20,8*rho)`.

### Example E: ellipse

**Input correction:** `P0`, `P2` and `Q` alone do **not** determine rho. For example, `rho=1/3`, `1/2` or `2/3` all use the same three points but yield different conics. The brief's request to *recover* an ellipse rho from those three points has no unique mathematical answer.

**Reconstructed / exactly checked.** Select `rho=1/3`. Then:

    w = (1/3)/(2/3) = 1/2
    Q = (20,8), M=(20,0)
    S = M + (1/3)*(Q-M) = (20,8/3)
    height ratio = (8/3)/8 = 1/3
    G(x,y) = (x-20)^2/400 + 3*y^2/64 + y/4 -1.

Complete the squares:

    (x-20)^2/(1600/3) + (y+8/3)^2/(256/9) = 1.

Thus:

    center C = (20,-8/3) = (20,-2.6666666667)
    major semiaxis a = 40/sqrt(3) = 23.0940107676 (horizontal)
    minor semiaxis b = 16/3 = 5.3333333333 (vertical)
    focal distance d = sqrt(a^2-b^2) = 8*sqrt(71)/3 = 22.4697327285
    focal eccentricity e = d/a = sqrt(213)/15 = 0.9729679680.

Choose the right-hand focus and principal vertex for one **candidate** COM tuple:

    F = (20+8*sqrt(71)/3, -8/3)
      = (42.4697327285, -2.6666666667)
    Vtx = (20+40/sqrt(3), -8/3)
        = (43.0940107676, -2.6666666667)
    right directrix: x = 20 + a^2/d
                       = 20 + 200/sqrt(71)
                       = 43.7356331639.

Check `P0`: its directrix distance is `43.7356331639`. Its focus distance is approximately `42.5534`, which equals `e` times the directrix distance. The same relation holds for `P2` and `S`. The left-hand focus and vertex are mathematically equivalent alternatives, but **not necessarily COM-equivalent**.

**Important:** The source segment between `P0` and `P2` passes through its upper shoulder `(20,8/3)`. It does *not* pass through the right-hand major-axis vertex. This makes a direct `XApex=Vtx` interpretation particularly uncertain.

One interior source sample is

    B(1/4) = (100/13, 24/13)
           = (7.6923076923, 1.8461538462).

For inverse verification using `G`, `G(M)=-1`, `G(Q)=4`, hence `w^2=-G(M)/G(Q)=1/4` and `rho=1/3`.

**Candidate COM tuple in metres:**

    F   = (0.0424697327285, -0.0026666666667, 0)
    Vtx = (0.0430940107676, -0.0026666666667, 0)
    P0  = (0,0,0)
    P2  = (0.040,0,0).

**Unverified:** `XApex` interpretation and correct trimmed arc.

### Example P: parabola

**Reconstructed / exactly checked.** Select `rho=1/2`, hence `w=1`:

    Q=(20,8), M=(20,0), S=(20,4)
    height ratio = 4/8 = 1/2
    G(x,y) = (x-20)^2/400 + y/4 -1 = 0
    y = 4 - (x-20)^2/100.

Write the parabola as `x'^2=4p*(y-4)`. Then:

    4p=-100, p=-25
    classical vertex Vtx=(20,4)
    focus F=(20,4-25)=(20,-21)
    directrix y=4-(-25)=29
    eccentricity e=1.

Endpoint check:

    |P0-F| = sqrt(20^2+21^2) = 29
    distance(P0, directrix y=29) = 29.

The same equality holds at `P2`. Interior sample `B(1/4)=(10,3)` satisfies `y=4-(10-20)^2/100=3`.

The tangent intersection remains `Q=(20,8)`, **not** `Vtx=(20,4)`. For inverse verification, `G(M)=-1`, `G(Q)=1`, so `w=1` and `rho=1/2`.

**Candidate COM tuple in metres:**

    F=(0.020,-0.021,0)
    Vtx=(0.020,0.004,0)
    P0=(0,0,0)
    P2=(0.040,0,0).

**Unverified:** The COM method's use of `XApex`. A seat test of `CreateParabola` and `CreateConic` with these same values is useful.

### Example H: hyperbola

**Reconstructed / exactly checked.** Select `rho=2/3`, hence `w=2`:

    Q=(20,8), M=(20,0), S=(20,16/3)
    height ratio = (16/3)/8 = 2/3
    G(x,y) = (x-20)^2/400 - 3*y^2/256 + y/4 -1.

Complete the squares:

    (y-32/3)^2/(256/9) - (x-20)^2/(400/3) = 1.

Thus:

    center C=(20,32/3)=(20,10.6666666667)
    transverse semiaxis a=16/3=5.3333333333 (vertical)
    conjugate semiaxis b=20/sqrt(3)=11.5470053838
    focal distance d=sqrt(a^2+b^2)=4*sqrt(91)/3=12.7191893522
    eccentricity e=d/a=sqrt(91)/4=2.3848480035.

The source segment belongs to the **lower branch** because its shoulder is below the center. Use the lower classical vertex and focus:

    Vtx=(20,32/3-16/3)=(20,16/3)=(20,5.3333333333)
    F=(20,32/3-4*sqrt(91)/3)
     =(20,-2.0525226856)
    lower directrix y=32/3 - (16/3)^2/(4*sqrt(91)/3)
                      =32/3-64/(3*sqrt(91))
                      =8.4303256817.

For either endpoint, directrix distance is `8.4303256817`. Multiplying by `e` gives the distance to the lower focus (approximately `20.105`). Interior sample:

    B(1/4)=(140/11,48/11)
          =(12.7272727273,4.3636363636).

It satisfies the hyperbola equation. Inverse verification gives `G(M)=-1`, `G(Q)=1/4`, `w^2=4`, and `rho=2/3`.

**Candidate COM tuple in metres:**

    F=(0.020,-0.0020525226856,0)
    Vtx=(0.020,0.0053333333333,0)
    P0=(0,0,0)
    P2=(0.040,0,0).

**Unverified:** Native COM hyperbola construction, apex semantics, and lower-branch trimming.

### Example summary and independent checks

| Field | E: ellipse | P: parabola | H: hyperbola |
|---|---:|---:|---:|
| `rho` | `1/3` | `1/2` | `2/3` |
| `w` | `1/2` | `1` | `2` |
| Tangent intersection `Q` | `(20,8)` | `(20,8)` | `(20,8)` |
| Shoulder `S` | `(20,8/3)` | `(20,4)` | `(20,16/3)` |
| `G(M)` | `-1` | `-1` | `-1` |
| `G(Q)` | `4` | `1` | `1/4` |
| Inverse `-G(M)/G(Q)` | `1/4` | `1` | `4` |
| Source `B(1/4)` | `(100/13,24/13)` | `(10,3)` | `(140/11,48/11)` |
| Eccentricity `e` | `sqrt(213)/15` | `1` | `sqrt(91)/4` |

These equations and sample substitutions were independently checked with symbolic rational arithmetic. They establish the math, **not** an observed SOLIDWORKS result.

---

## T4. SolidCraft native storage and replay inputs

**Recorded (source-supplied) [SC1][SC2].** A native `solidcraft/57` sketch holds indexed 2D points and flattened curve-kind records. A representative conic record is:

```json
{
  "id": "conic42",
  "type": "conic",
  "a": 1,
  "b": 2,
  "apex": 3,
  "rho": 0.5
}
```

The indices in this sample are illustrative, not actual R1 design identifiers. The actual point records have `id` and `pos:[x,y]` in millimetres. A `Curve` also has wrapper fields such as `construction`, `reversed`, `centerline`, `fixed`, `infinite`, and `link`. These fields may be defaulted or omitted depending on serializer settings. The sketch's fixed origin usually occupies `points[0]`.

A conic record does **not** store a focus, directrix, focal eccentricity, classical vertex, equation coefficients, or explicit NURBS control-point weights. Its `apex` field is a **point index**, not an inline XYZ vector. The exact curve is recovered from the three indexed points and scalar rho.

Validation before replay:

1. Resolve `a`, `b`, and `apex` against the **same sketch's** `points[]`. Reject missing or invalid indices.
2. Evaluate any parameter expressions and constraints that can change point positions. Use evaluated coordinates, not stale pre-solve positions.
3. Check all coordinates and rho for finite values. Check native `0<rho<1` and note that MCP's tighter limits are command-input validation, not necessarily native file rejection rules.
4. Check distinct endpoints and non-collinear control triangle. Preserve source curve direction and `reversed` wrapper behavior if it applies to conics.
5. Convert the sketch's 2D frame to the chosen SOLIDWORKS sketch coordinate system. Convert millimetres to metres exactly at the API boundary.
6. Record source curve ID, feature ID, rho, indexed points, transform and expected sample coordinates. These form the verifier contract.

The source reports describe serializer and curve-evaluator behavior at SolidCraft tag `v0.43.0`; they were not runtime export tests. No R1 source design was supplied to this research pass, so the actual 854 records and their distributions remain uninspected.

---

## T5. Numerical robustness and tolerances

**Reconstructed guidance informed by geometric-numerics literature [M4].**

| Case | Numerical issue | Recommended fail-closed treatment |
|---|---|---|
| `Q` near the endpoint chord | `det(J)` approaches zero. The barycentric transform becomes ill-conditioned. | Apply a scale-aware area test, and use an adaptive orientation predicate if necessary. Stop on an effectively collinear triangle. |
| `rho` near `0` | `w=rho/(1-rho)` approaches zero. The curve approaches the chord and `G(Q)` scaling becomes fragile. | Preserve the exact source rho and use the rational parametric form for comparisons. Avoid normalizing by tiny quantities. |
| `rho` near `1` | `w` grows, and the curve approaches the control vertex. Floating-point magnitudes increase. | Compute `w` carefully, normalize coefficients, and reject overflow or loss of significance. |
| `rho` near `0.5` | For central-conic formulas, `H` is close to singular and the center/focal geometry may become very distant. | Use the parabola formula only for an actual parabola. Do not silently reclassify a nearby ellipse or hyperbola. Prefer the rational-spline route if COM requires unstable focal inputs. |
| Circle or near-circle | Principal eigenvalues coincide or nearly coincide. Axis and focus direction become unstable. | Detect axis ambiguity. Do not claim a unique classical vertex. A rational spline retains the exact source arc without choosing a major axis. |
| Non-symmetric/rotated conic | Choice of local axes and branch can change signs; angle and coordinate conversions can flip orientation. | Use matrix operations in an orthonormal sketch frame. Compare ordered endpoints, samples and tangents after applying the actual sketch transform. |
| Multiple 854-segment replay | A small error or one wrong branch can invalidate a later closed profile. | Record a per-curve verdict. Reject the enclosing profile when any required segment fails geometric verification. |

Use double precision for the mathematical conversion. Shewchuk's work documents why near-zero geometric determinants require careful predicates [M4]. No examined literature supplies a universal SOLIDWORKS replay tolerance. **Do not report a literature-backed numerical tolerance that was not found.**

For testing, propose `position_error <= max(abs_tol_m, rel_tol*model_scale_m)`, with separately configured tangent-angle and curve-distance tolerances. The project must choose `abs_tol_m` and `rel_tol` based on part scale, SOLIDWORKS output precision and downstream geometry needs. A test tolerance is not proof of mathematical exactness.

For an emitted curve, evaluate the source Bezier at `t=0,1/8,1/4,3/8,1/2,5/8,3/4,7/8,1`. Transform each point to model space. Use the returned SOLIDWORKS segment's underlying curve and `ICurve.GetClosestPointOn` to check the nearest distance **and** check that the parameter lies on the desired trimmed segment. `ICurve.Evaluate2` supports point/derivative readback [SW9]. Also compare endpoint derivatives, tangent intersection, source rho, conic type and branch. More samples can help detect gross error, but a finite sample alone cannot prove equal curves. A successful native entity plus consistent analytic/spline readback is stronger evidence.

---

## Conflicts and corrections to brief 09 / supporting research

| ID | Sources or claims | Conflict or missing premise | Resolution |
|---|---|---|---|
| C1 | Brief 09 T2 calls `apex A` a point **on** the curve; supplied SolidCraft schema calls `apex` the tangent intersection. | One symbol has two incompatible meanings. | Use `Q` for tangent intersection, `S` for shoulder, `Vtx` for classical vertex, `F` for focus. |
| C2 | Brief 09 T3 asks to infer rho from two endpoints plus apex. | Those three points define a control triangle, not the conic weight. | Impossible without an additional point, tangent/curvature datum or rho. Use an explicit example choice. |
| C3 | SOLIDWORKS UI calls the tangent intersection `Top vertex`; the API calls its second triplet `Apex`. | Similar words do not prove equal semantics. | Probe the actual returned geometry. Do not copy source `apex` to COM `XApex`. |
| C4 | Fillet API `[0.05,0.95]` and sketch conic UI. | They concern different commands. | Do not transfer the fillet range to sketch conics. |
| C5 | `ISplineParamData.SetControlPoints` and `CreateSplineByEqnParams` reference pages. | The former prescribes homogeneous `(wx,wy,wz,w)` for open 4D splines, the latter simply states `(x,y,z,w)`. | Separate tests and readback. Do not assume both accept the same wire format. |
| C6 | Official `CreateConic` example vs a naive classical focus/vertex interpretation. | The published coordinates imply incompatible endpoint eccentricities under that interpretation. | Treat the method's geometric mapping as unverified. Obtain a recorded UI macro and compare with hand-authored pure conic fixtures. |
| C7 | Rho threshold and the focal eccentricity `e`. | Both classify conic families but are **not the same number**. | Keep `rho`, `w` and `e` as separate typed quantities. |
| C8 | "Exact conic replay" versus "exact curve geometry". | A rational spline may represent the curve exactly but changes entity type and editable dimensions. | Record geometry fidelity separately from semantic/feature-tree fidelity. |

### New cross-report implementation insight

Reports 06/07 and brief 08 together suggest a safer implementation path: use the **source's exact rational representation** as the verifier's reference. A native `CreateConic` attempt must satisfy that reference. If it fails, an **exact weighted-spline** attempt is justified only when the project accepts a different sketch-entity type. This avoids building a production path around an undocumented `CreateConic` apex convention. Report 08's verified 2026 snapping issue also applies: create the sketch with controlled inference settings and restore the prior application state. For the weighted-spline path, some official spline APIs already bypass `AddToDB` display/inference settings [SW6].

---

## Still needs a SOLIDWORKS 2026 seat probe

| Priority | Probe | Action | Required pass condition |
|---|---|---|---|
| **P0** | `CreateConic` parameter semantics | Create the three numeric fixtures E/P/H with the proposed classical `F,Vtx,P0,P2`. Also try native UI top-vertex coordinates as the API apex in a separate controlled test. | Determine which inputs create a curve. Extract point samples, tangent lines and trimmed arc. The curve must match the exact source `B(t)` within the chosen model tolerance. |
| **P0** | Official example consistency | Run the exact documented `CreateConic` example. Inspect the actual returned segment, curve class, start/end, focus/apex geometry and displayed rho. | Explain the endpoint-eccentricity mismatch. Do not use parameter names as proof. |
| **P0** | Weighted-spline dimensional encoding | Use `CreateSplineParamData`, `Dimension=4`, `Order=3`, `ControlPointsCount=3`, `Periodic=False`, clamped knots. Supply **homogeneous** open-curve controls. | Created segment is non-null, has three controls, expected knots and correct weight. Read-back curves coincide with all source samples. |
| **P0** | One-argument packed-array route | Call `CreateSplineByEqnParams` with 4+6+12 doubles, once with `(x,y,z,w)` and once with `(wx,wy,wz,w)` on separate sketches. | Determine accepted SAFEARRAY form and actual geometry. Reject a non-null return if shape is wrong. |
| **P0** | Conic topology/branch | Use asymmetric control triangles, ellipses, parabolas and hyperbolas; reverse input endpoints. | Check source arc branch, endpoint order and that the result closes expected profiles without self-intersection. |
| **P1** | Circle and near-circle | Test a circular rational quadratic at `rho=5/(5+sqrt(29))` with the fixed `(0,0),(20,8),(40,0)` triangle; test nearby rho. | Determine whether native `CreateConic` supports coincident foci and how it chooses a vertex/axis. The rational-spline result should stay exact. |
| **P1** | Near-parabolic cases | Probe `rho` values immediately above/below `0.5`, plus exactly `0.5`. | No silent type flip, overflow or incorrect branch. Prefer spline if focal conversion fails. |
| **P1** | Sketch coordinate system | Create a sketch on a rotated/offset plane, with nonzero model XYZ. | Validate `ModelToSketchTransform` or inverse usage, millimetre-to-metre conversion and normal direction. |
| **P1** | Constraint and rebuild behavior | Create curves, then apply source-like dimensions and solve. | Final geometry, not only creation-time geometry, matches the source. Restore inference and solve settings after failures. |
| **P1** | 854-curve throughput | Replay a synthetic batch with matching curve and constraint distributions. | No lost curves, unexpected relations, excessive slowdown or memory failure. Each segment receives a stored verdict. |
| **P2** | Native conic rho range | Create sketch conics through the UI at `rho=0.01`, `0.05`, `0.95`, `0.99` as supported. | Determine actual bounds for this SOLIDWORKS 2026 seat. Do not assume the fillet bounds apply. |

**Release gate:** Do not mark the conic replay path as implemented until at least one method is proven exact by independent geometric readback on the licensed seat. If the contract requires a native SOLIDWORKS conic entity, a geometrically exact spline is still a different output and must carry a distinct verdict or block release.

---

## Sources and exact URLs

### SOLIDWORKS documentation

- **[SW1]** SOLIDWORKS Design Help 2025, *Conic PropertyManager* (shoulder, top vertex, Rho): https://help.solidworks.com/2025/english/solidworks/sldworks/HIDD_DVE_SKETCH_BCONIC.htm
- **[SW2]** SOLIDWORKS Design Help 2025, *Sketching Conics*: https://help.solidworks.com/2025/english/SolidWorks/sldworks/t_Sketching_Conics.htm
- **[SW3]** `ISketchManager.CreateConic` historical official API reference mirror, pinned commit (12 positional doubles and remarks): https://github.com/streetartist/solidworks-automation/blob/3513f70e0c30fe3db8f15641b8d350ccdb7367f1/docs/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~CreateConic.md
- **[SW4]** SOLIDWORKS API Help 2026, *Insert Conic Curve Example (VBA)*: https://help.solidworks.com/2026/english/api/sldworksapi/Insert_Conic_Curve_Example_VB.htm
- **[SW5]** `ISplineParamData.SetControlPoints` official Help 2024 (open dimension-4 homogeneous storage): https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISplineParamData~SetControlPoints.html
- **[SW6]** `ISketchManager.CreateSplineByEqnParams` official Help 2023 (4D weight layout): https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~CreateSplineByEqnParams.html
- **[SW6b]** Older `IModelDoc2.CreateSplineByEqnParams` 2020 (packed SafeArray): https://help.solidworks.com/2020/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDoc2~CreateSplineByEqnParams.html
- **[SW7]** `ISimpleFilletFeatureData2.DefaultConicRhoOrRadius` 2026 (fillet only): https://help.solidworks.com/2026/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2~DefaultConicRhoOrRadius.html
- **[SW8]** `ISketchManager.CreateSplinesByEqnParams2` 2022: https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~CreateSplinesByEqnParams2.html
- **[SW8b]** `ISplineParamData` members 2024: https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISplineParamData_members.html
- **[SW9]** `ICurve.GetClosestPointOn` 2024: https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ICurve~GetClosestPointOn.html
- **[SW9b]** `ICurve.Evaluate2` 2024: https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.icurve~evaluate2.html
- **[SW10]** `IModelDoc2.SketchParabola` 2025 (explicit focus/apex point conventions for the *older parabola* API, not a proof about `CreateConic`): https://help.solidworks.com/2025/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDoc2~SketchParabola.html

### SolidCraft source studies supplied with this request

- **[SC1]** `SolidCraft_v043_Nested_Schema_Research(1).md`, A1. Source-pinned at `v0.43.0`: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs#L145-L235
- **[SC2]** Same study, A1 conic and source links: https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/curves.rs#L94-L136 ; https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/sketch_create.rs#L1053-L1118
- **[SC3]** `SolidCraft_57_Format_Research(1).md`, native `solidcraft/57` feature/sketch serialization and command-vs-native distinctions. Source baseline: https://github.com/tangency-labs/solidcraft/tree/v0.43.0
- **[SC4]** `solidworks-api-usage-archaeology-07(1).md`, T1 exact `CreateConic` signature, official example and unresolved readback questions.

### Independent geometry and numerical references

- **[M1]** R. T. Farouki, *Computer numerical control algorithms*, slide 28 (PDF page index 27, rational quadratic Bezier equation and weight-based conic classification): https://faculty.engineering.ucdavis.edu/farouki/wp-content/uploads/sites/51/2021/07/Computer-numerical-control-algorithms.pdf
- **[M2]** University of Reading, *Conic sections*: https://www.met.reading.ac.uk/pplato2/h-flap/math2_3.html
- **[M3]** *Simple determination via complex arithmetic of geometric characteristics of Bezier conics*, Computer Aided Geometric Design 28 (2011), DOI: https://doi.org/10.1016/j.cagd.2011.06.007
- **[M4]** J. R. Shewchuk, *Fast Robust Predicates for Computational Geometry*: https://www.cs.cmu.edu/~quake/robust.html
- **[PTC1]** PTC Creo Help, *To Dimension a Conic Arc Using rho*: https://support.ptc.com/help/creo/creo_pma/r13/usascii/part_modeling/sketcher/To_Dimension_a_Conic_Arc_Using_rho.html

*Source handling:* Public URLs ground the documented claims. The algebraic conversion and numbers in this report are new derivations. They were checked independently but remain untested against the proprietary SOLIDWORKS creation behavior.
