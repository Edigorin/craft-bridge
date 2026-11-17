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

## 2026-11-17 - `fixed: true` + `exprs` is creatable through the MCP

Source-confirmed earlier (nested-schema research [S38]); the design
agent's D02/D10 fixtures were created exactly this way and re-open
correctly, closing open issue 3 of the nested-schema research for these
cases (saved `pos` matches `exprs` evaluation in the delivered files -
though stale-pos remains possible in general per [S34]-[S36], and the
loader's expr-wins rule covers it).
