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
if (Test-Path $probeReport) { Remove-Item $probeReport }
$probeOut = & cscript //nologo (Join-Path $PSScriptRoot "seat-probe.vbs") 2>&1 | ForEach-Object { "$_" }
$probeOk = (Test-Path $probeReport) -and (Get-Content $probeReport | Select-String -Pattern 'summary' -Quiet)
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

        # Provenance: tracked, unmodified, byte-verified.
        $rel = $macro.Substring($RepoRoot.Length).TrimStart("\", "/") -replace "\\", "/"
        git -C $RepoRoot ls-files --error-unmatch -- $rel 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "${name}: $rel is not a committed file" }
        git -C $RepoRoot diff --quiet -- $rel
        if ($LASTEXITCODE -ne 0) { throw "${name}: $rel has uncommitted changes" }
        $sha = (Get-FileHash -Algorithm SHA256 $macro).Hash.ToLower()
        $expectSha = (Select-String -Path $expect -Pattern "^sha256=").Line -replace "^sha256=", ""
        if ($sha -ne $expectSha.ToLower()) { throw "${name}: sha256 $sha != $expectSha" }

        $itemReport = Join-Path $ReportDir "$name-runtime-report.jsonl"
        $env:CRAFT_BRIDGE_REPORT = $itemReport
        if (Test-Path $itemReport) { Remove-Item $itemReport }
        Set-Content -Path $pathFile -Value @($itemReport, $item.FullName)

        $proc = Start-Process -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $PSScriptRoot "run-macro.vbs"), $macro) -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $ReportDir "$name-runner.log") -RedirectStandardError (Join-Path $ReportDir "$name-runner.err.log")
        if (-not $proc.WaitForExit(300000)) { $proc.Dispose(); throw "${name}: the macro runner timed out" }
        if (-not (Test-Path $itemReport)) { throw "${name}: no runtime report" }
        $lines = Get-Content $itemReport
        if (-not ($lines[-1] -match '"type":"summary"')) { throw "${name}: truncated runtime report" }
        $bad = @($lines | Where-Object { $_ -match '"status":"failed"' })
        if ($bad.Count -gt 0) { Write-Host "  runtime: FAILED steps" } else { Write-Host "  runtime: complete" }

        # Save the rebuilt part as evidence and check it.
        $checkOut = Join-Path $ReportDir "$name-checks.txt"
        $proc = Start-Process -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $PSScriptRoot "check-part.vbs"), $expect, $stepRef, $checkOut) -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $ReportDir "$name-check.log") -RedirectStandardError (Join-Path $ReportDir "$name-check.err.log")
        if (-not $proc.WaitForExit(300000)) { $proc.Dispose(); throw "${name}: the checker timed out" }
        $checks = if (Test-Path $checkOut) { Get-Content $checkOut } else { @("check:checker=failed") }
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
Remove-Item $pathFile -ErrorAction SilentlyContinue
$report = [ordered]@{ generated_by = "craft-bridge seat-harness"; items = $results; failed = $failed }
$report | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $ReportDir "harness-report.json")
Write-Host ""
Write-Host "harness-report.json written; $failed item(s) failed"
if ($failed -gt 0) { exit 2 }
Write-Host "PASS"
exit 0
