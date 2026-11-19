# VBScript/IDispatch COM array marshaling 11

**Filed:** 2026-11-19, produced by the owner's research agent, delivered
as a chat attachment and filed verbatim. Answer to
`research/inputs/11-vbscript-com-array-marshaling.md`.
Provenance note: the report's labels are its own (Recorded/Community/
Conflicting/Reconstructed-untested/Seat evidence). HEADLINE: archived
Microsoft KB 165967 CONFIRMS the marshaling hypothesis - VBScript can
inspect bounds of a non-Variant SAFEARRAY but its element indexing
supports only SAFEARRAY(VARIANT) - so the run-16 centroid failure is
explained by a typed SAFEARRAY, subject to capturing the container
subtype first (VarType dec/hex). For Each is NOT an approved workaround;
the ranked fix is a typed adapter (VBA or .NET/PowerShell) or moving the
readback to a typed host, with named refusals until then. API
corrections: `GetMassProperties2(Accuracy, ByRef Status, UseSelected)`
carries a ByRef (the seat's known uncallable shape) and has NO
configuration argument; the two-argument `GetBodies2` owner is
`IPartDoc`, not `IModelDoc2`; `IGet*` members are in-process only. The
companion `tools/probe-typed-arrays.vbs` is filed with this report.

---

# Research 11: VBScript / IDispatch COM array marshaling and SOLIDWORKS readbacks

**Research date:** 19 November 2026  
**Target:** `craft-bridge` acceptance layer, SOLIDWORKS 2026, `tools/check-part-logic.vbs` and `tools/check-part.vbs`  
**Basis:** supplied `11-vbscript-com-array-marshaling.md`, previous research 05 and 10, Microsoft automation references, SOLIDWORKS API Help, and source-labeled outside evidence  
**Execution:** public-source research only. No Windows Script Host, SOLIDWORKS seat, registered COM fixture, or production verifier was executed in this investigation.

SOLIDWORKS is a trademark of Dassault Systemes. Names identify its public interfaces; no affiliation is implied.

## Evidence convention

- **Recorded:** a published official API contract, official example, or archived original vendor documentation. An archived Microsoft Knowledge Base article is marked as such.
- **Community:** third-party code, forum investigation, or accepted community answer. It establishes an observation or approach, not universal runtime behavior.
- **Conflicting:** the source text, source versions, or supplied seat observation disagree. Both remain visible.
- **Reconstructed / untested:** a diagnostic, COM hypothesis, or proposed implementation that requires a target Windows/SOLIDWORKS probe.
- **Seat evidence (source-supplied):** the results reported in brief 11. These were not independently reproduced.

In signatures, `System.Object` is a .NET interop declaration. It does **not** establish whether a returned COM `VARIANT` contains `SAFEARRAY(VARIANT)`, `SAFEARRAY(Double)`, or another element type. The array's reported values and the on-wire storage type are separate claims.

## Findings at a glance

| Question | Finding | Evidence | Consequence |
|---|---|---|---|
| Can VBScript index `SAFEARRAY(Double)` received via COM? | **No**, according to the archived original Microsoft KB 165967: the engine indexes `SAFEARRAY(VARIANT)` only. | **Recorded** [M1], supported by SAS [M2] | The centroid failure has a strong explanation. Do not add `Set`, `CDbl`, or default-property guesses around `raw(i)`. |
| Does `IsArray` plus successful `LBound` prove element access? | **No**. Microsoft explicitly describes bounds inspection on non-Variant arrays. | **Recorded** [M1] | Treat metadata checks and element checks as different stages. |
| Does run-16 prove that the actual SOLIDWORKS return is `VT_ARRAY | VT_R8`? | **No**. It strongly suggests an unsupported array representation, but lacks `VarType(raw)` and native `VARIANT.vt`. | **Seat evidence** and **Reconstructed / untested** [M1][M3] | Capture the container subtype before selecting a remedy. |
| Can `For Each` be used as the fix? | No verified proof. It works for ordinary Variant arrays and COM collections, but need not permit reading a non-Variant SAFEARRAY. | **Reconstructed / untested** [M1][M5][M6] | Test it, but do not use it in acceptance until demonstrated. |
| Is `GetMassProperties2` a direct VBScript fallback? | It has `ByRef Status As System.Integer`, plus an array return. The seat previously failed comparable typed ByRef outputs. | **Recorded** [S3], **seat evidence** | Treat direct VBScript use as unqualified. Use a typed COM client or VBA for the controlled cross-check. |
| Is the same issue relevant to relation and dimension inventory? | Yes. Several SOLIDWORKS methods return arrays, but the API pages do not pin their `SAFEARRAY` element type. | **Recorded** [S7]-[S15] | Probe each return family. Never extrapolate from one working array. |
| Is `IModelDoc2.GetBodies2(BodyType, BVisibleOnly)` correct? | **No verified method with that owner.** The documented two-argument member is `IPartDoc.GetBodies2`. | **Recorded** [S12] | Correct the interface in the acceptance-layer call table. |
| Does `PowerShell -> cscript` change the array subtype? | No official source establishes such a transformation when PowerShell only launches `cscript`. | **Reconstructed / untested** [M7][M8] | Compare direct and scheduled launches to isolate host settings. |
| What is the strongest implementation fix? | Use a typed client/adapter to read the source array and return explicit `SAFEARRAY(VARIANT)` values to VBScript, or do the verification in VBA/.NET. | **Recorded design guidance** [M1][M2], **Reconstructed / untested** implementation | Fail closed until both component type and numeric cross-checks pass. |

## T1. Array marshaling rules

### 1.1 The Microsoft rule is documented, not speculation

**Recorded - archived original Microsoft Knowledge Base Q165967.** The article specifically addresses a COM method returning a non-Variant SAFEARRAY to VBScript. It states that VBScript permits bounds checking and passing such arrays to other automation methods, but its element indexing supports only `SAFEARRAY(VARIANT)`. The recommendation is to return a `SAFEARRAY` whose entries are `VARIANT` values. Microsoft provides a contrasting `SAFEARRAY(BSTR)` versus `SAFEARRAY(VARIANT)` ATL example. [M1]

**Applicability limit.** The KB article lists early VBScript engines (1.1 through 3.0). It is strong historical evidence of the language design, not a published test of the Windows Script Host engine installed with SOLIDWORKS 2026. Independent SAS developer documentation describes the same limitation and an explicit marshaling adapter. [M2]

**Community corroboration.** A July 2026 Microsoft Q&A answer describes `ADODB.Stream.Read(1)` returning a byte SAFEARRAY. It reports that bounds inspection succeeds while `buf(0)` raises Type mismatch. Microsoft Q&A answers are community evidence, not a Microsoft product specification. [M9]

**Conflicting diagnostic text.** Q165967 quotes a different failure message, equivalent to object/member access not being supported. Run 16 reports VBScript err 13 (`Type mismatch`). No official source found maps each unsupported `SAFEARRAY` base type to a single required VBScript error number. **Do not implement an error-code-only subtype detector.** [M1]

### 1.2 On-wire types and expected diagnostics

A `VARIANT` uses a type field (`VARTYPE`). An array value combines `VT_ARRAY` (`0x2000`, decimal 8192) with the element type. Common element tags are `VT_R8=5`, `VT_BSTR=8`, `VT_DISPATCH=9`, `VT_VARIANT=12`, `VT_UNKNOWN=13`, and `VT_UI1=17`. `VT_BYREF=0x4000` is a separate modifier. [M3][M4]

| COM array representation | Expected VBScript `VarType(raw)` value, if preserved | VBScript `raw(i)` | Status |
|---|---:|---|---|
| `VT_ARRAY | VT_VARIANT` | `8192 + 12 = 8204` | Direct indexing supported | **Recorded** [M1][M10] |
| `VT_ARRAY | VT_R8` (`double`) | `8192 + 5 = 8197` | Direct indexing not supported by documented VBScript rule | **Recorded** rule, **Reconstructed** exact diagnostic [M1][M10] |
| `VT_ARRAY | VT_BSTR` (`string`) | `8192 + 8 = 8200` | Not supported | **Recorded** example [M1] |
| `VT_ARRAY | VT_DISPATCH` | `8192 + 9 = 8201` | Not guaranteed. Object members might need a typed client | **Reconstructed / untested** [M1][M3] |
| `VT_ARRAY | VT_UNKNOWN` | `8192 + 13 = 8205` | Not guaranteed. `IUnknown` alone is not a scriptable object | **Reconstructed / untested** [M1][M3] |
| `VT_ARRAY | VT_UI1` (`byte`) | `8192 + 17 = 8209` | Not supported by the general rule | **Recorded** rule; **Community** matching example [M1][M9] |

These are expected `VarType` results when the VBScript runtime retains the corresponding array subtype. The actual result must be logged. The table is not a claim that each SOLIDWORKS property returns that exact COM type.

**Recorded.** In the Visual Basic `VarType` model, `vbArray=8192`, `vbDouble=5`, `vbVariant=12`. The array flag combines with the element code. A regular `Array(...)` has the expected value `8204`. The function can inspect the container without accessing an individual element. [M10][M11]

**Recorded and source-supplied.** `IsArray(raw)`, `LBound(raw)` and `UBound(raw)` can work for a non-Variant SAFEARRAY. They validate the presence of an array and its bounds, not element access. `TypeName(raw)` can identify the array type, with the archived KB using `Variant()` for a Variant-element array. [M1]

**Reconstructed / untested diagnostic:** capture these independently: `VarType(raw)`, `TypeName(raw)`, `IsArray(raw)`, `LBound(raw)`, `UBound(raw)`, `raw(0)`, `VarType(raw(0))`, and `For Each`. Never put the whole sequence under a single unchecked `On Error Resume Next`. A failure while evaluating `raw(0)` prevents its value from reaching `VarType` or `TypeName`.

### 1.3 `For Each`: arrays are not COM collections

**Recorded.** A `For Each` loop can enumerate an ordinary array or a COM collection. For a COM collection, the `_NewEnum` property exposes `IEnumVARIANT`, which the scripting engine uses automatically. This object-collection mechanism does not imply that a raw `SAFEARRAY` has an `IEnumVARIANT` interface. [M5][M6][M12]

**Unverified.** There is no evidence in the inspected references that `For Each` can read `SAFEARRAY(Double)` elements when direct indexing fails. Since Q165967 says non-Variant array *contents* are not manipulable, `For Each` is at best a diagnostic experiment. If it happens to work on the target seat, verify enumeration count, order, subtypes and numerical values before qualifying it. [M1]

**Ordering:** For a one-dimensional native Variant array, traversal in ascending index order is the usual VB family behavior. The sources checked here do not establish a universal ordering contract for every COM object enumerator. Compare values at known indices when the array is indexable. Do not assign semantic X/Y/Z positions using an unvalidated object-enumerator order.

**If a COM object lacks a usable `_NewEnum`:** `For Each` over that object may fail. That is a different condition from a SAFEARRAY element access failure. The precise error number is host- and object-dependent. Do not generalize a single number. [M6]

### 1.4 `ByRef`, `ByVal`, coercion and VBA

- **Recorded:** the VBScript language stores variables as `Variant` values. A VBA `Dim x(2) As Double` declaration has no VBScript equivalent. VBA can index typed arrays. [M7][M11]
- **Recorded:** Microsoft KB 165967 says a non-Variant SAFEARRAY can be forwarded unchanged to another compatible automation component. A typed client can read the array and repackage each value as a `VARIANT` entry. [M1]
- **Reconstructed / untested:** changing a VBScript function parameter from `ByRef` to `ByVal` does not, by itself, prove element-by-element conversion. A copy of the outer Variant can retain the same inner typed SAFEARRAY.
- **Reconstructed / untested:** `Array(raw)` makes a new array containing **one element** whose value is the original `raw` array. It does not flatten or retype the original elements. A `For Each`/copy loop requires successful element reads and therefore does not solve the access problem on its own.
- **Recorded for VBScript:** `CVar` is not an ordinary VBScript conversion function. `Val`, VBA typed declarations and `Dim ... As Double()` must not appear in a `.vbs` output. `CStr`/`CDbl` act on values, not on an inaccessible element expression. [M13]
- **Recorded:** COM `out`/`ByRef System.Integer` parameters are distinct from a method's `System.Object` return. A VBScript `Variant` with subtype Long does not automatically establish that the seat's IDispatch binder can accept a typed `ByRef Integer`. The supplied seat evidence already reports comparable failures. Classify calls with such outputs separately. [S3]

**PowerShell and hosting.** Microsoft documents that `cscript.exe` hosts VBScript and that `CreateObject` obtains a COM server. No inspected Microsoft reference says that launching `cscript.exe` from PowerShell changes `IDispatch::Invoke` result arrays into a different element type. A scheduled task may change bitness, account, session, registration visibility, or the application instance. Those variables can cause different COM behavior, but each needs a separate test. This is a reconstructed distinction, not a documented explanation of run 16. [M7][M8]

## T2. SOLIDWORKS array-returning API contracts

### 2.1 Member matrix

Every declaration below is from SOLIDWORKS published interop pages. `Object` means **declared return type**, not `SAFEARRAY(VARIANT)`.

| Owner / method | Recorded declaration, shortened | Recorded returned payload | Typed ByRef output? | Safe VBScript policy |
|---|---|---|---|---|
| `IMassProperty.CenterOfMass` | `ReadOnly Property CenterOfMass As System.Object` | 0-based `Double[3]`, `[x,y,z]` | No | Call is valid. Inspect container subtype. Reject unreadable entries. [S1] |
| `IMassProperty2.CenterOfMass` | Same | Three doubles, selected body/component scope | No | Same risk. Set scope/units. Recalculate when documented conditions require it. [S2] |
| `IModelDocExtension.GetMassProperties2` | `GetMassProperties2(Accuracy As Integer, ByRef Status As Integer, UseSelected As Boolean) As Object` | 0-based twelve-double mass array | **Yes** | Not a qualified *direct VBScript* fallback on this seat. Use VBA/.NET and check status. [S3] |
| `IBody2.GetMassProperties` | `GetMassProperties(Density As Double) As Object` | 12 positions, solid-body centroid at `[0..2]` | No | Method can be called. Indexing still unqualified. This is per body, not an aggregate. [S4] |
| `IDimension.GetSystemValue3` | `GetSystemValue3(WhichConfigurations As Integer, Config_names As Object) As Object` | Configuration values; array demonstrated in VBA example | No | `Config_names` is input, not ByRef. Test return subtype. [S5] |
| `IDimension.SetSystemValue3` | `SetSystemValue3(NewValue As Double, WhichConfigurations As Integer, Config_names As Object) As Integer` | Scalar status enum | No | Callable in principle. Validate status and dimension state. [S6] |
| `ISketchRelationManager.GetRelations` | `GetRelations(Filter As Integer) As Object` | Array of relation objects | No | Type inspection required. `GetRelationsCount(Filter)` is scalar and can be used separately. [S7] |
| `ISketchRelation.GetDefinitionEntities2` | `GetDefinitionEntities2() As Object` | Array of original defining entities, potentially null entries | No | Type inspection; null/unsupported entities fail closed. [S8] |
| `ISketchRelation.GetEntities` | `GetEntities() As Object` | Array of relation entities, sometimes internal sketch entities | No | Type inspection. Not a replacement for definition entities. [S9] |
| `ISketchRelation.GetEntitiesType` | `GetEntitiesType() As Object` | Array of enum-coded entity types | No | Numeric array may be typed. Probe rather than assume indexing. [S10] |
| `ISketchRelation.GetEntitiesCount` | `GetEntitiesCount() As Integer` | Scalar relation-operand count | No | May be called without element access. [S11] |
| `IPartDoc.GetBodies2` | `GetBodies2(BodyType As Integer, BVisibleOnly As Boolean) As Object` | Array of `Body2` entities, order not stable | No | Correct two-argument owner. The count path succeeding says nothing about `raw(i)` or object access. [S12] |
| `IMassProperty.PrincipleAxesOfInertia` | `PrincipleAxesOfInertia(Axis As Integer) As Object` | `Double[3]` vector for axis `0`, `1` or `2` | No | Same typed-double risk as centroid. [S13] |
| `IMassProperty.PrincipleMomentsOfInertia` | `ReadOnly Property PrincipleMomentsOfInertia As Object` | 0-based `Double[3]` | No | Same risk. [S14] |
| `IMassProperty.GetMomentOfInertia` | `GetMomentOfInertia(WhereTaken As Integer) As Object` | 0-based `Double[9]` in tensor order | No | Same risk. [S15] |
| `ISldWorks.GetFirstDocument` | `GetFirstDocument() As Object` | A **single model document** | No | Object traversal, not array indexing. Use `Set` and check `Nothing`. [S16] |
| `IModelDoc2.GetNext` | `GetNext() As Object` | A **single next document** | No | Same. [S17] |

**Owner correction.** The brief uses `IModelDoc2.GetBodies2(BodyType, BVisibleOnly)`. SOLIDWORKS identifies that two-argument method as `IPartDoc.GetBodies2`. `IComponent2.GetBodies2(BodyType)` has one argument and is obsolete in favor of `GetBodies3`, which itself has an `out BodiesInfo` argument. A `ModelDoc2` supplies common document services. The part-specific body API belongs to `IPartDoc`. [S12][S18]

**Specific status and configuration correction.** `IModelDocExtension.GetMassProperties2` takes `Accuracy, Status, UseSelected`. There is **no configuration-name input in this signature**. Select the intended active configuration through the document before using it. Do not substitute obsolete `IModelDoc2.GetMassProperties2(Status)` or `IModelDocExtension.GetMassProperties(Accuracy,Status)`: both also have `ByRef Status`, and the latter returns **13** values, not twelve. [S3][S19]

**API conflict:** The `IBody2.GetMassProperties` page calls the return an "array of objects" in its summary but describes twelve doubles in the remarks. Neither phrase alone specifies the wire VARTYPE of the array. Log the runtime subtype. [S4]

**Relation element risks:** `GetRelations` may be an array of dispatch objects, typed interface pointers, or Variants containing COM objects. The API page does not state which storage form IDispatch returns. A successful `GetRelationsCount` cannot establish which one. The same applies to `GetDefinitionEntities2`, `GetEntities`, and the numeric `GetEntitiesType` array. [S7]-[S11]

**In-process `I*` alternatives:** Many SOLIDWORKS methods named `IGet...` return unmanaged pointers or use pointer-based arrays. The SOLIDWORKS pages state that these are **not supported by VBA, VB.NET, C#, or C++/CLI**. Do not treat `IGetCenterOfMass`, `IGetRelations`, `IGetEntitiesType`, or `IGetMomentOfInertia` as VBScript fallbacks. Use the documented non-`I` Automation members or a properly typed host. [S20]

### 2.2 Copy-ready declarations and VBA usage

These are API shapes. They are **not** VBScript implementation proof.

```vb
' SOLIDWORKS interop signatures as documented in .NET-style VBA form.
' IModelDocExtension
Function GetMassProperties2( _
    ByVal Accuracy As Integer, _
    ByRef Status As Integer, _
    ByVal UseSelected As Boolean) As Object

' IBody2
Function GetMassProperties(ByVal Density As Double) As Object

' IDimension
Function GetSystemValue3( _
    ByVal WhichConfigurations As Integer, _
    ByVal Config_names As Object) As Object
Function SetSystemValue3( _
    ByVal NewValue As Double, _
    ByVal WhichConfigurations As Integer, _
    ByVal Config_names As Object) As Integer

' ISketchRelationManager
Function GetRelations(ByVal Filter As Integer) As Object
Function GetRelationsCount(ByVal Filter As Integer) As Integer

' ISketchRelation
Function GetDefinitionEntities2() As Object
Function GetEntities() As Object
Function GetEntitiesType() As Object
Function GetEntitiesCount() As Integer

' IPartDoc
Function GetBodies2( _
    ByVal BodyType As Integer, _
    ByVal BVisibleOnly As Boolean) As Object
```

**Recorded official example:** A SOLIDWORKS 2026 VBA example calls `swModel.Extension.GetMassProperties2(1, nStatus, True)`, then reads indices `0..11` and separately checks status. That VBA indexing example does not prove the same array can be indexed by `cscript.exe`. [S21]

```vb
' VBA only. Recorded public call form; model scope and status must be checked.
Dim props As Variant
Dim status As Long
props = swModel.Extension.GetMassProperties2(1, status, False)
If status <> 0 Then Err.Raise vbObjectError + 110, , "MASS_STATUS"
If Not IsArray(props) Then Err.Raise vbObjectError + 111, , "MASS_NOT_ARRAY"
If LBound(props) <> 0 Or UBound(props) <> 11 Then _
    Err.Raise vbObjectError + 112, , "MASS_WRONG_LENGTH"
Debug.Print props(0), props(1), props(2)
```

**Reconstructed / untested:** A typed C# or VBA probe should execute the cross-check outside VBScript and then publish numeric values through a documented boundary. Two independently read arrays might expose the same underlying CAD mass calculation. Agreement is useful for a marshaling check, but does not by itself prove correct CAD geometry.

### 2.3 Dimensions are not all arrays of equivalent shape

`GetSystemValue3(swThisConfiguration, Empty)` returns a value that official VBA code reads as `values(0)`. Other configuration requests can return several values, and supplied `Config_names` can be a BSTR or a BSTR array. In contrast, `SetSystemValue3` returns a **scalar status**. [S5][S6][S22]

Therefore the verifier needs separate rules for (a) array element accessibility, (b) configuration identity, (c) dimension state, and (d) numeric unit conversion. Do not treat a successful setter status as evidence that a later getter returned a usable numeric element.

### 2.4 Object traversal is a different mechanism

`Set doc = swApp.GetFirstDocument()` and `Set doc = doc.GetNext()` receive COM object references directly. They do not ask the scripting engine to subscript a SAFEARRAY. This explains why working document traversal does **not** refute the documented array limitation. These calls still require end-of-chain checks and appropriate document type validation. [S16][S17]

## T3. Minimal fixtures and probe design

### 3.1 Windows-only fixture without SOLIDWORKS

**Reconstructed / untested Windows script:** run `cscript //nologo probe-typed-arrays.vbs` from an ordinary console. This fixture contrasts three array families:

1. `Array(1.25,2.5,3.75)` is a native VBScript Variant-element array. Expected `VarType=8204` and working indexing.
2. `Scripting.Dictionary.Keys` returns an array with a documented indexing example. It is a useful COM-produced control. The array's actual `VarType` should still be recorded. [M14]
3. `ADODB.Stream.Read(3)` on a short binary stream is a COM-produced byte array. An accepted 2026 community example reproduces an index failure for that type. This is a **byte** fixture, not proof of the `Double` case. [M9][M15]

The companion `.vbs` file contains guarded probes for `VarType`, `TypeName`, bounds, indexed access, and `For Each`. It does not open a SOLIDWORKS document or modify the registry.

**Optional exact-Double fixture:** create a registered COM object with two property getters that return the same `[1.25,2.5,3.75]` data:

```c
// IDL shape (a specification for a fixture, not a compiled component)
HRESULT GetTyped([out, retval] VARIANT *result);   // vt=VT_ARRAY|VT_R8
HRESULT GetBoxed([out, retval] VARIANT *result);   // vt=VT_ARRAY|VT_VARIANT
```

Build the arrays with `SafeArrayCreate` and set each type explicitly. The archived Microsoft KB includes a comparable ATL `VBSARRAY` fixture for `BSTR` and `VARIANT` elements. The old sample is not a tested x64 SOLIDWORKS 2026 component. [M1][M4]

**Acceptable proof:** on the same `cscript.exe`, `GetTyped` permits bounds but fails element access, while `GetBoxed` permits access to three finite doubles. If both are accessible, capture the exact engine version and native `VARIANT.vt`, then revise the hypothesis. Do not infer array subtype only from `System.Object`.

### 3.2 `mass-com-container` capture protocol

**Reconstructed / untested.** In the existing verifier, run this read-only sequence for the rebuilt document and STEP reference, using the same COM API and scope:

1. Record process executable, bitness, account/session, SOLIDWORKS process identity, active document, configuration, and mass-property object kind.
2. After `raw = mp.CenterOfMass`, record `Err.Number`, `IsArray`, `VarType(raw)`, `TypeName(raw)`, and `LBound`/`UBound` separately. Record the decimal and hexadecimal array subtype.
3. Attempt `raw(0)`, then `VarType(raw(0))`, and then `For Each`. Capture each error independently with `Err.Description`. Do not let an earlier error contaminate a later result.
4. Repeat with `IMassProperty2.CenterOfMass`, `IBody2.GetMassProperties(Density)`, and the reference STEP document. The density value and body scope must match.
5. In **typed VBA/.NET**, call `IModelDocExtension.GetMassProperties2(1, status, False)`. Capture status, array subtype, and all twelve values. Do not make that call a mandatory direct-VBScript path until the ByRef seat issue is resolved.
6. Compare two documented centroid representations in the **same coordinate frame**. Repeat direct `cscript` and scheduled-task execution with the same script and model.

**Important distinction:** the native COM type inspector, if built, must capture `VARIANT.vt` and `SafeArrayGetVartype()` as two separate observations. A discrepancy is possible if a COM server uses a `VARIANT` wrapper with an array descriptor. `VarType(raw)` is the first simple diagnostic, not proof of the native descriptor flags. [M3][M4]

### 3.3 Reusable fail-closed VBScript guard

**Reconstructed / untested.** This code is suitable **only after** a producer supplies an indexable `SAFEARRAY(VARIANT)` with Double entries. It refuses raw typed SAFEARRAYs rather than pretending to convert them. Values must already be in consistent metric units and a known coordinate system.

```vbscript
Option Explicit
Const E_ARRAY = 8192
Const E_VARIANT = 12
Const E_DOUBLE = 5
Const MAX_COORD_M = 1000  ' Policy guard; replace with project-specific limit.

Function ReadTripleOrFail(raw, label)
  Dim vt, i, lo, hi, v, out(2)
  If Not IsArray(raw) Then Err.Raise 513, , label & ":NOT_ARRAY"
  vt = VarType(raw)
  If vt <> E_ARRAY + E_VARIANT Then
    Err.Raise 514, , label & ":UNREADABLE_ARRAY_SUBTYPE_" & vt
  End If
  lo = LBound(raw): hi = UBound(raw)
  If lo <> 0 Or hi <> 2 Then Err.Raise 515, , label & ":ARRAY_BOUNDS"
  For i = 0 To 2
    If VarType(raw(i)) <> E_DOUBLE Then
      Err.Raise 516, , label & ":COMPONENT_NOT_DOUBLE_" & i
    End If
    v = raw(i)
    If Not (v = v) Then Err.Raise 517, , label & ":NAN_" & i
    If Not (Abs(v) <= MAX_COORD_M) Then
      Err.Raise 518, , label & ":NONFINITE_OR_RANGE_" & i
    End If
    out(i) = v
  Next
  ReadTripleOrFail = Array(out(0), out(1), out(2))
End Function
```

This example deliberately accepts only `Double` components. It does not try to convert a string, a COM object, or a `Single`. The numeric range is an example policy, not a value supplied by SOLIDWORKS documentation. In production, protect the caller with a narrow error handler and write an explicit failure verdict. Catch any unexpected runtime fault as `ARRAY_READ_ERROR`; never default a missing centroid component to zero.

**Limits of the snippet:** `VarType` semantics come from Visual Basic documentation, and this exact function was not executed under VBScript here. The precise behavior of `NaN`/infinity values delivered from an external COM component must also be tested. A typed .NET adapter can enforce `Double.IsFinite` (or explicit infinity/NaN rejection on older frameworks) before exposing the result.

### 3.4 Acceptance rules and named refusals

| Category | Accept only if | Named refusal |
|---|---|---|
| Acquisition | Property call succeeds and yields an array | `MASS_PROPERTY_READ_FAILED`, `NOT_ARRAY` |
| Representation | Native subtype is understood and indexable, or a typed adapter explicitly converted it | `UNREADABLE_ARRAY_SUBTYPE`, `ARRAY_READ_ERROR` |
| Shape | Exactly one dimension with zero-based indices `0..2` | `ARRAY_RANK`, `ARRAY_BOUNDS`, `ARRAY_LENGTH` |
| Components | Exactly three accessible, finite Double values, no COM objects, no coercion | `COMPONENT_UNREADABLE`, `COMPONENT_NOT_DOUBLE`, `COMPONENT_NOT_FINITE`, `COMPONENT_OUT_OF_RANGE` |
| Scope | Same body set, active configuration, units, and coordinate frame | `MASS_SCOPE_MISMATCH`, `MASS_CONFIGURATION_MISMATCH`, `MASS_UNITS_UNKNOWN`, `MASS_FRAME_MISMATCH` |
| Independent acquisition | Center-of-mass triple agrees with `GetMassProperties2[0..2]` via qualified typed client within documented/project tolerance | `MASS_CROSSCHECK_UNAVAILABLE`, `MASS_CROSSCHECK_FAILED`, `MASS_CROSSCHECK_MISMATCH` |
| Relation inventory | Returned relations and defining entities are readable, count matches, normalized relation multisets match the source | `RELATION_ARRAY_UNREADABLE`, `RELATION_COUNT_MISMATCH`, `RELATION_ENTITY_UNREADABLE`, `RELATION_INVENTORY_MISMATCH` |
| Dimension readback | Configuration-specific value array accessible, one expected state and type, value valid | `DIMENSION_ARRAY_UNREADABLE`, `DIMENSION_CONFIG_MISMATCH`, `DIMENSION_STATE_MISMATCH`, `DIMENSION_VALUE_MISMATCH` |

Use a configurable tolerance policy, for example `abs(a-b) <= max(absTol, relTol*max(abs(a),abs(b)))`. The brief does not supply numeric absolute/relative thresholds. **Do not invent authoritative tolerance limits.** An agreement between two mass-property methods is a marshaling and API-use check, not an independent shape oracle. Compare against STEP geometry or another independent reference separately.

### 3.5 Alternatives, ranked for this codebase

1. **Preferred: typed .NET verifier or COM bridge.** Read `double[]` and object arrays in C# through the SOLIDWORKS interop. Expose a narrow, verified output: JSON with invariant numeric formatting, or a COM `SAFEARRAY(VARIANT)` containing only Double scalars. Emit the calculation status, scope, configuration and units. The adapter must not silently convert unknown objects.
2. **Also practical: VBA macro owns the readbacks.** The official SOLIDWORKS VBA examples index these arrays. Return results to `check-part.vbs` through a file/IPC contract with a success sentinel, then validate again in VBScript. This preserves the current scheduler but adds a macro execution dependency.
3. **Limited: direct VBScript.** Keep scalar methods and already proven object traversals. Accept arrays only if the specific method's type and element access pass seat probes. `For Each` is not an approved workaround based on public evidence.

For the many arrays in relation and dimension verification, a single typed adapter can reduce separate late-binding failure modes. Keep one explicit conversion contract and a per-method identity/shape validator.

## Conflicts and corrections

| Item | Evidence A | Evidence B | Resolution |
|---|---|---|---|
| Centroid elements | **Recorded:** SOLIDWORKS says `Double[3]`. [S1] | **Seat:** every `raw(i)` fails err 13 despite bounds success. | **Likely non-Variant SAFEARRAY.** Confirm `VarType(raw)` and native VARTYPE. `Double[3]` describes values, not COM element layout. |
| Error number | **Recorded:** old KB sample reports an unsupported member error. [M1] | **Seat:** type mismatch err 13. | No published per-type error matrix found. Match behavior and native type, not message alone. |
| `For Each` | **Recorded:** can enumerate normal arrays/collections. [M5][M6] | **Recorded:** VBScript cannot manipulate non-Variant array contents. [M1] | Do not assume typed SAFEARRAY enumeration works. Seat probe only. |
| `IBody2.GetMassProperties` payload | **Recorded:** Return Value uses wording "array of objects". [S4] | **Recorded:** Remarks specify twelve doubles. [S4] | Observe runtime `VarType`. Both descriptions are insufficient to pin COM storage. |
| Bodies interface | **Source brief:** `IModelDoc2.GetBodies2` | **Recorded:** `IPartDoc.GetBodies2(BodyType,BVisibleOnly)`. [S12] | Use `IPartDoc`. For assembly components, use the separate `IComponent2` API. |
| 12-double cross-check access | **Recorded:** `GetMassProperties2` supplies 12 values. [S3] | **Recorded and seat:** has typed ByRef status, which this seat previously rejected through VBScript. | Cross-check in VBA/.NET first. No direct VBScript guarantee. |
| Host cause | **Source brief:** PowerShell/scheduled path could matter. | **Recorded:** cscript hosts the same VBScript language. [M7][M8] | Compare accounts, bitness and invocation. No source supports automatic PowerShell array rewriting. |
| Relation arrays | **Recorded:** return arrays of relation objects and entities. [S7]-[S10] | **Unverified:** `SAFEARRAY` element VARTYPE for these interfaces. | Probe each family before enabling exact inventory acceptance. |

## Still needs a seat probe

| Priority | Probe | Exact test | Pass / stop criterion |
|---|---|---|---|
| P0 | `mass-com-container` | On original and STEP: log `VarType(raw)`, `TypeName(raw)`, rank/bounds, element read error, `For Each` error and count. | Distinguishes known Variant array from unsupported typed array without guessing. |
| P0 | `mass-double-fixture` | Compare custom COM `SAFEARRAY(R8)` and `SAFEARRAY(VARIANT)` with the same three doubles under `cscript`. | Proves or rejects the direct-index hypothesis on target Windows version. |
| P0 | `mass-adapter-verify` | In typed client, compare `CenterOfMass` to `GetMassProperties2[0..2]` after checking `Status=0`. Return validated Doubles. | Three finite doubles, same scope/frame/config, declared tolerances met. |
| P0 | `mass-host-parity` | Launch the same array probe directly and from scheduled PowerShell task. | Same engine version, COM identity, subtype, values and verdict. |
| P1 | `body-array` | On a part, call `IPartDoc.GetBodies2(0,False)`. Check `VarType`, bounds, `raw(0)`, object identity. | Body count and every enumerated body readable. Index order not used as identity. |
| P1 | `relation-array` | Make sketch with three known relations. Compare `GetRelationsCount(0)` with array length and object indices. | All relations and their type readable, no extras. |
| P1 | `relation-operand-arrays` | Inspect `GetDefinitionEntities2`, `GetEntities`, `GetEntitiesType` for ordinary and external-edge relations. | Exact non-null definition operands and validated enum alignment. |
| P1 | `dimension-values` | Compare `GetSystemValue3(1, Empty)` against VBA readback for one- and multi-configuration dimensions. | Array shape, types, values and scope stable. |
| P2 | `inertia-arrays` | Capture nine- and three-element inertial arrays and subtype. | Correct sizes/order, finite values, known coordinate frame. |
| P2 | `vbscript-out-param` | Re-test `GetMassProperties2(1, status, False)` with a declared Variant/Long subtype, isolated from the verifier. | Either documented success and correct status or named `BYREF_UNSUPPORTED`. Do not block adapter path on it. |

## Source index

### Microsoft / COM

- **[M1] Recorded (archived original Microsoft KB 165967):** [PRB: Script Error Occurs When Referencing Non-variant Array](https://ftp.zx.net.nz/pub/archive/ftp.microsoft.com/MISC/KB/en-us/165/967.HTM). Explains indexing limitation, bounds checks, pass-through and typed-versus-Variant array fixture.
- **[M2] Recorded (vendor documentation):** [SAS: Using VBScript](https://support.sas.com/rnd/itech/doc/dist-obj/winclnt/winvbscr.html). Explains converter for typed SAFEARRAYs and typed out parameters.
- **[M3] Recorded:** [Microsoft, VARIANT type constants](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/3fe7db9f-5803-4dc4-9d14-5425d3f5461f).
- **[M4] Recorded:** [Microsoft, SAFEARRAY structure and flags](https://learn.microsoft.com/en-us/windows/win32/api/oaidl/ns-oaidl-safearray) and [array manipulation functions](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/automat/array-manipulation-functions).
- **[M5] Recorded:** [Microsoft, For Each in VBA](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/for-eachnext-statement).
- **[M6] Recorded:** [Microsoft, `_NewEnum` on a COM collection](https://learn.microsoft.com/en-us/previous-versions/exchange-server/exchange-10/ms527562%28v%3Dexchg.10%29) and [ActiveX collection enumerators](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/automat/design-considerations-for-activex-objects).
- **[M7] Recorded:** [Microsoft, scripting with COM under Windows Script Host](https://learn.microsoft.com/en-us/windows/win32/com/using-com-objects-in-windows-script-host).
- **[M8] Recorded:** [Microsoft, `cscript` command-line options](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/cscript).
- **[M9] Community:** [2026 Microsoft Q&A: byte SAFEARRAY from `ADODB.Stream`](https://learn.microsoft.com/en-us/answers/questions/5959932/unexpected-error-when-dereferencing-a-byte-array-i).
- **[M10] Recorded:** [Microsoft, `VarType` function, array flag and element constants](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/vartype-function).
- **[M11] Recorded:** [Microsoft, VarType constants](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/vartype-constants) and [Microsoft, using arrays in VBA](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/using-arrays).
- **[M12] Recorded:** [Microsoft, `_NewEnum` and `IEnumVARIANT`](https://learn.microsoft.com/en-us/windows/win32/seccrypto/attributes-newenum).
- **[M13] Community / language reference:** [VBScript unsupported conversion functions](https://flylib.com/books/en/1.492.1/appendix_a_vbscript_functions_and_keywords.html) and [Microsoft-origin VBScript Array language reference mirror](https://documentation.help/MS-Office-VBScript/vsfctArray.htm).
- **[M14] Recorded:** [Microsoft, `Dictionary.Keys`](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/keys-method).
- **[M15] Recorded:** [Microsoft, `ADODB.Stream.Read`](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/read-method-ado) and [stream Type property](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/type-property-ado-stream).

### SOLIDWORKS API Help

- **[S1] Recorded:** [`IMassProperty.CenterOfMass`](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty~CenterOfMass.html).
- **[S2] Recorded:** [`IMassProperty2.CenterOfMass` (2026)](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty2~CenterOfMass.html).
- **[S3] Recorded:** [`IModelDocExtension.GetMassProperties2`](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imodeldocextension~getmassproperties2.html).
- **[S4] Recorded:** [`IBody2.GetMassProperties`](https://help.solidworks.com/2026/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IBody2~GetMassProperties.html).
- **[S5] Recorded:** [`IDimension.GetSystemValue3`](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.idimension~getsystemvalue3.html).
- **[S6] Recorded:** [`IDimension.SetSystemValue3`](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.interop.sldworks~SolidWorks.interop.sldworks.IDimension~SetSystemValue3.html).
- **[S7] Recorded:** [`ISketchRelationManager.GetRelations`](https://help.solidworks.com/2022/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelationManager~GetRelations.html) and [`GetRelationsCount`](https://help.solidworks.com/2025/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISketchRelationManager~GetRelationsCount.html).
- **[S8] Recorded:** [`ISketchRelation.GetDefinitionEntities2`](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelation~GetDefinitionEntities2.html).
- **[S9] Recorded:** [`ISketchRelation.GetEntities`](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isketchrelation~getentities.html).
- **[S10] Recorded:** [`ISketchRelation.GetEntitiesType`](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isketchrelation~getentitiestype.html).
- **[S11] Recorded:** [`ISketchRelation.GetEntitiesCount`](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isketchrelation~getentitiescount.html).
- **[S12] Recorded:** [`IPartDoc.GetBodies2` (2026)](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IPartDoc~GetBodies2.html).
- **[S13] Recorded:** [`IMassProperty.PrincipleAxesOfInertia`](https://help.solidworks.com/2026/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty~PrincipleAxesOfInertia.html).
- **[S14] Recorded:** [`IMassProperty.PrincipleMomentsOfInertia`](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty~PrincipleMomentsOfInertia.html).
- **[S15] Recorded:** [`IMassProperty.GetMomentOfInertia`](https://help.solidworks.com/2024/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty~GetMomentOfInertia.html).
- **[S16] Recorded:** [`ISldWorks.GetFirstDocument`](https://help.solidworks.com/2021/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.isldworks~getfirstdocument.html).
- **[S17] Recorded:** [`IModelDoc2.GetNext`](https://help.solidworks.com/2024/english/api/sldworksapi/solidworks.interop.sldworks~solidworks.interop.sldworks.imodeldoc2~getnext.html).
- **[S18] Recorded:** [`IComponent2.GetBodies2`](https://help.solidworks.com/2014/english/api/sldworksapi/SolidWorks.interop.sldworks~SolidWorks.interop.sldworks.IComponent2~GetBodies2.html) and [`IComponent2.GetBodies3`](https://help.solidworks.com/2018/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IComponent2~GetBodies3.html).
- **[S19] Recorded:** [`IModelDocExtension.GetMassProperties` (13 entries)](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IModelDocExtension~GetMassProperties.html) and [`IModelDoc2.GetMassProperties2` (obsolete)](https://help.solidworks.com/2022/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDoc2~GetMassProperties2.html).
- **[S20] Recorded:** [`IGetCenterOfMass` in-process restriction](https://help.solidworks.com/2012/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IMassProperty~IGetCenterOfMass.html), [`IGetRelations` restriction](https://help.solidworks.com/2024/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelationManager~IGetRelations.html), [`IGetEntitiesType` restriction](https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISketchRelation~IGetEntitiesType.html).
- **[S21] Recorded:** [Get Mass Properties Example (VBA), 2026](https://help.solidworks.com/2026/english/api/sldworksapi/Get_Mass_Properties_of_ActiveDoc_Example_VB.htm).
- **[S22] Recorded:** [Change Dimensions of Gear Mate Example (VBA)](https://help.solidworks.com/2012/English/api/sldworksapi/Change_Dimensions_of_Gear_Mate_Example_VB.htm).

---

**Decision for craft-bridge:** The available public references support the non-Variant SAFEARRAY diagnosis. They do **not** prove that `IMassProperty.CenterOfMass` returns `VT_ARRAY|VT_R8` on this seat. Capture the subtype first. If the diagnosis holds, do not attempt to coerce individual elements in VBScript. Convert them in a typed host or move the readback there, and keep named refusal behavior until the adapter and all affected arrays pass seat tests.
