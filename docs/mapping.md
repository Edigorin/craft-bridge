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

Replayed as generated sketches on the Front plane plus a blind extrude.
Placement must be literal and on the base plane; sizes keep their
parametrisation through dimension equations. Off-plane placement and non-Z
axes stop (they need extrude start-offset work).

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
| `points[]`: `{"id", "pos": [u, v] (mm), "fixed"}` | entity endpoints drawn at `u/1000, v/1000, 0` metres | yes |
| `curves[]` line: `{"id","type":"line","a","b","construction"}` (a/b: **indices into points[]**) | `SketchManager.CreateLine(x1,y1,0,x2,y2,0)` | yes |
| `curves[]` circle: `{"id","type":"circle","c","r","construction"}` | `SketchManager.CreateCircleByRadius(cx,cy,0,r)` | yes |
| constraint `coincident` `{"p","q"}` | select both `ISketchPoint`s → `SketchAddConstraints "sgCoincident"` | pending |
| `horizontal` / `vertical` `{"l"}` | select the line segment → `"sgHorizontal"` / `"sgVertical"` | pending (checkpoint covers horizontal) |
| `parallel` `{"a","b"}` | select both segments → `"sgParallel"` | pending |
| `perpendicular` `{"a","b"}` | select both → `"sgPerpendicular"` | pending |
| `tangent` `{"a","b"}` | select both → `"sgTangent"` | pending |
| `equal` `{"a","b"}` | select both → `"sgEqual"` | pending |
| dimension `length` `{"l","value"(mm),"param"?}` | `AddDimension2` on the segment; `Parameter(...).SystemValue = value/1000` | pending (checkpoint covers it) |
| `distance`/`distance_x`/`distance_y` `{"p","q","value"(mm)}` | `AddDimension2` on the two points | pending |
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
sketch coordinates). The plane is accepted only when that frame **exactly
equals** the SolidCraft frame for the plane — axes, signs, and sketch origin
at the model origin:

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

That equality is what "verbatim (u, v)" promises: coordinates land where
they land in SolidCraft. A plane with the right normal but a different
in-plane orientation, or an offset origin, is **refused** and the step fails
closed; the report names every probed plane's observed frame so the mismatch
is legible. Coordinates are never silently reinterpreted, and full
coordinate equivalence is never claimed beyond this verified frame (plan
review 2026-11: the earlier role-matching text conflicted with the probe
and is gone). Preserving the original *view* orientation in SolidWorks is a
later nicety, not part of fidelity.

**Solve verification (runtime):** after relations and driving dimensions are
added, the macro reads the solved geometry back (line endpoints via
`ISketchPoint.X/Y/Z`, circle centre/radius) and compares against the recipe's
stored coordinates with a nominal tolerance of 1e-6 m. Every supported entity
must read back; a missing readback can never report `created`. On any
relation/dimension API error or mismatch: **restore**, don't just delete —
the sketch is deleted and recreated from the stored coordinates, driving
dimensions are reapplied, and the result is verified again and reported
`degraded` (or `failed` if the clean rebuild also mismatches). Deleting
relations alone is not a degrade path: the solver may already have moved
entities. A representative parameter-change check (edit a driving dimension,
see the geometry follow) is an owner-seat acceptance item
(`docs/seat-acceptance.md`), not something the offline tests can prove.

## Extrude (tracer phase)

| SolidCraft | SolidWorks API | Checkpoint-verified |
|---|---|---|
| `type: "extrude"`, `profiles: {"type":"all"}` | select the sketch, `FeatureManager.FeatureExtrusion2` blind single-side | pending |
| `extent.distance` literal (`"12 mm"`, `"0.5 in"`, bare = mm) | depth in metres; non-literal expressions are **unsupported until expressions land** (stop rule) | pending |
| `direction: "positive"|"negative"` | extrude direction flag | pending |
| `operation: "new_body"` | plain extrude in a fresh part | pending |
| `convention`, `distance2`, `start_offset`, `taper`, `to`/`from`, `through_all`, `thin`, `cut`/`join`/`intersect`, `profiles` other than `all` | **stop rule** (unsupported in the tracer phase) | n/a |

Extrude verification: the feature must exist and its depth dimension must
read back within tolerance. There is no degrade path for a failed extrude in
this phase: it is `failed`.

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
