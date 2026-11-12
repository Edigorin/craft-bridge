# seat-setup.ps1 - one command for the owner's Windows seat: verify and run the
# compatibility checkpoint, then collect one results file to paste back.
# Prerequisites: SolidWorks installed and licensed, Git for Windows.
# Review this script and corpus/compat-check/minimal.swb before running.

param(
    [string]$ExpectSha256 = "0c2cb4bbdaf561694e8b890ace51185924e3bc55db0d01eaa7bb56550c6c8458"
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$reportDir = Join-Path $env:USERPROFILE "craft-bridge-report"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null

# SolidWorks version through COM (informational; opens no document).
$ver = "unavailable"
try {
    $sw = New-Object -ComObject SldWorks.Application
    $ver = $sw.RevisionNumber()
} catch {
    $ver = "unavailable: $($_.Exception.Message)"
}

# Run the provenance-checked smoke (corpus-only, committed, byte-verified).
& (Join-Path $PSScriptRoot "seat-smoke.ps1") `
    -Macro (Join-Path $repo "corpus\compat-check\minimal.swb") `
    -ExpectSha256 $ExpectSha256 `
    -ReportDir $reportDir
$code = $LASTEXITCODE

# One file to paste back.
$results = Join-Path $reportDir "results.txt"
"craft-bridge seat run" | Set-Content $results
"date: $(Get-Date -Format o)" | Add-Content $results
"solidworks revision: $ver" | Add-Content $results
"seat-smoke exit code: $code" | Add-Content $results
"" | Add-Content $results
"runtime-report.jsonl:" | Add-Content $results
if (Test-Path (Join-Path $reportDir "runtime-report.jsonl")) {
    Get-Content (Join-Path $reportDir "runtime-report.jsonl") | Add-Content $results
} else {
    "(no report produced)" | Add-Content $results
}

Write-Host ""
Write-Host "==== paste the contents of $results back to the agent ===="
exit $code
