# SOLIDWORKS API method-page research: probe queue 04

**Filed:** 2026-11-17, produced by the owner's research agent from public
API documentation. Answer to `research/inputs/04-solidworks-method-pages-probe-queue.md`.
Provenance note: several of this report's P0 unknowns were answered the
same day by `corpus/compat-check/probe-refplane.swb` (docs/source-probe-evidence.md):
`IEquationMgr.Add` returns the inserted index on this seat and the stored
equation reads back exactly; the offset-plane dimension name is `D1`
(qualified `D1@<feature>` both accepted and driving); `InsertRefPlane(8,
d, 0,0,0,0)` with a pre-selected origin plane works and the transform
origin is entries 9..11; bare equation numbers are DOCUMENT units. Still
open from this report: the `Add2`/`Add3` recommendation, flip-bit
semantics (256/8192), `CreateConic`'s signature (P0 blocker for Pierre),
move/copy precedence, combine tool preservation, and modal-dialog
blocking.

---

**Research date:** 2026-11-17
**Target:** SOLIDWORKS 2026 desktop macro seat, revision 34.x
**Project:** `craft-bridge`, replay macro generation
**Input:** `04-solidworks-method-pages-probe-queue.md`, follow-up to research brief 03
**Scope:** Public SOLIDWORKS API documentation, SDK programming guides, documented examples, and older official method pages for version comparisons.

SOLIDWORKS is a Dassault Systèmes trademark. Names in this report identify its APIs. No affiliation is implied.

## Evidence and implementation rules

| Label | Meaning |
|---|---|
| **Documented** | A linked official method page, enumeration page, or SDK guide states the behavior or signature. The linked year is the **documentation version**, not proof of an identical 2026 implementation. |
| **Inferred** | The behavior follows from documented types or examples, but the precise case is not directly specified. |
| **Unverified** | Available official text does not settle the issue or two official versions conflict. A 2026 seat observation is necessary. |

**Default rule:** Use COM object references and documented feature-data builders when available. Validate each created feature, its definition, and the rebuilt geometry. Treat return values as one observation, not proof of a correct model. All geometry coordinates and lengths below use **metres** unless an API page specifies otherwise. Angles normally use **radians**, but ambiguous signatures remain unverified until measured.

---

## T1. Equation manager: parametric reference-plane blocker

### Member surface

| API member | Signature / return shape | Evidence and version notes |
|---|---|---|
| `IModelDoc2.GetEquationMgr` | Returns `IEquationMgr` | **Documented.** [Equation manager interface](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IEquationMgr.html). |
| `IEquationMgr.GetCount` | `GetCount() As Long` | **Documented.** Enumerate indexes `0..GetCount-1`. [Members](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IEquationMgr_members.html). |
| `Add` | `Add(Index As Long, Equation As String) As Long` | **Documented.** Obsolete; use `Add2` or `Add3`. `Index=-1` appends. Current page describes inserted index or `-1` error. [Add](https://help.solidworks.com/2020/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~Add.html). |
| `Add2` | `Add2(Index As Long, Equation As String, Solve As Boolean) As Long` | **Documented, contradictory.** Available since SOLIDWORKS 2010 (rev 18.0). `Index=-1` appends. `Solve=False` delays evaluation. [2013](https://help.solidworks.com/2013/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~Add2.html); [2017](https://help.solidworks.com/2017/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iequationmgr~add2.html). |
| `Add3` | `Add3(Index As Long, Equation As String, Solve As Boolean, WhichConfigurations As Long, ConfigNames As Object) As Long` | **Documented.** Configuration-aware form for supported multiconfiguration models. Inserted index or `-1`. [Add3](https://help.solidworks.com/2023/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~Add3.html). Use `Add2` if configuration control is not needed. |
| `Equation` | `Equation(Index As Long) As String` (get and set) | **Documented.** Contains the expression with the equation's left-hand side. Assign to change an equation. Not a numeric value. [Equation](https://help.solidworks.com/2011/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~Equation.html). |
| `Value` | `Value(Index As Long) As Double` (get) | **Documented.** Evaluated **numeric** result, not the expression. [Value](https://help.solidworks.com/2010/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~Value.html). |
| `Delete` | `Delete(Index As Long) As Long` | **Documented, version-sensitive return.** Documentation versions disagree on whether the success return is an index or `0`; inspect postcondition rather than use the number alone. [2013](https://help.solidworks.com/2013/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~Delete.html); [2023](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~Delete.html). |
| `EvaluateAll` | `EvaluateAll() As Long` | **Documented.** The [2022 page](https://help.solidworks.com/2022/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~EvaluateAll.html) reports `-1` for success **and** failure. Do not use this value as a status flag. |
| `SetEquationAndConfigurationOption` | `SetEquationAndConfigurationOption(Index As Long, Equation As String, WhichConfigurations As Long, ConfigNames As Object) As Long` | **Documented.** Modify equation text/configuration assignment; distinguish it from plain `Equation(Index)=...`. [Method](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~SetEquationAndConfigurationOption.html). |
| `Status` | `Status As Long` (read-only) | **Documented.** For indexed operations, index of most recently successful equation; `-1` on error. It is not a complete rebuild diagnostic. [Status](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~Status.html). |
| Other documented properties | `GlobalVariable(Index)`, `AutomaticRebuild`, `AutomaticSolveOrder`, `Disabled(Index)`, `ConfigurationCount`, `FilePath`, configuration-specific and suppression accessors | **Documented names; exact signatures vary by member.** Use the [complete official member index](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IEquationMgr_members.html) for the full surface rather than assume a simple `Edit`, plural `Equations`, or plural `Values` method exists. |

**Critical return-value conflict:** The 2013 `Add2` page explicitly lists `-1` for both outcomes. The 2017 page instead gives the new equation index on success and `-1` on error. **Unverified for 2026:** Whether `Add2` consistently returns the index on the target seat. Probe using an empty list, two valid insertions, and one malformed expression. Compare `GetCount`, `Equation(index)`, `Status`, and `Value` after a rebuild. [2013 method page](https://help.solidworks.com/2013/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEquationMgr~Add2.html), [2017 method page](https://help.solidworks.com/2017/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iequationmgr~add2.html).

### Equation syntax and identity

| Case | Form / consequence | Status |
|---|---|---|
| Global variable | `"BAR" = 0.025` (quoted symbol on the left) | **Documented.** [Add2](https://help.solidworks.com/2017/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iequationmgr~add2.html). |
| Dimension equation | `"D1@Sketch1" = "BAR" / 2` and UI-derived `"D1@Extrude2" = ...` | **Documented** grammar: names in double quotation marks, left side is a qualified dimension. [Add2](https://help.solidworks.com/2017/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iequationmgr~add2.html). |
| Offset plane's dimension | `"D1@Plane1"` is a plausible feature-dimension qualified name. | **Inferred.** The method page does not guarantee the name `D1` or `Plane1` for a generated offset plane. |
| Runtime dimension name | Passing `IDimension.Name` or a name from `GetDimension2` verbatim into an equation. | **Unverified.** Read the real display-dimension full name and compare to a manually recorded equation. Do not assume a short name is enough. |
| Nested/assembly qualification | Context-sensitive paths can differ from a bare `D1@FeatureName`. | **Unverified** for the replay's part and assembly use cases. Capture exact UI-created equation and API readback before normalizing. |
| Edit variable and rebuild once | Set `Equation(index)`, call `EvaluateAll`, call `EditRebuild3`, inspect dependents. | **Inferred** sequence. One rebuild fully re-evaluating every dependent feature is **Unverified**. |
| Error reporting | `Status`, return code, equation list and `Value` can detect some failures. | **Documented** accessors, **Unverified** coverage. Some invalid geometric consequences may need a feature or body check. |

**Recommended probe:** Create a plane whose offset depends on a global variable. Save its dimension name. Change the variable from 10 mm to 20 mm. Capture equation text, evaluated values, feature rebuild state, plane transform, and actual offset before and after `EditRebuild3`. This directly tests dependency propagation.

---

## T2. Reference planes and selection

### Method contract and constraints

```vb
' IFeatureManager
Function InsertRefPlane( _
    FirstConstraint As Long, FirstConstraintAngleOrDistance As Double, _
    SecondConstraint As Long, SecondConstraintAngleOrDistance As Double, _
    ThirdConstraint As Long, ThirdConstraintAngleOrDistance As Double _
) As Object
```

**Documented:** `InsertRefPlane` uses selected reference entities. The first, second, and third reference entities use selection marks `0`, `1`, and `2`. The constraint arguments are bitmasks; distance arguments are metres and angular arguments use the API angular unit. [InsertRefPlane method](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertRefPlane.html), [constraint enum](https://help.solidworks.com/2022/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swRefPlaneReferenceConstraints_e.html?id=2.10.1.634).

| Constraint | Enum value | Typical selection proposal | Evidence |
|---|---:|---|---|
| `swRefPlaneReferenceConstraint_Parallel` | `1` | Planar face or plane | **Documented** bit, **Inferred** acceptance for a concrete geometry. |
| `..._Perpendicular` | `2` | Plane / straight edge or axis | **Documented** bit, **Inferred** combination. |
| `..._Coincident` | `4` | Plane / planar face | **Documented** bit, **Inferred** combination. |
| `..._Distance` | `8` | One plane or planar face plus signed or unsigned distance | **Documented** bit, **Inferred** reference-specific acceptance. |
| `..._Angle` | `16` | Plane and line/axis for an angled plane | **Documented** bit, **Unverified** allowed selection/constraint mapping. |
| `..._Tangent` | `32` | Curved face with another orienting reference | **Documented** bit, **Unverified** complete pairing. |
| `..._Project` | `64` | Projected reference | **Documented** bit, **Unverified** pairing. |
| `..._MidPlane` | `128` | Two parallel planes / planar faces | **Documented** bit, **Inferred** exact acceptance. |
| `..._OptionFlip` | `256` | Modifier of an existing primary constraint | **Documented** bit; geometric meaning for offsets **Unverified**. |
| `..._OptionReferenceFlip` | `8192` | Alternate reference direction | **Documented** bit; precise offset side **Unverified**. |

*Constraint spellings are represented by their enum prefixes. The exact member spelling should be copied from the target interop type library, not reconstructed from the abbreviations above.* [Official constraint enum](https://help.solidworks.com/2022/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swRefPlaneReferenceConstraints_e.html?id=2.10.1.634).

**Unused arguments:** Calling `InsertRefPlane(8, 0.01, 0, 0, 0, 0)` for a one-reference offset is a reasonable proposal, but **Unverified** for every accepted reference/constraint pairing. The signature always requires six positional values in VBA. A COM call cannot omit the last four values just because they are unused. Probe both successful and rejected combinations. [Method signature](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertRefPlane.html).

**Flip test:** For the same selected `Front Plane`, compare `(8, +0.01)`, `(8 Or 256, +0.01)`, `(8 Or 8192, +0.01)`, and `(8, -0.01)`. Record signed distance along the base plane's normal. **Unverified:** Whether 256/8192 reverse the side, the reference orientation, or the value interpretation. Do not hardcode a semantic interpretation from the numeric flags alone.

### Created feature and transform

| API or observation | Finding | Status and source |
|---|---|---|
| Tree feature type | `IFeature.GetTypeName2()` can distinguish `RefPlane`; inspect returned feature. | **Documented** feature type vocabulary; verify actual created feature on seat. [GetTypeName2](https://help.solidworks.com/2022/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeature~GetTypeName2.html). |
| Specific feature | `IFeature.GetSpecificFeature2()` is expected to return `IRefPlane` for a plane feature. | **Inferred** from [IRefPlane interface](https://help.solidworks.com/2025/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IRefPlane~Transform.html). Confirm `TypeName` on 2026. |
| Definition edit | `IFeature.GetDefinition()` exposes `IRefPlaneFeatureData`. Relevant properties include `Distance`, `Angle`, `Type2`, and reference direction. | **Documented.** [IRefPlaneFeatureData members](https://help.solidworks.com/2019/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IRefPlaneFeatureData_members.html). |
| Transform | `IRefPlane.Transform` returns `IMathTransform`; its `ArrayData` gives the transform. | **Documented.** [IRefPlane.Transform](https://help.solidworks.com/2025/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IRefPlane~Transform.html). |
| Transform element layout | 16 values. Rotation occupies `0..8`; translation **`9..11`**; scale `12`; remaining entries `13..15` are unused. Translation is in metres. | **Documented.** [CreateTransform](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IMathUtility~CreateTransform.html). Matrix multiplication conventions still matter for interpreting axes. |
| Offset dimension name | One generated display dimension for an offset plane, often represented as `D1@PlaneN`. | **Unverified.** Enumerate feature display dimensions and compare their full names against Equation Manager text. |
| Select again for sketch | Select the returned `IFeature` with `Select2(False, 0)`, then call `ISketchManager.InsertSketch(True)` to enter a sketch. | **Documented** selection/insert pattern; verify state and active sketch. [Reference-plane/sketch example](https://help.solidworks.com/2024/english/api/sldworksapi/Create_Cylinder_Example_CPlusPlus_COM.htm). |
| Name-based selection | `IModelDocExtension.SelectByID2(featureName, "PLANE", ..., mark=0, ...)` is a documented selection pattern. | **Documented** generic grammar, **Unverified** if generated/localized/assembly names work unchanged. [SelectByID2](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~SelectByID2.html). |

Prefer `IFeature.Select2` on the object returned by creation instead of reconstructing its user-visible name.

---

## T3. Cut extrusions, combine, body deletion

### Cut-extrusion API

```vb
' IFeatureManager; 27 positional arguments
Function FeatureCut4( _
    Sd As Boolean, Flip As Boolean, Dir As Boolean, _
    T1 As Long, T2 As Long, D1 As Double, D2 As Double, _
    Dchk1 As Boolean, Dchk2 As Boolean, _
    Ddir1 As Boolean, Ddir2 As Boolean, _
    Dang1 As Double, Dang2 As Double, _
    OffsetReverse1 As Boolean, OffsetReverse2 As Boolean, _
    TranslateSurface1 As Boolean, TranslateSurface2 As Boolean, _
    NormalCut As Boolean, UseFeatScope As Boolean, _
    UseAutoSelect As Boolean, AssemblyFeatureScope As Boolean, _
    AutoSelectComponents As Boolean, PropagateFeatureToParts As Boolean, _
    T0 As Long, StartOffset As Double, _
    FlipStartOffset As Boolean, OptimizeGeometry As Boolean _
) As Feature
```

**Documented.** [FeatureCut4 (2024)](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurecut4.html).

| Argument group | Meaning | Evidence |
|---|---|---|
| `Sd`, `Flip`, `Dir` | Single-direction cut, reverse/remove outside profile setting, and direction control | **Documented** [FeatureCut4](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurecut4.html). Exact interaction with open profiles requires a seat probe. |
| `T1`, `T2` | End conditions of first and second directions; see `swEndConditions_e` | **Documented.** [FeatureCut4](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurecut4.html). |
| `D1`, `D2`, `Dchk*`, `Ddir*`, `Dang*` | Depth, draft enabled, draft direction, draft angle, respectively | **Documented.** Depths in metres and angles in radians. [FeatureCut4](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurecut4.html). |
| `OffsetReverse*`, `TranslateSurface*` | Options relevant to end-condition face/surface offsets | **Documented** arguments. [FeatureCut4](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurecut4.html). |
| `NormalCut`, `OptimizeGeometry` | Primarily sheet-metal normal-cut options | **Documented.** Use `False` for ordinary solid extrusions. [FeatureCut4](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurecut4.html). |
| `UseFeatScope`, `UseAutoSelect`, assembly scope settings | Control which bodies/components the feature affects | **Documented** arguments. Confirm source-body scoping in a multibody part. [FeatureCut4](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurecut4.html). |
| `T0`, `StartOffset`, `FlipStartOffset` | Start condition (`swStartConditions_e`) and offset from sketch plane | **Documented.** [FeatureCut4](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurecut4.html). |

**Difference from boss extrusion:** `FeatureExtrusion2` / `FeatureExtrusion3` use a separate argument list and add material. Do not reuse the `FeatureCut4` positional parameter list for boss creation. **Documented** by their distinct API entry points. [FeatureCut4](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurecut4.html), [IFeatureManager members](https://help.solidworks.com/2019/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager_members.html).

### Combine operation

```vb
' IFeatureManager
Function InsertCombineFeature( _
    OperationType As Long, MainBody As Body2, ToolVar As Variant _
) As Feature
```

| Question | Finding | Evidence |
|---|---|---|
| Operation values | `SWBODYINTERSECT=15901`, `SWBODYCUT=15902`, `SWBODYADD=15903` | **Documented.** [swBodyOperationType_e](https://help.solidworks.com/2024/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swBodyOperationType_e.html?format=P&value=). |
| Body references | `MainBody` takes a body object; `ToolVar` accepts the tool body/bodies as a variant. Method also supports a marked-selection workflow. | **Documented.** [InsertCombineFeature](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html). |
| Marked selection | Main body mark `1`, tools mark `2` in the documented selection-based route. | **Documented.** [InsertCombineFeature](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html). |
| Operation-specific main-body requirement | Subtract (`SWBODYCUT`) needs a target body; add/intersect do not use the target in the same way. | **Documented** method constraints. [InsertCombineFeature](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html). |
| Best VBA route | Pass direct `Body2` objects where possible. If `ToolVar` coercion fails, use documented marked selections with empty body parameters. | **Inferred** implementation choice; probe both variants on target 2026 seat. |
| `ICombineBodiesFeatureData` | Exposes operation and relevant selected bodies, e.g. `BodiesToCombine`, `MainBody`, `OperationType`. | **Documented.** [Members](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ICombineBodiesFeatureData_members.html). |
| Preserve tool bodies | No documented keep-tool / preserve-tools member appears in `ICombineBodiesFeatureData`. | **Documented** interface limitation. Whether an existing body can be retained by another feature sequence is **Unverified**. [Members](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ICombineBodiesFeatureData_members.html). |
| Editing | `IFeature.GetDefinition` / `IFeature.ModifyDefinition` is the general feature-data editing route. | **Documented** general route, **Unverified** for changing a combine feature's body set after creation. [Combine data members](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ICombineBodiesFeatureData_members.html). |

A replay implementation must not assume that SOLIDWORKS preserves the original tool body after `SWBODYCUT`. Check body count, topology, and names after creation. A later retained copy may require an explicit copy before combining.

### Delete / keep bodies

```vb
' IFeatureManager
Function InsertDeleteBody2(KeepBodies As Boolean) As Feature
```

**Documented:** Select the bodies before the call. `KeepBodies=False` means delete the selected bodies; `True` means keep selected bodies and delete the others. A Delete/Keep Body feature appears in the tree, and its definition uses `IDeleteBodyFeatureData`, including `Bodies` and `KeepBodies`. [InsertDeleteBody2](https://help.solidworks.com/2015/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertDeleteBody2.html), [IDeleteBodyFeatureData members](https://help.solidworks.com/2023/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IDeleteBodyFeatureData_members.html?id=0ea97e6fba8f4d7ab10404594f3324eb).

**Unverified:** What happens to dependent downstream features when a source body is deleted, and which exact `GetTypeName2()` string describes the resulting feature. Probe a clean standalone body and a body referenced by a later fillet. Record rebuild flags and suppressed/failed downstream features.

---

## T4. Body naming, selection, transformation, mass properties

| Question | Result | Evidence |
|---|---|---|
| Read or set body name | `IBody2.Name As String` supports get/set. Use unique names. | **Documented.** [IBody2.Name](https://help.solidworks.com/2021/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IBody2~Name.html). |
| Name generated by feature | May depend on feature tree and multibody ordering; do not rely on `Boss-Extrude1[1]`. | **Unverified** for the exact replay template and document language. Inspect each new body and assign a unique generated name. |
| `SelectByID2` body grammar | A body may be selected by a feature-dependent display name. | **Unverified.** Capture SOLIDWORKS selection macro output and compare selections in the 2026 seat. Prefer body-object selection. |
| Body object selection | `IBody2.Select2(Append As Boolean, Data As SelectData) As Boolean`. `Data.Mark` supplies a selection mark. | **Documented.** [IBody2.Select2](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IBody2~Select2.html). |
| Entity vs body selection | `IEntity.Select4` is for `IEntity` implementers; body references have their own `Select2`. | **Documented** distinct interfaces. [IBody2.Select2](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IBody2~Select2.html). |
| Move/copy edit | The generated feature provides `IMoveCopyBodyFeatureData` for reading bodies and transforms. | **Documented.** [IMoveCopyBodyFeatureData members](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IMoveCopyBodyFeatureData_members.html). |

### Move/copy body exact signature

```vb
' IFeatureManager
Function InsertMoveCopyBody2( _
    TransX As Double, TransY As Double, TransZ As Double, _
    TransDist As Double, _
    RotPointX As Double, RotPointY As Double, RotPointZ As Double, _
    RotAngleX As Double, RotAngleY As Double, RotAngleZ As Double, _
    BCopy As Boolean, NumCopies As Long _
) As Feature
```

**Documented:** Bodies or surface bodies use selection mark **1**. A rotation axis/reference uses **2**. Translation direction/reference uses **4**. A translation vertex can use **8**. `BCopy` enables body copies and `NumCopies` specifies the requested count. [2022 method](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html), [2013 method](https://help.solidworks.com/2013/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html).

**Documented contradiction across versions:** The 2013 method page says a call supplying both translation and rotation applies **only translation**. The 2022 method page states that rotation parameters have precedence and translation arguments apply only if rotation parameters are null or `Nothing`. **Unverified for VBA/2026:** These are declared as non-nullable `Double` in the COM/NET signature, so the exact zero/default behavior needs a seat test. [2013](https://help.solidworks.com/2013/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html), [2022](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html).

| Open parameter | Why it matters | Minimal observation |
|---|---|---|
| `TransX/Y/Z` versus `TransDist` | **Unverified** whether XYZ are independent translations or direction information when a reference is selected. | Move a cube +10 mm along X with no marked direction, then along an edge with mark `4`; compare body centroid. |
| `RotAngleX/Y/Z` | **Unverified** use of axis-angle versus independent Euler components and exact angle units in this method. | Rotate asymmetric block around one axis by known angle, record vertex coordinates and feature definition. |
| Reference mark combinations | **Documented** mark assignments, **Unverified** how missing references interact with numerical parameters. | Repeat a translation with marks `1` only, then `1+4` and `1+4+8`. |
| Combined transform | Contradictory old/new pages. | Supply nonzero translation and rotation in one call, compare against two separate sequential features. |

**Implementation rule:** For mixed moves and rotations, create separate features until the 2026 probe establishes the semantics.

### Mass property array

```vb
' IBody2
Function GetMassProperties(Density As Double) As Variant
```

| Index | Quantity | Index | Quantity |
|---:|---|---:|---|
| `0` | Centre of mass X (m) | `6` | `Ixx` (kg m²) |
| `1` | Centre of mass Y (m) | `7` | `Iyy` (kg m²) |
| `2` | Centre of mass Z (m) | `8` | `Izz` (kg m²) |
| `3` | Volume (m³) | `9` | `Ixy` (kg m²) |
| `4` | Surface area (m²) | `10` | `Izx` (kg m²) |
| `5` | Mass (kg) | `11` | `Iyz` (kg m²) |

**Documented:** The body API returns a 12-element numeric array for a **solid** body and accepts an explicit density. For a sheet body, slots `3` and `4` contain area and circumference. For a wire body, slot `3` contains length. Body coordinates are in the body's creation coordinate system, not automatically the assembly coordinate system. [IBody2.GetMassProperties](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ibody2~getmassproperties.html). **Unverified:** Whether body-level and document-level mass-properties computations agree after equivalent density, selected body set, and coordinate-system settings. Compare against `IModelDocExtension.CreateMassProperty`/its current variant or `GetMassProperties2` only after establishing consistent settings. [Document-level mass-property API](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imodeldocextension~getmassproperties2.html).

---

## T5. Edge identity, picking and fillet builder

### Geometry descriptors

| API | Return / role | Evidence |
|---|---|---|
| `IEdge.GetCurve()` | Returns the supporting `ICurve` for the edge. | **Documented.** [IEdge geometry](https://help.solidworks.com/2012/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iedge~getcurveparams3.html). |
| `IEdge.GetCurveParams3()` | Returns curve-parameter data including trimmed parameter domain and boundary geometry. `GetCurveParams2` is obsolete. | **Documented.** [GetCurveParams3](https://help.solidworks.com/2012/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iedge~getcurveparams3.html). |
| `IEdge.GetStartVertex()` / `IEdge.GetEndVertex()` | Return boundary `IVertex` objects when present. Closed or special edges may not expose two distinct vertices. | **Documented** API names. [GetStartVertex](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEdge~GetStartVertex.html), [GetEndVertex](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IEdge~GetEndVertex.html). |
| `ICurve.GetLength3(UStart As Double, UEnd As Double) As Double` | Length of a curve between its parameter limits, in metres. | **Documented.** [GetLength3](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ICurve~GetLength3.html?format=P&value=). |
| `ICurve.Evaluate2(Parameter As Double, NumberOfDerivatives As Long) As Variant` | Evaluates XYZ coordinates and requested derivative vectors at a curve parameter. | **Documented.** [Evaluate2](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.icurve~evaluate2.html). |
| Edge descriptor | Arc/circle/line type, endpoint positions, length, sampled midpoint and tangent; add adjacent-face information when needed. | **Inferred** stable matching strategy, not a promised persistent identity. Collect fields after every rebuild. |

**Probe:** For one line, arc, and closed circle edge, record `GetCurveParams3`, start/end vertices (including `Nothing`), sampled midpoint, first derivative and `GetLength3`. Compare these with numerical edge length and body bounding box. Use tolerance-aware matching rather than exact double equality.

### Selection and verification

| Operation | Finding | Evidence |
|---|---|---|
| `IModelDocExtension.SelectByRay(X,Y,Z,DirX,DirY,DirZ,Radius,TypeWanted,Append,Mark,Option) As Boolean` | Selects geometry intersected by a pick ray. It is not guaranteed to return the globally nearest edge. | **Documented** signature; nearest-edge limitation **Inferred**. [SelectByRay](https://help.solidworks.com/2025/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~SelectByRay.html). |
| `SelectAtPoint` | No confirmed method with this name was established on the relevant standard model/extension interface. | **Unverified** name. Do not emit this call without a 2026 Object Browser confirmation. |
| `ISelectData.Mark` | Assign the required selection mark to an entity selected by object. | **Documented.** [IBody2.Select2](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IBody2~Select2.html). |
| `ISelectionMgr.GetSelectedObject6(Index As Long, Mark As Long) As Object` | Read an actual selected COM object. Selection index is 1-based. Use an applicable mark filter or `-1` for all. | **Documented.** [GetSelectedObject6](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iselectionmgr~getselectedobject6.html). |
| Selection type | `GetSelectedObjectType3(index, mark)` checks edge/face/sketch type after the pick. | **Documented** companion API. [GetSelectedObject6](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.iselectionmgr~getselectedobject6.html). |

**Implementation rule:** If a topology `IEdge` object already exists, select the object directly rather than use a ray. After selection, read `GetSelectedObject6` and compare its geometric descriptor to the intended edge.

### Constant-radius fillet using the feature-data builder

The official `ISimpleFilletFeatureData2` interface supports a constant-radius fillet and requires explicit options and edge selection. [Interface](https://help.solidworks.com/2022/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISimpleFilletFeatureData2.html).

1. Obtain `IFeatureManager.CreateDefinition(swFmFillet)` and cast the result to `ISimpleFilletFeatureData2`.
2. Call `Initialize(swConstRadiusFillet)`. Configure circular cross-section, overflow handling, tangent propagation, and radius.
3. Select the target edges with **mark `1`**. Set `DefaultRadius` in **metres**.
4. Call `IFeatureManager.CreateFeature(data)`. Confirm a non-`Nothing` feature, read its definition, rebuild and inspect its affected edge geometry.

**Documented:** Builder and selection mark from [ISimpleFilletFeatureData2](https://help.solidworks.com/2022/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISimpleFilletFeatureData2.html), radius units from [DefaultRadius](https://help.solidworks.com/2025/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISimpleFilletFeatureData2~DefaultRadius.html). **Unverified:** Whether the resulting radius dimension has a stable grammar such as `D1@FilletN`. Enumerate actual display dimensions rather than assume the string.

### Persistent topology references

**Documented:** `IModelDocExtension.GetPersistReference3(object)` creates a persistent-reference byte identifier. `GetObjectByPersistReference3(referenceBytes, ErrorCode)` can resolve the reference in a compatible model, including after save/reopen, subject to documented validity. [Persistent reference identifiers guide](https://help.solidworks.com/2026/english/api/sldworksapiprogguide/Overview/Persistent_Reference_IDs.htm?id=1.4.45).

**Unverified:** Persistence across topology-changing rebuilds. A generated fillet, split, merge, or reordered feature can invalidate an edge reference even when the part remains valid. Probe three stages: immediate reuse, rebuild with only a dimensional change, then save/reopen and structural topology change. Record the returned object, error code and geometry descriptor. Never rely solely on persistent bytes to locate an edge after feature-tree edits.

---

## T6. Sketch curves: conics, arcs, splines, ellipse

### Conics: current blocker

| Question | Finding | Evidence |
|---|---|---|
| Method existence | `ISketchManager.CreateConic` is listed as an API member in official older SDK indexes. | **Documented.** [SketchManager members](https://help.solidworks.com/2018/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager_members.html). |
| Complete `CreateConic` arguments | Official accessible method text in this research did **not** establish the complete ordered parameter list or its parameter meanings. | **Unverified.** Read the 2026 type library in Object Browser and record the signature before implementing. |
| `rho`, apex/vertex, point order | No verified official page pinned which arguments specify endpoints, apex, control vertex and conic constant. | **Unverified.** Do not assume rational-quadratic `rho=0.5` means parabola for this API. |
| Supported conic types | No complete source-backed enumeration of accepted types. | **Unverified.** Probe representative elliptic, parabolic and hyperbolic cases and read the resulting curve. |
| `CreateConicByEqnParams` | An exact official declaration was not established in the available member-page evidence. | **Unverified.** Do not call until found in target Object Browser or official method page. |

**Minimal conic probe:** Use the 2026 VBA Object Browser to copy the full signature. Draw a known symmetric conic. Change only one shape parameter at a time. For each result, record endpoints, apex, type, parametric curve and tangent. Keep the probe code separate from `craft-bridge` until the coordinates and shape parameter agree with a known analytic conic.

### Arcs

```vb
' ISketchManager
Function CreateArc( _
    XC As Double, YC As Double, ZC As Double, _
    X1 As Double, Y1 As Double, Z1 As Double, _
    X2 As Double, Y2 As Double, Z2 As Double, _
    Direction As Short _
) As SketchSegment
```

**Documented:** The first XYZ triple is the centre. The next two are start and end points. `Direction` controls which arc is created. Use `ISketchArc` to read centre and endpoints via its sketch-point accessors and verify them against the input coordinates. [CreateArc](https://help.solidworks.com/2013/English/Api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateArc.html), [ISketchArc members](https://help.solidworks.com/2025/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchArc_members.html?id=2.2.1.359.0).

| Arc detail | Status | Probe |
|---|---|---|
| Centre/start/end readback | **Documented** sketch-arc point accessors; confirm object types and coordinate transforms. | Record start/end/centre and compare in sketch coordinates and model coordinates. |
| Arc radius dimension | **Documented** model dimension APIs include `AddRadialDimension2` and `AddDiameterDimension2`. | Select only arc, call radial/diameter dimension, inspect attached `IDimension` and value. |
| Included angle and arc length dimensions | **Unverified** general selection pattern and attachment behavior with `AddDimension2`. | Create examples using UI; record macro and selected segment count, then replay. |
| `Direction` numeric convention | **Unverified** for all degenerate/near-360-degree cases. | Repeat same endpoints in both directions and compare angular sweep. |

### Splines

```vb
' ISketchManager
Function CreateSpline3( _
    PointData As Variant, Surfs As Variant, Direction As Variant, _
    SimulateNaturalEnds As Boolean, ByRef Status As Variant _
) As Variant
```

**Documented:** `PointData` is a flat array of point coordinates. The official 2D example uses at least six values (two XYZ triples), while the remarks separately state that Z is not used for a 2D sketch. `Surfs` and `Direction` must be `Nothing` for an ordinary 2D spline. For an on-surface spline, they are arrays of `ISurface` and `IMathVector` objects. `Status` is a Boolean array for on-surface projection and is empty for a 2D spline. The call returns an `ISketchSegment` or null. Available since SOLIDWORKS 2018 (rev 26.0). [CreateSpline3](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateSpline3.html).

| Question | Finding | Evidence |
|---|---|---|
| 2D points, off-surface spline | Supply at least two point-coordinate groups, `Surfs=Nothing`, `Direction=Nothing`. Use `Status` as output. | **Documented** 2D mode. **Unverified** if the VBA `PointData` arrangement has any dimensional edge cases. [CreateSpline3](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateSpline3.html). |
| Interpolation | Method constructs a spline from supplied points. Whether each point becomes a fit point within a specific tolerance is **Unverified** here. | Probe each input point's distance to the resulting spline after rebuild. |
| Closed spline | No fully verified rule about repeating first point versus an explicit closure setting. | **Unverified.** Compare both cases and inspect topology. |
| Fit-point readback | A spline's parameter/curve-data accessors can be read from `ISketchSpline`, including `GetSplineParams4`. | **Documented** example; **Unverified** which returned points correspond to original fit points. [2026 parameter example](https://help.solidworks.com/2026/English/api/sldworksapi/Get_Spline%27s_Parameters_Example_VB.htm). |
| `CreateSplineByEqnParams(Parameters As Variant)` | A separate API builds a parametric spline from a structured array; do **not** assume `PointData` layout applies. | **Documented** API existence; full knot/control/weight record layout **Unverified** in this report. [Method](https://help.solidworks.com/2023/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateSplineByEqnParams.html). |

### Ellipses and elliptical arcs

```vb
' ISketchManager
Function CreateEllipse( _
    XC As Double, YC As Double, ZC As Double, _
    XMajor As Double, YMajor As Double, ZMajor As Double, _
    XMinor As Double, YMinor As Double, ZMinor As Double _
) As SketchSegment

Function CreateEllipticalArc( _
    XC As Double, YC As Double, ZC As Double, _
    XMajor As Double, YMajor As Double, ZMajor As Double, _
    XMinor As Double, YMinor As Double, ZMinor As Double, _
    X1 As Double, Y1 As Double, Z1 As Double, _
    X2 As Double, Y2 As Double, Z2 As Double, _
    Direction As Short _
) As SketchSegment
```

**Documented:** Ellipse arguments specify the centre and points on the major/minor axis, not independent axis direction vectors or a single radius. Elliptical arcs add start/end coordinates and direction. [CreateEllipse](https://help.solidworks.com/2025/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateEllipse.html), [CreateEllipticalArc](https://help.solidworks.com/2022/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateEllipticalArc.html).

**Unverified:** How the full ellipse and bounded elliptical arc read back exact major/minor points after dimensional edits. Record `SketchSegment.GetCurve`, geometry type, centre, major axis, minor axis and endpoint parameters.

### Sketch relations

**Documented:** `IModelDoc2.SketchAddConstraints(IdStr As String)` adds a relation to selected sketch entities. Relation identifiers used in official examples include `sgTANGENT`, `sgCONCENTRIC`, `sgCOINCIDENT`, `sgCORADIAL`, `sgFIXED` and others. It is a method call using a relation-name string, not a generic method named `AddConstraints`. [SketchAddConstraints](https://help.solidworks.com/2016/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDoc2~SketchAddConstraints.html).

**Unverified:** The complete matrix of accepted conic/spline/arc entity combinations and required counts for tangent, coincidence, concentric, pierce and symmetry relations. Build one miniature sketch per relation. Capture exact selected-object count/types and relation creation, then inspect `ISketchRelation` after reconstruction. Avoid generic relation application across dissimilar curve types.

---

## T7. Sweeps on SOLIDWORKS 2026

### Current non-obsolete creation path

**Documented:** The supported builder is the `ISweepFeatureData` feature definition. Obtain the definition using `IFeatureManager.CreateDefinition` with the sweep-feature type, set profile, path and supported options, then call `IFeatureManager.CreateFeature`. It supports swept boss, cut and surface workflows. [Sweep feature guide](https://help.solidworks.com/2021/English/api/sldworksapiprogguide/Overview/Sweep_Features_and_SweepFeatureData_Objects.htm), [ISweepFeatureData interface](https://help.solidworks.com/2022/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISweepFeatureData.html).

```vb
' Outline only: this is not a copy-ready VBA macro.
Set data = model.FeatureManager.CreateDefinition(swFmSweep)
' Assign supported profile/path, guide curves, twist and alignment options.
' Keep ThinFeature = False for an ordinary solid sweep.
Set feat = model.FeatureManager.CreateFeature(data)
```

| Builder member | Accepted role | Evidence |
|---|---|---|
| `ISweepFeatureData.Profile` | A sketch profile or supported profile geometry (other options for cut). | **Documented.** [Profile](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISweepFeatureData~Profile.html). |
| `.Path` | Path from sketch geometry, a curve, or model edge geometry. | **Documented** supported categories; profile/path intersection cases **Unverified**. [Path](https://help.solidworks.com/2024/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISweepFeatureData~Path.html). |
| `.GuideCurves` | An array of supported guide-curve references. For a sketch profile, each guide must coincide with the profile or a point in the profile sketch. Guides are not used for bidirectional sweeps. | **Documented.** [Sweep SDK guide](https://help.solidworks.com/2019/English/api/sldworksapiprogguide/Overview/Sweep_Features_and_SweepFeatureData_Objects.htm?format=P&value=), [GuideCurves](https://help.solidworks.com/2021/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISweepFeatureData~GuideCurves.html). |
| `.TwistControlType` | Twist strategy (`swTwistControlType_e`). | **Documented.** [Twist enum](https://help.solidworks.com/2025/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swTwistControlType_e.html?id=bfb09829e1e64bb0b574bd909594f337). |
| `.StartTangencyType`, `.EndTangencyType` | End-orientation settings; availability depends on sweep type. | **Documented.** [StartTangencyType](https://help.solidworks.com/2026/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISweepFeatureData~StartTangencyType.html). |
| `.PathAlignmentType` | Alignment control; pairs with twist and direction-vector options. | **Documented.** [Sweep feature guide](https://help.solidworks.com/2021/English/api/sldworksapiprogguide/Overview/Sweep_Features_and_SweepFeatureData_Objects.htm). |
| `.ThinFeature`, `.ThinWallType`, `.SetWallThickness` | Thin-walled sweep setup. | **Documented.** Leave thin mode false for the default solid profile. [Sweep feature guide](https://help.solidworks.com/2021/English/api/sldworksapiprogguide/Overview/Sweep_Features_and_SweepFeatureData_Objects.htm). |
| Tangency/smoothing settings | Match tangency and guide interpolation where the selected profile/path type permits. | **Documented** property families; effect on this replay's geometry **Unverified**. [ISweepFeatureData members](https://help.solidworks.com/2026/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISweepFeatureData_members.html?id=2.2.1.419.0). |

### Old method compatibility

The `IFeatureManager.InsertProtrusionSwept3` API was superseded by `InsertProtrusionSwept4`. By 2018/2019, `InsertProtrusionSwept4` was also marked obsolete. Older `IModelDoc2.InsertProtrusionSwept4` is a different **11-argument** method and should not be confused with the feature manager's method of the same name. [2019 IFeatureManager members](https://help.solidworks.com/2019/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager_members.html), [2026 IModelDoc2 method](https://help.solidworks.com/2026/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDoc2~InsertProtrusionSwept4.html).

For reference, the feature manager's older `InsertProtrusionSwept3` has the following positional form. **Documented but obsolete; do not use as the 2026 default:** [InsertProtrusionSwept3](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertProtrusionSwept3.html).

```vb
Function InsertProtrusionSwept3( _
    Propagate As Boolean, Alignment As Boolean, TwistCtrlOption As Short, _
    KeepTangency As Boolean, BAdvancedSmoothing As Boolean, _
    StartMatchingType As Short, EndMatchingType As Short, _
    IsThinBody As Boolean, Thickness1 As Double, Thickness2 As Double, _
    ThinType As Short, PathAlign As Short, Merge As Boolean, _
    UseFeatScope As Boolean, UseAutoSelect As Boolean, _
    TwistAngle As Double, BMergeSmoothFaces As Boolean _
) As Feature
```

| Enum/option | Confirmed member values or issue | Evidence |
|---|---|---|
| `swTwistControlFollowPath` | `0` | **Documented.** [swTwistControlType_e](https://help.solidworks.com/2025/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swTwistControlType_e.html?id=bfb09829e1e64bb0b574bd909594f337). |
| `swTwistControlKeepNormalConstant` | `1` | **Documented.** [enum](https://help.solidworks.com/2025/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swTwistControlType_e.html?id=bfb09829e1e64bb0b574bd909594f337). |
| `swTwistControlFollowPathFirstGuideCurve` | `2` | **Documented.** [enum](https://help.solidworks.com/2025/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swTwistControlType_e.html?id=bfb09829e1e64bb0b574bd909594f337). |
| `swTwistControlFollowFirstSecondGuideCurves` | `3` | **Documented.** [enum](https://help.solidworks.com/2025/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swTwistControlType_e.html?id=bfb09829e1e64bb0b574bd909594f337). |
| `swTwistControlConstantTwistAlongPath` | `8` | **Documented.** [enum](https://help.solidworks.com/2025/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swTwistControlType_e.html?id=bfb09829e1e64bb0b574bd909594f337). |
| `swTwistControlNormalConstantTwistAlongPath` | `9` | **Documented.** [enum](https://help.solidworks.com/2025/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swTwistControlType_e.html?id=bfb09829e1e64bb0b574bd909594f337). |
| `swTangencyType_e` | Includes `swTangencyNone=0`, `swTangencyNormalToProfile=1`, `swTangencyDirectionVector=2`, `swTangencyAllFaces=3`, `swTangencyMinimumTwist=10`; allowed members depend on property. | **Documented.** [swTangencyType_e](https://help.solidworks.com/2026/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swTangencyType_e.html). |
| `StartMatchingType` / `EndMatchingType` / `PathAlign` in obsolete method | Correspond to older alignment/tangency enums. Do **not** assume these map one-to-one to current feature-data properties. | **Documented** old arguments; **Unverified** crosswalk. [Old method](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertProtrusionSwept3.html). |
| Guide must meet profile | Each guide curve must coincide with the profile or a point on the profile sketch. Guides cannot be used for bidirectional sweeps. | **Documented.** [Sweep programming guide](https://help.solidworks.com/2019/English/api/sldworksapiprogguide/Overview/Sweep_Features_and_SweepFeatureData_Objects.htm?format=P&value=). The acceptance of specific 2D/3D/composite types remains **Unverified**. |
| `KeepTangency` and advanced smoothing | Older arguments affect tangency continuity and surface smoothing. The current data model exposes different controls. | **Documented** old argument names; **Unverified** equivalence in 2026. [Old method](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertProtrusionSwept3.html). |

**Documented sweep selection marks:** Sketch-profile sweep selects the profile with `1`, path with `4`, guide curves with `2`, and optional direction vector with `128`. A swept boss needs a closed sketch profile. A one-direction sweep path must start on the profile plane. [Sweep SDK guide](https://help.solidworks.com/2019/English/api/sldworksapiprogguide/Overview/Sweep_Features_and_SweepFeatureData_Objects.htm?format=P&value=).

**Minimal sweep probe:** Sweep a circular sketch along a 2D line/arc path and then a 3D sketch path. Add (a) an intersecting guide sketch, (b) a nonintersecting guide, (c) a model edge, (d) a composite curve. For each case, log whether the setter accepts the object, whether `CreateFeature` returns a feature, and whether the body exists after rebuild. Repeat with normal-constant and follow-path twist.

---

## T8. Macro execution, rebuild and modal dialogs

| API/issue | Exact form or semantics | Evidence |
|---|---|---|
| `ISldWorks.RunMacro` | `RunMacro(FilePathName As String, ModuleName As String, ProcedureName As String) As Boolean` | **Documented, obsolete.** [RunMacro](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISldWorks~RunMacro.html). |
| `ISldWorks.RunMacro2` | `RunMacro2(FilePathName As String, ModuleName As String, ProcedureName As String, Options As Long, ByRef Error As Long) As Boolean` | **Documented.** Use in preference to obsolete `RunMacro`. [RunMacro2](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISldWorks~RunMacro2.html). |
| Run options | `swRunMacroDefault=0` and `swRunMacroUnloadAfterRun=1`. | **Documented.** [swRunMacroOption_e](https://help.solidworks.com/2026/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swRunMacroOption_e.html). |
| Run errors | `Error` is a writable `Long` output for the macro-run error enumeration. | **Documented.** [RunMacro2](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISldWorks~RunMacro2.html). |
| Common VBA type mismatch | The `Options` variable and `Error` output must be `Long`. The path/module/procedure args must be `String`. Passing a wrong ByRef type may fail before invocation. | **Documented** parameter types, **Inferred** COM/VBA failure mechanism. `Dim errCode As Long` explicitly. [RunMacro2](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISldWorks~RunMacro2.html). |
| `IModelDoc2.EditRebuild3` | `EditRebuild3() As Boolean` requests rebuild of features needing rebuild in active configuration. | **Documented.** [EditRebuild3](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imodeldoc2~editrebuild3.html). A `True` result does not prove every feature has the intended shape. |
| `IModelDoc2.ForceRebuild3` | `ForceRebuild3(TopOnly As Boolean) As Boolean` forces a rebuild. | **Documented.** [ForceRebuild3](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imodeldoc2~forcerebuild3.html). |
| `IModelDocExtension.GetLastFeatureAdded` | Returns most recently created feature. May return a subfeature rather than the intended parent in certain contexts. | **Documented.** [GetLastFeatureAdded](https://help.solidworks.com/2023/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~GetLastFeatureAdded.html). Capture immediately and validate `GetTypeName2`. |
| Modal dialog blocking | No confirmed official page in this research guarantees that an interactive VBA macro continues while a modal input/save dialog is open. | **Unverified.** Treat any modal dialog as potentially blocking the automation. Probe with an external watchdog. |
| Headless invocation | `RunMacro2` invokes a macro through SOLIDWORKS; it does not by itself provide a documented unattended/headless service contract. | **Inferred** from desktop API operation. Avoid claiming reliable headless execution based only on method availability. |

### Safe execution rule

Run one test document per probe. Save as a new file. Log the macro entry, each feature creation, `RunMacro2` Boolean, `Error` code, `EditRebuild3` Boolean, body count, and feature state. A controller should detect a stalled SOLIDWORKS process rather than wait forever for a modal dialog. Log whether `GetLastFeatureAdded` was captured immediately or after a rebuild.

---

## Prioritized unknowns and minimal SOLIDWORKS 2026 seat probes

| Priority | Unknown | Status | Minimal probe observation | Implementation rule until settled |
|---|---|---|---|---|
| **P0** | `Add2` success/failure return in 2026 | **Unverified; documentation conflicts** | Append two valid equations and one malformed equation; record return, `GetCount`, `Status`, `Equation` and `Value`. | Accept only verified postconditions, not the return code alone. |
| **P0** | Plane dimension name and use in equations | **Unverified** | Create offset plane; enumerate its display dimensions; paste full name verbatim into equation and rebuild. | Discover name at runtime, never construct `D1@Plane1` blindly. |
| **P0** | Offset side controlled by `OptionFlip` and `OptionReferenceFlip` | **Unverified** | Compare transform origins with `8`, `8|256`, `8|8192` and negative distance. | Determine side from measured plane transform. |
| **P0** | Conic signature / rho / apex layout | **Unverified** | Read 2026 Object Browser signature; create one known conic; compare curve samples to analytic geometry. | Disable `CreateConic` generation until probe passes. |
| **P0** | Move/copy translation versus rotation priority | **Unverified; documentation conflicts** | Run same asymmetrical block with translation-only, rotation-only and both in one call. | Use separate transform features. |
| **P0** | Combine cut preserves or consumes tool bodies | **Unverified** | Subtract two overlapping cubes; inspect bodies before/after, save/reopen. | Copy needed tools before combine. |
| **P0** | Specific guide-curve object type acceptance for swept boss | **Unverified; intersection is documented** | Use profile-coincident 2D and 3D guide sketches, model edges and composite curves; inspect feature and body. | Require profile coincidence and restrict guides to tested object types. |
| **P0** | Modal dialog blocks `RunMacro2` | **Unverified** | In disposable test file deliberately show save/input prompt; record controller responsiveness and error. | Avoid modal UI and use watchdog. |
| **P1** | `InsertRefPlane` combination table and unused slots | **Unverified** | Vary plane/face/axis/line references with Distance, MidPlane, Angle and Tangent. | Whitelist successful combinations. |
| **P1** | Global-variable propagation after one rebuild | **Unverified** | Change variable, compare `.Value`, plane transform and dependent geometry before/after rebuild. | Rebuild and verify measured geometry. |
| **P1** | `Delete` status on 2026 | **Unverified; documentation differs** | Delete equation index, compare count and remaining text. | Treat changed list as success, not raw integer alone. |
| **P1** | Direct `ToolVar` body array versus selection marks | **Unverified** | Same combine using direct body arguments, then selected marked body objects. | Use only route verified in the 2026 VBA host. |
| **P1** | Downstream features after body deletion | **Unverified** | Delete body referenced by subsequent fillet; inspect feature state. | Prevent deletion of source bodies still referenced. |
| **P1** | Body naming and name-based selection syntax | **Unverified** | Create two bodies, rename, inspect `Name`, try `SelectByID2` and compare selected objects. | Prefer `IBody2.Select2` object selection. |
| **P1** | `TransDist` / mark 4 and 8 behavior; rotation units | **Unverified** | Apply one known directional move and 90-degree rotation; compare vertex coordinates. | Validate actual transform via geometry. |
| **P1** | Edge persistent reference across topology edits | **Unverified** | Save reference, rebuild with dimension change, save/reopen, then add fillet/split; resolve ID. | Match by geometry if persistent reference fails. |
| **P1** | Fillet radius display-dimension name | **Unverified** | Create one fillet, enumerate dimensions, compare radius and feature name. | Resolve dimension from returned feature. |
| **P1** | Spline interpolation and closed-spline inputs | **Unverified** | Create 2D spline with three points, measure fit residual; repeat with duplicate start/end. | Do not assume closed or through-point behavior. |
| **P1** | Arc and elliptical arc dimension/relations | **Unverified** | Use each relation with recorded selected objects and compare generated relation object. | Use a relation-specific selection recipe. |
| **P1** | `CreateSplineByEqnParams` flattened array layout | **Unverified** | Capture a minimal 2026 official example or use Object Browser, then read back knots/weights. | Do not emit guessed knot arrays. |
| **P1** | `GetLastFeatureAdded` parent versus subfeature | **Documented caveat; seat scenario unverified** | Log immediate feature type after plane, fillet and sweep creation. | Validate against expected feature type and tree location. |
| **P2** | Body versus document mass-properties agreement | **Unverified** | Compute same cube volume, mass and inertia with same density and selection. | Specify density and source-body set explicitly. |
| **P2** | Full 2026 sweep enum/property compatibility matrix | **Unverified** | Cycle documented twist/tangency options with selected profile and guide. | Use only proven property combinations. |

### Suggested probe order

1. **Parameter backbone:** `Add2` return, create offset plane, read dimension name, change global variable, rebuild, compare plane transforms.
2. **Solid operations:** Cut boss, combine subtract/intersect, delete body, move/copy isolated translation and rotation.
3. **Topology:** Enumerate edge descriptors, verify selections, build a fillet, save/reopen persistent reference.
4. **Sketch and sweep:** Conic signature and shape, spline variants, arc/ellipse relations, simple swept boss, guide curves.
5. **Replay harness:** `RunMacro2`, rebuild status, last-feature identity and modal-dialog watchdog in a disposable model.

**Acceptance criterion:** A probe passes only when the macro call, returned COM objects, feature tree, saved/reopened document, and quantitative geometry all agree with the intended construction. A non-`Nothing` feature or a `True` Boolean alone is insufficient.

## Source and verification boundary

All linked sources are public SOLIDWORKS API documentation pages or official SDK/programming-guide material. This report uses original summaries and API identifiers; it reproduces no documentation prose. Historical pages identify version-specific conflicts. Some current pages expose incomplete text through the public web interface, which is why the conic signature and several acceptance matrices remain **Unverified**. The report did **not** execute SOLIDWORKS 2026, inspect the actual type library, or run the listed seat probes.
