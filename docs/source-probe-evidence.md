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

## 2026-11-19 - typed mass-adapter qualification (synthetic fixtures)

Synthetic only: the corpus comparison fixtures (corpus/box-compare and
corpus/box-compare-shifted - independently authored box recipes, 30x20x10
mm, known centroid (15,10,5) mm; the shifted reference is the same box
translated +20 mm X). Measured on the seat through the checker ->
mass-adapter path:

- **CenterOfMass marshaling (final answer):** the typed interop metadata
  path returns `System.Object[]` of BOXED `System.Double` values; the
  VBScript wire shows `VT_ARRAY|VT_R8` (VarType 8197). The accepted
  contract is exact rank-one, zero-based, length-three vectors with
  strictly Double elements (double[] or boxed-Double Variant arrays);
  every other element type is a named refusal. The original `double[]`
  hypothesis is formally revised (research 11's revise clause).
- **PowerShell COM binding traps, both named:** the language cast of an
  RCW to an interop interface refuses when the assembly came from
  LoadFrom ("Cannot convert System.__ComObject to ISldWorks"), and
  PowerShell's binder mistakes a 0-arg method on an untyped COM return
  for a property then indexes it (DISP_E_BADINDEX at GetTitle - which
  VBScript's combined method/property flags mask, explaining the
  checker's working `d.GetTitle`). The reliable path is invocation
  through the interop metadata (MethodInfo/PropertyInfo Invoke, QI by
  IID).
- **GetMassProperties2** (Accuracy, ByRef Status, UseSelected) is
  callable through the metadata path with the ByRef status traveling in
  the Invoke args array; its flat mass vector carries the documented
  twelve-double layout (thirteen accepted as the legacy sibling variant;
  volume at index 3 in both).
- **Fixture discriminator proven end to end:** box-compare reports
  `centroid distance 0 m` placement=pass with the full verdict set green;
  box-compare-shifted reports `centroid distance 0.02 m ... past the
  0.0001 m tolerance` placement=fail and nothing else fails. A
  defaulted-zero-triple adapter would wrongly pass the shifted case, so
  the pair qualifies the acquisition.

## 2026-11-23 - probe-mergescope: Phase B P0 merge/scope (synthetic fixtures)

Independently authored synthetic three-box fixtures (30x20x10 mm,
20x20x10 mm overlapping, 10x10x10 mm control; volumes 6e-6/4e-6/1e-6
m3, centroid distances 10/26.925824/32.015621 mm, analytic overlap
3e-6 m3) on fresh parts per case, bodies identified by volume
fingerprint. All measurements VBA-side through IPartDoc.GetBodies2 and
IBody2.GetMassProperties; run through the provenance-checked seat-smoke
runner.

Measured semantics (all fixture gates passed first; controls invariant
in every case):

- **The marks form is THE combine form**: `Select2(append, SelectData)`
  with the mark in the SelectData (the `Select(append, mark)` legacy
  form also works; `Select2(append, mark)` raises err 13 - the reflected
  signatures are `Select2(Append, SelectData)` / `Select(Append, Mark)`).
  The mark roles: cut = target mark 1 + tool mark 2; add/intersect =
  every operand as tool mark 2 (the companion to the documented
  `MainBody = Nothing` rule).
- **The enum families pair exactly**: creation `swBodyOperationType_e`
  (add 15903 / cut 15902 / intersect 15901) reads back as
  `swCombineBodiesOperationType_e` (0 / 1 / 2) through
  ICombineBodiesFeatureData.OperationType under
  AccessSelections/ReleaseSelectionAccess. Readback also confirms the
  operand shape: cut carries MainBody=1 and BodiesToCombineCount=1;
  add/intersect carry MainBody=0 and count 2.
- **Volumes are exact per the fixture arithmetic**: add 7e-6
  (6+4-3), cut 3e-6 (6-3), intersect 3e-6 (the overlap). Controls
  invariant at 1e-6 everywhere.
- **The array form is UNCALLABLE**: `InsertCombineFeature` with a
  Variant array of Body2 returns Nothing for every operation (the
  documented-by-research-12 but unverified marshaling path - confirmed
  uncallable here). The emitter must use the marks form.
- **Join extrude (the plaque's gate), FeatureExtrusion2 arg 18/19 =
  Merge/UseFeatScope**: Merge=True into a single touched target is exact
  (target 6e-6 + joined material 1e-6 = 7e-6, count 2, control
  invariant); Merge=False yields a separate new body (count 3); with TWO
  touched candidates and UseFeatScope=False, the merge COALESCES every
  touched body into one result (only untouched controls stay separate) -
  the measured semantics research 12 left open. The scoped variant (UseFeatScope=True with
  auto-select) coalesces IDENTICALLY: target-selectivity must come from
  which geometry the profile touches (or explicit scope-body selection,
  the P1 follow-up). The plaque's reliefs touch only their backing - the
  proven single-target case.

Battery verdict (the reviewed postconditions): the marks form carries
correct operation, target identity, volume, count and untouched
controls in every case; the enum families pair correctly; the join
semantics are established. The array form's named outcome is the
required form-comparison finding. P0 merge/scope: SATISFIED (P0 body
registry, edge-resolver and expression placement remain; the P1 probes
gate emission together with P0).

The probe also recorded two compile-layer lessons for hand-written probe
VBA: typed VBA rejects `ModelDoc2.GetBodies2` (it is `IPartDoc`'s -
the same owner correction research 12 carries), and an undefined helper
call is a module-load compile error that RunMacro reports only as
"invocation returned: failure" - the mechanical bisect (committed
variants, single-definition proc scans, name-resolution lints) is the
working discovery route.

## 2026-11-17 - `fixed: true` + `exprs` is creatable through the MCP

Source-confirmed earlier (nested-schema research [S38]); the design
agent's D02/D10 fixtures were created exactly this way and re-open
correctly, closing open issue 3 of the nested-schema research for these
cases (saved `pos` matches `exprs` evaluation in the delivered files -
though stale-pos remains possible in general per [S34]-[S36], and the
loader's expr-wins rule covers it).

## probe-exprplacement (P0 3/4, SATISFIED 2026-11-23) — seat evidence

Verdict run `probe-verdict ok` (task craft-probe-exprplacement, commit
3b19114; seat report probe-exprplacement/runtime-report.jsonl). Fixture: a
carried-corner box (carrier line from the FIXED sketch origin to the rect
corner, corner shared via sgCOINCIDENT, carrier marked construction
geometry) with two equation-driven globals (`"myPos" = 15mm` driving the
carrier-length placement dim, `"myW" = 20mm` driving the rect-width size
dim, both via the emitter's `eqMgr.Add(-1, ...)` discipline with stored-text
readback), plus a static 10x10x10 mm control body. Fixture gate verified
volumes 3e-6 / 1e-6 m3 and analytic centroid distance 0.034003676 m.

- **EP1 placement is parametric**: `myPos` 15mm->25mm moves the body
  (D1=.025 exact); the size dim D2=.02, driven volume 3e-6 and the control
  all invariant. A static coordinate snapshot is refuted.
- **EP2 size edits are isolated**: `myW` 20mm->30mm -> D2=.03, volA
  4.5e-6 exact; D1 stays .025; control invariant.
- **EP3 the dependency survives save/close/reopen**: after the reopen an
  INDEPENDENT placement edit (`myPos` 25mm->20mm) still drives D1=.02 with
  D2=.03 and volA 4.5e-6 invariant. (DEPENDENCY_LOST is refuted for this
  form.)

Named outcomes implemented: PLACEMENT_NOT_PARAMETRIC, DEPENDENCY_LOST.
Consequence: the Phase B expression layer may treat primitive placement as
parametric for equation-driven placement parameters in this form; Pierre's
first blocker (research 12 P0 row, cross-report observation C) is cleared.

### Probe-iteration lessons (VBA/COM on the seat)

- `CreateCornerRectangle` returns an ARRAY of segments: `Set` on it is
  type mismatch (13), and `Is Nothing` on an array raises 424. Use a plain
  Variant assignment and `IsArray` guards.
- A line-segment select "succeeds" as an extrude profile selection but
  hands FeatureExtrusion2 no contour (it returns Nothing): the profile is
  the SKETCH (the sketch object or SelectByID2 fallback).
- A stray non-construction line in the profile sketch gives the profile
  detector two contours and the extrude finds none. Carrier lines must be
  construction geometry (`seg.ConstructionGeometry = True`).
- `InsertSketch` toggles: after dimension/constraint/equation operations
  the sketch may already be closed; a blind close RE-OPENS an empty sketch.
  Guard every close (`If Not ActiveSketch Is Nothing Then`).
- Two `Dim sk` lines in one procedure = Name redefined = module-load
  failure; blind string replaces must be asserted and the dup scan run.
- Equation edits (`eqMgr.Equation(idx) = text`) work and read back
  verbatim, matching the emitter's discipline.

### Shared-session hygiene (the phantom-document case)

The probe's cleanup walk counted four documents mid-run that no external
PowerShell walk could see: real unsaved PartDoc fixtures of FAILED probe
runs (before their finish-path close existed). Root causes and rules now
in force: (1) `New-Object ...SldWorksClass` in a scheduled PowerShell task
does NOT see the macro's instance (the binder-attach trap, same family as
the mass-adapter saga) - only the in-process VBA walk is authoritative;
(2) concurrent probe instances must never share a save target - every run
saves to a timestamped path; (3) launches are strictly serialized (one
macro at a time, wait for the report); (4) `corpus/compat-probe/cleanup-
strays.swb` (committed, narrow: exact observed titles, unsaved-only,
refuses entirely if any saved document is open) restores the clean-session
precondition; probes themselves never close documents they did not open.
(5) the finish-path close must drop its handle (`Set swDoc = Nothing`) or
the ownership check refuses a second time on the stale handle.

## probe-edgeresolver (P0 4/4, SATISFIED 2026-11-23) — seat evidence

Verdict run `probe-verdict ok` (task craft-probe-edgeresolver, commit
edcbe1a; seat report probe-edgeresolver/runtime-report.jsonl). Fixture: a
30x10x10 mm main box (3e-6 m3) whose four long edges are near-identical
twins, plus a 10x10x10 mm control body (1e-6 m3, analytic centroid
distance 0.04 m). Geometric matcher: endpoints via the reflected
IEdge.GetStartVertex/GetEndVertex + IVertex.GetPoint chain, length 1e-6 m,
|direction dot| > 0.999 (orientation-free), pick-point-to-segment distance
6 mm, candidates restricted to the volume-fingerprinted target body.

- **ER1 exactly one match**: the unique query resolves the intended edge
  (endpoints 0/0/0 -> 0/.03/0) at baseline and after the supported
  rebuild (-> 0/.04/0). EDGE_NOT_FOUND / EDGE_AMBIGUOUS are the named
  refusals; the lowest index is never chosen.
- **ER2 the ambiguous twin query refuses**: a pick point equidistant
  (5 mm) from two twin long edges matches exactly 2 candidates and the
  resolver refuses to choose (EDGE_AMBIGUOUS enforced).
- **ER3 persistent-reference round-trip**: GetPersistReference3 capture
  (shape recorded: vartype 8209 = VT_ARRAY|VT_UI1, 365 bytes - recorded
  only, NEVER compared as a raw equality oracle), GetObjectByPersist-
  Reference3 resolve with the status code checked (0 = Ok), identity
  established by re-measuring the resolved entity's geometry. The
  garbage-reference negative refuses with status 1
  (swPersistReferencedObject_Invalid = PERSIST_RESOLVE_FAILED as
  required). The baseline reference SURVIVES the upstream dimension edit
  (30->40 mm) and resolves to the UPDATED geometry (length .04) on the
  fingerprinted body.
- **ER4 checked body ownership**: the matched and the resolved edge both
  carry IEdge.GetBody fingerprints of the target body at every step
  (EDGE_WRONG_BODY named).

Consequence: the Phase B edge resolver contract (research 12 P0 row, T1)
holds on this seat for this form: geometric matching with a hard
exactly-one rule, refusal on ambiguity, and persistent references that
survive supported rebuilds with entity-test identity. Pierre's fillet
gate (P1 D06) can build on this.

### Iteration lessons (edge probe)

- IVertex's point getter is `GetPoint` (returns Object/array).
  `GetPoint3` does not exist (438). Reflection against
  `api\redist\SolidWorks.Interop.sldworks.dll` (LoadFrom path used by
  mass-adapter.ps1) settles member names before the seat run - the
  interop is not loadable by assembly NAME in a scheduled task.
- `Err.Clear` + `Err.Number` checks do NOT catch raises unless the
  procedure carries its own `On Error Resume Next`: without it a raise
  propagates to the caller's handler and the checks never run. Every
  helper that inspects Err.Number must own its error context.
- The default part template's first plane maps sketch (u,v) to model
  (y,x) on this seat: model-space queries must follow the MEASURED frame.
  The fixture gate now pins both body centroids individually so a
  template change fails loudly at the gate.
- Reflected IEdge surface confirms GetStartVertex/GetEndVertex/GetBody/
  GetCurve/GetCurveParams2/3/GetClosestPointOn (GetClosestPointOn is
  available for pick-point matching in production code).

### Frame correction (2026-11-23, from probe-moveremove's gate)

The template frame note above ("sketch (u,v) -> model (y,x)") was an
UNDER-DETERMINED reading: the edge-resolver fixture's cross-section was
square (10x10 mm), so v->x and v->z were indistinguishable there (the
identical centroid pins and twin-query geometry held either way, so all
edge-resolver results stand). The move fixture's non-square section
(30x20 mm) settled it: the default template's first plane maps sketch
u->+y, v->+z, extrude blind ->+x (the first plane is the YZ/Right
plane). Every model-space query must follow the MEASURED frame; fixture
gates pin individual centroids on NON-SQUARE sections so this class of
ambiguity fails loudly at the gate.

### API correction (2026-11-23): InsertMoveCopyBody2 parameter order

The research-12 reconstructed signature put `TransDist` FIRST. The
official parameter descriptions ("TransZ: Value for delta Z; applies to
translation"), the documented parameter list of the older
`InsertMoveCopyBody` (TransX: TransY: TransZ: TransDist: ...), and real
VBA call sites all establish the actual order:

    (TransX, TransY, TransZ, TransDist, RotPointX/Y/Z, RotAngleX/Y/Z,
     BCopy, NumCopies)

Seat evidence: a call with the delta vector in the TransDist slot
(created-but-rebuild-error) vs the corrected order (exact displacement
verified against the analytic oracle). The probe records the confirmed
order as the emitter's input contract; the research output stands as
filed, with this correction living in the evidence record.
