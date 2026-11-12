# seat-smoke.ps1 - run one reviewed corpus macro on the owner's SolidWorks seat
# and check its runtime report. This is the ONLY supported way to execute a
# macro on the seat (AGENTS.md seat-safety rule).
#
# It refuses to run unless ALL of these hold:
#   1. the macro sits inside this repository's corpus/ directory;
#   2. git reports the file as tracked and unmodified (committed, reviewed);
#   3. the file's SHA-256 matches -ExpectSha256 (the exact reviewed bytes).
#
# Review this script and the macro before running either. It runs only where
# the operator invokes it, only against a licensed SolidWorks.

param(
    [Parameter(Mandatory = $true)][string]$Macro,
    [Parameter(Mandatory = $true)][string]$ExpectSha256,
    [Parameter(Mandatory = $true)][string]$ReportDir,
    [int]$TimeoutSeconds = 120,
    [switch]$AllowDegraded
)

$ErrorActionPreference = "Stop"

# ---- provenance checks: committed, unmodified, byte-verified ----
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$macroFull = (Resolve-Path $Macro).Path
$corpus = (Resolve-Path (Join-Path $repo "corpus")).Path
if (-not $macroFull.StartsWith($corpus, [StringComparison]::OrdinalIgnoreCase)) {
    throw "refusing to run: the macro is not inside the repository corpus ($corpus)"
}
$rel = $macroFull.Substring($repo.Length).TrimStart("\", "/") -replace "\\", "/"
git -C $repo ls-files --error-unmatch -- $rel 2>$null | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "refusing to run: $rel is not a committed file"
}
git -C $repo diff --quiet -- $rel
if ($LASTEXITCODE -ne 0) {
    throw "refusing to run: $rel has uncommitted changes"
}
$actual = (Get-FileHash -Algorithm SHA256 $macroFull).Hash
if ($actual -ne $ExpectSha256.ToUpperInvariant()) {
    throw "refusing to run: sha256 of $rel is $actual, expected $($ExpectSha256.ToUpperInvariant())"
}

# ---- report setup: only inside the operator-chosen directory ----
$reportDirFull = (Resolve-Path $ReportDir).Path
$reportPath = Join-Path $reportDirFull "runtime-report.jsonl"
if (Test-Path $reportPath) { Remove-Item $reportPath } # never trust a stale report
$env:CRAFT_BRIDGE_REPORT = $reportPath

# ---- run ----
Write-Host "running $rel (sha256 verified) - report: $reportPath"
try {
    $sw = New-Object -ComObject SldWorks.Application
} catch {
    throw "could not attach to SolidWorks via COM (installed and licensed?): $($_.Exception.Message)"
}
Write-Host "attached to SolidWorks"
$sw.Visible = $true
# The legacy 3-argument RunMacro is the primary call: RunMacro2's ByRef error
# argument does not marshal through PowerShell late binding on some machines
# (TYPE_E_ELEMENTNOTFOUND / 0x8002802B). RunMacro2 stays as the fallback.
$invocation = "none"
$ok = $false
try {
    $ok = [bool]$sw.RunMacro($macroFull, "", "main")
    $invocation = "RunMacro"
} catch {
    Write-Host "RunMacro failed: $($_.Exception.Message) - trying RunMacro2"
    try {
        $runError = 0
        $ok = [bool]$sw.RunMacro2($macroFull, "", "main", 1, [ref]$runError)
        $invocation = "RunMacro2 (runError=$runError)"
    } catch {
        throw "both macro invocations failed ($invocation attempt gave: $($_.Exception.Message))"
    }
}
Write-Host "invocation: $invocation - returned $ok"

# ---- wait for the report ----
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
while (-not (Test-Path $reportPath) -and (Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
}
if (-not (Test-Path $reportPath)) { throw "no runtime report appeared at $reportPath" }

# The summary line is the completion signal; wait for it.
$complete = $false
while ((Get-Date) -lt $deadline) {
    $lines = Get-Content $reportPath -ErrorAction SilentlyContinue
    if ($lines -and ($lines[-1] -match '"type":"summary"')) { $complete = $true; break }
    Start-Sleep -Milliseconds 500
}
if (-not $complete) { throw "the runtime report is truncated: no summary line (macro crashed or is still running)" }

# ---- check the report ----
$created = 0; $degraded = 0; $failed = 0; $notExecuted = 0; $steps = 0; $summaryStatus = "?"
foreach ($line in Get-Content $reportPath) {
    if ($line -match '"type":"step"') {
        $steps++
        if ($line -match '"status":"created"') { $created++ }
        elseif ($line -match '"status":"degraded"') { $degraded++ }
        elseif ($line -match '"status":"failed"') { $failed++ }
        elseif ($line -match '"status":"not-executed"') { $notExecuted++ }
    }
    elseif ($line -match '"type":"summary"') {
        if ($line -match '"status":"([^"]+)"') { $summaryStatus = $Matches[1] }
    }
}
if ($steps -lt 1) { throw "the runtime report contains no step lines" }
Write-Host "report: $steps step(s): $created created, $degraded degraded, $failed failed, $notExecuted not-executed - summary: $summaryStatus"

if ($failed -gt 0 -or $summaryStatus -ne "ok") { exit 2 }
if ($degraded -gt 0 -and -not $AllowDegraded) { exit 3 }
Write-Host "PASS"
exit 0
