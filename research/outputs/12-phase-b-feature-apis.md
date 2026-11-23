# Phase B feature APIs 12

**Filed:** 2026-11-19, produced by the owner's research agent, delivered
as a chat attachment and filed verbatim. Answer to
`research/inputs/12-phase-b-feature-apis.md`.
Provenance note: the report's labels are its own (Recorded/Community/
Conflicting/Reconstructed-untested/Source-supplied). Headline contracts
for the Phase B emitter/verifier: persistent references
(GetPersistReference3 byte array + GetObjectByPersistReference3's ByRef
status - typed-client territory per brief 11) with the recorded warning
that raw ID bytes are NOT timeless identity; no stable GetEdges()
ordinal contract exists (never address edges by index); SolidCraft
`edge_refs.min/max` are BODY bounds, not edge bounds; fillets via
CreateDefinition(swFmFillet)/ISimpleFilletFeatureData2 with mark-1 edge
selections and AccessSelections/ReleaseSelectionAccess readback;
InsertCombineFeature's operation enum (15901..15903) is a DIFFERENT
family from ICombineBodiesFeatureData's (0..2); no keep_tools parameter
exists (source keep_tools=true refuses until a copy/retention path is
probed); the body registry uses GetFaces+IFace2.GetBody (GetBody
obsolete) with GetBodies2 order explicitly unstable;
InsertMoveCopyBody2 takes 12 inputs and CANNOT mix translation and
rotation in one feature (editions even disagree which wins - C01, never
mix); InsertDeleteBody2(False) is the body-remove feature; sweep marks
(1 profile / 4 path / 2 guides / 128 direction) and FeatureRevolve2's
20-arg torus path (R>r>0, TorusParams 8 doubles). The report's probe
table maps onto the four Phase B probes and keeps brief 09's conic gate
independent.

---

# Research 12: Phase B SOLIDWORKS feature APIs

**Date:** 19 November 2026  
**Consumer:** `craft-bridge` emitter (`src/swb.rs`) and acceptance verifier  
**Target:** SOLIDWORKS 2026 desktop, VBA replay macros (`.swb`) and VBScript checker  
**Scope:** Brief 12, tasks T1-T4, with cross-checks against briefs 04, 07, 09, 10, 11 and SolidCraft `solidcraft/57` at source tag `v0.43.0`  
**Execution:** Public-source documentation research. No licensed SOLIDWORKS seat was used. No generated macro was run in SOLIDWORKS.

SOLIDWORKS is a Dassault Systemes trademark. The name identifies the API; this report does not imply affiliation.

## Evidence categories and contract

- **Recorded**: Explicit SOLIDWORKS API Help declaration, official product help, or official example. A declaration proves the call shape, not a specific seat result.
- **Community**: Pinned third-party code or durable forum usage. It proves the call was written, not that it succeeds on this seat.
- **Conflicting**: Two published contracts, editions, or independently observed results disagree. Both remain visible.
- **Reconstructed / untested**: An implementation proposal assembled from documented members. Do not ship it before a 2026 seat probe.
- **Source-supplied**: Facts taken from the earlier SolidCraft source research, especially the version-pinned schema and evaluator. They are not new runtime measurements.

Use metres and radians for SOLIDWORKS geometric inputs. SolidCraft's native document uses millimetres for stored point positions and named length expressions. Convert at the boundary. `Double` in the code means a VBA `Double`; `Long` represents a 32-bit COM `Integer`/`System.Int32`. The exact errors from a failed COM call must be captured and recorded, not assumed from the method's return type.

## Findings at a glance

| Area | Finding | Status | Implementation action |
|---|---|---|---|
| Entity IDs | `GetPersistReference3` returns a model-associated persistent-reference array. `GetObjectByPersistReference3` resolves it with a ByRef status. | **Recorded** | Use a VBA/.NET helper. Validate resolution and geometry after rebuild. |
| Persistent bytes | A persistent ID's internal byte sequence can change while resolution remains valid. | **Recorded** | Do not compare raw bytes as a timeless geometric identity. |
| Edge ordering | `IPartDoc.GetBodies2` can reorder bodies. No stable `IBody2.GetEdges` index contract was found. | **Recorded** for bodies; **unverified** for edges | Never address a source edge by a global ordinal alone. |
| SolidCraft edge references | `edge_refs[].min/max` are **body** bounds, not edge bounds. `edge_names` and pick coordinates are separate fields. | **Source-supplied** | Require a unique geometric match and checked body owner. |
| Fillet creation | Create `ISimpleFilletFeatureData2` with `CreateDefinition(swFmFillet)`. Select edge inputs with mark 1. | **Recorded** | Prefer the builder to the obsolete `FeatureFillet` call. |
| Fillet readback | `DefaultRadius`, `Edges`, and selection access are available. | **Recorded** | Read the selected edges while feature-definition selections are accessible. |
| Boolean combine | `InsertCombineFeature` takes `swBodyOperationType_e` and body references or marked selections. | **Recorded** | Keep target/tool roles explicit, then verify body count and mass. |
| Enum conflict | Combine method enum values are `15901..15903`; feature-data operation enum values are `0..2`. | **Recorded** | Do not pass feature-data enum values to the creation method. |
| Tool retention | No direct `keep_tools` argument exists in `InsertCombineFeature`. | **Recorded** for signature; **untested** for workaround | Gate `SolidCraft combine.keep_tools=true` until copy/retention path works. |
| Body registry | `IFeature.GetBody` is obsolete; `IFeature.GetFaces` + `IFace2.GetBody` is recommended. | **Recorded** | Register outputs using checked names, owning bodies, and feature relationships. |
| Move/copy | `InsertMoveCopyBody2` takes 12 inputs and cannot combine rotation and translation in one feature. | **Recorded** | Emit separate transforms; check their order and actual matrix. |
| Move conflict | The 2013 page says translation wins on mixed inputs; 2022 says rotation wins. | **Conflicting** | Never send mixed inputs. Probe 2026 before relying on either precedence. |
| Body removal | `InsertDeleteBody2(False)` creates a Body-Delete/Keep feature. | **Recorded** | Prefer to deleting the producing feature. Verify targeted absence. |
| Sweeps | `CreateDefinition(swFmSweep)` uses profile/path/guide selections. | **Recorded** | Map SolidCraft orientation, rails, thin settings only after a seat probe. |
| Torus | Closed circular profile revolved 360 degrees about a coplanar external axis is a candidate. | **Reconstructed / untested** | Test ring torus and toroidal-surface readback before enabling. |
| Verifier COM arrays | Each `Object` array family may marshal differently in VBScript. | **Recorded** from prior research and seat evidence | Fail closed if an element cannot be inspected; do not infer from a working count. |

**Recommended order:** Verify the body registry before Boolean selection. Verify a unique edge resolver before enabling fillets. Prove transform and body deletion after the registry is stable. Finish by binding expression-based primitives and reference geometry so that changes propagate through all later features.

---

## T1. Persistent entity identity and the edge resolver

### T1.1 Official persistent-reference contract

**Recorded:** SOLIDWORKS has model-scoped persistent reference IDs for selectable geometry. The `IModelDocExtension` methods are:

```vb
' Published COM/VB declarations, shown as VBA-compatible forms.
' GetPersistReference3(DispObj As Object) As Object
' GetObjectByPersistReference3( _
'     PersistId As Object, ByRef ErrorCode As Long) As Object
```

`GetPersistReference3` returns an array holding the reference ID. Microsoft C++ examples expect a byte SAFEARRAY. The API's displayed `As Object` declaration does **not** make the elements VARIANT values. The resolver sets `ErrorCode` from `swPersistReferencedObjectStates_e`, a bitmask:

| State | Value | Meaning |
|---|---:|---|
| `swPersistReferencedObject_Ok` | 0 | Referenced entity resolved |
| `swPersistReferencedObject_Invalid` | 1 | Invalid reference |
| `swPersistReferencedObject_Suppressed` | 2 | Associated topological object suppressed |
| `swPersistReferencedObject_Deleted` | 4 | Associated topological object deleted |

The method can return a base `IFeature` object for some specialized feature types. The caller must use `GetSpecificFeature2` when it needs the specialized interface. A persistent ID belongs to its model context, not a global namespace. Do not resolve it through a different open document without an explicit model mapping.

**Recorded:** SOLIDWORKS warns that the internal byte representation can change after rebuilds or between releases, while the ID's ability to resolve the intended entity is maintained. The API gives no general guarantee that a reference survives deletion, split/merge topology changes, or replacement by a different entity. Store the reference and its model context, then test the resolved entity and the intended geometry.

Sources: [GetPersistReference3](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.interop.sldworks~SolidWorks.interop.sldworks.IModelDocExtension~GetPersistReference3.html), [GetObjectByPersistReference3](https://help.solidworks.com/2026/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~GetObjectByPersistReference3.html), [persistent-reference guide](https://help.solidworks.com/2026/english/api/sldworksapiprogguide/Overview/Persistent_Reference_IDs.htm), [status enum](https://help.solidworks.com/2026/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swPersistReferencedObjectStates_e.html?id=2.10.1.596), [official byte-array example](https://help.solidworks.com/2014/English/api/sldworksapi/Get_Object_s_Persistent_Reference_ID_Example_CPlusPlus_COM.htm).

**Reconstructed / untested: capture and resolve in VBA.** `swModel` and `edgeEntity` must both belong to the same part. `storedRef` is a VBA `Variant` containing the returned byte-array data.

```vb
Option Explicit

Function CaptureEntityRef(ByVal swModel As SldWorks.ModelDoc2, _
                          ByVal edgeEntity As SldWorks.Entity) As Variant
    Dim raw As Variant
    raw = swModel.Extension.GetPersistReference3(edgeEntity)
    If Not IsArray(raw) Then Err.Raise vbObjectError + 1201, , _
        "PERSIST_CAPTURE_NOT_ARRAY"
    CaptureEntityRef = raw
End Function

Function ResolveEntityRef(ByVal swModel As SldWorks.ModelDoc2, _
                          ByVal storedRef As Variant) As Object
    Dim status As Long
    Dim obj As Object
    If Not IsArray(storedRef) Then Err.Raise vbObjectError + 1202, , _
        "PERSIST_INPUT_NOT_ARRAY"
    status = -1
    Set obj = swModel.Extension.GetObjectByPersistReference3(storedRef, status)
    If status <> swPersistReferencedObject_Ok Then _
        Err.Raise vbObjectError + 1203, , "PERSIST_RESOLVE_STATUS_" & CStr(status)
    If obj Is Nothing Then Err.Raise vbObjectError + 1204, , _
        "PERSIST_RESOLVE_EMPTY"
    Set ResolveEntityRef = obj
End Function
```

The VBA declarations are based on official .NET/C# signatures. **Do not copy the resolver into VBScript without another probe:** `ByRef ErrorCode As Integer` is exactly the kind of output parameter that caused a type mismatch on this seat in earlier research. If the checker needs this readback, execute it inside the VBA emitter or a typed .NET adapter and pass a validated primitive result to VBScript.

**Additional note:** The older `GetPersistReference`/`GetObjectByPersistReference2` families are not compatible with the newer `*3` reference IDs. The `IGet*` versions use in-process pointer-oriented call patterns and should not replace the Automation-friendly `Get*` methods in VBA. Source: [GetPersistReference3 remarks](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDocExtension~GetPersistReference3.html), [IGetPersistReference3 remarks](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDocExtension~IGetPersistReference3.html).

### T1.2 Entity enumeration and geometry readback

| Member | VBA-compatible shape or result | Stability and purpose | Evidence |
|---|---|---|---|
| `IPartDoc.GetBodies2(BodyType As Long, BVisibleOnly As Boolean)` | Returns an `Object` containing an array of `Body2` | The returned **order may vary**. It supports solid and sheet bodies. | **Recorded** [2026 method](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IPartDoc~GetBodies2.html) |
| `IBody2.GetEdges()` | Returns `Object` array of `Edge` | Enumerates current topology, not a proven stable edge index. | **Recorded** [IBody2 member index](https://help.solidworks.com/2021/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IBody2_members.html) |
| `IFace2.GetEdges()` | Returns `Object` array of `Edge` | Restricts candidates to a face, but order stability remains unverified. | **Recorded** [IFace2 interface](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFace2.html) |
| `IEdge.GetBody()` | Returns `Body2` | Verify the edge belongs to the intended body. | **Recorded** [IEdge members](https://help.solidworks.com/2021/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iedge_methods.html) |
| `IEdge.GetCurve()` | Returns underlying `Curve` | **Must precede** `GetCurveParams3`. | **Recorded** [GetCurveParams3](https://help.solidworks.com/2012/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iedge~getcurveparams3.html) |
| `IEdge.GetCurveParams3()` | Returns `CurveParamData` | Use for curve type/parameter domain; not an entity identity by itself. | **Recorded** [GetCurveParams3](https://help.solidworks.com/2012/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iedge~getcurveparams3.html) |
| `IEdge.GetStartVertex()`, `GetEndVertex()` | Returns `Vertex` or `Nothing` | Useful for open edges. A closed edge may have no distinct endpoint pair. | **Recorded** [IEdge members](https://help.solidworks.com/2021/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iedge_methods.html) |
| `IEdge.GetID()` | Returns imported-edge ID | Only for imported-body edge identity. It is **not** a general persistent ID. | **Recorded** [IEdge member index](https://help.solidworks.com/2021/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iedge_methods.html) |
| `IEntity.Select4(Append As Boolean, Data As SelectData)` | Returns Boolean | Selects an entity through its live COM reference and `ISelectData.Mark`. | **Recorded** [IEntity member index](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IEntity.html) |
| `IModelDocExtension.SelectByID2(...)` | Returns Boolean | Name and geometry-pick selection. Fragile if names/positions change. | **Recorded** [SelectByID2](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDocExtension~SelectByID2.html) |
| `IFeature.GetFaces()` + `IFace2.GetBody()` | `Object` array then `Body2` per face | Links current feature faces to owning bodies, without guaranteeing unique output-body ownership. | **Recorded** [IFeature member index](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeature_members.html) |

`IEdge.GetCurveParams2` is obsolete but documents an 11-double layout that includes endpoints and encoded curve type. It can inform a diagnostic probe. Prefer `GetCurveParams3` in production. An edge's orientation can reverse when traversed in another face; use coedge sense when orientation matters. Sources: [GetCurveParams2](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEdge~GetCurveParams2.html), [GetCurveParams3](https://help.solidworks.com/2012/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iedge~getcurveparams3.html).

**No documented stable index was found** for `GetEdges()` after arbitrary rebuilds, fillets, splits or face merges. This is an evidence gap, not proof that every call changes order. A sorted geometric signature is a matching aid, not a CAD-native unique ID.

### T1.3 SolidCraft's actual serialized edge reference

**Source-supplied:** SolidCraft `solidcraft/57` stores fillet picks primarily as 3-D positions (`edges`). The feature wrapper can also store `edge_names` and `edge_refs` with exactly these fields:

```json
{"dir": [0.0, 0.0, 1.0], "min": [0.0, 0.0, 0.0], "max": [20.0, 15.0, 10.0]}
```

`dir` is a unit vector for a straight edge and `[0,0,0]` for a curved edge. `min`/`max` are the **picked body's full bounding box**, not the edge's bounding box. This means different edges on the same body can have identical `edge_refs`. The `edge_names` field can help with source-side resolution, but a name is not automatically a native SOLIDWORKS persistent reference.

Sources: [SolidCraft feature wrapper, v0.43.0](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L3290-L3315), [edge-reference production and resolution](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L5475-L5570). The earlier user-supplied *SolidCraft 57 Format Research* and *Nested Schema Research* provide the corresponding field definitions.

**Reconstructed / untested matching rule:** At the *pre-feature* state, restrict candidate edges to the source body. For each candidate, inspect curve class, endpoints or representative points, length, tangent direction and adjacent faces. Test the pick point's distance to that candidate. Accept a match only if **exactly one** candidate satisfies all chosen tolerances. Record its SOLIDWORKS persistent reference before modifying the topology. On ambiguity or disappearance, stop with `EDGE_AMBIGUOUS` or `EDGE_NOT_FOUND`; never choose the lowest edge index.

**Important:** A persistent reference to an edge selected in a part being built is a **SOLIDWORKS** identity. SolidCraft's saved `edge_names` are **source-side** identities. An emitter must create and maintain a bridge map between them. No official cross-CAD identity conversion is documented.

### T1.4 Why VBScript needs its own array probe

Brief 11 established that VBScript can inspect the bounds of a typed SAFEARRAY even when indexed elements raise `Type mismatch`. The persistent-reference return is a byte array. `GetBodies2`, `GetEdges`, `GetFaces`, `ISimpleFilletFeatureData2.Edges`, and `ISweepFeatureData.GuideCurves` are separate return families. A passing `IsArray` or `UBound` check does not establish that `raw(i)` works. Check each family on the 2026 seat with `VarType(raw)`, `LBound`, `UBound`, `For Each` and guarded element access. Do not promote a count to evidence of valid elements.

The strongest safe division is **VBA/.NET for typed geometric inspection**, with the checker consuming explicit primitive values that passed finite-number and identity checks. If the checker must call SOLIDWORKS directly, each return family needs its own marshaling probe. Sources: [SOLIDWORKS persistent-ID C++ example](https://help.solidworks.com/2014/English/api/sldworksapi/Get_Object_s_Persistent_Reference_ID_Example_CPlusPlus_COM.htm) and the attached Research 11 report.

---

## T2. Constant-radius fillets (D06)

### T2.1 Current builder versus older methods

**Recorded:** The older `IFeatureManager.FeatureFillet` call is marked obsolete. Its seven inputs are:

```vb
' IFeatureManager.FeatureFillet( _
'     Options As Long, R1 As Double, Ftyp As Long, _
'     OverflowType As Long, Radii As Object, _
'     SetBackDistances As Object, PointRadiusArray As Object) As Object
```

The historical `IModelDoc2.FeatureFillet*` family and the `IPartDoc`-named call in the research prompt should not be treated as the preferred SOLIDWORKS 2026 creation interface. Use the documented feature-data builder when a simple constant-radius fillet is needed:

1. `IFeatureManager.CreateDefinition(swFmFillet)` returns a simple fillet feature-data object.
2. `ISimpleFilletFeatureData2.Initialize(swConstRadiusFillet)` sets the constant-radius type.
3. Set `ConicTypeForCrossSectionProfile = swFeatureFilletCircular`.
4. Select the intended edges with mark `1`, set `DefaultRadius` in **metres**, then create the feature with `IFeatureManager.CreateFeature`.

Sources: [legacy FeatureFillet](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager~FeatureFillet.html), [ISimpleFilletFeatureData2 interface and official example](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2.html?id=2.2.1.334).

**Reconstructed / untested VBA builder.** The emitter supplies a *resolved* edge object. The helper rejects missing selections and a missing feature. It does not substitute for geometry validation.

```vb
Option Explicit

Function CreateConstantEdgeFillet( _
    ByVal swModel As SldWorks.ModelDoc2, _
    ByVal resolvedEdge As SldWorks.Edge, _
    ByVal radiusMetres As Double) As SldWorks.Feature

    Dim mgr As SldWorks.FeatureManager
    Dim sm As SldWorks.SelectionMgr
    Dim selData As SldWorks.SelectData
    Dim ent As SldWorks.Entity
    Dim fData As SldWorks.SimpleFilletFeatureData2
    Dim feat As SldWorks.Feature
    Dim selected As Boolean

    If radiusMetres <= 0# Then Err.Raise vbObjectError + 1210, , _
        "FILLET_INVALID_RADIUS"
    Set mgr = swModel.FeatureManager
    Set sm = swModel.SelectionManager
    Set selData = sm.CreateSelectData
    selData.Mark = 1
    Set ent = resolvedEdge
    swModel.ClearSelection2 True
    selected = ent.Select4(False, selData)
    If Not selected Then Err.Raise vbObjectError + 1211, , _
        "FILLET_EDGE_SELECTION_FAILED"

    Set fData = mgr.CreateDefinition(swFmFillet)
    If fData Is Nothing Then Err.Raise vbObjectError + 1212, , _
        "FILLET_DEFINITION_MISSING"
    fData.Initialize swConstRadiusFillet
    fData.ConicTypeForCrossSectionProfile = swFeatureFilletCircular
    fData.DefaultRadius = radiusMetres
    Set feat = mgr.CreateFeature(fData)
    If feat Is Nothing Then Err.Raise vbObjectError + 1213, , _
        "FILLET_CREATE_FAILED"
    Set CreateConstantEdgeFillet = feat
End Function
```

**Limits:** Multi-edge inputs need a separate selection loop using `Append=True` after the first edge. The seat must verify `Set ent = resolvedEdge`, the selection manager's `CreateSelectData`, and the builder's initialization/selection ordering. Some methods accept face or feature picks as well as edge picks; the replay's `edge_refs` does not authorize silently widening a target to an entire face.

### T2.2 Reverse readback

**Recorded:** `ISimpleFilletFeatureData2` exposes `DefaultRadius`, `Edges`, and `GetEdgeCount`. `DefaultConicRhoOrRadius` applies to a **fillet cross-section**, not a sketch conic. Its documented `rho` range must not be substituted for SolidCraft's conic `rho` from brief 09.

Read the fillet feature through `IFeature.GetDefinition()`. To retrieve selected edges, call `AccessSelections(swModel, Nothing)`, inspect the definition, then always call `ReleaseSelectionAccess()`. The selection access can temporarily roll the feature tree back. Release it before rebuilding or enumerating later features. Reading the *post-fillet model edge list* is not equivalent to reading the original fillet input edge list: the operation can replace or consume the selected edge.

**Reconstructed / untested readback sequence:**

```vb
Option Explicit

Sub CheckFilletDefinition(ByVal swModel As SldWorks.ModelDoc2, _
                          ByVal feat As SldWorks.Feature, _
                          ByVal expectedRadius As Double)
    Dim data As SldWorks.SimpleFilletFeatureData2
    Dim accessed As Boolean
    Dim edgeValues As Variant

    Set data = feat.GetDefinition
    If data Is Nothing Then Err.Raise vbObjectError + 1214, , _
        "FILLET_DEFINITION_READ_FAILED"
    accessed = data.AccessSelections(swModel, Nothing)
    If Not accessed Then Err.Raise vbObjectError + 1215, , _
        "FILLET_SELECTION_ACCESS_FAILED"

    On Error GoTo RestoreAndFail
    If Abs(data.DefaultRadius - expectedRadius) > 0.000001 Then _
        Err.Raise vbObjectError + 1216, , "FILLET_RADIUS_MISMATCH"
    edgeValues = data.Edges
    If Not IsArray(edgeValues) Then _
        Err.Raise vbObjectError + 1217, , "FILLET_EDGE_ARRAY_INVALID"
    ' Next: verify every element and its persistent/geometric identity.
    ' Do not infer a successful match from UBound(edgeValues) alone.

    data.ReleaseSelectionAccess
    Exit Sub
RestoreAndFail:
    On Error Resume Next
    data.ReleaseSelectionAccess
    On Error GoTo 0
    Err.Raise vbObjectError + 1218, , "FILLET_READBACK_FAILED"
End Sub
```

The example deliberately does not claim that `data.Edges` elements are directly accessible from VBScript or that the radius alone verifies geometry. Verify edge count, each input edge's original body, radius, resulting feature status (`GetErrorCode2`) and a geometric postcondition. The `0.000001 m` in the snippet is a **test parameter**, not a published universal tolerance. Derive acceptance tolerances from each fixture's dimensions and the replay specification.

Sources: [ISimpleFilletFeatureData2 members](https://help.solidworks.com/2026/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2_members.html), [ISimpleFilletFeatureData2 interface](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2.html?id=2.2.1.334), [feature definition](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeature_members.html).

### T2.3 Failure behavior and geometry

**Recorded:** `CreateFeature` returns an `IFeature` when creation succeeds, and official documentation does not promise a specific COM exception or error code for each invalid edge set. **Reconstructed / untested:** Treat `Nothing`, a failed rebuild, a nonzero feature error or unexpected body topology as separate named failures. A successful return **alone** does not mean the intended edges were filleted.

For D06, a controlled test can use a rectangular block with one uniquely selected outside edge. Verify the volume change has the expected direction, the edge is replaced by an appropriate blended surface, and unrelated edges remain unchanged. A second test must include two equal-length parallel edges with similar body bounds to prove the resolver does not select by ordinal or global nearest point alone.

---
## T3. Boolean operations, body registry, move and removal

### T3.1 Combine creation: exact contract and two different enums

**Recorded:** The current feature-manager entry is:

```vb
' Function IFeatureManager.InsertCombineFeature( _
'     OperationType As Long, _
'     MainBody As SldWorks.Body2, _
'     ToolVar As Object) As SldWorks.Feature
```

For this **method**, the operation codes are `swBodyOperationType_e`:

| Operation | Symbol | Numeric value | MainBody input |
|---|---|---:|---|
| Add / union | `SWBODYADD` | 15903 | `Nothing` |
| Subtract / cut | `SWBODYCUT` | 15902 | Target `Body2` |
| Common / intersect | `SWBODYINTERSECT` | 15901 | `Nothing` |

`ToolVar` is an array of body objects. Instead of passing objects, an official alternative selects the main body with mark **1**, selects tool bodies with mark **2**, then calls `InsertCombineFeature(operation, Nothing, emptyArray)`. `IInsertCombineFeature` uses an in-process pointer/array convention and must not replace the Automation-friendly method in VBA.

**Critical enum distinction:** `ICombineBodiesFeatureData.OperationType`, used to **read or edit** a combine definition, takes `swCombineBodiesOperationType_e`. Its documented values are `Add=0`, `Subtract=1`, and `Common=2`. These are **not** accepted values for the `InsertCombineFeature` creation call unless an undocumented conversion exists. Do not infer such a conversion.

Sources: [InsertCombineFeature method](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html), [swBodyOperationType_e](https://help.solidworks.com/2024/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swBodyOperationType_e.html), [Combine feature-data members](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ICombineBodiesFeatureData_members.html), [swCombineBodiesOperationType_e](https://help.solidworks.com/2024/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swCombineBodiesOperationType_e.html).

**Reconstructed / untested VBA example, cut one tool from one target:**

```vb
Option Explicit

Function CombineSubtract(ByVal swModel As SldWorks.ModelDoc2, _
                         ByVal target As SldWorks.Body2, _
                         ByVal tool As SldWorks.Body2) As SldWorks.Feature
    Dim f As SldWorks.Feature
    Dim tools As Variant
    tools = Array(tool)
    Set f = swModel.FeatureManager.InsertCombineFeature( _
        SWBODYCUT, target, tools)
    If f Is Nothing Then Err.Raise vbObjectError + 1220, , _
        "COMBINE_FEATURE_FAILED"
    Set CombineSubtract = f
End Function
```

`tools = Array(tool)` is **not** yet a verified 2026 VBA marshaling path for the `Body2` array. An alternate seat probe uses marked `Body2` selections and passes an empty array. For union and common, supply `MainBody = Nothing` according to the method contract and record the resulting output body identity. Do not assume the first returned body is the Boolean output.

**Readback:** `ICombineBodiesFeatureData` exposes `BodiesToCombine`, `MainBody`, `OperationType`, `GetBodiesToCombineCount`, `AccessSelections`, and `ReleaseSelectionAccess`. Get the definition with `IFeature.GetDefinition`. Use the selection-access protocol to inspect operands, then release it. Compare the operation enum using its *own* enum family. Check the result's rebuilt body count, body ownership, volume and pre/post bounding boxes. See [Combine feature-data members](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ICombineBodiesFeatureData_members.html).

### T3.2 SolidCraft semantics versus a SOLIDWORKS combine

**Source-supplied:** SolidCraft `FeatureKind::Combine` has `target`, `tools`, `operation`, and `keep_tools`. The stored operation values include `join`, `cut`, and `intersect` (with `new_body` also in the wider generic operation enum). The source evaluation removes tool bodies when `keep_tools=false` and preserves them when `keep_tools=true`. Names in `targets` (when present in other features), `participants`, and `body_names` have separate purposes; do not collapse them into one selection list.

Source: [SolidCraft Combine fields](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L984-L995), [operation enum](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L206-L243), [SolidCraft feature guidance](https://docs.solidcraft.io/guide/features.html).

**Gap:** `InsertCombineFeature` does not accept a `keep_tools` parameter. The feature-data members cited above do not expose an equivalent property. Consequently:

- For `keep_tools=false`, a direct combine may be a suitable mapping. Confirm actual tool consumption and body count on the seat.
- For `keep_tools=true`, **do not silently set it to false**. Candidate workarounds include creating separate copies before the Boolean, using a non-consuming transient body operation followed by feature creation, or preserving a separate independent body. Each path needs a feature-history and geometry probe.
- The source `target`/`tools` roles are required even where SOLIDWORKS union/intersect pass `MainBody=Nothing`. Identify the intended output after creation and verify its name and geometry.

A cut must reduce the intended target's volume without changing unrelated bodies. A common operation must produce the expected overlap volume. A union must preserve total intended material volume modulo intersection volume. Compare the actual output against the baseline reference, not only these qualitative signs.

### T3.3 Body registry: stable mapping through history

**Recorded:** `IPartDoc.GetBodies2(swSolidBody, False)` returns a body array. SOLIDWORKS explicitly warns that the **order may change**. Therefore a registry such as `sourceBodyName -> GetBodies2()(index)` is invalid after rebuilds. `IPartDoc`, not `IModelDoc2`, owns this method.

`IFeature.GetBody` and `IGetBody2` are obsolete. The official replacements are `IFeature.GetFaces`/`IGetFaces2` together with `IFace2.GetBody`. `GetFaces` supplies an array of faces, while `IFace2.GetBody` identifies the body containing each face. A single feature can affect more than one body. A feature can also contain no suitable faces for a given use case. This chain is **not** a universal `GetOutputBody` method.

Sources: [2026 GetBodies2](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IPartDoc~GetBodies2.html), [obsolete IFeature.GetBody](https://help.solidworks.com/2026/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeature~GetBody.html), [IFeature.GetFaces and error methods](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeature_members.html).

**Reconstructed registry design:** After every geometry-producing feature, rebuild and snapshot all solid bodies. Prefer a unique recorded `IBody2.Name` coupled with a current-model body reference. Capture stable persistent IDs for identifiable faces/edges, creating-feature identity, volume, centroid and approximate bounds as cross-checks. Keep separate records of consumed and newly generated body identities. Re-resolve entries before use. Do not reuse a prior live object pointer after an operation that may replace the underlying topology without checking it.

Suggested registry record:

```text
source_body_name
source_component_or_occurrence
source_creating_feature_id
current_solidworks_part_id
current_body_name
current_persistent_reference_if_available
current_feature_name_and_output_relation
geometry_fingerprint(volume, centroid, solid_count, optional faces)
last_validated_history_step
state = unique | missing | ambiguous | consumed
```

**Reconstructed / untested VBA enumeration:**

```vb
Option Explicit

Sub CheckSolidBodyInventory(ByVal swPart As SldWorks.PartDoc)
    Dim allBodies As Variant
    Dim i As Long
    Dim body As SldWorks.Body2
    allBodies = swPart.GetBodies2(swSolidBody, False)
    If Not IsArray(allBodies) Then Err.Raise vbObjectError + 1221, , _
        "BODY_LIST_NOT_ARRAY"
    For i = LBound(allBodies) To UBound(allBodies)
        Set body = allBodies(i)
        If body Is Nothing Then Err.Raise vbObjectError + 1222, , _
            "BODY_LIST_NULL_MEMBER"
        Debug.Print body.Name
        ' Record identity and numeric geometry. Do not depend on i.
    Next i
End Sub
```

**Caveat:** The snippet is VBA, not VBScript. `GetBodies2` may legitimately return an empty result or a null state in an empty part. The verifier should classify expected empty inventory separately from an invalid returned array. For positive-body fixtures, refusal is correct.

### T3.4 Move/copy: one feature per transform family

**Recorded:** The current `IFeatureManager` method has **12 positional arguments**:

```vb
' Function IFeatureManager.InsertMoveCopyBody2( _
'     TransX As Double, TransY As Double, TransZ As Double, _
'     TransDist As Double, _
'     RotPointX As Double, RotPointY As Double, RotPointZ As Double, _
'     RotAngleX As Double, RotAngleY As Double, RotAngleZ As Double, _
'     BCopy As Boolean, NumCopies As Long) As SldWorks.Feature
```

The target body must be selected with mark **1**. Additional references use mark **2** (rotation), mark **4** (translation), and mark **8** (translation vertex). `BCopy=True` requests copies, and `NumCopies` sets the count. `TransDist` has a separate documented role from the XYZ translation components. The translation/rotation reference semantics and `NumCopies` behavior for `BCopy=False` require a seat probe.

**Conflicting official editions:** The [2013 method page](https://help.solidworks.com/2013/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html) says a mixed call performs only the translation. The [2022 method page](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html) says a mixed call performs only the rotation. Both agree that **one feature cannot apply both operations**. The emitter must therefore make two features and re-resolve the moved body between them. Mixed-input behavior is never an acceptable dependency.

**Source-supplied:** SolidCraft `move` stores `bodies`, `translate:[Expr;3]`, optional `rotate_axis`, `origin`, `angle`, `copy`, and `copy_frames`. Its transform is supplied to the geometry kernel; the exact matrix order for combined pivot rotation and translation was not independently established in the earlier reports. Unlike `InsertMoveCopyBody2`, the source move is a logical transform with both rotation and translation fields. Source: [SolidCraft Move serialization](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L2181-L2205), [Move evaluator](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L10781-L10834).

**Reconstructed / untested call sequence:** The following uses only translation and assumes the selected body is valid. The `NumCopies=0` choice is **a probe candidate**, not yet an established seat value.

```vb
' Assumes body was selected with mark 1 and only translation is wanted.
Dim fMove As SldWorks.Feature
Set fMove = swModel.FeatureManager.InsertMoveCopyBody2( _
    0.01, 0#, 0#, 0#, _
    0#, 0#, 0#, 0#, 0#, 0#, False, 0)
If fMove Is Nothing Then Err.Raise vbObjectError + 1223, , _
    "BODY_TRANSLATION_FAILED"
```

For arbitrary SolidCraft axis-angle rotations, no reviewed `InsertMoveCopyBody2` contract specifies a direct axis vector input. The method takes X/Y/Z angles and optional selected rotation references. A numerical conversion to Euler angles requires exact axis-order and local/world-frame rules. **Do not guess** that `RotAngleX/Y/Z` directly represent the arbitrary SolidCraft `rotate_axis`. A frame/axis alignment feature, a rotation-reference selection, or a different feature-data setter might be necessary. Probe a non-origin pivot and an asymmetric body before introducing general rotation.

**Dependency test:** Capture the source body's vertices or reference geometry before move. After creating the first feature, obtain the new live body identity, select it, create the second feature, and verify the combined transform on at least three non-collinear points. Compare final centroid and bounding box against the source STEP output. A symmetric box or origin-centred body alone can hide a wrong rotation.

### T3.5 Body removal: delete a body, not its history

**Recorded:** SOLIDWORKS provides a Body-Delete/Keep **feature**:

```vb
' Function IFeatureManager.InsertDeleteBody2( _
'     KeepBodies As Boolean) As SldWorks.Feature
```

If `KeepBodies=False`, the selected bodies are deleted from downstream geometry. If `True`, selected bodies are kept. The method creates a timeline feature and has an `IDeleteBodyFeatureData` definition. This is closer to SolidCraft's serialized `remove { bodies: [...] }` than selecting and deleting the **feature that created a body**.

Source: [InsertDeleteBody2 (2015+, official)](https://help.solidworks.com/2015/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertDeleteBody2.html), [official VBA example](https://help.solidworks.com/2024/English/api/sldworksapi/Insert_Delete_Body_Feature_Example_VB.htm).

**Reconstructed / untested VBA:**

```vb
Option Explicit

Function RemoveBodyAtHistoryStep(ByVal swModel As SldWorks.ModelDoc2, _
                                 ByVal chosenBody As SldWorks.Body2) _
                                 As SldWorks.Feature
    Dim ent As SldWorks.Entity
    Dim sd As SldWorks.SelectData
    Dim ok As Boolean
    Dim f As SldWorks.Feature
    Set sd = swModel.SelectionManager.CreateSelectData
    sd.Mark = 0
    Set ent = chosenBody
    swModel.ClearSelection2 True
    ok = ent.Select4(False, sd)
    If Not ok Then Err.Raise vbObjectError + 1224, , _
        "REMOVE_BODY_SELECT_FAILED"
    Set f = swModel.FeatureManager.InsertDeleteBody2(False)
    If f Is Nothing Then Err.Raise vbObjectError + 1225, , _
        "REMOVE_BODY_FEATURE_FAILED"
    Set RemoveBodyAtHistoryStep = f
End Function
```

**Seat caution:** `IBody2` selection may use `IBody2.Select2` rather than an `IEntity` cast. The official delete-body example selects the body by `SelectByID2` with type `SOLIDBODY`. The above is a candidate to **compile and probe**, not a confirmed body-interface conversion. If the `Entity` cast fails, use the documented body-selection interface and keep mark semantics unchanged.

**Recorded distinction:** `IModelDocExtension.DeleteSelection2(DeleteOptions As Long) As Boolean` deletes selected model entities or features and can affect dependent features. It does not directly implement source `remove` semantics. Source: [DeleteSelection2](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~DeleteSelection2.html).

**Acceptance:** Before removal, verify that the intended body is uniquely selected. After the Body-Delete/Keep feature and rebuild, verify that the target body is absent. Verify that all unrelated bodies remain and the original creation feature stays in history. If a downstream feature requires the removed body, detect and report that broken dependency instead of silently removing it.

---
## T4. Sweep and torus, Phase C input

### T4.1 Sweep feature and selection marks

**Recorded:** SOLIDWORKS documents a feature-data builder for sweeps. For a **swept boss**:

```vb
' Set sweepData = swModel.FeatureManager.CreateDefinition(swFmSweep)
' Set sweepFeature = swModel.FeatureManager.CreateFeature(sweepData)
```

Use `swFmSweepCut` for a cut and the documented surface-feature type for a surface sweep, rather than assuming that a swept boss can serve every SolidCraft operation. The sweep feature data exposes `Profile`, `Path`, `GuideCurves`, `PathAlignmentType`, `TwistControlType`, `Merge`, `FeatureScope`, `FeatureScopeBodies`, `AutoSelect`, thin-wall settings, and getters for twist and wall thickness. The exact availability of some setter methods depends on the selected sweep type.

**Recorded selection marks from the programming guide:**

| Selection | Mark | Conditions |
|---|---:|---|
| Sketch profile for a swept boss | 1 | The boss profile must be closed. |
| Path for a sketch-profile sweep | 4 | A curve, model-edge sequence, or sketched-curve sequence. |
| Each guide curve | 2 | Must meet the sketch profile or a profile point. |
| Direction reference | 128 | Plane, line, axis, edge or supported direction reference. |
| Circular-profile sweep path | Special case | The guide uses mark 4 to select the sketch line/edge defining the circular-profile sweep. Do not copy sketch-profile rules blindly. |

For a one-directional sweep, the start of the path must lie in the plane of the profile. A bidirectional sweep follows additional constraints. Follow the selection/feature-data contract for the actual sweep type. Sources: [Sweep features and feature-data guide](https://help.solidworks.com/2021/English/api/sldworksapiprogguide/Overview/Sweep_Features_and_SweepFeatureData_Objects.htm), [2026 ISweepFeatureData members](https://help.solidworks.com/2026/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISweepFeatureData_members.html?id=8afcd5e9d83b42e2a51d8dcd4c74b9d9).

**Reconstructed / untested skeleton:**

```vb
Option Explicit

Function CreateSelectedBossSweep(ByVal swModel As SldWorks.ModelDoc2) _
                                 As SldWorks.Feature
    Dim d As SldWorks.SweepFeatureData
    Dim f As SldWorks.Feature
    ' Preconditions: sketch profile selected with mark 1;
    ' path selected with mark 4; optional guides selected with mark 2.
    Set d = swModel.FeatureManager.CreateDefinition(swFmSweep)
    If d Is Nothing Then Err.Raise vbObjectError + 1230, , _
        "SWEEP_DEFINITION_FAILED"
    ' Set documented orientation, twist and merge/scope properties here.
    ' Do not guess a SolidCraft orientation-to-enum mapping.
    Set f = swModel.FeatureManager.CreateFeature(d)
    If f Is Nothing Then Err.Raise vbObjectError + 1231, , _
        "SWEEP_FEATURE_FAILED"
    Set CreateSelectedBossSweep = f
End Function
```

**Source-supplied:** SolidCraft native `sweep` stores `sketch`, `profiles`, `path_sketch`, `path`, and optional `path_body`, `path_edges`, `orientation`, `twist`, `scale`, `rails`, `rail_fit`, `operation`, and `targets`. `orientation`, `twist`, and `rails` cannot be assumed to map directly to SOLIDWORKS enum values or selection patterns. In particular, scaling a profile continuously along the sweep may require a different feature family or an explicit approximation policy. Source: [SolidCraft feature-kind inventory](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L802-L2250), [official features](https://docs.solidcraft.io/guide/features.html).

**Acceptance:** Probe a closed circular/rectangular profile on a straight path first, then a curved path and a nontrivial orientation. Verify the path, profile, output body count, reference surface and cross-section orientation at several path stations. Add rails and twist only after proving the base path. Never report a generated swept solid as exact merely because `CreateFeature` returned a feature.

### T4.2 Toroidal solid by revolve

**Recorded:** `IFeatureManager.FeatureRevolve2` creates base/boss/cut revolves. Its **20-argument** contract is:

```vb
' Function IFeatureManager.FeatureRevolve2( _
'     SingleDir As Boolean, IsSolid As Boolean, _
'     IsThin As Boolean, IsCut As Boolean, _
'     ReverseDir As Boolean, BothDirectionUpToSameEntity As Boolean, _
'     Dir1Type As Long, Dir2Type As Long, _
'     Dir1Angle As Double, Dir2Angle As Double, _
'     OffsetReverse1 As Boolean, OffsetReverse2 As Boolean, _
'     OffsetDistance1 As Double, OffsetDistance2 As Double, _
'     ThinType As Long, ThinThickness1 As Double, ThinThickness2 As Double, _
'     Merge As Boolean, UseFeatScope As Boolean, _
'     UseAutoSelect As Boolean) As SldWorks.Feature
```

Select the profile sketch with mark **0** and the axis with mark **4 or 16**. Angles use **radians**. `swEndCondBlind=0` with `Dir1Angle=2*pi` is the candidate for a full revolution. See [FeatureRevolve2, official 2021 declaration](https://help.solidworks.com/2021/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureRevolve2.html) and [2024 method](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurerevolve2.html).

**Reconstructed / untested ring-torus construction:** Create a closed circle of radius `r` on a sketch plane that also contains the revolution axis. Place the circle centre at a perpendicular distance `R` from that axis. Ensure **`R > r > 0`** for a ring torus without a self-intersecting profile. Revolve through `2*pi`. An off-axis circle creates an analytic toroidal surface. The sketch axis must not pass through the circle's interior.

```vb
Option Explicit

Function RevolveSelectedRingProfile(ByVal swModel As SldWorks.ModelDoc2) _
                                    As SldWorks.Feature
    Dim pi As Double
    Dim f As SldWorks.Feature
    pi = 4# * Atn(1#)
    ' Precondition: closed circle sketch selected with mark 0.
    ' Coplanar revolution axis selected with mark 4 or 16.
    Set f = swModel.FeatureManager.FeatureRevolve2( _
        True, True, False, False, False, False, _
        swEndCondBlind, swEndCondBlind, 2# * pi, 0#, _
        False, False, 0#, 0#, 0, 0#, 0#, _
        False, False, True)
    If f Is Nothing Then Err.Raise vbObjectError + 1232, , _
        "TORUS_REVOLVE_FAILED"
    Set RevolveSelectedRingProfile = f
End Function
```

The code is intended for an isolated new solid. The seat must establish the required sketch/axis selection order and validate the resulting geometry. To preserve `SolidCraft torus` as a **distinct body**, use a non-merging operation and inspect the body count. Thin-wall, cut and scoped variants are separate tests.

**Recorded geometric readback:** An analytic toroidal `ISurface` exposes `TorusParams` as an `Object` holding **eight doubles**:

```text
[center.x, center.y, center.z, axis.x, axis.y, axis.z, R, r]
```

The two radii and centre are in **metres**. The radius that reaches the outside of a standard ring torus is `R + r`. The published documentation also describes self-intersecting apple/lemon torus parameter ranges, so do not generalize a clean ring-torus test to every `R/r` pair. A positive ring torus has ideal volume `V = 2*pi^2*R*r^2` and surface area `S = 4*pi^2*R*r`; use these **as additional independent checks**, not substitutes for topology and profile history.

Source: [ISurface.TorusParams](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISurface~TorusParams.html). The `TorusParams` return is **another typed-array candidate**, so brief 11's VBScript restriction still applies. Prefer a VBA/.NET readback helper and pass checked primitive values to the verifier. The similarly named `ITorusParams` page uses a pointer-style return declaration that is not an Automation-safe replacement.

---

## Cross-report observations that affect Phase B

### A. Two separate identity systems

SolidCraft's saved `edge_names`, `edge_refs`, and geometric pick points belong to the **source design**. SOLIDWORKS persistent references belong to the **reconstructed part**. Persisting a SOLIDWORKS ID after the first match improves later verification, but it does not derive the original source match. The edge resolver must first establish the cross-CAD mapping from geometry and source identity, then preserve and test the new SOLIDWORKS identity.

### B. Feature scope is not a body-retention flag

Source `combine.keep_tools`, source `targets`/`participants`, SOLIDWORKS combine tool consumption, and SOLIDWORKS boss/cut feature scope are **four separate behaviors**. Do not implement them with one Boolean in the emitter. A correct Boolean result can still be wrong if another body was unintentionally consumed or altered.

### C. Expression prim placement must be tested before more Booleans

**Source-supplied:** SolidCraft's `box`, `cylinder`, and similar primitives can contain expression-based placement values. For `box`, dimensions are `length`, `width`, `height`, while position `corner` is a placement expression and does not necessarily receive a generated `dN` dimension name. For `cylinder`, position `base` is separate from `radius` and `height`. Changing a driving parameter should move the body without changing its dimensions, unless the parameter also drives a dimension.

Sources: [SolidCraft v0.43.0 primitive and expression schema](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs), [expression engine](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/expr.rs). Also see the user-supplied brief 06 command-to-native research and brief 04 SOLIDWORKS equation research.

**Reconstructed / untested:** The emitter must give the primitive placement an actual driving dependency in SOLIDWORKS, such as an equation-driven plane/sketch or referenced dimensions. Merely evaluating a SolidCraft expression once and writing its numeric coordinates makes a static snapshot. A static feature can pass the first STEP comparison and fail after the source parameter changes. Prove the dependency by changing **one placement parameter** and **one dimension parameter** separately, rebuilding, and checking both the new and unaffected coordinates.

### D. Verification cannot depend on a successful return alone

A feature object from `CreateFeature`, `InsertCombineFeature`, `InsertMoveCopyBody2` or `InsertDeleteBody2` is one observation. For a positive verdict, also require a successful rebuild, expected body identity and count, numeric geometry invariants, and the intended feature inputs. Brief 10's dimension/relation verification and brief 11's typed-array safeguards remain in force.

### E. The exact-conic gate remains independent

Even if the body registry, fillets, Booleans and sweeps pass, the **R1 design's 854 conics** still need the exact-representation gate from brief 09. A substituted ordinary spline or sampled polyline is not an exact conic replay unless the accepted representation and geometry equivalence are independently proved and the feature-type difference is explicitly allowed.

---

## Conflicts and unresolved questions

| ID | Conflict or gap | Evidence | Fail-closed rule |
|---|---|---|---|
| C01 | `InsertMoveCopyBody2` mixed translation/rotation precedence differs by documentation edition. | **Conflicting:** [2013](https://help.solidworks.com/2013/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html) versus [2022](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html). | Never mix. Emit separate feature steps and verify the full transform. |
| C02 | Combine's method enum and feature-data enum use different values. | **Recorded:** [InsertCombineFeature](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html), [feature-data interface](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ICombineBodiesFeatureData_members.html). | Validate enum family by method name. Refuse unknown operation codes. |
| C03 | Source `keep_tools=true` lacks a direct parameter in the combine creation signature. | **Source-supplied** SolidCraft schema versus **Recorded** SOLIDWORKS signature. | Refuse `COMBINE_KEEP_TOOLS_UNSUPPORTED` until a checked copy/retention path exists. |
| C04 | Body and edge array ordinals can be unstable or undocumented. | **Recorded:** `GetBodies2` explicitly may reorder. Edge-order guarantee not recovered. | Require source-name and geometric identification. Refuse ambiguous matches. |
| C05 | SolidCraft `edge_refs.min/max` look like edge bounds but represent body bounds. | **Source-supplied** from pinned evaluator. | Do not identify an edge with that field alone. |
| C06 | VBScript can read array bounds on the 2026 seat but fails on typed-array indexed elements. | **Seat evidence in brief 11** and Microsoft marshaling research. | Use VBA/.NET or probe each return family. Refuse unsupported elements. |
| C07 | General SolidCraft axis-angle does not directly match MoveCopyBody's XYZ angles. | **Source-supplied** transform semantics and **Recorded** SW method. | Refuse arbitrary-axis rotations until frame conversion and order are measured. |
| C08 | Fillet input edges may not exist after the fillet. | **Reconstructed / untested**, consistent with feature-data selection access. | Verify inputs through feature definition, not by scanning only the final edge list. |
| C09 | Sweep orientation, rails, profile scale and thin-wall behavior are not one-to-one API enums. | **Source-supplied** `sweep` fields; **Recorded** `ISweepFeatureData`. | Accept only mapped/tested settings. Refuse unsupported options. |
| C10 | Torus near self-intersection may not be representable as the same solid topology. | **Recorded** toroidal-surface parameter classes; **Reconstructed** revolve path. | Require `R>r>0` for initial exact ring-torus support. |
| C11 | Feature-face membership does not prove a unique generated body for all feature types. | **Recorded** obsolete GetBody path, current GetFaces/GetBody chain. | Validate body registry against full pre/post body inventories. |
| C12 | `NumCopies` and `TransDist` corner cases remain unverified. | **Recorded** names and types but not full combined behavior. | Probe explicit zero/one copy and translation-reference cases. |

## Still needs a 2026 licensed-seat probe

The following probes use a **new disposable part** each time. Capture the model version, active document, COM error numbers/descriptions, selected object types/marks, feature-definition readbacks, rebuild codes, body inventory, model geometry, and a verdict sentinel. Negative probes must not modify a production design.

| Priority | Roadmap gate / fixture | Call and state to exercise | Required postconditions | Named refusal |
|---|---|---|---|---|
| P0 | **Body registry**: two separate cuboids with different dimensions | Enumerate `IPartDoc.GetBodies2`, rename where permitted, rebuild, save/reopen, get feature faces and owning bodies | Two unique source-to-live identities after each step, independent of returned order | `BODY_NOT_FOUND`, `BODY_AMBIGUOUS`, `BODY_REGISTRY_CHANGED` |
| P0 | **Edge resolver**: one long edge among near-identical edges | Enumerate edge/face arrays, geometry descriptors, `GetPersistReference3`, `GetObjectByPersistReference3` in VBA; edit upstream dimensions | Exactly one edge matches before fillet and after supported rebuild; bytes are never used as a raw equality oracle | `EDGE_NOT_FOUND`, `EDGE_AMBIGUOUS`, `PERSIST_RESOLVE_FAILED` |
| P0 | **Merge/scope**: overlapping and non-overlapping bodies | `InsertCombineFeature` for add/cut/common. Test direct-body and selection-mark forms; `Array(Body2...)` versus marks. | Correct operation, target identity, volume, count and untouched bodies | `COMBINE_FAILED`, `COMBINE_WRONG_TARGET`, `COMBINE_SCOPE_MISMATCH` |
| P0 | **Expression primitive placement**: two bodies, one driven origin parameter | Change placement parameter alone, then size parameter alone; rebuild/reopen | Body location follows placement change; unrelated size does not change; feature dependency remains editable | `PLACEMENT_NOT_PARAMETRIC`, `DEPENDENCY_LOST` |
| P1 | **D04 move**: asymmetric body, non-origin pivot | Translation alone; rotation alone; then sequential pair, each with re-resolved body; copy true/false | Full affine transform matches source; correct body identity, copy count and preserved originals | `MOVE_WRONG_FRAME`, `MOVE_WRONG_ORDER`, `MOVE_COPY_COUNT_MISMATCH` |
| P1 | **D04 remove**: three bodies, remove only middle | `InsertDeleteBody2(False)` with marked body. Read `IDeleteBodyFeatureData`; rebuild/reopen | Deleted body absent, others present, creating feature remains, no hidden collateral change | `REMOVE_BODY_FAILED`, `REMOVE_WRONG_BODY`, `REMOVE_COLLATERAL` |
| P1 | **D06 fillet**: unique edge and ambiguous twin-edge negative | Builder, mark 1; `GetDefinition`, `AccessSelections`, `Edges`, `DefaultRadius` | Unique intended edge and radius verified against source; resulting geometry and error state valid | `FILLET_EDGE_MISMATCH`, `FILLET_RADIUS_MISMATCH`, `FILLET_ACCESS_FAILED` |
| P1 | **D07 cut**: subtract body/tool with `keep_tools=false/true` | Direct Combine cut; copy tool when requested; test resulting body names and volumes | Target cut, original tool retention matches source, unrelated bodies unchanged | `COMBINE_KEEP_TOOLS_UNSUPPORTED`, `CUT_TOOL_CONSUMED` |
| P2 | **Sweep/R2**: straight and curved path | `CreateDefinition(swFmSweep)` with profile mark1, path4, optional guides2 | Correct profile, path, orientation, solid count and cross-sections | `SWEEP_UNSUPPORTED_ORIENTATION`, `SWEEP_WRONG_PATH` |
| P2 | **Torus**: ring torus with known `R`, `r` | Sketch circle + full revolve + `ISurface.TorusParams` | One toroidal solid, centre/axis/radii match, correct volume, rebuild stable | `TORUS_PARAMS_MISMATCH`, `TORUS_NOT_SOLID` |
| P2 | **VBScript array samples** across every added readback | `VarType`, `IsArray`, bounds, `raw(i)`, `For Each` and all output params | All required objects and numerics are read without error, or a typed adapter returns equivalent checked values | `COM_ARRAY_ELEMENT_UNREADABLE`, `COM_BYREF_UNCALLABLE` |

### Suggested acceptance sequence

1. Establish the body registry and unique edge resolver. Save a baseline model and compare it with a fresh rebuild.
2. Test Boolean add/cut/common and exact scope. Test body count, retained tools and geometry after each step.
3. Prove expression placement and independent parameter edits. Reject static-coordinate snapshots.
4. Test move/copy and Body-Delete/Keep with three-body fixtures. Verify identity at every history step.
5. Enable fillet and sweep/torus features only after their input selections and reverse readbacks pass.
6. Run the original synthetic and design fixtures against the exact STEP reference. Enforce the independent conic gate.

The fixture order avoids attributing a missing body or wrong edge to a fillet when the registry is already incorrect.

## Emitter and checker contracts

**Emitter (VBA):**

- Preserve user preferences when entering/exiting sketch creation (brief 08). Never rely on inference for exact endpoint geometry.
- Resolve source body/edge references before selecting an operation. Record exact mark and selected-object type.
- Never use a returned array ordinal as a durable model identifier.
- Treat failed `Select4`, `SelectByID2`, `CreateFeature` or rebuild as errors with named refusal codes.
- Save the entity reference within the correct model context. Resolve and validate it before every topology-dependent operation.
- Convert SolidCraft mm to SOLIDWORKS m, and source angles to radians where required.
- Preserve source tool-retention, scope, feature order and parameters. If a mapping is unsupported, refuse it rather than approximating silently.

**Acceptance checker (VBScript or typed adapter):**

- Require an explicit successful COM call and inspected result type. A valid array bound alone is insufficient.
- Refuse object-valued or inaccessible numeric components until a typed adapter confirms conversion (brief 11).
- Verify input feature definitions under the documented selection-access protocol. Release the access before further rebuild work.
- Verify body count, expected names, centroid/volume and topology changes. Where available, compare against independent reference STEP geometry.
- Check all expected positive and negative postconditions. Do not infer success from a zero-length exception log.
- Emit a named failure such as `EDGE_AMBIGUOUS` or `BODY_NOT_FOUND` when an identity cannot be established.

## Key sources

### Official SOLIDWORKS API Help

1. [Persistent Reference IDs, programming guide 2026](https://help.solidworks.com/2026/english/api/sldworksapiprogguide/Overview/Persistent_Reference_IDs.htm).
2. [GetPersistReference3](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~GetPersistReference3.html).
3. [GetObjectByPersistReference3](https://help.solidworks.com/2026/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~GetObjectByPersistReference3.html).
4. [Persistent-reference state enum](https://help.solidworks.com/2026/english/api/swconst/SolidWorks.Interop.swconst~SOLIDWORKS.Interop.swconst.swPersistReferencedObjectStates_e.html?id=2.10.1.596).
5. [Official C++ byte-array example](https://help.solidworks.com/2014/English/api/sldworksapi/Get_Object_s_Persistent_Reference_ID_Example_CPlusPlus_COM.htm).
6. [IPartDoc.GetBodies2](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IPartDoc~GetBodies2.html).
7. [IEdge.GetCurveParams3](https://help.solidworks.com/2012/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iedge~getcurveparams3.html).
8. [IFeature members, including faces, definitions and errors](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeature_members.html).
9. [IFeature.GetBody obsolete](https://help.solidworks.com/2026/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeature~GetBody.html).
10. [Obsolete FeatureFillet creation method](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager~FeatureFillet.html).
11. [ISimpleFilletFeatureData2 interface and official builder](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2.html?id=2.2.1.334).
12. [ISimpleFilletFeatureData2 member inventory](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2_members.html?id=3e2e92900bb94199a10948bc9ad82acc).
13. [InsertCombineFeature](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html).
14. [Combine feature-data members](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ICombineBodiesFeatureData_members.html).
15. [InsertMoveCopyBody2, 2013](https://help.solidworks.com/2013/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html).
16. [InsertMoveCopyBody2, 2022](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html).
17. [InsertDeleteBody2](https://help.solidworks.com/2015/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertDeleteBody2.html).
18. [Official Delete/Keep Body VBA example](https://help.solidworks.com/2024/English/api/sldworksapi/Insert_Delete_Body_Feature_Example_VB.htm).
19. [Sweep feature-data guide](https://help.solidworks.com/2021/English/api/sldworksapiprogguide/Overview/Sweep_Features_and_SweepFeatureData_Objects.htm).
20. [ISweepFeatureData members 2026](https://help.solidworks.com/2026/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISweepFeatureData_members.html?id=8afcd5e9d83b42e2a51d8dcd4c74b9d9).
21. [FeatureRevolve2](https://help.solidworks.com/2021/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureRevolve2.html).
22. [ISurface.TorusParams](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISurface~TorusParams.html).

### Source-supplied SolidCraft references (tag `v0.43.0`)

23. [Document and feature schema](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs).
24. [Edge resolver and geometry evaluator](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L5475-L5570).
25. [Move evaluation](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L10781-L10834).
26. [Body removal](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval_more.rs#L193-L205).
27. [Native format and feature guide](https://docs.solidcraft.io/guide/features.html).

**Limitations:** Documentation availability varies by year and URL. The years shown are API Help editions, not evidence of seat execution. Some composite VBA examples are reconstructed from individual documented members and remain untested. The report does not establish the exact shape of every 2026 COM SAFEARRAY or the output body identity after every Boolean operation. Each unresolved case has a named seat probe above.
