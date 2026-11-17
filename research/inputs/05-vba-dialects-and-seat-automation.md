# Research brief 05 - VBA dialects and Windows seat automation

A different kind of research: not CAD API surface, but the LANGUAGE AND
HOST surface our tooling and macros actually run on. Three real bugs this
week came from dialect confusion: `Val` (VBA) called from VBScript where
it does not exist; `1#` type characters (VBA) parsed as a date literal in
VBScript; and `Option Explicit` turning an undeclared variable into a
runtime error that `On Error Resume Next` swallowed, silently emptying a
whole verdict block. A differential reference would have prevented all
three.

Scope: VBA as hosted inside SOLIDWORKS macros (.swb run through the
SolidWorks process) versus VBScript as run by cscript/wscript, plus the
Windows automation surface between our orchestrating PowerShell and both.

## Ground rules

- Sources: official Microsoft VBScript/VBA language references, the
  SOLIDWORKS API "Getting started / VBA" guides, Microsoft Learn pages
  for Task Scheduler and COM, reputable long-lived references (devguru,
  vb2the max, stackoverflow answers with citations) - mark lower-trust
  sources as **Inferred**. Cite URLs per fact.
- Evidence labels **Documented** / **Inferred** / **Unverified**. For
  language-surface questions, prefer worked mini-examples in the report
  (5-10 lines each) over prose.
- Deliverable: one markdown report, differential tables first, then the
  automation section, then an unknowns/probe table.

## T1. Language differential tables (the core deliverable)

Side-by-side, VBA (SolidWorks host) vs VBScript (cscript):

1. **Numeric literals**: type characters (`# ! @ $ % &`) - legal where?
   Date literals (`#...#`) and their collision risk; hex/octal (`&H`,
   `&O`); exponent forms; underscores in literals.
2. **Conversion/format functions**: `Val`, `Str`, `CStr`, `CDbl`, `CInt`,
   `CLng`, `Format`, `FormatNumber`, `StrComp`, locale sensitivity of
   each (which read the decimal separator, which do not).
3. **Declarations**: `Dim x As T` (typed) - legal where? `Const x As T`?
   `Option Explicit` behavior differences (undeclared variable: compile
   error vs runtime error vs silent variant?); scope rules.
4. **Error handling**: `On Error Resume Next` / `On Error GoTo 0` /
   `On Error GoTo label` - semantics per dialect; `Err` object lifetime
   (does executing another On Error reset it? does a successful call
   clear it?); `Err.Raise` / `Err.Clear` availability.
5. **Strings**: `Mid`/`Mid$`, `Left$`, `InStr` compare modes, `Replace`
   argument forms, `Asc`/`Chr` vs `AscW`/`ChrW`, quoting/escaping.
6. **Collections/objects**: `CreateObject`/`GetObject`, late vs early
   binding risks (compile-time member resolution), `Set`/`Nothing`/
   `TypeName`/`VarType`/`IsObject`/`IsArray`/`IsEmpty`/`IsNull`, the
   `WithEvents`/class module surface (VBScript subset).
7. **Statement syntax**: statement separators (`:`), line continuation
   (`_`), comments (`'`, `Rem`), trailing semicolons (illegal where?),
   `If ... Then` single-line vs block mixing rules (a real pairing bug we
   hit), `Select Case` fallthrough, `For Each` over COM collections.
8. **A ready-to-run checklist** a reviewer can apply to a .vbs and a .swb
   to catch cross-dialect usage (like a lint spec).

## T2. cscript/wscript execution

- Exit codes (WScript.Quit), stdout/stderr capture under redirection and
  pipes (which stream `WScript.Echo` uses), the `//nologo //B` flags.
- File I/O idioms (OpenTextFile/CreateTextFile modes, `FreeFile`,
  overwrite semantics), and why "one line, one handle" write patterns
  avoid sharing violations.
- Host differences: cscript vs wscript, and Windows PowerShell 5.1
  hosting VBScript-ish? (no - clarify what hosts what).
- Locale surface: `IsNumeric`/`CDbl`/`CStr`/`Val`/`FormatCurrency` under
  comma-decimal locales; the safe parsing recipe (what we implemented:
  shape-validate then `Val`-less arithmetic - confirm or improve).

## T3. VBA inside SOLIDWORKS

- Which VBA version/features the SOLIDWORKS macro editor supports
  (VBA6/VBA7, 32/64-bit PtrSafe, user forms, class modules, references
  dialog - "SOLIDWORKS 20xx Type Library" early binding).
- Macro entry-point conventions (main/Main, arguments, return), and how
  RunMacro/RunMacro2 map to them (cross-ref with brief 04 T8).
- The document modal-state problem: documented behavior of automation
  calls while a dialog is open; known SW behaviors for the
  input-dimension-value dialog and save prompts; best practice for
  headless runs.
- Long-running macros and the VBE "compile error" dialog: how a compile
  error surfaces through the API (RunMacro return? exception? event?) -
  this is how our probe failures manifest.

## T4. Task Scheduler + COM automation (Windows surface)

- `schtasks /Create /XML`: XML namespace/version pitfalls (the 2004 vs
  2006 namespace trap), `InteractiveToken` semantics (why GUI apps need
  it), working directory and environment inheritance, `Last Result` exit
  codes.
- Quoting through `-Command` vs `-File` for powershell.exe under Task
  Scheduler: which quoting survives, the `*>1` all-streams-to-FILENAME
  trap, why parse errors vanish (stderr unredirected), and the robust
  pattern (a -File wrapper script with Start-Transcript).
- COM in-process identity: with multiple SOLIDWORKS instances running,
  what does `CreateObject("SldWorks.Application")` attach to (ROT
  behavior)? How to target a specific instance; safe restart sequences
  (terminate vs graceful close; leftover modal states).
- Redirected process output from a service context (our
  Invoke-SeatCommand ProcessStartInfo pattern): deadlocks, encoding
  (OEM vs Unicode), and exit-code propagation.

## T5. Unknowns and probe suggestions

Close with the standard table. Everything labeled **Unverified** gets a
one-line runnable snippet a human can paste (cscript test file or
PowerShell one-liner) to settle it on the seat.
