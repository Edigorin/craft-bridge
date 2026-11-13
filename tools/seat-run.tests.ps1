# seat-run.tests.ps1 - synthetic process tests for tools/seat-run.ps1.
# Runs on any Windows machine: no SolidWorks, no corpus, no seat needed.
# Every seat script gates on exit codes, so the exit-code plumbing itself
# must be proven - a gate reading $null is worse than no gate.
#
# Usage: powershell -ExecutionPolicy Bypass -File .\tools\seat-run.tests.ps1
# Exit codes: 0 all passed, 1 a case failed.

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "seat-run.ps1")

$failures = 0
function Check($name, $condition, $detail) {
    if ($condition) {
        Write-Host "pass: $name"
    } else {
        $script:failures++
        Write-Host "FAIL: $name - $detail"
    }
}

# 1. zero exit
$r = Invoke-SeatCommand -FilePath "cmd" -ArgumentList @("/c", "exit 0") -TimeoutSeconds 30
Check "zero exit reads as 0" (($r.ExitCode -eq 0) -and (-not $r.TimedOut)) "ExitCode=$($r.ExitCode) TimedOut=$($r.TimedOut)"

# 2. nonzero exit
$r = Invoke-SeatCommand -FilePath "cmd" -ArgumentList @("/c", "exit 7") -TimeoutSeconds 30
Check "nonzero exit reads as 7" ($r.ExitCode -eq 7) "ExitCode=$($r.ExitCode)"

# 3. misleading success output: the text says success, the code disagrees.
# The gate must key on the code - this is exactly the Start-Process trap.
$r = Invoke-SeatCommand -FilePath "cmd" -ArgumentList @("/c", "echo invocation returned: success & exit 5") -TimeoutSeconds 30
Check "success text never overrides the code" (($r.ExitCode -eq 5) -and ($r.StdOut -match "success")) "ExitCode=$($r.ExitCode) StdOut='$($r.StdOut)'"

# 4. both output streams are captured separately
$r = Invoke-SeatCommand -FilePath "cmd" -ArgumentList @("/c", "echo to-out & echo to-err 1>&2") -TimeoutSeconds 30
Check "stdout captured" ($r.StdOut -match "to-out") "StdOut='$($r.StdOut)'"
Check "stderr captured" ($r.StdErr -match "to-err") "StdErr='$($r.StdErr)'"

# 5. arguments with spaces survive quoting
$dir = Join-Path $env:TEMP ("seat run test " + [Guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Path $dir | Out-Null
$scriptPath = Join-Path $dir "my script.cmd"
Set-Content -Path $scriptPath -Value "@echo off`r`necho spaced ok`r`nexit 3"
try {
    $r = Invoke-SeatCommand -FilePath "cmd" -ArgumentList @("/c", $scriptPath) -TimeoutSeconds 30
    Check "spaced path runs" (($r.ExitCode -eq 3) -and ($r.StdOut -match "spaced ok")) "ExitCode=$($r.ExitCode) StdOut='$($r.StdOut)'"
} finally {
    # Left in place: the owner's seat runs no delete commands.
}

# 6. timeout: the child is killed, the result says so, and nothing blocks
$r = Invoke-SeatCommand -FilePath "cmd" -ArgumentList @("/c", "ping -n 30 127.0.0.1 >nul") -TimeoutSeconds 2
Check "timeout is reported" ($r.TimedOut -and ($null -eq $r.ExitCode)) "TimedOut=$($r.TimedOut) ExitCode=$($r.ExitCode)"

# 7. evidence archival: stale files move aside, never delete (owner rule)
$adir = Join-Path $env:TEMP ("seat archive test " + [Guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Path $adir | Out-Null
$src = Join-Path $adir "runtime-report.jsonl"
$gone = Move-SeatEvidenceAside -Path (Join-Path $adir "missing.jsonl")
Check "archive of absent source is a no-op" ($null -eq $gone) "returned='$gone'"
Set-Content -Path $src -Value "payload-line"
$arch = Move-SeatEvidenceAside -Path $src -Suffix "first"
Check "archive preserves content and clears the source" (($null -ne $arch) -and (-not (Test-Path -LiteralPath $src)) -and ((Get-Content -LiteralPath $arch) -eq "payload-line")) "arch='$arch'"
Set-Content -Path $src -Value "second"
$threw = $false
try { Move-SeatEvidenceAside -Path $src -Suffix "first" | Out-Null } catch { $threw = $true }
Check "archive collision fails closed" ($threw -and ((Get-Content -LiteralPath $src) -eq "second") -and ((Get-Content -LiteralPath $arch) -eq "payload-line")) "threw=$threw"
$threw = $false
try { Move-SeatEvidenceAside -Path $src -Suffix "missing-parent\file" | Out-Null } catch { $threw = $true }
Check "archive move failure throws and preserves the source" ($threw -and ((Get-Content -LiteralPath $src) -eq "second")) "threw=$threw"

if ($failures -gt 0) {
    Write-Host "$failures test(s) failed"
    exit 1
}
Write-Host "all seat-run tests passed"
exit 0
