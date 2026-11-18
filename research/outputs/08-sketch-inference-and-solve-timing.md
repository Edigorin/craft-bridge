# SOLIDWORKS sketch inference and solve timing 08

**Filed:** 2026-11-18, produced by the owner's research agent from public
documentation, delivered as a chat attachment and filed verbatim. Answer
to `research/inputs/08-sketch-inference-and-solve-timing.md`.
Provenance note: the report's "seat evidence (supplied)" sections quote
this repo's own probe results (probe-snap/probe-dimkind, docs/source-
probe-evidence.md); everything else is public-source research and carries
the report's own Recorded/Community/Conflicting/Reconstructed labels.
Key items folded into the emitter the same day: ISketchManager.AddToDB is
the documented no-inference member (the probe tested the wrong
interface), and disabling swSketchInference couples Automatic Relations
and the child snap options - the capture/restore must cover the whole
preference vector. The `Note` type-name collision lesson is filed in
docs/source-probe-evidence.md.

---

# Research 08 — SOLIDWORKS sketch inference and solve timing

**Research date:** 18 November 2026  
**Target:** craft-bridge, SOLIDWORKS 2026 (VBA/COM), SolidCraft native `solidcraft/57`  
**Controlling input:** `08-sketch-inference-and-solve-timing.md`  
**Related inputs:** research reports 04, 05, 06, 07, `SolidWorks_API_Research`, `SolidCraft_57_Format_Research`, and `SolidCraft_v043_Nested_Schema_Research`  
**Execution:** Public documentation and source analysis only. No licensed SOLIDWORKS 2026 seat was accessed for this report.

SOLIDWORKS is a trademark of Dassault Systèmes. This report uses its name only to identify the product and APIs.

## 0. Evidence definitions

- **Recorded:** An official SOLIDWORKS API declaration, API example, or product-help statement. The linked year identifies the documentation edition. It does not prove the 2026 seat behaves identically.
- **Community:** Public third-party code or a discussion. It establishes a reported use or observed outcome, not a guarantee.
- **Conflicting:** Published sources disagree, or a documented statement and the supplied 2026 seat observation address the same behavior differently. Both are reported.
- **Reconstructed / untested:** A proposed replay rule or combined call sequence. The emitter must not treat it as verified until a 2026 seat probe passes.
- **Seat evidence (supplied):** Results stated in brief 08 from `probe-snap` / `probe-dimkind` on 18 November 2026. These results were not repeated in this research environment.

All distances passed to SOLIDWORKS geometry and dimension APIs are in **metres**, and angles are normally in **radians**. SolidCraft sketch coordinates and constraint lengths are in **millimetres**. Convert explicitly before the COM call.

## 1. Findings at a glance

| Area | Finding | Evidence | Emitter decision |
|---|---|---|---|
| T1: master preference | `swSketchInference = 249` is the system option **Enable snapping**. It affects sketch snap options. Disabling it also clears Automatic Relations in the documented UI behavior. | **Recorded**, [R01], [R02], [R03]; **seat evidence** | Retain 249-off as the known seat-tested path. Capture and restore **all affected preferences**, not only 249. |
| T1: midpoint observation | Turning off 268 alone did not stop the ~0.6 mm, one-axis midpoint-alignment defect. | **Seat evidence** | The defect cannot be attributed solely to the midpoint snap setting. Probe related inference and horizontal/vertical-to-point options. |
| T1: preference scope | These options are **system/application** settings on `ISldWorks`, not sketch-local settings. The historic `swSnapToPoints` etc. are obsolete **document** options. | **Recorded**, [R01], [R04], [R05] | Do not use the obsolete option IDs as substitutes. Do not leave application preferences changed after a replay. |
| T2: missing `AddToDB` | `ISketch.AddToDB` is not the documented member. **`ISketchManager.AddToDB As Boolean` is documented since SOLIDWORKS 2008.** It creates geometry directly and bypasses snapping/inferencing. | **Recorded**, [R06], [R07]; **seat evidence** for err 438 on the wrong object | Test `swModel.SketchManager.AddToDB = True` as the independent no-inference path. Restore it on error. |
| T2: solve control | `ISketch.EnableSolving` is not documented. `ISketch.SetAutomaticSolve(Boolean) As Boolean`, `ISketch.GetAutomaticSolve()`, and `ISketchManager.AutoSolve As Boolean` are documented. | **Recorded**, [R08], [R09], [R10] | Use the documented member and preserve its previous state. Do not use solve suppression to claim coordinates are final. |
| T2: preference setter | `ISldWorks.SetUserPreferenceToggle` is a **Sub/void** method in official declarations, including 2011 and 2019. | **Recorded**, [R11], [R12] | Use a statement, then independently read back the setting. Do not assign a success return. |
| T3: dimension writes | Neither `IDimension.SystemValue` nor the creation APIs guarantee a synchronous completed sketch solve. Official change-dimension examples explicitly rebuild the part. | **Recorded**, [R13], [R14], [R15] | Treat assignment as a requested edit. Verify geometry after a known solve/rebuild boundary. |
| T3: sketch status | `ISketch.GetConstrainedStatus` and `swConstrainedStatus_e` distinguish under/fully/over-constrained, invalid, no solution, and **AutoSolve off** (`7`). | **Recorded**, [R16], [R17] | Report sketch status separately from coordinate readback. Status `7` is not proof of a valid final sketch. |
| T3: endpoint APIs | `GetStartPoint2` / `GetEndPoint2` belong to `ISketchLine` and other **specific curve interfaces**, not the generic `ISketchSegment` member list. | **Recorded**, [R18], [R19], [R20] | Get the specific curve object before requesting endpoint `ISketchPoint` objects. |
| T3: rebuild while editing | `EditRebuild3` documents rebuilding needed features, not preservation of sketch edit mode. The UI documentation says **Rebuild exits** sketch edit mode; the supplied seat observed `EditRebuild3` doing so. | **Recorded**, [R21], [R22]; **seat evidence** | Do not call `EditRebuild3` in a code path that assumes editing remains active. No documented rebuild-and-stay call was established. |
| T4: axis dimension kinds | `IModelDoc2.AddHorizontalDimension2` and `.AddVerticalDimension2` specify the axis. `IModelDocExtension.AddSpecificDimension` provides an additional type-controlled API. | **Recorded**, [R23], [R24], [R25] | Use explicit kind and check `IDisplayDimension.Type2`. |
| T4: negative dimensions | SOLIDWORKS product Help permits negative **position** dimensions to reverse direction. This does **not** document a negative `IDimension.SystemValue` contract for every linear dimension. | **Recorded**, [R26]; **untested** API mapping | Test signed `distance_x`/`distance_y` separately from unsigned lengths or radii. |
| Cross-report | SolidCraft stores **explicit relations** and **parameter expressions** separately from raw points. A numerically matching but unconstrained SOLIDWORKS sketch may be a replay failure. | **Source-confirmed in supplied reports** | Check relation type, driving state, parameter dependency, and geometry, not only one coordinate. |

## 2. T1 — preference surface and snap hierarchy

### 2.1 Meaning and scope of every preference in brief 08

The numeric values below were obtained by the user's 2026 interop reflection, as stated in brief 08. The official [Relations/Snaps API mapping][R01] confirms the modern enum **names and system-level accessors**. Except where stated, official text describes the **user-interface behavior**. It does not separately promise that each toggle changes all API-created arcs, splines, or conics.

| `swUserPreferenceToggle_e` member | 2026 value from brief | Documented purpose | Default / scope and API caveat |
|---|---:|---|---|
| `swSketchAutomaticRelations` | 9 | Create geometric relations automatically while adding sketch entities. | **System** option. Factory value not individually established here. `Enable snapping` OFF also clears it in the UI description. [R01], [R02], [R03] |
| `swSnapToPoints` | 60 | **Obsolete** old document option. Despite its name, replaced by modern **`swSketchSnapsGrid`**, not `swSketchSnapsPoints`. | Historic **document** scope. Do not use with `ISldWorks` in new replay code. [R01] |
| `swSnapToAngle` | 61 | **Obsolete** old document snap-to-angle switch. | Replaced by system `swSketchSnapsAngle`. [R01] |
| `swSketchInferFromModel` | 95 | Enable inferencing related to model edges and vertices when sketching on a model face. | **System** option. Not the same as sketch-to-sketch snapping. No separate API behavior guarantee. [R01] |
| `swSnapOnlyIfGridDisplayed` | 142 | **Obsolete** old document grid snap condition. | Replaced by system `swSketchSnapToGridIfDisplayed`. [R01] |
| `swSketchInference` | **249** | Master **Enable snapping** setting. The UI setting affects all sketch snap options; clearing it also clears Automatic Relations. | **System** option. Snap family defaults ON except Grid. Actual altered child flags and restoration order need 2026 readback. [R01]–[R03] |
| `swSketchNoSolveMove` | 250 | Move sketch entities **without solving** dimensions or relations during a drag/move. | **System** UI option. Not a general suspend-solver switch for `CreateLine` or a replacement for `AutoSolve`. [R27] |
| `swSketchSnapsPoints` | 266 | Snap to endpoints and sketch points. | **System** option, normally ON in factory snap group. [R01], [R02] |
| `swSketchSnapsCenterPoints` | 267 | Snap to centre points of circles/arcs and similar entities. | **System** option, normally ON in factory snap group. [R01], [R02] |
| `swSketchSnapsMidPoints` | 268 | Snap to midpoints of sketch entities. | **System** option, normally ON. Turning it off alone did **not** cure the seat's one-coordinate movement. [R01]; seat evidence |
| `swSketchSnapsQuadrantPoints` | 269 | Snap to quadrant locations on curves. | **System** option, normally ON. [R01], [R02] |
| `swSketchSnapsIntersections` | 270 | Snap to geometric intersections. | **System** option, normally ON. [R01], [R02] |
| `swSketchSnapsNearest` | 271 | Control nearest-point inference/snap behavior. The API mapping associates ON with broad snapping and OFF with a proximity requirement. | **System** option, normally ON. Treat exact proximity logic as UI wording, not a numerical threshold. [R01] |
| `swSketchSnapsTangent` | 272 | Snap to tangent locations on arcs, circles, splines, etc. | **System** option, normally ON. [R01], [R02] |

### 2.2 Additional documented settings absent from the seat list

These modern enum **names** are documented. Numeric values were not supplied for them and are not invented here. Fetch exact values from the installed 2026 `swconst` interop when needed. [R01], [R28]

| Additional member | UI effect | Relevance to the reported defect |
|---|---|---|
| `swSketchSnapsPerpendicular` | Snap a line perpendicular to another line. | Could influence a line endpoint or direction near existing geometry. |
| `swSketchSnapsParallel` | Snap a new line parallel to another. | Possible alignment candidate. |
| **`swSketchSnapsHVLines`** | Snap horizontal/vertical relative to an existing line. | **High-priority probe:** one-axis alignment can occur without a literal midpoint snap. |
| **`swSketchSnapsHVPoints`** | Snap horizontally or vertically relative to an existing sketch point. | **High-priority probe:** the observed change affected only the local `u` coordinate. |
| `swSketchSnapsLength` | Snap line length to grid increments, even if grid is hidden. | May change size despite precise endpoint input. |
| `swSketchSnapsGrid` | Snap to the grid. | The only Sketch Snap disabled in factory defaults; it replaces old `swSnapToPoints`. |
| `swSketchSnapToGridIfDisplayed` | Permit grid snap only while the grid is visible. | Conditional grid influence; replaces old document option. |
| `swSketchSnapsAngle` | Snap line orientation to angle steps. | May change direction and the resulting endpoint. |
| `swUserPreferenceDoubleValue_e.swSketchSnapsAngleValue` | Angle increment for angular snapping (a **Double**, not Boolean). | Record if testing angle snaps. Do not pass to `GetUserPreferenceToggle`. |

Other relevant **system** preferences include `swScaleSketchOnFirstDimension` and `swSketchOverdefiningDimsPromptToSetState`. The first can scale a sketch when its **first** dimension is specified. The second can permit an interactive decision about driving versus driven dimensions. Both need a deterministic policy for unattended replay. [R29], [R02]

### 2.3 What the documentation does and does not establish

**Recorded:** Official help calls 249 *Enable snapping*. It states that clearing this setting also clears *Automatic Relations*. It states that the factory Sketch Snaps are enabled except Grid. The UI has **Quick Snaps**, which can override individually disabled snap options. [R02], [R03]

**Recorded:** `ISketchManager.CreateCircle` documentation explicitly connects `AddToDB=True` with bypassing inferencing, automatic relations, and grid snaps for API-created geometry. `ISketchManager.CreateLine` also documents the AddToDB alternative. These are stronger API-specific statements than the UI option descriptions. [R07], [R30]

**Not established:** There is no documented, exhaustive internal precedence table for 249 versus 266–272, `swSketchAutomaticRelations`, and the extra HV/parallel settings during **COM-created** curves. The manual's word *all* describes UI snap control, not a tested master-mask truth table for every geometry API. There is also no documented factory default for every non-snap sketch preference or a guarantee that a user's changed profile matches factory values.

**Reconciliation with supplied seat evidence:** A new endpoint aligned to another line's midpoint **u** coordinate, while **v** stayed unchanged. That could be horizontal/vertical inference to an existing sketch point, midpoint inferencing, a relation, or a combination. The evidence identifies the effective **249-off remedy**. It does not isolate the internal source of the alignment. The 268-only failure is consistent with that distinction. Test this through a controlled factorial probe rather than assigning a cause.

**Restoration hazard:** Because the UI states that toggling **Enable snapping** can also affect Automatic Relations and the child Sketch Snaps, save the **whole relevant preference vector** before changing 249. Restore each prior value, then verify every readback. Restoring only 249 is not an adequate guarantee. This is a **reconstructed / untested** engineering rule inferred from the documented UI coupling. [R02], [R03]

## 3. T2 — programmatic inference and solve control

### 3.1 Exact interface ownership

| Attempted name | Result | Correct documented API |
|---|---|---|
| `ISketch.AddToDB` | **Seat evidence:** late-bound error 438 on SOLIDWORKS 2026. No such member appears in the reviewed `ISketch` member list. **Not a demonstrated removed API.** | `ISketchManager.AddToDB As Boolean` — since SOLIDWORKS 2008 FCS (rev. 16.0). [R06], [R10] |
| `ISketch.EnableSolving` | **Seat evidence:** late-bound error 438. No documented entry was recovered from the reviewed historic/current member lists. Do not claim proof that a method never existed in any build. | `ISketch.GetAutomaticSolve()` and `ISketch.SetAutomaticSolve(SolveFlag As Boolean) As Boolean`; or `ISketchManager.AutoSolve As Boolean`. [R08]–[R10] |
| `ISketchManager.InferenceMode` | Exists in older documentation but is marked **obsolete**. | Use `ISldWorks.Get/SetUserPreferenceToggle(swSketchInference)` or `ISketchManager.AddToDB`. [R31] |
| `ISketchManager.AutoInference` | Exists in older documentation but is **obsolete**. | Use the same system inference toggle. [R32] |
| `IModelDoc2.SetAddToDB` | Older documented API marked **obsolete**. | Use `ISketchManager.AddToDB` on new macros. [R33] |

**Recorded `AddToDB` behavior:** With `AddToDB=True`, the manager adds geometry directly and avoids the interactive endpoint/grid snap path. The manager documentation says to restore `False` after drawing. Paired with `DisplayWhenAdded=False`, it can improve drawing speed. However, it may prevent some creation steps from being available to Undo. Restore display settings and redraw if display was disabled. [R06], [R07], [R34]

**Community example:** The independent CodeStack troubleshooting guide uses a saved `swModel.SketchManager.AddToDB` Boolean, sets it to `True`, creates a line, and restores it. It also reports failures if interactive creation depends on whether the target area is visible. This is community evidence, not a SOLIDWORKS guarantee. [C01]

**Recorded solve control:** `ISketch.SetAutomaticSolve(False)` temporarily prevents automatic solves while making many sketch edits. Official help explicitly instructs callers to capture the previous state through `GetAutomaticSolve`, then restore it. The setter returns `Boolean` status. `ISketchManager.AutoSolve` offers a manager-level property. These are not snap switches. [R08], [R09]

**Do not call `ISketch.ConstrainAll` as a substitute for the source relations.** This method adds inferred relations to the whole sketch. In craft-bridge, use the **explicit** SolidCraft relation list. Otherwise SOLIDWORKS may introduce relations absent from the native design. [R35]

### 3.2 `SetUserPreferenceToggle` signature and conflicting historic prose

**Recorded declaration (2011, 2019 and 2024):**

```vb
' ISldWorks — exactly two arguments, no return value.
Sub SetUserPreferenceToggle( _
    UserPreferenceValue As Long, _
    OnFlag As Boolean)

' Typical call:
swApp.SetUserPreferenceToggle swSketchInference, False
```

`ISldWorks.GetUserPreferenceToggle(UserPreferenceValue As Long) As Boolean` is the matching system-option getter. **Do not** confuse it with `IModelDocExtension.SetUserPreferenceToggle`, which is a separate document-setting API and returns `Boolean`. [R04], [R11], [R12], [R36]

**Conflicting (official text):** The 2011 and 2013 API help pages declare a `Sub`, yet also contain a *Return Value* paragraph and a contradictory numeric description for `OnFlag` (`0` meaning ON). The same pages give a `True`-enables example. The 2019 and 2024 pages keep the void declaration without the conflicting return contract. Follow the typed declaration and verify the state through the getter. Do not copy the old prose as a return-value specification. [R11], [R12], [R37]

**Recorded capture/restore pattern:** An official SOLIDWORKS example saves `GetUserPreferenceToggle(swShowErrorsEveryRebuild)`, temporarily changes it, runs a rebuild, then restores the original value. It does not provide an exception-safe framework. The code below supplies that missing protection. [R38]

### 3.3 VBA: exact-line creation with preference capture and restoration

**Reconstructed / untested.** This standalone macro assumes an **open visible part with an active 2D sketch**. It uses the seat-proven `swSketchInference=False` strategy. It snapshots the snap/relations options most likely to change together. The code must be compiled and run on the 2026 seat before it becomes an emitter template. It deliberately does **not** disable `AutoSolve`, since an unsolved geometry readback would be misleading.

```vb
Option Explicit

Sub ProbeExactLineWithRestoration()
    Dim swApp As SldWorks.SldWorks
    Dim swModel As SldWorks.ModelDoc2
    Dim swMgr As SldWorks.SketchManager
    Dim swSeg As SldWorks.SketchSegment
    Dim prefIds As Variant
    Dim oldPrefs() As Boolean
    Dim i As Long
    Dim haveSnapshot As Boolean
    Dim oldAddToDB As Boolean
    Dim oldDisplay As Boolean
    Dim oldAutoSolve As Boolean
    Dim failureNumber As Long
    Dim failureText As String
    Dim restoreFailed As Boolean

    On Error GoTo Fail
    Set swApp = Application.SldWorks
    Set swModel = swApp.ActiveDoc
    If swModel Is Nothing Then Err.Raise vbObjectError + 801, , "No active document"
    Set swMgr = swModel.SketchManager
    If swMgr.ActiveSketch Is Nothing Then _
        Err.Raise vbObjectError + 802, , "No active sketch"

    ' System settings. Numeric values come from the seat's interop enums.
    prefIds = Array( _
        swSketchInference, swSketchAutomaticRelations, _
        swSketchInferFromModel, swSketchSnapsPoints, _
        swSketchSnapsCenterPoints, swSketchSnapsMidPoints, _
        swSketchSnapsQuadrantPoints, swSketchSnapsIntersections, _
        swSketchSnapsNearest, swSketchSnapsTangent, _
        swSketchSnapsPerpendicular, swSketchSnapsParallel, _
        swSketchSnapsHVLines, swSketchSnapsHVPoints, _
        swSketchSnapsLength, swSketchSnapsGrid, _
        swSketchSnapToGridIfDisplayed, swSketchSnapsAngle)

    ReDim oldPrefs(LBound(prefIds) To UBound(prefIds))
    For i = LBound(prefIds) To UBound(prefIds)
        oldPrefs(i) = swApp.GetUserPreferenceToggle(CLng(prefIds(i)))
    Next i
    oldAddToDB = swMgr.AddToDB
    oldDisplay = swMgr.DisplayWhenAdded
    oldAutoSolve = swMgr.AutoSolve
    haveSnapshot = True

    swApp.SetUserPreferenceToggle swSketchInference, False
    If swApp.GetUserPreferenceToggle(swSketchInference) Then _
        Err.Raise vbObjectError + 803, , "Cannot disable inference"

    ' 10 mm by 5 mm example. In-plane coordinates are in metres.
    Set swSeg = swMgr.CreateLine(0#, 0#, 0#, 0.01, 0.005, 0#)
    If swSeg Is Nothing Then _
        Err.Raise vbObjectError + 804, , "CreateLine returned Nothing"

    ' Before production use, read back endpoints and check them.
    GoTo RestoreState

Fail:
    failureNumber = Err.Number
    failureText = Err.Description
    Resume RestoreState

RestoreState:
    If haveSnapshot Then
        On Error Resume Next
        ' Restore master first, then individual snap settings.
        For i = LBound(prefIds) To UBound(prefIds)
            Err.Clear
            swApp.SetUserPreferenceToggle CLng(prefIds(i)), oldPrefs(i)
            If Err.Number <> 0 Then restoreFailed = True
        Next i
        Err.Clear
        swMgr.AddToDB = oldAddToDB
        If Err.Number <> 0 Then restoreFailed = True
        Err.Clear
        swMgr.DisplayWhenAdded = oldDisplay
        If Err.Number <> 0 Then restoreFailed = True
        Err.Clear
        swMgr.AutoSolve = oldAutoSolve
        If Err.Number <> 0 Then restoreFailed = True

        For i = LBound(prefIds) To UBound(prefIds)
            Err.Clear
            If swApp.GetUserPreferenceToggle(CLng(prefIds(i))) <> oldPrefs(i) Then _
                restoreFailed = True
            If Err.Number <> 0 Then restoreFailed = True
        Next i
        Err.Clear
        If swMgr.AddToDB <> oldAddToDB Then restoreFailed = True
        If swMgr.DisplayWhenAdded <> oldDisplay Then restoreFailed = True
        If swMgr.AutoSolve <> oldAutoSolve Then restoreFailed = True
        On Error GoTo 0
    End If

    If restoreFailed Then _
        Err.Raise vbObjectError + 805, , "Preference restoration failed"
    If failureNumber <> 0 Then Err.Raise failureNumber, , failureText
End Sub
```

**Macro review notes:**

1. The full 18-toggle snapshot is a cautious implementation choice, not a proven complete list of every coupled setting. A fresh 2026 snapshot after turning 249 OFF will show which flags actually change.
2. The restore order is **untested**. If restoring one child flag changes a previously restored master, the verification loop detects the mismatch and rejects the run. Do not silently retry or leave settings changed.
3. The code assumes a standard VBA module with SOLIDWORKS references, not VBScript. The 05 report warns that VBScript lacks VBA typed declarations. If a `.vbs` launcher is used, its syntax must be independent from the `.swp` macro.
4. For the alternate `AddToDB` route, retain the same cleanup structure and set `swMgr.AddToDB = True` before drawing. Do **not** combine both strategies in the first comparison probe. Change one variable at a time.
5. A restart/crash can bypass VBA cleanup. For unattended automation, a separate seat-side watchdog should detect changed application preferences at the next run. This recovery mechanism is **proposed**, not supplied by the API documentation.

### 3.4 VBA: per-sketch solve suspension

**Documented member calls; reconstructed combined sequence:**

```vb
Dim swSketch As SldWorks.Sketch
Dim wasSolving As Boolean
Dim ok As Boolean

Set swSketch = swModel.SketchManager.ActiveSketch
If swSketch Is Nothing Then Err.Raise 5, , "No active sketch"
wasSolving = swSketch.GetAutomaticSolve()
ok = swSketch.SetAutomaticSolve(False)
If Not ok Then Err.Raise 5, , "Could not disable sketch solve"

' Make a controlled batch of geometry / relation changes here.
' A production wrapper must restore state even when a call fails.

ok = swSketch.SetAutomaticSolve(wasSolving)
If Not ok Then Err.Raise 5, , "Could not restore sketch solve"
```

After the restore, call `GetConstrainedStatus` and read actual geometry. **Do not** assume that setting AutoSolve back to `True` resolves every deferred edit immediately. The published method page does not define a synchronous completion point. The macro should use an exception-safe cleanup structure like the previous example. [R08], [R17]

## 4. T3 — solve, readback and rebuild timing

### 4.1 `IDimension.SystemValue` and `SetSystemValue3`

**Recorded:** `IDimension.SystemValue` is a get/set value in system units, but official help marks it **obsolete** in favor of configuration-aware methods. The relevant modern setter is:

```vb
' IDimension; status is swSetValueReturnStatus_e.
Function SetSystemValue3( _
    NewValue As Double, _
    WhichConfigurations As Long, _
    Config_names As Object) As Long
```

`swSetValue_InThisConfiguration` selects the active configuration. The `Config_names` argument matters when using `swSetValue_InSpecificConfigurations`. For a simple current-configuration edit, the **reconstructed / untested** call is:

```vb
Dim status As Long
status = swDim.SetSystemValue3( _
    0.012, swSetValue_InThisConfiguration, Empty)
If status <> swSetValue_Successful Then _
    Err.Raise 5, , "Dimension edit was rejected. Status=" & CStr(status)
```

**Recorded status values:** `swSetValue_Successful=0`, `Failure=1`, `InvalidValue=2`, `DrivenDimension=3`, `ModelNotLoaded=4`, `FrozenFeatureOwner=5`. A success status concerns acceptance of the **value edit**, not proof that all dependent sketch coordinates and features reached the required final geometry. [R39], [R40]

**Recorded rebuild example:** Official *Change Dimension* examples assign a new dimension value, then explicitly call `EditRebuild3`. This supports a rebuild boundary for dependent model changes. Neither the property page nor these examples states that an in-sketch `SystemValue` assignment immediately finishes a sketch solve. Neither source gives a maximum delay or guarantees that an immediate `ISketchPoint.X` read is final. [R13]–[R15]

**Recorded API ownership:** The brief also mentions `ISketch.AddDimension`/`AddDimension2`. The researched dimension creators belong to `IModelDoc2` and `IModelDocExtension`, **not** to `ISketch`; no documented `ISketch.AddDimension2` member was recovered from the inspected `ISketch` index. [R10], [R23]–[R25], [R54]

**Seat evidence:** Some immediate re-reads after dimension edits showed no movement (`probe-dimkind edit-follow`). This is consistent with the absence of a documented synchronous contract. Do not classify it as a confirmed COM caching bug or as a proven engine defect without a controlled rebuild test.

### 4.2 Geometry readback: correct object interfaces

**Recorded:** `ISketchPoint.X/Y/Z` expose the sketch point coordinates. `ISketchSegment` is a generic interface. For a line, cast the returned segment to `ISketchLine` and use `GetStartPoint2` and `GetEndPoint2`. Equivalent named endpoints exist on `ISketchArc` and certain other curve-specific interfaces. [R18]–[R20], [R41]

**Official example pattern, adapted to a newly returned line; reconstructed / untested as one block:**

```vb
Dim line As SldWorks.SketchLine
Dim pStart As SldWorks.SketchPoint
Dim pEnd As SldWorks.SketchPoint

If swSeg Is Nothing Then Err.Raise 5, , "Missing segment"
If swSeg.GetType <> swSketchLINE Then _
    Err.Raise 5, , "Expected a line segment"
Set line = swSeg
Set pStart = line.GetStartPoint2
Set pEnd = line.GetEndPoint2
If pStart Is Nothing Or pEnd Is Nothing Then _
    Err.Raise 5, , "Line endpoint readback failed"

Debug.Print "Start (m):"; pStart.X; pStart.Y; pStart.Z
Debug.Print "End   (m):"; pEnd.X; pEnd.Y; pEnd.Z
```

**Recorded independent line readback:** `ISketch.GetLines2(CrossHatchOption As Short)` returns a flat double array, **12 doubles per line**. The six metadata fields precede `[start_x,start_y,start_z,end_x,end_y,end_z]`. The offset for line `i` is `12*i+6..11` for the endpoint coordinates (assuming the returned array's first index is zero). The official VBA example uses this layout. This gives a second readback path, separate from the original `ISketchLine` reference. It does not, by itself, guarantee the array is fresh before a solve. [R42], [R43]

**Coordinate-space correction:** SOLIDWORKS sketch-point coordinates are in the sketch's own coordinate frame. To compare with world-space STEP, transform them using the sketch transform. `ISketch.ModelToSketchTransform` returns the world-to-sketch transform. Its `Inverse` maps sketch coordinates to the model frame. Never compare a raw SolidCraft `(u,v)` against a global XYZ position without this conversion. [R44], [R43]

**Recorded constraint status:** `ISketch.GetConstrainedStatus()` reports values defined by `swConstrainedStatus_e`: `1=Unknown`, `2=Under`, `3=Fully`, `4=Over`, `5=No solution`, `6=Invalid solution`, `7=Autosolve off`. This is a useful additional diagnostic, not a substitute for endpoint and parameter checks. [R16], [R17]

### 4.3 Rebuilding and editing mode

| API or operation | Official statement | Implication for replay |
|---|---|---|
| `IModelDoc2.EditRebuild3() As Boolean` | Rebuilds only features needing rebuild in the active configuration. The method reference does **not** promise continued sketch edit mode. [R21] | Do not assume the active sketch survives the call. |
| `IModelDoc2.ForceRebuild3(TopOnly As Boolean)` | Requests a broader rebuild. [R45] | More work does not imply a more predictable sketch-edit state. |
| UI *Rebuild* while editing sketch | Help says rebuilding exits sketch edit mode. [R22] | Consistent with the observed 2026 `EditRebuild3` exit; not direct API wording. |
| `ISketchManager.InsertSketch(True)` | Toggles sketch editing and updates/rebuilds on exit. [R46] | A documented exit-and-rebuild path. |
| `ISketchManager.InsertSketch(False)` | Changes edit state without that requested update. [R46] | **Not** a documented rebuild-while-staying-in-sketch path. |
| `IModelDoc2.EditSketch` after selecting a sketch | Enters edit mode on the selected sketch. [R47] | Re-entry is available after a rebuild, but object references and selection must be checked. |
| `IModelView.GraphicsRedraw` | Updates drawing display. [R34] | Not documented as a solver trigger. Do not substitute redraw for rebuild. |

**No documented rebuild-and-stay-in-edit API was established by the public references consulted.** If the application needs a rebuild to make the edit visible, exit the sketch, rebuild, then select and re-enter it. Test the exact transition order on the seat. A direct per-sketch `SetAutomaticSolve(True)` might suffice for some in-edit cases, but documentation does not guarantee it.

### 4.4 Under-constrained geometry can move

**Recorded:** SOLIDWORKS describes under-defined sketches as having degrees of freedom. Users can move points and curves until additional dimensions/relations constrain them. The documentation does **not** specify which of several mathematically valid solutions the solver must choose after a new dimension. The presence of a driving dimension does not fix the absolute location of an otherwise free shape. [R48]

**Reconstructed replay rule:** Define the intended origin and relations before applying driving values. Do not depend on initial numeric coordinates to act as implicit fixed constraints. For a design where the source intentionally has free degrees of freedom, preserve that fact and verify feature geometry after the solve. To test parameter-driven motion, compare before/after geometry with more than one scalar dimension value.

**Subtle distinction from SolidCraft:** SolidCraft may treat a point with `exprs.x/y` referencing a parameter as fixed to an evaluated expression. A plain numeric `pos` is **not** automatically fixed. Its native `{type:"fix",p}` is a different constraint. These distinctions were confirmed by the supplied SolidCraft v0.43.0 source report, not by SOLIDWORKS help.

## 5. T4 — horizontal, vertical, zero and negative dimensions

### 5.1 Explicit APIs and signatures

**Recorded:** Both axis-specific methods are on `IModelDoc2`, not `ISketchManager`:

```vb
Function AddHorizontalDimension2(X As Double, Y As Double, Z As Double) As Object
Function AddVerticalDimension2(X As Double, Y As Double, Z As Double) As Object
```

Both require the appropriate entities selected **before** the call. The three input coordinates locate the dimension **text** and are in metres. They are not the dimension's numeric value. The methods return the newly created display dimension or can fail. They are available since at least SOLIDWORKS 2001Plus. [R23], [R24]

**Recorded official creation pattern:** The SOLIDWORKS *Create Specific Dimension in a Sketch (VBA)* example selects a sketch, calls `IModelDoc2.EditSketch`, selects two lines, calls `Part.Extension.AddSpecificDimension(..., swAngularDimension, error)`, then calls `Part.SketchManager.InsertSketch True`. This is an **official API example**, not evidence that the macro recorder emitted those statements. No authoritative sketch-recorder transcript showing a preference snapshot or `AddToDB` was recovered. [R51]

**Recorded:** The more recent typed alternative is:

```vb
' IModelDocExtension
Function AddSpecificDimension( _
    X As Double, Y As Double, Z As Double, _
    DimensionType As Long, ByRef Error As Long) As Object
```

For the relevant types, `swHorLinearDimension=11` and `swVertLinearDimension=12`. Its `Error` is `swAddSpecificDimension_e`, with `Success=0` and `DimTypeMismatch=1`. The method was added in SOLIDWORKS 2018 (rev. 26.0), and the official VBA example uses it for an angular sketch dimension. The 2019 and 2022 reference editions agree on horizontal/vertical support but differ on additional dimension types. [R25], [R49], [R50], [R51]

**Recorded:** `IDisplayDimension.Type2` reports a `swDimensionType_e` value. After creation, compare it against the requested 11 or 12. This check prevents a successful-looking but wrong type from entering the replay result. [R52]

**Reconstructed / untested code:** The following assumes that `firstPt` and `secondPt` are valid `SldWorks.SketchPoint` objects from the **active** sketch:

```vb
Dim firstPt As SldWorks.SketchPoint
Dim secondPt As SldWorks.SketchPoint
Dim dd As SldWorks.DisplayDimension
Dim dimError As Long
Dim selected As Boolean

' firstPt and secondPt are resolved before this excerpt.
swModel.ClearSelection2 True
selected = firstPt.Select4(False, Nothing)
If Not selected Then Err.Raise 5, , "First point selection failed"
selected = secondPt.Select4(True, Nothing)
If Not selected Then Err.Raise 5, , "Second point selection failed"

' Dimension-text position: 15 mm, 10 mm, 0 in model coordinates.
Set dd = swModel.Extension.AddSpecificDimension( _
    0.015, 0.01, 0#, swHorLinearDimension, dimError)
If dd Is Nothing Or dimError <> swAddSpecificDimension_Success Then _
    Err.Raise 5, , "Horizontal dimension creation failed"
If dd.Type2 <> swHorLinearDimension Then _
    Err.Raise 5, , "Unexpected dimension type"

' Existing documented alternative:
' Set dd = swModel.AddHorizontalDimension2(0.015, 0.01, 0#)
' For vertical: swVertLinearDimension / AddVerticalDimension2.
```

The point `Select4(Append As Boolean, Data As SelectData)` signature is official, but passing `Nothing` and selecting these exact points as a complete creation recipe need a 2026 seat probe. [R53]

### 5.2 Automatic dimension inference

**Recorded:** `IModelDoc2.AddDimension2(X,Y,Z)` creates a dimension from the current selection and text placement. Its API help warns that selection location matters and that a name-based `SelectByID2` selection can ignore the intended endpoint. When the entities do not identify a dimension unambiguously, it directs users to `IModelDocExtension.AddDimension`. Product help states that the Smart Dimension tool may change dimension type based on where its annotation is placed. [R54], [R55]

**Not established:** A precise, documented algorithm that converts `AddDimension2` selection geometry and text position into horizontal versus vertical versus parallel dimension type. No stable inference tolerance was found. For craft-bridge, do **not** infer a required `distance_x` from an `AddDimension2` result. Use the axis-specific methods and read back `Type2`.

**Recorded dialog hazard:** An older `IAddDimension2` API page advises disabling `swInputDimValOnCreate` to suppress the value-entry dialog during macro dimension creation. Preserve and restore that preference if the unattended seat displays it. [R55]

### 5.3 Zero-axis extent

**Seat evidence:** For a requested horizontal or vertical dimension whose selected points had **zero difference** along that axis, the corresponding axis-specific method returned `Nothing`, while `Err.Number` was `0`.

**Public documentation gap:** Neither axis-specific method page establishes that exact failure case, its numerical tolerance, or its result code. It is therefore **not** a general documented API rule. The seat result is sufficient to justify a preventive emitter check on that seat.

**Reconstructed fail-closed rule:** Before creating a dimension, compute `delta_u` for a horizontal dimension or `delta_v` for vertical **in sketch space**. If the target axis has zero extent within the calibrated tolerance, do not call the creator as if success were guaranteed. Decide whether the source means a geometric coincidence/alignment relation, a driven zero measurement, or an intentionally zero signed parameter. Do not introduce an arbitrary tiny offset to force a dimension into existence. Record the mismatch if no exact mapping exists.

### 5.4 Signed values

**Recorded, product UI:** SOLIDWORKS accepts zero and negative *sketch position dimensions*. A negative position value reverses the entity's side relative to its reference. It does not state that negative geometric **lengths** or **radii** are accepted. [R26]

**Not established:** Whether `IDimension.SystemValue = -d` preserves a signed `distance_x/distance_y`, normalizes it, moves the selected geometry, or rejects it for every horizontal/vertical dimension type. It also does not specify selection-order dependence. Probe this using `SetSystemValue3` status, readback, and endpoint positions. Test `+5 mm`, `0`, `-5 mm` with both selection orders and with a fixed reference point. A positive displayed magnitude is not sufficient evidence that the sign and geometry are correct.

## 6. Cross-report synthesis: implications for craft-bridge

The following findings depend on **comparison of the seven attached reports** and the new official API research. They are engineering deductions, not new seat measurements.

| Comparison | Additional finding | Proposed behavior |
|---|---|---|
| Brief 08 snap error vs. SolidCraft serialized sketch | SolidCraft commands can infer relations, but the native file **stores the resulting explicit constraints**. Replaying through SOLIDWORKS inference can create additional, unwanted constraints. | Suppress SOLIDWORKS inference while creating curves. Then add **only** serialized SolidCraft relations/dimensions. Verify relation inventory. |
| SolidCraft `distance_x`/`distance_y` vs. SOLIDWORKS axis dimensions | Both describe local 2D directions, but SolidCraft lengths are **mm** and SOLIDWORKS COM values are **m**. Sketch orientation can differ from world axes. | Convert mm→m; use axis-specific API; compare endpoints in **local sketch frame**; transform for STEP/world check. |
| SolidCraft `SPoint.pos` vs. `SPoint.exprs` | The supplied v0.43.0 source report cautions that serialized numeric `pos` might lag behind an authoritative parameter expression. SOLIDWORKS can separately lag a solve/rebuild after an edit. | Resolve SolidCraft expressions before authoring, then measure SOLIDWORKS after a proven solve/rebuild boundary. Never compare stale native `pos` with stale COM point snapshots. |
| SolidCraft point `fixed`/`fix` vs. SOLIDWORKS under-constraint | A numeric creation coordinate does not imply a fixed position in either solver. | Reconstruct the source's intended constraints. Do not indiscriminately fix every point merely to stop observed movement. |
| `swScaleSketchOnFirstDimension` vs. replay edit-follow failures | A system preference can rescale sketch geometry during first-dimension creation, independent of the sketch-inference problem. | Snapshot this setting. Probe first dimension with it ON/OFF. Consider temporarily disabling it when the native source does not request global scaling, then restore. |
| `AddToDB` bypass vs. Undo and large fixtures | Direct database insertion can improve speed but can change user Undo behavior. | Prefer the existing seat-tested 249-off path for initial correctness. Benchmark `AddToDB` on dedicated fixture documents before adopting it for larger suites. |
| `ISketch.GetConstrainedStatus` vs. geometry verification | A fully constrained state (`3`) is **neither proof of correct shape nor of correct parametric dependency**. It only reports the solver constraint state. | Gate on dimensions, geometry, relations, parameter dependency and post-rebuild status together. |
| 04/07 API call research vs. 05 VBA/WSH research | Valid SOLIDWORKS VBA code does not automatically work as VBScript. The 2026 macro seat and its launcher need separate syntax and status contracts. | Keep early-bound VBA in `.swp`. Keep external `.vbs` / PowerShell orchestration separate. Return machine-readable failure after any preference or solve restoration error. |
| Sketch-level solve vs. feature-level rebuild | `ISketch.SetAutomaticSolve` governs sketch-solving policy, while `EditRebuild3` concerns model-feature rebuilding. They are separate phases. | Report **sketch solve**, **feature rebuild**, and **STEP validation** as distinct checkpoints. |

### 6.1 Recommended emitter transaction (reconstructed / untested)

1. **Load source:** Parse native `solidcraft/57`, evaluate point expressions, and resolve curve and constraint IDs. Preserve the source's driving/reference dimension distinction.
2. **Capture seat state:** Record 249, 9, the snap children and relevant dimension preferences. Record `SketchManager.AddToDB`, `DisplayWhenAdded`, `AutoSolve`, and the active sketch/selection state.
3. **Create geometry:** Prefer the already observed 249-off strategy initially. Draw geometry using exact local coordinates converted to metres. Do not allow SOLIDWORKS to introduce inferred relations.
4. **Check initial coordinates:** Read the created curves using their specific interfaces. Compare all endpoints within a calibrated tolerance. Fail if there is a difference such as the ~0.6 mm one-axis snap.
5. **Add explicit relations:** Recreate the serialized SolidCraft constraints that have a verified SOLIDWORKS equivalent. Record which source constraints cannot map exactly.
6. **Add dimensions:** Select exact objects. Use explicit horizontal/vertical creation and check `IDisplayDimension.Type2`. Check the driving/reference state. Set the intended value via the documented setter and record its status.
7. **Solve and inspect:** Restore any suppressed AutoSolve state. Check `GetConstrainedStatus`. Read curve geometry again after the chosen solve boundary. Do not accept status `7`, `5`, or `6` as a solved result.
8. **Rebuild model:** Exit sketch and rebuild when necessary. If further sketch editing is required, re-enter the actual sketch feature and obtain fresh references.
9. **Validate:** Compare source and target sketch geometry, constraints, driven values, dependent parameter edits and final solids. Verify transformed STEP geometry independently where applicable.
10. **Restore preferences:** Restore the complete saved preference vector in guaranteed cleanup. Read it back. If any preference fails restoration, record a seat-contamination failure even if the model geometry matches.

**Do not adopt an automatic retry that changes dimensions, creates substitute relations, or perturbs zero-length geometry to get a passing result.** Such a retry can produce a valid SOLIDWORKS file that does not represent the source design.

## 7. Conflicts and unresolved interpretations

| Question | Evidence A | Evidence B | Current decision |
|---|---|---|---|
| Does `ISketch.AddToDB` exist? | Supplied 2026 `ISketch` call failed with error 438. | Official `ISketchManager.AddToDB` property exists since 2008. [R06] | **Wrong interface**, not evidence of a removed capability. |
| Does `ISketch.EnableSolving` exist? | Supplied call failed with error 438. | Official sketch APIs are `GetAutomaticSolve` and `SetAutomaticSolve`; manager also has `AutoSolve`. [R08], [R09] | Replace attempted member with the documented controls. No removal date established. |
| Is `swSketchInference=249` a master? | UI says Enable snapping affects all snap types and clearing it clears Automatic Relations. [R02], [R03] | No documented COM truth table; 268 alone did not stop the supplied snap. | Treat 249 as effective seat-wide master control, but do not infer exact per-child precedence. |
| Does `SetUserPreferenceToggle` return a success value? | 2011–2013 help contains a *Return Value* paragraph and contradictory `OnFlag` prose. [R37] | The formal declaration is `Sub`; 2019+ documentation and 2026 interop show void. [R11], [R12] | Use the `Sub` form; read back setting, not return code. |
| Is setting `SystemValue` synchronous? | The setter's API page does not specify a completed sketch solve. [R13] | Official examples issue a rebuild; supplied seat observed stale immediate point movement. [R14], [R15] | No immediate readback guarantee. Use explicit solve/rebuild checkpoints. |
| Does `EditRebuild3` always exit sketch editing? | API page documents rebuild scope but not edit-mode side effect. [R21] | UI rebuild documentation says it exits editing; supplied 2026 seat saw this. [R22] | Plan for exit on target seat. Do not assert a universal COM guarantee. |
| Are endpoint arrays on generic `ISketchSegment`? | Brief 08 names generic `GetStartPoint`/`GetEndPoint` forms. | Member lists and official example place object endpoint APIs on `ISketchLine` and other specific interfaces. [R18]–[R20] | Cast by segment kind. For line-array readback use `ISketch.GetLines2`. |
| Can zero-axis extent be dimensioned? | Supplied 2026 seat returned `Nothing`, `Err=0`. | Public method pages do not state this failure threshold. [R23], [R24] | Guard the zero case using seat-calibrated tolerance. Do not present threshold as official. |
| Can a negative linear dimension be set by API? | Product help permits negative position dimensions. [R26] | `IDimension` API help does not define signed value behavior for each dimension kind. [R13], [R39] | Probe sign, selection order, and geometry before exact signed replay. |
| Which object owns the dimension-creation calls? | Brief mentions `ISketchManager` / `IModelDoc2`. | Official signatures place them on `IModelDoc2`; the type-controlled call is on `IModelDocExtension`. [R23]–[R25] | Emit calls on the documented owners. |

## 8. Still needs a SOLIDWORKS 2026 seat probe

| Priority | Probe and controlled variations | Observables | Pass / fail criterion |
|---|---|---|---|
| **P0** | Reproduce 0.6 mm alignment: 249 ON/OFF, 268 ON/OFF, 9 ON/OFF, `HVPoints` ON/OFF, `HVLines` ON/OFF, then combinations. | Input/output `(u,v)`; explicit relations created; all preferences before/after. | Exact coordinates within project tolerance; identify the minimal effective flags without assuming cause. |
| **P0** | Test `SketchManager.AddToDB=True` **alone** for `CreateLine`, `CreateCircleByRadius`, `CreateArc` and at least one spline. Compare to 249 OFF. | Endpoint/curve parameters, created relations, Undo behavior, performance. | No snap drift or unwanted relations. Document any Undo limitation. |
| **P0** | Turn 249 OFF and then restore only 249 in an isolated fixture; compare the entire preference vector. Repeat with complete snapshot restoration. | Before, after-disable, after-restore Boolean arrays. | All settings exactly restored. If not, adopt full-vector restore and tested ordering. |
| **P0** | Test `ISketch.GetAutomaticSolve` / `.SetAutomaticSolve`, manager `AutoSolve`, and their interaction. | Method availability, return values, status `7`, points before/after restore. | No use of absent members; restored state identical and final geometry solved. |
| **P0** | Create one anchored and one free sketch. Add driving X/Y dimensions and change values while edit is active. | Immediate endpoint reads, `GetLines2`, `GetConstrainedStatus`, after-exit geometry. | Define the first reliable readback stage. Document free-entity movement. |
| **P0** | Call `EditRebuild3`, `ForceRebuild3`, and `InsertSketch(True/False)` from active editing in separate fresh fixtures. | Active sketch before/after, COM object validity, geometry, feature rebuild. | Identify safe exit/reopen sequence. Do not assume stay-in-edit. |
| **P0** | Explicit `AddHorizontalDimension2`/`AddVerticalDimension2` and `AddSpecificDimension(11/12)` on two sketch points. Test both selection orders. | `Nothing`, error, `Type2`, measured value, signed delta. | Exact requested axis kind; fail on wrong type. |
| **P0** | Zero axis and near-zero axis: delta 0, small positive and negative deltas around the seat's geometry tolerance. | Return object, `Err.Number`, `Type2`, point movement, final status. | Calibrate guard threshold. Never add artificial geometry to pass. |
| **P0** | `SetSystemValue3` with +5, 0, -5 mm on horizontal/vertical **position** dimensions, with anchored reference point. | Setter status, SystemValue, display magnitude, endpoint order and coordinates. | Exact sign/direction semantics measured. Fail on rejected/normalized values. |
| **P1** | First sketch dimension with `swScaleSketchOnFirstDimension` ON and OFF. | Initial/final point coordinates, all dimensions, global preference. | Confirm no unintended model scaling. |
| **P1** | Add driving dimension to fully defined sketch and to already constrained geometry; vary overdefinition dialog preference and `swInputDimValOnCreate`. | Blocking UI, driven state, creation status, automation completion. | No modal interruption or silent conversion to reference dimension. |
| **P1** | Read line points via `ISketchLine` object, fresh `GetSketchSegments`, and `ISketch.GetLines2` at each solve stage. | Coordinate agreement and object identity. | Independent readbacks converge to expected final positions. |
| **P1** | Test named 2D sketch on `XY`, `XZ`, `YZ` and a custom plane. | Sketch-local `(u,v)` and transformed world XYZ. | Reconstructed plane frame and sign match STEP geometry. |
| **P1** | Test an input point with SolidCraft `exprs` and a changed parameter after native save/reload. | Stored `pos`, expression-evaluated coordinates, SOLIDWORKS post-edit values. | Compare authoritative evaluated geometry; no false stale-pos failure. |
| **P1** | Trigger an exception mid-drawing, restoration exception, and launcher termination. | Every saved preference and manager state; verdict/exit signal. | Fail-closed state restoration and explicit recovery message. |

## 9. Source index

**Official SOLIDWORKS sources** (documentation year is part of the citation):

[R01]: https://help.solidworks.com/2025/English/api/swconst/SO_SketchRelationsSnaps.htm "System Options > Sketch > Relations/Snaps (2025)"
[R02]: https://help.solidworks.com/2026/english/SolidWorks/sldworks/r_relations_snaps_options.htm "Relations/Snaps Options (2026)"
[R03]: https://help.solidworks.com/2026/English/SolidWorks/acadhelp/c_Inferencing.htm "Inferencing (2026)"
[R04]: https://help.solidworks.com/2025/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISldWorks~GetUserPreferenceToggle.html "GetUserPreferenceToggle"
[R05]: https://help.solidworks.com/2025/English/api/swconst/SO_SketchRelationsSnaps.htm "Obsolete snap enumerators"
[R06]: https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isketchmanager~addtodb.html "SketchManager.AddToDB, undo and restoration"
[R07]: https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isketchmanager~createcircle.html "SketchManager.CreateCircle"
[R08]: https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketch~SetAutomaticSolve.html "Sketch.SetAutomaticSolve"
[R09]: https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~AutoSolve.html "SketchManager.AutoSolve"
[R10]: https://help.solidworks.com/2020/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketch_members.html "ISketch members"
[R11]: https://help.solidworks.com/2019/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isldworks~setuserpreferencetoggle.html "SetUserPreferenceToggle (2019)"
[R12]: https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISldWorks~SetUserPreferenceToggle.html "SetUserPreferenceToggle (2024)"
[R13]: https://help.solidworks.com/2023/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IDimension~SystemValue.html "Dimension.SystemValue"
[R14]: https://help.solidworks.com/2010/english/api/sldworksapi/change_dimension_example_vb.htm "Change Dimension example VBA"
[R15]: https://help.solidworks.com/2025/english/api/sldworksapi/Change_Dimension_Example_CSharp.htm "Change Dimension example C#"
[R16]: https://help.solidworks.com/2020/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketch_members.html "ISketch.GetConstrainedStatus and GetAutomaticSolve"
[R17]: https://help.solidworks.com/2026/English/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swConstrainedStatus_e.html "swConstrainedStatus_e"
[R18]: https://help.solidworks.com/2022/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchSegment_members.html "ISketchSegment members"
[R19]: https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchPoint.html "ISketchPoint and specific curve endpoints"
[R20]: https://help.solidworks.com/2022/english/api/sldworksapi/Get_All_Elements_of_Sketch_Example_VB.htm "Get all sketch elements VBA example"
[R21]: https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imodeldoc2~editrebuild3.html "EditRebuild3"
[R22]: https://help.solidworks.com/2020/English/SolidWorks/sldworks/t_Exiting_Sketches.htm "Exiting Sketches"
[R23]: https://help.solidworks.com/2018/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDoc2~AddHorizontalDimension2.html "AddHorizontalDimension2"
[R24]: https://help.solidworks.com/2015/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDoc2~AddVerticalDimension2.html "AddVerticalDimension2"
[R25]: https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDocExtension~AddSpecificDimension.html "AddSpecificDimension"
[R26]: https://help.solidworks.com/2023/english/SolidWorks/sldworks/t_zero_negative_dimensions.htm "Zero and negative sketch dimensions"
[R27]: https://help.solidworks.com/2026/english/SolidWorks/sldworks/c_Sketch_Settings_Menu.htm?id=28.1 "Sketch Settings Menu"
[R28]: https://help.solidworks.com/2026/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swUserPreferenceToggle_e.html "Preference enum (2026)"
[R29]: https://help.solidworks.com/2026/English/api/swconst/SO_Sketch.htm "System Options > Sketch (2026)"
[R30]: https://help.solidworks.com/2021/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~CreateLine.html "SketchManager.CreateLine"
[R31]: https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~InferenceMode.html "Obsolete InferenceMode"
[R32]: https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchManager~AutoInference.html "Obsolete AutoInference"
[R33]: https://help.solidworks.com/2026/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDoc2~SetAddToDB.html "Obsolete ModelDoc2.SetAddToDB"
[R34]: https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~DisplayWhenAdded.html "SketchManager.DisplayWhenAdded"
[R35]: https://help.solidworks.com/2022/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketch~ConstrainAll.html "Sketch.ConstrainAll"
[R36]: https://help.solidworks.com/2025/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~SetUserPreferenceToggle.html "Document preference setter"
[R37]: https://help.solidworks.com/2011/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISldWorks~SetUserPreferenceToggle.html "2011 conflicting setter prose"
[R38]: https://help.solidworks.com/2024/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDoc2~ShowFeatureErrorDialog.html "Save/set/restore example"
[R39]: https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.interop.sldworks~SolidWorks.interop.sldworks.IDimension~SetSystemValue3.html "SetSystemValue3"
[R40]: https://help.solidworks.com/2026/english/api/swconst/SOLIDWORKS.Interop.swconst~SOLIDWORKS.Interop.swconst.swSetValueReturnStatus_e.html "SetValue return status"
[R41]: https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchPoint~X.html "SketchPoint.X"
[R42]: https://help.solidworks.com/2025/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketch~GetLines2.html "Sketch.GetLines2"
[R43]: https://help.solidworks.com/2022/english/api/sldworksapi/Get_Lines_in_Sketch_Example_VB.htm "Get Lines in Sketch VBA example"
[R44]: https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketch~ModelToSketchTransform.html "Sketch model transform"
[R45]: https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imodeldoc2~forcerebuild3.html "ForceRebuild3"
[R46]: https://help.solidworks.com/2023/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchManager~InsertSketch.html "InsertSketch"
[R47]: https://help.solidworks.com/2020/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDoc2~EditSketch.html "EditSketch"
[R48]: https://help.solidworks.com/2026/English/SolidWorks/sldworks/c_Sketch_Status_Conventions.htm?format=P&value= "Under-constrained sketch behavior"
[R49]: https://help.solidworks.com/2026/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swDimensionType_e.html "Dimension type values"
[R50]: https://help.solidworks.com/2026/english/api/swconst/SolidWorks.Interop.swconst~SolidWorks.Interop.swconst.swAddSpecificDimension_e.html "AddSpecificDimension statuses"
[R51]: https://help.solidworks.com/2020/English/api/sldworksapi/Create_Specific_Dimension_Example_VB.htm "Official specific-dimension VBA example"
[R52]: https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IDisplayDimension~Type2.html "DisplayDimension.Type2"
[R53]: https://help.solidworks.com/2023/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchPoint~Select4.html "SketchPoint.Select4"
[R54]: https://help.solidworks.com/2020/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDoc2~AddDimension2.html "Generic AddDimension2"
[R55]: https://help.solidworks.com/2021/english/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDoc2~IAddDimension2.html "Dimension selection and input dialog"

**Community source:**

[C01]: https://jiaqiwang969.github.io/solidworks-GPT/docs/codestack/solidworks-api/troubleshooting/macros/create-sketch-segments-error/ "CodeStack: sketch segment creation and AddToDB"

**Attached source reports:** `08-sketch-inference-and-solve-timing.md` (supplied 2026 seat findings); `SolidCraft_v043_Nested_Schema_Research(1).md` (native sketch constraint and point-expression semantics); `06-solidcraft-command-research-report(1).md` (command versus serialized design); `SolidCraft_57_Format_Research(1).md` (native format); `04-solidworks-method-pages-research(1).md` and `solidworks-api-usage-archaeology-07(1).md` (API readbacks and COM calling shapes); `05-vba-dialects-and-seat-automation-research(1).md` (VBA versus WSH); `SolidWorks_API_Research(1).md` (baseline SOLIDWORKS API research).

**Final research status:** The documented member ownership, call shapes, system-option mapping, and dimension types are pinned. The **exact solver timing**, **restoration coupling**, **negative-axis dimension behavior**, and **2026 COM readback boundaries** remain seat-dependent. Do not promote any reconstructed policy to a verified emitter rule without the P0 probes.
