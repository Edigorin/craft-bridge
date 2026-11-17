# Follow-up brief: nested-schema research + three more micro designs

Second round for the design-side agent. The first brief's research report
arrived and is already shaping the loader (thank you - the `cut` spelling,
the expr-over-pos rule, `t` as a fraction, and the edge-ref caution all
landed). Two deliverables again: **Part A - nested-schema research** (the
one gap that now blocks implementation), and **Part B - three additional
micro designs** for the batch currently being drawn.

---

# PART A - NESTED-SCHEMA RESEARCH

The first report gave the feature-kind inventory but left the nested types
open ("a full exhaustive schema of every nested custom type needs a
separate source extraction"). We now know exactly which ones matter,
because the real designs use them heavily: one design carries 854 conics
and 512 arcs across its sketches, 43 `custom` planes, and 5,564 `fix`
constraints; another is built from splines. Please extract from the same
v0.43.0 source (same citation style), answering each item with the exact
serialized field names, types and semantics:

## A1. Curve kinds (highest priority)

For each of `arc {c,a,b}`, `conic {a,b,apex,rho}`, `spline
{pts,control?,degree?,periodic?,nurbs?,handles?}`, `ellipse {c,m,r}`:

- Exact meaning of every field (which are point indices into `points`,
  which are numbers, which are expressions). For `arc`: are `a`/`b`
  endpoints on the arc (and what decides the sweep side), or angles? Can
  an arc exceed 180 degrees? Direction convention?
- For `conic`: what are `a`, `b`, `apex` (point indices?), what does
  `rho` mean and what range (ellipse/parabola/hyperbola classification)?
  Is the curve an interpolating conic through a, b with apex?
- For `spline`: are `pts` interpolation points or control points? How are
  knots derived (clamped/unclamped)? What do `control`, `nurbs`,
  `handles` add, and when does the writer emit them? Endpoint tangency
  handling.
- For `ellipse`: what is `m` (major-axis endpoint? half-width?) and `r`
  (minor radius as a point? a number?).
- Which of these can carry `construction: true`, and what does that
  guarantee?

## A2. Constraint kinds (exhaustive)

The full serialized `ConstraintKind` inventory with per-kind field names
and reference types (point index / curve index / value / `param` /
`driven`). We already know `fix {p}`, `coincident`, `horizontal_points`,
`vertical_points`, `horizontal`, `vertical`, `parallel`, `perpendicular`,
`tangent`, `equal`, `length`, `distance`, `distance_x`, `distance_y`,
`radius`, `diameter`, `angle`. What else exists (symmetry, midpoint,
fix_angle/fix_length, path constraints, spline handles constraints,
`offset`-related, equation links)? What does `driven: true` mean exactly
(a driven/reference dimension)? What is the exact `param` binding
semantics (the dimension is driven by that model parameter)?

## A3. `custom` planes and other plane internals

- `plane.type: "custom"` has field `plane`: what is its nested `Plane`
  shape (explicit origin/normal/axes? face-derived `GeoRef`?). One real
  design has 43 of these on sketches; the exact serialized form decides
  whether we can replay them or must stop named.
- The nested `GeoRef` variants (face picks, edge picks) as serialized:
  field names, identity persistence (names? indices?).
- `at_angle`, `through_point`, `placed`, `midplane`, `three_points`
  nested field types (expressions vs points vs refs) - a compact table is
  fine.

## A4. Value serialization conventions

- `Expr`: when does the writer emit a raw JSON number vs a string? (We
  see `0.0` raw and `"0"` strings in the same arrays.) What is the bare
  number's unit (mm?) and the canonical form on save?
- `Position` (3-D pick points, e.g. fillet `edges`): bare `[x,y,z]` vs
  `{"at": [x,y,z]}` - when does the writer emit which?
- Expr3-like triples (`translate`, `origin`, `center`, `base`,
  `rotate_axis`, `corner`): number vs expression per component?

## A5. Booleans, bodies and names

- `targets` on extrude/revolve/sweep/loft: exact serialization and the
  precise semantics of an EMPTY `targets` with `operation: "join"` (the
  real plaque has join extrudes with no targets). Does it mean "all
  bodies in the feature's component"?
- `param_names` / `dN`: confirm these are OUTPUT dimension names
  (provenance), and whether their values live anywhere in the file.
- Body naming rules: when do unnamed bodies get auto names, and can a
  later feature reference a body by an auto-generated name?

## A6. Timeline and expression edge cases

- `marker` precision: 0-based index into `features`? Inclusive or
  exclusive? What does the evaluator do with features past it, and does
  the STEP export exclude them?
- On save, does the exporter recompute `pos` from `exprs` (so a mismatch
  never occurs in exported files)? If not always, when can they diverge?
- Coordinate `exprs` edge cases: a point with only ONE axis expressed;
  unit-bearing literals inside coordinate expressions; negative results;
  expressions referencing OTHER points (is that even possible?).
- `text_params`: shape and whether features can reference them.

If any answer cannot be established from source, say so explicitly.

---

# PART B - THREE MORE MICRO DESIGNS

Same rules as the first brief (asymmetric, distinctive mass, mm, small,
Name.json + Name.step each, one capability per design, delivered with a
one-line note and a parameter-edit suggestion). Add these to the batch in
progress. IMPORTANT: if the real sketches show curve kinds the exporter
writes as `arc`/`conic`/`spline`, Design 8 is the highest-priority design
in this whole program after the offset-plane micro - please treat it so.

## Design 8 - Curve-kind micro (arc, conic, spline)

One sketch containing THREE separate closed profiles (or three tiny
designs if cleaner):
- a profile using an `arc` (include one arc over 180 degrees if the
  exporter can produce it),
- a profile using a `conic` (rho that makes it clearly elliptical, not
  circular),
- a profile using a `spline` through 6+ points (asymmetric).
Extrude each (or the combined sketch). Say exactly which curve kind each
profile uses. Acceptance: each curve kind replays with its geometry
verified against STEP.

## Design 9 - Custom-plane micro

A sketch on a plane of the type the exporter writes for a face-derived or
explicit custom plane (the form that appears as `plane.type: "custom"` -
please export it the way the tool naturally does, not a `placed` plane if
`custom` is what the tool writes for your construction method). One
extrude on that sketch. Acceptance: the sketch lands on the right plane
in 3D, verified against STEP. A one-line note describing what the `plane`
object contains is essential here.

## Design 10 - Fix-constraint and one-axis-expression micro

One sketch with:
- two points carrying `fix` constraints,
- one point with an expression on the X axis only (Y numeric),
- one point with an expression on the Y axis only (X numeric),
- one point with expressions on both axes,
- one point with neither (plain numeric).
Plus one extrude. Acceptance: editing the driving parameter moves only
the expressed coordinates of each point.

## Also please confirm (from the first brief's open item)

- Whether a point with `fixed: true` AND `exprs` is creatable through the
  MCP commands (the first report flagged this as unverified). If not,
  export such a point by whatever path works and note it.
- After each save, check the written `format` string is exactly
  `solidcraft/57` (per the first report's versioning warning) and note
  the app/build version used.
