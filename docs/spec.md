# Spec: craft-bridge — rebuild SolidCraft designs in SolidWorks as native parametric models

## Problem Statement

Designers who want to try SolidCraft on real work face a one-way door: if the
software does not work out for them, their designs are stranded, and clients who
expect SolidWorks-native files cannot be served. Neutral exports (STEP and the
like) lose the parametric history — there is no way today to take a design built
in SolidCraft into SolidWorks and continue working on it there.

## Solution

craft-bridge rebuilds a SolidCraft design inside the user's own SolidWorks as a
native parametric part. The user packs the design into a `.craftx` artifact,
runs the bridge CLI to generate a plain-text `.swb` macro, runs that macro in
SolidWorks 2018 or later, and gets a native `.sldprt` with a real feature tree —
sketches with relations and driving dimensions, features, and parameters as
equations — plus a plan report before running and a runtime report after. Steps
that cannot be replayed fall back to an embedded body instead of failing the
model; nothing happens silently.

## User Stories

1. As a SolidCraft prospect, I want to rebuild my SolidCraft part in SolidWorks with its parametric history, so that trying SolidCraft is not a one-way door.
2. As a working designer, I want my sketches to arrive with their relations and driving dimensions, so that I can re-drive the design in SolidWorks.
3. As a working designer, I want my parameters to arrive as SolidWorks equations, so that global edits keep working after the move.
4. As a designer sharing with clients, I want to hand clients an artifact and a macro they can run in their own SolidWorks, so that I can deliver native files without owning a SolidWorks seat myself.
5. As a client's SolidWorks engineer, I want a native `.sldprt` with a real feature tree, so that I can keep editing with the tools I already have.
6. As a user, I want a plan report before I run anything, so that I know which steps will map, degrade, or fall back.
7. As a user, I want a runtime report after the macro runs, so that I know exactly what was created, degraded, embedded as fallback, or failed.
8. As a user with an exotic feature, I want it embedded as a fallback body at that point in the tree, so that my part still opens and the rest of the model survives.
9. As a user, I want the generated macro to be plain text, so that I can review exactly what will run before running it.
10. As a user, I want failures to leave a partial model plus a report entry instead of a crash, so that I can finish urgent work and report the problem afterwards.
11. As a user on inch-unit designs, I want units preserved through the rebuild, so that dimensions come out right the first time.
12. As a user on SolidWorks 2018 or later, I want the macro to run on my version, so that I am not forced to upgrade.
13. As a bridge developer, I want a documented, versioned replay profile for the artifact, so that the bridge never depends on SolidCraft internals.
14. As a bridge developer, I want golden tests of the generated macro text and plan report, so that fidelity is regression-tested without a SolidWorks seat.
15. As a bridge developer, I want unknown recipe entries to become fallback bodies instead of being guessed at, so that the generator can never mis-execute a command it does not understand.
16. As the seat tester, I want one harness command that replays the whole synthetic corpus and emits a pass/fail report, so that acceptance is repeatable and cheap.
17. As a maintainer, I want the synthetic corpus committed (recipes, artifacts, rebuild outputs), so that fidelity claims are publicly verifiable.
18. As a maintainer, I want the clean-room exemption and its hard boundary recorded in both repos, so that future contributors neither re-litigate it nor leak across it.
19. As a SolidCraft maintainer, I want the SolidCraft-side export extensions to land as ordinary reviewed changes in my own repo's process, so that the core stays clean-room and consistent.
20. As a user receiving an artifact, I want safe handling of untrusted files (bounded sizes, safe extraction, escaped generated code), so that a crafted artifact cannot harm my machine.
21. As a user in the later assemblies phase, I want assemblies and their joints to arrive as SolidWorks mates, so that mechanism intent survives the move.
22. As a user in the later configurations phase, I want SolidCraft configurations to arrive as SolidWorks configurations, so that part families stay one document.
23. As a user in the later add-in phase, I want an installable SolidWorks add-in that reads artifacts directly, so that I do not manage script files by hand.
24. As a future maintainer of another CAD target, I want the artifact and reports not to hardcode SolidWorks semantics, so that a second target can join without redesign.

## Implementation Decisions

- Two runnable components: the **bridge CLI** (cross-platform; pack, inspect,
  build) and a **seat-side harness** (Windows, run on the operator's own licensed
  SolidWorks seat).
- The artifact is a `.craftx` container per the shape of SolidCraft's draft
  RFC 0001: manifest, STEP fallback geometry, face/parameter tables. The bridge
  owns a **versioned replay profile** layered over that shape; it *adds* the
  recipe payload the RFC does not define and lists its deviations from the draft.
  Compatibility with a future CamCraft bridge is an intent, not a claim of
  established interoperability.
- **v1 accepts single-part source documents** exactly as SolidCraft's timeline
  script export accepts them today (no components, occurrences, parts, links,
  joints or transferred bodies). Packing surfaces the exporter's omission
  warnings into the artifact manifest so no information is silently dropped. The
  export extensions for assemblies and configurations are phase-2 preparation,
  filed on SolidCraft's own tracker — not v1 prerequisites.
- **Fallback granularity**: per-boundary STEP snapshots produced at pack time by
  replaying recipe prefixes through SolidCraft's own CLI when it is available. A
  fallback step replaces its target body at that timeline point and the rebuild
  continues against the replacement; a step that fails mid-way rolls back to its
  pre-step state; downstream steps reference the replacement body. Without the
  CLI, the artifact carries a single final-body snapshot and the plan report
  marks fallback as coarse (from the first unmappable step onward).
- **Two reports, never conflated.** The generator emits a **plan report**
  (statuses: mapped, degrade-planned, fallback-planned, unmapped, not-executed)
  that is deterministic and golden-tested. The generated macro writes a **runtime
  report** incrementally (statuses: created, degraded, fallback, failed, with a
  message per step), so failures before completion still leave results. Golden
  tests prove generation only; only the seat proves replay.
- **Constraint replay with objective acceptance**: every stored relation is
  re-created and must solve identically. Acceptance is checked by solved geometry
  within a stated tolerance (nominal 1e-6 m), relation inventory comparison where
  the API exposes it, fully-constrained/DOF status where available, and a
  parameter-change test on representative sketches (driving dimension changes,
  geometry follows). Relations with no SolidWorks equivalent (pattern-point
  transforms, point-along-curve, pierce, signed arc sweeps and kin — listed in
  the mapping table) are recorded as degrade triggers, not promised. If
  degradation to geometry plus driving dimensions also fails, the step becomes a
  fallback body.
- **Mapping table**: SolidCraft step → SolidWorks API member, documented per
  feature, targeting the conservative member set available since SolidWorks 2018.
  Parameters map to equations. Later phases: configurations to configurations,
  joints to mates.
- **Reports** are machine-readable JSON plus a human summary; statuses are
  replayed/degraded/fallback/failed, with not-executed used in plan reports.
- **Hostile input**: artifacts are untrusted — safe extraction (no path
  traversal), size caps, VBA string-literal escaping in generated macros, recipe
  entries translated only from a known command allowlist (unknown entries become
  fallback bodies, never executed), output written only to the requested output
  directory.
- **Seat safety**: the harness runs only committed, reviewed synthetic-corpus
  items; macros or fixtures from unreviewed public PRs never execute on the seat.
- **Boundary operational rules**: no SolidWorks-derived observation becomes a
  SolidCraft requirement or test expectation; SolidCraft-side issues and PRs are
  drafted from SolidCraft's own sources and generic requirements; every
  SolidCraft-side publication is shown to the owner for approval before filing.
- **Repository**: Apache-2.0; no third-party materials committed (no interop
  assemblies, documentation excerpts, icons); trademark disclaimer in the
  README; starts user-owned, transferable to an organisation later.

## Testing Decisions

- A good test observes external behaviour only: given an artifact, the macro
  text and plan report that come out; on the seat, the rebuilt model (tree shape,
  dimension values, mass properties against the STEP reference) and the runtime
  report that comes back. Internals are never asserted.
- Modules tested: artifact packer/reader and profile validator (golden contract
  tests), the generator and mapping table (golden macro text per fixture
  recipe), both report builders, and the seat harness's acceptance checks.
- Prior art: SolidCraft's own script-export round-trip tests (extended in place
  on the SolidCraft side, per its rule that regression tests join the area's
  existing test file); golden-file generator tests for the bridge.
- Seams: one primary seam — the bridge CLI end to end on any OS (`.craftx` in,
  macro text plus plan report out) — and one acceptance seam: the seat-side
  harness against real SolidWorks. The SolidCraft side stays in its existing
  seams.

## Out of Scope

- Writing or reading `.sldprt` files; any claim of lossless translation (the
  tree is reconstructed, not translated).
- v1: assemblies and joints-to-mates, configurations, the SolidWorks add-in,
  client-distribution packaging, additional CAD targets, PMI/GD&T, drawings,
  FeatureWorks integration. Decisions for these are recorded above as later
  phases.
- Real client parts in the synthetic corpus; monetised distribution (needs
  partner-program review); anything crossing the hard boundary into SolidCraft.

## Further Notes

- Legal: the `.swb` text artifact is an architectural choice that minimises
  API-distribution exposure; it is not a legal opinion. The Solution Partner
  fee question and exact EULA wording remain open (see ADR 0003, which cites the
  sources consulted).
- "SolidWorks 2018 and later" is a compatibility target until seat-side evidence
  exists; tested versions are reported separately from the target.
- The clean-room exemption grants no redistribution rights for third-party
  materials.
- SOLIDWORKS is a registered trademark of Dassault Systèmes; references are
  nominative and imply no affiliation.
