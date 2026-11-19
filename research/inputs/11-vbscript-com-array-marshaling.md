# Research brief 11 - VBScript/IDispatch COM array marshaling and array-returning SolidWorks members

Follow-up to brief 10 (verification readbacks). D01 run 16 (seat,
2026-11-19) turned brief 10's unresolved centroid question into a precise
observed shape, and the same shape now gates the whole acceptance layer -
every inventory readback brief 10 recommends is an array-returning COM
member consumed from VBScript.

## The observed shape (seat evidence, not to be re-derived)

`tools/check-part.vbs` (cscript, Windows PowerShell-hosted scheduled
task, SolidWorks 2026) reads `IMassProperty.CenterOfMass`, documented as
a zero-based `Double[3]` (brief 10 [M1]). On BOTH the rebuilt part and
the STEP reference:

- the property read itself succeeds (no error);
- `IsArray(raw)` = True; `LBound`/`UBound` succeed and give exactly
  three elements (bounds accepted by the shape check);
- EVERY indexed element access `raw(i)` raises err 13 Type mismatch -
  plain assignment `comp = raw(i)`, the `Set comp = raw(i)` fallback,
  and even `VarType(raw(i))` / `TypeName(raw(i))` (the evaluation of the
  element itself raises, so no type identity of an element is readable);
- the container itself is therefore readable, the elements are not.

Leading hypothesis (unproven): the property marshals as a TYPED
SAFEARRAY (e.g. VT_ARRAY | VT_R8) inside the Variant, and VBScript late
binding cannot index non-Variant safe arrays - while `IsArray`,
`LBound`/`UBound` and possibly `For Each` (IEnumVARIANT) still work.
Alternative hypotheses welcome: VT_ARRAY | VT_UNKNOWN / VT_DISPATCH of
non-IDispatch interfaces, a wrapper object with a parameterized default,
or a host-level marshaling quirk of this PowerShell->cscript path.

## Ground rules (same as briefs 03/04/07/08/09/10)

- Public sources only: Microsoft Learn / MSDN / VBScript and VBA
  language references, VB6/COM interop documentation, SOLIDWORKS API
  Help (all reachable editions), official examples, pinned mirrors,
  long-lived KB articles and forum solutions with lasting value. No
  licensed seat.
- Label every claim: **Recorded** (official declaration/doc),
  **Community** (third-party; usage not success), **Conflicting**
  (sources disagree; keep both), **Reconstructed / untested** (derived;
  needs a seat check before use).
- Quote signatures and language rules exactly; write them copy-ready.
  SOLIDWORKS is a Dassault Systemes trademark; nominative reference only.

## T1. VBScript/IDispatch array marshaling rules

1. The exact wire-level semantics of Variant/SAFEARRAY delivery to
   VBScript late binding: which `VT_ARRAY | <type>` combinations can
   VBScript index with `arr(i)`, and what does each unsupported
   combination raise (error number and text)? Cite the strongest
   Microsoft documentation of the limitation ("VBScript cannot access
   elements of an array of a type other than Variant" or similar), and
   say plainly if the public docs are silent.
2. What `IsArray`, `VarType` (the vbArray flag combination),
   `TypeName`, `LBound`/`UBound` return for a typed SAFEARRAY in a
   Variant. Does `VarType(raw)` itself succeed where `VarType(raw(i))`
   raises?
3. `For Each` over COM-returned arrays: IEnumVARIANT semantics for
   typed safe arrays - does enumeration deliver the elements when
   indexing cannot? Ordering guarantees for 1-D arrays? What raises when
   the array has no enumerator?
4. The documented acquisition patterns that DO work from VBScript for a
   typed array: For Each, any coercion idioms (`CVar`, `Array()`), the
   role of `ByRef` vs `ByVal` in the receiving COM signature, and the
   VBA-dialect differences (VBA CAN index typed arrays - Dim ... As
   Double() - VBScript cannot declare types; cite both).
5. Any documented reason the PowerShell->cscript hosting path would
   change this (the checker runs under a scheduled task invoking
   cscript; the COM object is obtained via
   CreateObject("SldWorks.Application")).

## T2. The array-returning SolidWorks members the acceptance layer consumes

For each member below: the declared return type in the interop
documentation, the documented element type/order/base, any
ByRef/out parameters that VBScript late binding cannot pass (the
RunMacro2/LoadFile4 precedent), and the safest documented VBScript
consumption pattern. Call out every member that is UNCALLABLE from
VBScript for the same ByRef reason, and give its callable alternative.

1. `IMassProperty.CenterOfMass` / `IMassProperty2.CenterOfMass` -
   Double[3] (brief 10 [M1]/[M6]).
2. `IModelDocExtension.GetMassProperties2` - the flat 12-double array;
   its full signature including the status/configuration parameters:
   are any of them ByRef/out? Is it callable from VBScript at all?
   (brief 10 [M13] flagged late-binding complexity here - pin it down.)
3. `IBody2.GetMassProperties(Density)` - Double[12] per body (brief 10
   [P2]).
4. `IDimension.GetSystemValue3` / `SetSystemValue3` - the Object
   return of configuration values and the config-name argument shapes
   (brief 10 [D4]/[D5]).
5. `ISketchRelationManager.GetRelations` / `GetRelationsCount` -
   Object array of relation objects (brief 10 [R2]/[R3]).
6. `ISketchRelation.GetDefinitionEntities2` / `GetEntities` /
   `GetEntitiesType` / `GetEntitiesCount` (brief 10 [R5]/[R6]/[R9]).
7. `IModelDoc2.GetBodies2(BodyType, BVisibleOnly)` - the Object array
   of Body2 the checker already counts (its count path works today -
   how does its element access behave?).
8. `IMassProperty.PrincipleAxesOfInertia` / `PrincipleMomentsOfInertia`
   / `GetMomentOfInertia` - Double[3]/Double[9] (brief 10 [M8]-[M10]).
9. `IModelDoc2.GetFirstDocument`/`GetNext` chains - object traversal
   (works today; note what makes it different from arrays).

## T3. Fixtures and probe design (what the seat must still prove)

1. A minimal synthetic fixture that reproduces the run-16 shape outside
   SolidWorks if possible (a COM object whose property returns a typed
   safe array) - can the failure be reproduced and the fix proven
   without the seat? If a standard Windows COM object exposes one
   (Scripting.Dictionary variants, ADODB, Excel's Range.Value as a 2-D
   Variant array...), give the exact repro snippet.
2. For the seat probe already planned (mass-com-container): the exact
   capture set you would add beyond container identity, per-element
   errors and For Each - e.g. `VarType(raw) And vbArray`, enumeration
   count vs bounds, `GetMassProperties2` cross-check values.
3. Acceptance criteria for choosing an acquisition form: exactly three
   finite numerics, values agreeing across two documented paths
   (CenterOfMass vs GetMassProperties2's first three entries), and the
   named refusal set when any check fails.

## Output

A markdown report filed as `research/outputs/11-*.md`, evidence labels,
findings-at-a-glance table, conflicts table, copy-ready snippets, and an
explicit "still needs a seat probe" table. The consumer is
tools/check-part-logic.vbs (CentroidText/ComponentNumber) and the future
relation/dimension inventory readbacks from brief 10: every acquisition
must stay fail-closed with named refusals, and object-valued elements
are refused until their source is understood (brief 10's rule).
