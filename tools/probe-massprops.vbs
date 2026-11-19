' probe-massprops.vbs - the mass-com-container probe (research 10 P0,
' research/outputs/10-verification-readbacks.md; protocol per research 11
' 3.2, research/outputs/11-vbscript-com-array-marshaling.md). SYNTHETIC
' ONLY: it imports the committed synthetic tests/fixtures/box.step
' (independently authored, one solid), characterizes how
' IMassProperty.CenterOfMass marshals into VBScript on this seat, tries
' the documented alternate acquisition paths, and closes exactly the
' document it opened with the verified-close discipline. No real-part
' artifact is read or written; the report is synthetic evidence.
'
' Usage: cscript //nologo probe-massprops.vbs <step-file> <report-file>
' Exit codes: 0 the probe completed and wrote its report (the report
' carries pass/fail per capture); 4 refused before running; 5 the probe
' could not complete.
'
' The question being answered (Microsoft KB 165967 is the recorded
' hypothesis): VBScript can inspect the bounds of a non-Variant
' SAFEARRAY but its element indexing supports only SAFEARRAY(VARIANT).
' The container's own VarType/TypeName (decimal and hex) distinguishes
' the representations; For Each remains an EXPERIMENT, never an
' approved workaround; the documented alternates are captured with
' their exact signatures (GetMassProperties2 carries a ByRef Status -
' the seat's known uncallable shape - so its failure is a named
' finding, not a probe defect).
Option Explicit
Dim swApp, stepPath, reportPath, fso, outFile
Dim preTitles, preErr, d, t2
Dim refDoc, refTitle, importErrDesc, importData, ok, n, dsc
Dim report(300), reportN
Dim solidsText, sheetsText, volRaw, volErr, mp, bodiesArr, sheetsArr
Dim usRaw, mp2, recalcRaw, statusVar, arr2
Dim body0, bmRaw, bmErr, bi, bv, be, j, jv, je, w
Dim cnt2, isOurs, closeErr

reportN = 0
Sub Emit(ByVal s)
  If reportN <= 300 Then
    report(reportN) = s
    reportN = reportN + 1
  End If
End Sub

If WScript.Arguments.Count < 2 Then
  WScript.Echo "usage: probe-massprops.vbs <step-file> <report-file>"
  WScript.Quit 4
End If
stepPath = WScript.Arguments(0)
reportPath = WScript.Arguments(1)

Set fso = CreateObject("Scripting.FileSystemObject")
' The shared decision helpers (NumStr, IsPreExistingTitle): the probe
' reports in the same locale-independent number text as the checker.
Err.Clear
On Error Resume Next
ExecuteGlobal fso.OpenTextFile(fso.GetParentFolderName(WScript.ScriptFullName) & "\check-part-logic.vbs", 1).ReadAll
If Err.Number <> 0 Then
  WScript.Echo "error: check-part-logic.vbs does not execute: " & Err.Description
  WScript.Quit 4
End If
On Error GoTo 0

Err.Clear
On Error Resume Next
Set swApp = CreateObject("SldWorks.Application")
n = Err.Number
dsc = Err.Description
On Error GoTo 0
If n <> 0 Then
  WScript.Echo "error: cannot attach to SolidWorks: " & dsc
  WScript.Quit 4
End If

' ---- ownership snapshot BEFORE the import (the CloseOwnedDoc rule) ----
preTitles = ""
preErr = 0
Err.Clear
On Error Resume Next
Set d = swApp.GetFirstDocument
If Err.Number <> 0 Then
  preErr = Err.Number
Else
  Do While Not d Is Nothing
    Err.Clear
    t2 = d.GetTitle
    If Err.Number <> 0 Then
      preErr = Err.Number
      Exit Do
    End If
    preTitles = preTitles & t2 & vbCrLf
    Err.Clear
    Set d = d.GetNext
    If Err.Number <> 0 Then
      preErr = Err.Number
      Exit Do
    End If
  Loop
End If
On Error GoTo 0
Emit "probe:pre_enumeration=err " & NumStr(preErr)
If preErr <> 0 Then
  Emit "probe:verdict=refused (the ownership enumeration failed; nothing imported)"
  GoTo Finish
End If

' ---- import the committed synthetic box (the proven check-part shape) ----
Set refDoc = Nothing
importErrDesc = ""
Err.Clear
On Error Resume Next
Set importData = swApp.GetImportFileData(stepPath)
n = Err.Number
dsc = Err.Description
On Error GoTo 0
If n <> 0 Then
  importErrDesc = "GetImportFileData err " & NumStr(n) & ": " & dsc
Else
  ok = False
  Err.Clear
  On Error Resume Next
  ok = swApp.LoadFile3(stepPath, "r", importData)
  n = Err.Number
  dsc = Err.Description
  On Error GoTo 0
  If n <> 0 Then
    importErrDesc = "LoadFile3 err " & NumStr(n) & ": " & dsc
  ElseIf ok = False Then
    importErrDesc = "LoadFile3 returned False (no document was opened)"
  Else
    Err.Clear
    On Error Resume Next
    Set refDoc = swApp.ActiveDoc
    n = Err.Number
    dsc = Err.Description
    On Error GoTo 0
    If n <> 0 Or refDoc Is Nothing Then
      importErrDesc = "the imported document could not be taken from ActiveDoc (err " & NumStr(n) & ": " & dsc & ")"
      Set refDoc = Nothing
    Else
      Err.Clear
      On Error Resume Next
      refTitle = refDoc.GetTitle
      n = Err.Number
      On Error GoTo 0
      If n <> 0 Then refTitle = ""
      If Trim(refTitle) = "" Then
        importErrDesc = "the imported document reports an empty title; refusing to attribute it"
        Set refDoc = Nothing
      ElseIf IsPreExistingTitle(preTitles, refTitle) Then
        importErrDesc = "the import activated a pre-existing document <" & refTitle & ">; refusing to attribute it"
        Set refDoc = Nothing
      End If
    End If
  End If
End If
If refDoc Is Nothing Then
  Emit "probe:import=fail " & importErrDesc
  Emit "probe:verdict=refused (the synthetic fixture did not import)"
  GoTo Finish
End If
Emit "probe:import=pass title <" & refTitle & ">"

' ---- the geometry gate: one solid and positive volume FIRST ----
Err.Clear
On Error Resume Next
bodiesArr = refDoc.GetBodies2(0, False)
n = Err.Number
On Error GoTo 0
solidsText = "unreadable (err " & NumStr(n) & ")"
If n = 0 Then
  If IsArray(bodiesArr) Then
    solidsText = NumStr(UBound(bodiesArr) - LBound(bodiesArr) + 1)
  ElseIf IsEmpty(bodiesArr) Or IsNull(bodiesArr) Then
    solidsText = "0"
  Else
    solidsText = "unexpected shape"
  End If
End If
Err.Clear
On Error Resume Next
sheetsArr = refDoc.GetBodies2(1, False)
n = Err.Number
On Error GoTo 0
sheetsText = "unreadable (err " & NumStr(n) & ")"
If n = 0 Then
  If IsArray(sheetsArr) Then
    sheetsText = NumStr(UBound(sheetsArr) - LBound(sheetsArr) + 1)
  ElseIf IsEmpty(sheetsArr) Or IsNull(sheetsArr) Then
    sheetsText = "0"
  Else
    sheetsText = "unexpected shape"
  End If
End If
Emit "probe:geometry=solids " & solidsText & " sheets " & sheetsText

' legacy mass property: all solid bodies (the documented default), system
' units SET and read back (the scope/unit controls research 10 requires)
Err.Clear
On Error Resume Next
Set mp = refDoc.Extension.CreateMassProperty()
n = Err.Number
dsc = Err.Description
On Error GoTo 0
Emit "probe:massproperty_create=err " & NumStr(n) & " " & dsc
Err.Clear
On Error Resume Next
usRaw = Empty
usRaw = mp.UseSystemUnits
n = Err.Number
On Error GoTo 0
Emit "probe:use_system_units_read=err " & NumStr(n) & " value " & NumStr(usRaw)
Err.Clear
On Error Resume Next
mp.UseSystemUnits = True
n = Err.Number
On Error GoTo 0
Emit "probe:use_system_units_set=err " & NumStr(n)
Err.Clear
On Error Resume Next
volRaw = Empty
volRaw = mp.Volume
volErr = Err.Number
dsc = Err.Description
On Error GoTo 0
Emit "probe:volume_legacy=err " & NumStr(volErr) & " " & dsc & " vartype " & NumStr(VarType(volRaw)) & " value " & NumStr(volRaw)
If solidsText = "1" And volErr = 0 Then
  If IsNumeric(volRaw) Then
    If volRaw > 0 Then
      Emit "probe:geometry_gate=pass (one solid, positive volume; centroid results below are interpretable)"
    Else
      Emit "probe:geometry_gate=fail (volume not positive; centroid results below are UNINTERPRETABLE)"
    End If
  Else
    Emit "probe:geometry_gate=fail (volume unreadable; centroid results below are UNINTERPRETABLE)"
  End If
Else
  Emit "probe:geometry_gate=fail (the one-solid/positive-volume precondition did not hold; centroid results below are UNINTERPRETABLE)"
End If

' ---- the container and element captures (the probe's core;
' research 11 3.2: every observation captured independently) ----
Sub CaptureCenter(ByVal mpObj, ByVal tag)
  Dim raw, i, k, lo2, hi2, errN, errD
  Dim vtC, tnC, isArr, isObj, lo2b, hi2b
  Dim val, errRead, errSet, errVt, errTn, vtE, tnE, dd1, dd2
  Dim cnt, it, itVt, itTn, itVal, loopErr
  Err.Clear
  On Error Resume Next
  raw = Empty
  raw = mpObj.CenterOfMass
  errN = Err.Number
  errD = Err.Description
  On Error GoTo 0
  Emit tag & ".get=err " & NumStr(errN) & " " & errD
  If errN <> 0 Then Exit Sub
  ' container identity (decimal AND hex subtype, per research 11)
  Err.Clear
  On Error Resume Next
  isObj = IsObject(raw)
  isArr = IsArray(raw)
  vtC = -1
  tnC = "?"
  vtC = VarType(raw)
  If Err.Number <> 0 Then vtC = -1
  Err.Clear
  tnC = TypeName(raw)
  If Err.Number <> 0 Then tnC = "?"
  Err.Clear
  lo2 = -9999
  hi2 = -9999
  lo2 = LBound(raw)
  hi2 = UBound(raw)
  errN = Err.Number
  On Error GoTo 0
  Emit tag & ".container=isobject " & NumStr(isObj) & " isarray " & NumStr(isArr) & " vartype " & NumStr(vtC) & " (0x" & Hex(vtC) & ") typename " & tnC
  Emit tag & ".bounds=err " & NumStr(errN) & " lbound " & NumStr(lo2) & " ubound " & NumStr(hi2)
  ' dimensionality: a second-dimension probe must fail for a 1-D array
  Err.Clear
  On Error Resume Next
  lo2b = -9999
  hi2b = -9999
  lo2b = LBound(raw, 2)
  hi2b = UBound(raw, 2)
  errN = Err.Number
  On Error GoTo 0
  If errN = 0 Then
    Emit tag & ".dimension=2+ (bounds2 " & NumStr(lo2b) & ".." & NumStr(hi2b) & ")"
  Else
    Emit tag & ".dimension=1 (second-dimension probe raised err " & NumStr(errN) & ")"
  End If
  ' indexed element access forms (the money capture): each observation
  ' guarded INDEPENDENTLY so one failure never contaminates the next
  For k = 0 To 1
    i = lo2 + k
    val = Empty
    errRead = 0
    dd1 = ""
    Err.Clear
    On Error Resume Next
    val = raw(i)
    errRead = Err.Number
    dd1 = Err.Description
    On Error GoTo 0
    errSet = 0
    dd2 = ""
    Err.Clear
    On Error Resume Next
    Set body0 = Nothing
    Set body0 = raw(i)
    errSet = Err.Number
    dd2 = Err.Description
    On Error GoTo 0
    errVt = 0
    vtE = -1
    Err.Clear
    On Error Resume Next
    vtE = VarType(raw(i))
    errVt = Err.Number
    If errVt <> 0 Then vtE = -1
    On Error GoTo 0
    errTn = 0
    tnE = "?"
    Err.Clear
    On Error Resume Next
    tnE = TypeName(raw(i))
    errTn = Err.Number
    If errTn <> 0 Then tnE = "?"
    On Error GoTo 0
    Emit tag & ".element" & NumStr(i) & "=read_err " & NumStr(errRead) & " [" & dd1 & "] set_err " & NumStr(errSet) & " [" & dd2 & "] vartype_err " & NumStr(errVt) & " typename_err " & NumStr(errTn) & " vartype " & NumStr(vtE) & " typename " & tnE
  Next
  ' For Each enumeration: an EXPERIMENT (research 11: not an approved
  ' workaround) - count, per-item identity and value, loop error
  cnt = 0
  loopErr = 0
  Err.Clear
  On Error Resume Next
  For Each it In raw
    cnt = cnt + 1
    itVt = -1
    itTn = "?"
    itVal = Empty
    itVt = VarType(it)
    If Err.Number <> 0 Then itVt = -1
    Err.Clear
    itTn = TypeName(it)
    If Err.Number <> 0 Then itTn = "?"
    Err.Clear
    If Not IsObject(it) Then
      itVal = it
      If Err.Number <> 0 Then itVal = Empty
    End If
    Err.Clear
    Emit tag & ".enum" & NumStr(cnt) & "=vartype " & NumStr(itVt) & " typename " & itTn & " value " & NumStr(itVal)
    If cnt >= 6 Then Exit For
  Next
  loopErr = Err.Number
  On Error GoTo 0
  Emit tag & ".for_each=count " & NumStr(cnt) & " loop_err " & NumStr(loopErr)
End Sub

CaptureCenter mp, "probe:center_legacy"

' ---- bodies-array element access (research 11 P1 body-array) and the
' IBody2.GetMassProperties cross-check (12 doubles, caller density) ----
Err.Clear
On Error Resume Next
Set body0 = Nothing
Set body0 = bodiesArr(LBound(bodiesArr))
n = Err.Number
dsc = Err.Description
On Error GoTo 0
Emit "probe:body_element=err " & NumStr(n) & " " & dsc & " isnothing " & NumStr(body0 Is Nothing)
If n = 0 Then
  If Not body0 Is Nothing Then
    Err.Clear
    On Error Resume Next
    bmRaw = Empty
    bmRaw = body0.GetMassProperties(1000)
    bmErr = Err.Number
    dsc = Err.Description
    On Error GoTo 0
    Emit "probe:body_massproperties=err " & NumStr(bmErr) & " " & dsc & " isarray " & NumStr(IsArray(bmRaw))
    If bmErr = 0 Then
      If IsArray(bmRaw) Then
        Emit "probe:body_massproperties_shape=vartype " & NumStr(VarType(bmRaw)) & " (0x" & Hex(VarType(bmRaw)) & ") bounds " & NumStr(LBound(bmRaw)) & ".." & NumStr(UBound(bmRaw))
        For bi = LBound(bmRaw) To LBound(bmRaw) + 3
          Err.Clear
          On Error Resume Next
          bv = Empty
          bv = bmRaw(bi)
          be = Err.Number
          On Error GoTo 0
          Emit "probe:body_massproperties_item" & NumStr(bi) & "=err " & NumStr(be) & " value " & NumStr(bv)
        Next
      End If
    End If
  End If
End If

' ---- the documented alternates (scope controlled; named results) ----
Err.Clear
On Error Resume Next
Set mp2 = refDoc.Extension.CreateMassProperty2()
n = Err.Number
dsc = Err.Description
On Error GoTo 0
Emit "probe:massproperty2_create=err " & NumStr(n) & " " & dsc & " isnothing " & NumStr(mp2 Is Nothing)
If Not mp2 Is Nothing Then
  Err.Clear
  On Error Resume Next
  mp2.UseSystemUnits = True
  n = Err.Number
  On Error GoTo 0
  Emit "probe:massproperty2_units_set=err " & NumStr(n)
  Err.Clear
  On Error Resume Next
  recalcRaw = Empty
  recalcRaw = mp2.Recalculate()
  n = Err.Number
  On Error GoTo 0
  Emit "probe:massproperty2_recalculate=err " & NumStr(n) & " value " & NumStr(recalcRaw)
  CaptureCenter mp2, "probe:center_massproperty2"
  Err.Clear
  On Error Resume Next
  volRaw = Empty
  volRaw = mp2.Volume
  volErr = Err.Number
  On Error GoTo 0
  Emit "probe:volume_massproperty2=err " & NumStr(volErr) & " value " & NumStr(volRaw)
End If

' The DOCUMENTED signature (research 11 [S3]):
' GetMassProperties2(Accuracy As Integer, ByRef Status As Integer,
' UseSelected As Boolean) - the ByRef Status is the seat's known
' uncallable shape (the RunMacro2/LoadFile4 precedent), so a failure
' here is a NAMED finding, not a probe defect.
statusVar = Empty
Err.Clear
On Error Resume Next
arr2 = Empty
arr2 = refDoc.Extension.GetMassProperties2(1, statusVar, False)
n = Err.Number
dsc = Err.Description
On Error GoTo 0
Emit "probe:getmassproperties2=err " & NumStr(n) & " " & dsc & " status_vartype " & NumStr(VarType(statusVar)) & " status_value " & NumStr(statusVar)
If n = 0 Then
  Emit "probe:getmassproperties2_shape=isarray " & NumStr(IsArray(arr2)) & " vartype " & NumStr(VarType(arr2)) & " (0x" & Hex(VarType(arr2)) & ")"
  If IsArray(arr2) Then
    Emit "probe:getmassproperties2_bounds=" & NumStr(LBound(arr2)) & ".." & NumStr(UBound(arr2))
    For j = LBound(arr2) To LBound(arr2) + 5
      Err.Clear
      On Error Resume Next
      jv = Empty
      jv = arr2(j)
      je = Err.Number
      On Error GoTo 0
      Emit "probe:getmassproperties2_item" & NumStr(j) & "=err " & NumStr(je) & " value " & NumStr(jv)
    Next
    cnt2 = 0
    Err.Clear
    On Error Resume Next
    For Each it In arr2
      cnt2 = cnt2 + 1
      If cnt2 >= 13 Then Exit For
    Next
    On Error GoTo 0
    Emit "probe:getmassproperties2_for_each=count " & NumStr(cnt2)
  End If
End If

' ---- verified close of exactly the opened document ----
Finish:
If refDoc Is Nothing Then
  Emit "probe:close=skipped (no document opened)"
Else
  cnt2 = 0
  isOurs = False
  Err.Clear
  On Error Resume Next
  Set d = swApp.GetFirstDocument
  n = Err.Number
  On Error GoTo 0
  If n <> 0 Then
    Emit "probe:close=fail (the pre-close enumeration failed)"
  Else
    Do While Not d Is Nothing
      Err.Clear
      t2 = d.GetTitle
      If Err.Number <> 0 Then
        cnt2 = -1
        Exit Do
      End If
      If t2 = refTitle Then
        cnt2 = cnt2 + 1
        If d Is refDoc Then isOurs = True
      End If
      Err.Clear
      Set d = d.GetNext
    Loop
    If cnt2 <> 1 Or Not isOurs Then
      Emit "probe:close=fail (the title does not identify exactly the opened document; nothing closed)"
    Else
      Err.Clear
      On Error Resume Next
      swApp.CloseDoc refTitle
      closeErr = Err.Number
      On Error GoTo 0
      cnt2 = 0
      Err.Clear
      On Error Resume Next
      Set d = swApp.GetFirstDocument
      If Err.Number = 0 Then
        Do While Not d Is Nothing
          Err.Clear
          t2 = d.GetTitle
          If t2 = refTitle Then cnt2 = cnt2 + 1
          Err.Clear
          Set d = d.GetNext
        Loop
      Else
        cnt2 = -1
      End If
      On Error GoTo 0
      If closeErr = 0 And cnt2 = 0 Then
        Emit "probe:close=pass (opened here, closed here, absence verified)"
      Else
        Emit "probe:close=fail (close_err " & NumStr(closeErr) & " remaining " & NumStr(cnt2) & ")"
      End If
    End If
  End If
  Emit "probe:verdict=completed (see the captures above)"
End If

Set outFile = fso.CreateTextFile(reportPath, True)
For w = 0 To reportN - 1
  outFile.WriteLine report(w)
Next
outFile.Close
WScript.Quit 0
