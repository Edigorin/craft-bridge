# craft-bridge: instructions for agents

craft-bridge rebuilds SolidCraft designs inside SolidWorks as native parametric
models (replay, not translation). This file is the hard rules; the program spec is
`docs/spec.md`, vocabulary in `GLOSSARY.md`, decisions in `docs/adr/`.

## Hard rules

- **Clean-room exemption (this repo only).** SolidCraft's rules forbidding driving
  or taking data from commercial CAD products do not apply here: in this repository
  you may drive SolidWorks, use its public API documentation, and handle
  SolidWorks-produced files. See `docs/adr/0002-exemption-and-boundary.md`.
- **Hard boundary.** Nothing derived from SolidWorks — files, measurements,
  observations, API findings — may ever enter the SolidCraft repository: not its
  code, tests, docs, issues, PRs, comments or attachments. SolidCraft-side work is
  written purely from SolidCraft's own sources and generic, independently authored
  requirements. SolidCraft-side publications are shown to the owner for approval
  before filing.
- **Seat safety.** The owner's licensed seat never executes macros or fixture
  files from unreviewed public PRs. Only committed, reviewed synthetic-corpus
  items run on the seat. Real client parts are never committed anywhere.
- **Hostile input.** Artifacts are untrusted: safe extraction (no path traversal),
  size caps, VBA string-literal escaping in generated macros, recipe entries
  translated only from a known command allowlist (unknown entries become fallback
  bodies, never executed), output written only to the requested output directory.
- **Trademarks.** SOLIDWORKS is a Dassault Systèmes mark; nominative reference
  only, no affiliation implied. No third-party materials in the repo: no interop
  assemblies, documentation excerpts, icons or fonts.
- **License.** Apache-2.0 only.
- Code, comments, commit messages and docs are in English.
