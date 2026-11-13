' check-part.vbs - verify the rebuilt part in the running SolidWorks session
' against an expect file, and against the STEP reference body's mass
' properties. Invoked only by tools/seat-harness.ps1.
'
' Usage: cscript //nologo check-part.vbs <expect-file> <step-reference> <out-file>
' Expect file lines: feature:<name>=<kind>, dim:<name>=<metres>, volume_tol=<rel>
' Output file lines: check:<name>=pass|fail, detail:<name>=<text>, volume:<side>=<value>
Option Explicit
Dim swApp, swDoc, expectPath, stepPath, outPath, f, line, parts
If WScript.Arguments.Count < 3 Then
  WScript.Echo "usage: check-part.vbs <expect> <step-reference> <out>"
  WScript.Quit 4
End If
expectPath = WScript.Arguments(0)
stepPath = WScript.Arguments(1)
outPath = WScript.Arguments(2)

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

Dim outFile
Dim fso
Set fso = CreateObject("Scripting.FileSystemObject")
Set outFile = fso.CreateTextFile(outPath, True)

' ---- the rebuilt part's mass properties ----
Dim rebuiltVol, mp
Err.Clear
Set mp = swDoc.Extension.CreateMassProperty()
If Err.Number = 0 Then
  rebuiltVol = mp.Volume
  outFile.WriteLine "volume:rebuilt=" & Trim(Str(rebuiltVol))
Else
  outFile.WriteLine "check:mass=fail"
  outFile.WriteLine "detail:mass=cannot read the rebuilt part's mass properties: " & Err.Description
End If

' ---- the STEP reference body's mass properties (both sides by SolidWorks);
' skipped when the expect file says the reference is not a real export ----
Dim refDoc, refVol, skipVolume, minFeatures, treeCount, featWalk
skipVolume = False
minFeatures = 0
Dim scan
Set scan = CreateObject("Scripting.FileSystemObject").OpenTextFile(expectPath, 1)
Do Until scan.AtEndOfStream
  line = Trim(scan.ReadLine)
  If line = "volume_compare=skip" Then skipVolume = True
  If Left(line, 12) = "min_features=" Then minFeatures = CInt(Mid(line, 13))
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
outFile.WriteLine "detail:tree_count=" & Trim(Str(treeCount))
If treeCount >= minFeatures Then
  outFile.WriteLine "check:tree_count=pass"
Else
  outFile.WriteLine "check:tree_count=fail"
  outFile.WriteLine "detail:tree_count=expected at least " & Trim(Str(minFeatures))
End If

If skipVolume Then
  outFile.WriteLine "check:mass=skipped"
End If
Dim refDoc, refVol
If Not skipVolume Then
Err.Clear
Set refDoc = swApp.LoadFile4(stepPath, "r", Nothing, "")
If Err.Number <> 0 Or refDoc Is Nothing Then
  outFile.WriteLine "check:step_reference=fail"
  outFile.WriteLine "detail:step_reference=cannot load the STEP reference: " & Err.Description
Else
  Set mp = refDoc.Extension.CreateMassProperty()
  If Err.Number = 0 Then
    refVol = mp.Volume
    outFile.WriteLine "volume:step_reference=" & Trim(Str(refVol))
    If rebuiltVol > 0 And refVol > 0 Then
      If Abs(rebuiltVol - refVol) / refVol < 0.001 Then
        outFile.WriteLine "check:mass_vs_reference=pass"
      Else
        outFile.WriteLine "check:mass_vs_reference=fail"
        outFile.WriteLine "detail:mass_vs_reference=rebuilt " & Trim(Str(rebuiltVol)) & " reference " & Trim(Str(refVol))
      End If
    End If
  End If
  refDoc.Quit False
End If
End If
' Volume equality is only expected for fallback-free corpus items; the
' harness decides per item whether the comparison applies.

' ---- expected features and dimensions ----
Dim feat As Object
On Error Resume Next
Const swDocPART = 1
Set f = CreateObject("Scripting.FileSystemObject").OpenTextFile(expectPath, 1)
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
        Dim dimName, dp
        dimName = Mid(parts(0), 5)
        Err.Clear
        Set dp = swDoc.Parameter(dimName)
        If Err.Number = 0 And Not dp Is Nothing Then
          If Abs(dp.SystemValue - CDbl(parts(1))) < 0.000001 Then
            outFile.WriteLine "check:dim:" & dimName & "=pass"
          Else
            outFile.WriteLine "check:dim:" & dimName & "=fail"
            outFile.WriteLine "detail:dim:" & dimName & "=value " & Trim(Str(dp.SystemValue)) & " expected " & parts(1)
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
outFile.Close
WScript.Quit 0
