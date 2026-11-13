# compat-probe: diagnostic-only compile probes

These five macros are **diagnostic instruments, not fidelity goldens**. They
exist to localize one defect: every generated corpus item fails at macro
invocation (`run-macro.vbs` exit 6) with no runtime report at all, while the
hand-written `corpus/compat-check/minimal.swb` compiles and runs. The leading
hypothesis is a whole-module VBA compile failure in generated-only code; it
is a hypothesis, not a proven premise (see the control probe below), and
runner exit 6 does not identify a cause by itself.

Each probe is derived mechanically from the committed
`corpus/tracer-sketch-equations/item.swb` by replacing selected procedure
**bodies** with stubs. Signatures, declarations, procedure terminators and
every retained line stay byte-identical; a span check verifies that only the
intended body spans differ.

| probe | keeps real bodies | isolates |
|---|---|---|
| `probe-shape` | none (all step bodies stubbed) | module declarations, bootstrap, `main`, `ClearSlotsAll`, shared helpers |
| `probe-eq` | `AddGlobalEquation`, `AddDimEquation` | the equations family |
| `probe-sketch` | `BuildSketch2`, `DrawGeometry2`, `Relations2`, `Dims2`, `VerifySketch2`, `CleanSketch2` | the sketch family |
| `probe-extrude` | `BuildExtrude3` | the extrude family |
| `probe-control` | none, plus a broken never-called sub (three explanatory comments precede it; nothing else is appended) | the compilation premise itself |

## How to read a round

The macro writes its report header before anything else, so a **fresh
runtime report means the module compiled and ran**; no fresh report plus
runner exit 6 means it did not. Report writers suppress filesystem errors,
so a missing report is not absolute proof on its own - read it together
with the runner log. Runs are serialized through `tools/seat-smoke.ps1`
with explicit `-ExpectSha256` pins, one evidence directory per probe.

Interpretation requires the baseline first:

- `probe-shape` produces a fresh report **and** `probe-control` produces no
  fresh report (exit 6): unreachable code does compile in this host, so
  whole-module compilation holds and the family probes are valid
  compilation tests.
- `probe-shape` produces a fresh report **and** `probe-control` also
  reports: this host does not compile unreachable code. Retained but
  unreachable bodies (for example `probe-eq`'s real `AddDimEquation`, which
  is only called from the stubbed `Dims2`) are not exonerated by any probe,
  and the bisection design must be reworked before conclusions.
- `probe-shape` fails: the defect is in shared declarations, signatures,
  the bootstrap, `main` or `ClearSlotsAll` (the helpers are byte-identical
  to the compiling checkpoint, so they are already exonerated). The control
  then proves nothing about compilation and must not be interpreted.
- exactly one family probe fails **at invocation with no fresh report**:
  the defect is in that family; bisect within it next. A family probe that
  reports and then shows a runtime failure is a different signal (the
  family compiles; something fails later) and is read as such.
- several family probes fail at invocation: multiple defects or a shared
  dependency; split further before fixing.
- all family probes report (the deliberately broken control is excluded by
  design) but the real item still fails at invocation: interactions between
  families or a module-size effect; investigate by recombination.

A probe that reports is diagnostic evidence only. It is never acceptance
evidence, and no gate is relaxed for it.

## Round two: recombination

Round one outcome: `probe-shape` reported, `probe-control` did not (whole-
module compilation confirmed), and all three family probes reported with
their real bodies visibly executing - yet the full item still fails at
invocation. The matrix branch is "interactions between families or a
module-size effect", so round two probes recombine:

| probe | keeps real bodies | tests |
|---|---|---|
| `probe-eq-sketch` | equations + sketch families | that pair |
| `probe-eq-extrude` | equations + extrude family | that pair |
| `probe-sketch-extrude` | sketch + extrude families | that pair |
| `probe-pad` | none, plus 250 appended comment lines | source-size hypothesis (larger than the failing item, zero executable content added) |

If `probe-pad` reports while the full item fails, size is excluded and the
defect is an interaction. A pair that fails at invocation localizes it; all
pairs reporting would point at a three-way interaction or at the one
remaining delta: the full item's stub-free body set as a whole.

## Seat safety

Probes are committed synthetic corpus items and run only through
`tools/seat-smoke.ps1` (git-tracked, unmodified against HEAD, sha256-pinned
at invocation). They create a fresh part from the default template and write
the standard JSONL report: the same risk class as the compatibility
checkpoint. Removal of this directory after the defect is found is an
ordinary reviewed git change, never a delete command on the seat machine.

The no-delete rule means **no filesystem deletions** on the seat machine.
Inside the fresh SolidWorks part, the probes' inherited `DiscardProbeSketch`
and `CleanSketch2` helpers delete sketch entities by design (the documented
probe-sketch cleanup contract); that CAD-internal cleanup is not a file
deletion and is part of the reviewed behavior.

Reviewed: plan verdict 2026-11-13 (APPROVE WITH CHANGES, all folded in);
byte review of this directory and the tools changes (APPROVE WITH CHANGES,
all folded in) before first seat use.
