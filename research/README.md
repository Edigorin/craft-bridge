# research/

Research traffic for craft-bridge: the prompts we hand to research agents
(`inputs/`) and the reports they return (`outputs/`). Numbered so an
output's index matches the input that asked for it (an input may yield
several outputs; a follow-up input continues the same number with a
letter).

Conventions:

- **Inputs** are self-contained briefs: ground rules (evidence labels,
  citation style, no documentation excerpts in prose, nominative trademark
  use), the specific questions, and the delivery format.
- **Outputs** are filed verbatim with a provenance header stating what
  the report already changed in the codebase. They are evidence, not
  instructions: implementation decisions cite them but do not follow them
  blindly.
- What the outputs leave **unverified** becomes the seat probe queue
  (committed synthetic probes under `corpus/compat-check/`, run through
  `tools/seat-smoke.ps1`). Probe results are evidence too and are logged
  in `docs/source-probe-evidence.md`.
- Design fixtures produced for us (test parts) are private deliverables
  and live untracked under `uploads/` per the AGENTS.md seat-safety rule;
  only research TEXT is committed here.

Current state (2026-11-17):

| # | Input | Output |
|---|---|---|
| 01 | `inputs/01-real-design-test-brief.md` (test designs + format research) | `outputs/01-solidcraft-57-format-research.md` |
| 02 | `inputs/02-nested-schema-and-micro-designs.md` | `outputs/02-solidcraft-57-nested-schema-research.md` |
| 03 | `inputs/03-solidworks-api-research-brief.md` | `outputs/03-solidworks-api-research.md` |
| 04 | `inputs/04-solidworks-method-pages-probe-queue.md` | pending |
| 05 | `inputs/05-vba-dialects-and-seat-automation.md` | pending |
| 06 | `inputs/06-solidcraft-command-shapes-and-plugins.md` | pending |
