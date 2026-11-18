' check-part.vbs - verify the rebuilt part in the running SolidWorks session
' against an expect file, and against the STEP reference body's mass
' properties. Invoked only by tools/seat-harness.ps1 and
' tools/seat-run-real.ps1.
'
' Usage: cscript //nologo check-part.vbs <expect-file> <step-reference> <out-file> <owned-title>
' The owned title is the run-bound handoff from the macro's report: the
' only document this checker will ever close on the macro's behalf. It is
' never guessed from whatever happens to be active. The STEP reference
' document is independently owned by THIS checker: it is opened here and
' closed here with the same verified-close discipline (a live enumeration
' before and after), reported as check:ref_close.
'
' Expect file lines: feature:<name>=<kind>, dim:<name>=<metres>,
'   min_features=<n>, ref_bodies=<n>, volume_tol=<rel>, volume_tol_abs=<m3>,
'   placement_tol=<m>, volume_compare=skip
'   ref_bodies    expected solid body count of the STEP reference import
'                 (and of the replayed part); required unless skipped.
'   volume_tol    relative volume tolerance (REQUIRED unless skipped; the
'                 hardcoded relative tolerance is gone - this parameter is
'                 honored and validated at the tooling boundary).
'   volume_tol_abs  absolute volume floor in m3 (optional, default 0).
'   placement_tol centroid distance tolerance in metres (REQUIRED unless
'                 skipped).
'   volume_compare=skip  the synthetic corpus placeholder: no reference is
'                 loaded and the mass entries report skipped.
' Output file lines: check:<name>=pass|fail|skipped, detail:<name>=<text>,
'   volume:<side>=<value>. The mass comparison entry is ALWAYS emitted:
'   missing, invalid, nonpositive or unreadable values fail named and are
'   never an absent entry. Volume passes iff
'   |rebuilt - reference| <= max(volume_tol * reference, volume_tol_abs);
'   the body-count and placement checks run alongside it - volume
'   agreement alone never proves replay. The pure decision code lives in
'   check-part-logic.vbs (loaded below, also driven offline by
'   check-part-logic.tests.vbs); this file owns only the COM calls.
Option Explicit
Dim swApp, swDoc, expectPath, stepPath, outPath, ownedTitle, f, line, parts
Dim logicPath, logicText, logicFile
Dim mp, rebuiltText, rebuiltCx, rebuiltCy, rebuiltCz, rebuiltSolidsText, rebuiltAcquireErr
Dim bodiesArr
Dim skipVolume, noSolid, minFeatures, volumeTol, volumeTolAbs, placementTol, refBodies
Dim scan, scanFso
Dim treeCount, featWalk
Dim refDoc, refTitle, refText, refCx, refCy, refCz
Dim importErrDesc
Dim distSq, distErr, dist
Dim docTypeText, solidsText, sheetsText, refAcquireErr
Dim distText, blocks, dtRaw, refBodiesArr, refSheetsArr, refCloseLines
Dim preTitles, preErr, d, t2
If WScript.Arguments.Count < 4 Then
  WScript.Echo "usage: check-part.vbs <expect> <step-reference> <out> <owned-title>"
  WScript.Quit 4
End If
expectPath = WScript.Arguments(0)
stepPath = WScript.Arguments(1)
outPath = WScript.Arguments(2)
ownedTitle = WScript.Arguments(3)
If Trim(ownedTitle) = "" Then
  WScript.Echo "error: empty owned-title handoff; nothing will be checked or closed"
  WScript.Quit 4
End If

On Error Resume Next
Set swApp = CreateObject("SldWorks.Application")
If Err.Number <> 0 Then
  WScript.Echo "error: cannot attach to SolidWorks: " & Err.Description
  WScript.Quit 4
End If
Set swDoc = swApp.ActiveDoc
If swDoc Is Nothing Then
  WScript.Echo "error: no active document (the macro run must leave the part open)"
  WScript.Quit 5
End If
' The active document must be the handed-off one: the checks run on it and
' the close below targets the handoff by name. A mismatch refuses both.
If swDoc.GetTitle <> ownedTitle Then
  WScript.Echo "error: the active document is not the handed-off owned document"
  WScript.Quit 5
End If

Dim outFile
Dim fso
Set fso = CreateObject("Scripting.FileSystemObject")
Set outFile = fso.CreateTextFile(outPath, True)

' ---- the pure decision logic (shared with the offline test driver) ----
' Fail closed: without the logic module no verdict can be trusted, so the
' required entries are written by hand as failures and the checker exits.
logicPath = fso.GetParentFolderName(WScript.ScriptFullName) & "\check-part-logic.vbs"
Err.Clear
Set logicFile = fso.OpenTextFile(logicPath, 1)
If Err.Number <> 0 Then
  outFile.WriteLine "detail:mass=check-part-logic.vbs is missing; the decision logic cannot run"
  outFile.WriteLine "check:mass=fail"
  outFile.WriteLine "check:mass_vs_reference=fail"
  outFile.Close
  WScript.Quit 4
End If
logicText = logicFile.ReadAll
logicFile.Close
Err.Clear
ExecuteGlobal logicText
If Err.Number <> 0 Then
  outFile.WriteLine "detail:mass=check-part-logic.vbs does not execute: " & Err.Description
  outFile.WriteLine "check:mass=fail"
  outFile.WriteLine "check:mass_vs_reference=fail"
  outFile.Close
  WScript.Quit 4
End If

' ---- the rebuilt part's mass properties (acquired; verdicts below) ----
rebuiltText = ""
rebuiltSolidsText = ""
rebuiltAcquireErr = ""
Err.Clear
Set mp = swDoc.Extension.CreateMassProperty()
If Err.Number <> 0 Then
  rebuiltAcquireErr = "cannot read the rebuilt part's mass properties: " & Err.Description
Else
  Err.Clear
  If Not IsNumeric(mp.Volume) Then
    rebuiltAcquireErr = "the rebuilt volume did not come back numeric (vartype " & NumStr(VarType(mp.Volume)) & ")"
  Else
    rebuiltText = NumStr(mp.Volume)
    rebuiltAcquireErr = CentroidText(mp, rebuiltCx, rebuiltCy, rebuiltCz, "the rebuilt centroid")
  End If
End If
Err.Clear
bodiesArr = swDoc.GetBodies2(0, False)
rebuiltSolidsText = CountText(bodiesArr, Err.Number)

' ---- the expect parameters (raw text; validated by the logic module) ----
skipVolume = False
noSolid = False
minFeatures = 0
volumeTol = ""
volumeTolAbs = ""
placementTol = ""
refBodies = ""
Set scanFso = CreateObject("Scripting.FileSystemObject")
Set scan = scanFso.OpenTextFile(expectPath, 1)
Do Until scan.AtEndOfStream
  line = Trim(scan.ReadLine)
  If line = "volume_compare=skip" Then skipVolume = True
  If line = "solid=none" Then noSolid = True
  If Left(line, 12) = "min_features=" Then minFeatures = CInt(Mid(line, 13))
  If Left(line, 15) = "volume_tol_abs=" Then volumeTolAbs = Mid(line, 16)
  If Left(line, 11) = "volume_tol=" Then volumeTol = Mid(line, 12)
  If Left(line, 14) = "placement_tol=" Then placementTol = Mid(line, 15)
  If Left(line, 11) = "ref_bodies=" Then refBodies = Mid(line, 12)
Loop
scan.Close

' Tree walk: count features and check the minimum.
treeCount = 0
Err.Clear
Set featWalk = swDoc.FirstFeature
Do While Not featWalk Is Nothing
  treeCount = treeCount + 1
  Set featWalk = featWalk.GetNextFeature
Loop
outFile.WriteLine "detail:tree_count=" & NumStr(treeCount)
If treeCount >= minFeatures Then
  outFile.WriteLine "check:tree_count=pass"
Else
  outFile.WriteLine "check:tree_count=fail"
  outFile.WriteLine "detail:tree_count=expected at least " & NumStr(minFeatures)
End If

' ---- the STEP reference: import validation, mass, bodies, placement ----
' Every outcome is decided by the pure logic module; this block only
' acquires values and closes the reference document it opened.
refText = ""
docTypeText = "unreadable"
solidsText = "missing or unreadable"
sheetsText = "unreadable"
refAcquireErr = ""
refTitle = ""
distText = ""
blocks = ""
' FINDING 4 fix: ownership is established BEFORE the import. Documents
' already open belong to the user and are never closed; the snapshot
' decides later whether the reference title pre-existed.
preTitles = ""
preErr = 0
Err.Clear
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
If skipVolume Then
  Err.Clear
  blocks = MassSkipLines(rebuiltText, noSolid)
  If Err.Number <> 0 Then
    blocks = "detail:mass=the decision call failed: " & Err.Description & vbCrLf & _
      "check:mass=fail" & vbCrLf & "check:mass_vs_reference=skipped" & vbCrLf
  End If
  blocks = blocks & RefImportSkipLines()
  blocks = blocks & BodiesSkipLines()
  blocks = blocks & PlacementSkipLines()
  outFile.Write blocks
  outFile.WriteLine "check:ref_close=skipped"
Else
  ' The documented STEP-import shape (LoadFile4: FileName, ArgString,
  ' ImportData, Errors As Long OUT). This seat's VBScript cannot marshal
  ' the ByRef Errors output (RunMacro2's ByRef Long raises Type mismatch
  ' regardless of CLng), so ImportStepDoc walks the marshalling forms
  ' ONCE each and reconciles the ownership snapshot between attempts: a
  ' marshalling failure opens no document, but any unknown title stops
  ' the ladder as ambiguous ownership.
  If preErr <> 0 Then
    refAcquireErr = "cannot load the STEP reference: the ownership enumeration failed (err " & NumStr(preErr) & ") - refusing import"
    docTypeText = "unreadable"
    solidsText = "missing or unreadable"
    sheetsText = "unreadable"
  Else
    Err.Clear
    Set refDoc = ImportStepDoc(swApp, stepPath, preTitles, importErrDesc)
    If refDoc Is Nothing Then
      refAcquireErr = "cannot load the STEP reference: " & importErrDesc
      docTypeText = "unreadable"
      solidsText = "missing or unreadable"
      sheetsText = "unreadable"

    If Err.Number <> 0 Then refTitle = ""
    Err.Clear
    dtRaw = refDoc.GetType
    If Err.Number = 0 Then
      If dtRaw = 1 Then
        docTypeText = "part"
      ElseIf dtRaw = 2 Then
        docTypeText = "assembly"
      ElseIf dtRaw = 3 Then
        docTypeText = "drawing"
      Else
        docTypeText = "other"
      End If
    Else
      docTypeText = "unreadable"
    End If
    Err.Clear
    refBodiesArr = refDoc.GetBodies2(0, False)
    solidsText = CountText(refBodiesArr, Err.Number)
    Err.Clear
    refSheetsArr = refDoc.GetBodies2(1, False)
    sheetsText = CountText(refSheetsArr, Err.Number)
    Err.Clear
    Set mp = refDoc.Extension.CreateMassProperty()
    If Err.Number <> 0 Then
      refAcquireErr = "cannot read the STEP reference's mass properties: " & Err.Description
    Else
      Err.Clear
      If Not IsNumeric(mp.Volume) Then
        refAcquireErr = "the reference volume did not come back numeric (vartype " & NumStr(VarType(mp.Volume)) & ")"
      Else
        refText = NumStr(mp.Volume)
        refAcquireErr = CentroidText(mp, refCx, refCy, refCz, "the reference centroid")
      End If
    End If
    If rebuiltText <> "" And refText <> "" And rebuiltAcquireErr = "" And refAcquireErr = "" Then
      ' the distance computation is checked end to end: a failed read or
      ' an unrepresentable value is named, never coerced
      Err.Clear
      On Error Resume Next
      distSq = (rebuiltCx - refCx) ^ 2 + (rebuiltCy - refCy) ^ 2 + (rebuiltCz - refCz) ^ 2
      distErr = Err.Number
      If distErr <> 0 Then
        distText = "missing or invalid: the squared distance raised err " & NumStr(distErr)
        Err.Clear
      ElseIf distSq <> distSq Or distSq < 0 Then
        distText = "missing or invalid: the squared distance is not finite"
      ElseIf distSq = 0 Then
        distText = NumStr(0)
      Else
        dist = Sqr(distSq)
        If Err.Number <> 0 Then
          distText = "missing or invalid: the square root raised err " & NumStr(Err.Number)
          Err.Clear
        Else
          distText = NumStr(dist)
        End If
      End If
      On Error GoTo 0
    Else
      distText = "missing or unreadable"
    End If
    End If
  End If

  ' The reference document is opened and closed by this checker - verified
  ' closed by a live enumeration (the CloseOwnedDoc discipline), reported
  ' as check:ref_close on every path below.
  refCloseLines = ""
  If refDoc Is Nothing Then
    refCloseLines = "check:ref_close=skipped" & vbCrLf
    refCloseLines = refCloseLines & "detail:ref_close=no reference document was opened" & vbCrLf
  ElseIf preErr <> 0 Then
    refCloseLines = "check:ref_close=fail" & vbCrLf & "detail:ref_close=the pre-import document enumeration failed; ownership of the reference close cannot be established" & vbCrLf
  ElseIf IsPreExistingTitle(preTitles, refTitle) Then
    refCloseLines = "check:ref_close=fail" & vbCrLf & "detail:ref_close=the reference title was already open before the import; refusing to close a pre-existing document (the imported document is preserved, not closed)" & vbCrLf
  Else
    refCloseLines = VerifiedCloseLines(swApp, refDoc, refTitle)
  End If

  ' The verdicts: the required mass entry is always emitted, named.
  Err.Clear
  blocks = MassLines(rebuiltText, refText, volumeTol, volumeTolAbs)
  If Err.Number <> 0 Then
    blocks = "detail:mass=the decision call failed: " & Err.Description & vbCrLf & _
      "check:mass=fail" & vbCrLf & "check:mass_vs_reference=fail" & vbCrLf
  End If
  blocks = blocks & RefImportLines(refBodies, docTypeText, solidsText, sheetsText)
  blocks = blocks & BodiesLines(refBodies, rebuiltSolidsText)
  blocks = blocks & PlacementLines(distText, placementTol)
  If refAcquireErr <> "" Then
    blocks = blocks & "detail:mass=" & refAcquireErr & vbCrLf
  End If
  If rebuiltAcquireErr <> "" Then
    blocks = blocks & "detail:mass=" & rebuiltAcquireErr & vbCrLf
  End If
  outFile.Write blocks
  outFile.Write refCloseLines
End If

' ---- expected features and dimensions ----
Dim feat
On Error Resume Next
Const swDocPART = 1
Dim walkFso
Set walkFso = CreateObject("Scripting.FileSystemObject")
Set f = walkFso.OpenTextFile(expectPath, 1)
Do Until f.AtEndOfStream
  line = Trim(f.ReadLine)
  If Len(line) > 0 And Left(line, 1) <> "#" Then
    parts = Split(line, "=", 2)
    If UBound(parts) = 1 Then
      If Left(parts(0), 8) = "feature:" Then
        Dim featName
        featName = Mid(parts(0), 9)
        Err.Clear
        Set feat = swDoc.FeatureByName(featName)
        If Err.Number = 0 And Not feat Is Nothing Then
          If parts(1) = "" Or feat.GetTypeName2() = parts(1) Then
            outFile.WriteLine "check:feature:" & featName & "=pass"
          Else
            outFile.WriteLine "check:feature:" & featName & "=fail"
            outFile.WriteLine "detail:feature:" & featName & "=type " & feat.GetTypeName2() & " expected " & parts(1)
          End If
        Else
          outFile.WriteLine "check:feature:" & featName & "=fail"
          outFile.WriteLine "detail:feature:" & featName & "=missing from the tree"
        End If
      ElseIf Left(parts(0), 4) = "dim:" Then
        Dim dimName, dp, expVal
        dimName = Mid(parts(0), 5)
        Err.Clear
        Set dp = swDoc.Parameter(dimName)
        If Err.Number = 0 And Not dp Is Nothing Then
          ' VBScript has no Val (that is VBA): parse through the logic
          ' module's locale-free parser and fail closed on bad text.
          If Not ParsePeriod(parts(1), expVal) Then
            outFile.WriteLine "check:dim:" & dimName & "=fail"
            outFile.WriteLine "detail:dim:" & dimName & "=expected value " & parts(1) & " is not a period-decimal number"
          ElseIf Abs(dp.SystemValue - expVal) < 0.000001 Then
            outFile.WriteLine "check:dim:" & dimName & "=pass"
          Else
            outFile.WriteLine "check:dim:" & dimName & "=fail"
            outFile.WriteLine "detail:dim:" & dimName & "=value " & NumStr(dp.SystemValue) & " expected " & parts(1)
          End If
        Else
          outFile.WriteLine "check:dim:" & dimName & "=fail"
          outFile.WriteLine "detail:dim:" & dimName & "=missing"
        End If
      End If
    End If
  End If
Loop
f.Close

' The owner rule: runs leave no documents open. Close exactly the
' handed-off document - the run-bound title from the macro's report, never
' whatever is active - and verify the close by a live enumeration. An
' empty or ambiguous title is refused; every enumeration call is checked
' immediately and any failure leaves the close unverified.
Dim closeTitle, cnt, isOurs
closeTitle = ownedTitle
If Trim(closeTitle) = "" Then
  outFile.WriteLine "check:close:=fail"
  outFile.WriteLine "detail:close:=empty handoff; nothing closed"
  outFile.Close
  WScript.Quit 1
End If
cnt = 0
isOurs = False
Err.Clear
Set d = swApp.GetFirstDocument
If Err.Number <> 0 Then
  outFile.WriteLine "check:close:=fail"
  outFile.Close
  WScript.Quit 1
End If
Do While Not d Is Nothing
  Err.Clear
  t2 = d.GetTitle
  If Err.Number <> 0 Then
    outFile.WriteLine "check:close:=fail"
    outFile.Close
    WScript.Quit 1
  End If
  If t2 = closeTitle Then
    cnt = cnt + 1
    If d Is swDoc Then isOurs = True
  End If
  Err.Clear
  Set d = d.GetNext
  If Err.Number <> 0 Then
    outFile.WriteLine "check:close:=fail"
    outFile.Close
    WScript.Quit 1
  End If
Loop
If cnt <> 1 Or Not isOurs Then
  outFile.WriteLine "check:close:=fail"
  outFile.WriteLine "detail:close:=the handoff does not identify exactly the checked document"
  outFile.Close
  WScript.Quit 1
End If
Err.Clear
swApp.CloseDoc closeTitle
If Err.Number <> 0 Then
  outFile.WriteLine "check:close:=fail"
  outFile.Close
  WScript.Quit 1
End If
cnt = 0
Err.Clear
Set d = swApp.GetFirstDocument
If Err.Number <> 0 Then
  outFile.WriteLine "check:close:=fail"
  outFile.Close
  WScript.Quit 1
End If
Do While Not d Is Nothing
  Err.Clear
  t2 = d.GetTitle
  If Err.Number <> 0 Then
    outFile.WriteLine "check:close:=fail"
    outFile.Close
    WScript.Quit 1
  End If
  If t2 = closeTitle Then cnt = cnt + 1
  Err.Clear
  Set d = d.GetNext
  If Err.Number <> 0 Then
    outFile.WriteLine "check:close:=fail"
    outFile.Close
    WScript.Quit 1
  End If
Loop
If cnt <> 0 Then
  outFile.WriteLine "check:close:=fail"
  outFile.Close
  WScript.Quit 1
End If
outFile.WriteLine "check:close:=pass"
outFile.Close
WScript.Quit 0

' Typed acquisition of a body count from GetBodies2(BodyType,
' BVisibleOnly): an unreadable call, an unexpected return shape or
' unreadable array bounds is a NAMED refusal; a typed Empty/Null return
' proves the body set is empty. Never a guessed count.
Function CountText(ByVal res, ByVal arrErr)
  Dim lo2, hi2
  If arrErr <> 0 Then
    CountText = "missing or unreadable"
    Exit Function
  End If
  If IsArray(res) Then
    Err.Clear
    On Error Resume Next
    lo2 = LBound(res)
    hi2 = UBound(res)
    If Err.Number = 0 Then
      CountText = NumStr(hi2 - lo2 + 1)
    Else
      CountText = "unexpected return shape (array bounds unreadable)"
    End If
    On Error GoTo 0
    Exit Function
  End If
  If IsEmpty(res) Or IsNull(res) Then
    CountText = "0"
    Exit Function
  End If
  CountText = "unexpected return shape (vartype " & NumStr(VarType(res)) & ")"
End Function

' Verified close of the independently owned STEP reference document: the
' CloseOwnedDoc discipline - a live enumeration must identify exactly the
' opened object before the close and prove its absence after. Every
' failure path reports check:ref_close=fail and leaves the claim unmade.
Function VerifiedCloseLines(ByVal app, ByVal doc, ByVal title)
  Dim out, d2, t2, cnt, isOurs
  On Error Resume Next
  out = ""
  If Trim(title) = "" Then
    out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=empty reference title; nothing closed" & vbCrLf
    VerifiedCloseLines = out
    Exit Function
  End If
  cnt = 0
  isOurs = False
  Err.Clear
  Set d2 = app.GetFirstDocument
  If Err.Number <> 0 Then
    out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=the open-document enumeration failed before the close" & vbCrLf
    VerifiedCloseLines = out
    Exit Function
  End If
  Do While Not d2 Is Nothing
    Err.Clear
    t2 = d2.GetTitle
    If Err.Number <> 0 Then
      out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=a document title could not be read before the close" & vbCrLf
      VerifiedCloseLines = out
      Exit Function
    End If
    If t2 = title Then
      cnt = cnt + 1
      If d2 Is doc Then isOurs = True
    End If
    Err.Clear
    Set d2 = d2.GetNext
    If Err.Number <> 0 Then
      out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=the open-document enumeration broke before the close" & vbCrLf
      VerifiedCloseLines = out
      Exit Function
    End If
  Loop
  If cnt <> 1 Or Not isOurs Then
    out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=the reference title does not identify exactly the opened document" & vbCrLf
    VerifiedCloseLines = out
    Exit Function
  End If
  Err.Clear
  app.CloseDoc title
  If Err.Number <> 0 Then
    out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=CloseDoc failed: " & Err.Description & vbCrLf
    VerifiedCloseLines = out
    Exit Function
  End If
  cnt = 0
  Err.Clear
  Set d2 = app.GetFirstDocument
  If Err.Number <> 0 Then
    out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=the open-document enumeration failed after the close" & vbCrLf
    VerifiedCloseLines = out
    Exit Function
  End If
  Do While Not d2 Is Nothing
    Err.Clear
    t2 = d2.GetTitle
    If Err.Number <> 0 Then
      out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=a document title could not be read after the close" & vbCrLf
      VerifiedCloseLines = out
      Exit Function
    End If
    If t2 = title Then cnt = cnt + 1
    Err.Clear
    Set d2 = d2.GetNext
    If Err.Number <> 0 Then
      out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=the open-document enumeration broke after the close" & vbCrLf
      VerifiedCloseLines = out
      Exit Function
    End If
  Loop
  If cnt <> 0 Then
    out = "check:ref_close=fail" & vbCrLf & "detail:ref_close=the reference document still answers to its title after the close" & vbCrLf
    VerifiedCloseLines = out
    Exit Function
  End If
  out = "check:ref_close=pass" & vbCrLf
  VerifiedCloseLines = out
End Function

' One LoadFile4 marshalling form, attempted once. The ByRef Errors output
' is a known VBScript late-binding failure point on this seat (the
' RunMacro2 ByRef Long precedent), so the form list covers the shapes
' VBScript can pass; every outcome is captured as errNum/errDesc and a
' silent Nothing return is a named error.
Function TryLoadForm(ByVal app, ByVal stepPath, ByVal importData, ByVal formIndex, ByRef errNum, ByRef errDesc)
  Dim outLong
  Set TryLoadForm = Nothing
  errNum = 0
  errDesc = ""
  On Error Resume Next
  Err.Clear
  Select Case formIndex
    Case 1
      outLong = CLng(0)
      Set TryLoadForm = app.LoadFile4(stepPath, "r", importData, outLong)
    Case 2
      Set TryLoadForm = app.LoadFile4(stepPath, "r", importData, CLng(0))
    Case 3
      outLong = CLng(0)
      Set TryLoadForm = app.LoadFile4(stepPath, "r", Nothing, outLong)
    Case 4
      Set TryLoadForm = app.LoadFile4(stepPath, "r", Nothing, CLng(0))
  End Select
  errNum = Err.Number
  errDesc = Err.Description
  Err.Clear
  On Error GoTo 0
  If errNum = 0 Then
    If TryLoadForm Is Nothing Then
      errNum = 424
      errDesc = "LoadFile4 returned nothing"
    End If
  End If
End Function

' Ownership reconciliation between import attempts: every document must
' carry a title from the pre-import snapshot. An unknown title means an
' attempt may have opened a document without returning its handle - the
' ladder stops and names it, and nothing is closed (the user owns what
' cannot be attributed).
Function OwnershipReconciled(ByVal app, ByVal preSnapshot, ByRef note)
  Dim d2, t3, errN
  OwnershipReconciled = False
  note = ""
  On Error Resume Next
  Err.Clear
  Set d2 = app.GetFirstDocument
  errN = Err.Number
  If errN <> 0 Then
    note = "the ownership re-walk failed (err " & NumStr(errN) & ")"
    Err.Clear
    On Error GoTo 0
    Exit Function
  End If
  Do While Not d2 Is Nothing
    Err.Clear
    t3 = d2.GetTitle
    If Err.Number <> 0 Then
      note = "a document title could not be read during reconciliation"
      Err.Clear
      On Error GoTo 0
      Exit Function
    End If
    If InStr(1, preSnapshot, t3 & vbCrLf, vbBinaryCompare) = 0 Then
      note = "a document appeared without a handle: <" & t3 & ">"
      On Error GoTo 0
      Exit Function
    End If
    Err.Clear
    Set d2 = d2.GetNext
    If Err.Number <> 0 Then
      note = "the ownership re-walk failed mid-scan (err " & NumStr(Err.Number) & ")"
      Err.Clear
      On Error GoTo 0
      Exit Function
    End If
  Loop
  On Error GoTo 0
  OwnershipReconciled = True
End Function

' STEP import through the marshalling forms above, reconciled between
' attempts. The failure text names every attempt.
Function ImportStepDoc(ByVal app, ByVal stepPath, ByVal preSnapshot, ByRef errText)
  Dim importData, res, i, n, dsc, recon
  Set ImportStepDoc = Nothing
  errText = ""
  Err.Clear
  On Error Resume Next
  Set importData = app.GetImportFileData(stepPath)
  n = Err.Number
  dsc = Err.Description
  Err.Clear
  On Error GoTo 0
  If n <> 0 Then
    errText = "GetImportFileData err " & NumStr(n) & ": " & dsc
    Exit Function
  End If
  For i = 1 To 4
    Set res = TryLoadForm(app, stepPath, importData, i, n, dsc)
    If Not res Is Nothing Then
      Set ImportStepDoc = res
      Exit Function
    End If
    errText = errText & "form " & NumStr(i) & " err " & NumStr(n) & ": " & dsc & "; "
    If Not OwnershipReconciled(app, preSnapshot, recon) Then
      errText = errText & "ambiguous ownership after form " & NumStr(i) & ": " & recon
      Exit Function
    End If
  Next
End Function
