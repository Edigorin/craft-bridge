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
# The macro runs inside the SolidWorks process, which may be an instance that
# is already open and inherited an older environment. The path therefore also
# travels as a one-line file under %TEMP%, which every process can see.
$env:CRAFT_BRIDGE_REPORT = $reportPath
$pathFile = Join-Path $env:TEMP "craft-bridge-report-path.txt"
Set-Content -Path $pathFile -Value $reportPath

# ---- run: all SolidWorks COM goes through tools/run-macro.vbs (VBScript is
# IDispatch-native; PowerShell's late binder has failed on this seat) ----
Write-Host "running $rel (sha256 verified) - report: $reportPath"
$runner = Join-Path $repo "tools\run-macro.vbs"
$vbsOut = & cscript //nologo $runner $macroFull 2>&1 | ForEach-Object { "$_" }
$vbsExit = $LASTEXITCODE
$vbsOut | ForEach-Object { Write-Host $_ }
$vbsOut | Set-Content (Join-Path $reportDirFull "smoke-log.txt")
Write-Host "runner exit code: $vbsExit"

# ---- wait for the report ----
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
while (-not (Test-Path $reportPath) -and (Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
}
if (-not (Test-Path $reportPath)) {
    Get-ChildItem $reportDirFull | Out-String | Add-Content (Join-Path $reportDirFull "smoke-log.txt")
    throw "no runtime report appeared at $reportPath (directory listing appended to smoke-log.txt)"
}

# The summary line is the completion signal; wait for it.
$complete = $false
while ((Get-Date) -lt $deadline) {
    $lines = Get-Content $reportPath -ErrorAction SilentlyContinue
    if ($lines -and ($lines[-1] -match '"type":"summary"')) { $complete = $true; break }
    Start-Sleep -Milliseconds 500
}
if (-not $complete) {
    Get-ChildItem $reportDirFull | Out-String | Add-Content (Join-Path $reportDirFull "smoke-log.txt")
    throw "the runtime report is truncated: no summary line (macro crashed or is still running)"
}
Remove-Item $pathFile -ErrorAction SilentlyContinue

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
