# seat-run-real.ps1 - run ONE owner-provided real design on the seat under
# the AGENTS.md "Real-part private-run exception" (owner-approved
# 2026-11-17). This is the ONLY supported path for a real client design to
# touch the seat; the synthetic committed-corpus runners (seat-smoke.ps1,
# seat-harness.ps1) keep their own provenance requirements unchanged.
#
# The exception's hard rules, enforced here:
#   - every real-part artifact (source design, generated macro, derived
#     report, log, plan, sidecar) stays UNTRACKED: inputs live under
#     uploads/<design>/ and everything this runner writes goes under
#     runs/<design>/ - both gitignored, and a TRACKED input is refused;
#   - every run pins source-design, generated-macro, reference-STEP and
#     tooling shas in the untracked manifest and this runner verifies every
#     byte against those pins - anything not pinned is refused;
#   - the FIRST run of each design needs the owner's explicit approval of
#     the exact macro sha256 (recorded as approved_macro_sha256 in
#     runs/<design>/manifest.json by the orchestrator after the owner says
#     yes); later runs of the SAME macro hash proceed; any macro change
#     re-approvals;
#   - real-design acceptance REQUIRES mass_vs_reference=pass (real designs
#     always compare against their reference STEP - there is no placeholder).
#
# Usage: powershell -ExecutionPolicy Bypass -File .\tools\seat-run-real.ps1 -Design <name>
# The manifest (runs/<design>/manifest.json), written by the orchestrator:
#   {
#     "design": "<name>",
#     "files": {
#       "source":    { "path": "uploads/<name>/<file>", "sha256": "<hex>" },
#       "macro":     { "path": "uploads/<name>/<file>", "sha256": "<hex>" },
#       "reference": { "path": "uploads/<name>/<file>", "sha256": "<hex>" },
#       "expect":    { "path": "uploads/<name>/<file>", "sha256": "<hex>" }
#     },
#     "tooling": {
#       "tools/seat-run.ps1": "<hex>", "tools/run-macro.vbs": "<hex>",
#       "tools/check-part.vbs": "<hex>", "tools/check-part-logic.vbs": "<hex>"
#     },
#     "approved_macro_sha256": "<hex>"
#   }
# Every run also writes runs/<design>/run-manifest.json: the pins this run
# verified and its outcome (the untracked per-run pin record the exception
# requires).
#
# Exit codes: 0 accepted; 2 the run completed but acceptance failed;
# 4 refused before running (manifest, paths, pins, approval, provenance);
# 5 the run could not complete (runner/checker errors).

param(
    [Parameter(Mandatory = $true)][string]$Design,
    [string]$RepoRoot
)
$ErrorActionPreference = "Stop"
# $PSScriptRoot is not bound inside param() default expressions when the
# script runs under powershell.exe -File (the scheduled-task form), so the
# default resolves here, in the body.
if (-not $RepoRoot) { $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path }
. (Join-Path $PSScriptRoot "seat-run.ps1")

# Every tooling script this run executes - THIS RUNNER INCLUDED - must be
# pinned: the pins cover the execution path, not just the repo copy.
function Get-RequiredTooling { @("tools/seat-run-real.ps1", "tools/seat-run.ps1", "tools/run-macro.vbs", "tools/check-part.vbs", "tools/check-part-logic.vbs") }

function Get-RealFileSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLower()
}

# A manifest input path must be relative and resolve INSIDE
# uploads/<design>/ - no absolute paths, no traversal, no other tree.
function Test-RealInputPath {
    param(
        [Parameter(Mandatory = $true)][string]$DesignName,
        [Parameter(Mandatory = $true)][string]$RelPath,
        [Parameter(Mandatory = $true)][string]$Root
    )
    if ($RelPath -eq "" -or [System.IO.Path]::IsPathRooted($RelPath)) {
        throw "refusing: the manifest pins a non-relative path ($RelPath)"
    }
    $full = [System.IO.Path]::GetFullPath((Join-Path $Root $RelPath))
    $zone = [System.IO.Path]::GetFullPath((Join-Path (Join-Path $Root "uploads") $DesignName))
    $prefix = $zone + [System.IO.Path]::DirectorySeparatorChar
    if (-not $full.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "refusing: $RelPath is not inside uploads/$DesignName/"
    }
    return $full
}

# Real-part artifacts stay UNTRACKED (the hard rule): a file git knows is
# refused, loudly.
# A native command's stderr becomes ErrorRecords under
# $ErrorActionPreference='Stop' EVEN when redirected to $null (PS 5.1),
# and git's expected "pathspec did not match" for an untracked file goes
# to stderr - so a plain call would abort the run. Invoke git with the
# preference relaxed locally (restored on exit) and capture its output.
function Invoke-QuietGit {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string[]]$GitArgs
    )
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $out = @(& git -C $Root @GitArgs 2>$null)
        return [pscustomobject]@{ Out = $out; Code = $LASTEXITCODE }
    } finally {
        $ErrorActionPreference = $prevEap
    }
}

function Test-RealUntracked {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$RelPath
    )
    $r = Invoke-QuietGit -Root $Root -GitArgs @("ls-files", "--error-unmatch", "--", $RelPath)
    $code = $r.Code
    if ($code -eq 0) {
        throw "refusing: $RelPath is a tracked file; real-part artifacts must stay untracked (AGENTS.md)"
    }
    if ($code -ne 1) {
        throw "refusing: the untracked check for $RelPath could not be established (git exited $code)"
    }
}

# A whole zone (manifest and output destinations live here) must contain
# no tracked file at all.
function Test-RealZoneUntracked {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$RelDir
    )
    $r = Invoke-QuietGit -Root $Root -GitArgs @("ls-files", "--", $RelDir)
    $out = $r.Out
    $code = $r.Code
    if ($code -ne 0) {
        throw "refusing: the untracked check for $RelDir could not be established (git exited $code)"
    }
    if (@($out).Count -gt 0) {
        throw "refusing: $RelDir contains tracked files; real-part artifacts must stay untracked (AGENTS.md)"
    }
}

# Every pinned byte must match its manifest pin.
function Test-RealPin {
    param(
        [Parameter(Mandatory = $true)][string]$ExpectedSha,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$What
    )
    if (-not (Test-Path -LiteralPath $Path)) { throw "refusing: the pinned $What does not exist ($Path)" }
    $actual = Get-RealFileSha256 -Path $Path
    if ($actual -ne $ExpectedSha.ToLower()) {
        throw "refusing: $What sha256 $actual does not match the manifest pin $($ExpectedSha.ToLower())"
    }
    return $actual
}

# The owner's approval token is the exact macro sha256 recorded in the
# manifest after the owner approves these bytes.
function Test-RealApproval {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$MacroSha
    )
    $approved = ""
    if ($Manifest.PSObject.Properties["approved_macro_sha256"] -and $Manifest.approved_macro_sha256) {
        $approved = "$($Manifest.approved_macro_sha256)".ToLower()
    }
    if ($approved -eq "") {
        throw "refusing: this design's first run needs the owner's explicit approval of the exact macro sha256 ($MacroSha) - recorded as approved_macro_sha256 in the manifest after the owner says yes"
    }
    if ($approved -ne $MacroSha.ToLower()) {
        throw "refusing: the macro changed since approval (approved $approved, now $($MacroSha.ToLower())); the owner's re-approval is required"
    }
}

# The runtime report itself: complete, successful, and free of failed
# steps (a failed preference restoration or close fails the run, exactly
# as the corpus harness requires). Returns "" when accepted.
function Test-RealReport {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Lines)
    if (@($Lines).Count -eq 0) { return "the runtime report is empty" }
    if ($Lines[-1] -notmatch '"type":"summary"') { return "truncated runtime report (no summary line)" }
    if ($Lines[-1] -notmatch '"status":"ok"') { return "the runtime summary is not ok: $($Lines[-1])" }
    $bad = @($Lines | Where-Object { $_ -match '"status":"failed"' })
    if ($bad.Count -gt 0) { return "the runtime report carries failed steps: $($bad[0])" }
    return ""
}

# Real-design acceptance: the FULL verdict set - the mass comparison and
# every companion check (import, bodies, placement, reference close) -
# each present EXACTLY ONCE and passing; a real design always compares
# against its reference STEP (no placeholder). Returns "" when accepted,
# else the named reason.
function Test-RealAcceptance {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Checks)
    $required = @("check:mass", "check:mass_vs_reference", "check:step_reference", "check:bodies", "check:placement", "check:ref_close")
    foreach ($name in $required) {
        $hits = @($Checks | Where-Object { $_ -like "$name=*" })
        if ($hits.Count -eq 0) { return "the required $name entry is absent" }
        if ($hits.Count -gt 1) { return "the $name entry is not unique" }
        if ($hits[0] -ne "$name=pass") { return "$name is not pass ($($hits[0]))" }
    }
    $bad = @($Checks | Where-Object { $_ -match '=fail' })
    if ($bad.Count -gt 0) { return "failed checks: $($bad -join '; ')" }
    return ""
}

# The manifest shape: every input pinned, every pin shaped like a sha256.
function Read-RealManifest {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { throw "refusing: no manifest at $Path" }
    $raw = Get-Content -LiteralPath $Path -Raw
    try {
        $m = $raw | ConvertFrom-Json
    } catch {
        throw "refusing: the manifest is not valid JSON: $($_.Exception.Message)"
    }
    foreach ($what in @("source", "macro", "reference", "expect")) {
        $entry = $null
        if ($m.PSObject.Properties["files"] -and $m.files.PSObject.Properties[$what]) { $entry = $m.files.$what }
        if ($null -eq $entry -or -not $entry.path -or -not $entry.sha256) {
            throw "refusing: the manifest does not pin $what (files.$what.path and .sha256 are required)"
        }
        if ("$($entry.sha256)" -notmatch '^[0-9a-fA-F]{64}$') {
            throw "refusing: the manifest pin for $what is not a sha256 ($($entry.sha256))"
        }
    }
    if (-not $m.PSObject.Properties["tooling"] -or $null -eq $m.tooling) {
        throw "refusing: the manifest does not pin the tooling"
    }
    return $m
}


# The tooling this runner executes must all be pinned; anything not pinned
# is refused. The pins are verified against the EXECUTED bytes - the files
# at ScriptDir (where this script and its callees live) - never against a
# possibly different checkout at Root. Returns the verified pins.
function Test-RealToolingPins {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$ScriptDir
    )
    $toolPins = @{}
    foreach ($tool in Get-RequiredTooling) {
        if (-not $Manifest.tooling.PSObject.Properties[$tool] -or -not $Manifest.tooling.$tool) {
            throw "refusing: the manifest does not pin $tool (anything not pinned is refused)"
        }
        $toolFull = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir (Split-Path -Leaf $tool)))
        $toolPins[$tool] = Test-RealPin -ExpectedSha "$($Manifest.tooling.$tool)" -Path $toolFull -What "tooling $tool (executed bytes)"
    }
    return $toolPins
}

# Every manifest input AND every extra pinned file (fallback sidecars are
# CONSUMED by the run): confined to uploads/<design>/, untracked, pinned.
# Anything the run consumes without a pin is refused.
function Test-RealInputPins {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$DesignName,
        [Parameter(Mandatory = $true)][string]$Root
    )
    $inputs = @{}
    foreach ($what in $Manifest.files.PSObject.Properties.Name) {
        $entry = $Manifest.files.$what
        $full = Test-RealInputPath -DesignName $DesignName -RelPath "$($entry.path)" -Root $Root
        $rel = "$($entry.path)" -replace "\\", "/"
        Test-RealUntracked -Root $Root -RelPath $rel
        $null = Test-RealPin -ExpectedSha "$($entry.sha256)" -Path $full -What $what
        $inputs[$what] = $full
    }
    foreach ($what in @("source", "macro", "reference", "expect")) {
        if (-not $inputs.ContainsKey($what)) {
            throw "refusing: the manifest does not pin $what"
        }
    }
    # Consumed fallback sidecars beside the macro must each be pinned.
    $macroDir = Split-Path -Parent $inputs["macro"]
    $sidecars = @(Get-ChildItem -LiteralPath $macroDir -Filter "fallback-step-*.step" -File -ErrorAction SilentlyContinue)
    foreach ($sc in $sidecars) {
        $scRel = ($sc.FullName.Substring([System.IO.Path]::GetFullPath($Root).Length)).TrimStart("\", "/") -replace "\\", "/"
        $hit = @($Manifest.files.PSObject.Properties | Where-Object { "$($_.Value.path)" -replace "\\", "/" -eq $scRel })
        if ($hit.Count -eq 0) {
            throw "refusing: a consumed fallback sidecar is not pinned ($scRel)"
        }
    }
    return $inputs
}

# The per-run pin record the exception requires: a UNIQUE file per run
# under runs/<design>/runs/, written BEFORE the run with the planned pins
# and patched with its outcome afterwards. Earlier records are never
# touched - every run's evidence survives the next one.
function New-RunRecord {
    param(
        [Parameter(Mandatory = $true)][string]$RunsDir,
        [Parameter(Mandatory = $true)][string]$DesignName,
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$MacroSha,
        [Parameter(Mandatory = $true)]$Verified
    )
    $runId = "run-" + (Get-Date -Format "yyyyMMdd-HHmmss") + "-" + [Guid]::NewGuid().ToString("N").Substring(0, 8)
    $runsSub = Join-Path $RunsDir "runs"
    New-Item -ItemType Directory -Force -Path $runsSub | Out-Null
    $path = Join-Path $runsSub "$runId.json"
    $record = [ordered]@{
        run_id = $runId
        started = (Get-Date).ToString("o")
        design = $DesignName
        approved_macro_sha256 = "$($Manifest.approved_macro_sha256)"
        macro_sha256 = $MacroSha
        verified = $Verified
        outcome = "started"
    }
    $record | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $path
    return $path
}

function Complete-RunRecord {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Outcome
    )
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $raw = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    $raw.outcome = $Outcome
    $raw | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Path
}

function Invoke-RealDesignRun {
    param(
        [Parameter(Mandatory = $true)][string]$DesignName,
        [Parameter(Mandatory = $true)][string]$Root
    )
    # The design name becomes a path segment twice: refuse anything that
    # could escape it.
    if ($DesignName -eq "" -or $DesignName -match '[\\/:\*\?"<>\|]' -or $DesignName -match '\.\.') {
        throw "refusing: the design name is not a plain directory name ($DesignName)"
    }
    $runsDir = Join-Path (Join-Path $Root "runs") $DesignName
    $manifestPath = Join-Path $runsDir "manifest.json"
    Test-RealZoneUntracked -Root $Root -RelDir "runs/$DesignName"
    Test-RealUntracked -Root $Root -RelPath "runs/$DesignName/manifest.json"
    $manifest = Read-RealManifest -Path $manifestPath
    if ("$($manifest.design)" -ne $DesignName) {
        throw "refusing: the manifest design ($($manifest.design)) is not $DesignName"
    }

    # Inputs: confined to uploads/<design>/, untracked, byte-pinned.
    $inputs = Test-RealInputPins -Manifest $manifest -DesignName $DesignName -Root $Root

    # Tooling: every script this runner executes must be pinned and match
    # the EXECUTED bytes (at $PSScriptRoot), this runner included.
    $toolPins = Test-RealToolingPins -Manifest $manifest -ScriptDir $PSScriptRoot

    # The approval gate: the exact macro bytes the owner approved.
    $macroSha = Get-RealFileSha256 -Path $inputs["macro"]
    Test-RealApproval -Manifest $manifest -MacroSha $macroSha

    # Everything this run writes stays under runs/<design>/ (untracked).
    New-Item -ItemType Directory -Force -Path $runsDir | Out-Null
    $reportPath = Join-Path $runsDir "runtime-report.jsonl"
    $checkOut = Join-Path $runsDir "checks.txt"
    Move-SeatEvidenceAside -Path $reportPath | Out-Null
    Move-SeatEvidenceAside -Path $checkOut | Out-Null

    # The per-run pin record the exception requires (untracked): written
    # BEFORE the invocation with the planned pins, so an exception can
    # never leave a run without evidence.
    $verified = [ordered]@{
        source = [ordered]@{ path = $manifest.files.source.path; sha256 = (Get-RealFileSha256 -Path $inputs["source"]) }
        macro = [ordered]@{ path = $manifest.files.macro.path; sha256 = $macroSha }
        reference = [ordered]@{ path = $manifest.files.reference.path; sha256 = (Get-RealFileSha256 -Path $inputs["reference"]) }
        expect = [ordered]@{ path = $manifest.files.expect.path; sha256 = (Get-RealFileSha256 -Path $inputs["expect"]) }
        tooling = $toolPins
    }
    $runRecord = New-RunRecord -RunsDir $runsDir -DesignName $DesignName -Manifest $manifest -MacroSha $macroSha -Verified $verified
    $accepted = ""
    try {
    # Run the macro exactly like the corpus harness does: the TEMP path
    # file protocol, the true exit code, a complete runtime report.
    $pathFile = Join-Path $env:TEMP "craft-bridge-report-path.txt"
    $env:CRAFT_BRIDGE_REPORT = $reportPath
    $macroDir = Split-Path -Parent $inputs["macro"]
    Set-Content -Path $pathFile -Value @($reportPath, $macroDir)
    $r = Invoke-SeatCommand -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $PSScriptRoot "run-macro.vbs"), $inputs["macro"]) -TimeoutSeconds 600
    $r.StdOut | Set-Content (Join-Path $runsDir "runner.log")
    $r.StdErr | Set-Content (Join-Path $runsDir "runner.err.log")
    if ($r.TimedOut) {
        $accepted = "the macro runner timed out; resolve it in SolidWorks, then rerun"
    } elseif ($null -eq $r.ExitCode) {
        $accepted = "run-macro.vbs exit code is unreadable"
    } elseif ($r.ExitCode -ne 0) {
        $accepted = "run-macro.vbs exited $($r.ExitCode)"
    } elseif (-not (Test-Path -LiteralPath $reportPath)) {
        $accepted = "no runtime report"
    } else {
        $lines = @(Get-Content -LiteralPath $reportPath)
        $accepted = Test-RealReport -Lines $lines
        if ($accepted -eq "") {
            $ownedTitle = ""
            foreach ($ln in $lines) {
                if ($ln -match '"command":"owned-document"' -and $ln -match '"message":"([^"]*)"') { $ownedTitle = $Matches[1] }
            }
            if (-not $ownedTitle) {
                $accepted = "the macro reported no owned-document handoff; nothing will be closed"
            } else {
                $rc = Invoke-SeatCommand -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $PSScriptRoot "check-part.vbs"), $inputs["expect"], $inputs["reference"], $checkOut, $ownedTitle) -TimeoutSeconds 300
                $rc.StdOut | Set-Content (Join-Path $runsDir "check.log")
                $rc.StdErr | Set-Content (Join-Path $runsDir "check.err.log")
                if ($rc.TimedOut) {
                    $accepted = "the checker timed out; resolve it in SolidWorks, then rerun"
                } elseif ($null -eq $rc.ExitCode) {
                    $accepted = "check-part.vbs exit code is unreadable"
                } elseif ($rc.ExitCode -ne 0) {
                    $accepted = "check-part.vbs exited $($rc.ExitCode)"
                } elseif (-not (Test-Path -LiteralPath $checkOut) -or (Get-Item -LiteralPath $checkOut).Length -eq 0) {
                    $accepted = "the checker wrote no evidence"
                } else {
                    $accepted = Test-RealAcceptance -Checks @(Get-Content -LiteralPath $checkOut)
                }
            }
        }
    }
    } catch {
        $accepted = "the run raised: $($_.Exception.Message)"
    }

    Complete-RunRecord -Path $runRecord -Outcome $(if ($accepted -eq "") { "accepted" } else { "refused: $accepted" })
    if ($accepted -eq "") {
        Write-Host "PASS: $DesignName accepted (the full verdict set passes; pins recorded in $runRecord)"
        return 0
    }
    Write-Host "FAILED: $accepted"
    return 2
}

if ($MyInvocation.InvocationName -ne '.') {
    try {
        $code = Invoke-RealDesignRun -DesignName $Design -Root $RepoRoot
        exit $code
    } catch {
        $msg = $_.Exception.Message
        Write-Host $msg
        if ($msg -like "refusing:*") { exit 4 }
        exit 5
    }
}
