' check-part-logic.tests.vbs - drive tools/check-part-logic.vbs's decision
' logic with fixture inputs and assert the report blocks it produces.
' Runs on any Windows machine under cscript: no SolidWorks, no seat, no
' corpus. This is the checker's negative-test surface (fixture reports):
' nonpositive/missing volumes, tolerance edges including the
' relative-vs-absolute behavior, and assembly/surface/incomplete imports.
'
' Usage: cscript //nologo check-part-logic.tests.vbs
' Exit codes: 0 all passed, 1 a case failed.
Option Explicit
Dim fso, logicPath, logicFile, logicText, failures
Set fso = CreateObject("Scripting.FileSystemObject")
logicPath = fso.GetParentFolderName(WScript.ScriptFullName) & "\check-part-logic.vbs"
Set logicFile = fso.OpenTextFile(logicPath, 1)
logicText = logicFile.ReadAll
logicFile.Close
ExecuteGlobal logicText

failures = 0

Sub Check(ByVal name, ByVal condition, ByVal detail)
  If condition Then
    WScript.Echo "pass: " & name
  Else
    failures = failures + 1
    WScript.Echo "FAIL: " & name & " - " & detail
  End If
End Sub

Function Has(ByVal block, ByVal needle)
  Has = (InStr(1, block, needle, vbBinaryCompare) > 0)
End Function

Function VerdictOf(ByVal block, ByVal key)
  ' The last check:<key>= value in a block.
  Dim lines, i, prefix, v
  lines = Split(block, vbCrLf)
  prefix = "check:" & key & "="
  v = ""
  For i = 0 To UBound(lines)
    If Left(lines(i), Len(prefix)) = prefix Then v = Mid(lines(i), Len(prefix) + 1)
  Next
  VerdictOf = v
End Function

' ---- volumes: missing, invalid, nonpositive ----
Dim b
b = MassLines("", "0.01", "0.001", "0")
Check "missing rebuilt volume fails mass", (VerdictOf(b, "mass") = "fail" And VerdictOf(b, "mass_vs_reference") = "fail"), b
Check "missing rebuilt volume is named", Has(b, "rebuilt volume missing or unreadable"), b
b = MassLines("0.01", "", "0.001", "0")
Check "missing reference volume fails named", (VerdictOf(b, "mass") = "fail" And Has(b, "reference volume missing or unreadable")), b
b = MassLines("0", "0.01", "0.001", "0")
Check "nonpositive rebuilt volume fails named", (VerdictOf(b, "mass") = "fail" And Has(b, "rebuilt volume nonpositive")), b
b = MassLines("-0.01", "0.01", "0.001", "0")
Check "negative rebuilt volume fails named", (VerdictOf(b, "mass") = "fail" And Has(b, "rebuilt volume nonpositive")), b
b = MassLines("0.01", "0", "0.001", "0")
Check "nonpositive reference volume fails named", (VerdictOf(b, "mass") = "fail" And Has(b, "reference volume nonpositive")), b
b = MassLines("abc", "0.01", "0.001", "0")
Check "nonnumeric rebuilt volume fails named", (VerdictOf(b, "mass") = "fail" And Has(b, "rebuilt volume not numeric")), b
b = MassLines("0.01", "0.01", "", "0")
Check "missing volume_tol fails named", (VerdictOf(b, "mass") = "fail" And Has(b, "volume_tol missing or invalid")), b
b = MassLines("0.01", "0.01", "-0.001", "0")
Check "negative volume_tol fails named", (VerdictOf(b, "mass") = "fail" And Has(b, "volume_tol negative")), b
b = MassLines("0.01", "0.01", "0.001", "-1")
Check "negative volume_tol_abs fails named", (VerdictOf(b, "mass") = "fail" And Has(b, "volume_tol_abs negative")), b

' ---- the comparison and its edges (relative vs absolute) ----
b = MassLines("0.01", "0.01", "0.001", "0")
Check "equal volumes pass", (VerdictOf(b, "mass") = "pass" And VerdictOf(b, "mass_vs_reference") = "pass"), b
Check "the entry carries both tolerances", (Has(b, "rel_tol 0.001") And Has(b, "abs_tol 0")), b
Check "the entry carries both volumes", (Has(b, "volume:rebuilt=0.01") And Has(b, "volume:step_reference=0.01")), b
' Exactly at the relative limit: |0.01 - 0.01001| = 1e-5 = 0.001 * 0.01.
b = MassLines("0.01001", "0.01", "0.001", "0")
Check "a difference exactly at the relative limit passes", (VerdictOf(b, "mass") = "pass"), b
b = MassLines("0.010011", "0.01", "0.001", "0")
Check "a difference past the relative limit fails", (VerdictOf(b, "mass") = "fail"), b
' Relative-vs-absolute: at a tiny reference (1e-6 m3) the relative bound is
' 1e-9, so the absolute floor 5e-8 governs and admits 2e-8.
b = MassLines("0.00000102", "0.000001", "0.001", "0.00000005")
Check "the absolute floor governs small volumes (pass)", (VerdictOf(b, "mass") = "pass"), b
b = MassLines("0.00000108", "0.000001", "0.001", "0.00000005")
Check "the absolute floor governs small volumes (fail past it)", (VerdictOf(b, "mass") = "fail"), b
' At a large reference the relative bound governs even with a floor.
b = MassLines("1.0005", "1", "0.001", "0.00000005")
Check "the relative bound governs large volumes", (VerdictOf(b, "mass") = "pass"), b
b = MassLines("1.002", "1", "0.001", "0.00000005")
Check "the relative bound governs large volumes (fail past it)", (VerdictOf(b, "mass") = "fail"), b
' The absolute floor also passes where the relative bound would fail.
b = MassLines("1.002", "1", "0.001", "0.01")
Check "the absolute floor can widen the limit", (VerdictOf(b, "mass") = "pass"), b

' ---- the synthetic placeholder keeps its behavior ----
b = MassSkipLines("0.01", False)
Check "the skip placeholder is skipped on both entries", (VerdictOf(b, "mass") = "skipped" And VerdictOf(b, "mass_vs_reference") = "skipped"), b
b = MassSkipLines("", False)
Check "an unreadable rebuilt volume still fails the data gate", (VerdictOf(b, "mass") = "fail" And VerdictOf(b, "mass_vs_reference") = "skipped"), b

' ---- reference import validation: assembly, surface, incomplete ----
b = RefImportLines("1", "part", "1", "0")
Check "a one-part one-body import passes", (VerdictOf(b, "step_reference") = "pass"), b
b = RefImportLines("1", "assembly", "3", "0")
Check "an assembly import fails named", (VerdictOf(b, "step_reference") = "fail" And Has(b, "imported as an assembly")), b
b = RefImportLines("1", "part", "0", "3")
Check "a surface import fails named", (VerdictOf(b, "step_reference") = "fail" And Has(b, "a surface import")), b
b = RefImportLines("1", "part", "0", "0")
Check "an empty body set fails named", (VerdictOf(b, "step_reference") = "fail" And Has(b, "no solid body")), b
b = RefImportLines("3", "part", "2", "0")
Check "an incomplete import fails named", (VerdictOf(b, "step_reference") = "fail" And Has(b, "incomplete import")), b
b = RefImportLines("1", "part", "3", "0")
Check "an extra-body import fails named", (VerdictOf(b, "step_reference") = "fail" And Has(b, "solid bodies where 1 were expected")), b
b = RefImportLines("abc", "part", "1", "0")
Check "an invalid expected count fails named", (VerdictOf(b, "step_reference") = "fail" And Has(b, "expected solid body count is")), b
b = RefImportLines("1", "part", "missing or unreadable", "0")
Check "an unreadable body count fails named", (VerdictOf(b, "step_reference") = "fail" And Has(b, "solid body count is")), b

' ---- body count and placement (the guards alongside volume) ----
b = BodiesLines("1", "1")
Check "matching body counts pass", (VerdictOf(b, "bodies") = "pass"), b
b = BodiesLines("1", "2")
Check "a body-count mismatch fails named", (VerdictOf(b, "bodies") = "fail" And Has(b, "carries 2 solid bodies where 1 were expected")), b
b = BodiesLines("1", "")
Check "an unreadable body count fails named", (VerdictOf(b, "bodies") = "fail" And Has(b, "rebuilt body count is")), b
b = PlacementLines("0.00001", "0.0001")
Check "a centroid inside the tolerance passes", (VerdictOf(b, "placement") = "pass"), b
b = PlacementLines("0.001", "0.0001")
Check "a centroid past the tolerance fails named", (VerdictOf(b, "placement") = "fail" And Has(b, "past the")), b
b = PlacementLines("0.001", "")
Check "a missing placement_tol fails named", (VerdictOf(b, "placement") = "fail" And Has(b, "placement_tol missing or invalid")), b
b = PlacementLines("missing or unreadable", "0.0001")
Check "an unreadable centroid distance fails named", (VerdictOf(b, "placement") = "fail" And Has(b, "centroid distance is")), b

' ---- validators at the tooling boundary ----
Check "zero is not a positive volume", (PositiveError("0") = "nonpositive"), PositiveError("0")
Check "a positive volume validates", (PositiveError("1E-06") = ""), PositiveError("1E-06")
Check "zero IS a valid tolerance", (NonnegError("0") = ""), NonnegError("0")
Check "a fractional count is refused", (CountError("1.5") = "not an integer"), CountError("1.5")
Check "a zero count is refused", (CountError("0") = "nonpositive"), CountError("0")
Check "a valid count validates", (CountError("2") = ""), CountError("2")

' ---- locale-independent parsing (finding 3): the text shape is strict
' period-decimal; comma-decimal text is refused - exactly one of the two
' parses whatever the host locale says ----
Dim pv
Check "period-decimal text parses with its value", (ParsePeriod("0.01", pv) And Abs(pv - 0.01) < 0.0000000001), "pv=" & NumStr(pv)
Check "comma-decimal text is refused", (Not ParsePeriod("0,01", pv)), "pv=" & NumStr(pv)
Check "sign and exponent forms parse", (ParsePeriod("-1E-06", pv) And Abs(pv + 0.000001) < 0.000000000001), "pv=" & NumStr(pv)
Check "a bare decimal parses", (ParsePeriod(".5", pv) And Abs(pv - 0.5) < 0.0000000001), "pv=" & NumStr(pv)
Check "malformed shapes are refused", ((Not ParsePeriod("1.2.3", pv)) And (Not ParsePeriod("1e", pv)) And (Not ParsePeriod("abc", pv)) And (Not ParsePeriod("", pv)) And (Not ParsePeriod("+", pv))), "junk parsed"
Check "the validators inherit the locale-safe parse", ((PositiveError("0.01") = "") And (PositiveError("0,01") = "not numeric")), "comma text validated"

' ---- the solid=none contract: a no-solid item verifies the absence
' two-way; an unexpected solid or an unreadable read fails named ----
Dim ms
ms = MassSkipLines("0", True)
Check "a zero volume passes a solid=none item", ((VerdictOf(ms, "solid") = "pass") And (VerdictOf(ms, "mass") = "skipped") And (VerdictOf(ms, "mass_vs_reference") = "skipped")), ms
ms = MassSkipLines("0.0004", True)
Check "an unexpected solid fails a solid=none item", ((VerdictOf(ms, "solid") = "fail") And (Has(ms, "unexpected solid volume"))), ms
ms = MassSkipLines("missing or unreadable", True)
Check "an unreadable read cannot prove absence", ((VerdictOf(ms, "solid") = "fail") And (VerdictOf(ms, "mass") = "fail") And (Has(ms, "absence unverifiable"))), ms
ms = MassSkipLines("0", False)
Check "a solid-expected item keeps the fail-closed data gate", ((VerdictOf(ms, "solid") = "") And (VerdictOf(ms, "mass") = "fail")), ms

' ---- a zero found count is named downstream, never refused as invalid
' (finding 2: the surface/empty-import branches must be reachable) ----
Check "a zero found count is valid", (FoundCountError("0") = ""), FoundCountError("0")
Check "a negative found count is refused", (FoundCountError("-1") = "negative"), FoundCountError("-1")
Check "a fractional found count is refused", (FoundCountError("1.5") = "not an integer"), FoundCountError("1.5")
b = RefImportLines("1", "part", "0", "3")
Check "the surface naming is reached for zero solids", Has(b, "a surface import"), b

' ---- reference ownership (finding 4): a pre-existing title is the
' user's document and is never closed ----
Check "a pre-existing title is detected", IsPreExistingTitle("Doc1" & vbCrLf & "Doc2" & vbCrLf, "Doc2"), "not detected"
Check "a new title is not pre-existing", (Not IsPreExistingTitle("Doc1" & vbCrLf, "Doc2")), "false positive"
Check "an empty title is never pre-existing", (Not IsPreExistingTitle("Doc1" & vbCrLf, "")), "false positive"
Check "an empty snapshot owns nothing", (Not IsPreExistingTitle("", "Doc1")), "false positive"

' ===== the typed mass-adapter seam: the transaction decision and the
' strict numeric contract (the COM acquisition lives in
' tools/mass-adapter.ps1; this module never touches a COM object and
' never coerces). Fixture cases cover the review list: timeout, nonzero
' exit despite a sentinel, stale/truncated/duplicate output, late
' failure, extreme numeric tokens.
Dim volA, cxA, cyA, czA, volB, cxB, cyB, czB, why, pv2

Function AdapterTry(ByVal exitC, ByVal timedOut, ByVal fixture)
  why = AdapterResponse(exitC, timedOut, fixture, "Part A", "Part B", volA, cxA, cyA, czA, volB, cxB, cyB, czB)
  AdapterTry = (why = "")
End Function

Function GoodResponse()
  GoodResponse = "adapter:status=ok" & vbCrLf & _
    "adapter:process=1234" & vbCrLf & _
    "adapter:doc_count=2" & vbCrLf & _
    "adapter:doc[0].title=Part A" & vbCrLf & _
    "adapter:doc[0].volume=0.000006" & vbCrLf & _
    "adapter:doc[0].cx=0.015" & vbCrLf & _
    "adapter:doc[0].cy=0.01" & vbCrLf & _
    "adapter:doc[0].cz=0.005" & vbCrLf & _
    "adapter:doc[0].crosscheck=ok" & vbCrLf & _
    "adapter:doc[1].title=Part B" & vbCrLf & _
    "adapter:doc[1].volume=0.000006" & vbCrLf & _
    "adapter:doc[1].cx=-0.015" & vbCrLf & _
    "adapter:doc[1].cy=0.01" & vbCrLf & _
    "adapter:doc[1].cz=0.005" & vbCrLf & _
    "adapter:doc[1].crosscheck=ok" & vbCrLf & _
    "adapter:end" & vbCrLf
End Function

Function WithLine(ByVal fixture, ByVal needle, ByVal replacement)
  WithLine = Replace(fixture, needle, replacement, 1, 1, vbBinaryCompare)
End Function

Check "a complete adapter response publishes both triples", (AdapterTry(0, False, GoodResponse()) And volA = 0.000006 And cxA = 0.015 And czB = 0.005 And cxB = -0.015), why
Check "an unresolved timeout is named and publishes nothing", ((Not AdapterTry(0, True, GoodResponse())) And InStr(1, why, "timed out") > 0 And cxA = 0 And volA = 0), why
Check "a nonzero exit despite a sentinel publishes nothing", ((Not AdapterTry(1, False, GoodResponse())) And InStr(1, why, "exited 1") > 0 And cxA = 0), why
Check "a failure response surfaces the adapter's own reason", ((Not AdapterTry(5, False, "adapter:status=failed interop assembly missing" & vbCrLf & "adapter:end" & vbCrLf)) And InStr(1, why, "interop assembly missing") > 0), why
Check "a failed status response is refused named", ((Not AdapterTry(0, False, WithLine(GoodResponse(), "adapter:status=ok", "adapter:status=failed boom"))) And InStr(1, why, "not a success response") > 0), why
Check "a missing final sentinel is refused", ((Not AdapterTry(0, False, WithLine(GoodResponse(), "adapter:end", "adapter:tail=oops"))) And InStr(1, why, "sentinel") > 0), why
Check "trailing data after the sentinel is refused", (Not AdapterTry(0, False, GoodResponse() & "adapter:extra=1" & vbCrLf)), why
Check "truncated output is refused and publishes nothing", ((Not AdapterTry(0, False, Left(GoodResponse(), 200))) And cxA = 0), why
Check "a duplicated record is refused", (Not AdapterTry(0, False, WithLine(GoodResponse(), "adapter:doc[0].cx=0.015", "adapter:doc[0].volume=0.015"))), why
Check "a title mismatch is refused named", ((Not AdapterTry(0, False, WithLine(GoodResponse(), "adapter:doc[0].title=Part A", "adapter:doc[0].title=Part C"))) And InStr(1, why, "Part A") > 0), why
Check "a non-period component is refused", (Not AdapterTry(0, False, WithLine(GoodResponse(), "adapter:doc[0].cx=0.015", "adapter:doc[0].cx=0,015"))), why
Check "an extreme exponent token is refused before evaluation", ((Not AdapterTry(0, False, WithLine(GoodResponse(), "adapter:doc[0].cx=0.015", "adapter:doc[0].cx=1E999999999"))) And InStr(1, why, "not a bounded finite number") > 0), why
Check "an out-of-range component is refused", (Not AdapterTry(0, False, WithLine(GoodResponse(), "adapter:doc[0].cx=0.015", "adapter:doc[0].cx=1E300"))), why
Check "the parser itself refuses an extreme exponent", (Not ParsePeriod("1E999999999", pv2)), "parsed"
Check "a bounded exponent still parses", (ParsePeriod("1E300", pv2) And Abs(pv2) > 1E200), "pv=" & NumStr(pv2)
Check "an oversized response is refused", (Not AdapterTry(0, False, String(20000, "x"))), why

' the single distance owner (moved from the checker)
Check "a coincident centroid pair gives zero", (CentroidDistanceText(1, 2, 3, 1, 2, 3) = "0"), CentroidDistanceText(1, 2, 3, 1, 2, 3)
Check "a known centroid distance computes", (CentroidDistanceText(0, 0, 0, 3, 4, 0) = "5"), CentroidDistanceText(0, 0, 0, 3, 4, 0)

' ---- the distText vocabulary: the placement distance text is
' classified by NonnegError's documented mapping (empty -> missing or
' invalid, non-period -> not numeric, nonfinite -> not finite, negative
' -> negative, else the numeric comparison) ----
Check "an empty distance is missing or invalid", (NonnegError("") = "missing or invalid"), NonnegError("")
Check "an unavailable distance is not numeric", (NonnegError("missing or unreadable") = "not numeric"), NonnegError("missing or unreadable")
Check "a nonfinite distance is named", (NonnegError("1E300") = "not finite"), NonnegError("1E300")
Check "a negative distance is named", (NonnegError("-0.001") = "negative"), NonnegError("-0.001")
Check "a numeric distance validates", (NonnegError("0.00001") = ""), NonnegError("0.00001")
b = PlacementLines("missing or unreadable", "0.0001")
Check "an unavailable distance renders its class and fails", (VerdictOf(b, "placement") = "fail" And Has(b, "centroid distance (not numeric: missing or unreadable)")), b
b = PlacementLines("-0.001", "0.0001")
Check "a negative distance renders its class and fails", (VerdictOf(b, "placement") = "fail" And Has(b, "centroid distance (negative: -0.001)")), b
b = PlacementLines("1E300", "0.0001")
Check "a nonfinite distance renders its class and fails", (VerdictOf(b, "placement") = "fail" And Has(b, "centroid distance (not finite: 1E300)")), b
b = PlacementLines("", "0.0001")
Check "an empty distance renders missing or invalid and fails", (VerdictOf(b, "placement") = "fail" And Has(b, "the centroid distance is missing or invalid")), b

If failures > 0 Then
  WScript.Echo failures & " test(s) failed"
  WScript.Quit 1
End If
WScript.Echo "all check-part-logic tests passed"
WScript.Quit 0
