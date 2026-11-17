# Mapping: SolidCraft recipe → SolidWorks API

The translation table the generator works from. Everything here is pinned
from SolidCraft's own serialization (`/home/jerome/Projects/solidcraft`:
`crates/engine/src/cmd/script.rs`, `crates/doc/src/document.rs`,
`crates/sketch/src/model.rs`) and from SolidWorks' public API documentation.
The last column is filled in when the compatibility checkpoint runs on the
seat. One allowlist decides what is translated; reports are rendered from the
same decision, so they cannot diverge from what the macro does.

## Expressions (the supported subset)

One bounded engine (`src/replay/expr.rs`) parses, evaluates and translates
SolidCraft expressions. Numeric literals are *poly*: their dimension comes
from context — bare `2` in `2 * width` doubles a length rather than being two
millimetres — and an explicit unit (`mm|cm|m|in|ft|deg|rad`) fixes the
dimension immediately. Parameters carry their own unit; a bare result reads in
the caller's context (feature literals as millimetres, non-literal expressions
as the design unit). Same-dimension division yields a count. Unsupported and
refused: functions, implicit multiplication, fractional powers, powers of
dimensioned values, mixed-dimension arithmetic, division by zero, unknown
names. Hostile-input caps: 400 characters, 128 tokens, depth 16, identifiers
64 characters — long names are rejected, never truncated into aliases.

Translation to SolidWorks equation syntax preserves the same meaning
(`40 mm` -> `40mm`, bare numbers stay bare, identifiers -> quoted global
names) and is escaped for the macro separately from VBA-literal escaping.

## Fallback snapshots (the hybrid contract)

An artifact may carry fallback snapshots: STEP bodies of the model state
after a recipe step has run in SolidCraft (`snapshots/step_NNNN.step`, scope
`boundary`) or one end-of-recipe body (scope `final`, the coarse rule). Packed
with `pack --snapshot N=file` / `--final-snapshot file`, bounded and
inventory-checked. At an unmappable step the macro runs a transaction:
restore the pre-step state (delete every created feature and sketch), import
the boundary body, invalidate every live binding slot, and continue replay
against the replacement. A final snapshot terminates replay after its import.
Artifacts without snapshots report their missing coverage and terminate at
the first unmapped step. API members: `ImportFile` and `EditDelete`
(checkpoint-pending like the rest).

## Primitives (box, cylinder)

Replayed as generated sketches on the plane carrying the XY source frame
plus a blind extrude whose side resolves through the direction truth table.
The profile maps through `MapU`/`MapV` **like sketch geometry**: the box is
four `CreateLine` segments between mapped corners in deterministic source
order (ln0 is the SC-u edge from the corner and carries the length
dimension, ln1 the SC-v edge and the width dimension - the equation-to-edge
binding never depends on a host segment order), and the cylinder is a
mapped base centre with its radius unchanged (the map is a validated
isometry, so sizes stay SI metres). Placement must be literal and on the
base plane; sizes keep their parametrisation through dimension equations
under the sign contract (the sign resolved from the dimension readback
into `AddDimEquation`'s `signVal` and `regSign`; `SlotRef` normalizes
downstream references). Off-plane placement and non-Z axes stop (they need
extrude start-offset work); a cylinder radius at or below 2e-6 m stops at
plan time (see the circle-size kind below).

The map class is **signed permutations only** (bounded support - this seat
only exhibits permutation frames): `MapIsPermutation` refuses anything else
with a named reason before any drawing. The map's orientation (`gMapSide`)
is unconstrained - the truth table composes it.

The generated box carries its **shape contract** explicitly - no automatic
relation is assumed: `sgCOINCIDENT` closes the four corners, classified
`sgHORIZONTAL2D`/`sgVERTICAL2D` relations (runtime `AxisClass`) pin
rectangularity and the edge directions, and `sgFix` on the corner point
anchors the literal placement (the cylinder centre is anchored the same
way). Creation-time application is behaviourally indistinguishable from a
pre-held predicate (the recorded limitation class); the parameter-edit
behaviour is pinned by `probe-primorient`'s `box-edit-follow` case and
remains an owner-seat acceptance check, like `sketch.edit-follow`.

The **circle-size dimension kind** is settled seat evidence: `AddDimension2`
on a circle is **diameter-valued** (frame-sign-chain measured .02 for radius
.01; folded as the `dimScale` convention in a76b0e7/39dad15). Radius
dimensions carry `dimScale` 2 at the API boundary - the value target is
twice the radius, the equation text wraps `2*(...)`, the expect is scaled,
and `SlotRef` normalizes downstream references back to source semantics
through `regScale` composed with the sign (`regSign`) in the factor form.
The plan still refuses a radius at or below 2e-6 m: below twice the 1e-6 m
verification tolerance a wrong radius-valued reading is indistinguishable
from the convention (and a halved driven radius would pass the geometry
check). The runtime check refuses any reading that is not the diameter
reading before the value is written and the dimension registered.
`probe-primorient`'s cylinder case cross-checks the convention on the
primitive path.

`AddDimEquation` ends in `EditRebuild3`, which **exits sketch edit**
(seat-measured): every primitive dimension equation is followed by a
`SolveSketch` re-entry before any further sketch API call, and the solved
profile placement is read back against the mapped coordinates afterwards
(evidence, not assumption - the revolve helper-axis standard).

The **operation orientation**: the SolidCraft height extends toward +Z,
the XY source normal (source-pinned kernel semantics: the box spans
`corner.z..corner.z + height`, the cylinder `base..base + height * axis`
with the axis pinned to +Z and sizes positive), so its end sits toward
`+1 * gMapSide` in sketch-normal units and the call flag is
`dirFlag = (wantSide = TT_PRIM_DIR_TRUE_SIDE)` - the extrude family's
composition. `TT_PRIM_DIR_TRUE_SIDE` is **MEASURED (-1)** (probe-primorient
`box-dir-false`/`box-dir-true`, 2026-11-17, on the Top frame's
non-identity map: `Dir=True` put the blind 0.012 m box at z in
[-0.012, 0], `Dir=False` at [0, 0.012]; the reading equals
`TT_EX_DIR_TRUE_SIDE` on a second frame and the composition held end to
end there - see the truth-table row). The guard stays in the macro for a
zeroed cell: while it reads 0 every primitive step fails with a named
reason naming its probe and never reports `created`.

## Modifications (fillet, chamfer, shell, draft, hole)

Coordinate picks in model millimetres (identity map per the orientation
policy) with fail-closed `SelectByID2` type checks. Fillet is constant-radius
(variant flags become degrade triggers: style, fit, corner, turn, setbacks);
chamfer is equal-distance (unequal stops); shell honours the direction field
(default inside); draft needs an origin neutral plane; holes are simple or
drilled (counterbore, countersink, threads, to-object and sketch-point holes
stop). Feature-call shapes: `FeatureFillet3`, `FeatureChamfer3`,
`FeatureShell`, `FeatureDraft`, `SimpleHole2` — all checkpoint-pending.

## Patterns and mirrors

Mirrors replay about origin planes with live seed bindings: seeds must be
verified native features (registered only after their own verification,
fail-closed when missing), never fallback bodies. Body mirrors stop. Feature
patterns (rectangular and circular) currently stop at the plan level: their
direction and axis selection story needs exact API members, pinned when the
docs are reachable again; the parser and emitter seams are in place.

## Equations (this phase)

Parameters become SolidWorks equations: user parameters as global variables
(`"width"=40mm`), model parameters and non-literal feature inputs as dimension
equations whose left-hand side is the dimension name **captured at runtime**
(never assumed D1..Dn) and whose right-hand side is the translated expression.
A dimension's runtime name is registered in a slot **only after its own
verification**; cross-references render those names at runtime and dead-slot
guards fail closed, so a step whose parameter source fell back can never
silently drive a stale equation.
Bare length literals are suffixed (`20mm`) so the equation means the same
thing whatever the part's document units are. An equation is verified by
evaluation: added, rebuilt, and the dimension's value must match the
evaluated expectation before the step reports `created`. Expressions may
reference global parameters only in this phase; a model-parameter reference
stops that step, and a dimension whose binding is not expressible keeps its
solved value with the binding reported as a degrade trigger.

## Units

| Where | Unit |
|---|---|
| Sketch coordinates, circle radius, constraint `value` lengths | mm, always (never affected by `document.units`) |
| Constraint `value` angles (`angle`, `line_angle`, `arc_sweep`) | radians, always |
| Extrude/feature length literals (`"10 mm"`, `"0.5 in"`, bare `"10"` = mm) | parse to metres |
| SolidWorks API inputs and readback | metres and radians, always |
| Runtime report numbers | `Str()` output: period decimal separator, locale-independent |

## Recipe envelope (what the parser accepts)

`{ "about": {...ignored...}, "commands": [ {"command": "...", "params": {...}}, ... ] }`

- `document.units` → `{"units": "mm|cm|m|in|ft"}`. Recorded; all values replay
  in mm regardless, so this is informational until expressions land.
- `parameters.add` → `{"name", "expression", "unit", "comment"?, "favorite"?}`.
  **Not executed** in this phase (equations land with the mapping family that
  introduces them); recorded in the plan report only. Never reported as
  runtime `created`.
- `timeline.add` → `{"feature": {<feature, fields inlined, "type" tag>},
  "params": [ {"name","expression","unit"}, ... ]}` (an **array** of model
  parameters; feature inputs are separate, inside the feature's fields).
- Any other command, or any feature the tracer cannot translate: the
  **stop rule** — model replay stops at that step, the step is planned as
  `fallback-planned` (fallback bodies land with the hybrid-fallback ticket),
  and every later step is `not-executed`. Nothing downstream of an unmapped
  step is ever claimed as created.

## Sketch (tracer phase)

| SolidCraft | SolidWorks API | Checkpoint-verified |
|---|---|---|
| `type: "sketch"`, `plane: {type:"origin", name:"XY"\|"XZ"\|"YZ"}` | probe for the origin plane carrying the SolidCraft frame (see Orientation), `SketchManager.InsertSketch True` on it | partial |
| `points[]`: `{"id", "pos": [u, v] (mm), "fixed"}` | entity endpoints drawn at the **mapped** `MapU(u/1000, v/1000), MapV(...), 0` (see Frame adaptation) | yes |
| `curves[]` line: `{"id","type":"line","a","b","construction"}` (a/b: **indices into points[]**) | `SketchManager.CreateLine(x1,y1,0,x2,y2,0)` | yes |
| `curves[]` circle: `{"id","type":"circle","c","r","construction"}` | `SketchManager.CreateCircleByRadius(cx,cy,0,r)` | yes |
| constraint `coincident` `{"p","q"}` | select both `ISketchPoint`s → `SketchAddConstraints "sgCoincident"` | pending |
| `horizontal` / `vertical` `{"l"}` | **classified at runtime** against the sketch map: the SC axis lands on the sketch s or t axis and the constraint becomes `"sgHorizontal"` / `"sgVertical"` there; a non-permutation map omits it with a named reason | pending (checkpoint covers the classified form) |
| `parallel` `{"a","b"}` | select both segments → `"sgParallel"` | pending |
| `perpendicular` `{"a","b"}` | select both → `"sgPerpendicular"` | pending |
| `tangent` `{"a","b"}` | select both → `"sgTangent"` | pending |
| `equal` `{"a","b"}` | select both → `"sgEqual"` | pending |
| dimension `length` `{"l","value"(mm),"param"?}` | `AddDimension2` on the segment (text position in **model space**); kind verified against the point projections; sign resolved into the value and the equation RHS | pending (checkpoint covers it) |
| `distance`/`distance_x`/`distance_y` `{"p","q","value"(mm)}` | `AddDimension2` on the two points; `distance_x/y` are **classified at runtime** like the axis relations; a non-permutation map omits them with a named reason and a dead equation slot | pending |
| `radius` / `diameter` `{"c","value"(mm)}` | `AddDimension2` on the circle | pending |
| `angle` `{"a","b","value"(rad)}` | `AddDimension2` on the two segments (radians) | pending |
| any other relation type (`pattern_point`, `point_along`, `pierce`, `symmetric`, `arc_sweep`, …) | **degrade trigger**: recorded, never silently dropped | n/a |
| any other entity (`arc`, `spline`, `ellipse`, `conic`) | **degrade trigger** for the tracer phase | n/a |

**Orientation (the frame contract):** SolidCraft is Z-up, SolidWorks is
Y-up, and a seat's origin planes are not portable: one seat maps Front to
the ZY plane, names are localized, and tree positions vary. So the target
plane is found by **probing**, never by name, role or tree position: the
macro opens a throwaway sketch on each plane-like feature and reads its
frame from `ModelToSketchTransform` (the sketch axes in model coordinates
are the columns of the rotation; the translation is the model origin in
sketch coordinates). Since the frame-adaptation batch (owner decision
2026-11-13) the plane is accepted when the probe sketch sits on the
**geometric source plane** — translation at the model origin and normal
**parallel** to the source normal (either sign) — and the probed frame then
publishes a per-sketch **adaptation map** that puts (u, v) coordinates where
they belong. Strict frame equality is retired from the acceptance path and
remains only as a report diagnostic (`SketchFrameIs`, how close a probed
frame came to strict equality). The source frames it compares against are:

| SolidCraft plane | sketch u | sketch v | normal (u x v) |
|---|---|---|---|
| `XY` | +X | +Y | +Z |
| `XZ` | +X | -Z | +Y |
| `YZ` | +Y | +Z | +X |

The frames are source-pinned from SolidCraft's own plane table
(`solidcore/crates/solidcore-record/src/plane.rs:29-34`, in the SolidCraft
repository), never inferred from the plane-name letters. The XY row is
seat-verified behaviour; the XZ and YZ rows are source-verified semantics
whose seat compatibility awaits its first corpus item.

An offset origin is still **refused** and the step fails closed; the report
names every probed plane's observed frame (with its strict-match diagnostic)
so a mismatch is legible. Preserving the original *view* orientation in
SolidWorks is a later nicety, not part of fidelity.

**Frame adaptation (the map):** each accepted probe publishes a 2x2 map from
the probed transform: SC `(u, v)` is drawn at sketch `(s, t)` with
`s = u*gMap00 + v*gMap10`, `t = u*gMap01 + v*gMap11` (the sketch axes in
model coordinates are the transform's columns). Publication is fail-closed
and transactional — validity is cleared before every probe and set only
after the complete validation passes: finite entries, 16-entry `ArrayData`
with scale 1 (entry 12), unit axes, mutual orthogonality, and a
right-handed 3D frame `u x v = n`; all tolerances 1e-6. A proper 3D frame
with the normal **opposing** the source normal yields an improper 2D map —
supported, recorded in the map's side flag. Per-sketch map state: a feature
that references an earlier sketch runs against that sketch's map; probe
sketches (`PlaneNameFor`) never touch persistent state; sketch deletion
invalidates the entry before deletion, verifies the deletion against the
pinned point enumeration, re-probes and rebinds.

Axis-dependent work (the `horizontal`/`vertical` relations and the
`distance_x/y` dimensions) is **classified at runtime**: for a
signed-permutation map the SC axis lands on the sketch s or t axis and the
constraint kind follows it there (signs never affect h/v relations); the
dimension sign is resolved from what the dimension measures against the
source value and written into the assigned value **and the equation RHS**,
so later parameter edits keep tracking. A non-permutation map **omits** the
affected relation or dimension with a named reason and a dead equation slot
(terminating: never re-attempted as a failure) - bounded support, this seat
only exhibits permutation frames. Extrude and revolve replay runs through
the **direction truth table** on ANY valid adaptation map (the extrude side
decisions use only the map's orientation `gMapSide`; the revolve axis is
mapped like geometry and its sweep factor passes through the map unchanged
- see "Direction truth table"). Primitive steps (batch 3) draw their
profiles and sizes through the same map on signed-permutation frames and
resolve the extrude side through the direction truth table's own cell
(`TT_PRIM_DIR_TRUE_SIDE`, probe-primorient); the zeroed-cell guard remains
in the macro - while that cell is unmeasured every primitive step fails
with a named reason naming its probe, and nothing ever reports `created`
on an unestablished path.

**Evidence rules (batch 1):** every selection is checked (`Select` returns
Boolean and its result is captured). `SketchAddConstraints` has **no return
value** (API doc, `IModelDoc2.SketchAddConstraints`), so relation
application is established behaviourally - the constraint's own predicate
must hold on the solved geometry (levelling, coincidence, parallelism,
perpendicularity, equal length). A relation whose predicate already held on
the drawn coordinates is geometrically indistinguishable from a missing
one: recorded limitation. Tangency has no cheap solved-geometry predicate
over two lines and rests on the selection and coordinate checks. Dimension
kinds are doubly established: the created value is matched against the
point projections (with an explicit **refusal when the projection
magnitudes coincide**), and the value edit must carry exactly the measured
quantity - a wrong-kind dimension provably moves the wrong axis. Registered
dimension slots carry their sign (`regSign`) and downstream equation
references normalize back to source semantics through `SlotRef`, so
parameter edits keep meaning what the source meant. Feature dimension
equations (extrude depths, revolve angles) follow the same contract: the
dimension carries the absolute value, the sign travels through
`AddDimEquation`'s `signVal` into the stored equation text, and `regSign`
normalizes downstream references; after a two-sided argument swap each
equation binds to the dimension that carries ITS source side, never a
literal D number. A parameter edit that flips a source sign still cannot
follow a flipped geometry side within one replay (the baked side flags) -
a recorded limitation of the same class as the baked sketch-dimension
signs. The
input-dimension-value preference is disabled with a verified readback and
restored with one at every exit. No VBA identifier may shadow an intrinsic
(`sgn`, `val`, `mid`, ...): `Dim sgn As Double` is a **compile error** that
the runner reports only as a bare invocation failure with no report -
regressed by `no_identifier_shadows_a_vba_intrinsic`.

**Solve verification (runtime):** after relations and driving dimensions are
added, the macro reads the solved geometry back (line endpoints via
`ISketchPoint.X/Y/Z`, circle centre/radius) and compares against the recipe's
stored coordinates with a nominal tolerance of 1e-6 m. Every supported entity
must read back; a missing readback can never report `created`. Deletion
verification accepts only a **proven-empty** point collection: a failed
query, a non-array result or an unestablishable bound refuses (a failed
query is never proof of emptiness). On any
relation/dimension API error or mismatch: **restore**, don't just delete —
the map is invalidated first, the sketch contents are deleted and the
deletion verified against the pinned point enumeration, the frame is
re-probed and the map republished, the geometry is redrawn from the stored
coordinates, driving dimensions are reapplied (their registry slots cleared
and rebound), and the result is verified again and reported `degraded` (or
`failed` if the clean rebuild also mismatches). Deleting relations alone is
not a degrade path: the solver may already have moved entities. A
representative parameter-change check (edit a driving dimension, see the
geometry follow) runs in the checkpoint (`sketch.edit-follow`) as an
owner-seat acceptance item (`docs/seat-acceptance.md`); the offline tests
pin its text, not its behaviour.

## Extrude (tracer phase)

| SolidCraft | SolidWorks API | Checkpoint-verified |
|---|---|---|
| `type: "extrude"`, `profiles: {"type":"all"}` | select the sketch, `FeatureManager.FeatureExtrusion2` (the call shapes below) | pending |
| `extent.distance` literal or expression (`"12 mm"`, `"0.5 in"`, bare = mm) | depth in metres; non-literal expressions become dimension equations under the sign contract (Equations) | pending |
| `direction: "positive"|"negative"` | blind extrude: the material's end is `e1 = direction-sign * distance` (source-pinned #2700: a negative distance turns its own side over). The `Dir` flag is resolved through the direction truth table and the sketch's map side | pending |
| `distance2` | two-sided: the body spans between its two ends `e1 = direction-sign * distance` and `e2 = -direction-sign * distance2`; the two depth arguments (and their D1/D2 dimension identities) swap per the truth table so each end lands on its source side. Ends on the SAME side of the sketch plane are refused before the call: no supported call shape spans them | pending |
| `through_all` | one-sided through-all on `e1`'s side; the saved distance only names the side (#10415) and carries no depth dimension, no equation and no slot. `distance2` with `through_all` is a named stop (its call shape is unprobed) | pending |
| `direction: "symmetric"` | mid-plane extrude; `distance` is the total thickness (`convention: total_distance`; `legacy_per_side` doubles it and keeps literal distances only - a parametric one would drive the doubled dimension from the unscaled expression and is refused). Direction-free: no truth-table cell is consulted | pending |
| `operation: "new_body"` | plain extrude in a fresh part | pending |
| `start_offset`, `taper`, `to`/`from`, `to_offset`, `corner`, `to_body_apart`, `thin`, `cut`/`intersect`, `profiles` other than `all` | **stop rule** (unsupported in the tracer phase) | n/a |

Extrude verification: the feature must exist and its depth dimension must
read back within tolerance (each two-sided depth against the side that
actually carries it after the swap). There is no degrade path for a failed
extrude in this phase: it is `failed`.

## Direction truth table (extrude and revolve, batch 2)

The physical side a call flag selects is **measured on the seat**, never
assumed. Every constant below lives in `src/truth.rs` and is emitted into
the macro as a `TT_*` VBA constant. **Measured 2026-11-16** from the four
direction probes' runtime reports (all green, exit 0); `0` remains the
UNMEASURED sentinel and every runtime decision that consults an unmeasured
cell refuses with a named reason naming its probe - a step whose direction
semantics cannot be established fails and never reports `created`.

| Constant | Meaning | Evidence |
|---|---|---|
| `TT_EX_DIR_TRUE_SIDE = -1` | `FeatureExtrusion2`'s `Dir=True` sends material toward TT * n_sk | probe-extrudedir: Dir=True put the blind 0.012 m box at x in [-0.012, 0] on the verified Front frame (n_sk = +X) = MINUS the normal; Dir=False at [0, 0.012]; D1 identity in SI metres |
| `TT_EX_FIRST_SIDE = +1` | with the two-sided call shape (`Dir=False`), the first depth argument's end sits toward TT * n_sk | probe-twosided: first 0.010 / second 0.030 put the box at x in [-0.03, 0.01] - the FIRST end on the PLUS normal side; D1 = 0.010, D2 = 0.030 confirm argument identity |
| `TT_EX_THROUGH_ALL_DIR = -1` | the through-all call shape's `Dir=True` reading | probe-throughall, measured directly (its own cases, never inferred from blind): Dir=True gave x in [-1, 0], Dir=False [0, 1] - through-all obeys the Dir flag like blind. The probe's contract allowed an inconclusive result; the fresh part produced measurable bodies both ways. No D1 dimension exists on a through-all feature |
| `TT_REV_SENSE = +1` | with the blind revolve call shape (`ReverseDir=False`), the sweep is the right-hand rotation by +angle about the drawn axis direction (start to end) when +1 | probe-revolvedir blind-270 cases: ReverseDir=False's centroid at (u,n) = (-0.0046, -0.0046) = mid-angle -135 deg = toward MINUS the normal first = right-hand +angle about the drawn +t axis (t_hat x u_hat = -n_hat); ReverseDir=True mirrored. Centroids via MassProperty `centerofmass` (American spelling; `centreofmass` raises 438) |
| `TT_REV_TWO_FIRST_SIDE = +1` | with the two-sided revolve call shape (`ReverseDir=False`), the first angle sweeps with right-hand sense TT about the drawn axis direction | probe-revolvedir twosided-100-150: D1 = 100 deg, D2 = 150 deg (radian identities), centroid at mid-angle +24.6 deg -> span [-100, +150]: the first angle on the MINUS-normal side = the same right-hand +angle sense as the blind form (two independent measurements agreeing) |
| `TT_REV_PROBE_CHIRALITY = -1` | the evidence-scope gate: the configuration chirality sign((drawn axis direction x profile radial side) . sketch normal) the probe establishes | probe-revolvedir's pinned setup: drawn axis (0,-0.010)->(0,0.080) = +t, profile on the +u side, n_sk = +X -> (t_hat x u_hat).n_hat = -1 |
| `TT_PRIM_DIR_TRUE_SIDE = -1` | the primitive blind call shape (byte-identical to the extrude blind form): the height extends toward TT * n_sk for `Dir=True` | probe-primorient box-dir-false/box-dir-true (2026-11-17): on the verified Top frame (u=(0,1,0) v=(-1,0,0) n=(0,0,1), the non-identity map m00=0 m01=-1 m10=1 m11=0 side=+1) Dir=True put the blind 0.012 m box at z in [-0.012, 0] = MINUS the normal, Dir=False at [0, 0.012]; D1 identity and SystemValue in SI metres. The reading equals TT_EX_DIR_TRUE_SIDE on this second frame (the cross-check the evidence scope required) and the emitter's composition (wantSide = +1 * gMapSide) held end to end: side +1 resolved to Dir=False and landed the height at model +Z, the SC side. Evidence scope: the measured positive-normal source-XY configuration only - never proof of the reflected case (see the primitive rows below) |

The revolve rows are read under two candidate structural laws (the flag
selects an absolute side in units of the normal, or a right-hand sense
about the drawn axis direction): the four probe cases share one frame and
one directed axis and cannot distinguish them. The two readings produce
**identical flags** exactly where the configuration chirality `gMapSide *
sigma` equals `TT_REV_PROBE_CHIRALITY` - that scope is the executable
gate (the plan review's evidence-scoped restriction): directional revolve
forms (blind partial, two-sided) run there under either law and refuse
with a named reason outside it. Full turns and mid-plane forms are
direction-free and consult no cell. The sweep-direction seat-acceptance
check remains the outer guard for the residual structural hypotheses
(e.g. whether the flag follows the drawn endpoint order or a
host-canonicalized axis direction).

Source-side rules the adaptation applies on top of the constants
(source-pinned from SolidCraft's own sources):

- **Extrude sides** (#2700 `extrude_convention_tests`): the first end is
  `e1 = direction-sign * distance`, the second `e2 = -direction-sign *
  distance2`; a negative distance turns its own side over and the body
  spans between the two ends. In units of `n_sk` an end at `e` sits toward
  `sign(e) * gMapSide` - the map side (the map's orientation) is the only
  map entry the extrude needs, so any valid adaptation map works, not only
  signed permutations.
- **Revolve sweep** (`kernel/src/build.rs` `canonical_revolve_axis` /
  `revolved`): partial turns go toward the side the sketch normal points
  to, independent of the axis direction sign and of which side the profile
  lies on. The emitter classifies the profile's side of the axis
  (`truth::profile_side`, from the NON-CONSTRUCTION profile geometry only -
  line endpoints and circle centres with their radii; construction axes,
  unused points and a circle's extent beyond its centre are accounted for)
  and refuses with the reason named when the profile straddles the axis or
  no side exists. The axis is mapped through the sketch's map **like
  geometry** (both helper-axis endpoints go through `MapU`/`MapV`, so the
  drawn direction is the mapped source direction) which makes the sweep
  factor pass through the map unchanged - again any valid map works. The
  helper axis's mapped placement is read back after a solve: evidence, not
  assumption.
- **Revolve angles** (`doc/src/eval.rs` `revolve_extent_for_handle`): a
  blind partial sweeps `[0, angle]` from the sketch plane toward the
  source normal (a negative angle turns its own sweep over); two-sided is
  the interval `[-angle2, +angle]` - `angle` on the normal side, `angle2`
  "turned back from the profile" - so BOTH angles must be positive and a
  signed combination is refused before feature creation, never folded into
  an opposite-sided pair; symmetric splits the angle either side "whichever
  way the axis points"; a full turn (360 deg) is convention-free and
  consults no cell.
- **Primitive height** (batch 3; source-pinned kernel semantics): the
  SolidCraft box spans `corner.z..corner.z + height` and the cylinder
  `base..base + height * axis` with the axis pinned to +Z and sizes
  positive (`parse_prim`), so on the base plane the height extends toward
  the XY source normal. In units of `n_sk` its end sits toward
  `+1 * gMapSide` and the call flag is
  `dirFlag = (wantSide = TT_PRIM_DIR_TRUE_SIDE)` - the extrude blind
  composition, on a call shape byte-identical to the extrude family's.
  The cell is MEASURED (-1, probe-primorient 2026-11-17; the reading
  equals the extrude cell on a second frame and the composition held end
  to end there) and the zeroed-cell guard stays in the macro: while it
  reads 0 every primitive refuses with a named reason naming its probe.
  The
  probe measures the XY plane's positive-normal configuration on this seat:
  it cannot distinguish a transform-relative sketch normal from a
  host-canonicalized normal that agrees there, and `gMapSide = -1` is
  unreachable on the owner's seat (every origin plane publishes a
  permutation map with side +1). Filling the cell is evidence for the
  measured configuration only - never proof of the reflected case, which
  stays an accepted residual under the seat-acceptance outer guard like
  the extrude family's composition on unmeasured frames.
- **Direction-free forms**: symmetric (mid-plane) extrude and revolve and
  full-turn revolves span both sides, so no direction decision exists -
  their builders never consult the truth table. Their single argument is
  the total depth/angle: the mid-plane revolve's TOTAL reading is now
  evidence-confirmed (probe-revolvedir `midplane-270`: the volume equals
  the blind 270 deg case exactly - 8.4823e-05 - and D1 read back the
  passed 270 deg, so the mid-plane angle is the whole turn, matching the
  documented call shape); the symmetric extrude's mid-plane depth scale
  rests on the same documented semantics with its D1 echo readback - no
  probe case covers it (recorded gap).

## Revolve (tracer phase)

| SolidCraft | SolidWorks API | Checkpoint-verified |
|---|---|---|
| `type: "revolve"`, `profiles: {"type":"all"}` | select the profile sketch plus the axis, `FeatureManager.FeatureRevolve2` | pending |
| `axis: {"sketch_line"}` | the profile's own line as the axis (already drawn mapped; its endpoints went through `MapU`/`MapV` at draw time) | pending |
| `axis`: `sketch_axis` x/y, world axes in-plane, `{origin, dir}` | an anchored helper construction line drawn through the sketch's map like geometry, with its placement read back | pending |
| `angle` blind partial (`convention: "sketch_normal"`) | sweep `[0, angle]` from the sketch plane toward the source normal; `ReverseDir` from the truth table | pending |
| `angle` = 360 deg | full turn; convention-free, no truth-table cell consulted | pending |
| `angle` + `angle2` | two-sided, interval `[-angle2, +angle]`; the angle arguments (and their D1/D2 dimension identities) swap per the truth table; non-positive angles refused | pending |
| `direction: "symmetric"` | mid-plane revolve; `angle` is the whole turn split either side | pending |
| a profile that crosses its revolve axis | **refused named** before the call on every form - the source kernel's own unconditional rule (`revolved()`: "the profile crosses the revolve axis", any angle) | n/a |
| `to`, `convention: "legacy_axis_start"` on a partial turn, model/construction axes, edge/face axes, `profiles` other than `all` | **stop rule** (unsupported in the tracer phase) | n/a |

Revolve verification: the feature must exist and its angle dimension(s) must
read back within tolerance (each angle against the side that carries it
after the swap). The sweep direction and the volume of partial revolves are
owner-seat acceptance checks.

## Document handling

The macro always creates a **fresh part** from the user's default template
(`GetUserPreferenceStringValue(swDefaultTemplatePart)` → `NewDocument`). It
never touches an active document: an opt-in for building into an open document
is a later feature. No template configured = clear failure, reported.

## Runtime report (JSONL, written by the macro)

One line per JSON object, written incrementally, in the operator's directory
only (path from `CRAFT_BRIDGE_REPORT`, set by `tools/seat-smoke.ps1`):

1. `{"type":"header","macro":...,"bridge_version":...}` — first, before any
   model mutation.
2. `{"type":"step","index":n,"command":...,"status":"created|degraded|failed|not-executed","message":...}`
   — one per attempted step, flushed as it happens.
3. `{"type":"summary","status":"ok|failed"}` — **last**, the completion
   signal; its absence means truncation.

JSON string escaping in the macro is separate from VBA-literal escaping in the
generator; numbers are written with `Str()` (locale-independent). The plan
report (`build --plan`) is the offline **prediction** (statuses: mapped,
degrade-planned, fallback-planned, not-executed); the runtime report is the
**truth**. The two are never merged.

## Compatibility checkpoint (verified vocabulary)

`corpus/compat-check/minimal.swb` proves before the generator grows:
`Sub main` entry in `.swb`, `Application.SldWorks`, the default-template
`NewDocument` path, `SelectByID2` origin planes, `InsertSketch`, `CreateLine`
and `CreateCircleByRadius` in metres, `SketchAddConstraints "sgHorizontal"`,
`AddDimension2` + `Parameter("D1@Sketch1").SystemValue`, the JSONL report with
its completion signal, and failure handling that still writes one. The
"Checkpoint-verified" column above is filled from that run
(`docs/seat-acceptance.md` records the SolidWorks version).

Seat findings so far (first contact, SolidWorks revision 34.3.2 / 2026):

- The legacy **`RunMacro(path, "", "main")` is the verified invocation**;
  `RunMacro2`'s ByRef error argument raises TYPE_E_ELEMENTNOTFOUND /
  "Type mismatch" under both PowerShell and VBScript late binding, so the
  runner (`tools/run-macro.vbs`) tries RunMacro2 first and falls back.
- The macro runs **inside the SolidWorks process**: an already-open instance
  inherits an older environment, so `CRAFT_BRIDGE_REPORT` cannot be trusted to
  be visible. The report path therefore also travels as a one-line file under
  `%TEMP%`, which is set at login and always visible
  (`ReportPathFromHost` in the macro).
- A `.swb` RunMacro "success" means the VBA did not crash out — the Fail
  handler swallows errors, so only the runtime report proves the work
  happened. Never infer success from the invocation result.

Diagnostic recovery (the macro compiles, runs, and records attributed
failures) is established separately from full seat replay acceptance (the
rebuilt model matches the design): the first is a prerequisite for the
second, never a substitute.

**Report routing (runtime):** the report path travels in a one-line file
under the TEMP directory, rewritten before every run, and the macro reads
that file first; `CRAFT_BRIDGE_REPORT` is only the fallback for when the
file is missing or blank. A long-lived SolidWorks process can carry a stale
environment inherited from whatever launched it - preferring the
environment silently misrouted every report after a restart (2026-11-14)
and made sound runs look like failures.

**Equation manager (runtime):** on this seat (rev 34.3.2) the late-bound
`IEquationMgr.Add2` call rejected the two-argument form ("Argument not
optional"); the classic two-argument `Add` works, and so does `Add2` with a
third argument. The emitter calls `Add`, fails closed unless the returned
index is valid, and reads the stored equation and its evaluated value back
after the rebuild. This is observed late-bound behaviour on 34.3.2; the
help pages consulted (help.solidworks.com, `Add2` 2023 and 2026, `Add`
2025) show the two-argument `Add2` form.
The manager's `Count` member is likewise unsupported here (runtime 438,
confirmed by the eqdiag probe alongside working `Equation(i)`, `Value(i)`
and `EditRebuild3`): the insert-index upper bound is checked through the
`Equation(idx)` readback, which raises on an out-of-range index and fails
closed while also proving the stored text.
The equation manager reports evaluated values in the units the right-hand
side writes (eqdiag on 34.3.2: `"W1"=40mm` -> `40`; a unitless right-hand
side reads back as written). The global-equation check therefore compares
against `expect * evalScale` with the scale carried per dimension from the
planner: lengths `1000` (millimetres, demonstrated), counts `1`
(demonstrated), angles `0` = unestablished, which fails closed with an
explicit message rather than guessing a conversion.

**Document cleanup (runtime):** the owner rule (2026-11-16) is that test
runs leave no documents open - by coordinated sequencing, never a
close-all, and the authorization to close is bound to the current
invocation. Smoke runs write `close-after-run` on the third line of the
host path file and the macro closes its own document at the end; the
authorization is consumed on every path out of the run - success, failure
and timeout alike - when the run rewrites the path file back to its two
stable lines, so a later manual replay can never inherit it. Harness
runs leave the flag absent: acceptance checking needs the part open. The
macro always reports its owned title, and the harness hands exactly that
run-bound title to `tools/check-part.vbs`, which closes it after the last
check and refuses to close anything else - never falling back to whatever
is active. The macro's close mode is tri-state: `close`, `keep`, or
`unknown`; an unknown mode (an unreadable path file) reports a failed
close-mode step and a failed summary rather than silently keeping open.
Every closer refuses an empty or ambiguous title before the name-based
close (a live enumeration must find exactly one open document answering to
the captured title and it must be the macro's own object), uses the exact
returned title, checks every enumeration call immediately and stops as
unverified on any failure, verifies the close by the same enumeration
showing absence, and reports refusals as failures. An unverified close
fails the run.

