' seat-probe.vbs - the smallest possible macro run: writes one line to the
' report path. Separates "the macro invocation is broken" from "our generated
' macros are broken". Invoked by tools/seat-harness.ps1 first.
Option Explicit
Dim reportPath, f
Dim sh
Set sh = CreateObject("WScript.Shell")
reportPath = sh.Environment("PROCESS")("CRAFT_BRIDGE_REPORT")
If reportPath = "" Then
  WScript.Echo "probe: CRAFT_BRIDGE_REPORT is not set"
  WScript.Quit 2
End If
On Error Resume Next
Dim fso
Set fso = CreateObject("Scripting.FileSystemObject")
Set f = fso.CreateTextFile(reportPath, True)
If Err.Number <> 0 Then
  WScript.Echo "probe: cannot write the report: " & Err.Description
  WScript.Quit 3
End If
f.WriteLine "{""type"":""header"",""macro"":""seat-probe""}"
f.WriteLine "{""type"":""step"",""index"":0,""command"":""probe"",""status"":""created"",""message"":""plain VBScript wrote this""}"
f.WriteLine "{""type"":""summary"",""status"":""ok""}"
f.Close
WScript.Echo "probe: wrote " & reportPath
WScript.Quit 0
