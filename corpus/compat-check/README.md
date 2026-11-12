# corpus/compat-check

The **compatibility checkpoint**: one hand-written, line-by-line-reviewed
macro (`minimal.swb`) that must run cleanly on one seat before the generator
emits anything. It proves the vocabulary the generator will rely on:

- `.swb` syntax, the `main` entry point, and failure handling;
- fresh-part creation from the user's default template (never the active
  document — see `docs/seat-acceptance.md`);
- origin-plane selection, sketch insertion, `CreateLine` and
  `CreateCircleByRadius` in **metres**;
- one relation (`SketchAddConstraints "sgHorizontal"`) and one driving
  dimension (`AddDimension2` + `Parameter("D1@Sketch1").SystemValue`);
- the JSONL runtime report: header first, one line per step, the `summary`
  line last as the completion signal, written incrementally, and still
  written when the macro fails early.

## Running it on the seat

From the repository root on the Windows seat, after reviewing the script and
the macro:

```powershell
# The SHA-256 of the reviewed bytes (get it from git: git show HEAD:corpus/compat-check/minimal.swb | sha256sum)
.\tools\seat-smoke.ps1 -Macro .\corpus\compat-check\minimal.swb `
  -ExpectSha256 <sha256-of-minimal.swb> -ReportDir <your-output-directory>
```

The smoke script refuses to run anything that is not a committed, unmodified
file inside `corpus/`, and it verifies the exact bytes against
`-ExpectSha256` before execution. It sets `CRAFT_BRIDGE_REPORT`, runs the
macro through `RunMacro2`, then checks the report is fresh, complete and
consistent. See `tools/seat-smoke.ps1` for the exact invocation and the
documented uncertainty around the module/procedure arguments.

## What a pass records

Record the SolidWorks version and the outcome in `docs/seat-acceptance.md`.
A failure is data: it tells the generator which member name or syntax to
adjust, and the mapping table gains a note.
