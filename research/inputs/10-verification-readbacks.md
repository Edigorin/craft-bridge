# Research brief 10 - verification readbacks: mass properties, relation inventory, dimension state

Follow-up to briefs 08/09. The replay acceptance layer verifies rebuilt
models against references and verifies sketches against their sources;
three readback questions are open and are DOCUMENTATION-answerable (the
seat probes cover behavior, these are the API contracts behind them):

1. The checker's centroid acquisition receives `IMassProperty.CenterOfMass`
   components as OBJECTS through VBScript COM (seat evidence: "the rebuilt
   centroid component 0 is an object"; the same shape for the reference
   path). The documented return type and the marshaling rules decide the
   correct acquisition code.
2. Phase B must verify that a rebuilt sketch contains EXACTLY the
   serialized source relations - "verify relation inventory" (brief 08
   cross-report) - which needs the documented enumeration APIs.
3. The dimension verification distinguishes driving from reference
   (driven) dimensions and reads values back through the documented
   accessors; the current emitter reads SystemValue and behavioral
   effects only.

## Ground rules (same as briefs 03/04/07/08/09)

- Public sources only: SOLIDWORKS API Help (all reachable editions),
  official examples, pinned mirrors where pages are missing, pinned
  third-party code, long-lived forum solutions. No licensed seat.
- Label every claim: **Recorded** (official declaration/example),
  **Community** (third-party code; usage not success), **Conflicting**
  (sources disagree; keep both), **Reconstructed / untested** (derived;
  needs a seat check before use).
- Quote signatures exactly; write them copy-ready. Units: metres/radians
  unless stated. SOLIDWORKS is a Dassault Systemes trademark; nominative
  reference only.

## T1. IMassProperty: return types and COM marshaling

1. The exact declarations of `IMassProperty` members the checker uses or
   may use: `Volume`, `CenterOfMass`, `PrincipalAxesOfInertia`,
   `MomentsOfInertia`, `SurfaceArea`, `Density`, and any `Get*` variants.
   For each: the declared return type (Double, Double(), Variant,
   Object...), the array's element type and base, and the 2026-vs-older
   differences.
2. How a `Double()` (SAFEARRAY) return marshals into VBScript through
   IDispatch: when do elements arrive as plain numeric Variants and when
   as wrapper objects? Cite the strongest documentation or interop
   evidence available; if the public sources are silent, say so plainly
   and give the strongest RECONSTRUCTED acquisition pattern (the safe
   forms: `VarType`/`TypeName` per element, `IsObject` before assignment,
   default-property coercion rules in VBScript).
3. `CreateMassProperty()` vs `CreateMassProperty2()` (and any newer):
   signatures, differences, which the 2026 host documents, and whether
   mass properties require a rebuild/save state to be current (this
   matters for a just-built unsaved part).
4. Known VBScript/VBA dialect hazards for these returns (the repo already
   pins: VBScript has no `Val`; `IsNumeric`/`CDbl` are locale-sensitive;
   `SetUserPreferenceToggle` is a void Sub; ByRef Long outputs raise Type
   mismatch from VBScript on this seat). Any documented fixes for
   array-returning properties.

## T2. Sketch relation inventory (verification readback)

1. The documented APIs to enumerate a sketch's relations/constraints
   after creation: `ISketch` members (e.g. GetConstraintsCount/2,
   relation collections, `ISketchRelation` if it exists), and the
   feature-level equivalents. Exact signatures and the relation-type
   vocabulary (how each relation kind identifies itself in the readback:
   coincident, fix, horizontal, vertical, parallel, perpendicular,
   tangent, equal, distance...).
2. How to distinguish a relation the API created from one the solver
   INFERRED (the replay forbids inferred extras; brief 08's rule "add
   only serialized SolidCraft relations"). Any documented flags
   (inferred/added-by-user/solver-added).
3. Driving vs reference relations/dimensions in the readback: any
   documented state on relations equivalent to a dimension's driven
   flag.
4. Deleting or identifying relations by owner (for the degrade/recreate
   path's verification that a wipe really removed them).

## T3. Dimension state readback

1. `IDimension`/`IDisplayDimension` members for: driving vs driven
   (reference) state, the current value vs the driving value
   (`SystemValue` is obsolete per brief 08; `GetSystemValue3`/`SetSystemValue3`
   status enums), value per configuration, and the dimension's owner
   feature/sketch. Exact signatures.
2. The documented meaning of `IDisplayDimension.Type2` values (the
   swDimensionType_e table) as a readback of the CREATED kind - the
   emitter's classified-axis dims should verify against it (brief 09/08
   recommended this).
3. Dimension naming: `Name` vs `FullName` vs `SystemName` semantics for
   sketch dimensions (the equation layer binds names; the readback must
   agree with what `IEquationMgr` stores).
4. Any documented readback for "the dimension is driven by an equation"
   vs a manual value (the replay binds equations; verification should
   distinguish them).

## Output

A markdown report filed as `research/outputs/10-*.md`, evidence labels,
findings-at-a-glance table, conflicts table, copy-ready signatures, and
an explicit "still needs a seat probe" table. The consumer is the
craft-bridge acceptance layer (tools/check-part.vbs) and the emitter's
verification code: every readback it uses must be the documented member
with the documented semantics, and every acquisition must be
fail-closed with named refusals.
