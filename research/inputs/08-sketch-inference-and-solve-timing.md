# Research brief 08 - sketch inference and solve timing semantics

Follow-up to briefs 03/04/07 and to today's seat evidence (docs/source-
probe-evidence.md, probe-snap 2026-11-18): macro-created sketch geometry
runs the sketcher's INFERENCE engine, which moved a new line endpoint
onto an existing line's midpoint u-coordinate (a ~0.6 mm alignment zone,
the v coordinate untouched). The seat probe established empirically that
`swUserPreferenceToggle_e.swSketchInference` (value 249) OFF during
drawing places every endpoint exactly, while `swSketchSnapsMidPoints`
(268) alone does not, and that `ISketch.AddToDB` / `ISketch.EnableSolving`
do not exist on SOLIDWORKS 2026 (late-bound err 438). This brief asks
for the DOCUMENTED semantics around those findings and the solve/readback
timing the replay macros depend on.

## Ground rules (same as briefs 03/04/07)

- Public sources only: SOLIDWORKS API Help (all year editions reachable),
  archived/mirrored API reference pages, official SDK examples, pinned
  third-party code, long-lived forum solutions. No licensed seat.
- Label every claim: **Recorded** (official example/reference text),
  **Community** (third-party code; proves usage, not success),
  **Conflicting** (sources disagree; keep both), **Reconstructed /
  untested** (assembled; needs a seat probe before use).
- Quote only the relevant call sequences; explanations are original.
  Units are SOLIDWORKS internal (metres, radians) unless stated.
- SOLIDWORKS is a Dassault Systemes trademark; nominative reference only.

## T1. The sketch-inference preference surface (documentation)

The seat's `SolidWorks.Interop.swconst.dll` enumerates (values read by
reflection): `swSketchAutomaticRelations = 9`, `swSnapToPoints = 60`,
`swSnapToAngle = 61`, `swSketchInferFromModel = 95`,
`swSnapOnlyIfGridDisplayed = 142`, `swSketchInference = 249`,
`swSketchNoSolveMove = 250`, `swSketchSnapsPoints = 266`,
`swSketchSnapsCenterPoints = 267`, `swSketchSnapsMidPoints = 268`,
`swSketchSnapsQuadrantPoints = 269`, `swSketchSnapsIntersections = 270`,
`swSketchSnapsNearest = 271`, `swSketchSnapsTangent = 272`.

For EACH of these (and any inference/snapping toggle the docs list that
this set misses), recover from the official documentation:

1. The documented semantics: what behaviour the toggle governs, and
   whether it is documented to affect API-created geometry
   (`ISketchManager.CreateLine`, `CreateCircleByRadius`, `CreateArc`,
   `CreateSpline*`) or only interactive sketching.
2. The documented default state and scope (application-wide? per
   document? per sketch?).
3. The documented relationship between `swSketchInference` and the
   `swSketchSnaps*` family and `swSketchAutomaticRelations`: is 249 a
   master switch for 266-272? Is there a documented hierarchy?

The empirical evidence to reconcile (do not contradict it silently): 249
OFF stopped the endpoint snap; 268 OFF alone did not; toggles are
application-wide and must be captured/restored.

## T2. Programmatic suspension of inference and solving

1. `ISketch.AddToDB` and `ISketch.EnableSolving`: find any documentation,
   archived interface member lists, or cited code for these names on any
   SOLIDWORKS version. The 2026 seat reports err 438 (absent) through
   late binding. If they existed and were removed, find when and with
   what replacement. If they never existed, say so explicitly.
2. Any OTHER documented API that draws sketch geometry without
   inference/auto-relations (an interface member, a mode flag, a
   document state). Include `ISketchManager` members the docs describe
   as bypassing relations, and the macro-recorder's own output patterns
   for sketch creation if any survive in official examples.
3. The documented behaviour of `SetUserPreferenceToggle` (it returns
   Void on the 2026 interop - confirm the documented signature across
   versions) and any official examples showing the capture/set/restore
   discipline for temporary preference changes in macros.

## T3. Sketch solve and readback timing

1. `IDimension.SystemValue` write semantics: is the sketch solve
   synchronous with the property set? Cite official text on when
   dependent geometry updates. `ISketch.AddDimension`/`AddDimension2`
   after-creation solve behaviour likewise.
2. When do point/segment readbacks (`ISketchPoint.X/Y/Z`,
   `ISketchSegment.GetStartPoint/GetEndPoint` arrays and their `*2`
   object forms) reflect a solve? Any documented staleness (the seat saw
   some immediate re-reads after value edits that showed no movement,
   probe-dimkind edit-follow cases) - does the documentation require a
   rebuild (`EditRebuild3`, `ForceRebuild3`) or sketch re-entry for
   readbacks to be current?
3. `EditRebuild3` documented interaction with sketch edit mode (the seat
   measured: it EXITS sketch edit). Confirm/contradict from the docs;
   recover the documented way to rebuild and STAY in sketch edit.
4. Solver behaviour on under-constrained sketches when a dimension is
   added or its value set: does the documentation say which entities may
   move (the seat observed a free shape sliding when dimensions were
   driven on a sketch without fix relations)?

## T4. Dimension creation kinds (small)

1. `ISketchManager`/`IModelDoc2` dimension-creation members that create a
   HORIZONTAL or VERTICAL dimension explicitly: the seat confirms
   `AddHorizontalDimension2` / `AddVerticalDimension2` work on two
   selected sketch points and are deterministic (probe-dimkind). Recover
   the documented signatures, the documented kind-inference rule for
   `AddDimension2` (if any), and the documented behaviour when the
   requested axis has zero extent (the seat saw Nothing returned, err 0).
2. The documented sign semantics of `IDimension.SystemValue` for linear
   dimensions: can it be negative, and what does setting a negative
   value do?

## Output

A markdown report filed as `research/outputs/08-*.md`, with the same
evidence labels, a findings-at-a-glance table, a conflicts table, and an
explicit "still needs a seat probe" table. This report feeds the
craft-bridge emitter's sketch-drawing path (exact endpoint placement,
verified preference restoration) and the fail-closed value-edit checks;
recovered call shapes should be written so they can be pasted into
generated VBA.
