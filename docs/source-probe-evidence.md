# Source-engine probe evidence (owner's design agent)

Probe results produced against the live SolidCraft engine by the owner's
design agent. Filed here because they settle replay-contract questions
the source research left open. Newest first.

## 2026-11-17 - Sweep `path` list order is set-like (Phase C input)

**Probe:** `SweepOrderObliqueProbe` (two sweeps sharing a profile and a
two-segment path, one with `path: ["l1","l2"]`, one with
`path: ["l2","l1"]`, on an oblique 63.435-degree joint) plus the same
reversal applied to `SweepMicro` itself (90-degree joint). Comparison by
`inspect.compare` at 0.001 mm tolerance with dense sampling.

**Result:** forward and reversed lists produce IDENTICAL solids - largest
deviation 8.5e-14 mm (oblique) and 2.3e-14 mm (SweepMicro), volume,
bounding box and centre of mass all equal. The engine accepts the
permuted list without error and evaluates the same sweep.

**Established:** the serialized `path` array is not an ordered sequence
for evaluation - permutation does not change geometry on connected
two-segment paths. A faithful sweep replay must derive the path chain
from curve CONNECTIVITY (endpoint chaining), never trust list order.

**Not established (still open):** behavior on other path topologies
(branching, gaps, three-plus segments) and the semantics of reversing an
individual curve's direction (the `reversed` wrapper flag).

**Verdict change:** `SweepMicro` is corrected to **PASS** (its earlier
FAIL was the reversal-as-fault detector not applying). Its edit-follow
(PathLegA 26.2 -> 31.0 mm: max X +4.8 mm, volume 337.847 -> 378.695 mm3,
restored) and STEP parity (3.4e-16 relative) were always good.

**Source files (untracked, in the design delivery):**
SweepOrderObliqueProbe.json (sha256 26550e1c97f55f761560b557e027e5ea2ea1453cb3fcf481789c449b652e67f0),
SweepOrderObliqueProbe.calls.json (sha256 ed0cdccd84eee78515bbceba1ca2a856425e4c2c9677793392847893b2cc0ff0),
SweepMicro.validation.json (updated with `path_order_probe`).

**MCP command-shape notes from the call log** (for future builder
prompts; the command API differs from the serialized schema):
`sketch.line` takes `points` as a list of 2-positions plus `closed` and
`infer` flags; `sketch.constraint.fix` takes `entities` as point names
like `l1.start`; `solid.sweep` takes `path_sketch`, `path` (curve id
list), `operation: "new"` (the serialized enum writes `new_body`) and
`body_name`. `timeline.inspect` returns the serialized `definition`
under the feature's own field names.

## 2026-11-17 - probe-refplane (P01+P03): offset planes are fully pinned

**Probe:** `corpus/compat-check/probe-refplane.swb` (hand-written on the
minimal.swb skeleton), run through `tools/seat-smoke.ps1` on the seat
(swf 34.3.2). Final run: summary ok, all steps created.

**P01 established:**
- `InsertRefPlane(8, 0.012, 0, 0, 0, 0)` with a pre-selected origin plane
  (plain `Feature.Select(False)` = mark 0) creates the offset plane. The
  created feature is `Plane1` of type `RefPlane`, renamable via
  `IFeature.Name`.
- `IRefPlane.Transform` ArrayData(16) layout: **the origin lives at
  entries 9..11** (not 12..14); the offset plane at +0.012 read
  `[0,1,0,-1,0,0,0,0,1,0,0,.012,1,0,0,0]` - the rotation part is the
  plane frame, and the offset sign (+Z for Top) is recorded.
- The plane feature's display dimension name reads **`D1`** (feature-
  level; the qualified form is `D1@CB_Plane_A`).
- A sketch opens on the reference plane by selecting the **feature
  object** (`Feature.Select(False)` then `InsertSketch True`); a circle
  created at (0,0,0) is in SKETCH space - `SketchSegment.GetCenterPoint2`
  returns a SketchPoint object whose X/Y/Z are sketch-space (0,0,0 here),
  NOT model space.

**P03 established (the parametric chain):**
- `Dimension.SystemValue` (metres) drives the plane: 0.019 moved it.
- The full equation chain works: global equation `"CB_Off" = 23mm` +
  dimension equation `"D1@CB_Plane_A" = "CB_Off"` - **both name forms
  (qualified and bare) are accepted and drive**; the stored text reads
  back exactly (the canary discipline: readback + driven-value check).
- **Unit rule (empirical): a BARE number in equation text is in DOCUMENT
  units** - `"CB_Off" = 0.023` drove the dimension to 0.023 mm
  (0.000023 m), not 0.023 m. Unit-suffixed text (`23mm`) is unambiguous.
  This confirms the emitted-macro convention of unit-suffixed literals.
- **Sketch associativity holds**: after driving the plane to 0.023, the
  sketch's `ModelToSketchTransform` origin read (0,0,-0.023) - the sketch
  follows the plane, and the inverse-origin math of the frame map
  (translation = negative plane offset along the normal) checks out.

**Host quirks discovered (VBA inside SolidWorks):**
- `Sqr(x)` raises "Invalid procedure call or argument" on a **zero**
  argument in this host (reproduced twice; `SafeSqr` guards it - the
  probe keeps the helper).
- `Str(NaN)`/`Sqr(NaN)` likewise raise err 5; `SafeNum` guards reporting.
- A trailing `;` on a VBA statement is a compile error (VBScript habit);
  an undeclared variable under `Option Explicit` compiles the macro out
  of existence silently (RunMacro "returned: failure" with no report).

**Replay consequence:** the offset-plane implementation (docs A1 of the
SW research) now has every blocker answered: the emitter can create the
plane with `InsertRefPlane(8, distance_m, 0,0,0,0)`, rename it, bind its
`D1@<name>` dimension to a global equation with unit-suffixed text, and
open sketches on the feature object - and the frame-map translation rule
is confirmed for offset planes.

## 2026-11-18 - probe-prefvector: preference-vector hygiene proven

**Probe:** `corpus/compat-check/probe-prefvector.swb` (the generated
exact-draw helpers verbatim + an 18-bit vector recorder; two byte-review
rounds folded, final APPROVE), run through `tools/seat-smoke.ps1`.
Summary ok. Records:
- S-success: vector 111111111111111010 before = after (equal=1); L2 end
  placed at .0116,.01,0 EXACTLY under BeginExactDraw/EndExactDraw.
- F-drawfail: the deliberate vbObjectError+560 (-2147220944) captured,
  restored through the DrawFail discipline, vector equal=1.
The bit pattern independently confirms research 08's factory defaults:
entries 15 (swSketchSnapsGrid) and 17 (swSketchSnapsAngle) read 0 while
the other 16 read 1.

**Compile lesson (filed with the Note collision):** the first two
probe-prefvector variants failed to load (RunMacro failure, no report).
A subtractive bisect (probe-snap v6 + the generated helpers = green)
isolated the breaker to RunCaseFail's body; rewriting it with byte-
proven constructs (the cErrNum/cErrMsg names, single-line gates, an
expectedErr flag) loaded green. The exact offending construct was not
pinned; the working form is kept.

## 2026-11-18 - probe-snap: CreateLine endpoint inference and the switch that stops it

**Probe:** `corpus/compat-check/probe-snap.swb` (hand-written on the
minimal/probe-refplane skeleton; five byte-review rounds folded, final
APPROVE), run through `tools/seat-smoke.ps1` (swf 34.3.2). Summary ok,
six cases, all toggles captured pre-state and restored verified.

**QS1 established:**
- **The inference reproduces exactly**: a macro `CreateLine` endpoint
  whose target u sits ~0.6 mm from an existing line's midpoint u is
  PLACED AT THE MIDPOINT U (v untouched). Both accessors agree
  (SketchPoint and the Double() arrays). Control (target far from every
  midpoint) places exactly.
- **`swUserPreferenceToggle_e.swSketchInference` (249) OFF during
  drawing stops the snap**: every endpoint lands exactly. This is the
  fix the emitter uses (capture pre-state, switch off, readback-enforced,
  draw, restore verified on every exit).
- `swSketchSnapsMidPoints` (268) OFF alone does NOT stop it (the snap
  persists) - the master inference switch is required.
- `ISketch.AddToDB` and `ISketch.EnableSolving` are **absent** on this
  host (late-bound err 438, recorded as data; the enum values came from
  the seat's `SolidWorks.Interop.swconst.dll` by reflection).
- `ISldWorks.SetUserPreferenceToggle` returns **VOID** (reflection on the
  interop): a setter result cannot be captured - the enforced readback IS
  the setter verification. (v5 of the probe failed to compile on exactly
  this.)

**Not established / open:** the trigger threshold's exact value (~0.6 mm
observed); whether other inference kinds (nearest/tangent/intersection)
snap API-created endpoints similarly; whether the documented surface has
a per-call suspension (research brief 08).

## 2026-11-18 - probe-dimkind: dimension-kind creation is pinned

**Probe:** `corpus/compat-check/probe-dimkind.swb` (hand-written on the
minimal.swb/probe-refplane.swb skeleton; plan review APPROVE WITH CHANGES
folded, byte review APPROVE), run through `tools/seat-smoke.ps1` on the
seat (swf 34.3.2). Report: 17 steps, all created, summary ok. Geometry is
independently authored synthetic data (13x8 mm and 8x13 mm diagonals,
axis-pure controls) - never client geometry.

**QD1 established:**
- `AddDimension2` with two live sketch point handles selected creates an
  **ALIGNED** dimension for a diagonal pair at EVERY witness placement the
  emitter's heuristic family can produce (midpoint-over/below/above,
  side-left/right): five placements, five aligned dimensions (value
  15.264 mm = sqrt(13^2+8^2) mm exactly). **The witness placement never
  determines the dimension kind.**
- `AddHorizontalDimension2` / `AddVerticalDimension2` exist and are
  **deterministic**: horizontal always measures the sketch-u projection,
  vertical the sketch-v projection - across swapped slopes (13x8 and
  8x13), reversed point-selection order, and axis-pure controls. For an
  axis-pure pair the opposite-axis member returns Nothing with err 0 (a
  zero-extent axis dimension does not exist); that is data, not a failure.
- **Parameter-change acceptance (the emitter's value-edit contract)
  holds** for the explicit members: driving the value by +0.002 moved
  exactly the measured projection and restored exactly, leaving the
  perpendicular projection unchanged. The aligned dimension at the old
  heuristic moved BOTH axes proportionally (13->14.703, 8->9.048 mm for a
  +2 mm drive along the line) - the reason an aligned dim must never
  drive a coordinate driver.
- Both explicit members were invoked late-bound through an `Object`
  alias; the members are present in the type library at runtime (err 0).

**Emitter contract pinned from the evidence (src/swb.rs):** the
classified-axis (DistanceX/DistanceY) arm dispatches creation on the
RUNTIME axis class (`AxisClass`) - class 0 -> AddHorizontalDimension2,
class 1 -> AddVerticalDimension2 - through the late-bound `docO` alias,
never on the declared kind and never on the witness placement. Every
fail-closed guard is unchanged: the classified-axis projection check, the
wrong-axis-kind check, the projection-magnitudes-coincide refusal, the
sign/value checks and the value-edit carried-the-geometry checks. All
other dimension arms (Distance, Length, Radius, Diameter, Angle) keep the
AddDimension2 creation byte for byte (tests/build_tests.rs
`non_classified_axis_dims_keep_the_add_dimension2_creation`;
tracer_macro_is_pinned unchanged).

**Integration case (emitter output on the seat):**
`corpus/frame-diagonal-dims/` - a generated macro whose two driver dims
span a diagonal pair and one axis-pure control (chained parameter
equation included). **Seat-verified 2026-11-18** (task craft-diag-dims,
seat-smoke, committed hash b6e275e0...): "6 relations and 3 dimensions
replayed", summary ok - all three classified-axis dimensions created
through the explicit members and every kind/value-edit guard passed.

**Not established / open:** the seat evidence shows occasional
no-movement on the probe's immediate value-edit re-read (b-e and most
fv attempts recorded no geometry movement although the value edit
itself did not raise). The emitter's value-edit checks may therefore
occasionally fail closed on solver timing; if a run trips on "the value
edit did not carry the ... geometry", that is the next probe question
(rebuild-before-reread vs immediate read).

## 2026-11-19 - probe-typed-arrays + probe-massprops: the centroid array is a typed SAFEARRAY(Double)

Synthetic evidence only (the WSH engine probe runs anywhere; the
SolidWorks probe imports the committed synthetic
`tests/fixtures/box.step`, an independently authored 158-entity AP203
box - one manifold solid, volume 0.000006 m3 = 30x20x10 mm, geometry
gate pass, verified close pass). No real-part data appears here.

**probe-typed-arrays (WSH engine, no SolidWorks):** `Array(1.25,2.5,3.75)`
-> VarType 8204 (VT_ARRAY|VT_VARIANT), TypeName `Variant()`, indexing
works (items vbDouble). `Scripting.Dictionary.Keys` -> 8204, indexing
works. `ADODB.Stream.Read(3)` -> **VarType 8209 (0x2009) TypeName
`Byte()`** - bounds 0..2 fine, **every `a(i)` raises err 13 Type
mismatch**, and `For Each` raises **err 451 (Object not a collection)**
after one item. This is Microsoft KB 165967's documented rule on this
engine: VBScript indexes SAFEARRAY(VARIANT) only; bounds inspection
works on typed arrays; For Each is NOT a workaround.

**probe-massprops (seat, SolidWorks 2026, synthetic box):**
`IMassProperty.CenterOfMass` and `IMassProperty2.CenterOfMass` both
arrive as **VarType 8197 (0x2005) TypeName `Double()`** - a typed
SAFEARRAY(Double), zero-based 0..2, one dimension. Every element access
form fails **err 13** (plain assignment, Set, VarType(element),
TypeName(element)). `For Each` fails **err 451** after one item.
`GetBodies2(0, False)` bounds-count works but its elements are
unindexable the same way. `GetMassProperties2(1, Status, False)` (the
documented `Accuracy, ByRef Status, UseSelected` shape) fails **err 13**
from VBScript - the ByRef Status is the seat's known uncallable shape
(the RunMacro2/LoadFile4 precedent). `CreateMassProperty2()` +
`Recalculate()` (True) + `UseSystemUnits` all work; volume scalars read
fine on both interfaces.

**Conclusion pinned:** no VBScript access form can read the centroid
triple on this seat (research 11's hypothesis confirmed at the wire).
Acquisition moves to a typed adapter (PowerShell + the SolidWorks interop
assembly: real `double[]` indexing, `[ref]` status for
GetMassProperties2, cross-check CenterOfMass against
GetMassProperties2[0..2]); the checker keeps every verdict and validates
the adapter's text strictly. All default-property/.Value coercion is
gone from the acquisition path (research 10/11's rule).

## 2026-11-17 - `fixed: true` + `exprs` is creatable through the MCP

Source-confirmed earlier (nested-schema research [S38]); the design
agent's D02/D10 fixtures were created exactly this way and re-open
correctly, closing open issue 3 of the nested-schema research for these
cases (saved `pos` matches `exprs` evaluation in the delivered files -
though stale-pos remains possible in general per [S34]-[S36], and the
loader's expr-wins rule covers it).
