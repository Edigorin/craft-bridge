# Owner-seat acceptance

What only a licensed SolidWorks seat can prove, and what to record. Goldens
prove generation; only these checks prove replay. Nothing here runs
automatically: every macro goes through `tools/seat-smoke.ps1` (committed,
unmodified, byte-verified corpus items only) and every check below is run by
the owner.

## Compatibility checkpoint (`corpus/compat-check/minimal.swb`)

- [ ] `seat-smoke.ps1` runs the macro; the runtime report shows all steps
      `created` and summary `ok`.
- [ ] The part tree shows one sketch with one line and one circle, one
      horizontal relation, one 40 mm driving dimension.
- [ ] Edit the dimension (say to 50 mm): the line follows. (The representative
      parameter-change check.)
- [ ] Record the SolidWorks version and year here:

| Date | SolidWorks version | Checkpoint | Smoke script | Notes |
|---|---|---|---|---|
| | | | | |

## Per-feature acceptance (mapping tickets)

For every feature family the generator maps:

- [ ] The rebuilt feature tree shows the feature with the expected name and
      type (not an import, not a body).
- [ ] Solved geometry matches the recipe within tolerance (the macro's own
      readback says `created`; spot-check one dimension by editing it).
- [ ] Driving dimensions exist and drive the model.
- [ ] A deliberate failure case (a recipe with an unmapped step) stops at the
      boundary and reports `fallback-planned` / `not-executed` — never silent.
- [ ] A deliberate degradation case (a relation outside the tracer subset)
      reports `degraded` and the geometry still matches.
- [ ] A deliberately early failure (e.g. no default template) still leaves a
      runtime report with the summary line.

## Evidence

Keep reviewed synthetic evidence (screenshots of the tree and the edited
dimension) attached to the ticket that delivered the feature. Never commit
real client parts, and never commit SolidWorks-produced files outside
`corpus/` (AGENTS.md seat-safety and hostile-input rules).

## v1 release checklist

- [ ] Every corpus item's macro runs on the seat (`tools/seat-smoke.ps1`) and
      its runtime report is complete: header, one line per recipe step,
      summary last.
- [ ] Checkpoint-verified column filled in `docs/mapping.md`; every
      "pending" API member either verified or explicitly recorded as
      adjusted after a seat finding.
- [ ] Tested SolidWorks versions recorded in the table above; the "2018 and
      later" target confirmed or amended in `docs/mapping.md` and the spec.
- [ ] Equations verified on the seat: a global-variable edit propagates to a
      sketch dimension and a feature dimension (the parameter-change check).
- [ ] Fallback verified on the seat: an artifact with a boundary snapshot
      rebuilds past the unmapped step; a coarse artifact terminates cleanly.
- [ ] Degradation and failure paths spot-checked: a dropped relation reports
      `degraded`, an early failure still leaves a complete report.
- [ ] README and `docs/replay-profile.md` match the shipped behaviour;
      the disclaimer and the open legal question still stand.
- [ ] No real client parts anywhere in the repository or its history.

## Corpus coverage (v1)

The synthetic corpus fixtures under `tests/fixtures/` cover: a constrained
sketch with equations (`tracer`), the stop rule and degradation triggers
(`tracer-stop`), revolve with both axis styles (`tracer-revolve`), primitives
with cross-references (`tracer-prim`), the five modification families
(`tracer-mod`), and mirrors with live seed bindings (`tracer-pat`). Fallback
paths are exercised in the build tests with packed boundary and final
snapshots. Anything the seat reports as a correction becomes a new fixture.

## Runner quirks (measured on the seat)

- `RunMacro2` raises "Type mismatch" on this seat **on every invocation**,
  successful or not - it is unusable and only the legacy `RunMacro` return
  value is the invocation gate (`tools/run-macro.vbs` falls back
  automatically). Do not read the "RunMacro2 failed" line as a diagnosis.
- A macro that fails to **compile** shows up as: RunMacro returns failure,
  exit code 6 from the runner, and **no runtime report at all** (the report
  file is only created once `main` runs). Bisect with committed compile
  probes under `corpus/compat-probe/` (the `probe-ck*` series did exactly
  this for the `sgn` identifier defect).
- **A scheduled-task wrapper ending `exit $LASTEXITCODE` reports 0 when
  seat-smoke THROWS** (a thrown error does not set `$LASTEXITCODE`), and
  seat-smoke resolves `-ReportDir` with `Resolve-Path`, which fails on a
  missing directory before anything is written. A green task result with
  no report is a FALSE PASS (probe-primorient's 10:15 "exit 0" run, whose
  probe never started) - trust the report's summary line, never the task
  result alone.
- VBA intrinsic names (`Sgn`, `Val`, `Mid`, ...) cannot be identifiers:
  `Dim sgn As Double` is a compile error. Enforced offline by
  `no_identifier_shadows_a_vba_intrinsic` (tests/build_tests.rs).

## Sketch API semantics (measured on the seat, 2026-11-16)

- **`EditRebuild3` EXITS sketch edit mode.** Any further sketch API call
  then silently does nothing (`CreateLine` returns Nothing,
  `SketchAddConstraints` no-ops with no error, dimension creation fails).
  Every mid-sketch rebuild goes through `SolveSketch` (rebuild + name-
  selected re-entry + verification). This caused the checkpoint's first two
  relation failures and the probe's "second CreateLine returns Nothing".
- **`SketchAddConstraints` names are the SW2006+ table** and are silently
  ignored when wrong: lines `sgHORIZONTAL2D`/`sgVERTICAL2D`, point pairs
  `sgHORIZONTALPOINTS2D`/`sgVERTPOINTS2D`, coincidence `sgCOINCIDENT`,
  equal length `sgSAMELENGTH`, `sgPARALLEL`, `sgPERPENDICULAR`, `sgTANGENT`.
  The legacy `sgHorizontal`/`sgVertical`/`sgEqual` spellings are silent
  no-ops. `SketchAddConstraints` has **no return value**; application is
  established behaviourally (the constraint's own predicate on the solved
  geometry, after `SolveSketch`).
- **Circle radius is `GetRadius`** (a method); `ci.Radius` raises 438.
  `GetCenterPoint2` returns the centre handle (X/Y/Z readable, Select
  works); `GetStartPoint` on a circle returns a perimeter point (array).
- **`AddDimension2` on a circle is diameter-valued** (2026-11-17,
  frame-sign-chain: the created dimension measures .02 for a drawn radius
  .01). Radius dimensions carry the `dimScale` 2 convention at the API
  boundary: value target `2 * r`, equation text `2*(...)`, scaled expect,
  and `regScale`/`regSign` normalize `SlotRef` references back to source
  semantics (a76b0e7/39dad15).
- **`Dim sgn`-style names are compile errors**: VBA intrinsic names cannot
  be identifiers. A compile error presents as RunMacro returning failure
  with no report at all - bisect with committed compile probes.
- **Select a sketch by its live FEATURE OBJECT, never by name.**
  `SelectByID2(name, "SKETCH", ...)` returns False even with the right
  name once a probe sketch has been created and deleted before the target
  sketch (the tracer flow): probe-extrudedir's by-name select worked only
  while the sketch was the first-ever one. `Feature.Select False` on a
  live object is the pinned-reliable selection (probe-endpoints,
  DiscardProbeSketch); the generator stores `skFeatN` per sketch step via
  the proven `FindFeatureByName` walk, selects through it everywhere
  (feature calls, SolveSketch re-entry, DeleteCreated) and keeps the
  by-name form only as fallback, refusing with `TreeSummary()` when both
  fail (b2e9878).

## Primitive orientation probe (`probe-primorient`, v1)

- [x] **The 10:15 "exit 0" run was a FALSE PASS - the probe never ran.**
      The task wrapper's `exit $LASTEXITCODE` reports 0 when seat-smoke
      throws (a thrown error does not set it), and seat-smoke resolves
      `-ReportDir` with `Resolve-Path`, which fails when the directory is
      absent - the harness had created `probe-primorient-b2` while the
      registered task pointed at `probe-primorient`. Nothing was written
      anywhere; the empty `probe-primorient-b2` was all there was. Task
      results alone never prove a run (the runner-quirks rule above).
- [x] **The real run (2026-11-17 11:35 local, same sha-pinned task) is
      green where it counts.** Macro
      `corpus/compat-probe/probe-primorient.swb` sha256
      `e4e4771da3749cd15565b8acfd64ff12782d9c51cd6a796f132aa2251276257f`.
      `box-dir-false` and `box-dir-true` completed on the verified Top
      frame (u=(0,1,0) v=(-1,0,0) n=(0,0,1), map m00=0 m01=-1 m10=1
      m11=0 side=+1): the mapped profile and both size dimensions
      replayed with sign-resolved values (D1 = "D1", SystemValue = .04
      / .015, signVal=1), `D1@Boss-Extrude1` read back 0.012 SI metres,
      and GetPartBox measured **Dir=False at z in [0, .012]** and
      **Dir=True at z in [-.012, 0]**. Every owned document closed and
      verified (the close rule held on every path).
- [x] **`TT_PRIM_DIR_TRUE_SIDE = -1` is recorded** (src/truth.rs,
      docs/mapping.md): Dir=True sends material toward MINUS the sketch
      normal - the same reading as `TT_EX_DIR_TRUE_SIDE` on a second
      frame (the cross-check the evidence scope required), and the
      emitter's composition held end to end: side +1 resolved to
      Dir=False and landed the height at model +Z, the SC side.
      Evidence scope stays narrow: the measured positive-normal
      source-XY configuration only; the reflected case is never proven.
- [x] **The circle-size kind cross-check passed** on the primitive path:
      `kind evidence: created=.012 getradius=.006 twice-getradius=.012`
      - the diameter-valued reading, consistent with the settled
      `dimScale` convention (a76b0e7/39dad15).
- [ ] **`box-edit-follow` did not run (probe v2 follow-up).** The v1
      `CircleDim` kind guard predates the diameter convention and
      refused the diameter reading; its failure classified as a setup
      failure and aborted the remaining case. Probe v2 (byte review per
      version): update the kind guards to the `dimScale` convention,
      classify kind-guard refusals as measurement failures, re-run for
      the shape-contract edit-follow evidence (the corner anchor and
      rectangle behaviour under a parameter change).

## Extrude and mass APIs (measured on the seat, 2026-11-16)

- `FeatureExtrusion2` takes **23 arguments**; `T0/StartOffset/FlipStartOffset`
  are not optional (err 449 "Argument not optional"). Full-arity calls in the
  emitter end `, 0, 0#, False)`.
- **Direction truth table (Front plane = SC YZ, n = +X):** `Dir=True` sends
  material toward **minus** the sketch normal, `Dir=False` toward **plus**.
  Two-sided: `D1` (first depth) to the **plus**-normal side, `D2` (second) to
  the **minus**-normal side. Through-all follows the same Dir rule.
  Dimension identities: `D1@Boss-Extrude1` / `D2@Boss-Extrude1`, SI metres.
- `IPartDoc.GetPartBox(True)` returns `Double()` of 6 (c0..c2 one diagonal
  corner, c3..c5 the other), SI metres.
- Mass properties: the centroid member is `centerofmass` (American spelling;
  `centreofmass` raises 438). Volume + centroid captured per body.
- Construction geometry is `ISketchSegment.ConstructionGeometry` (get/set);
  `.Construction` raises 438.

## Sketch constraint names (behaviourally verified, 2026-11-17)

The full table is pinned by probe-relnames with per-name predicates measured
after the solve (level deltas, shared coordinates, length differences,
cross/dot products, tangency gap):

| Constraint | Name | Verified |
|---|---|---|
| line horizontal | `sgHORIZONTAL2D` | level-delta = 0 |
| line vertical | `sgVERTICAL2D` | level-delta = 0 |
| points horizontal | `sgHORIZONTALPOINTS2D` | shared coordinate = 0 |
| points vertical | `sgVERTICALPOINTS2D` | shared coordinate = 0 |
| equal length | `sgSAMELENGTH` | length difference = 0 |
| parallel | `sgPARALLEL` | cross product = 0 |
| perpendicular | `sgPERPENDICULAR` | dot product = 0 |
| tangent (line-circle) | `sgTANGENT` | tangency gap = 0 |
| coincident (points) | `sgCOINCIDENT` | merges the points (items green) |

Trap: `sgVERTPOINTS2D` is a SILENT NO-OP (no error, no effect) - the working
spelling is `sgVERTICALPOINTS2D`. And a merge invalidates pre-solve point
handles: re-acquire before measuring (automation error otherwise).

## The `solid=none` expect declaration

`expect.txt` may declare `solid=none` for corpus items whose macros
deliberately build no solid (sketch-only frame probes, stop-rule items).
The checker then verifies the absence two-way: a clean zero volume gives
`check:solid=pass` and `check:mass=skipped`; any solid volume gives
`check:solid=fail` (an unexpected body); an unreadable mass read cannot
prove absence and fails named. Items without the declaration keep the
fail-closed data gate: a missing, unreadable or nonpositive rebuilt
volume is `check:mass=fail`. Real designs never declare `solid=none`.

## The proven FeatureExtrusion2 call carries Merge/scope flags (2026-11-17)

The public API research (research/outputs/03-solidworks-api-research.md, A2) names the
23 positional arguments: position 18 = Merge, 19 = UseFeatScope,
20 = UseAutoSelect. Our corpus-proven emission passes True, True, True:
the extrude MERGES with automatically selected bodies in feature scope.
The corpus is single-body, so this never distinguished `new_body` from
`join` - which is exactly why the operation gate stops at new_body and
the merge/scope probe (P04) is required before Phase B claims any
multibody semantics. Also documented there: source join/cut/intersect do
NOT all map to this boss call - subtractive extrusions use the cut
feature API and intersections use InsertCombineFeature (marks: target 1,
tools 2; operation constants 15903/15902/15901).

## Task XML and runner lessons (research 05 confirmed)

- The harness task template once used `*>1 | Out-File ...` for log capture:
  PowerShell's `*>1` redirects all streams to a FILE NAMED `1` (that is
  the stray `1` in the repo root), it does not merge into stdout. Use
  `2>&1 | Out-File` deliberately. The corrected template runs a -File
  wrapper script with Start-Transcript (see the probe runner pattern in
  docs/source-probe-evidence.md).
- `RunMacro2(path, module, procedure, options, error)` takes
  `ByRef Error As Long`; a bare VBScript Variant makes the late binder
  raise "Type mismatch" BEFORE the macro runs - the recurring
  "RunMacro2 failed: Type mismatch" in every runner log. tools/run-macro.vbs
  now coerces with `CLng`; the legacy `RunMacro` fallback stays.
- VBScript has no `Val`, `Str` or `Format` (those are VBA): a call raises
  a swallowed error under `On Error Resume Next`. tools/check-part.vbs
  used `Val` in the `dim:` expect path - now the logic module's
  `ParsePeriod`. The tooling tests pin the ban list.


## 2026-11-19 - session wedges, macro-invocation fatigue, and the guarded restart

SolidWorks's VBA layer wedges while COM stays alive: the window reports
Responding=True and COM reads answer (revision, document walks), but
RunMacro/RunMacro2 fail for every macro ("invocation returned: failure",
run-macro.vbs exit 6) and `ISldWorks.ExitApp` silently does nothing. A
modal dialog on the instance produces the same signature. Observed
twice; the second instance matched a CUMULATIVE pattern: roughly 20-25
macro invocations in one session before invocation starts failing.

Operating rules pinned (synthetic tooling behavior):

1. **Restart the session before each long macro sequence** (harness
   qualification runs, multi-item work). Budget roughly 20-25 macro
   invocations per session.
2. **Never launch seat work while a restart task is running** - poll the
   restart until it settles and `craft-swalive` (the probe-ckcompile
   smoke) reports Last Result 0. A harness launched mid-restart failed
   every item.
3. `craft-swalive` is the macro-execution health signal (0 = macros
   run). `craft-swrestart` only LAUNCHES SolidWorks; it cannot clear a
   wedged instance.
4. **Guarded force-restart** (owner-authorized 2026-11-19): verify zero
   open documents (COM walk; the count is also suspect while wedged, so
   re-verify immediately before), Stop-Process, relaunch via
   craft-swrestart, verify with craft-swalive. Never kill with documents
   open - that is the owner's decision.
5. After a forced termination, the fresh instance may sit behind a
   startup/recovery dialog that blocks macros: the owner dismisses it.

And the process rule that cost a debugging round: **every
check-part-logic.vbs change runs the cscript fixture suite on the seat
before any further seat work** - an off-by-one in the adapter parser
would have been caught by the fixtures on their first run; source pins
alone are insufficient (the review said exactly this).
