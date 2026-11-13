# compat-probe: diagnostic-only compile probes

These five macros are **diagnostic instruments, not fidelity goldens**. They
exist to localize one defect: every generated corpus item fails at macro
invocation (`run-macro.vbs` exit 6) with no runtime report at all, while the
hand-written `corpus/compat-check/minimal.swb` compiles and runs. The leading
hypothesis is a whole-module VBA compile failure in generated-only code; it
is a hypothesis, not a proven premise (see the control probe below).

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
| `probe-control` | none, plus a broken never-called sub | the compilation premise itself |

## How to read a round

The macro writes its report header before anything else, so **any runtime
report means the module compiled and ran**; no report plus runner exit 6
means it did not. Runs are serialized through `tools/seat-smoke.ps1` with
explicit `-ExpectSha256` pins, one evidence directory per probe.

- `probe-control` produces no report: unreachable code does compile in this
  host, so whole-module compilation holds and the family results below
  interpret normally.
- `probe-control` produces a report: this host does **not** compile
  unreachable code. Stubbed bodies are not exonerated by any probe, the
  bisection design is invalid and must be reworked before conclusions.
- `probe-shape` fails: the defect is in shared declarations, signatures,
  the bootstrap, `main` or `ClearSlotsAll` (the helpers are byte-identical
  to the compiling checkpoint, so they are already exonerated).
- exactly one family probe fails: the defect is in that family; bisect
  within it next.
- several family probes fail: multiple defects or a shared dependency;
  split further before fixing.
- all probes report but the real item fails: interactions between families
  or a module-size effect; investigate by recombination.

A probe that reports is diagnostic evidence only. It is never acceptance
evidence, and no gate is relaxed for it.

## Seat safety

Probes are committed synthetic corpus items and run only through
`tools/seat-smoke.ps1` (git-tracked, unmodified against HEAD, sha256-pinned
at invocation). They create a fresh part from the default template and write
the standard JSONL report: the same risk class as the compatibility
checkpoint. Removal of this directory after the defect is found is an
ordinary reviewed git change, never a delete command on the seat machine.

Reviewed: plan verdict 2026-11-13 (APPROVE WITH CHANGES, all folded in);
byte review of this directory and the tools changes before first seat use.
