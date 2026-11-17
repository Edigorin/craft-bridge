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
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "seat-run.ps1")

# The four tooling scripts this runner executes: each must be pinned.
function Get-RequiredTooling { @("tools/seat-run.ps1", "tools/run-macro.vbs", "tools/check-part.vbs", "tools/check-part-logic.vbs") }

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
function Test-RealUntracked {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$RelPath
    )
    git -C $Root ls-files --error-unmatch -- $RelPath 2>$null | Out-Null
    if ($LASTEXITCODE -eq 0) {
        throw "refusing: $RelPath is a tracked file; real-part artifacts must stay untracked (AGENTS.md)"
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

# Real-design acceptance: mass_vs_reference=pass is REQUIRED - a real
# design always compares against its reference STEP (no placeholder) - and
# no check may fail. Returns "" when accepted, else the named reason.
function Test-RealAcceptance {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Checks)
    $mass = @($Checks | Where-Object { $_ -match '^check:mass_vs_reference=' })
    if ($mass.Count -eq 0) { return "the required check:mass_vs_reference entry is absent" }
    if ($mass.Count -gt 1) { return "the check:mass_vs_reference entry is not unique" }
    if ($mass[0] -ne "check:mass_vs_reference=pass") { return "mass_vs_reference is not pass ($($mass[0]))" }
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

# Every manifest input: confined to uploads/<design>/, untracked, and
# byte-identical to its pin. Returns the resolved full paths.
function Test-RealInputPins {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$DesignName,
        [Parameter(Mandatory = $true)][string]$Root
    )
    $inputs = @{}
    foreach ($what in @("source", "macro", "reference", "expect")) {
        $entry = $Manifest.files.$what
        $full = Test-RealInputPath -DesignName $DesignName -RelPath "$($entry.path)" -Root $Root
        $rel = "$($entry.path)" -replace "\\", "/"
        Test-RealUntracked -Root $Root -RelPath $rel
        $null = Test-RealPin -ExpectedSha "$($entry.sha256)" -Path $full -What $what
        $inputs[$what] = $full
    }
    return $inputs
}

# The tooling this runner executes must all be pinned; anything not pinned
# is refused. Returns the verified pins keyed by tooling path.
function Test-RealToolingPins {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$Root
    )
    $toolPins = @{}
    foreach ($tool in Get-RequiredTooling) {
        if (-not $Manifest.tooling.PSObject.Properties[$tool] -or -not $Manifest.tooling.$tool) {
            throw "refusing: the manifest does not pin $tool (anything not pinned is refused)"
        }
        $toolFull = [System.IO.Path]::GetFullPath((Join-Path $Root $tool))
        $toolPins[$tool] = Test-RealPin -ExpectedSha "$($Manifest.tooling.$tool)" -Path $toolFull -What "tooling $tool"
    }
    return $toolPins
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
    $manifest = Read-RealManifest -Path $manifestPath
    if ("$($manifest.design)" -ne $DesignName) {
        throw "refusing: the manifest design ($($manifest.design)) is not $DesignName"
    }

    # Inputs: confined to uploads/<design>/, untracked, byte-pinned.
    $inputs = Test-RealInputPins -Manifest $manifest -DesignName $DesignName -Root $Root

    # Tooling: every script this runner executes must be pinned and match.
    $toolPins = Test-RealToolingPins -Manifest $manifest -Root $Root

    # The approval gate: the exact macro bytes the owner approved.
    $macroSha = Get-RealFileSha256 -Path $inputs["macro"]
    Test-RealApproval -Manifest $manifest -MacroSha $macroSha

    # Everything this run writes stays under runs/<design>/ (untracked).
    New-Item -ItemType Directory -Force -Path $runsDir | Out-Null
    $reportPath = Join-Path $runsDir "runtime-report.jsonl"
    $checkOut = Join-Path $runsDir "checks.txt"
    Move-SeatEvidenceAside -Path $reportPath | Out-Null
    Move-SeatEvidenceAside -Path $checkOut | Out-Null

    # The per-run pin record the exception requires (untracked).
    $runManifest = [ordered]@{
        design = $DesignName
        verified = [ordered]@{
            source = [ordered]@{ path = $manifest.files.source.path; sha256 = (Get-RealFileSha256 -Path $inputs["source"]) }
            macro = [ordered]@{ path = $manifest.files.macro.path; sha256 = $macroSha }
            reference = [ordered]@{ path = $manifest.files.reference.path; sha256 = (Get-RealFileSha256 -Path $inputs["reference"]) }
            expect = [ordered]@{ path = $manifest.files.expect.path; sha256 = (Get-RealFileSha256 -Path $inputs["expect"]) }
            tooling = $toolPins
        }
        approved_macro_sha256 = "$($manifest.approved_macro_sha256)"
        outcome = "started"
    }

    # Run the macro exactly like the corpus harness does: the TEMP path
    # file protocol, the true exit code, a complete runtime report.
    $pathFile = Join-Path $env:TEMP "craft-bridge-report-path.txt"
    $env:CRAFT_BRIDGE_REPORT = $reportPath
    $macroDir = Split-Path -Parent $inputs["macro"]
    Set-Content -Path $pathFile -Value @($reportPath, $macroDir)
    $r = Invoke-SeatCommand -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $PSScriptRoot "run-macro.vbs"), $inputs["macro"]) -TimeoutSeconds 600
    $r.StdOut | Set-Content (Join-Path $runsDir "runner.log")
    $r.StdErr | Set-Content (Join-Path $runsDir "runner.err.log")
    $accepted = ""
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
        if ($lines.Count -eq 0 -or -not ($lines[-1] -match '"type":"summary"')) {
            $accepted = "truncated runtime report"
        } else {
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

    $runManifest.outcome = if ($accepted -eq "") { "accepted" } else { "refused: $accepted" }
    $runManifest | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $runsDir "run-manifest.json")
    if ($accepted -eq "") {
        Write-Host "PASS: $DesignName accepted (mass_vs_reference=pass; pins recorded in runs/$DesignName/run-manifest.json)"
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
