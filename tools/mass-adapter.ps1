# mass-adapter.ps1 - the typed mass-property adapter (research 11 ranked
# option 1: "typed .NET verifier or COM bridge"; probe evidence:
# docs/source-probe-evidence.md 2026-11-19 - CenterOfMass arrives in
# VBScript as a typed SAFEARRAY(Double) (VarType 8197) that no VBScript
# access form can read).
#
# Runs INSIDE the checker's scheduled-task session (never from SSH),
# spawned once per check by tools/check-part.vbs. It performs NO document
# mutation whatsoever: no creation, no activation, no closing, no quit -
# read-only mass acquisition on documents that already exist.
#
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File mass-adapter.ps1
#          -Request <request-file> -Out <out-file> -ExpectedPid <pid>
# Request file: one "title=<title>" line per document to read (exactly
# two for the checker), titles must be single-line. Out file: the strict
# record contract the checker parses (see below). The out file is never
# overwritten: a pre-existing file is preserved with a .prev-<stamp>
# suffix (the repo's evidence convention).
#
# Output contract (invariant-culture period-decimal, ASCII):
#   adapter:status=ok
#   adapter:process=<pid>
#   adapter:doc_count=<n>
#   adapter:doc[i].title=<title>
#   adapter:doc[i].volume=<period-decimal>
#   adapter:doc[i].cx=<period-decimal>
#   adapter:doc[i].cy=<period-decimal>
#   adapter:doc[i].cz=<period-decimal>
#   adapter:doc[i].crosscheck=ok
#   adapter:end            <- final sentinel, nothing after it
# Failure: adapter:status=failed <named reason> + adapter:end, exit 5.
#
# Acquisition discipline per document: the typed interop chain is
# QUALIFIED at runtime (the raw CenterOfMass result must be exactly a
# one-dimensional lower-bound-zero length-three System.Double array -
# inspected before any conversion, no default-property/.Value/CDbl
# coercion anywhere), and the triple is CROSS-CHECKED against
# GetMassProperties2(1, [ref]status, False)[0..2] with status 0 and a
# documented project tolerance (1E-9 m absolute - a project choice, not
# a literature-backed tolerance). Object-valued or unindexable results
# are named refusals, never conversions.
param(
    [Parameter(Mandatory = $true)][string]$Request,
    [Parameter(Mandatory = $true)][string]$Out,
    [Parameter(Mandatory = $true)][string]$ExpectedPid
)
$ErrorActionPreference = "Stop"

function Write-Result([string]$Path, [string[]]$Lines) {
    # preserve prior evidence (never overwrite), then write atomically
    if (Test-Path -LiteralPath $Path) {
        $stamp = Get-Date -Format "yyyyMMddTHHmmss"
        $prev = "$Path.prev-$stamp"
        Move-Item -LiteralPath $Path -Destination $prev -Force
    }
    $tmp = "$Path.partial"
    Set-Content -LiteralPath $tmp -Value $Lines -Encoding Ascii
    Move-Item -LiteralPath $tmp -Destination $Path -Force
}

function Fail([string]$Path, [string]$Reason) {
    Write-Result $Path @("adapter:status=failed $Reason", "adapter:end")
    Write-Host "adapter: failed $Reason"
    exit 5
}

# Typed COM access through the interop metadata (MethodInfo/PropertyInfo
# Invoke), which QIs the RCW by IID: immune to both failure classes the
# qualification produced - PowerShell's binder mistaking a 0-arg method
# for a property (DISP_E_BADINDEX at $doc.GetTitle()), and the language
# cast refusing a LoadFrom interop identity ("Cannot convert
# System.__ComObject to ISldWorks").
function T-Method($t, $obj, [string]$name, $callArgs) {
    return $t.GetMethod($name).Invoke($obj, $callArgs)
}
function T-Get($t, $obj, [string]$name) {
    return $t.GetProperty($name).GetValue($obj, @())
}
function T-Set($t, $obj, [string]$name, $v) {
    $t.GetProperty($name).SetValue($obj, $v, @())
}

function Format-Invariant([double]$V) {
    # round-trippable, always period-decimal
    return $V.ToString("G17", [System.Globalization.CultureInfo]::InvariantCulture)
}

function Get-Triple([object]$Raw, [string]$Tag) {
    # Strict typed acquisition, qualification-revised: the interop marshals
    # the documented Double[3] (research 10 [M1]) to .NET as a VARIANT
    # array of BOXED System.Double values (System.Object[]), and the wire
    # to VBScript as VT_ARRAY|VT_R8. Both shapes are accepted ONLY with
    # exact rank-one, lower-bound-zero, length-three form and strictly
    # double elements - every other element type (COM object, string,
    # null) is a named refusal. No coercion of any value ever happens.
    if ($null -eq $Raw) { throw "$Tag : the raw result is null" }
    $typeName = $Raw.GetType().FullName
    if ($Raw -isnot [double[]] -and $Raw -isnot [object[]]) {
        throw "$Tag : the raw result is not a Double() or a boxed-Double Variant array (got $typeName)"
    }
    if ($Raw.Rank -ne 1) { throw "$Tag : the array is not one-dimensional (rank $($Raw.Rank))" }
    if ($Raw.GetLowerBound(0) -ne 0) { throw "$Tag : the array is not zero-based (lower bound $($Raw.GetLowerBound(0)))" }
    if ($Raw.Length -ne 3) { throw "$Tag : the array length is not 3 (got $($Raw.Length))" }
    foreach ($v in $Raw) {
        if ($v -isnot [double]) {
            $elType = if ($null -eq $v) { "null" } else { $v.GetType().FullName }
            throw "$Tag : a component is not a Double value (got $elType)"
        }
        $d = [double]$v
        if ([double]::IsNaN($d) -or [double]::IsInfinity($d)) { throw "$Tag : a component is not finite" }
        if ([math]::Abs($d) -gt 1E100) { throw "$Tag : a component is out of range" }
    }
    return $Raw
}

try {
    $stage = "request parse"
    # ---- the request (single-line titles; nothing executable) ----
    if (-not (Test-Path -LiteralPath $Request)) { Fail $Out "the request file is missing" }
    $lines = @(Get-Content -LiteralPath $Request)
    if ($lines.Count -lt 1 -or $lines.Count -gt 4) { Fail $Out "the request carries $($lines.Count) lines (1..4 expected)" }
    $titles = @()
    foreach ($l in $lines) {
        if ($l -notmatch '^title=(.+)$') { Fail $Out "a request line is not title=<title>" }
        $t = $Matches[1]
        if ($t -match '[\r\n]') { Fail $Out "a requested title is not single-line" }
        $titles += $t
    }
    $distinct = @($titles | Select-Object -Unique)
    if ($distinct.Count -ne $titles.Count) { Fail $Out "the requested titles are not distinct" }

    # ---- application-instance handshake (the checker computed the same
    # identity; exactly one SolidWorks process must exist) ----
    $stage = "process handshake"
    $procs = @(Get-Process -Name "SLDWORKS" -ErrorAction SilentlyContinue)
    if ($procs.Count -ne 1) { Fail $Out "ambiguous SolidWorks process identity ($($procs.Count) processes)" }
    $pid1 = [string]$procs[0].Id
    if ($pid1 -ne $ExpectedPid) { Fail $Out "the SolidWorks process identity changed (expected $ExpectedPid, found $pid1)" }

    # ---- the typed interop chain (qualified at runtime) ----
    # The late-bound RCW cast proved unreliable on this seat (a LoadFrom
    # interop identity does not associate with the COM object's type
    # information: "Cannot convert System.__ComObject to ISldWorks").
    # The documented typed path is the interop COCLASS creation: load the
    # assembly by display name (the GAC primary interop assembly, redist
    # fallback) and create the COM object THROUGH its interop type - the
    # same typed chain a compiled C# helper would use. Get-Triple below
    # stays the runtime qualification gate on every array that arrives.
    $stage = "interop assembly load"
    $dllName = "SolidWorks.Interop.sldworks"
    $loaded = $false
    try { [void][System.Reflection.Assembly]::Load($dllName); $loaded = $true } catch { }
    if (-not $loaded) {
        $dll = "C:\Program Files\SOLIDWORKS Corp\SOLIDWORKS\api\redist\$dllName.dll"
        if (-not (Test-Path -LiteralPath $dll)) { Fail $Out "the SolidWorks interop assembly is missing" }
        [void][System.Reflection.Assembly]::LoadFrom($dll)
    }
    $stage = "typed application creation"
    $sw = New-Object SolidWorks.Interop.sldworks.SldWorksClass
    $tSw = [SolidWorks.Interop.sldworks.ISldWorks]
    $tDoc = [SolidWorks.Interop.sldworks.IModelDoc2]
    $tExt = [SolidWorks.Interop.sldworks.IModelDocExtension]
    $tMp = [SolidWorks.Interop.sldworks.IMassProperty]
    $useUnits = $true
    $crossTol = 1E-9   # project choice: cross-check tolerance in metres

    # ---- unique, exact document matching by complete enumeration ----
    $found = @{}
    $stage = "doc walk: GetFirstDocument"
    $doc = T-Method $tSw $sw "GetFirstDocument" @()
    while ($null -ne $doc) {
        $stage = "doc walk: GetTitle"
        $t = [string](T-Method $tDoc $doc "GetTitle" @())
        if ($titles -contains $t) {
            if ($found.ContainsKey($t)) { Fail $Out "the title <$t> matches more than one open document" }
            $found[$t] = $doc
        }
        $stage = "doc walk: GetNext"
        $doc = T-Method $tDoc $doc "GetNext" @()
    }
    $stage = "doc match check"
    foreach ($t in $titles) {
        if (-not $found.ContainsKey($t)) { Fail $Out "the title <$t> matches no open document" }
    }

    # ---- per-document typed acquisition (every stage named in any
    # failure - the qualification discipline) ----
    $outLines = @("adapter:status=ok", "adapter:process=$pid1", "adapter:doc_count=$($titles.Count)")
    $i = 0
    $stage = "start"
    foreach ($t in $titles) {
        $stage = "doc[$i] <$t>: Extension"
        $ext = T-Get $tDoc $found[$t] "Extension"
        $stage = "doc[$i] <$t>: CreateMassProperty"
        $mp = T-Method $tExt $ext "CreateMassProperty" @()
        if ($null -eq $mp) { Fail $Out "doc[$i] <$t>: CreateMassProperty returned null" }
        $stage = "doc[$i] <$t>: UseSystemUnits set"
        T-Set $tMp $mp "UseSystemUnits" $useUnits

        $stage = "doc[$i] <$t>: CenterOfMass get"
        $raw = T-Get $tMp $mp "CenterOfMass"
        $triple = Get-Triple $raw "doc[$i] <$t> CenterOfMass"
        $stage = "doc[$i] <$t>: Volume get"
        $vol = [double](T-Get $tMp $mp "Volume")
        if ([double]::IsNaN($vol) -or [double]::IsInfinity($vol)) { Fail $Out "doc[$i] <$t>: the volume is not finite" }
        if ($vol -le 0) { Fail $Out "doc[$i] <$t>: the volume is not positive" }

        # cross-check against the documented flat 12-double array
        [int]$status = 0
        $stage = "doc[$i] <$t>: GetMassProperties2 call"
        $callArgs = @([object]1, [object]0, [object]$false)
        $flat = T-Method $tExt $ext "GetMassProperties2" $callArgs
        $status = [int]$callArgs[1]
        $stage = "doc[$i] <$t>: GetMassProperties2 status"
        if ($status -ne 0) { Fail $Out "doc[$i] <$t>: GetMassProperties2 status is $status" }
        $stage = "doc[$i] <$t>: GetMassProperties2 shape"
        if ($flat -isnot [double[]] -and $flat -isnot [object[]]) { Fail $Out "doc[$i] <$t>: the flat mass array has an unsupported shape (got $($flat.GetType().FullName))" }
        if ($flat.Rank -ne 1 -or $flat.GetLowerBound(0) -ne 0 -or $flat.Length -ne 12) { Fail $Out "doc[$i] <$t>: the flat mass array is not a zero-based length-12 vector" }
        foreach ($fv in $flat) {
            if ($fv -isnot [double]) { Fail $Out "doc[$i] <$t>: a flat mass component is not a Double value" }
        }
        $stage = "doc[$i] <$t>: cross-check values"
        for ($k = 0; $k -lt 3; $k++) {
            $fk = [double]$flat[$k]
            if ([double]::IsNaN($fk) -or [double]::IsInfinity($fk)) { Fail $Out "doc[$i] <$t>: a cross-check component is not finite" }
            if ([math]::Abs([double]$triple[$k] - $fk) -gt $crossTol) {
                Fail $Out "doc[$i] <$t>: the centroid cross-check failed on component $k"
            }
        }
        if ([math]::Abs($vol - [double]$flat[3]) -gt ($crossTol + 1E-12 * [math]::Abs($vol))) {
            Fail $Out "doc[$i] <$t>: the volume cross-check failed"
        }

        $outLines += "adapter:doc[$i].title=$t"
        $outLines += "adapter:doc[$i].volume=$(Format-Invariant $vol)"
        $outLines += "adapter:doc[$i].cx=$(Format-Invariant ([double]$triple[0]))"
        $outLines += "adapter:doc[$i].cy=$(Format-Invariant ([double]$triple[1]))"
        $outLines += "adapter:doc[$i].cz=$(Format-Invariant ([double]$triple[2]))"
        $outLines += "adapter:doc[$i].crosscheck=ok"
        $i = $i + 1
    }
    $outLines += "adapter:end"
    Write-Result $Out $outLines
    exit 0
} catch {
    Fail $Out ("at stage [" + $stage + "]: " + $_.Exception.Message)
}
