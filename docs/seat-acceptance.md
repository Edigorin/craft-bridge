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
- **`Dim sgn`-style names are compile errors**: VBA intrinsic names cannot
  be identifiers. A compile error presents as RunMacro returning failure
  with no report at all - bisect with committed compile probes.
