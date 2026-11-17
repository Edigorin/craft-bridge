' check-part-logic.vbs - the PURE decision logic of tools/check-part.vbs:
' expect-parameter validation and the check verdicts. No COM, no
' filesystem, no SolidWorks. Every function maps validated TEXT to the
' exact report lines the checker writes, so the decision code exists once
' and is drivable without a seat: tools/check-part.vbs loads this file
' with ExecuteGlobal on the seat path, and tools/check-part-logic.tests.vbs
' loads the same bytes and drives it with fixture inputs offline.
'
' Report contract (the harness and the real-part runner parse these):
'   check:<name>=pass|fail|skipped   - one verdict per check, ALWAYS
'   detail:<name>=<text>             - the named reason / the values used
'   volume:<side>=<value>            - SI cubic metres, period separator
' The required mass comparison entry is ALWAYS emitted - a missing,
' invalid, nonpositive or unreadable volume on either side is
' check:mass=fail and check:mass_vs_reference=fail, never an absent entry.
'
' Volume comparison model (documented here and in check-part.vbs):
'   pass iff |rebuilt - reference| <= max(rel_tol * reference, abs_tol)
' Both tolerances are reported on every verdict. rel_tol is the documented
' volume_tol parameter (validated at this boundary; there is no hardcoded
' default). abs_tol is the documented volume_tol_abs floor (optional,
' default 0 - the relative bound alone then governs).
Option Explicit

' Locale-independent number text (period decimal separator). CStr follows
' the host locale; the separator is extracted from CStr(1.5), never
' assumed. Used for every number this module prints.
Function NumStr(ByVal n)
  Dim s, sep
  s = CStr(n)
  sep = Replace(Replace(CStr(1.5), "1", ""), "5", "")
  If sep <> "." And sep <> "" Then s = Replace(s, sep, ".")
  NumStr = s
End Function

' A positive, finite number as text: "" when valid, else the named reason.
Function PositiveError(ByVal text)
  Dim v
  PositiveError = "missing or unreadable"
  If Trim(text) = "" Then Exit Function
  If Not IsNumeric(Trim(text)) Then
    PositiveError = "not numeric"
    Exit Function
  End If
  Err.Clear
  On Error Resume Next
  v = CDbl(Trim(text))
  If Err.Number <> 0 Then
    PositiveError = "not numeric"
    On Error GoTo 0
    Exit Function
  End If
  On Error GoTo 0
  If v <> v Or Abs(v) > 1E+100 Then
    PositiveError = "not finite"
    Exit Function
  End If
  If v <= 0 Then
    PositiveError = "nonpositive"
    Exit Function
  End If
  PositiveError = ""
End Function

' A finite number >= 0 as text: "" when valid, else the named reason.
Function NonnegError(ByVal text)
  Dim v
  NonnegError = "missing or invalid"
  If Trim(text) = "" Then Exit Function
  If Not IsNumeric(Trim(text)) Then
    NonnegError = "not numeric"
    Exit Function
  End If
  Err.Clear
  On Error Resume Next
  v = CDbl(Trim(text))
  If Err.Number <> 0 Then
    NonnegError = "not numeric"
    On Error GoTo 0
    Exit Function
  End If
  On Error GoTo 0
  If v <> v Or Abs(v) > 1E+100 Then
    NonnegError = "not finite"
    Exit Function
  End If
  If v < 0 Then
    NonnegError = "negative"
    Exit Function
  End If
  NonnegError = ""
End Function

' A body count as text: "" when a positive integer, else the named reason.
Function CountError(ByVal text)
  Dim v
  CountError = "missing or invalid"
  If Trim(text) = "" Then Exit Function
  If Not IsNumeric(Trim(text)) Then
    CountError = "not numeric"
    Exit Function
  End If
  Err.Clear
  On Error Resume Next
  v = CDbl(Trim(text))
  If Err.Number <> 0 Then
    CountError = "not numeric"
    On Error GoTo 0
    Exit Function
  End If
  On Error GoTo 0
  If v <> v Or Abs(v) > 1E+100 Then
    CountError = "not finite"
    Exit Function
  End If
  If v <> Fix(v) Then
    CountError = "not an integer"
    Exit Function
  End If
  If v < 1 Then
    CountError = "nonpositive"
    Exit Function
  End If
  CountError = ""
End Function

' The REQUIRED mass block. The two volumes are SI cubic metres; relText is
' the volume_tol parameter, absText the optional volume_tol_abs floor
' (empty text = 0). The block ALWAYS ends with both verdicts - the
' fail-closed contract is structural, not conditional.
Function MassLines(ByVal rebuiltText, ByVal refText, ByVal relText, ByVal absText)
  Dim out, errR, errF, errT, errA, absText2
  Dim rebuilt, refVol, rel, absTol, limit, diff, verdict
  out = ""
  absText2 = absText
  If Trim(absText2) = "" Then absText2 = "0"
  errR = PositiveError(rebuiltText)
  errF = PositiveError(refText)
  errT = NonnegError(relText)
  errA = NonnegError(absText2)
  If errR = "" Then
    out = out & "volume:rebuilt=" & NumStr(CDbl(Trim(rebuiltText))) & vbCrLf
  End If
  If errF = "" Then
    out = out & "volume:step_reference=" & NumStr(CDbl(Trim(refText))) & vbCrLf
  End If
  If errR <> "" Or errF <> "" Or errT <> "" Or errA <> "" Then
    ' Fail closed and name each cause; the verdicts are still emitted.
    If errR <> "" Then out = out & "detail:mass=rebuilt volume " & errR & ": " & rebuiltText & vbCrLf
    If errF <> "" Then out = out & "detail:mass=reference volume " & errF & ": " & refText & vbCrLf
    If errT <> "" Then out = out & "detail:mass=volume_tol " & errT & ": " & relText & vbCrLf
    If errA <> "" Then out = out & "detail:mass=volume_tol_abs " & errA & ": " & absText2 & vbCrLf
    out = out & "detail:mass_vs_reference=comparison refused: a required value is missing or invalid" & vbCrLf
    out = out & "check:mass=fail" & vbCrLf
    out = out & "check:mass_vs_reference=fail" & vbCrLf
    MassLines = out
    Exit Function
  End If
  rebuilt = CDbl(Trim(rebuiltText))
  refVol = CDbl(Trim(refText))
  rel = CDbl(Trim(relText))
  absTol = CDbl(Trim(absText2))
  diff = Abs(rebuilt - refVol)
  limit = rel * refVol
  If absTol > limit Then limit = absTol
  verdict = "fail"
  If diff <= limit Then verdict = "pass"
  out = out & "detail:mass_vs_reference=rebuilt " & NumStr(rebuilt) & " reference " & NumStr(refVol) & " difference " & NumStr(diff) & " rel_tol " & NumStr(rel) & " abs_tol " & NumStr(absTol) & " limit " & NumStr(limit) & vbCrLf
  out = out & "check:mass=" & verdict & vbCrLf
  out = out & "check:mass_vs_reference=" & verdict & vbCrLf
  MassLines = out
End Function

' The synthetic placeholder (volume_compare=skip): the corpus keeps its
' skip-with-placeholder behavior; every required entry is still emitted.
' An unreadable rebuilt volume still fails check:mass (the data gate), as
' the historical path did.
Function MassSkipLines(ByVal rebuiltText)
  Dim out, errR
  out = ""
  errR = PositiveError(rebuiltText)
  If errR = "" Then
    out = out & "volume:rebuilt=" & NumStr(CDbl(Trim(rebuiltText))) & vbCrLf
    out = out & "detail:mass=volume comparison skipped by expect (volume_compare=skip)" & vbCrLf
    out = out & "check:mass=skipped" & vbCrLf
    out = out & "check:mass_vs_reference=skipped" & vbCrLf
  Else
    out = out & "detail:mass=rebuilt volume " & errR & ": " & rebuiltText & vbCrLf
    out = out & "detail:mass=volume comparison skipped by expect (volume_compare=skip)" & vbCrLf
    out = out & "check:mass=fail" & vbCrLf
    out = out & "check:mass_vs_reference=skipped" & vbCrLf
  End If
  MassSkipLines = out
End Function

' The STEP reference import verdict: expected solid body count vs found.
' docTypeText is "part", "assembly", "drawing", "other" or "unreadable";
' solidsText/sheetsText are counts as text ("missing or unreadable" on a
' failed read). Assembly, surface/empty and incomplete imports fail named.
Function RefImportLines(ByVal expectedText, ByVal docTypeText, ByVal solidsText, ByVal sheetsText)
  Dim out, errE, errS, expected, solids, sheets
  out = ""
  errE = CountError(expectedText)
  errS = CountError(solidsText)
  If errS = "" Then
    out = out & "detail:step_reference=expected " & Trim(expectedText) & " solid bodies, found " & Trim(solidsText) & " solids and " & Trim(sheetsText) & " sheets, document type " & docTypeText & vbCrLf
  Else
    out = out & "detail:step_reference=expected " & Trim(expectedText) & " solid bodies, found (" & errS & ": " & solidsText & "), document type " & docTypeText & vbCrLf
  End If
  If errE <> "" Then
    out = out & "detail:step_reference=the expected solid body count is " & errE & ": " & expectedText & vbCrLf
    out = out & "check:step_reference=fail" & vbCrLf
    RefImportLines = out
    Exit Function
  End If
  If docTypeText = "assembly" Then
    out = out & "detail:step_reference=the STEP reference imported as an assembly; a single part is expected" & vbCrLf
    out = out & "check:step_reference=fail" & vbCrLf
    RefImportLines = out
    Exit Function
  End If
  If docTypeText <> "part" Then
    out = out & "detail:step_reference=the reference document is not a part: " & docTypeText & vbCrLf
    out = out & "check:step_reference=fail" & vbCrLf
    RefImportLines = out
    Exit Function
  End If
  If errS <> "" Then
    out = out & "detail:step_reference=the solid body count is " & errS & ": " & solidsText & vbCrLf
    out = out & "check:step_reference=fail" & vbCrLf
    RefImportLines = out
    Exit Function
  End If
  expected = CDbl(Trim(expectedText))
  solids = CDbl(Trim(solidsText))
  sheets = 0
  If IsNumeric(Trim(sheetsText)) Then sheets = CDbl(Trim(sheetsText))
  If solids = 0 Then
    If sheets > 0 Then
      out = out & "detail:step_reference=a surface import: no solid body where " & NumStr(expected) & " were expected" & vbCrLf
    Else
      out = out & "detail:step_reference=no solid body in the import (empty body set) where " & NumStr(expected) & " were expected" & vbCrLf
    End If
    out = out & "check:step_reference=fail" & vbCrLf
    RefImportLines = out
    Exit Function
  End If
  If solids < expected Then
    out = out & "detail:step_reference=incomplete import: found " & NumStr(solids) & " of " & NumStr(expected) & " expected solid bodies" & vbCrLf
    out = out & "check:step_reference=fail" & vbCrLf
    RefImportLines = out
    Exit Function
  End If
  If solids > expected Then
    out = out & "detail:step_reference=the import carries " & NumStr(solids) & " solid bodies where " & NumStr(expected) & " were expected (an assembly or multi-body import)" & vbCrLf
    out = out & "check:step_reference=fail" & vbCrLf
    RefImportLines = out
    Exit Function
  End If
  out = out & "check:step_reference=pass" & vbCrLf
  RefImportLines = out
End Function

Function RefImportSkipLines()
  RefImportSkipLines = "check:step_reference=skipped" & vbCrLf
End Function

' The rebuilt part's solid body count against the expected count (the
' body-count check that runs alongside the volume comparison: volume
' agreement alone never proves replay).
Function BodiesLines(ByVal expectedText, ByVal rebuiltText)
  Dim out, errE, errR, expected, rebuilt
  out = ""
  errE = CountError(expectedText)
  errR = CountError(rebuiltText)
  If errR = "" Then
    out = out & "detail:bodies=rebuilt " & Trim(rebuiltText) & " expected " & Trim(expectedText) & vbCrLf
  Else
    out = out & "detail:bodies=rebuilt (" & errR & ": " & rebuiltText & ") expected " & Trim(expectedText) & vbCrLf
  End If
  If errE <> "" Then
    out = out & "detail:bodies=the expected body count is " & errE & ": " & expectedText & vbCrLf
    out = out & "check:bodies=fail" & vbCrLf
    BodiesLines = out
    Exit Function
  End If
  If errR <> "" Then
    out = out & "detail:bodies=the rebuilt body count is " & errR & ": " & rebuiltText & vbCrLf
    out = out & "check:bodies=fail" & vbCrLf
    BodiesLines = out
    Exit Function
  End If
  expected = CDbl(Trim(expectedText))
  rebuilt = CDbl(Trim(rebuiltText))
  If rebuilt <> expected Then
    out = out & "detail:bodies=the rebuilt part carries " & NumStr(rebuilt) & " solid bodies where " & NumStr(expected) & " were expected" & vbCrLf
    out = out & "check:bodies=fail" & vbCrLf
    BodiesLines = out
    Exit Function
  End If
  out = out & "check:bodies=pass" & vbCrLf
  BodiesLines = out
End Function

Function BodiesSkipLines()
  BodiesSkipLines = "check:bodies=skipped" & vbCrLf
End Function

' The placement check (the second guard alongside volume): the distance
' between the rebuilt and reference volume centroids, in metres, against
' the documented placement_tol parameter (required here - no default).
Function PlacementLines(ByVal distText, ByVal tolText)
  Dim out, errD, errT, dist, tol
  out = ""
  errD = NonnegError(distText)
  errT = NonnegError(tolText)
  If errD = "" Then
    out = out & "detail:placement=centroid distance " & NumStr(CDbl(Trim(distText))) & " m, tolerance " & Trim(tolText) & " m" & vbCrLf
  Else
    out = out & "detail:placement=centroid distance (" & errD & ": " & distText & "), tolerance " & Trim(tolText) & " m" & vbCrLf
  End If
  If errT <> "" Then
    out = out & "detail:placement=placement_tol " & errT & ": " & tolText & vbCrLf
    out = out & "check:placement=fail" & vbCrLf
    PlacementLines = out
    Exit Function
  End If
  If errD <> "" Then
    out = out & "detail:placement=the centroid distance is " & errD & ": " & distText & vbCrLf
    out = out & "check:placement=fail" & vbCrLf
    PlacementLines = out
    Exit Function
  End If
  dist = CDbl(Trim(distText))
  tol = CDbl(Trim(tolText))
  If dist > tol Then
    out = out & "detail:placement=the rebuilt body sits " & NumStr(dist) & " m from the reference centroid, past the " & NumStr(tol) & " m tolerance" & vbCrLf
    out = out & "check:placement=fail" & vbCrLf
    PlacementLines = out
    Exit Function
  End If
  out = out & "check:placement=pass" & vbCrLf
  PlacementLines = out
End Function

Function PlacementSkipLines()
  PlacementSkipLines = "check:placement=skipped" & vbCrLf
End Function
