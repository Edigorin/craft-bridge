# Research brief 09 - conic representations and the rho conversion

Follow-up to briefs 03/04/07. Brief 07 established from official examples
and mirrored reference pages that `ISketchManager.CreateConic` takes
**twelve doubles - focus XYZ, apex XYZ, start XYZ, end XYZ - and no rho
argument**, and that the SOLIDWORKS user-interface conic command instead
takes two endpoints, an apex, and a rho value. A real design gates on
this: 854 conic curves in the Pierre design must be replayed exactly.
The gap this brief asks you to close is the **representation conversion**:
given a conic defined the way CAD sources store it (typically endpoints +
apex + conic constant rho), produce the focus/apex/start/end quadruple
that `CreateConic` consumes - exactly, with every edge case named.

## Ground rules (same as briefs 03/04/07/08)

- Public sources only: mathematics references (textbooks, encyclopedic
  sources, university notes), SOLIDWORKS product Help (all editions
  reachable), API Help, official SDK examples, pinned third-party CAD
  code (any CAD kernel that converts conic forms), long-lived forum
  solutions. No licensed seat.
- Label every claim: **Recorded** (official documentation/example),
  **Community** (third-party code; usage not success), **Conflicting**
  (sources disagree; keep both), **Reconstructed / untested** (derived;
  needs a seat probe or a worked numeric check before use).
- Prefer exact formulas with named variables over prose. Every formula
  must come with a worked numeric example (below). Quote only the
  relevant fragments; explanations are original.
- SOLIDWORKS is a Dassault Systemes trademark; nominative reference only.

## T1. The conic constant: definitions and ranges

1. The exact definition of the conic constant / rho used in CAD sketchers:
   its geometric meaning (focus-directrix eccentricity? the apex-tangent
   angle convention? both?), and the precise rho ranges that select
   ellipse (including circle), parabola, and hyperbola. Cite the
   SOLIDWORKS product Help's conic documentation for its own convention
   (the UI's rho semantics) and at least one independent mathematical
   source. Note explicitly if different systems define rho differently
   (e.g. rho = cos(alpha) vs rho = e).
2. Which parts of the rho range the SOLIDWORKS UI exposes (clamps,
   limits like 0.05..0.95 seen in the fillet context - confirm whether
   the SKETCH conic shares any documented range restriction).
3. Whether the apex of a CAD conic is the curve's vertex (the point of
   maximum curvature / the point where the tangent is parallel to the
   chord?) - pin the exact geometric definition used by the SOLIDWORKS
   conic command, with the source.

## T2. The conversion: endpoints + apex + rho -> focus + apex + start + end

Derive (or recover from cited sources) the exact construction:

1. Given chord endpoints P1, P2 and apex A (a point on the curve between
   them) and rho: the construction of the conic's focus F, and the
   verification that (focus F, apex A, start P1, end P2) reproduce the
   same curve. Include:
   - the tangent lines at P1 and P2 and their intersection (often used in
     the classic construction - pin how rho relates to the segment
     division of that intersection);
   - the directrix and eccentricity formulation;
   - special cases: A = midpoint of the chord (circle/ellipse symmetric
     case), rho = 0.5 (parabola), rho > 0.5 / rho < 0.5, collinear or
     degenerate inputs, and endpoints that make the apex unreachable.
2. The INVERSE: given (focus, apex, start, end) as `CreateConic` consumes,
   how to read back rho and the endpoints - for verification of replayed
   geometry. Is the quadruple sufficient to determine a unique conic?
   Which conic classes can `CreateConic` produce from it (brief 07's
   reference lists circle, ellipse, parabola, hyperbola)?
3. Any documented or community statement about how the SOLIDWORKS UI's
   (endpoints, apex, rho) maps to the COM API's (focus, apex, start,
   end): does the product perform exactly the classical construction
   above? Cite whatever exists; if nothing exists say so plainly.

## T3. Worked numeric examples (required)

Produce at least three fully worked examples with exact numbers, each
stating every intermediate quantity (tangent intersection, division
ratio, focus coordinates, directrix distance):

1. Ellipse case: P1=(0,0), P2=(40,0) mm, apex at (20,8) mm - recover rho
   from these three points first (that is itself an open sub-question:
   given only endpoints + apex, what rho does SOLIDWORKS assume? state
   it), then the focus/apex/start/end quadruple.
2. Parabola case: rho = 0.5 exactly, with any clean numeric chord/apex.
3. Hyperbola case: rho > 0.5, clean numbers.

For each, state how a replay implementation can VERIFY its output (e.g.
re-derive rho from the quadruple and compare; sample points on the curve
and check the focus-directrix property).

## T4. SolidCraft-side conic storage (what the replay reads)

From the supplied/referenced SolidCraft research reports (57 format and
nested schema research), summarize exactly how conic curves are STORED
in a native design: field names, units, which of (endpoints, apex, rho,
focus, control points, equation coefficients) appear, and any validation
the format applies. If the reports leave fields ambiguous, say which
fields need a source check rather than guessing. This section may quote
the attached reports; label those quotes as source-supplied.

## T5. Numerical robustness (short)

1. Conditioning: how sensitive is the focus reconstruction to apex
   positions near the chord, near rho=0.5 transitions, or at the
   ellipse/hyperbola boundary? Cite numerical-geometry guidance where it
   exists.
2. Units and tolerances: the conversion should run in metres; state any
   tolerance recommendations found in the literature for equality checks
   on derived conic quantities.

## Output

A markdown report filed as `research/outputs/09-*.md`, with the evidence
labels, a findings-at-a-glance table, a conflicts table, worked examples
as copy-checkable arithmetic (not code that needs a seat), and an
explicit "still needs a seat probe" table covering anything about
`CreateConic` itself that only the seat can settle (e.g. whether the
created curve's readback matches the inputs). The consumer is the
craft-bridge emitter's conic replay for solidcraft/57 designs: exact
geometry or an honest named stop, never an approximation presented as
exact.
