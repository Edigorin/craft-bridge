' probe-typed-arrays.vbs - Windows Script Host array-marshaling probe.
' Filed verbatim 2026-11-19 from the owner's research agent (research 11
' companion deliverable; provenance: research/outputs/
' 11-vbscript-com-array-marshaling.md T3.1). Standalone diagnostic: no
' SOLIDWORKS, no file writes; run under cscript on any Windows host.
Option Explicit
' Research 11: Windows Script Host array-marshaling probe.
' Run: cscript //nologo probe-typed-arrays.vbs
' This script does not use SOLIDWORKS or write files.

Dim normalArray, keysArray, bytesArray, d, stm

Sub Say(s)
  WScript.Echo s
End Sub

Sub ProbeArray(tag, a)
  Dim vt, typ, lo, hi, e, code, desc, i, x, n, probeType
  Say "=== " & tag & " ==="
  On Error Resume Next

  Err.Clear
  vt = VarType(a)
  code = Err.Number: desc = Err.Description
  If code = 0 Then
    Say "CONTAINER_VARTYPE=" & CStr(vt) & " BASE_TYPE=" & CStr(vt And 4095)
  Else
    Say "CONTAINER_VARTYPE_FAILED=" & code & " " & desc
  End If

  Err.Clear
  typ = TypeName(a)
  code = Err.Number: desc = Err.Description
  If code = 0 Then
    Say "CONTAINER_TYPENAME=" & typ
  Else
    Say "CONTAINER_TYPENAME_FAILED=" & code & " " & desc
  End If

  Err.Clear
  If Not IsArray(a) Then
    Say "NOT_ARRAY"
    On Error GoTo 0
    Exit Sub
  End If

  Err.Clear
  lo = LBound(a): hi = UBound(a)
  code = Err.Number: desc = Err.Description
  If code <> 0 Then
    Say "BOUNDS_FAILED=" & code & " " & desc
    On Error GoTo 0
    Exit Sub
  End If
  Say "BOUNDS=" & lo & "," & hi & " COUNT=" & (hi - lo + 1)

  For i = lo To hi
    Err.Clear
    x = a(i)
    code = Err.Number: desc = Err.Description
    If code <> 0 Then
      Say "INDEX_" & i & "_FAILED=" & code & " " & desc
    Else
      Err.Clear
      probeType = VarType(x)
      code = Err.Number: desc = Err.Description
      If code <> 0 Then
        Say "INDEX_" & i & "_TYPE_FAILED=" & code & " " & desc
      ElseIf IsObject(x) Then
        Say "INDEX_" & i & "_OBJECT_TYPE=" & TypeName(x)
      Else
        Say "INDEX_" & i & "_TYPE=" & probeType & " VALUE=" & CStr(x)
      End If
    End If
  Next

  Err.Clear
  n = 0
  For Each e In a
    n = n + 1
  Next
  code = Err.Number: desc = Err.Description
  If code = 0 Then
    Say "FOR_EACH_COUNT=" & n
  Else
    Say "FOR_EACH_FAILED=" & code & " " & desc & " COUNT_BEFORE_ERROR=" & n
  End If

  On Error GoTo 0
End Sub

normalArray = Array(1.25, 2.5, 3.75)
Call ProbeArray("VBScript Array(Double,Double,Double)", normalArray)

On Error Resume Next
Err.Clear
Set d = CreateObject("Scripting.Dictionary")
If Err.Number <> 0 Then
  Say "DICT_CREATE_FAILED=" & Err.Number & " " & Err.Description
Else
  d.Add "a", 1
  d.Add "b", 2
  d.Add "c", 3
  keysArray = d.Keys
  If Err.Number <> 0 Then
    Say "DICT_KEYS_FAILED=" & Err.Number & " " & Err.Description
  Else
    Call ProbeArray("Scripting.Dictionary.Keys", keysArray)
  End If
End If
On Error GoTo 0

On Error Resume Next
Err.Clear
Set stm = CreateObject("ADODB.Stream")
If Err.Number <> 0 Then
  Say "ADO_CREATE_FAILED=" & Err.Number & " " & Err.Description
Else
  Err.Clear
  stm.Type = 2
  stm.Charset = "us-ascii"
  stm.Open
  stm.WriteText "XYZ"
  stm.Position = 0
  stm.Type = 1
  bytesArray = stm.Read(3)
  If Err.Number <> 0 Then
    Say "ADO_READ_FAILED=" & Err.Number & " " & Err.Description
  Else
    Call ProbeArray("ADODB.Stream.Read(3) binary", bytesArray)
  End If
  stm.Close
End If
On Error GoTo 0

Say "PROBE_COMPLETE; retain all output, including failures"
