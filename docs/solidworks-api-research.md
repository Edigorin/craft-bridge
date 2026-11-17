# SOLIDWORKS API research for craft-bridge

**Filed:** 2026-11-17, produced by the owner's research agent from public
API documentation only (no seat execution). Kept in craft-bridge as the
evidence behind the seat probes and the Phase B/C emitters. Key pins that
already changed our understanding: the `FeatureExtrusion2` argument map
(positions 18/19/20 = Merge/UseFeatScope/UseAutoSelect — our corpus-proven
call carries True/True/True), the `InsertRefPlane` six-argument shape with
pre-selected references (marks 0/1/2), the negative distinction that
join/cut/intersect do NOT map to the boss-extrusion call (cut-extrusion
and Combine APIs instead), `CreateConic`'s existence, and the per-method
selection-mark table. The probe queue P01–P14 at the end is the seat
work plan.

---

**Research date:** 2026-11-17
**Target:** SOLIDWORKS 2026 VBA/COM replay macros.
**Scope:** Public API documentation only. No live seat execution.
**Trademark:** SOLIDWORKS is a trademark of Dassault Systèmes. This document is independent research and implies no affiliation.

**Evidence labels:** **Documented** = explicitly stated in official API help. **Inferred** = plausible from published API structure or examples, but not guaranteed by the cited contract. **Unverified** = requires a seat probe or further documentation. A documented method's existence does not prove a proposed replay mapping.

## A1. Reference planes — priority 1

| Fact | Status | Source |
|---|---|---|
| `IFeatureManager.InsertRefPlane(FirstConstraint As Integer, FirstConstraintAngleOrDistance As Double, SecondConstraint As Integer, SecondConstraintAngleOrDistance As Double, ThirdConstraint As Integer, ThirdConstraintAngleOrDistance As Double) As Object`. Each integer is a `swRefPlaneReferenceConstraints_e` bitmask. Each double supplies an angle or distance as applicable. The return is a reference-plane object. | **Documented** | [InsertRefPlane](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertRefPlane.html) |
| Reference entities are selected before the call. The first, second, and third references use selection marks `0`, `1`, and `2`. Marks choose **which selected entity**, not the type of geometric constraint. | **Documented** | [InsertRefPlane](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertRefPlane.html) |
| Constraint bitmask: `Parallel=1`, `Perpendicular=2`, `Coincident=4`, `Distance=8`, `Angle=16`, `Tangent=32`, `Project=64`, `MidPlane=128`, `OptionFlip=256`, `OptionOriginOnCurve=512`, `OptionProjectToNearestLocation=1028`, `OptionProjectAlongSketchNormal=2056`, `ParallelToScreen=4096`, `OptionReferenceFlip=8192`. Note that 1028 and 2056 are published numeric values and are **not** pure powers of two. | **Documented** | [Reference constraints enum](https://help.solidworks.com/2022/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swRefPlaneReferenceConstraints_e.html?id=2.10.1.634) |
| Proposed offset plane: preselect an origin plane as reference 1, call `InsertRefPlane(8, 0.012, 0, 0, 0, 0)` for a 12 mm offset. This is the logical interpretation of `Distance=8`; success, direction, and the use of zero for unused slots require testing. | **Inferred** | [InsertRefPlane](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertRefPlane.html), [constraints](https://help.solidworks.com/2022/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swRefPlaneReferenceConstraints_e.html?id=2.10.1.634) |
| `Distance Or OptionFlip` (264) is the candidate for a flipped offset. `MidPlane=128` is the candidate for the midpoint of two references. `Angle=16` is the candidate for a plane angled to a reference, probably requiring a second axis/edge reference. Exact allowed reference types and combinations are **not pinned** by the enum alone. | **Unverified** | [Reference constraints enum](https://help.solidworks.com/2022/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swRefPlaneReferenceConstraints_e.html?id=2.10.1.634) |
| `IRefPlane.Transform` gives the plane transform relative to world coordinates. `GetRefPlaneParams` is obsolete. The transform provides a basis for origin/axis validation, but axis orientation and matrix layout must be checked against `IMathTransform` and a live plane. | **Documented** (property); **Unverified** (specific frame mapping) | [IRefPlane members](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IRefPlane_members.html) |
| A feature can be renamed through `IFeature.Name`. A sketch should be attached by selecting the actual `IRefPlane`/feature entity, rather than assuming that `PLANE@<feature>` is a valid `SelectByID2` grammar. | **Inferred** (recommended procedure); **Unverified** (string grammar and selection mark) | [IRefPlane members](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IRefPlane_members.html) |
| A plane offset can potentially be driven by its dimension through `IEquationMgr` or `AddDimEquation`. The exact generated name (`D1@Plane1` or otherwise), equation syntax, editability, and whether an equation drives `InsertRefPlane` output are **not established** by the documentation inspected. Never hardcode `D1@Plane1` without querying the created feature's display dimensions. | **Unverified** | [InsertRefPlane](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertRefPlane.html) |

**Version:** `InsertRefPlane` is available from SOLIDWORKS 2010 FCS (revision 18.0). The cited calls exist before the SOLIDWORKS 2026 target. [Source](https://help.solidworks.com/2010/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IFeatureManager~InsertRefPlane.html).

## A2. FeatureExtrusion2 and feature scope

**Exact positional signature** (`Boolean`, `Integer`, and `Double` types shown by group):

```vb
FeatureExtrusion2( _
  Sd As Boolean, Flip As Boolean, Dir As Boolean, _
  T1 As Integer, T2 As Integer, D1 As Double, D2 As Double, _
  Dchk1 As Boolean, Dchk2 As Boolean, Ddir1 As Boolean, Ddir2 As Boolean, _
  Dang1 As Double, Dang2 As Double, _
  OffsetReverse1 As Boolean, OffsetReverse2 As Boolean, _
  TranslateSurface1 As Boolean, TranslateSurface2 As Boolean, _
  Merge As Boolean, UseFeatScope As Boolean, UseAutoSelect As Boolean, _
  T0 As Integer, StartOffset As Double, FlipStartOffset As Boolean) As Feature
```

| Position(s) | Meaning | Status | Source |
|---|---|---|---|
| 1–3 | `Sd` is single-direction mode, `Flip` controls direction 1 reversal, and `Dir` represents a direction-related setting. Their complete interaction, especially with a reversed sketch normal, needs a probe. | **Documented** (names); **Unverified** (full directional mapping) | [FeatureExtrusion2](https://help.solidworks.com/2024/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureExtrusion2.html) |
| 4–7 | `T1`, `T2` select the two end conditions. `D1`, `D2` set their distances. | **Documented** | [FeatureExtrusion2](https://help.solidworks.com/2024/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureExtrusion2.html) |
| 8–13 | `Dchk1`, `Dchk2` enable draft; `Ddir1`, `Ddir2` choose inward/outward draft; `Dang1`, `Dang2` are draft angles. | **Documented** | [FeatureExtrusion3 parameters](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureExtrusion3.html) |
| 14–17 | `OffsetReverse1/2` control offset direction from an end face or plane. `TranslateSurface1/2` select translated-surface versus true-offset behavior. These matter with offset-from-surface ends. | **Documented** | [FeatureExtrusion3 parameters](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureExtrusion3.html) |
| **18** | **`Merge`**. `True` requests a merged result in a multibody part; `False` requests no merge. | **Documented** | [FeatureExtrusion2](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureExtrusion2.html) |
| **19** | **`UseFeatScope`**. `True` limits the affected bodies to the defined feature scope; `False` affects all applicable bodies. | **Documented** | [FeatureExtrusion2](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureExtrusion2.html) |
| **20** | **`UseAutoSelect`**. `True` automatically selects affected bodies; `False` requires explicit body selection when using feature scope. | **Documented** | [FeatureExtrusion2](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureExtrusion2.html) |
| 21–23 | `T0` specifies the start condition, `StartOffset` supplies a distance when start is offset, and `FlipStartOffset` reverses that offset. | **Documented** | [FeatureExtrusion3 parameters](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~FeatureExtrusion3.html) |

| End condition | Numeric value | Status | Source |
|---|---:|---|---|
| `swEndCondBlind` | 0 | **Documented** | [End conditions enum](https://help.solidworks.com/2026/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swEndConditions_e.html) |
| `swEndCondThroughAll` | 1 | **Documented** | Same |
| `swEndCondThroughNext` | 2 | **Documented** | Same |
| `swEndCondUpToVertex` | 3 | **Documented**, obsolete usage | Same |
| `swEndCondUpToSurface` | 4 | **Documented**, obsolete usage | Same |
| `swEndCondOffsetFromSurface` | 5 | **Documented** | Same |
| `swEndCondMidPlane` | 6 | **Documented** | Same |
| `swEndCondUpToBody` | 7 | **Documented** | Same |
| `swEndCondThroughAllBoth` | 9 | **Documented** | Same |
| `swEndCondUpToSelection` | 10 | **Documented** | Same |
| `swEndCondUpToNext` | 11 | **Documented** | Same |

| Replay question | Status | Finding |
|---|---|---|
| Does `Merge=False` always produce a distinct solid body? | **Unverified** | Intended behavior is no merge, but body creation can still fail due to invalid profile, zero thickness, or feature errors. Verify body count and identity after rebuild. |
| Does `Merge=True` merge every touching body or only scoped bodies? | **Unverified** | Combine geometry and feature scope must be checked on two touching bodies with an explicitly scoped feature. |
| Can source `join`, `cut`, and `intersect` all be mapped to this call? | **Documented** (negative distinction) | `FeatureExtrusion2` is an extruded boss/base feature, not a general boolean operator. Use cut-feature APIs for subtractive extrusions and Combine for body intersections. [Combine](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html). |

**Version:** `FeatureExtrusion2` is superseded by `FeatureExtrusion3`, which adds support for reference selection in start/end conditions. Prefer `FeatureExtrusion3` when using these advanced end conditions. [FeatureExtrusion3](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featureextrusion3.html).

## A3. Multibody operations, identity, mass

| Fact | Status | Source |
|---|---|---|
| `IPartDoc.GetBodies2(BodyType As Integer, BVisibleOnly As Boolean) As Object` returns bodies of the requested type. `BVisibleOnly=True` excludes hidden bodies. Valid supported types are solid (`swSolidBody=0`) and sheet (`swSheetBody=1`). The returned order can change. | **Documented** | [GetBodies2](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IPartDoc~GetBodies2.html), [swBodyType](https://help.solidworks.com/2025/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swBodyType_e.html) |
| `IBody2.GetEdges`, `IBody2.GetFaces`, `IBody2.GetEdgeCount`, `IBody2.GetFaceCount` exist. Validate the returned Variant before iteration. | **Documented** (method existence); **Unverified** (0/1/N COM shapes) | [IBody2 members](https://help.solidworks.com/2021/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IBody2_members.html) |
| `IFeatureManager.InsertCombineFeature(OperationType As Integer, MainBody As Body2, ToolVar As Object) As Feature`. `ToolVar` contains the bodies to combine. `SWBODYADD=15903`, `SWBODYCUT=15902`, `SWBODYINTERSECT=15901`. For CUT, `MainBody` is the target. For ADD and INTERSECT, set `MainBody=Nothing`. | **Documented** | [InsertCombineFeature](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html), [operation enum](https://help.solidworks.com/2024/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swBodyOperationType_e.html) |
| Alternative Combine selection: target uses mark 1 and tool bodies mark 2. Then pass `Nothing` for `MainBody` and an empty array for `ToolVar`. `IInsertCombineFeature` is not suitable for VBA because it is the in-process array form. | **Documented** | [InsertCombineFeature](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html), [IInsertCombineFeature](https://help.solidworks.com/2019/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~IInsertCombineFeature.html) |
| Whether Combine can retain tool bodies with a particular keep-tools setting is not established by `InsertCombineFeature`'s signature. Do not silently assume source keep-tools semantics. | **Unverified** | [Combine feature data](https://help.solidworks.com/2024/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ICombineBodiesFeatureData_members.html) |
| `IFeatureManager.InsertMoveCopyBody2(TransX,TransY,TransZ,TransDist,RotPointX,RotPointY,RotPointZ,RotAngleX,RotAngleY,RotAngleZ,BCopy,NumCopies) As Feature`; first ten arguments are Double, `BCopy` is Boolean, `NumCopies` is Integer. It creates a feature, unlike a transient edit of a body geometry object. | **Documented** | [InsertMoveCopyBody2](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html) |
| Move/Copy marks: body 1, rotation reference 2, translation reference 4, translation vertex 8. Translation and rotation in one call are not combined: if both are given, rotation wins. Use sequential features if both are required. | **Documented** | [InsertMoveCopyBody2](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertMoveCopyBody2.html) |
| `IBody2.GetMassProperties(Density As Double) As Object` returns a 12-double array. For solids, indexes 0–11 are `[cx, cy, cz, volume, area, mass, Ixx, Iyy, Izz, Ixy, Izx, Iyz]`. Density is supplied by the caller. Native dimensions follow SI units. | **Documented** | [GetMassProperties](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ibody2~getmassproperties.html) |
| `IBody2.GetBodyBox() As Object` returns `[x1,y1,z1,x2,y2,z2]` in metres for an axis-aligned box. The official documentation warns that this box is approximate and can vary after rebuild. Do not use it as an exact geometric oracle. | **Documented** | [GetBodyBox](https://help.solidworks.com/2022/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IBody2~GetBodyBox.html) |
| `IBody2.Name` / selecting `SOLIDBODY` by displayed body name, default name patterns, the delete-body feature API, and a stable mapping from original body to move/copy result remain unresolved in this pass. | **Unverified** | [IBody2 members](https://help.solidworks.com/2021/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IBody2_members.html) |

## A4. Edge and face identity; fillets

| Fact | Status | Source |
|---|---|---|
| Enumerate topology through `IBody2.GetEdges` and `IBody2.GetFaces`. Use direct edge objects for selection and retain geometric descriptors for verification. Do not build undocumented strings such as `EDGE@Feature@index`. | **Documented** (enumeration API); **Inferred** (replay policy) | [IBody2 members](https://help.solidworks.com/2021/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IBody2_members.html) |
| `IModelDocExtension.GetPersistReference3(DispObj As Object) As Object` returns an ID array. `GetObjectByPersistReference3(PersistId As Object, ByRef ErrorCode As Integer) As Object` resolves it. The bytes are opaque and may change between builds/releases. Use resolver APIs, not byte equality as a geometric test. | **Documented** | [GetPersistReference3](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.interop.sldworks~SolidWorks.interop.sldworks.IModelDocExtension~GetPersistReference3.html), [resolve](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~GetObjectByPersistReference3.html) |
| Persistent reference IDs are not a guarantee that an edge survives topological changes. Verify resolver error state, actual entity type, and geometry after each rebuild. | **Inferred** | [Persistent reference IDs](https://help.solidworks.com/2012/English/api/sldworksapiprogguide/overview/Persistent_Reference_IDs.htm), [resolve](https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~GetObjectByPersistReference3.html) |
| For constant-radius fillets, prefer feature definitions through `IFeatureManager.CreateDefinition`, `CreateFeature`, and `ISimpleFilletFeatureData2`. The legacy `FeatureFillet3` path is obsolete for constant-radius fillets as of 2020. | **Documented** | [FeatureFillet3](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurefillet3.html) |
| Legacy `FeatureFillet3(Options As Integer, R1 As Double, R2 As Double, Rho As Double, Ftyp As Integer, OverflowType As Integer, ConicRhoType As Integer, Radii As Object, Dist2Arr As Object, RhoArr As Object, SetBackDistances As Object, PointRadiusArray As Object, PointDist2Array As Object, PointRhoArray As Object) As Object`. The uniform symmetric radius is `R1` when the proper option bit is set. Variable-radius edge selections use mark 1 and control points mark 256. | **Documented** | [FeatureFillet3](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ifeaturemanager~featurefillet3.html) |
| `IEdge` endpoint, parameter, tangent, curve type and length calls; nearest-point selection precision; `SelectByRay` tolerance; `ISelectData` mark behavior; named-face lookup; and exact fillet dimension naming still need direct method-page inspection and a seat probe. | **Unverified** | [Persistent reference IDs](https://help.solidworks.com/2012/English/api/sldworksapiprogguide/overview/Persistent_Reference_IDs.htm) |

**Fail-closed edge policy (inferred):** enumerate all candidate edges; compute a distance using evaluated geometric curves, not merely bounding boxes. Require one winner within tolerance and a margin to the second-best candidate. Select the edge object. Read `ISelectionMgr.GetSelectedObject6` and compare the selected COM entity/persistent reference with the intended object. Recheck after fillet creation. Exact geometric-distance method calls remain in the probe queue.

## A5. Sketch arcs, splines, ellipses and conics

| Fact | Status | Source |
|---|---|---|
| `ISketchManager.CreateArc(XC,YC,ZC,X1,Y1,Z1,X2,Y2,Z2,Direction As Short) As SketchSegment`: first 9 arguments are Double coordinates in metres. `Direction=+1` means counter-clockwise from first to second endpoint; `-1` means clockwise. | **Documented** | [CreateArc](https://help.solidworks.com/2013/English/Api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateArc.html) |
| `ISketchManager.CreateSpline3(PointData As Object, Surfs As Object, Direction As Object, SimulateNaturalEnds As Boolean, ByRef Status As Object) As Object`. `PointData` contains point coordinates. Pass `Nothing` for surfaces and directions to make a 2D spline. This method interpolates supplied points in the sketch context. | **Documented** | [CreateSpline3](https://help.solidworks.com/2023/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateSpline3.html) |
| `CreateSpline` and `CreateSpline2` are obsolete relative to `CreateSpline3`. `CreateSplineByEqnParams` accepts B-spline control points and a knot vector. This is a distinct path from interpolation through 17 sampled points. | **Documented** | [SketchManager members](https://help.solidworks.com/2018/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager_members.html), [CreateSplineByEqnParams](https://help.solidworks.com/2012/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateSplineByEqnParams.html) |
| `CreateEllipse(XC,YC,ZC,XMajor,YMajor,ZMajor,XMinor,YMinor,ZMinor) As SketchSegment` creates a full ellipse from center and axis-defining points. `CreateEllipticalArc` creates a partial ellipse. | **Documented** | [CreateEllipse](https://help.solidworks.com/2023/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateEllipse.html) |
| SOLIDWORKS API help links an `ISketchManager.CreateConic` method. Therefore the statement that the sketch API has no conic entity is false. **The full signature, meaning of parameters, rational conic weight mapping, and range of supported conic types are not yet pinned.** Do not replace 854 conics with ellipses or splines by default. | **Documented** (existence), **Unverified** (shape mapping) | [CreateEllipse see also](https://help.solidworks.com/2023/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateEllipse.html) |
| Arc endpoint points can be accessed as sketch geometry, but exact endpoint dimensions, spline tangent constraints, sketch relation selection counts, closed-spline flag behavior, `AddDimension2` selection rules, and construction flag API require targeted verification. | **Unverified** | [SketchManager members](https://help.solidworks.com/2018/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager_members.html) |

**Replay policy (inferred):** if a source conic cannot be mapped with known parameters and verified geometric error, refuse faithful parametric replay. An explicit *approximate geometry* mode may use a spline but must record deviation and the lost conic semantics.

## A6. Sweeps with guides

| Fact | Status | Source |
|---|---|---|
| `IFeatureManager.InsertProtrusionSwept3(Propagate As Boolean, Alignment As Boolean, TwistCtrlOption As Short, KeepTangency As Boolean, BAdvancedSmoothing As Boolean, StartMatchingType As Short, EndMatchingType As Short, IsThinBody As Boolean, Thickness1 As Double, Thickness2 As Double, ThinType As Short, PathAlign As Short, Merge As Boolean, UseFeatScope As Boolean, UseAutoSelect As Boolean, TwistAngle As Double, BMergeSmoothFaces As Boolean) As Feature`. | **Documented** | [InsertProtrusionSwept3](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertProtrusionSwept3.html) |
| Select sweep profile with mark `1`, guide curves with mark `2`, and path with mark `4`. Guide curves are selected geometry, not positional numeric arguments. `PathAlign=0` gives default, `2` direction-vector alignment, and `3` all-faces alignment, when twist control is follow-path (`0`). | **Documented** | [InsertProtrusionSwept3](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertProtrusionSwept3.html) |
| This solid-boss sweep method has related cut and surface APIs. A cut method exists as `InsertCutSwept3`, with corresponding profile, guide and path marks. | **Documented** | [InsertCutSwept3](https://help.solidworks.com/2023/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCutSwept3.html) |
| `InsertProtrusionSwept3` is listed as obsolete in newer help in favor of `InsertProtrusionSwept4`; `InsertProtrusionSwept4` also has obsolescence notes in later API catalogs. Pin the 2026 supported entry point before committing a generator. | **Documented** (version warning); **Unverified** (2026 preferred construction path) | [2017 swept3 page](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertProtrusionSwept3.html), [2019 manager members](https://help.solidworks.com/2019/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager_members.html) |
| Whether selected 2D sketches, 3D sketches, curves or composite curves are accepted as a given guide, whether they must intersect the profile, and the exact twist enum members for replay remain unresolved here. | **Unverified** | [InsertProtrusionSwept3](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertProtrusionSwept3.html) |

## A7. COM and macro discipline

| Fact | Status | Source |
|---|---|---|
| `GetBodies2` returns an object containing an array and does not guarantee order. Other object-array APIs may have zero-element or `Empty`/`Nothing` behaviors, which the method declarations do not specify. Never assume one body or one edge gives a scalar object. | **Documented** (array and ordering); **Unverified** (0/1/N exact VBA representation) | [GetBodies2](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IPartDoc~GetBodies2.html) |
| VBA array guard: test `IsEmpty(v)`, `IsNull(v)`, `IsObject(v)`, then `IsArray(v)` before `LBound`/`UBound`. Under `On Error GoTo`, count only valid entries and check each element with `IsObject`. This is a defensive recipe, not a guarantee of the native COM shape. | **Inferred** | [GetBodies2](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IPartDoc~GetBodies2.html) |
| Selection marks are method-specific. Plane references use `0/1/2`, Combine uses `1/2`, sweep uses `1/2/4`, and Move/Copy uses `1/2/4/8`. Do not reuse a global mark table across unrelated calls. | **Documented** | [InsertRefPlane](https://help.solidworks.com/2022/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertRefPlane.html), [Combine](https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertCombineFeature.html), [Sweep](https://help.solidworks.com/2017/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IFeatureManager~InsertProtrusionSwept3.html) |
| Geometry positions and distances are specified in metres, angles in radians under usual SOLIDWORKS API conventions, and mass properties in SI. Double-check specific exceptions before converting document units. | **Documented** (coordinate/metre examples); **Inferred** (blanket rule for all APIs) | [CreateArc](https://help.solidworks.com/2013/English/Api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~CreateArc.html), [GetMassProperties](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.ibody2~getmassproperties.html) |
| The API exposes `IModelDocExtension.GetLastFeatureAdded`, which is preferable to assuming the last feature in tree enumeration is the newly created feature. Capture the return object from a feature-creation call when one is available. | **Documented** (method existence); **Inferred** (priority) | [IModelDocExtension members](https://help.solidworks.com/2011/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension_members.html) |
| `EditRebuild3` failure reporting, precise `SelectByID2` `Append`/mark behavior, `ClearSelection2` scope, COM exception classes, imported-feature fallback tree type, feature naming collisions and final tree order are not fully established by this research. | **Unverified** | [IModelDoc2 members](https://help.solidworks.com/2024/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDoc2_members.html) |

## B. Minimal committed seat probes

| ID | Probe geometry and observation | Resolves |
|---|---|---|
| P01 | Select Top plane, create offset 12 mm using distance constraint, rename `CB_Plane_A`, create a 10 mm circle sketch on it. Inspect transform, feature dimensions, `SelectByID2` grammar, and sketch mark. | A1 offset/selection/frame/dimension |
| P02 | Create two origin planes, then 12 mm offset flipped, a midplane, and a 30-degree angled plane about a selected line. Log constraint bitmasks and transforms. | A1 flip/midplane/angle |
| P03 | Bind a global variable to plane offset, change 12 to 19 mm, rebuild, and inspect offset and dimension identifier. | A1 equations |
| P04 | Create two disjoint boxes and a boss touching both. Toggle Merge and explicitly scope one body. Record identities and volumes. | A2 merge/scope |
| P05 | Extrude from an oriented sketch with `Flip` and `Dir` toggled. Compare bounds and surface normals. | A2 direction and draft |
| P06 | Create three boxes; use Combine add, cut, intersect. Test selected-body and direct-object forms and check whether tools survive. | A3 Boolean/keep tools |
| P07 | Create two named bodies, move by 10 mm, then rotate 45 degrees about a displaced edge using separate features; inspect names after rebuild. | A3 body names/transform |
| P08 | Create a rectangular block and a cylinder. Inspect 0/1/N arrays, mass 12-vector, box 6-vector, and body name selection. | A3/A7 arrays and measurements |
| P09 | On a block with two near edges, choose a point 0.1 mm from only one edge. Select object directly and inspect selection manager and persistent ID. Rebuild after fillet. | A4 nearest-edge/fail-closed |
| P10 | Fillet one of four block edges with radius 1.25 mm using a new feature definition. Inspect readback dimension and edge identity. | A4 fillet |
| P11 | Create CCW/CW arcs, a 17-point interpolating spline, and a closed spline. Inspect curve types, endpoints, relations, and fitted point positions. | A5 curves |
| P12 | Create elliptical arc and three source-representative conics, then compare mathematical parameters and result geometry. Refuse replay on unmatched conic. | A5 conics |
| P13 | Sweep a circular section along a 3D path with two guides. Test marks 1/2/4, then toggle twist/align settings. | A6 sweeps |
| P14 | Force a rebuild failure, invalid selection, missing body, and an imported solid. Log `Nothing`, Boolean status, error codes, and final feature/tree types. | A7 errors/fallback |

## Unknowns and implementation gates

| Priority | Unknown | Status | Seat probe | Implementation rule |
|---|---|---|---|---|
| Blocker | Offset-plane dimension identifier and equation binding | **Unverified** | P01, P03 | Do not synthesize `D1@Plane1` without observing the feature parameter. |
| Blocker | Offset plane selection for sketch, transform axis order, flip | **Unverified** | P01, P02 | Verify position and normal before sketch creation. |
| High | Exact merge/scope topology behavior | **Unverified** | P04 | Compare body count and mass after every operation. |
| High | Feature direction relative to sketch normal | **Unverified** | P05 | Check signed extrusion displacement. |
| High | Combine keep-tools and body naming | **Unverified** | P06, P07 | Preserve explicit body IDs in replay metadata. |
| High | Edge geometric nearest-point API and selection false positives | **Unverified** | P09 | Refuse tied or ambiguous candidates. |
| High | New constant-radius fillet feature-definition setup | **Unverified** | P10 | Do not fall back silently to obsolete feature call. |
| High | Full `CreateConic` signature and faithful source mapping | **Unverified** | P12 | No default approximation of source conics. |
| Medium | Spline endpoint/tangent and close behavior | **Unverified** | P11 | Verify endpoints and continuity. |
| Medium | Current preferred sweep builder and guide types | **Unverified** | P13 | Test with one profile, one path, two rails. |
| Medium | VBA 0/1/N arrays and rebuild/error semantics | **Unverified** | P08, P14 | Defensive type checks; no assumed counts. |
| Medium | Body delete, imported fallback, feature scope, rename collisions | **Unverified** | P07, P14 | Explicit post-rebuild topology checks. |

## Research limits

The report fixes the documented signatures, numeric enums, and several mark rules. It **does not** claim full coverage of each requested API. In particular, the equation interface, exact edge geometric descriptors, conic parameter mapping, body deletion, and modern feature-data builder steps need further official method-page research and/or probes. The current evidence does not justify executable macros for those seams without probing.
