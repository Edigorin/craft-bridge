# Verification readbacks 10

**Filed:** 2026-11-19, produced by the owner's research agent, delivered
as a chat attachment and filed verbatim. Answer to
`research/inputs/10-verification-readbacks.md`.
Provenance note: the report's labels are its own (Recorded/Community/
Conflicting/Reconstructed-untested/Seat evidence/Source-supplied). Its
headline findings reshape the acceptance layer: `IMassProperty.CenterOfMass`
is DOCUMENTED as a zero-based Double[3] and the seat's object-valued
elements are UNEXPLAINED (the report forbids silent default-property/.Value
coercion and demands the P0 `mass-com-container` probe before any
conversion is trusted); `CreateMassProperty()` is obsolete against
`CreateMassProperty2()`; the relation inventory is
`ISketch.RelationManager.GetRelations(swAll)` with
`GetDefinitionEntities2()` operands (`GetEntities()` may return internal
generated segments); NO public inferred-vs-authored relation flag exists
(staged snapshots + inference-off are the only provenance evidence); the
dimension readbacks are `DrivenState` (0 unknown / 1 driven / 2 driving),
`IDisplayDimension.Type2` (11 horizontal / 12 vertical / 2 linear / 3
angular / 5 radius / 6 diameter), `GetSystemValue3` array-shaped, and
`Name`/`FullName` vs `IEquationMgr` left-hand sides are NOT
interchangeable. The worked probe code in T1.3/T2.5/T3.4 is
reconstructed-untested and carries the report's own strict-refusal
contract.

---

# Research 10 — SOLIDWORKS verification readbacks

**Date:** 19 November 2026  
**Consumer:** `craft-bridge`, SOLIDWORKS 2026 replay emitter and `tools/check-part.vbs`  
**Scope:** T1–T3 of `10-verification-readbacks.md`, plus cross-report checks  
**Execution:** Public-source research. **No SOLIDWORKS 2026 seat, VBScript COM host, or replay macro was executed.**

SOLIDWORKS is a trademark of Dassault Systèmes. Its name identifies the API under study. No affiliation is implied.

## Evidence labels

- **Recorded:** An official SOLIDWORKS API reference, enum page, or official example gives the stated declaration or behavior. A reference page alone does not prove a particular target-seat result.
- **Community:** A third-party example demonstrates source-level usage, not verified execution. This report does not rely on community code where official references suffice.
- **Conflicting:** Documentation, terminology, or supplied seat observations differ. Both are retained.
- **Reconstructed / untested:** An implementation proposal assembled from documented interfaces. It needs a SOLIDWORKS 2026 seat test.
- **Seat evidence (supplied):** An observation quoted in the research brief. It was not repeated in this investigation.
- **Source-supplied:** A fact taken from the earlier SolidCraft/SOLIDWORKS research attachments. It is not a new runtime test.

## Findings at a glance

| Area | Finding | Evidence | Emitter/checker action |
|---|---|---|---|
| Centroid return | `IMassProperty.CenterOfMass` and `IMassProperty2.CenterOfMass` have an `Object` return declaration that represents an array of three doubles. The documented elements are **not COM objects**. | **Recorded** [M1], [M6] | Inspect the container and every element. Refuse an object-valued element until its source is understood. |
| Legacy/new mass API | `CreateMassProperty()` is obsolete. `CreateMassProperty2()` creates `IMassProperty2`, available from SOLIDWORKS 2020. | **Recorded** [M4], [M5] | Pin body scope, units, and configuration. Compare old and new APIs on the seat. |
| Recalculation | Old `CreateMassProperty()` explicitly claims current recalculated values even when `NeedsRebuild` is set. New `IMassProperty2.Recalculate()` is available. No save-before-read requirement was found. | **Recorded** [M4], [M7] | Rebuild to verify geometry, then recalculate properties. Do not treat valid mass properties as proof of a valid feature history. |
| Relation inventory | `ISketch.RelationManager.GetRelations(0)` enumerates relation objects. `GetRelationsCount(0)` supplies the expected count. | **Recorded** [R1], [R2], [R3] | Compare a normalized multiset of relation types, definition entities, suppression state, and associated dimensions. |
| Relation operands | `GetDefinitionEntities2()` returns the actual definition entities. `GetEntities()` may return internal generated entities instead. | **Recorded** [R5], [R6] | Prefer definition entities for source identity checks. |
| Inferred provenance | A public `ISketchRelation` property identifying *automatically inferred* versus *explicitly authored* relations was **not found**. | **Reconstructed / untested**, API member inventory [R7] | Disable inference during drawing and inspect relation changes after each insertion. Final inventory alone does not prove provenance. |
| Dimension state | `IDimension.DrivenState`: `0` unknown, `1` driven, `2` driving. | **Recorded** [D1], [D2] | Fail if unknown. Verify reference/driving intent for each emitted dimension. |
| Dimension kind | `IDisplayDimension.Type2`: general linear `2`, horizontal linear `11`, vertical linear `12`, angular `3`, radius `5`, diameter `6`. | **Recorded** [D3] | Check type separately from value, including horizontal/vertical axis classification. |
| Dimension value | `GetSystemValue3()` returns an `Object` containing configuration-aware numeric values. `SetSystemValue3()` returns a status enum; `0` means success. | **Recorded** [D4], [D5], [D6] | Check array shape, status, value, configuration, and post-rebuild geometry. |
| Dimension identity | `Name`, `FullName`, and equation-manager left-hand sides are not interchangeable strings. | **Recorded** [D8], [D9], [D12] | Resolve the owning feature and compare against the actual equation text. Never invent `D1@SketchN`. |

---

## T1. Mass-property readbacks and COM acquisition

### T1.1 `IMassProperty` legacy member contract

The following are .NET interop declarations of COM members. `System.Object` on an array-returning member does **not** mean that the elements are objects. The method description defines their contents. For this interface the documented array bases are zero. [M1]–[M4]

| Member | Published return shape | Data / order | Source |
|---|---|---|---|
| `CenterOfMass` | `ReadOnly Property CenterOfMass As System.Object` | Zero-based `Double[3]`: `x,y,z`, normally metres | [M1] |
| `Volume` | `ReadOnly Property Volume As System.Double` | Scalar volume, normally m³ | [M2] |
| `SurfaceArea` | `ReadOnly Property SurfaceArea As System.Double` | Scalar area, normally m² | [M3] |
| `Density` | `ReadOnly Property Density As System.Double` | Scalar density, normally kg/m³ | [M3a] |
| `Mass` | `ReadOnly Property Mass As System.Double` | Scalar mass, normally kg | [M3b] |
| `PrincipleAxesOfInertia(Axis As Integer)` | `ReadOnly Property ... As System.Object` | Zero-based `Double[3]` unit vector; axis index `0..2` | [M8] |
| `PrincipleMomentsOfInertia` | `ReadOnly Property ... As System.Object` | Zero-based `Double[3]`: principal moments | [M9] |
| `GetMomentOfInertia(WhereTaken As Integer)` | `Function ... As System.Object` | Zero-based `Double[9]`: tensor entries in the documented order | [M10] |

**Recorded — spelling difference:** The legacy names use `Principle...`, not `Principal...`. They remain spelled that way in the SOLIDWORKS 2026 reference. The modern `IMassProperty2` interface uses `Principal...`. Do not normalize COM member spellings. [M8], [M11]

**Recorded — tensor order:** The legacy `GetMomentOfInertia` result uses `Lxx,Lxy,Lxz,Lyx,Lyy,Lyz,Lzx,Lzy,Lzz`. The principal moments and principal axes are different outputs. They must not be compared without a common coordinate frame and unit convention. [M9], [M10]

**Recorded — scope:** The legacy creator uses all available solid bodies unless the caller specifies bodies with `IMassProperty.AddBodies`. If no coordinate system is set, its reference frame is the document origin. No claim is made about a non-solid or invalid body being included. [M4]

### T1.2 `IMassProperty2` and creator differences

Copyable interface-level signatures:

```vb
' IModelDocExtension — COM member declarations represented in VBA form.
Function CreateMassProperty() As SldWorks.MassProperty
Function CreateMassProperty2() As Object

' IMassProperty2 — selected members.
Property Get CenterOfMass() As Object          ' Documented Double(0 To 2)
Property Get Volume() As Double
Property Get SurfaceArea() As Double
Property Get Density() As Double
Property Get Mass() As Double
Property Get PrincipalMomentsOfInertia() As Object  ' Double(0 To 2)
Property Get PrincipalAxesOfInertia(ByVal Axis As Long) As Object
Function GetMomentOfInertia(ByVal WhereTaken As Long) As Object
Function Recalculate() As Boolean
```

The block expresses the member contracts for readers and generators. It is **not** a valid stand-alone VBA `Interface` declaration. In VBA, call these properties on SOLIDWORKS objects. Sources: [M4]–[M7], [M11], [M12].

| Difference | `CreateMassProperty()` / `IMassProperty` | `CreateMassProperty2()` / `IMassProperty2` |
|---|---|---|
| Introduced | SOLIDWORKS 2003 SP2 | SOLIDWORKS 2020 FCS, revision 28.0 |
| API status | Obsolete | Current replacement listed by SOLIDWORKS |
| Return | `MassProperty` interface object | `Object` holding an `IMassProperty2` interface |
| Scope | Default all solid bodies; `AddBodies` can set subset | Selected items (bodies/components), selection conditions, and `SelectedItems` must be controlled |
| Refresh | Creator explicitly reports recalculated values regardless of `NeedsRebuild` | `Recalculate()` returns a Boolean; use after changing overrides or hidden-body inclusion |
| Units | System/document units governed by `UseSystemUnits` | System/document units governed by `UseSystemUnits` |
| Principal-axis names | `PrincipleAxesOfInertia` / `PrincipleMomentsOfInertia` | `PrincipalAxesOfInertia` / `PrincipalMomentsOfInertia` |

**Recorded:** `CreateMassProperty2()` can return `Nothing` when no suitable solid-body selection is available. The documented behavior depends on selected bodies/components and on the model type. The checker must not assume it always measures the full model. [M5]

**Reconstructed / untested:** For parity with an entire-part reference, select or specify the intended solid bodies explicitly, then set `UseSystemUnits = True` and check `Recalculate()` before reading. Record selected-body identities and count as part of the verification evidence. Confirm precise selection semantics on the target seat. [M5]–[M7]

**Recorded:** Neither creator method documents a requirement that the part must first be saved. The older API explicitly states that it calculates current mass properties despite `IModelDocExtension.NeedsRebuild`. This does **not** prove that the current feature tree is free of rebuild errors. A successful mass read does not certify the replay. [M4]

### T1.3 Why `CenterOfMass(0)` can appear as an object

**Seat evidence (supplied):** `tools/check-part.vbs` reported that centroid component 0 was an object, both on the rebuilt and reference acquisition paths. The brief does not include the exact value of `VarType`, `TypeName`, or the code that first stored the returned value.

**Recorded:** `CenterOfMass` returns `System.Object` at the interop declaration. The documented **property value** is an array of three doubles. [M1], [M6]

**Recorded:** VB/VBA `VarType` identifies COM/Variant categories. The `vbObject` value is `9`; `vbDouble` is `5`; `vbArray` is `8192`. `IsObject` can be true for an object reference whose value is `Nothing`. `Set` assigns object references, while numeric array elements should be read with ordinary assignment. These Microsoft language references do not prove exactly how the target VBScript/IDispatch stack marshals a particular SOLIDWORKS value. [V1]–[V3]

**Unresolved:** No official SOLIDWORKS source was found that explains a `VT_DISPATCH` or other COM object inside `CenterOfMass`'s documented `Double[3]`. Plausible causes include a different upstream return, a wrong variable being indexed, a wrapper introduced by the host, or a mistaken `IsObject` diagnostic. None is established. Do not silently use an object's default property, `.Value`, or `CDbl(object)` as a corrective conversion.

**Reconstructed / untested — strict VBScript acquisition:** This is a diagnostic and fail-closed acquisition function for `check-part.vbs`. It assumes that the caller already obtained the correct mass-property object. It accepts only documented double elements. It never coerces strings or COM objects.

```vbscript
Option Explicit

Const VT_DOUBLE = 5

Sub Refuse(code, detail)
    WScript.Echo "FAIL " & code & " " & detail
    WScript.Quit 20
End Sub

Function CenterOfMass3(mp, tag)
    Dim raw, lower, upper, i, v, out(2), errNo

    If mp Is Nothing Then Refuse "MASS_NULL", tag
    On Error Resume Next
    Err.Clear
    raw = mp.CenterOfMass
    errNo = Err.Number
    On Error GoTo 0
    If errNo <> 0 Then Refuse "MASS_COM_GET", tag & " Err=" & errNo

    If IsObject(raw) Then Refuse "MASS_ARRAY_OBJECT", tag
    If Not IsArray(raw) Then Refuse "MASS_NOT_ARRAY", tag

    On Error Resume Next
    Err.Clear
    lower = LBound(raw): upper = UBound(raw)
    errNo = Err.Number
    On Error GoTo 0
    If errNo <> 0 Then Refuse "MASS_BOUNDS", tag & " Err=" & errNo
    If lower <> 0 Or upper <> 2 Then _
        Refuse "MASS_BAD_SHAPE", tag & " bounds=" & lower & ":" & upper

    For i = 0 To 2
        ' Do not assign an unexpected object to a numeric Variant.
        If IsObject(raw(i)) Then _
            Refuse "MASS_ELEMENT_OBJECT", tag & " index=" & i
        If IsNull(raw(i)) Or IsEmpty(raw(i)) Then _
            Refuse "MASS_ELEMENT_EMPTY", tag & " index=" & i
        If VarType(raw(i)) <> VT_DOUBLE Then _
            Refuse "MASS_ELEMENT_TYPE", tag & " index=" & i & _
                " vartype=" & VarType(raw(i)) & _
                " typename=" & TypeName(raw(i))
        v = raw(i)
        out(i) = v
    Next
    CenterOfMass3 = out
End Function
```

`CenterOfMass3` is **not seat-proven**. The requirement for subtype `vbDouble` is deliberately strict. A legitimate wrapper or alternate numeric subtype will produce a named refusal, which the seat probe can then examine. Add finite-number and reasonable-magnitude checks at the host/serializer boundary before accepting machine-generated values.

**Recommended diagnostic output on failure:** `IsArray(raw)`, `VarType(raw)`, `TypeName(raw)`, `LBound`, `UBound`, and per-element `IsObject`, `VarType`, `TypeName`. If an element is an object, use `Set suspect = raw(i)` in an isolated diagnostic. Read `TypeName(suspect)` without invoking an undocumented default property. Do not use that diagnostic assignment in the accepted conversion path.

**Source-supplied VBScript hazards:** The earlier VBA/VBScript research reports that `.vbs` lacks `Val`, typed `Dim ... As ...`, and VBA numeric literal type suffixes. It warns that `CDbl` and `IsNumeric` can be locale-sensitive. The brief also reports ByRef `Long` type mismatches on the seat. Keep the mass checker free of such conversions. [P1]

### T1.4 Alternative mass-property arrays

**Recorded:** `IModelDocExtension.GetMassProperties2` exposes a legacy, zero-based 12-double mass array. Its order is center coordinates (3), volume, area, mass, then inertia values. It has status/configuration parameters that can complicate VBScript late binding. The API marks it obsolete in favor of newer mass-property acquisition. [M13]

**Source-supplied:** `IBody2.GetMassProperties(Density)` also returns a 12-element array, but it measures a **specific body** and uses caller-supplied density. It is not an automatic replacement for a scoped, multi-body part read. [P2]

**Reconstructed / untested:** Use the alternate arrays only in a separate cross-check probe. Do not switch the acceptance checker to them merely because its current centroid path returns unexpected objects. Ensure the compared body sets, frame, unit system, and density are the same.

---
## T2. Sketch relation inventory

### T2.1 Documented enumeration API

**Recorded:** A sketch owns an `ISketchRelationManager`, available through `ISketch.RelationManager`. The manager has the following methods. `GetRelations` is the Automation-compatible form. The parallel `IGetRelations` form is an in-process pointer API and is explicitly unsupported for VBA. It must not appear in a `.vbs` checker. [R1]–[R4]

```vb
' Relevant .NET interop declarations, shown in VBA-style form.
' These are method signatures, not a class or interface definition to paste.
Property Get ISketch.RelationManager() As SldWorks.SketchRelationManager
Function ISketchRelationManager.GetRelations(ByVal Filter As Long) As Object
Function ISketchRelationManager.GetRelationsCount(ByVal Filter As Long) As Long
Function ISketchRelationManager.DeleteRelation( _
    ByVal ThisRelation As SldWorks.SketchRelation) As Boolean
Function ISketchRelationManager.DeleteAllRelations() As Boolean

Function ISketchRelation.GetRelationType() As Long
Function ISketchRelation.GetEntities() As Object
Function ISketchRelation.GetDefinitionEntities2() As Object
Function ISketchRelation.GetEntitiesType() As Object
Function ISketchRelation.GetEntitiesCount() As Long
Function ISketchRelation.GetDisplayDimension() As Object
Property Get ISketchRelation.Suppressed() As Boolean
```

**Recorded:** The filter `swSketchRelationFilterType_e` is not the same enum as `swConstraintType_e`. These are its documented values. [R4a]

| Filter | Value | Intended selection |
|---|---:|---|
| `swAll` | 0 | All relations |
| `swDangling` | 1 | Dangling relations |
| `swOverDefining` | 2 | Overdefining relations |
| `swExternal` | 3 | External relations |
| `swDefinedInContext` | 4 | In-context relations |
| `swLocked` | 5 | Locked relations |
| `swBroken` | 6 | Broken relations |
| `swSelectedEntities` | 7 | Relations on selected entities |

Use `GetRelationsCount(swAll)` and `GetRelations(swAll)` for the complete inventory. Additional filters help classify errors but are not replacements for `swAll`. [R2], [R3]

**Recorded:** `ISketchSegment.GetRelations()` can inspect relations on one segment. It is not a complete substitute for the sketch manager. It can miss relations involving only points or other entities. [R12]

**Recorded:** The official *Get All Sketch Relations and Their Types* VBA example obtains a selected feature, calls `GetSpecificFeature2`, reads `RelationManager`, then calls `GetRelations(swAll)` and each relation's `GetRelationType`. This establishes the usage sequence. The sample is not a complete fail-closed checker. [R11]

### T2.2 Relation type vocabulary

**Recorded:** `ISketchRelation.GetRelationType()` returns `swConstraintType_e` values. Representative values required by the replay are below. Some source constraints have no direct single-relationship equivalent. [R8]

| SOLIDWORKS member | Value | Source-side comparison candidate |
|---|---:|---|
| `swConstraintType_DISTANCE` | 1 | Distance / length only after checking operand kind |
| `swConstraintType_ANGLE` | 2 | Angle |
| `swConstraintType_RADIUS` | 3 | Radius |
| `swConstraintType_HORIZONTAL` | 4 | Horizontal line |
| `swConstraintType_VERTICAL` | 5 | Vertical line |
| `swConstraintType_TANGENT` | 6 | Tangent |
| `swConstraintType_PARALLEL` | 7 | Parallel |
| `swConstraintType_PERPENDICULAR` | 8 | Perpendicular |
| `swConstraintType_COINCIDENT` | 9 | Coincident point entities |
| `swConstraintType_CONCENTRIC` | 10 | Concentric |
| `swConstraintType_SYMMETRIC` | 11 | Symmetric |
| `swConstraintType_ATMIDDLE` | 12 | Midpoint |
| `swConstraintType_SAMELENGTH` | 14 | Equal length |
| `swConstraintType_DIAMETER` | 15 | Diameter |
| `swConstraintType_FIXED` | 17 | Fixed entity |
| `swConstraintType_HORIZPOINTS` | 25 | Horizontal point-pair |
| `swConstraintType_VERTPOINTS` | 26 | Vertical point-pair |
| `swConstraintType_COLINEAR` | 27 | Collinear |
| `swConstraintType_ATPIERCE` | 40 | Pierce |
| `swConstraintType_ARCLENGTH` | 44 | Arc length |
| `swConstraintType_ALONGXPOINTS` | 51 | Distance along X (check operand representation) |
| `swConstraintType_ALONGYPOINTS` | 52 | Distance along Y (check operand representation) |
| `swConstraintType_CONICRHO` | 82 | Conic rho, if created/read as dimension relation |

**Source-supplied:** SolidCraft `solidcraft/57`, at the inspected v0.43.0 tag, has 37 serialized `ConstraintKind` variants. It stores point/curve **indices**, optional `param`, optional `driven`, and numeric values. Length units are mm and angles are radians. One SolidCraft command can emit several serialized constraints. The bridge therefore needs an explicit *mapping contract* from each serialized variant to one or more SOLIDWORKS relations/dimensions. It must not compare only a total count. [P3], [P4]

**Reconstructed / untested:** For each source constraint, define an expected normalized record:

```text
(sketch_feature_id,
 source_constraint_id,
 target_relation_type,
 operand_ids_and_roles,
 target_arity,
 suppression_state,
 dimension_kind_if_any,
 dimension_driven_state_if_any,
 system_value_if_any,
 configuration)
```

Use source operand indices to find the emitter's stable SOLIDWORKS sketch-point/segment objects. When the API returns an unexpected entity, unmatched relation, or unknown relation type, fail closed rather than silently map by coordinate proximity.

### T2.3 `GetEntities` versus `GetDefinitionEntities2`

**Recorded:** `GetEntities()` returns the entities associated with a relation. SOLIDWORKS may generate an internal sketch segment when the original selection uses an external edge or face. The method may then return that internal segment rather than the selected source entity. `GetDefinitionEntities2()` returns the actual entities that originally defined the relation. It was introduced in SOLIDWORKS 2014. [R5], [R6]

**Recorded:** `GetEntitiesType()` returns an array of `swSketchRelationEntityTypes_e` values. The enum includes `Point=2`, `Line=3`, `Arc=4`, `Ellipse=5`, `Spline=7`, `Plane=10`, and `Dimension=14`. The documented type enum does not provide a direct persistent source ID. [R9]

**Reconstructed / untested:** Prefer `GetDefinitionEntities2()` for operand identity. Use `GetEntitiesType()` as an independent diagnostic. Do not assume the type array always aligns one-to-one with **definition** entities when internal geometry is involved. Measure this on the seat for external references, offset-edge relations, and model-edge selections.

**Recorded:** SOLIDWORKS recommends checking the returned arrays for null objects. A successful relation count does not guarantee that every returned array element contains a valid relation or entity. [R2], [R6]

### T2.4 Relation state, provenance, and deletion

**Recorded:** `ISketchRelation.Suppressed` is a Boolean property. `GetDisplayDimension()` can return an `IDisplayDimension` for a relation with a display dimension. It does not promise a display dimension for every geometric relation. [R7], [R10]

**Unresolved:** No documented `ISketchRelation.IsInferred`, `CreatedAutomatically`, `AddedByUser`, or equivalent origin flag was found. The public relation-member list does not establish the creation path. A final-state comparison can prove that the **resulting inventory** matches, but it cannot prove which operation created a particular relation. [R7]

**Source-supplied:** Brief 08 reports that setting the application preference `swSketchInference` (249) to off stopped unwanted endpoint snapping on the target 2026 seat. Disabling only `swSketchSnapsMidPoints` (268) did not. The application preferences must be restored after replay. [P5]

**Reconstructed / untested:** Enforce the final inventory as a multiset. For better provenance evidence, log three states: after creating geometry, after each explicit relation, and after solve/rebuild. If a relation appears before the emitter adds it, mark it `UNEXPECTED_RELATION` even if a later source relation would match it. Avoid deleting an unexpected relation and silently continuing.

**Recorded:** `DeleteRelation(relation)` deletes a specified logical sketch relation. `DeleteAllRelations()` deletes all logical relations in the sketch. Their Boolean return values must be checked. After either call, enumerate relations again. [R13], [R14]

**Unresolved:** These deletion methods are described for *logical relations*. Whether all dimensional annotations, special relations, external dependencies, and fixed-geometry flags disappear requires separate tests. Do not assume that zero returned relations also means no sketch dimensions or no residual fixed state. The rebuild verification must cover both relation objects and display dimensions.

### T2.5 Copy-ready VBA relation inventory probe

**Reconstructed / untested:** This VBA macro runs on the selected sketch feature. It records count, types, suppression state, definition entities, and dimension links. It does not yet establish the emitter's stable source-to-target entity mapping. Run it on a disposable part first.

```vb
Option Explicit

Sub main()
    Dim model As SldWorks.ModelDoc2
    Dim sm As SldWorks.SelectionMgr
    Dim feat As SldWorks.Feature
    Dim sketch As SldWorks.Sketch
    Dim mgr As SldWorks.SketchRelationManager
    Dim rel As SldWorks.SketchRelation
    Dim dd As SldWorks.DisplayDimension
    Dim dimObj As SldWorks.Dimension
    Dim relations As Variant
    Dim defs As Variant
    Dim i As Long, j As Long, n As Long

    Set model = Application.SldWorks.ActiveDoc
    If model Is Nothing Then Err.Raise 5, , "No active document"
    Set sm = model.SelectionManager
    If sm.GetSelectedObjectCount2(-1) <> 1 Then _
        Err.Raise 5, , "Select exactly one sketch feature"
    Set feat = sm.GetSelectedObject6(1, -1)
    If feat Is Nothing Then Err.Raise 5, , "Not a feature"
    Set sketch = feat.GetSpecificFeature2
    If sketch Is Nothing Then Err.Raise 5, , "Not a sketch feature"
    Set mgr = sketch.RelationManager
    If mgr Is Nothing Then Err.Raise 5, , "Missing relation manager"

    n = mgr.GetRelationsCount(0)  ' swAll = 0
    Debug.Print "SKETCH="; feat.Name; " RELATIONS="; n
    If n = 0 Then Exit Sub
    relations = mgr.GetRelations(0)
    If Not IsArray(relations) Then _
        Err.Raise 5, , "Nonarray relation inventory"
    If UBound(relations) - LBound(relations) + 1 <> n Then _
        Err.Raise 5, , "Relation count mismatch"

    For i = LBound(relations) To UBound(relations)
        If Not IsObject(relations(i)) Then _
            Err.Raise 5, , "Relation is not an object"
        Set rel = relations(i)
        If rel Is Nothing Then Err.Raise 5, , "Null relation"
        Debug.Print "REL"; i; " TYPE="; rel.GetRelationType; _
                    " SUPPRESSED="; rel.Suppressed

        defs = rel.GetDefinitionEntities2
        If Not IsArray(defs) Then _
            Err.Raise 5, , "Missing definition-entity array"
        For j = LBound(defs) To UBound(defs)
            If Not IsObject(defs(j)) Then _
                Err.Raise 5, , "Definition entity is not an object"
            If defs(j) Is Nothing Then _
                Err.Raise 5, , "Null definition entity"
            Debug.Print "   OPERAND"; j; " "; TypeName(defs(j))
        Next j

        Set dd = rel.GetDisplayDimension
        If Not dd Is Nothing Then
            Set dimObj = dd.GetDimension2(0)
            If dimObj Is Nothing Then _
                Err.Raise 5, , "Relation display has no Dimension"
            Debug.Print "   DIM="; dimObj.FullName; _
                        " KIND="; dd.Type2; _
                        " STATE="; dimObj.DrivenState
        End If
    Next i
End Sub
```

**Scope limit:** `GetDefinitionEntities2()` returns an array by contract, but null/special cases and the exact result for relation types with no ordinary geometric operands remain seat-test items. The probe deliberately refuses them. A production checker must build persistent operand identities, not use only `TypeName`.

---
## T3. Dimension state and value readbacks

### T3.1 Documented member contract

A display dimension and its underlying dimension are different COM objects. Use `IDisplayDimension.Type2` for the displayed **kind**, and `IDimension` for the driven state and numerical value. [D1], [D3], [D7]

| Member | Published signature / return | Check | Source |
|---|---|---|---|
| `IDisplayDimension.GetDimension2(Index)` | `Function GetDimension2(ByVal Index As Integer) As Dimension` | Get underlying dimension. Index `0` for ordinary dimensions; chamfer dimensions have separate components. | [D7] |
| `IDisplayDimension.Type2` | `ReadOnly Property Type2 As Integer` | `swDimensionType_e` identifies linear/axis/angular/radial/diametric kind. | [D3] |
| `IDimension.DrivenState` | `Property DrivenState As Integer` | `swDimensionDrivenState_e`, read/write. | [D1], [D2] |
| `IDimension.IsReference` | Boolean property | Secondary readback for reference dimensions. Check consistency with `DrivenState`. | [D0] |
| `IDimension.ReadOnly` | Boolean property | Read-only flag. **Not equivalent** to `DrivenState`. | [D5] |
| `IDimension.GetSystemValue3(WhichConfigurations, Config_names)` | `Function ... As Object` | Numeric values in SOLIDWORKS system units, per selected configurations. | [D4] |
| `IDimension.SetSystemValue3(NewValue, WhichConfigurations, Config_names)` | `Function ... As Integer` | Status code in `swSetValueReturnStatus_e`. | [D5], [D6] |
| `IDimension.SystemValue` | Scalar Double property, obsolete | Do not use as the primary acceptance readback. | [D4a] |
| `IDimension.GetFeatureOwner()` | `Function ... As Feature` | Identify owning feature or sketch. | [D10] |
| `IDimension.Name` | `ReadOnly Property ... As String` | Short name, such as `D1`. | [D8] |
| `IDimension.FullName` | `ReadOnly Property ... As String` | Dimension + owning feature + model name. | [D9] |
| `IDimension.IsDesignTableDimension()` | `Function ... As Boolean` | Identifies design-table control, not a general equation binding. | [D11] |

**Recorded:** The official VBA example for reading dimension values obtains an `IDimension`, uses `GetSystemValue3`, indexes the result as `value(0)`, and prints `DrivenState`, `IsReference`, and `ReadOnly`. It shows array-shaped acquisition and distinguishes these properties. This example uses an older display-dimension accessor, so use the documented `GetDimension2` in new code. [D0], [D7]

**Recorded:** `swDimensionDrivenState_e` is:

| Name | Value | Meaning for replay |
|---|---:|---|
| `swDimensionDrivenUnknown` | 0 | Unknown: refuse certification |
| `swDimensionDriven` | 1 | Reference / measured dimension |
| `swDimensionDriving` | 2 | Driving / constraining dimension |

The `DrivenState` property is settable, but changing it can affect degrees of freedom and solve results. Verification code should **read**, not mutate, it. [D1], [D2]

### T3.2 Dimension kind: the `Type2` enum

**Recorded:** `swDimensionType_e` defines the following values. The dimension kind is not the same as the underlying parameter data type (`IDimension.GetType`, `swDimensionParamType_e`). [D3], [D3a]

| Enum member | Value | Meaning |
|---|---:|---|
| `swDimensionTypeUnknown` | 0 | Unknown |
| `swOrdinateDimension` | 1 | Ordinate |
| `swLinearDimension` | 2 | General linear |
| `swAngularDimension` | 3 | Angular |
| `swArcLengthDimension` | 4 | Arc length |
| `swRadialDimension` | 5 | Radius |
| `swDiameterDimension` | 6 | Diameter |
| `swHorOrdinateDimension` | 7 | Horizontal ordinate |
| `swVertOrdinateDimension` | 8 | Vertical ordinate |
| `swZAxisDimension` | 9 | Z axis |
| `swChamferDimension` | 10 | Chamfer |
| `swHorLinearDimension` | 11 | Horizontal linear |
| `swVertLinearDimension` | 12 | Vertical linear |
| `swScalarDimension` | 13 | Scalar |
| `swRadialLinearDimension` | 14 | Doubled-distance radial |
| `swDiametricLinearDimension` | 15 | Doubled-distance linear |
| `swAngularOrdinateDimension` | 16 | Angular ordinate |

**Source-supplied comparison:** Brief 08 reports a target-seat success for `AddHorizontalDimension2` and `AddVerticalDimension2` on two selected sketch points. The acceptance layer must now verify that their created `Type2` values are `11` and `12`, respectively. The target-seat outcome was not independently retested here. [P5]

**Reconstructed / untested:** For a source `distance_x`, require horizontal-linear kind when the emitter uses a horizontal dimension. For `distance_y`, require vertical-linear kind. For a general distance, require general linear or a documented alternate mapping. Reject `Type2 = 0`. A value of the correct magnitude with the wrong axis kind is a replay failure.

### T3.3 Configuration-aware value readback

Copyable method declarations:

```vb
' IDimension (interop signatures rendered in VBA notation)
Function GetSystemValue3( _
    ByVal WhichConfigurations As Long, _
    ByVal Config_names As Object) As Object

Function SetSystemValue3( _
    ByVal NewValue As Double, _
    ByVal WhichConfigurations As Long, _
    ByVal Config_names As Object) As Long
```

**Recorded:** `GetSystemValue3` provides system-unit numerical values and accepts a configuration scope. `swInConfigurationOpts_e` defines `swThisConfiguration=1`, `swAllConfiguration=2`, and `swSpecifyConfiguration=3`. The official VBA example indexes the return as an array even for a current-configuration read. [D4], [D0], [D15]

**Recorded:** `SetSystemValue3` uses `swSetValueInConfiguration_e` for its scope, which is a **different enum** from the getter's scope. The commonly used meanings are current, all, or specified configurations. When specified configurations are requested, the caller provides one name or a list. For a single-configuration part, SOLIDWORKS can ignore the scope selection. [D5], [D15a]

| `swSetValueReturnStatus_e` | Code | Meaning |
|---|---:|---|
| `swSetValue_Successful` | 0 | Change accepted |
| `swSetValue_Failure` | 1 | General failure |
| `swSetValue_InvalidValue` | 2 | Invalid numeric value |
| `swSetValue_DrivenDimension` | 3 | Value cannot be set on a dimension driven by geometry |
| `swSetValue_ModelNotLoaded` | 4 | Model not loaded |
| `swSetValue_FrozenFeatureOwner` | 5 | Feature owner is frozen |

**Recorded — important terminology:** The `SetSystemValue3` help says it can change a **read-only** dimension. The result enum nevertheless includes an error for a **driven** dimension. A read-only flag and a reference/driven state are not the same concept. Do not resolve this apparent tension by treating reference dimensions as writable. [D5], [D6]

**Reconstructed / untested:** The acceptance checker should not write values. On a designated mutation probe, accept the set operation only when the return value is zero and the subsequent getter, solver state, and geometry match the request. A successful setter status alone does not prove that an under-constrained sketch moved as intended.

### T3.4 VBA readback probe for one display dimension

**Reconstructed / untested:** The following snippet expects an already-retrieved `IDisplayDimension`. It validates basic state and logs the system-value array. Place the function in a VBA macro module and call it on a disposable sketch. This is a read-only probe.

```vb
Option Explicit

Sub ProbeDisplayDimension(ByVal dd As SldWorks.DisplayDimension)
    Dim d As SldWorks.Dimension
    Dim owner As SldWorks.Feature
    Dim values As Variant
    Dim i As Long
    Dim state As Long, kind As Long

    If dd Is Nothing Then Err.Raise 5, , "DIM_NULL_DISPLAY"
    Set d = dd.GetDimension2(0)
    If d Is Nothing Then Err.Raise 5, , "DIM_NULL_DIMENSION"

    state = d.DrivenState
    kind = dd.Type2
    If state <> 1 And state <> 2 Then _
        Err.Raise 5, , "DIM_UNKNOWN_DRIVEN_STATE"
    If kind = 0 Then Err.Raise 5, , "DIM_UNKNOWN_KIND"

    Set owner = d.GetFeatureOwner
    If owner Is Nothing Then Err.Raise 5, , "DIM_UNKNOWN_OWNER"

    Debug.Print "DIM_NAME="; d.Name
    Debug.Print "DIM_FULL="; d.FullName
    Debug.Print "DIM_OWNER="; owner.Name
    Debug.Print "DIM_KIND="; kind; " DRIVEN_STATE="; state
    Debug.Print "DIM_REFERENCE="; d.IsReference

    values = d.GetSystemValue3(1, Empty) ' Current configuration
    If Not IsArray(values) Then _
        Err.Raise 5, , "DIM_SYSTEM_VALUES_NOT_ARRAY"
    If UBound(values) < LBound(values) Then _
        Err.Raise 5, , "DIM_EMPTY_VALUES"
    For i = LBound(values) To UBound(values)
        If IsObject(values(i)) Then _
            Err.Raise 5, , "DIM_VALUE_OBJECT"
        If VarType(values(i)) <> vbDouble Then _
            Err.Raise 5, , "DIM_VALUE_TYPE"
        Debug.Print "VALUE["; i; "]="; values(i)
    Next i
End Sub
```

**Seat probe required:** Confirm the array shape, type, and count for current, all, and selected configurations. The implementation is deliberately strict. It refuses an undocumented scalar or a numeric subtype other than Double.

### T3.5 Names, feature owners, and equation bindings

**Recorded:** `IDimension.Name` is the short dimension name. `IDimension.FullName` includes the short name, feature name, and model. The documented full syntax is:

```text
<DimensionName>@<FeatureName>@<ModelName>
```

An example in the API reference is `D1@Sketch1@Part4.Part`. `GetFeatureOwner()` yields the owning feature object. These readbacks are stronger than constructing dimension names from creation order. [D8]–[D10]

**Recorded:** `IFeature.Parameter("D1")` can retrieve a parameter relative to that feature without supplying the full document name. This is useful when validating an emitter's ledger after feature renaming. [D17]

**Unresolved:** No trustworthy `IDimension.SystemName` property was recovered from the checked official member indexes. The emitter must not call it on the assumption that it exists. Confirm the target type library if existing project code expects that name. [D18]

**Recorded:** `IEquationMgr.Equation(Index)` returns or sets the actual equation text. Its quoted left-hand side may use a qualified sketch/feature dimension such as `"D1@Sketch1"`. This is not always identical to `IDimension.FullName`, which includes the model. `IEquationMgr` also exposes equation count and properties for global variables, disabled equations, and configuration handling. [D12]

**Recorded:** `IDimension.IsDesignTableDimension()` detects design-table control. It does **not** certify that a dimension is controlled by an `IEquationMgr` equation. [D11]

**Reconstructed / untested — equation verification algorithm:**

1. Read each created dimension's `Name`, `FullName`, and owning feature.
2. Read the actual `IEquationMgr.Equation(i)` strings. Parse quoted left-hand sides without losing names, configuration context, or escaping.
3. Match an equation to a dimension using the feature owner and validated identity, not a guessed `D1@SketchN` name.
4. Record the complete expression string and the equation's enabled/configuration state. Compare them against the source `constraints[].param` and parameter expression.
5. Rebuild, then compare the evaluated dimension values and affected geometry. Refuse ambiguous equation matches.

**Source-supplied:** SolidCraft points may also carry `sketch.points[].exprs.x/y`, whose parameter-driven positions are fixed by the source model before solving. Such points can have no ordinary serialized `fix` constraint. Relation inventory, dimension inventory, and parameter-following coordinate checks must therefore remain separate verification channels. [P3], [P4]

---

## Cross-report findings for `craft-bridge`

1. **A complete sketch is more than its relation count.** The checker must compare relation type, definition entities, dimension kind, driven state, value, and equation binding. The source's fixed point expressions and curve `fixed` flags need separate treatment. A correct total count can hide omitted relations and unexpected inferred relations.
2. **The absence of a provenance flag changes the test design.** Brief 08's inference-preference restoration is a necessary drawing control. Staged inventory snapshots provide stronger evidence than a final comparison alone, but still do not provide a documented per-relation creation-origin field.
3. **The conic replay has an extra verification surface.** `swConstraintType_CONICRHO = 82` exists in the relation enum. For curves emitted as native SOLIDWORKS conics, inspect whether the system creates a conic-rho relation and whether its measured value matches SolidCraft's native `rho`. A weighted-spline fallback need not expose that relation. This is a **test candidate**, not a confirmed mapping. [R8], [P6]
4. **Cross-check dimension relations with display dimensions.** `ISketchRelation.GetDisplayDimension()` links the two API families where available. Avoid counting the same dimensional constraint twice. Also enumerate independent displayed dimensions, because the relation API does not guarantee that it exposes every display dimension. [R10]
5. **Coordinate and body scope are part of mass verification.** Comparing two `CenterOfMass` triples is meaningful only when both sides use the same units, coordinate frame, and bodies. New `IMassProperty2.SelectedItems` and `UseSystemUnits` require explicit checking. A centroid mismatch may be a scope error rather than a geometric error. [M5], [M6]
6. **Different naming conventions are API contracts.** `IMassProperty.Principle...` versus `IMassProperty2.Principal...`, `IDimension.GetType` versus `IDisplayDimension.Type2`, and `IDimension.FullName` versus equation left-hand sides require separate fields in the emitter. Normalize only for the internal model, never for the invoked COM member name.

## Conflicts and unresolved interpretation

| Question | Source A | Source B / observation | Resolution for current code |
|---|---|---|---|
| Centroid array elements | **Recorded:** `CenterOfMass` is a `Double[3]` in the API. [M1], [M6] | **Seat evidence:** VBScript reports an object at index 0 on two paths. | **Unresolved.** Log the exact COM type and acquisition path. Refuse object-valued elements. |
| Mass-property currency | **Recorded:** legacy creator says it recalculates even if `NeedsRebuild` is true. [M4] | A syntactically valid feature tree can still contain failed or unwanted geometry. | No direct contradiction. Validate rebuilt geometry separately from mass properties. |
| Original relation operands | **Recorded:** `GetEntities()` can return internal segments. [R5] | **Recorded:** `GetDefinitionEntities2()` returns defining entities. [R6] | Use definition entities for source mapping. Both may be logged. |
| Inferred versus authored relations | Seat shows inference can add unexpected relations. [P5] | No public provenance property found. [R7] | Prevent inference, compare staged snapshots, and refuse extras. Do not claim provenance from final state. |
| Dimension writable state | `SetSystemValue3` remarks mention changing read-only dimensions. [D5] | Status `DrivenDimension=3` rejects a geometry-driven dimension. [D6] | ReadOnly and driven are distinct. Respect the returned status. |
| Dimension names | `FullName` includes model. [D9] | Equation text may omit model qualification. [D12] | Normalize after checking actual owner and equation text. |
| Relation scope | `DeleteAllRelations` targets logical relations. [R14] | SolidCraft fixed point/curve state can exist outside that inventory. [P3] | After wipe, check dimensions, fixed flags, geometry, and remaining relations independently. |
| `SystemName` | Mentioned as an open question in brief 10 | Not recovered in checked `IDimension` reference | Treat it as absent until the 2026 type library proves otherwise. |

## Still needs a SOLIDWORKS 2026 seat probe

| Priority | Probe ID | Procedure | Accept / refuse condition |
|---|---|---|---|
| P0 | `mass-com-container` | Capture `VarType`, `TypeName`, `IsObject`, `IsArray`, bounds for both reference and rebuilt `CenterOfMass`. | Accept only zero-based numeric triples with verified component types. If object, record COM class and refuse. |
| P0 | `mass-new-vs-old` | On a fresh unsaved one-body part, compare `CreateMassProperty()` and `CreateMassProperty2()` with `UseSystemUnits=True`. Repeat on multibody and hidden-body cases. | Body scope, frame, volume, area, mass, centroid agree within test tolerances. |
| P0 | `rel-simple-mapping` | Create one sketch containing coincident, horizontal, vertical, tangent, equal, fixed, and distance relations. Enumerate all relations. | Correct types, counts, actual operands, and suppression state. |
| P0 | `rel-inference-provenance` | Create geometry with sketch inference off, then on. Snapshot relations after each geometry and relation call. | Unexpected relations are detected at their creation stage, even when final count matches. |
| P0 | `dim-axis-kind` | Create horizontal, vertical, general linear, and zero-axis-extent test dimensions. | `Type2=11/12/2` as appropriate; failures produce explicit refusal. |
| P0 | `dim-driven-reference` | Create a driving dimension and a reference dimension. Read `DrivenState`, `IsReference`, and `ReadOnly`. | Expected state and value. Unknown or inconsistent state fails. |
| P1 | `rel-definition-entities` | Make external-edge/face and offset relations. Compare `GetEntities`, `GetDefinitionEntities2`, and `GetEntitiesType`. | Definition operand identity matches authored selection. Null or extra operands fail. |
| P1 | `rel-dimension-link` | For distance/radius/conic-rho relations, call `GetDisplayDimension()` and `GetDimension2(0)`. | Check returned kind, state, values and expected absence for non-dimensional relations. |
| P1 | `dim-config-reads` | Read a dimension in one, all, and specified configurations. Log return variants and counts. | Stable typed arrays with per-configuration values. Refuse ambiguous order or shape. |
| P1 | `dim-set-status` | On a disposable model, attempt valid, invalid, driven, and frozen-owner writes. | Return codes follow documented enum; status 0 must also produce correct rebuilt geometry. |
| P1 | `dim-equation-owner` | Create equation-controlled and design-table-controlled dimensions, then rename sketch/feature. | Actual equation text and owner match the intended dimension. No guessed names. |
| P2 | `relation-delete-wipe` | Delete one relation, then all relations. Enumerate relation and dimension inventories and inspect solved state. | No residual prohibited relation/dimension/fixed state. |
| P2 | `inertia-names` | Call both legacy `Principle...` and modern `Principal...` interfaces on a known asymmetric solid. | Expected three/nine-double shapes and agreeing inertia in a common coordinate frame. |

## Recommended acceptance-layer refusal codes

| Code | Trigger |
|---|---|
| `MASS_COM_GET` | Mass-property getter fails |
| `MASS_NOT_ARRAY` / `MASS_BAD_SHAPE` | Centroid not exactly a zero-based triple |
| `MASS_ELEMENT_OBJECT` / `MASS_ELEMENT_TYPE` | Unsupported element value or COM type |
| `MASS_SCOPE_UNKNOWN` | Body/component selection not verified |
| `REL_ENUM_FAILED` | Cannot obtain complete `swAll` relation inventory |
| `REL_OPERAND_UNKNOWN` | Actual definition entity has no verified source identity |
| `REL_UNEXPECTED` / `REL_MISSING` | Normalized relation multiset differs |
| `REL_SUPPRESSED_MISMATCH` | A required relation is suppressed or an unexpected relation remains active |
| `DIM_UNKNOWN_STATE` / `DIM_KIND_MISMATCH` | Dimension state or kind differs from source intent |
| `DIM_VALUE_SHAPE` / `DIM_VALUE_MISMATCH` | Value array or evaluated value is not correct |
| `DIM_EQUATION_AMBIGUOUS` | Equation left-hand side does not resolve uniquely |
| `REBUILD_GEOMETRY_MISMATCH` | Rebuilt geometry differs despite successful API calls |

**Implementation priority:** Add strict acquisition and logging before changing geometry generation. Once both the reference and replay checkers use the same verified contract, introduce relation and dimension matching one feature at a time. An unsupported COM result must produce a named refusal, never a guessed conversion or an approximate pass.

---

## Source register

URLs point to official SOLIDWORKS API Help unless marked otherwise. Older editions are retained when they contain a more complete declaration or sample. The year on a page is the **documentation edition**, not a live test of SOLIDWORKS 2026.

### Mass properties and language interop

- **[M1]** [`IMassProperty.CenterOfMass`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty~CenterOfMass.html) — object declaration, three-double value.
- **[M2]** [`IMassProperty.Volume`, 2023](https://help.solidworks.com/2023/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imassproperty~volume.html) — scalar volume.
- **[M3]** [`IMassProperty.SurfaceArea`, 2023](https://help.solidworks.com/2023/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imassproperty~surfacearea.html) — scalar area.
- **[M3a]** [`IMassProperty.Density`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imassproperty~density.html) — read-only scalar density.
- **[M3b]** [`IMassProperty` member list, 2012](https://help.solidworks.com/2012/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty_members.html) — legacy mass-member inventory.
- **[M4]** [`IModelDocExtension.CreateMassProperty`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imodeldocextension~createmassproperty.html) — scope, coordinate system, and `NeedsRebuild` guarantee.
- **[M5]** [`IModelDocExtension.CreateMassProperty2`, 2022](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDocExtension~CreateMassProperty2.html) — current creator, selection semantics, availability.
- **[M6]** [`IMassProperty2.CenterOfMass`, 2026](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty2~CenterOfMass.html) — three-double array, recalculate conditions.
- **[M7]** [`IMassProperty2.Recalculate`, 2020](https://help.solidworks.com/2020/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty2~Recalculate.html) — Boolean refresh.
- **[M8]** [`IMassProperty.PrincipleAxesOfInertia`, 2026](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty~PrincipleAxesOfInertia.html) — legacy spelling and three doubles.
- **[M9]** [`IMassProperty.PrincipleMomentsOfInertia`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty~PrincipleMomentsOfInertia.html) — legacy spelling, three moments.
- **[M10]** [`IMassProperty.GetMomentOfInertia`, 2024](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty~GetMomentOfInertia.html) — nine values.
- **[M11]** [`IMassProperty2.PrincipalAxesOfInertia`, 2025](https://help.solidworks.com/2025/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty2~PrincipalAxesOfInertia.html) and [`PrincipalMomentsOfInertia`, 2024](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty2~PrincipalMomentsOfInertia.html) — modern spelling.
- **[M12]** [`IMassProperty2` members, 2025](https://help.solidworks.com/2025/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty2_members.html) — `UseSystemUnits`, scope and mass-property members.
- **[M13]** [`IModelDocExtension.GetMassProperties2`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imodeldocextension~getmassproperties2.html) — older flat mass array.
- **[V1]** [Microsoft VBA `VarType`](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/vartype-function) — Variant type flags. Used for diagnostics, not proof of VBScript's target marshaling.
- **[V2]** [Microsoft VBA `IsObject`](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/isobject-function) — object detection.
- **[V3]** [Microsoft VBA `Set`](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/set-statement) — object assignment semantics.

### Sketch relations

- **[R1]** [`ISketch.RelationManager`, 2023](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketch~RelationManager.html).
- **[R2]** [`ISketchRelationManager.GetRelations`, 2022](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelationManager~GetRelations.html).
- **[R3]** [`ISketchRelationManager.GetRelationsCount`, 2021](https://help.solidworks.com/2021/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelationManager~GetRelationsCount.html).
- **[R4]** [`ISketchRelationManager.IGetRelations`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelationManager~IGetRelations.html) — unsupported for VBA.
- **[R4a]** [`swSketchRelationFilterType_e`, 2026](https://help.solidworks.com/2026/english/api/swconst/SOLIDWORKS.Interop.swconst~SOLIDWORKS.Interop.swconst.swSketchRelationFilterType_e.html).
- **[R5]** [`ISketchRelation.GetEntities`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isketchrelation~getentities.html).
- **[R6]** [`ISketchRelation.GetDefinitionEntities2`, 2026](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelation~GetDefinitionEntities2.html).
- **[R7]** [`ISketchRelation` members, 2010](https://help.solidworks.com/2010/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelation_members.html) and [`ISketchRelation.Suppressed`, 2025](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelation~Suppressed.html).
- **[R8]** [`swConstraintType_e`, 2024](https://help.solidworks.com/2024/english/api/swconst/SOLIDWORKS.Interop.swconst~SOLIDWORKS.Interop.swconst.swConstraintType_e.html).
- **[R9]** [`ISketchRelation.GetEntitiesType`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isketchrelation~getentitiestype.html) and [`swSketchRelationEntityTypes_e`, 2024](https://help.solidworks.com/2024/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swSketchRelationEntityTypes_e.html).
- **[R10]** [`ISketchRelation.GetDisplayDimension`, 2026](https://help.solidworks.com/2026/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelation~GetDisplayDimension.html).
- **[R11]** [Official VBA example: Get All Sketch Relations and Their Types, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/get_all_sketch_relations_and_their_types_example_vb.htm).
- **[R12]** [`ISketchSegment.GetRelations`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isketchsegment~getrelations.html).
- **[R13]** [`ISketchRelationManager.DeleteRelation`, 2026](https://help.solidworks.com/2026/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelationManager~DeleteRelation.html).
- **[R14]** [`ISketchRelationManager.DeleteAllRelations`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelationManager~DeleteAllRelations.html).

### Dimensions and equations

- **[D0]** [Official VBA example: Get Dimension Values in All Configurations, 2023](https://help.solidworks.com/2023/english/api/sldworksapi/get_dimension_values_example_vb.htm).
- **[D1]** [`IDimension.DrivenState`, 2023](https://help.solidworks.com/2023/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.idimension~drivenstate.html).
- **[D2]** [`swDimensionDrivenState_e`, 2026](https://help.solidworks.com/2026/English/api/swconst/SOLIDWORKS.Interop.swconst~SOLIDWORKS.Interop.swconst.swDimensionDrivenState_e.html).
- **[D3]** [`swDimensionType_e`, 2026](https://help.solidworks.com/2026/English/api/swconst/SOLIDWORKS.Interop.swconst~SOLIDWORKS.Interop.swconst.swDimensionType_e.html) and [`IDisplayDimension.Type2`, 2019](https://help.solidworks.com/2019/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IDisplayDimension~Type2.html).
- **[D3a]** [`swDimensionParamType_e`, 2026](https://help.solidworks.com/2026/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swDimensionParamType_e.html).
- **[D4]** [`IDimension.GetSystemValue3`, 2016](https://help.solidworks.com/2016/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.idimension~getsystemvalue3.html).
- **[D4a]** [`IDimension.SystemValue`, 2023](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IDimension~SystemValue.html) — obsolete.
- **[D5]** [`IDimension.SetSystemValue3`, 2024](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.interop.sldworks~SolidWorks.interop.sldworks.IDimension~SetSystemValue3.html).
- **[D6]** [`swSetValueReturnStatus_e`, 2024](https://help.solidworks.com/2024/english/api/swconst/SOLIDWORKS.Interop.swconst~SOLIDWORKS.Interop.swconst.swSetValueReturnStatus_e.html).
- **[D7]** [`IDisplayDimension.GetDimension2`, 2019](https://help.solidworks.com/2019/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.idisplaydimension~getdimension2.html).
- **[D8]** [`IDimension.Name`, 2025](https://help.solidworks.com/2025/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IDimension~Name.html).
- **[D9]** [`IDimension.FullName`, 2017](https://help.solidworks.com/2017/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IDimension~FullName.html).
- **[D10]** [`IDimension.GetFeatureOwner`, 2024](https://help.solidworks.com/2024/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IDimension~GetFeatureOwner.html).
- **[D11]** [`IDimension.IsDesignTableDimension`, 2023](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IDimension~IsDesignTableDimension.html).
- **[D12]** [`IEquationMgr.Equation`, 2024](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IEquationMgr~Equation.html) and [`IEquationMgr` members, 2024](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IEquationMgr_members.html).
- **[D15]** [`swInConfigurationOpts_e`, 2025](https://help.solidworks.com/2025/English/api/swconst/SOLIDWORKS.Interop.swconst~SOLIDWORKS.Interop.swconst.swInConfigurationOpts_e.html).
- **[D15a]** [`swSetValueInConfiguration_e`, 2024](https://help.solidworks.com/2024/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swSetValueInConfiguration_e.html).
- **[D17]** [`IFeature.Parameter`, 2021](https://help.solidworks.com/2021/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeature~Parameter.html).
- **[D18]** [`IDimension` members, 2025](https://help.solidworks.com/2025/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IDimension_members.html) — inspected for a `SystemName` member, with no such verified member recovered.

### Supplied research files

- **[P1]** `05-vba-dialects-and-seat-automation-research(1).md` — VBA versus VBScript syntax, conversions, late binding, error handling.
- **[P2]** `SolidWorks_API_Research(1).md` — body mass properties and geometry readback contract.
- **[P3]** `SolidCraft_v043_Nested_Schema_Research(1).md` — 37 native constraint kinds, parameter and driven flags.
- **[P4]** `06-solidcraft-command-research-report(1).md` — command-to-serialized constraint mapping, multi-constraint operations.
- **[P5]** `08-sketch-inference-and-solve-timing.md` and `08-sketch-inference-and-solve-timing-research.md` — supplied 2026 seat inference observations and recommended creation controls.
- **[P6]** `09-conic-rho-conversion-research.md` — conic representation conversion and geometry-verification alternatives.

**Research conclusion:** The public API defines enough readback members to build strict mass, relation, and dimension checks. It does not explain the observed object-valued centroid elements or offer a documented relation-inference provenance flag. Both gaps require narrow seat probes. The acceptance layer must refuse unknown results rather than convert or classify them by assumption.
