# Research 05 — VBA dialects and Windows seat automation

**Filed:** 2026-11-17, produced by the owner's research agent from public
documentation. Answer to `research/inputs/05-vba-dialects-and-seat-automation.md`.
What this report already changed in the codebase: `tools/check-part.vbs`
used `Val()` (a VBA function VBScript does not have) in the `dim:` expect
path - replaced with the logic module's locale-safe `ParsePeriod`;
`tools/run-macro.vbs` passed a Variant to `RunMacro2`'s ByRef `Long`
argument (the recurring "RunMacro2 failed: Type mismatch") - coerced with
`CLng`; the `*>1` redirect finding explains the stray `1` file the
harness task has been writing in the repo root (the task XML template is
recorded in docs/seat-acceptance.md with the fix). The P01-P06 probes are
partly answered on the seat already (see docs/source-probe-evidence.md).

---

**Date:** 2026-11-17
**Scope:** SOLIDWORKS 2026 VBA macros, Windows Script Host VBScript, Windows PowerShell 5.1, Task Scheduler and COM automation.
**Evidence:** **Documented** = directly supported by a cited official reference. **Inferred** = implementation guidance or behavior inferred from documented principles. **Unverified** = no reliable host-specific evidence or no execution on the SOLIDWORKS seat.

> **Important:** No VBScript or SOLIDWORKS code was executed on the licensed Windows seat for this report. Examples are proposed probes unless identified as established syntax. Current Microsoft .NET Visual Basic reference pages are **not** authoritative for either classic VBA or VBScript.

## T1. Language differential tables

### 1. Numeric literals

| Syntax | SOLIDWORKS VBA 7 | VBScript under `cscript.exe` | Evidence / guard |
|---|---|---|---|
| `1`, `1.5`, `1E-3` | Supported | Supported | **Documented** basic syntax; use these common forms |
| `1#`, `1!`, `1@`, `1%`, `1&` | Legacy literal type suffixes; `#` means Double | Do **not** emit VBA numeric type suffixes | **Documented** VBA; **Unverified** exact VBScript diagnostics for every suffix |
| `Dim n#`, `Dim s$` | Type characters permitted on identifiers | Not a portable VBScript declaration | **Documented** VBA, **Inferred** VBScript portability guard |
| `#1/31/2026#` | Date literal | Date-literal form exists | **Documented** classic VB family; avoid ambiguous order and locale in serialized data |
| `&HFF`, `&O77` | Hexadecimal, octal | Hexadecimal, octal | **Documented** in VBA `Val`; **Unverified** exhaustive VBScript literal edge cases |
| `1_000`, `1.0D0`, `1.0R` | Do not use .NET numeric conventions in VBA | Do not use | **Inferred**, based on difference between .NET VB and VBA syntax |
| `1.0E+03` | Supported | Supported | **Inferred** for VBScript, seat probe recommended |

**VBA type-character reference:** https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/data-type-summary
**VBA `Val` reference:** https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/val-function

**Probe `numeric.vbs` — run with `cscript //nologo numeric.vbs`:**

```vbscript
Option Explicit
Dim a, b
a = 1.5
b = 1E-3
WScript.Echo a & " | " & b
' Run the next test in a SEPARATE file: a = 1#
' A syntax error prevents this whole file from running.
```

**Rule:** In generated `.vbs`, avoid literal and identifier type characters. Use plain decimal numbers. Do not use `#` outside a deliberate date literal.

### 2. Conversion and formatting

| Function | VBA | VBScript | Locale behavior / guard |
|---|---|---|---|
| `Val(s)` | Yes | No: do not emit | `Val` accepts `.` as decimal separator and stops at unrecognized text; **Documented** for VBA, **Unverified** host diagnostic for VBScript |
| `Str(n)` | Yes | Do not emit | VBA `Str` uses a period for decimal and reserves leading sign space; **Documented** VBA |
| `CStr(n)` | Yes | Yes | Locale-dependent numeric display. Do not use as machine serialization |
| `CDbl(s)` | Yes | Yes | Uses regional number settings. Comma-decimal input depends on locale |
| `CInt(s)`, `CLng(s)` | Yes | Yes | Locale-aware conversions; rounding and overflow require tests |
| `Format(...)` | Yes | Not part of VBScript's usual function set | Do not emit in `.vbs`; `FormatNumber` is a different function |
| `FormatNumber(...)` | Yes | Yes | Uses regional separators and optional group formatting |
| `FormatCurrency(...)` | Yes | Yes | Uses currency and regional settings |
| `StrComp(a,b,compare)` | Yes | Yes | Binary/text compare selected by mode, not a numeric parser |
| `IsNumeric(s)` | Yes | Yes | Locale and accepted forms may vary; not suitable as sole canonical grammar check |

**Sources:** https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/val-function ; https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/formatnumber-function ; https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/data-type-summary . **VBScript function exclusions:** **Inferred** from its reduced intrinsic surface and the brief's observed failures. Verify with the probe below.

**VBScript safe fixed-decimal parser (canonical wire format only):** Accept an optional minus sign, ASCII digits, and at most one `.`. Reject empty integers, exponent notation, grouping, `NaN`, whitespace, and any locale characters. Do not call `CDbl` on arbitrary inputs. The arithmetic below is locale independent for the accepted restricted syntax.

```vbscript
Option Explicit
Function ParseFixedDecimal(s)
  Dim i, c, sign, dotSeen, scale, n, digits
  If Len(s) = 0 Then Err.Raise 5, , "empty number"
  sign = 1: i = 1
  If Left(s, 1) = "-" Then sign = -1: i = 2
  If i > Len(s) Then Err.Raise 5, , "missing digits"
  dotSeen = False: scale = 1: n = 0: digits = 0
  For i = i To Len(s)
    c = Mid(s, i, 1)
    If c = "." And Not dotSeen Then
      dotSeen = True
    ElseIf c >= "0" And c <= "9" Then
      n = n * 10 + (Asc(c) - Asc("0"))
      digits = digits + 1
      If dotSeen Then scale = scale * 10
    Else
      Err.Raise 5, , "bad decimal character"
    End If
  Next
  If digits = 0 Then Err.Raise 5, , "missing digits"
  ParseFixedDecimal = sign * n / scale
End Function
WScript.Echo ParseFixedDecimal("-12.375")
```

**Parser limitation:** This compact parser permits `12.` and `.5`, and accepts long values until floating-point overflow or rounding. If the wire contract requires integer digits before the dot, enforce that condition separately. A strict production parser also needs maximum length/range and an explicit exponent policy.

### 3. Declarations and scope

| Construct | VBA 7 | VBScript | Evidence |
|---|---|---|---|
| `Dim x As Long`, `Dim x As Object` | Yes | No | **Documented** VBA; **Inferred** VBScript subset |
| `Const x As Long = 2` | Yes | No | **Documented** VBA; **Inferred** VBScript subset |
| `Dim x`, `Const x = 2` | Yes | Yes | **Documented** / common shared syntax |
| `Option Explicit` | Undeclared name is a compile-time error | Undeclared name is an error instead of an implicit Variant; exact phase requires seat probe | **Documented** VBA, **Unverified** VBScript phase |
| Without `Option Explicit` | Undeclared names can become Variant | Undeclared names can become Variant | **Documented** VBA; **Inferred** VBScript |
| Module-level variables | Yes | Script-level variables | **Inferred** host distinction |
| `Public` / `Private` members | Modules and classes | `Class` members have a more limited surface | **Inferred**; do not infer equivalent encapsulation |

**Sources:** https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/option-explicit-statement ; https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/const-statement

```vbscript
' explicit.vbs: inspect actual diagnostic and exit code
Option Explicit
Dim a
a = 1
On Error Resume Next
WScript.Echo notDeclared  ' Separate test from valid verdict code.
WScript.Echo "ERR=" & Err.Number
```

**Guard:** Require all names declared, even for one-off `.vbs` scripts. A parser or compile error can prevent execution before the `On Error` path or verdict writer runs.

### 4. Error handling

| Construct / event | VBA | VBScript | Evidence |
|---|---|---|---|
| `On Error Resume Next` | Yes | Yes | **Documented** |
| `On Error GoTo 0` | Disables handler | Disables resume-next mode | **Documented** for VBA; **Inferred** for VBScript |
| `On Error GoTo fail:` | Yes | No label handlers | **Documented** |
| `Err.Raise`, `Err.Clear`, `Err.Number` | Yes | Yes | **Documented** VBA, **Inferred** VBScript |
| Another `On Error` statement | Clears `Err` state | Test on seat before relying on state | **Documented** VBA; **Unverified** VBScript |
| A successful subsequent statement | Do **not** assume it clears `Err` | Do **not** assume it clears `Err` | **Inferred** safe programming rule |
| Procedure exit / `Resume` | Clears `Err` in documented VBA cases | Requires seat test for exact semantics | **Documented** VBA; **Unverified** VBScript |

**Sources:** https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/handling-errors-in-vbscript ; https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/clear-method-visual-basic-for-applications ; https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/err-object

**Recommended per-call check (VBScript):**

```vbscript
Option Explicit
Dim fso, fileObj, code, detail
Set fso = CreateObject("Scripting.FileSystemObject")
On Error Resume Next
Err.Clear
Set fileObj = fso.OpenTextFile("Z:\missing\file.txt", 1)
code = Err.Number: detail = Err.Description
On Error GoTo 0
If code <> 0 Then WScript.Echo "OPEN_FAILED " & code & " " & detail
```

**Guard:** Capture `Err.Number` and `Err.Description` immediately after each fallible call. Do not call another `On Error` before the snapshot. Never leave a large verdict block under `Resume Next`.

### 5. Strings

| Syntax | VBA | VBScript | Guidance |
|---|---|---|---|
| `Mid(s,2,1)`, `Left(s,2)`, `InStr(...)` | Yes | Yes | Shared safe surface |
| `Mid$`, `Left$` | Returns String (VBA) | Avoid `$` functions in `.vbs` | **Documented** VBA; **Unverified** VBScript diagnostic |
| `Replace(s,find,repl[,start[,count[,compare]]])` | Yes | Yes | Function argument forms are substantially shared; test edge cases |
| `Asc`, `Chr`, `AscW`, `ChrW` | Yes | Generally available | Unicode code units and system code page need explicit test |
| `InStr(...,vbBinaryCompare)` | Yes | Yes if constants are defined / numeric value supplied | Prefer named constants where host supports them |
| Literal quote | `""` inside `"..."` | Same | `"A""B"` produces `A"B` |
| Line continuation | space + `_` | Same | Do not insert a comment after `_` |

**VBA `$` reference:** https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/data-type-summary . The VBScript string function subset and Unicode behavior remain **Inferred** until tested.

### 6. Objects, collections, classes

| Feature | VBA | VBScript | Guard |
|---|---|---|---|
| `CreateObject`, `GetObject` | Yes | Yes | `CreateObject` may start an application; `GetObject` may attach to an active one |
| Early binding (`As SldWorks.SldWorks`) | Yes, if referenced type library exists | No typed declarations | Keep SOLIDWORKS API calls in VBA or a suitable COM client |
| Late binding (`Dim o: Set o = ...`) | Yes | Yes | Invalid members fail at runtime rather than at compilation |
| `Set x = Nothing` | Yes | Yes | Does not guarantee termination of the COM server |
| `TypeName`, `VarType`, `IsObject`, `IsArray`, `IsEmpty`, `IsNull` | Yes | Yes | Distinguish `Empty` from `Null` and `Nothing` |
| `For Each` over COM collection | Yes | Yes | Requires exposed enumerator; not all COM objects provide one |
| Class modules / UserForms / `WithEvents` | VBA IDE supports classes and forms | VBScript `Class` is narrower; no VBA UserForm / `WithEvents` equivalent | **Inferred**; use VBA for event sinks |

**Sources:** https://help.solidworks.com/2026/english/api/sldworksapiprogguide/overview/VBA_and_SolidWorks_x64.htm ; https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISldWorks~RunMacro2.html . Generic COM binding and specific VBScript feature claims are **Inferred**.

### 7. Statement syntax

| Syntax | VBA | VBScript | Guard |
|---|---|---|---|
| `:` separates statements | Yes | Yes | Avoid in generated control flow |
| ` _` continues a physical line | Yes | Yes | Keep a space before underscore |
| `' comment`, `Rem comment` | Yes | Yes | `Rem` needs statement boundary |
| Trailing `;` after a statement | Not a general statement terminator | Not a general statement terminator | Do not generate C/JS semicolons |
| `If x Then y` | Single-line form | Single-line form | Keep on one physical line |
| `If x Then` newline ... `End If` | Block form | Block form | Do not mix single-line and block branches |
| `Select Case` | No implicit fallthrough | No implicit fallthrough | Multiple matches inside one `Case` list are explicit |
| `For Each x In collection` | Yes | Yes | Beware `Nothing` and non-enumerable COM values |
| `ByVal`, `ByRef` in parameters | Yes | Yes | VBA typed signatures are not portable |

**Inferred shared syntax** based on classic VB language conventions. Validate generated control-flow patterns with separate `.vbs` syntax probes.

```vbscript
Option Explicit
Dim x
x = 1
If x = 1 Then
  WScript.Echo "true"
Else
  WScript.Echo "false"
End If
Select Case x
  Case 1: WScript.Echo "one"
  Case Else: WScript.Echo "other"
End Select
```

### 8. Static reviewer checklist / lint specification

Apply separately to every generated file:

| Rule | `.vbs` | SOLIDWORKS macro `.swp` / legacy `.swb` |
|---|---|---|
| Confirm actual file format and launcher | Use `cscript.exe` explicit path | Confirm `.swp` versus `.swb` accepted by seat and `RunMacro2` |
| `Option Explicit` at start | Required | Required in every generated module |
| Declarations | Never `As Type`; never `Dim a#` | Typed `Dim` allowed |
| Numeric literals | Reject `1#`, suffixes, separator `_` | Prefer common forms despite suffix support |
| Intrinsics | Reject `Val`, `Str`, `Format`, `$` string functions | Use supported VBA intrinsics |
| Error control | Reject `On Error GoTo label` | Require local error handler or checked `Resume Next` |
| COM | Require `Set` for objects; check `Err` | Confirm reference and types when early-bound |
| `If` and comments | Enforce whole block form for multiline code | Same |
| Output contract | One declared verdict writer and fixed exit codes | Write explicit success/failure sentinel before macro returns |
| Runtime tests | `cscript //nologo //B` + capture exit and stderr | Run through SOLIDWORKS with explicit macro module/procedure |
| Syntax check | Run in an isolated process before production | Compile inside target VBA editor before replay |

## T2. Windows Script Host execution

### Console and exit-code contract

`cscript.exe` runs a script in the console. `wscript.exe` uses the Windows GUI host. Use `cscript.exe` for machine-readable output. `WScript.Echo` is observable as console output under `cscript`; it is not a substitute for process exit codes. `WScript.Quit(code)` selects a process exit value. `//nologo` suppresses the banner. `//B` requests batch mode and suppresses interactive prompts. **Documented** for host options: https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/cscript . **Inferred** for stdout/stderr stream routing until tested.

```vbscript
' exit-probe.vbs
Option Explicit
WScript.Echo "RESULT ok"
WScript.Quit 23
```

```powershell
& "$env:WINDIR\System32\cscript.exe" //nologo //B .\exit-probe.vbs 1> .\stdout.txt 2> .\stderr.txt
"exit=$LASTEXITCODE"
Get-Content .\stdout.txt
Get-Content .\stderr.txt
```

**Exit strategy:** 0 for verified success; nonzero for preflight failure, macro failure, timeout, and missing verdict. Keep reason strings in the result file or stdout. Parse the actual numeric exit code, not just expected output lines.

### File I/O and sharing

VBScript uses `Scripting.FileSystemObject.OpenTextFile(path, iomode, create, format)` and `CreateTextFile(path, overwrite, unicode)`. `ForReading=1`, `ForWriting=2`, and `ForAppending=8` are conventional constants. VBA also has native `Open ... For Output As #n`, `FreeFile`, `Print #`, and `Close #`; do **not** move those file-number statements into VBScript. **Inferred** from distinct language host facilities.

```vbscript
Option Explicit
Dim fso, h
Set fso = CreateObject("Scripting.FileSystemObject")
Set h = fso.OpenTextFile("C:\Temp\verdict.txt", 2, True)
h.WriteLine "status=ok"
h.Close
Set h = Nothing
```

**Recommendation:** Open one file handle, write its complete result, and close it. For a result visible to another process, write to a temporary path and rename within the same directory after closing. An atomic rename is a useful design goal, not a guarantee across every filesystem or sharing state. Do not claim that opening and closing a handle prevents all sharing violations.

### Locale test matrix

Test `en-US`, `de-DE`, and `fr-FR` user cultures with these inputs: `"1.5"`, `"1,5"`, `"1,234.5"`, `"1 234,5"`, `"-0.25"`, `"1E3"`, `" 1.5 "`, empty, `"0x10"`. For each input, record `IsNumeric`, `CDbl`, `CStr` after parsing, and `FormatNumber`. The fixed-decimal parser above deliberately accepts a narrower language, so do not compare it with locale-aware conversions as though they share an input grammar. **Documented** locale dependence of `FormatNumber`: https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/formatnumber-function ; **Documented** locale distinction for `Val` / `CDbl`: https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/val-function .

## T3. VBA inside SOLIDWORKS

### Runtime and supported project form

SOLIDWORKS introduced VBA 7 in SOLIDWORKS 2013 SP0. In 64-bit SOLIDWORKS, imported Windows API `Declare` statements need `PtrSafe`. Pointer-sized values need `LongPtr`. **Documented:** https://help.solidworks.com/2026/english/api/sldworksapiprogguide/overview/VBA_and_SolidWorks_x64.htm . VBA macros can use modules, class modules, forms, and References for type libraries. The exact installed controls and type-library names must be inspected on the seat. **Unverified** for seat configuration.

```vb
Option Explicit
Sub main()
    Dim swApp As SldWorks.SldWorks
    Set swApp = Application.SldWorks
    Debug.Print swApp.RevisionNumber()
End Sub
```

`RunMacro(FilePathName, ModuleName, ProcedureName)` is obsolete. `RunMacro2(FilePathName, ModuleName, ProcedureName, Options, Error)` returns Boolean, with a separate output error enum. It was added in SOLIDWORKS 2009. The documented VBA example invokes `main` in a named module. A `Sub main()` without required parameters is the low-risk convention for generated macros. Arbitrary parameter passing and return values are not part of the shown `RunMacro2` signature. **Documented:** https://help.solidworks.com/2025/English/api/sldworksapi/SolidWorks.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISldWorks~RunMacro.html ; https://help.solidworks.com/2024/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.ISldWorks~RunMacro2.html .

```vb
' VBA caller with SOLIDWORKS type library reference
Option Explicit
Sub main()
    Dim app As SldWorks.SldWorks
    Dim ok As Boolean, macroError As Long
    Set app = Application.SldWorks
    ok = app.RunMacro2("C:\Temp\probe.swp", "Module1", "main", 0, macroError)
    Debug.Print "ok=" & CStr(ok) & " error=" & CStr(macroError)
End Sub
```

**Macro failure boundary:** The API documents the Boolean and `swRunMacroError_e`, but does not establish that a VBA editor compile-error dialog will always produce a timely return. The exact behavior with a compile error, a modal dialog, a runtime error and a hung macro remains **Unverified**. A watchdog must mark a run failed if either return status or an explicit verdict file is missing by its deadline.

### Modal dialogs and headless operation

**Inferred:** SOLIDWORKS is a desktop CAD application. User dialogs, missing references, unsaved-document prompts, input-dimension boxes and VBA compile-error dialogs can block unattended calls. A running interactive session is necessary for GUI access but does not make the application headless. The precise behavior of the dimension-value dialog and save prompts under `RunMacro2` is **Unverified**.

**Operational guard:** Use a dedicated interactive seat with one run at a time. Close unexpected documents and dialogs before each replay. Avoid UI-dependent dimension entry. Use model/API operations instead. Keep an external timeout and process identity log. Do not terminate all SOLIDWORKS instances without identifying ownership.

## T4. Task Scheduler, PowerShell and COM

### Task XML and user sessions

Task Scheduler's official schema uses namespace `http://schemas.microsoft.com/windows/2004/02/mit/task`. Treat a different namespace, such as a fabricated `2006` namespace, as suspect. The outer Task `version` attribute is distinct from the XML namespace. `InteractiveToken` means the user must already be logged on and the job runs in that interactive session. `S4U` does not store a password but has no normal access to network resources or encrypted files. **Documented:** https://learn.microsoft.com/en-us/windows/desktop/taskschd/task-scheduler-schema ; https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-logontype-principaltype-element .

`<Exec>` has `Command`, `Arguments`, and `WorkingDirectory`. The working directory must be explicit for relative paths. Environment variables in Task Scheduler execution properties can be cached by the task engine. **Documented:** https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-exec-actiongroup-element ; https://learn.microsoft.com/en-us/windows/win32/taskschd/execaction .

```xml
<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers />
  <Principals>
    <Principal id="Author">
      <UserId>DOMAIN\SeatUser</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings><Enabled>true</Enabled></Settings>
  <Actions Context="Author">
    <Exec>
      <Command>C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe</Command>
      <Arguments>-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "C:\Seat\run-seat.ps1"</Arguments>
      <WorkingDirectory>C:\Seat</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
```

**XML sample status: Unverified as a complete importable scheduled-task definition.** Replace the user and paths, generate a complete XML through Task Scheduler on the target OS, and compare or export it. An interactive desktop macro can still show UI despite `-NonInteractive`: the flag concerns PowerShell prompting, not SOLIDWORKS UI.

```powershell
schtasks.exe /Create /TN "CraftBridgeSeat" /XML "C:\Seat\task.xml" /F
schtasks.exe /Query /TN "CraftBridgeSeat" /V /FO LIST
schtasks.exe /Run /TN "CraftBridgeSeat"
```

**Last Result:** Treat it as a task/process result to interpret alongside history, exit code and your own verdict. Do not infer macro success from `0` alone. The task engine can start a wrapper successfully even when an internal action fails. **Inferred** control-design guidance.

### PowerShell quoting and logging

Prefer `powershell.exe ... -File "C:\Seat\run-seat.ps1"` over a nested `-Command` string in task XML. With `-Command`, quoting crosses XML, scheduler command parsing, PowerShell parsing and potentially `cmd.exe`. With `-File`, the script controls its own quoting. **Inferred** reliability recommendation.

PowerShell uses stream 1 for success, 2 for errors and `*` for all redirectable streams. Therefore `*>1` redirects all streams to a file named `1`. It does **not** merge streams into stdout. Use `*> "C:\Seat\logs\run.log"` or `2>&1 | ...` deliberately. **Documented:** https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_redirection .

```powershell
# C:\Seat\run-seat.ps1
$ErrorActionPreference = 'Stop'
$logDir = 'C:\Seat\logs'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$log = Join-Path $logDir ('seat-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.txt')
Start-Transcript -Path $log | Out-Null
try {
    & "$env:WINDIR\System32\cscript.exe" //nologo //B 'C:\Seat\probe.vbs'
    if ($LASTEXITCODE -ne 0) { throw "cscript exit=$LASTEXITCODE" }
    exit 0
} catch {
    Write-Error $_
    exit 1
} finally {
    Stop-Transcript | Out-Null
}
```

**Caution:** `exit` inside `try/catch` can interact with `finally` and transcript handling. Verify the final process exit code under both success and failure. Use absolute paths, and capture stderr separately if a downstream parser needs two independent streams.

### COM instance selection and restart

`CreateObject("SldWorks.Application")` and `GetObject(, "SldWorks.Application")` are different operations in classic COM clients. The practical assignment of a call to one of several simultaneously running SOLIDWORKS processes is **Unverified** here. Do not assume that a generic ProgID binds the intended PID. A Running Object Table (ROT) may expose running COM objects, but the moniker and registration behavior must be measured on this SOLIDWORKS build.

**Suggested seat protocol:** Record all SOLIDWORKS PIDs before launch. Launch one owned process. Record its PID/start time. Obtain the automation object. Compare its active document and process behavior with the intended instance. Reject ambiguous instances. On recovery, attempt document close and normal application exit first. Terminate only the identified owned process after the timeout. **Inferred**, not a documented SOLIDWORKS multi-instance guarantee.

### Child process output and deadlocks

With `.NET ProcessStartInfo`, redirecting stdout and stderr to pipes can deadlock when the parent waits while the child fills a pipe. Consume both streams concurrently, or redirect to files, then `WaitForExit` and inspect `ExitCode`. **Documented:** https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.processstartinfo.redirectstandarderror?view=netframework-4.8 ; https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.processstartinfo.redirectstandardoutput?view=netframework-4.8.1 .

```powershell
# PowerShell 5.1: simple low-volume child output check
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "$env:WINDIR\System32\cscript.exe"
$psi.Arguments = '//nologo //B "C:\Seat\probe.vbs"'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$p = [System.Diagnostics.Process]::Start($psi)
$outTask = $p.StandardOutput.ReadToEndAsync()
$errTask = $p.StandardError.ReadToEndAsync()
$p.WaitForExit()
$out = $outTask.Result: $err = $errTask.Result
"exit=$($p.ExitCode) stdout=$out stderr=$err"
```

**Probe:** Run this pattern with output larger than both pipe buffers, a child timeout, and non-ASCII characters. For production, add a timeout, a process-tree cleanup policy, and explicit output encoding. Whether PowerShell 5.1 decodes OEM or ANSI text correctly for a particular child depends on the stream's actual encoding. **Unverified** on this seat.

## T5. Unknowns and runnable seat probes

| ID | Question | Evidence | Test / acceptance condition |
|---|---|---|---|
| P01 | Does `Val("1.5")` parse or fail under `cscript`? | **Unverified** exact error | `WScript.Echo Val("1.5")`; record error number, phase and exit |
| P02 | How does `1#` fail under VBScript? | **Unverified** exact parser result | `WScript.Echo 1#`; run in separate `.vbs`; record syntax/line/code |
| P03 | Does `Option Explicit` fail before `Resume Next` can run? | **Unverified** exact phase | Run `explicit.vbs` above; check whether `ERR=` prints |
| P04 | What clears VBScript `Err`? | **Unverified** | Raise an error, snapshot, run success, snapshot, execute each `On Error` form, snapshot |
| P05 | Which VBScript intrinsics differ? | **Unverified** | Test `Str`, `Format`, `Mid$`, `Left$`, `AscW`, `ChrW` separately |
| P06 | Locale-specific parser behavior | **Unverified** | Run locale matrix under `de-DE` and `fr-FR`; record output strings |
| P07 | Which macro extension is accepted (`.swb` vs `.swp`)? | **Unverified** | Run same no-op module through `RunMacro2` by file type |
| P08 | Compile-error dialog return behavior | **Unverified** | Run a deliberately uncompilable macro under external watchdog; record return and GUI state |
| P09 | Modal dimension dialog behavior | **Unverified** | Leave an input dialog open; time one documented safe API call |
| P10 | Save prompt behavior | **Unverified** | Modify scratch document, trigger close through API, record prompt and timeout |
| P11 | Multiple SOLIDWORKS instances | **Unverified** | Start two scratch instances; compare `CreateObject`, `GetObject`, active documents and PIDs |
| P12 | Task XML import and session behavior | **Unverified** | Export task from seat, register from XML, test logged-on and logged-off states |
| P13 | PowerShell wrapper exit and logging | **Unverified** | Force a nonzero `WScript.Quit` and a syntax error; verify exit, transcript and stderr |
| P14 | Encoding and pipe pressure | **Unverified** | Emit 1 MB stdout and stderr plus Unicode, confirm no deadlock or data loss |

**Minimal independent VBScript probe runner (PowerShell):**

```powershell
$tests = @(
  @{ Name='val'; Code='WScript.Echo Val("1.5")' },
  @{ Name='suffix'; Code='WScript.Echo 1#' },
  @{ Name='dollar'; Code='WScript.Echo Left$("abc", 1)' },
  @{ Name='format'; Code='WScript.Echo Format(1.5, "0.0")' }
)
foreach ($t in $tests) {
  $path = Join-Path $env:TEMP ("probe-" + $t.Name + '.vbs')
  Set-Content -LiteralPath $path -Value $t.Code -Encoding ASCII
  $lines = & "$env:WINDIR\System32\cscript.exe" //nologo //B $path 2>&1
  '{0}: exit={1}; output={2}' -f $t.Name, $LASTEXITCODE, ($lines -join ' | ')
}
```

**Run order:** P01–P06 before generating more `.vbs`. P07–P10 before unattended macro replay. P11–P14 before adding concurrent or scheduled jobs.

## Final implementation decisions

1. Define two different source emitters: `VBScript` and `SOLIDWORKS_VBA7`. Never use a combined Visual Basic emitter without a dialect mode.
2. Create a static validator for banned constructs. It must reject typed VBScript declarations, numeric type suffixes, `Val`, VBA-only error labels, and malformed multiline `If` blocks.
3. Keep machine-to-machine numbers in one invariant decimal grammar. Validate before converting. Never use localized display formatting as an interchange format.
4. Keep one verified verdict file per operation. Require a process exit result, a macro result and a verdict marker where applicable.
5. Run the licensed SOLIDWORKS seat in a known interactive session. Use one managed SOLIDWORKS instance per replay until multi-instance binding passes a real probe.
6. Treat timeouts, ambiguous COM binding and modal dialogs as failures. Never convert a missing verdict into success.

**Limitations:** The report establishes a documented baseline, not certified seat behavior. In particular, no official source reviewed here settles the exact VBScript diagnostics, SOLIDWORKS modal-error callback behavior, or ROT instance selection under multiple SOLIDWORKS processes. The probes are required before relying on those behaviors.

**Trademark note:** SOLIDWORKS is a Dassault Systèmes trademark. Mention is descriptive only. This report is not affiliated with or endorsed by Dassault Systèmes.
