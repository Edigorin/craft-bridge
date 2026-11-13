# seat-harness.ps1 - one command for the corpus acceptance run (ticket #9).
#
# A probe first (plain VBScript must be able to write the runtime report),
# then every committed corpus item: provenance (tracked, unmodified,
# sha256-verified against expect.txt), the committed macro run through
# tools/run-macro.vbs, a complete runtime report required, the rebuilt part
# saved as evidence, and tools/check-part.vbs checking tree count, dimension
# values and mass properties against the STEP reference. Failures are recorded
# and the corpus continues. Nothing outside the committed corpus ever runs.
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$ReportDir = (Join-Path (Get-Location) "harness-report"),
    [switch]$AllowDegraded
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "seat-run.ps1")
$corpus = Join-Path $RepoRoot "corpus"
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null
$pathFile = Join-Path $env:TEMP "craft-bridge-report-path.txt"

$items = Get-ChildItem -Path $corpus -Directory | Where-Object {
    (Test-Path (Join-Path $_.FullName "item.craftx")) -and
    (Test-Path (Join-Path $_.FullName "item.swb")) -and
    (Test-Path (Join-Path $_.FullName "expect.txt"))
}
if (-not $items) { throw "no corpus items found under $corpus" }

$results = @()
$failed = 0

# ---- probe: plain VBScript writes the report the macros must write ----
$probeReport = Join-Path $ReportDir "probe-report.jsonl"
$env:CRAFT_BRIDGE_REPORT = $probeReport
Move-SeatEvidenceAside -Path $probeReport | Out-Null
$pr = Invoke-SeatCommand -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $PSScriptRoot "seat-probe.vbs")) -TimeoutSeconds 120
$probeOut = @($pr.StdOut -split "`r?`n") + @($pr.StdErr -split "`r?`n") | Where-Object { $_ -ne "" }
$probeOk = (-not $pr.TimedOut) -and ($pr.ExitCode -eq 0) -and (Test-Path $probeReport) -and (Get-Content $probeReport | Select-String -Pattern 'summary' -Quiet)
Write-Host "== probe (plain VBScript report write): $(if ($probeOk) { 'ok' } else { 'FAILED' })"
$probeOut | ForEach-Object { Write-Host "  $_" }
$results += [ordered]@{ item = "probe-vbs-report-write"; passed = [bool]$probeOk; output = @($probeOut) }
if (-not $probeOk) { $failed++ }

# ---- one guarded run per corpus item ----
foreach ($item in $items) {
    $name = $item.Name
    try {
        $macro = Join-Path $item.FullName "item.swb"
        $expect = Join-Path $item.FullName "expect.txt"
        $stepRef = Join-Path $item.FullName "reference.step"
        Write-Host "== $name"

        # Provenance: tracked, unmodified against HEAD (staged changes count
        # as uncommitted here), byte-verified.
        $rel = $macro.Substring($RepoRoot.Length).TrimStart("\", "/") -replace "\\", "/"
        git -C $RepoRoot ls-files --error-unmatch -- $rel 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "${name}: $rel is not a committed file" }
        git -C $RepoRoot diff HEAD --quiet -- $rel
        if ($LASTEXITCODE -ne 0) { throw "${name}: $rel differs from HEAD (commit it first)" }
        $sha = (Get-FileHash -Algorithm SHA256 $macro).Hash.ToLower()
        $expectSha = (Select-String -Path $expect -Pattern "^sha256=").Line -replace "^sha256=", ""
        if ($sha -ne $expectSha.ToLower()) { throw "${name}: sha256 $sha != $expectSha" }

        $itemReport = Join-Path $ReportDir "$name-runtime-report.jsonl"
        $env:CRAFT_BRIDGE_REPORT = $itemReport
        Move-SeatEvidenceAside -Path $itemReport | Out-Null
        Set-Content -Path $pathFile -Value @($itemReport, $item.FullName)

        $r = Invoke-SeatCommand -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $PSScriptRoot "run-macro.vbs"), $macro) -TimeoutSeconds 300
        $r.StdOut | Set-Content (Join-Path $ReportDir "$name-runner.log")
        $r.StdErr | Set-Content (Join-Path $ReportDir "$name-runner.err.log")
        if ($r.TimedOut) {
            Write-Host "  TIMEOUT: the macro runner is still connected to SolidWorks"
            Write-Host "  stopping the seat run: killing the runner does not stop the macro inside SolidWorks - resolve it there, then rerun"
            $results += [ordered]@{ item = $name; passed = $false; error = "macro runner timed out; seat run stopped" }
            $failed++
            break
        }
        if ($null -eq $r.ExitCode) { throw "${name}: run-macro.vbs exit code is unreadable" }
        if ($r.ExitCode -ne 0) { throw "${name}: run-macro.vbs exited $($r.ExitCode)" }
        if (-not (Test-Path $itemReport)) { throw "${name}: no runtime report" }
        $lines = @(Get-Content $itemReport)
        if ($lines.Count -eq 0 -or -not ($lines[-1] -match '"type":"summary"')) { throw "${name}: truncated runtime report" }
        $bad = @($lines | Where-Object { $_ -match '"status":"failed"' })
        if ($bad.Count -gt 0) { Write-Host "  runtime: FAILED steps" } else { Write-Host "  runtime: complete" }

        # Check the rebuilt part: fresh evidence only - an old checks file is
        # removed first and an empty or malformed one is a failure.
        $checkOut = Join-Path $ReportDir "$name-checks.txt"
        Move-SeatEvidenceAside -Path $checkOut | Out-Null
        $r = Invoke-SeatCommand -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $PSScriptRoot "check-part.vbs"), $expect, $stepRef, $checkOut) -TimeoutSeconds 300
        $r.StdOut | Set-Content (Join-Path $ReportDir "$name-check.log")
        $r.StdErr | Set-Content (Join-Path $ReportDir "$name-check.err.log")
        if ($r.TimedOut) {
            Write-Host "  TIMEOUT: the checker is still connected to SolidWorks"
            Write-Host "  stopping the seat run: killing the checker does not stop its work inside SolidWorks - resolve it there, then rerun"
            $results += [ordered]@{ item = $name; passed = $false; error = "checker timed out; seat run stopped" }
            $failed++
            break
        }
        if ($null -eq $r.ExitCode) { throw "${name}: check-part.vbs exit code is unreadable" }
        if ($r.ExitCode -ne 0) { throw "${name}: check-part.vbs exited $($r.ExitCode)" }
        if (-not (Test-Path $checkOut) -or (Get-Item $checkOut).Length -eq 0) { throw "${name}: the checker wrote no evidence" }
        $checks = @(Get-Content $checkOut)
        if (@($checks | Where-Object { $_ -notmatch '^(check|detail|volume):' }).Count -gt 0) { throw "${name}: malformed checker evidence" }
        $itemFailed = ($bad.Count -gt 0) -or (@($checks | Where-Object { $_ -match '=fail' }).Count -gt 0)
        if ($itemFailed) { $failed++ }
        $results += [ordered]@{ item = $name; runtime_ok = ($bad.Count -eq 0); checks = $checks; passed = -not $itemFailed }
        Write-Host "  checks: $(@($checks | Where-Object { $_ -match '=pass' }).Count) pass, $(@($checks | Where-Object { $_ -match '=fail' }).Count) fail"
    } catch {
        $failed++
        Write-Host "  FAILED: $($_.Exception.Message)"
        $tail = @()
        foreach ($log in @("$name-runner.log", "$name-runner.err.log", "$name-check.log")) {
            $path = Join-Path $ReportDir $log
            if (Test-Path $path) {
                $tail += "--- $log ---"
                $tail += (Get-Content $path | Select-Object -Last 12)
            }
        }
        $results += [ordered]@{ item = $name; passed = $false; error = $_.Exception.Message; logs = $tail }
    }
}
# The path file is left in place: the seat runs no delete commands, and
# every run overwrites it before use.
$report = [ordered]@{ generated_by = "craft-bridge seat-harness"; items = $results; failed = $failed }
$report | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $ReportDir "harness-report.json")
Write-Host ""
Write-Host "harness-report.json written; $failed item(s) failed"
if ($failed -gt 0) { exit 2 }
Write-Host "PASS"
exit 0
