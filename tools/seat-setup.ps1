# seat-setup.ps1 - one command for the owner's Windows seat: verify and run the
# compatibility checkpoint, then collect one results file to paste back.
# Prerequisites: SolidWorks installed and licensed, Git for Windows.
# Review this script and corpus/compat-check/minimal.swb before running.

param(
    [string]$ExpectSha256 = "fb1c7acee6d298f6a4333b9f20777d0dcd20e5e927d0925dbe43c498ff4874ac"
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$reportDir = Join-Path $env:USERPROFILE "craft-bridge-report"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null

# SolidWorks version and invocation details come from the VBScript runner's
# output (smoke-log.txt): no COM is used from PowerShell at all.

# Run the provenance-checked smoke (corpus-only, committed, byte-verified).
# results.txt must be written even when the smoke itself throws, so this call
# is caught and recorded rather than allowed to abort the script.
$smokeError = ""
try {
    & (Join-Path $PSScriptRoot "seat-smoke.ps1") `
        -Macro (Join-Path $repo "corpus\compat-check\minimal.swb") `
        -ExpectSha256 $ExpectSha256 `
        -ReportDir $reportDir
    $code = $LASTEXITCODE
} catch {
    $smokeError = $_.Exception.Message
    $code = 4
    Write-Host "seat-smoke stopped before the macro report: $smokeError"
}

# One file to paste back.
$results = Join-Path $reportDir "results.txt"
"craft-bridge seat run" | Set-Content $results
"date: $(Get-Date -Format o)" | Add-Content $results
"seat-smoke exit code: $code" | Add-Content $results
if ($smokeError -ne "") { "seat-smoke error: $smokeError" | Add-Content $results }
"" | Add-Content $results
"smoke-log.txt:" | Add-Content $results
if (Test-Path (Join-Path $reportDir "smoke-log.txt")) {
    Get-Content (Join-Path $reportDir "smoke-log.txt") | Add-Content $results
} else {
    "(runner produced no log)" | Add-Content $results
}
"" | Add-Content $results
"runtime-report.jsonl:" | Add-Content $results
if (Test-Path (Join-Path $reportDir "runtime-report.jsonl")) {
    Get-Content (Join-Path $reportDir "runtime-report.jsonl") | Add-Content $results
} else {
    "(no report produced)" | Add-Content $results
}

Write-Host ""
Write-Host "==== paste everything below back to the agent ===="
Get-Content $results | ForEach-Object { Write-Host $_ }
exit $code
