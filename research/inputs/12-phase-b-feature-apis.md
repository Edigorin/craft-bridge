# Research brief 12 - Phase B feature APIs: persistent entity identity, fillets, booleans, move/remove

Follow-up to briefs 10/11. D01 replays exact; the plaques and the
R1 design gate on Phase B (the project's own roadmap): "body
registry + merge/scope + edge-resolver + expression prim placement
(unlocks D03 joins, D04 move/remove, D06 fillets, D07 cut, plaque full
acceptance, R1)". This brief gathers the DOCUMENTED API contracts
behind those four probes so the seat probes can be shaped against the
real signatures instead of guesses.

The consumer is the replay emitter (`src/swb.rs`) and the acceptance
layer. Every emitted call must be the documented member with the
documented semantics; every readback must fail closed with named
refusals (brief 10/11's rules). VBScript-callability matters for the
checker; VBA is the emitter's language (the generated .swb macros).

## Ground rules (same as briefs 03/04/07/08/09/10/11)

- Public sources only: SOLIDWORKS API Help (all reachable editions),
  official examples, pinned mirrors where pages are missing, pinned
  third-party code, long-lived forum solutions. No licensed seat.
- Label every claim: **Recorded** (official declaration/example),
  **Community** (third-party code; usage not success), **Conflicting**
  (sources disagree; keep both), **Reconstructed / untested** (derived;
  needs a seat check before use).
- Quote signatures exactly; write them copy-ready in the emitter's VBA
  dialect. Units metres/radians unless stated. SOLIDWORKS is a Dassault
  Systemes trademark; nominative reference only.

## T1. Persistent entity identity (the edge-resolver problem)

SolidCraft features reference edges/faces of earlier bodies (e.g. fillet
`edge_refs`). The replay must resolve a SOURCE identity to a live
SOLIDWORKS edge/face at macro time, across rebuilds.

1. The documented identity mechanisms: `IEntity`/`GetEntity`,
   `IModelDoc2.Extension.GetPersistReference3` (persistent reference
   IDs - byte arrays? marshaling shape!), `IBody2.GetEdges` /
   `IFace2.GetEdges`, `IEdge` curve parameter readback, named
   selections (`IModelDoc2.Extension.SelectByID2` with names), and
   feature-based addressing (edge index within a body/feature). For
   each: exact signature, what survives a rebuild, what is stable
   across sessions, and how VBA consumes the result.
2. How an edge is addressed in `FeatureFillet`/other feature inputs:
   selection-based (`SelectByID2` / `ISelectData` marks) vs object-based
   (`IEdge` references in feature data). Which does the documented
   fillet path use?
3. Edge ordering guarantees (does `IBody2.GetEdges` order survive
   rebuild?) - and the documented way to derive a STABLE edge identity
   (endpoints + curve type + adjacent faces?) when no persistent ID
   exists.
4. VBScript-vs-VBA marshaling for every array in this area (brief 11's
   lesson: probe each return family; never extrapolate from a working
   count).

## T2. Constant-radius fillets (D06)

1. `IPartDoc.FeatureFillet` (the old-style call) vs
   `IModelDoc2.FeatureFillet`/`FeatureFillet3...` - the current
   documented creation entry for a constant-radius fillet on a chosen
   edge set. Full signature(s) copy-ready, radius units (metres?),
   failure behavior (what does the call return/raise when the edge set
   is invalid?), and how the resulting feature reports its inputs.
2. `ISimpleFilletFeatureData2` readback: radius source, the edge
   collection, `GetEdges` on the feature data, and the conic-rho
   variant (`DefaultConicRhoOrRadius` - fillet-only rho, not the sketch
   conic of research 09). When must the feature be rolled back to
   access its data?
3. The reverse readback the acceptance layer needs: given a created
   fillet feature, verify its radius and edge set against the source
   `edge_refs` (which members expose them after rebuild?).

## T3. Booleans, body registry, move/remove (D03/D04/D07)

1. `IPartDoc.InsertFeatureCombine` / `ICombineFeatureData` (or the
   current documented equivalent): add/subtract/common operation enum,
   target/tool body selection semantics, what happens to the tool body
   (consumed? kept?), and the feature-tree result (one body? which
   body?). Copy-ready VBA.
2. Body registry questions: `IPartDoc.GetBodies2` (owner correction
   from brief 11: IPartDoc, not IModelDoc2) ordering guarantees, and
   how a macro addresses "the body produced by feature N" after further
   features (feature-based body scoping? `IFeature.GetBodies2`?).
3. Move/copy body: `IPartDoc.MoveCopyBody` (or current documented
   form) - transform input, copy vs move, and the feature created.
   SolidCraft `move` semantics (translate/rotate bodies between
   operations) map how?
4. Body/feature REMOVAL (SolidCraft `remove`): the documented delete
   entries (`IFeature` delete via `IModelDoc2.DeleteSelection2`?
   `IBody2`...?), what happens to dependent features, and how the
   acceptance layer verifies absence (named refusal when a delete
   cannot be verified).

## T4. Sweep and torus (Phase C input, R2)

1. `InsertProtrusionSweep`/`FeatureSweep` (current documented form):
   profile/path selection, thin/thick options, orientation control -
   copy-ready. R2's `sweep` maps to it how?
2. Torus: no native primitive exists - the documented way to build one
   (revolve of a circle profile about a coplanar axis? sweep?). What
   are the minimal sketch requirements and the direction/angle
   semantics (the project's revolve truth table pins apply)?

## Cross-checks

- Brief 09's release gate still holds for conics; brief 11's
  marshaling rules apply to EVERY array in T1/T3 readbacks.
- The emitter's constraints: `SetUserPreferenceToggle` is Void (verify
  by readback), ByRef Long outputs are uncallable from VBScript (the
  checker must not use them), `Val()` does not exist in VBScript,
  `IsNumeric`/`CDbl` are locale-poisoned, relation names are the
  SW2006+ table (docs/seat-acceptance.md).

## Output

A markdown report filed as `research/outputs/12-*.md`, evidence labels,
findings-at-a-glance table, conflicts table, copy-ready VBA signatures,
and an explicit "still needs a seat probe" table mapped onto the four
Phase B probes (body registry, merge/scope, edge-resolver, expression
prim placement) plus D04 move/remove. The consumer is the emitter's
feature-generation code and the acceptance layer's readbacks.
