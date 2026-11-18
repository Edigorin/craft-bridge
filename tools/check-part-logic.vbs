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

' Strict period-decimal parsing: locale-INDEPENDENT. The text shape is
' validated explicitly (optional sign, digits, at most one period, optional
' exponent) and the value is converted with Val, which always reads a
' period - unlike IsNumeric/CDbl, which read "0.01" through the host's
' decimal separator and reject it on comma-decimal Windows. Every number
' these modules parse arrives as period-decimal text (NumStr output or the
' expect file), so exactly that shape is accepted.
Function ParsePeriod(ByVal text, ByRef value)
  Dim s, i, c, n, digits, dots, expSeen
  ParsePeriod = False
  value = 0
  s = Trim(text)
  n = Len(s)
  If n = 0 Then Exit Function
  i = 1
  c = Mid(s, i, 1)
  If c = "+" Or c = "-" Then i = i + 1
  digits = 0
  dots = 0
  expSeen = False
  Do While i <= n
    c = Mid(s, i, 1)
    If c >= "0" And c <= "9" Then
      digits = digits + 1
    ElseIf c = "." And Not expSeen Then
      dots = dots + 1
      If dots > 1 Then Exit Function
    ElseIf (c = "e" Or c = "E") And Not expSeen And digits > 0 Then
      expSeen = True
      digits = 0
      If i + 1 <= n Then
        c = Mid(s, i + 1, 1)
        If c = "+" Or c = "-" Then i = i + 1
      End If
    Else
      Exit Function
    End If
    i = i + 1
  Loop
  If digits = 0 Then Exit Function
  ' Convert exactly from the proven shape. VBScript has no Val (that is a
  ' VBA function - calling it here raised a swallowed Type mismatch and
  ' made every parse fail) and CDbl is locale-sensitive, so accumulate the
  ' mantissa by arithmetic and apply the decimal and exponent scales: no
  ' locale surface at all.
  value = PeriodValue(s)
  ParsePeriod = True
End Function

' The numeric value of period-decimal text whose shape ParsePeriod has
' proven: optional sign, mantissa digits, at most one period, optional
' exponent. Digit accumulation only - see ParsePeriod.
Function PeriodValue(ByVal s)
  Dim i, c, n, sign, mant, frac, expo, esign, inExp, inFrac, k, scale
  sign = 1.0
  mant = 0.0
  frac = 0
  expo = 0
  esign = 1
  inExp = False
  inFrac = False
  n = Len(s)
  i = 1
  c = Mid(s, i, 1)
  If c = "-" Then
    sign = -1.0
    i = i + 1
  ElseIf c = "+" Then
    i = i + 1
  End If
  Do While i <= n
    c = Mid(s, i, 1)
    If inExp Then
      If c = "-" Then
        esign = -1
      ElseIf c <> "+" Then
        expo = expo * 10 + (Asc(c) - 48)
      End If
    ElseIf c = "e" Or c = "E" Then
      inExp = True
    ElseIf c = "." Then
      inFrac = True
    Else
      mant = mant * 10.0 + (Asc(c) - 48)
      If inFrac Then frac = frac + 1
    End If
    i = i + 1
  Loop
  scale = esign * expo - frac
  If scale > 0 Then
    For k = 1 To scale
      mant = mant * 10.0
    Next
  Else
    For k = 1 To -scale
      mant = mant / 10.0
    Next
  End If
  PeriodValue = sign * mant
End Function

' A positive, finite number as text: "" when valid, else the named reason.
Function PositiveError(ByVal text)
  Dim v
  PositiveError = "missing or unreadable"
  If Trim(text) = "" Then Exit Function
  If Not ParsePeriod(text, v) Then
    PositiveError = "not numeric"
    Exit Function
  End If
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
  If Not ParsePeriod(text, v) Then
    NonnegError = "not numeric"
    Exit Function
  End If
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
  If Not ParsePeriod(text, v) Then
    CountError = "not numeric"
    Exit Function
  End If
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

' A FOUND body count as text: "" when an integer >= 0 (zero is meaningful -
' an empty import is named, not refused as invalid), else the reason.
Function FoundCountError(ByVal text)
  Dim v
  FoundCountError = "missing or invalid"
  If Trim(text) = "" Then Exit Function
  If Not ParsePeriod(text, v) Then
    FoundCountError = "not numeric"
    Exit Function
  End If
  If v <> v Or Abs(v) > 1E+100 Then
    FoundCountError = "not finite"
    Exit Function
  End If
  If v <> Fix(v) Then
    FoundCountError = "not an integer"
    Exit Function
  End If
  If v < 0 Then
    FoundCountError = "negative"
    Exit Function
  End If
  FoundCountError = ""
End Function

' Whether a title was already open before this invocation imported
' anything (the reference-ownership rule: a pre-existing document is the
' user's and is never closed). The snapshot is the vbCrLf-joined titles
' captured before the import.
Function IsPreExistingTitle(ByVal snapshotText, ByVal title)
  Dim parts, i
  IsPreExistingTitle = False
  If Trim(title) = "" Then Exit Function
  parts = Split(snapshotText, vbCrLf)
  For i = 0 To UBound(parts)
    If parts(i) = title Then
      IsPreExistingTitle = True
      Exit Function
    End If
  Next
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
    out = out & "volume:rebuilt=" & rebuiltText & vbCrLf
  End If
  If errF = "" Then
    out = out & "volume:step_reference=" & refText & vbCrLf
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
  If Not ParsePeriod(rebuiltText, rebuilt) Then
    out = out & "detail:mass=rebuilt volume not numeric: " & rebuiltText & vbCrLf
    out = out & "check:mass=fail" & vbCrLf
    out = out & "check:mass_vs_reference=fail" & vbCrLf
    MassLines = out
    Exit Function
  End If
  If Not ParsePeriod(refText, refVol) Then
    out = out & "detail:mass=reference volume not numeric: " & refText & vbCrLf
    out = out & "check:mass=fail" & vbCrLf
    out = out & "check:mass_vs_reference=fail" & vbCrLf
    MassLines = out
    Exit Function
  End If
  If Not ParsePeriod(relText, rel) Then
    out = out & "detail:mass=volume_tol not numeric: " & relText & vbCrLf
    out = out & "check:mass=fail" & vbCrLf
    out = out & "check:mass_vs_reference=fail" & vbCrLf
    MassLines = out
    Exit Function
  End If
  If Not ParsePeriod(absText2, absTol) Then
    out = out & "detail:mass=volume_tol_abs not numeric: " & absText2 & vbCrLf
    out = out & "check:mass=fail" & vbCrLf
    out = out & "check:mass_vs_reference=fail" & vbCrLf
    MassLines = out
    Exit Function
  End If
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
Function MassSkipLines(ByVal rebuiltText, ByVal noSolid)
  Dim out, errR
  out = ""
  If noSolid Then
    ' The item declares it builds no solid (solid=none): verify the
    ' absence two-way. A clean zero volume passes; any solid is an
    ' unexpected-body failure; an unreadable read cannot prove absence
    ' and fails named.
    errR = PositiveError(rebuiltText)
    If errR = "nonpositive" Then
      out = out & "detail:solid=no solid as declared (solid=none)" & vbCrLf
      out = out & "check:solid=pass" & vbCrLf
      out = out & "detail:mass=volume comparison skipped by expect (volume_compare=skip; solid=none)" & vbCrLf
      out = out & "check:mass=skipped" & vbCrLf
      out = out & "check:mass_vs_reference=skipped" & vbCrLf
    ElseIf errR = "" Then
      out = out & "volume:rebuilt=" & rebuiltText & vbCrLf
      out = out & "detail:solid=unexpected solid volume " & rebuiltText & " on a solid=none item" & vbCrLf
      out = out & "check:solid=fail" & vbCrLf
      out = out & "check:mass=skipped" & vbCrLf
      out = out & "check:mass_vs_reference=skipped" & vbCrLf
    Else
      out = out & "detail:solid=absence unverifiable (" & errR & ": " & rebuiltText & ")" & vbCrLf
      out = out & "check:solid=fail" & vbCrLf
      out = out & "detail:mass=rebuilt volume " & errR & ": " & rebuiltText & vbCrLf
      out = out & "check:mass=fail" & vbCrLf
      out = out & "check:mass_vs_reference=skipped" & vbCrLf
    End If
    MassSkipLines = out
    Exit Function
  End If
  errR = PositiveError(rebuiltText)
  If errR = "" Then
    out = out & "volume:rebuilt=" & rebuiltText & vbCrLf
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
  errS = FoundCountError(solidsText)
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
  expected = 0
  solids = 0
  sheets = 0
  If Not ParsePeriod(expectedText, expected) Then expected = -1
  If Not ParsePeriod(solidsText, solids) Then solids = -1
  If ParsePeriod(sheetsText, sheets) Then
    ' parsed
  Else
    sheets = 0
  End If
  ' A zero solid-body count is named, not refused as an invalid count: the
  ' surface/empty-import branches below must be reachable.
  If solids = 0 Then
    If sheets > 0 Then
      out = out & "detail:step_reference=a surface import: no solid body where " & Trim(expectedText) & " were expected" & vbCrLf
    Else
      out = out & "detail:step_reference=no solid body in the import (empty body set) where " & Trim(expectedText) & " were expected" & vbCrLf
    End If
    out = out & "check:step_reference=fail" & vbCrLf
    RefImportLines = out
    Exit Function
  End If
  If solids < expected Then
    out = out & "detail:step_reference=incomplete import: found " & Trim(solidsText) & " of " & Trim(expectedText) & " expected solid bodies" & vbCrLf
    out = out & "check:step_reference=fail" & vbCrLf
    RefImportLines = out
    Exit Function
  End If
  If solids > expected Then
    out = out & "detail:step_reference=the import carries " & Trim(solidsText) & " solid bodies where " & Trim(expectedText) & " were expected (an assembly or multi-body import)" & vbCrLf
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
  expected = 0
  rebuilt = 0
  If Not ParsePeriod(expectedText, expected) Then expected = -1
  If Not ParsePeriod(rebuiltText, rebuilt) Then rebuilt = -1
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
    out = out & "detail:placement=centroid distance " & Trim(distText) & " m, tolerance " & Trim(tolText) & " m" & vbCrLf
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
  dist = 0
  tol = 0
  If Not ParsePeriod(distText, dist) Then dist = -1
  If Not ParsePeriod(tolText, tol) Then tol = -1
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

' Typed numeric acquisition for one mass-property array component. The
' numeric variants convert through CDbl (no locale surface - the value is
' already binary); string components MUST parse as strict period-decimal
' (ParsePeriod - IsNumeric/CDbl are locale-sensitive and rejected the
' checker's own period-decimal values on comma-decimal Windows). Anything
' else is refused with a named reason and converted to nothing.
Function ComponentNumber(ByVal comp, ByRef num, ByRef note)
  Dim vt
  ComponentNumber = False
  num = 0
  note = ""
  If IsObject(comp) Then
    note = "component is an object"
    Exit Function
  End If
  vt = VarType(comp)
  If vt = vbByte Or vt = vbInteger Or vt = vbLong Or vt = vbSingle Or vt = vbDouble Or vt = vbCurrency Or vt = vbDecimal Then
    ' the value is already binary: a plain assignment carries it with no
    ' locale surface (IsNumeric/CDbl are locale-poisoned and Val does not
    ' exist in VBScript - both are pinned out of this module)
    num = comp
  ElseIf vt = vbString Then
    If Not ParsePeriod(CStr(comp), num) Then
      note = "string component is not period-decimal"
      Exit Function
    End If
  Else
    note = "unsupported component type"
    Exit Function
  End If
  If num <> num Then
    note = "component is not finite"
    num = 0
    Exit Function
  End If
  If Abs(num) > 1E+100 Then
    note = "component is not finite"
    num = 0
    Exit Function
  End If
  ComponentNumber = True
End Function

' Centroid acquisition, fail closed: the function reports a named failure
' unless every one of the three components validated, and the coordinates
' are published only AFTER all three - a failed read can never leave
' initialized zeros looking like a real centroid. Any array base is
' accepted as long as exactly three elements are present (VBScript COM
' marshalling can shift array bases).
Function CentroidText(ByVal mpObj, ByRef cx, ByRef cy, ByRef cz, ByVal what)
  Dim arr, lo2, hi2, k, comp, num, note, errNum, errDesc
  CentroidText = what & " acquisition did not complete"
  cx = 0
  cy = 0
  cz = 0
  Err.Clear
  On Error Resume Next
  arr = mpObj.centerofmass
  errNum = Err.Number
  errDesc = Err.Description
  If errNum <> 0 Then
    CentroidText = "cannot read " & what & ": " & errDesc & " (err " & NumStr(errNum) & ")"
    On Error GoTo 0
    Exit Function
  End If
  If Not IsArray(arr) Then
    CentroidText = what & " did not come back as an array (vartype " & NumStr(VarType(arr)) & ")"
    On Error GoTo 0
    Exit Function
  End If
  Err.Clear
  lo2 = LBound(arr)
  hi2 = UBound(arr)
  If Err.Number <> 0 Then
    CentroidText = what & " has unreadable bounds"
    Err.Clear
    On Error GoTo 0
    Exit Function
  End If
  If hi2 - lo2 <> 2 Then
    CentroidText = what & " has an unexpected shape (bounds " & NumStr(lo2) & ".." & NumStr(hi2) & ")"
    On Error GoTo 0
    Exit Function
  End If
  For k = 0 To 2
    Err.Clear
    If IsObject(arr(lo2 + k)) Then
      CentroidText = what & " component " & NumStr(k) & " is an object"
      On Error GoTo 0
      Exit Function
    End If
    comp = arr(lo2 + k)
    If Err.Number <> 0 Then
      CentroidText = what & " component " & NumStr(k) & " could not be read"
      Err.Clear
      On Error GoTo 0
      Exit Function
    End If
    If Not ComponentNumber(comp, num, note) Then
      CentroidText = what & " component " & NumStr(k) & " is not numeric (vartype " & NumStr(VarType(comp)) & "): " & note
      On Error GoTo 0
      Exit Function
    End If
    If k = 0 Then
      cx = num
    ElseIf k = 1 Then
      cy = num
    Else
      cz = num
    End If
  Next
  On Error GoTo 0
  CentroidText = ""
End Function
