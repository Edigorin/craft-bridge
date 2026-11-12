' run-macro.vbs - run one reviewed macro in SolidWorks. Invoked by
' tools/seat-smoke.ps1 through cscript; it performs no provenance checks of its
' own and must only ever be called from the smoke script.
'
' VBScript on purpose: SolidWorks COM is IDispatch-native here (this is the
' pattern of SolidWorks' own Task Scheduler macro templates), and PowerShell's
' late binder has failed on this seat twice (TYPE_E_ELEMENTNOTFOUND on
' RunMacro2's ByRef argument and on property puts).
'
' Usage: cscript //nologo run-macro.vbs <full-path-to-macro.swb>
' Exit codes: 0 ran, 4 cannot attach to SolidWorks, 5 both invocations failed,
' 6 the macro invocation returned failure.

Option Explicit
Dim swApp, ok, errCode, rev, macroPath

If WScript.Arguments.Count < 1 Then
  WScript.Echo "error: usage: run-macro.vbs <macro.swb>"
  WScript.Quit 4
End If
macroPath = WScript.Arguments(0)

' ---- attach to SolidWorks ----
On Error Resume Next
Set swApp = CreateObject("SldWorks.Application")
If Err.Number <> 0 Then
  WScript.Echo "error: cannot attach to SolidWorks: " & Err.Description
  WScript.Quit 4
End If
Err.Clear
swApp.Visible = True
rev = swApp.RevisionNumber()
If Err.Number <> 0 Then
  WScript.Echo "error: cannot query SolidWorks: " & Err.Description
  WScript.Quit 4
End If
WScript.Echo "solidworks revision: " & rev

' ---- run the macro: RunMacro2 first (documented), legacy RunMacro after ----
ok = swApp.RunMacro2(macroPath, "", "main", 1, errCode)
If Err.Number <> 0 Then
  WScript.Echo "RunMacro2 failed: " & Err.Description
  Err.Clear
  ok = swApp.RunMacro(macroPath, "", "main")
  If Err.Number <> 0 Then
    WScript.Echo "error: RunMacro failed too: " & Err.Description
    WScript.Quit 5
  End If
  WScript.Echo "invocation: RunMacro"
Else
  WScript.Echo "invocation: RunMacro2 (runError=" & errCode & ")"
End If

If ok Then
  WScript.Echo "invocation returned: success"
  WScript.Quit 0
Else
  WScript.Echo "invocation returned: failure"
  WScript.Quit 6
End If
