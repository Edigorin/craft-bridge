# SOLIDWORKS API usage archaeology 07

**Filed:** 2026-11-18, produced by the owner's research agent from public
sources (research date recorded in the report: 2026-11-17), delivered as a
chat attachment and filed verbatim. Answer to
`research/inputs/07-solidworks-usage-archaeology.md`.
Provenance note: this report's P0 seat-probe items are queue items for the
compat-check probes (docs/source-probe-evidence.md records what has already
run on the seat). Nothing in this report is seat-measured; the labels
Recorded/Community/Conflicting/Reconstructed are the report's own.

---

# Research 07: SOLIDWORKS API usage archaeology

**Research date:** 2026-11-17  
**Purpose:** Recover real call shapes for the craft-bridge SOLIDWORKS replay program.  
**Scope:** T1-T7 from `07-solidworks-usage-archaeology.md`.  
**Execution status:** Public-source research only. No licensed SOLIDWORKS seat was used.

## Evidence rules

- **Recorded**: A call in a SOLIDWORKS API Help example or official SDK example. This label does not assert that the example came from the interactive macro recorder.
- **Community**: A call in a public third-party repository. It confirms that an author used the form, not that the call succeeded.
- **Conflicting**: Source statements or call forms disagree. Both are retained.
- **Reconstructed / untested**: A candidate assembled from documented types or separate examples. Do not deploy it without a seat probe.

The quoted code is limited to relevant call sequences. Explanatory text is original. Examples often depend on a particular document, feature names, and selection state. Units refer to SOLIDWORKS internal units unless stated otherwise. SOLIDWORKS is a Dassault Systemes trademark; its name is used for identification only.

## Findings at a glance

| Area | Result | Confidence | Remaining action |
|---|---|---|---|
| T1 `CreateConic` | **12 doubles: focus XYZ, apex XYZ, start XYZ, end XYZ**. No separate rho input. Supports circle, ellipse, parabola, hyperbola. | High, official example and mirrored reference | Verify parameter geometry and conversion from source conic representation |
| T1 `CreateConicByEqnParams` | No SOLIDWORKS API reference entry or real call recovered from the searched sources. | Unresolved | Query the installed 2026 type library / VBA Object Browser |
| T2 `CreateSplineByEqnParams` | OLE packed layout is four properties, `N + order` knots, then `N * dimension` control-point values. | High for documented layout; untested call | Execute a known spline with the packed array |
| T2 `CreateSpline3` | Five parameters, with flat XYZ coordinate data, surface/direction arrays or `Nothing`, natural-end flag, output status. | High for shape | Verify 2D XYZ/Z handling and COM output variant |
| T3 fillet builder | `CreateDefinition(swFmFillet)`, `Initialize(swConstRadiusFillet)`, edge mark 1, profile/overflow/radius properties, `CreateFeature`. | High for component steps | Test constant-radius assembly and resulting driving dimensions |
| T4 move/copy | Twelve positional inputs; body mark 1; reference marks 2/4/8. **2018 and 2022 pages disagree on translation/rotation precedence.** | High for signature; contradictory for precedence | Execute combined-input test, TransDist test, angle-order test |
| T5 combine | Direct Body2/array route and selected-body route exist. Official selection example passes `Nothing, Nothing` for add. | High for selected-body call | Verify VBA `Array(Body2...)` marshaling and subtract semantics |
| T6 selection names | Real patterns recovered for sketch entities, faces/edges/vertices, solid bodies, planes, dimensions. | High | Test stability across rename, reorder, rebuild |
| T7 ref plane | `GetDefinition.Type2` is read in official example. `Distance` is accessed in a separate official example. | High | Verify modern constraint-based offset readback |
| T7 sweep | Official full builder uses profile mark 1, path 4, guide 2. Readback uses `GuideCurves` after `AccessSelections`. | High for creation/readback | Determine whether setter accepts sketch features, segments, or edges |

---

## T1. `ISketchManager.CreateConic` and `CreateConicByEqnParams`

### T1.1 Real `CreateConic` call

**Recorded: official SOLIDWORKS VBA example.**

```vb
Dim segment As SldWorks.SketchSegment
Set segment = Part.SketchManager.CreateConic( _
    0.109436, 0.080614, 0#, _
    0.048742, 0.022907, 0#, _
   -0.017812, 0#, 0#, _
   -0.006092, -0.069601, 0#)
Part.SketchManager.InsertSketch True
```

Source: [Insert Conic Curve Example (VBA), SOLIDWORKS API Help 2024](https://help.solidworks.com/2024/english/api/sldworksapi/insert_conic_curve_example_vb.htm). The [C# example, API Help 2020](https://help.solidworks.com/2020/English/api/sldworksapi/Insert_Conic_Curve_Example_CSharp.htm) uses the same ordered numeric values.

**Reference recovered from an archival mirror of the SOLIDWORKS API Help** (not inferred from the numeric sample):

```csharp
SketchSegment CreateConic(
    double XFocus, double YFocus, double ZFocus,
    double XApex,  double YApex,  double ZApex,
    double X1,     double Y1,     double Z1,
    double X2,     double Y2,     double Z2);
```

Source: [mirrored ISketchManager.CreateConic API page](https://github.com/streetartist/solidworks-automation/blob/3513f70e0c30fe3db8f15641b8d350ccdb7367f1/docs/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~CreateConic.md). An independently structured [API reference mirror](https://github.com/kilwizac/solidworks-api-mcp/blob/7885946e1ee8783128f2d9a01d9a97349a2fddb0/solidworks-api/sldworksapi/interfaces/ISketchManager/CreateConic.md) has the same names and order.

Parameter groups, in the exact order:

| Positions | Names | Meaning |
|---|---|---|
| 1-3 | `XFocus,YFocus,ZFocus` | Focus of the conic |
| 4-6 | `XApex,YApex,ZApex` | Apex or vertex of the conic |
| 7-9 | `X1,Y1,Z1` | Start point of the finite sketch conic |
| 10-12 | `X2,Y2,Z2` | End point of the finite sketch conic |

The reference lists **circle, ellipse, parabola, and hyperbola** as possible curve classes. The call returns a `SketchSegment` and works in a 2D sketch. It can create a sketch if none is active. For a generated design, prefer opening and controlling the sketch explicitly. The method uses point coordinates, not an input array. [Reference](https://github.com/kilwizac/solidworks-api-mcp/blob/7885946e1ee8783128f2d9a01d9a97349a2fddb0/solidworks-api/sldworksapi/interfaces/ISketchManager/CreateConic.md).

**Important for the 854-conic replay blocker:** `CreateConic` has **no rho argument**. The source design's conic constant must be converted into valid focus/apex/end-point geometry, if such a conversion is needed. Do not append a 13th argument. Do not assume that a source rho of `0.5` maps directly to any `CreateConic` input.

The SOLIDWORKS user-interface conic command lets a user choose two endpoints, an apex, and a rho value. That interface workflow is **not** the `CreateConic` COM signature. See [Sketching Conics, SOLIDWORKS Help 2024](https://help.solidworks.com/2024/English/SolidWorks/sldworks/t_Sketching_Conics.htm).

### T1.2 `CreateConicByEqnParams`

**Unresolved, no recovered usage.** Searches of official API examples, method names, and indexed GitHub code did not yield a SOLIDWORKS `ISketchManager.CreateConicByEqnParams` entry or call. The two indexed API-reference mirrors contain `CreateConic`, but the exact `CreateConicByEqnParams` path was absent in the checked mirror. This is **not proof that the installed COM type library lacks the method**.

There is consequently **no verified parameter-array record layout** for that name. Do not confuse it with `CreateSplineByEqnParams`, which is a different documented method.

**Seat probe:** In the VBA Object Browser, search `Conic`, `CreateConic`, and `CreateConicByEqnParams` across `SldWorks` type libraries. If the latter exists, capture the declaration. If it does not, use `CreateConic` or a controlled approximation policy. For `CreateConic`, measure the resulting curve class, endpoints, focus, and apex against the input values, then test one example of each supported conic class.

**Rho distinction:** The documented range `0.05...0.95` for `ISimpleFilletFeatureData2.DefaultConicRhoOrRadius` belongs to a **fillet cross-section**. It is not evidence about sketch `CreateConic` or its shape conversion. [Fillet feature-data reference](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2.html).

---

## T2. `CreateSplineByEqnParams` and `CreateSpline3`

### T2.1 Packed B-spline input

**Reference-confirmed layout; call below is reconstructed and untested.**

```vb
' Example candidate: degree 2, order 3, three unweighted 2D controls.
' Header (4) + knots (3 + 3 = 6) + controls (3 * 2 = 6).
Dim p As Variant
Dim seg As SldWorks.SketchSegment
p = Array( _
    2#, 3#, 3#, 0#, _
    0#, 0#, 0#, 1#, 1#, 1#, _
    0#, 0#, 0.05, 0.05, 0.1, 0#)
Set seg = swModel.SketchManager.CreateSplineByEqnParams(p)
```

Call signature: `SketchSegment CreateSplineByEqnParams(object Parameters)`. The older `IModelDoc2.CreateSplineByEqnParams` page specifies that the three logical data arrays are packed into **one OLE SafeArray**, in this order: `[Dimension, Order, ControlPointCount, Periodic, knots..., control coordinates...]`. The current `ISketchManager` page specifies the same three logical groups but does not explicitly explain the flattening. Read them together. Sources: [IModelDoc2 version, API Help 2020](https://help.solidworks.com/2020/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDoc2~CreateSplineByEqnParams.html), [ISketchManager version, API Help 2023](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~CreateSplineByEqnParams.html).

| Slot/group | Type in OLE array | Rule |
|---|---|---|
| `Dimension` | Integer-valued double | `2` = XY, `3` = XYZ, `4` = XYZW rational control points |
| `Order` | Integer-valued double | B-spline order (mathematically degree + 1) |
| `ControlPointCount` | Integer-valued double | `N` control points |
| `Periodic` | Integer-valued double | `0` false, nonzero true |
| Knots | Double sequence | Exactly `N + Order` entries |
| Control points | Double sequence | Exactly `N * Dimension` entries, ordered by point |

An unweighted 3D point contributes `(x,y,z)`. A rational point contributes `(x,y,z,w)`, where `w` is its weight. Coordinates are in **sketch space**. A closed spline must be marked periodic. The help also requires G1 continuity; it rejects a spline that violates it. The API creates geometry directly and may require a graphics redraw. [ISketchManager reference](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~CreateSplineByEqnParams.html).

**Practical limit:** No public, runnable `ISketchManager.CreateSplineByEqnParams(Array(...))` invocation was recovered in the searched official examples or indexed repositories. The code above is a layout test, **not a claimed successful macro**. The VBA `Variant` to `SAFEARRAY(double)` conversion is still a seat-probe item. A typed `Double()` vector may be required.

**Recorded alternative with stronger implementation evidence:** `ISplineParamData` avoids hand-packing an array. The official sample sets control points and knots, then calls `ISketchManager.CreateSplinesByEqnParams2`:

```vb
boolStatus = swSplineParaData.SetKnotPoints(arrKnotPts)
varSeg = swSketchMgr.CreateSplinesByEqnParams2(swSplineParaData)
```

It also reads `Dimension`, `Order`, `Periodic`, `ControlPointsCount`, and the returned knot/control arrays. This is the preferred first replay probe if an exact B-spline is required. Source: [Get Spline's Parameters Example (VBA), API Help 2023](https://help.solidworks.com/2023/English/api/sldworksapi/Get_Spline%27s_Parameters_Example_VB.htm). `CreateSplinesByEqnParams2` is documented from SOLIDWORKS 2011 FCS, revision 19.0. [Method reference](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~CreateSplinesByEqnParams2.html).

### T2.2 `CreateSpline3`

**Community usage; official 2018+ signature.**

```csharp
// Community caller: ptArray is a flat double[] of x,y,z groups.
SwObject = skeMgr.CreateSpline3(ptArray, null, null, true,
                                out var status) as ISketchSegment;
```

Source: [DynamoSolidWorks, `SketchSpline.cs`, pinned commit](https://github.com/weianweigan/DynamoSolidWorks/blob/90550bbe167be15b02ed215dcacbe44d2131aabe/SldWorksNodes/Sketch/SketchSpline.cs). **Code-quality warning:** That repository's `ptArray` loop copies `X` into all three slots. Its API invocation establishes the array type and five-argument call pattern, but the point packing implementation is incorrect. Do **not** copy its coordinate loop.

Another third-party caller uses `manager.CreateSpline3(point_data, None, None, False, status)`, where `status` is a by-reference `VARIANT`. It falls back to `CreateSpline2` on failure. [CAD-Agent-Hub, pinned source](https://github.com/Cai-aa/CAD-Agent-Hub/blob/112786ab0c79100f3f2417eb8ae9c3c2dfaf2daf/MCP/Solidworks/src/solidworks_mcp/native_features.py).

Official signature:

```csharp
object CreateSpline3(object PointData, object Surfs, object Direction,
                     bool SimulateNaturalEnds, out object Status);
```

- `PointData`: flat `(x,y,z)` triples, with at least two points.
- `Surfs`: array of `ISurface` objects for on-surface splines; `null`/`Nothing` for ordinary 2D splines.
- `Direction`: `IMathVector` array for projecting on-surface points; otherwise `null`/`Nothing`.
- `SimulateNaturalEnds`: Boolean used for 2D splines.
- `Status`: output array of per-point projection Booleans for on-surface splines; empty for ordinary 2D splines.

**Conflicting wording inside one official page:** It shows a six-number `(x,y,z,x,y,z)` example for 2D spline points, but also says SOLIDWORKS assumes no Z values for 2D sketches. The actual 2D COM marshaling and Z interpretation deserve a small probe. The documented introduction is **SOLIDWORKS 2018 FCS, revision 26.0**. [CreateSpline3 method, API Help 2023](https://help.solidworks.com/2023/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~CreateSpline3.html).

No `CreateSpline3` argument is a `SketchPoint` object array. A point-object array is therefore not the documented primary call form.

---

## T3. `ISimpleFilletFeatureData2` builder

### T3.1 Official builder example: face fillet

**Recorded: official VBA sample, shortened to the core statements.** The two `SelectByRay` calls are represented as already-completed face selections here.

```vb
Set dataObj = swFeatMgr.CreateDefinition(swFmFillet)
Set filletData = dataObj
filletData.Initialize swFaceFillet
' Official sample selects face set 1 with mark 2 and face set 2 with mark 4.
filletData.AsymmetricFillet = True
filletData.DefaultRadius = 0.01
filletData.DefaultDistance = 0.02
filletData.ConicTypeForCrossSectionProfile = swFeatureFilletCircular
Set feat = swFeatMgr.CreateFeature(filletData)
```

Source: [ISimpleFilletFeatureData2 interface and embedded VBA example, API Help 2022](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2.html). The source includes the actual ray coordinates and marks.

### T3.2 Constant-radius edge fillet: assembled builder

**Reconstructed / untested as one complete macro.** Every property and selection mark below has an official reference. Replace the edge coordinates with a known point on a real edge.

```vb
Dim data As SldWorks.SimpleFilletFeatureData2
Dim feat As SldWorks.Feature
Dim ok As Boolean

swModel.ClearSelection2 True
ok = swModel.Extension.SelectByID2( _
    "", "EDGE", 0.01, 0.02, 0.03, False, 1, Nothing, 0)
If Not ok Then Err.Raise 5, , "Edge selection failed"

Set data = swModel.FeatureManager.CreateDefinition(swFmFillet)
data.Initialize swConstRadiusFillet
data.ConicTypeForCrossSectionProfile = swFeatureFilletCircular
data.OverflowType = swFilletOverFlowType_Default
data.AsymmetricFillet = False
data.IsMultipleRadius = False
data.CurvatureContinuous = False
data.DefaultRadius = 0.005  ' 5 mm, in internal meters
' Optional: data.PropagateToTangentFaces = True
' Optional: data.KeepFeatures = True
' Optional: data.RoundCorners = True
Set feat = swModel.FeatureManager.CreateFeature(data)
If feat Is Nothing Then Err.Raise 5, , "Fillet creation failed"
```

Sources: [official builder instructions and option restrictions](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2.html), [overflow enum: default 0, keep edge 1, keep surface 2](https://help.solidworks.com/2023/english/api/swconst/SOLIDWORKS.Interop.swconst~SOLIDWORKS.Interop.swconst.swFilletOverFlowType_e.html), [interface members](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2_members.html).

Members and roles:

| Member | Meaning and constraints |
|---|---|
| `Initialize(swConstRadiusFillet)` | Constant-radius edge/face/feature fillet type. Not `swFaceFillet`. |
| `ConicTypeForCrossSectionProfile` | `swFeatureFilletCircular`, `swFeatureFilletConicRho`, or `swFeatureFilletConicRadius` for supported constant-radius modes. |
| `OverflowType` | `swFilletOverFlowType_Default`, `KeepEdge`, or `KeepSurface`. |
| `DefaultRadius` | Default fillet radius; the 0.01 example corresponds to 10 mm. |
| `AsymmetricFillet`, `DefaultDistance` | Second size used in an asymmetric fillet. |
| `PropagateToTangentFaces`, `KeepFeatures` | Optional tangent and retained-feature behavior. |
| `RoundCorners`, `ConstantWidth` | Optional mode flags, subject to feature type. |
| `CurvatureContinuous` | If true, the circular/conic cross-sectional profile setting and `RoundCorners` cannot be used together with it. |
| `IsMultipleRadius`, `SetRadius(item, radius)` | Multi-radius edge settings; `SetRadius` is the documented method name. |
| `DefaultConicRhoOrRadius` | Fillet-specific rho, supported from 0.05 to 0.95 for the conic-rho profile, or conic radius in the corresponding mode. |

A `SetNextCornerRadius` method was **not** found in the listed `ISimpleFilletFeatureData2` members. The API instead exposes `SetRadius`, `SetConicRhoOrRadius`, and setback-specific methods. [Interface members](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISimpleFilletFeatureData2_members.html). The variable-radius interface `IVariableFilletFeatureData2` has additional per-edge-end operations; do not use its member names interchangeably with `ISimpleFilletFeatureData2`.

### T3.3 Radius display dimension

**Recorded name, but not a guaranteed naming rule.** One official example selects:

```vb
ok = swModel.Extension.SelectByID2( _
    "D1@Fillet1@tutor1.SLDPRT", "DIMENSION", _
    0.0127275536990925, 0.0229950387838887, _
    0.0775856501351018, False, 0, Nothing, 0)
```

This proves `D1@Fillet1@<file>` is a real fillet-dimension name in an existing model. It does **not** prove all newly created fillets use `D1` or that all fillets expose one display dimension. Source: [Get Precisions for a Dimension Example (VBA), API Help 2025](https://help.solidworks.com/2025/english/api/sldworksapi/Get_Precisions_for_a_Dimension_Example_VB.htm). The replay program should enumerate the feature's display dimensions and read actual `Dimension.FullName` instead of constructing `D1@FilletN` blindly.

---

## T4. `InsertMoveCopyBody2`: slots, precedence, marks, readback

### T4.1 Official call

**Recorded: official Move Bodies (VBA) example.**

```vb
' After selecting the body or bodies:
Set feat = swModel.FeatureManager.InsertMoveCopyBody2( _
    0.1, 0.2, 0.3, 0#, _
    0#, 0#, 0#, _
    0#, 0#, 0#, _
    False, 1)
```

Source: [Move Bodies Example (VBA), API Help 2023](https://help.solidworks.com/2023/english/api/sldworksapi/move_bodies_example_vb.htm). An [earlier official example](https://help.solidworks.com/2012/english/api/sldworksapi/Move_Bodies_Example_VB.htm) uses the same 12-slot form and includes a separate selection of the origin using mark 8.

Exact signature and slot purpose:

| Position | Input | Meaning |
|---|---|---|
| 1-3 | `TransX`, `TransY`, `TransZ` | Translation deltas along X, Y, Z |
| 4 | `TransDist` | Translation distance parameter; interaction with XYZ and a reference entity remains unverified |
| 5-7 | `RotPointX`, `RotPointY`, `RotPointZ` | Rotation origin position |
| 8-10 | `RotAngleX`, `RotAngleY`, `RotAngleZ` | X, Y, Z rotation angle values; no official Euler composition order was recovered |
| 11 | `BCopy` | `True` creates a copy; `False` moves the body |
| 12 | `NumCopies` | Number of copies to create |

The method requires **12 arguments**. A [third-party Python wrapper](https://github.com/Eduardof0nt/Solidworks-MCP-Server/blob/ba4052006f8427826fad9b508c099be49553cc53/sw_client.py) passes **11**, apparently omitting `TransDist`. That is **Community / Conflicting** and should not be used as a valid signature. [Official method page, API Help 2022](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html).

### T4.2 Confirmed selection marks

**Recorded and reference-confirmed:**

| Entity | Selection mark |
|---|---:|
| Body or surface | 1 |
| Rotation edge, vertex, coordinate system | 2 |
| Translation edge, vertex, coordinate system | 4 |
| Translation destination vertex | 8 |

Example using a body and two translation vertices:

```vb
ok = ext.SelectByID2("Extrude-Thin1", "SOLIDBODY", _
                     0, 0, 0, False, 1, Nothing, 0)
ok = ext.SelectByID2("", "VERTEX", -0.085, 0, 0.065, _
                     True, 4, Nothing, 0)
ok = ext.SelectByID2("", "VERTEX", -0.085, -0.09, 0.065, _
                     True, 8, Nothing, 0)
Set feat = fm.InsertMoveCopyBody2( _
    0, 0, 0, 0, -0.085, 0, 0.065, 0, 0, 0, True, 1)
```

Source: [Modify Move/Copy Body Using Vertex Example (VB.NET), API Help 2023](https://help.solidworks.com/2023/english/api/sldworksapi/Move_and_Copy_Body_Using_Vertex_Example_VBNET.htm). The displayed call is rewritten in VBA assignment form, preserving the original arguments and selection marks.

**Community example of a rotation-axis selection:** The [SolidworksMCP placement implementation](https://github.com/czuryk/SolidworksMCP/blob/875be3d050c4c1c1fc641feaba3ee677d204e16a/solidworks_mcp/automation/shape_placement.py) selects bodies at mark 1, a sketch axis at mark 2, then calls `InsertMoveCopyBody2(0,0,0,0,0,0,0,angle,0,0,False,1)`. It uses separate features for translation and rotation. The success of its geometry is not independently verified here.

### T4.3 Translation versus rotation conflict

**Conflicting official documentation:**

- [2018 method page](https://help.solidworks.com/2018/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html) and [2013 method page](https://help.solidworks.com/2013/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html): if nonzero translation and rotation inputs are both present, only translation is applied.
- [2022 method page](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html): if both are present, only rotation is applied.

The official Move Bodies example uses only translation. It **does not decide this conflict**. A safe replay emits separate move and rotate features until a SW2026 seat probe resolves the precedence. No public test with verified geometry and both input groups nonzero was recovered.

The parameter names suggest three per-axis angle values rather than a conventional `(axis vector, single angle)` signature. However, an exact Euler convention, rotation order, and the interaction with selection mark 2 were **not** established by a tested example. The feature-data properties for rotation use radians. Treat radians as the likely unit here, but verify an input of `pi/2` before relying on it. [Feature-data `TransformX` unit note](https://help.solidworks.com/2018/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMoveCopyBodyFeatureData~TransformX.html).

### T4.4 `IMoveCopyBodyFeatureData` readback and edit

**Recorded official VBA example:**

```vb
Set moveFeat = part.FeatureByName("Body-Move/Copy1")
Set data = moveFeat.GetDefinition
ok = data.AccessSelections(part, Nothing)
data.TransformType = swTransformType_Translation
data.TransformX = 0.02
data.TransformY = 0.03
data.TransformZ = 0.04
ok = moveFeat.ModifyDefinition(data, part, Nothing)
data.ReleaseSelectionAccess
```

Source: [Move and Copy Body by Setting Transforms Example (VBA), API Help 2010](https://help.solidworks.com/2010/english/api/sldworksapi/Move_and_Copy_Body_By_Setting_Transforms_Example_VB.htm). Another [official sample](https://help.solidworks.com/2013/English/api/sldworksapi/Copy_and_Rotate_Body_using_Edge_Example_VB.htm) sets `TransformType = swTransformType_Rotation`, assigns an `Edge` as `TransformReferenceEntity`, and sets `TransformX = 0.09` radians.

Transform types are `swTransformType_None = 0`, `swTransformType_Translation = 1`, `swTransformType_Rotation = 2`. Always set the type before interpreting `TransformX/Y/Z`. In translation mode, transform values are meters; in rotation mode, radians. [Enum](https://help.solidworks.com/2020/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swMoveCopyBodyFeatureTransformType_e.html?id=fa18059dab3648678e0389f1d9222881), [feature-data property](https://help.solidworks.com/2025/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMoveCopyBodyFeatureData~TransformType.html).

---

## T5. `InsertCombineFeature`: VBA `ToolVar` and readback

### T5.1 Real selected-body call

**Recorded: official Combine Bodies (VBA) example.**

```vb
ok = ext.SelectByID2("Extrude-Thin1", "SOLIDBODY", _
                     0, 0, 0, False, 2, Nothing, 0)
ok = ext.SelectByID2("Boss-Extrude1", "SOLIDBODY", _
                     0, 0, 0, True, 2, Nothing, 0)
Set feat = fm.InsertCombineFeature( _
    swBodyOperationType_e.SWBODYADD, Nothing, Nothing)
```

Source: [Combine Bodies Example (VBA), API Help 2023](https://help.solidworks.com/2023/english/api/sldworksapi/Combine_Bodies_Example_VB.htm). Source text also preserved in an [independently hosted copy of the official example](https://github.com/f1owkang/solidworks-plugin-dev/blob/f084611fd6ee85071650cee069071377950f079c/references/source-docs/Other/Combine_Bodies_Example_VB.md).

**Conflicting method-page statement:** The [method reference](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager~InsertCombineFeature.html) says to pass `MainBody = Nothing` and `ToolVar = empty array` when selecting operands in advance. The official working example instead passes **`Nothing, Nothing`**. For an add operation with selection marks, prefer the example's actual call and verify the feature result.

### T5.2 Direct object-array route

**Community C# source shows this exact `ToolVar` shape:**

```csharp
object tools = orderedBodies.Skip(1).Cast<object>().ToArray();
Feature feat = model.FeatureManager.InsertCombineFeature(
    operation, orderedBodies[0], tools);
```

The same source tries several fallbacks, including an all-body array with a null main body, then a marked-selection route. This is evidence of the attempted call forms, not proof that each fallback works. Source: [Xuan-BOMS, pinned `bridge/Program.cs`](https://github.com/Xuan-BOMS/soildworks-mcp/blob/b8fa391432133ac0881e12352a67179d12d57e5f/bridge/Program.cs).

**Reconstructed VBA candidate, untested:**

```vb
Dim tools As Variant
' bodyA and bodyB are actual SldWorks.Body2 object references.
tools = Array(bodyA, bodyB)
Set feat = fm.InsertCombineFeature(SWBODYADD, Nothing, tools)
' For subtract, use MainBody = targetBody, ToolVar = Array(cutterBody).
' For intersect, use MainBody = Nothing, ToolVar = Array(bodyA, bodyB).
```

The method's `ToolVar` is `Object`; it accepts an array of `Body2`. Its `MainBody` rule depends on the operation: `Nothing` for `SWBODYADD`, target body for `SWBODYCUT`, `Nothing` for `SWBODYINTERSECT`. The callable member is `IFeatureManager.InsertCombineFeature(OperationType, MainBody, ToolVar)` and returns `IFeature`. [Official method page](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager~InsertCombineFeature.html).

The separate `IInsertCombineFeature` pointer-array method is for **in-process unmanaged C++** and is not supported for VBA/VB.NET/C#. Do not treat its `ToolsCount` / `ToolsArr` form as a normal macro signature. [IInsertCombineFeature method, API Help 2019](https://help.solidworks.com/2019/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager~IInsertCombineFeature.html).

### T5.3 Feature-data readback

**Recorded: official example.**

```vb
Set data = feat.GetDefinition
ok = data.AccessSelections(swModel, Nothing)
Debug.Print data.OperationType
' Also inspect data.MainBody and data.BodiesToCombine.
data.ReleaseSelectionAccess
```

Source: [Combine Bodies Example (VBA)](https://help.solidworks.com/2023/english/api/sldworksapi/Combine_Bodies_Example_VB.htm), [ICombineBodiesFeatureData members](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ICombineBodiesFeatureData_members.html).

Do not assume the enum used by `ICombineBodiesFeatureData.OperationType` has the same member names or values as the `swBodyOperationType_e` input. The feature-data property uses `swCombineBodiesOperationType_e` in its documentation. Preserve both enum names in code generation and compare their numeric values on the seat.

---

## T6. `SelectByID2`: official selection-name grammar

### T6.1 Recorded call shape

```vb
Dim selected As Boolean
selected = swModel.Extension.SelectByID2( _
    Name, Type, X, Y, Z, Append, Mark, Nothing, 0)
```

Nine arguments: case-sensitive `Name`, uppercase entity `Type`, coordinates `X/Y/Z`, `Append` Boolean, feature-specific selection `Mark`, optional callout, and selection option (commonly `0`). `Append = True` can toggle off an already selected entity; it is **not** a no-op. With a meaningful name, coordinates refer to the creation context, such as sketch space. Without a name, location coordinates are interpreted in model space. [SelectByID2 reference, API Help 2023](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDocExtension~SelectByID2.html).

### T6.2 Recorded patterns

| Entity | Recorded/official name and `Type` | Evidence | Stability |
|---|---|---|---|
| Sketch line, active sketch | `"Line1", "SKETCHSEGMENT"` | [Replace Sketch example](https://help.solidworks.com/2017/english/api/sldworksapi/replace_sketch_example_vb.htm) | Segment numbering can change after edits. |
| Sketch arc, active sketch | `"Arc1", "SKETCHSEGMENT"` | [Replace Sketch example](https://help.solidworks.com/2017/english/api/sldworksapi/replace_sketch_example_vb.htm) | Same caveat. |
| Sketch line, other/inactive sketch | `"Line1@Sketch1", "EXTSKETCHSEGMENT"` | [Get Sketch Slot example](https://help.solidworks.com/2026/English/api/sldworksapi/Get_Sketch_Slot_Using_Sketch_Point_and_Segment_Example_CSharp.htm) | Qualified, but sketch and entity names can change. |
| Sketch point, active sketch | `"Point1", "SKETCHPOINT"` | [SelectByID2 reference](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDocExtension~SelectByID2.html) | Active-sketch-specific. |
| Sketch point, inactive sketch | `"Point1@Sketch1", "EXTSKETCHPOINT"` | [Get Sketch Slot example](https://help.solidworks.com/2026/English/api/sldworksapi/Get_Sketch_Slot_Using_Sketch_Point_and_Segment_Example_CSharp.htm) | Qualified, but can be renamed or recreated. |
| Whole sketch | `"Sketch1", "SKETCH"` | [Official swept-boss example](https://help.solidworks.com/2025/english/api/sldworksapi/Insert_Swept-boss_Feature_Example_VB.htm) | Sketch name is not a permanent geometric identifier. |
| Face | `"", "FACE", x,y,z` | [Reference plane example](https://help.solidworks.com/2019/english/api/sldworksapi/Insert_Reference_Plane_Example_CSharp.htm) | Spatial pick changes when geometry changes. |
| Edge | `"", "EDGE", x,y,z` | [Reference plane example](https://help.solidworks.com/2019/english/api/sldworksapi/Insert_Reference_Plane_Example_CSharp.htm) | Spatial pick is vulnerable to topology changes. |
| Vertex | `"", "VERTEX", x,y,z` | [Move/copy vertex example](https://help.solidworks.com/2023/english/api/sldworksapi/Move_and_Copy_Body_Using_Vertex_Example_VBNET.htm) | Location and topological identity can change. |
| Solid body in multibody part | `"Extrude-Thin1", "SOLIDBODY"` | [Combine Bodies example](https://help.solidworks.com/2023/english/api/sldworksapi/Combine_Bodies_Example_VB.htm) | Body name may be derived from source feature or changed by user. |
| Origin plane | `"Front Plane", "PLANE"` / `"Top Plane", "PLANE"` | [Official swept-boss example](https://help.solidworks.com/2025/english/api/sldworksapi/Insert_Swept-boss_Feature_Example_VB.htm) | Built-in names may be localized or renamed. |
| Reference plane | `"Plane1", "PLANE"` | [Insert Reference Plane example](https://help.solidworks.com/2022/english/api/sldworksapi/Insert_Reference_Plane_Example_VBNET.htm) | Name changes on rename/reorder. |
| Dimension | `"D1@Fillet1@tutor1.SLDPRT", "DIMENSION"` | [Dimension precision example](https://help.solidworks.com/2025/english/api/sldworksapi/Get_Precisions_for_a_Dimension_Example_VB.htm) | Full context is needed; feature/part names can change. |

Example with meaningful selection marks:

```vb
ok = ext.SelectByID2("Sketch1", "SKETCH", 0, 0, 0, False, 1, Nothing, 0)
ok = ext.SelectByID2("Sketch2", "SKETCH", 0, 0, 0, True, 4, Nothing, 0)
ok = ext.SelectByID2("Sketch3", "SKETCH", 0, 0, 0, True, 2, Nothing, 0)
```

This is the official profile/path/guide-curve selection for a sweep. Selection marks belong to the **receiving feature's contract**. Mark `2` does not have one universal role across all commands. [Official sweep example](https://help.solidworks.com/2025/english/api/sldworksapi/Insert_Swept-boss_Feature_Example_VB.htm).

### T6.3 Implication for the replay resolver

A recorded name is useful as a **debug hint**, not a persistent identity. For faces, edges, and vertices, prefer tracked `IEntity` handles where available. Confirm geometry or an existing persistent reference after every regeneration. In the same feature-building session, `IEntity.Select4` or `IBody2.Select2` can avoid coordinate selection. The API reference explicitly recommends selecting an object directly when the object handle is already available. [SelectByID2 documentation](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDocExtension~SelectByID2.html).

---

## T7. Reference plane readback and sweep guides

### T7.1 `IRefPlaneFeatureData`: type readback

**Recorded: official reference-plane example.**

```vb
Set refPlane = fm.InsertRefPlane(16, 0.7853981633975, 4, 0, 0, 0)
ok = ext.SelectByID2("Plane1", "PLANE", 0, 0, 0, False, 0, Nothing, 0)
Set feat = swModel.SelectionManager.GetSelectedObject6(1, -1)
Set data = feat.GetDefinition
planeType = data.Type2
```

This example first selects a face and edge, then creates an angular constraint-based plane. `Type2 = 11` is the documented constraint-based classification in the sample's switch table. `Type` is also read but represents a different legacy/constraint category view. Source: [Insert Reference Plane Example (VB.NET), API Help 2022](https://help.solidworks.com/2022/english/api/sldworksapi/Insert_Reference_Plane_Example_VBNET.htm), [feature-data members](https://help.solidworks.com/2019/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IRefPlaneFeatureData_members.html).

**Recorded: separate official example of distance readback/edit.**

```vb
Set data = feat.GetDefinition
ok = data.AccessSelections(swModel, Nothing)
Debug.Print data.Distance
data.Distance = 0.1
ok = feat.ModifyDefinition(data, swModel, Nothing)
' ReleaseSelectionAccess when feature-data access is no longer needed.
```

Source: [Modify Plane by Editing Its Definition Example (VBA), API Help 2024](https://help.solidworks.com/2024/english/api/sldworksapi/Modify_Plane_by_Editing_Its_Definition_Example_VB.htm).

**Conflicting official property text:** `IRefPlaneFeatureData.Distance` describes offset distance in **meters** in its heading, but calls the property value **radians** later on the same page. Distance uses meters in the macro above. Treat the radians label as a documentation inconsistency, not as an instruction to convert lengths to angles. [Distance property, API Help 2021](https://help.solidworks.com/2021/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IRefPlaneFeatureData~Distance.html).

Reference planes created from SOLIDWORKS 2010 onward are constraint based. For complex modern planes, `Constraint`, `AngleOrDistance`, and `Reference` are also available in the feature data. A seat probe should compare those per-reference values against `Distance`. [Reference-plane feature-data members](https://help.solidworks.com/2019/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IRefPlaneFeatureData_members.html).

### T7.2 `ISweepFeatureData`: complete official builder

**Recorded: official swept-boss example.** It builds a profile sketch, path sketch, and guide sketch, then selects them and creates a sweep:

```vb
swModel.ClearSelection2 True
ok = ext.SelectByID2("Sketch1", "SKETCH", 0, 0, 0, False, 1, Nothing, 0)
ok = ext.SelectByID2("Sketch2", "SKETCH", 0, 0, 0, True, 4, Nothing, 0)
ok = ext.SelectByID2("Sketch3", "SKETCH", 0, 0, 0, True, 2, Nothing, 0)

Set sweepData = fm.CreateDefinition(swFmSweep)
sweepData.TangentPropagation = False
sweepData.AlignWithEndFaces = False
sweepData.TwistControlType = 0
sweepData.MaintainTangency = False
sweepData.AdvancedSmoothing = False
sweepData.StartTangencyType = 0
sweepData.EndTangencyType = 0
sweepData.ThinFeature = False
sweepData.SetWallThickness True, 0#
sweepData.SetWallThickness False, 0#
sweepData.ThinWallType = 0
sweepData.PathAlignmentType = 0
sweepData.Merge = True
sweepData.FeatureScope = True
sweepData.AutoSelect = True
sweepData.SetTwistAngle 0#
sweepData.MergeSmoothFaces = True
sweepData.CircularProfile = False
sweepData.CircularProfileDiameter = 0#
sweepData.Direction = 0
Set feat = fm.CreateFeature(sweepData)
```

Source: [Insert Swept-boss Feature Example (VBA), SOLIDWORKS API Help 2025](https://help.solidworks.com/2025/english/api/sldworksapi/Insert_Swept-boss_Feature_Example_VB.htm). The [pinned archival copy](https://github.com/f1owkang/solidworks-plugin-dev/blob/f084611fd6ee85071650cee069071377950f079c/references/source-docs/Feature/Insert_Swept-boss_Feature_Example_VB.md) contains the same sequence.

The **actual assigned object forms** in this official builder are selected **sketch features**, not direct setter arrays:

| Role | Input form | Mark |
|---|---|---:|
| Profile | `Sketch1` selected with `Type="SKETCH"` | 1 |
| Path | `Sketch2` selected with `Type="SKETCH"` | 4 |
| Guide curve | `Sketch3` selected with `Type="SKETCH"` | 2 |

The official sample does **not** assign `sweepData.Profile = ...`, `sweepData.Path = ...`, or `sweepData.GuideCurves = ...` during creation. Do not represent that direct-setter route as a recovered call.

### T7.3 Real guide-curve readback

**Recorded: same official example.**

```vb
Set data = feat.GetDefinition
count = data.GetGuideCurvesCount
ok = data.AccessSelections(swModel, Nothing)
curves = data.GuideCurves
For i = 0 To count - 1
    Set curve = curves(i)
    Debug.Print data.GetGuideCurvesType(i)
Next i
data.ReleaseSelectionAccess
```

Sources: [official swept-boss example](https://help.solidworks.com/2025/english/api/sldworksapi/Insert_Swept-boss_Feature_Example_VB.htm), [Get Guide Curves in Sweep Feature Example (VBA)](https://help.solidworks.com/2023/english/api/sldworksapi/Get_Guide_Curves_in_Sweep_Feature_Example_VBNET.htm).

`GuideCurves` is an `Object` array property. The [2021 property reference](https://help.solidworks.com/2021/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISweepFeatureData~GuideCurves.html) allows guide curves, edges, lines, and arcs. An [older 2017 page](https://help.solidworks.com/2017/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISweepFeatureData~GuideCurves.html) emphasizes feature objects. **The exact accepted COM objects on the setter, particularly `SketchSegment` versus `Feature`, remain untested.** The guide property is also restricted for certain direction/twist-control combinations. The selection-mark route above is the confirmed initial implementation.

For comparison, the older recorded/SDK route creates the same sweep with `InsertProtrusionSwept4(...)` after selecting the same three sketches with marks `1, 4, 2`. Its official example contains **20 positional inputs**. Source: [Get Guide Curves in Sweep Feature Example (VBA), archival copy](https://github.com/f1owkang/solidworks-plugin-dev/blob/f084611fd6ee85071650cee069071377950f079c/references/source-docs/Feature/Get_Guide_Curves_in_Sweep_Feature_Example_VB.md).

---

## Conflicts: example versus reference page

| ID | Sources | Conflict | Handling |
|---|---|---|---|
| C1 | `CreateConic` official sample vs assumptions from a sketch UI rho parameter | The API call takes **12 doubles**: focus, apex, start, end. No scalar rho. | Convert geometric representation before the call. Do not use a 13th scalar. |
| C2 | `CreateSplineByEqnParams` current page vs older OLE documentation | Current page says three logical arrays, but exposes one `Object` argument. Older page explains one packed SafeArray. | Use the packed layout as a test, with a typed-array seat probe. |
| C3 | `CreateSpline3` remarks on the same page | It specifies XYZ triples, then says 2D sketch point data contains no Z. | Test a nonzero-Z input in a 2D sketch. |
| C4 | `InsertMoveCopyBody2` 2013/2018 pages vs 2022 page | Older pages give translation priority; 2022 gives rotation priority when both are supplied. | Never combine the modes in one replay step until tested. |
| C5 | `InsertCombineFeature` method page vs official VBA example | Method page requests an empty tool array in selected mode; example passes `Nothing`. | Start with the official sample's selected-body form. Test both variants. |
| C6 | `IRefPlaneFeatureData.Distance` heading vs property-value text | One says meters; the other says radians. | Use meters for linear offsets; check physical separation. |
| C7 | `ISweepFeatureData.GuideCurves` 2017 vs 2021 page | The older description focuses on feature objects; newer description includes curves, edges, lines, arcs. | Use marked sketch-feature selection first. Probe direct setter objects separately. |
| C8 | Official 12-argument move/copy signature vs a third-party wrapper | One public wrapper invokes only 11 arguments. | Reject that wrapper's shape as incomplete. |
| C9 | Official and community `CreateSpline3` descriptions vs a third-party coordinate loop | A community caller uses the right `double[]` shape but mistakenly writes X to X, Y, and Z. | Trust the call signature, not its faulty array construction. |

The full source URLs appear in the corresponding T1-T7 sections.

---

## Still needs a SOLIDWORKS 2026 seat probe

| Priority | API | Minimal setup and action | Required observation / pass criterion |
|---|---|---|---|
| **P0** | `CreateConicByEqnParams` | Inspect the actual type library with VBA Object Browser; search exact method names. | Exists or absent, full exact declaration if present. No guessed array layouts. |
| **P0** | `CreateConic` | Create four conic examples with known focus, apex and endpoints. | Returned segment class, measured end positions, focus/apex behavior, successful rebuild. Determine which source conic types can be mapped exactly. |
| **P0** | `CreateSplineByEqnParams` | Create a quadratic B-spline with `N=3`, order 3, six knots and XY controls. Try `Variant`/`Array` and `Double()` SafeArray. | COM type accepted; non-null segment; control count and knot sequence read back. |
| **P1** | `CreateSpline3` | Pass two XYZ triples, including nonzero Z, in a 2D sketch; test status as by-reference Variant. | Coordinate interpretation, COM output type and visible geometry. |
| **P1** | Constant-radius fillet builder | Select one unambiguous edge with mark 1. Apply circular profile, default overflow, radius 0.005 m. | Created feature, 5 mm physical radius, actual display-dimension name and its value. |
| **P0** | `InsertMoveCopyBody2` | On one known body, set translation X = 0.010 m and rotation Z = 0.5 rad in the **same** call. Repeat as separate features. | Which motion wins in SW2026 and whether separate features give exact composition. |
| **P1** | `InsertMoveCopyBody2` | Translate using `TransDist` with a marked edge or vertex, then compare with XYZ translation. Rotate around one axis with an angle of `pi/2`. | Distance semantics, unit, rotation reference behavior and axis convention. |
| **P1** | `InsertCombineFeature` | Use two intersecting named bodies. Compare selected `Nothing/Nothing`, empty array, and `Array(bodyA,bodyB)`. Also test subtraction. | Which form creates a live feature and which operand remains after cut. |
| **P1** | `SelectByID2` resolver | Record names of an edge, body, reference plane and dimension. Rename source feature and rebuild. | What remains selectable. Check persistent-reference recovery as the alternative. |
| **P2** | `IRefPlaneFeatureData` | Make an offset plane at 0.010 m and a constraint-based angular plane. Read `Type`, `Type2`, `Distance`, per-reference `AngleOrDistance`. | Returned classifications and values versus physical plane offset/angle. |
| **P1** | `ISweepFeatureData` | Create the official 1/4/2 marked-sketch sweep. Read `GuideCurves` and its types. Try setting `GuideCurves` as arrays of `Feature`, `SketchSegment`, and `Edge` in isolated trials. | Creation success, accepted runtime object types, guide count, and physical sweep result. |

**Recommended replay order:** Conic type-library check and geometry first. Then the spline format and move/copy precedence probes. Follow with fillet, combine, and sweep. Keep a fresh part and a saved before/after model per probe so failures do not contaminate later results.

## Source-quality limitations

1. This report uses public SOLIDWORKS API Help, archival copies of API reference pages, and pinned third-party source. It does not establish runtime behavior on the owner's licensed SOLIDWORKS 2026 seat.
2. Examples labeled **Recorded** originate from official example pages. They are not always literal interactive macro recordings.
3. The `CreateConic` page had limited direct search-page accessibility. Two public mirrors preserve the detailed signature, and the official VBA/C# example confirms its ordered 12-value call.
4. GitHub searches did not recover a use of `CreateConicByEqnParams` or a proven packed `CreateSplineByEqnParams` call. Absence from those searches is not proof of absence from every SDK or local type library.
5. Direct VBA body-array marshaling, mixed rotation/translation precedence, and guide-curve setter input objects remain test requirements. No report statement should be used to bypass those tests.
