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
