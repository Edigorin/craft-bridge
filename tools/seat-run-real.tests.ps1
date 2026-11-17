# seat-run-real.tests.ps1 - synthetic tests for tools/seat-run-real.ps1's
# provenance gates and for the Windows script surface. Runs on any Windows
# machine: no SolidWorks, no seat, no client data. Fixture manifests and
# fixture inputs are independently authored synthetic bytes - never
# anything from uploads/. Scratch files are left in place: the owner's
# seat runs no delete commands.
#
# Usage: powershell -ExecutionPolicy Bypass -File .\tools\seat-run-real.tests.ps1
# Exit codes: 0 all passed, 1 a case failed.

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "seat-run.ps1")
. (Join-Path $PSScriptRoot "seat-run-real.ps1")

$failures = 0
function Check($name, $condition, $detail) {
    if ($condition) {
        Write-Host "pass: $name"
    } else {
        $script:failures++
        Write-Host "FAIL: $name - $detail"
    }
}
function Throws([scriptblock]$body) {
    try { & $body | Out-Null; return $false } catch { return $true }
}

$tools = Split-Path -Parent $PSCommandPath

# ---- Windows script syntax checks ----
foreach ($ps1 in @("seat-run.ps1", "seat-run-real.ps1", "seat-harness.ps1", "seat-smoke.ps1", "seat-setup.ps1")) {
    $parseOk = $true
    try { [void][scriptblock]::Create((Get-Content -LiteralPath (Join-Path $tools $ps1) -Raw)) } catch { $parseOk = $false }
    Check "syntax: $ps1 parses" $parseOk "scriptblock create threw"
}
# check-part.vbs must PARSE and stop at its usage gate without a seat.
$r = Invoke-SeatCommand -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $tools "check-part.vbs")) -TimeoutSeconds 30
Check "syntax: check-part.vbs parses and stops at usage" (($r.ExitCode -eq 4) -and (-not $r.TimedOut)) "ExitCode=$($r.ExitCode)"
# The decision-logic driver runs fully offline and must pass.
$r = Invoke-SeatCommand -FilePath "cscript" -ArgumentList @("//nologo", (Join-Path $tools "check-part-logic.tests.vbs")) -TimeoutSeconds 60
Check "check-part-logic.tests.vbs passes" (($r.ExitCode -eq 0) -and (-not $r.TimedOut)) "ExitCode=$($r.ExitCode) Out='$($r.StdOut)'"

# ---- a synthetic scratch tree: inputs under uploads/, outputs under runs/
# (the gitignored zones), one TRACKED file for the hard-rule refusal ----
$scratch = Join-Path $env:TEMP ("seat real test " + [Guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Path (Join-Path $scratch "uploads\d1") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $scratch "runs\d1") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $scratch "tools") -Force | Out-Null
git -C $scratch init 2>$null | Out-Null
$macro = Join-Path $scratch "uploads\d1\macro.swb"
$src = Join-Path $scratch "uploads\d1\source.json"
$ref = Join-Path $scratch "uploads\d1\ref.step"
$exp = Join-Path $scratch "uploads\d1\expect.txt"
$tracked = Join-Path $scratch "uploads\d1\tracked.bin"
Set-Content -Path $macro -Value "synthetic macro bytes"
Set-Content -Path $src -Value "{""synthetic"": true}"
Set-Content -Path $ref -Value "synthetic step bytes"
Set-Content -Path $exp -Value "volume_tol=0.001"
Set-Content -Path $tracked -Value "synthetic tracked bytes"
foreach ($t in @("seat-run.ps1", "run-macro.vbs", "check-part.vbs", "check-part-logic.vbs")) {
    Set-Content -Path (Join-Path $scratch "tools\$t") -Value "synthetic tooling bytes"
}
git -C $scratch add -- "uploads/d1/tracked.bin" 2>$null | Out-Null

function Sha($p) { (Get-FileHash -Algorithm SHA256 -LiteralPath $p).Hash.ToLower() }
$allTools = @("tools/seat-run.ps1", "tools/run-macro.vbs", "tools/check-part.vbs", "tools/check-part-logic.vbs")

# Every manifest is written and READ BACK: the runner consumes parsed JSON
# (PSObject semantics), so the fixtures must too.
function WriteManifest($approved, $toolsToPin, $dropExpect, $trackedMacro) {
    $tooling = [ordered]@{}
    foreach ($t in $toolsToPin) { $tooling[$t] = Sha (Join-Path $scratch "tools\$($t -replace 'tools/', '')") }
    $files = [ordered]@{
        source = [ordered]@{ path = "uploads/d1/source.json"; sha256 = (Sha $src) }
        macro = [ordered]@{ path = "uploads/d1/macro.swb"; sha256 = (Sha $macro) }
        reference = [ordered]@{ path = "uploads/d1/ref.step"; sha256 = (Sha $ref) }
        expect = [ordered]@{ path = "uploads/d1/expect.txt"; sha256 = (Sha $exp) }
    }
    if ($dropExpect) { $files.Remove("expect") }
    if ($trackedMacro) { $files["macro"] = [ordered]@{ path = "uploads/d1/tracked.bin"; sha256 = (Sha $tracked) } }
    $m = [ordered]@{ design = "d1"; files = $files; tooling = $tooling }
    if ($approved) { $m["approved_macro_sha256"] = $approved }
    $m | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $scratch "runs\d1\manifest.json")
    return Read-RealManifest -Path (Join-Path $scratch "runs\d1\manifest.json")
}

# ---- manifest parsing: shape is validated at the tooling boundary ----
$threw = Throws { Read-RealManifest -Path (Join-Path $scratch "runs\d1\no-such-manifest.json") }
Check "a missing manifest is refused" $threw "no throw"
Set-Content -Path (Join-Path $scratch "runs\d1\manifest.json") -Value "{not json"
$threw = Throws { Read-RealManifest -Path (Join-Path $scratch "runs\d1\manifest.json") }
Check "malformed manifest JSON is refused" $threw "no throw"
$m = WriteManifest $null $allTools $false $false
Check "a complete manifest parses" (($m.design -eq "d1") -and ($m.files.macro.path -eq "uploads/d1/macro.swb")) "design=$($m.design)"
$threw = Throws { WriteManifest $null $allTools $true $false }
Check "an unpinned input is refused" $threw "no throw"

# ---- path confinement (hostile input) ----
$threw = Throws { Test-RealInputPath -DesignName "d1" -RelPath (Join-Path $env:TEMP "outside.txt") -Root $scratch }
Check "an absolute input path is refused" $threw "no throw"
$threw = Throws { Test-RealInputPath -DesignName "d1" -RelPath "uploads/d1/../../secret.txt" -Root $scratch }
Check "a traversal input path is refused" $threw "no throw"
$threw = Throws { Test-RealInputPath -DesignName "d1" -RelPath "runs/d1/manifest.json" -Root $scratch }
Check "an input outside uploads/<design> is refused" $threw "no throw"
$p = Test-RealInputPath -DesignName "d1" -RelPath "uploads/d1/macro.swb" -Root $scratch
Check "a confined input path resolves" ($p -eq (Join-Path $scratch "uploads\d1\macro.swb")) "resolved='$p'"

# ---- pinning ----
$threw = Throws { Test-RealPin -ExpectedSha (Sha $src) -Path $macro -What "macro" }
Check "a pin mismatch is refused" $threw "no throw"
$got = Test-RealPin -ExpectedSha (Sha $macro) -Path $macro -What "macro"
Check "a matching pin verifies" ($got -eq (Sha $macro)) "got='$got'"
$threw = Throws { Test-RealPin -ExpectedSha (Sha $macro) -Path (Join-Path $scratch "uploads\d1\gone.bin") -What "gone" }
Check "a pinned but missing file is refused" $threw "no throw"

# ---- the hard rule: real-part artifacts stay untracked ----
$threw = Throws { Test-RealUntracked -Root $scratch -RelPath "uploads/d1/tracked.bin" }
Check "a tracked artifact is refused" $threw "no throw"
$ok = (Throws { Test-RealUntracked -Root $scratch -RelPath "uploads/d1/macro.swb" }) -eq $false
Check "an untracked artifact passes" $ok "threw"
$m = WriteManifest $null $allTools $false $true
$threw = Throws { Test-RealInputPins -Manifest $m -DesignName "d1" -Root $scratch }
Check "a tracked pinned input is refused end to end" $threw "no throw"

# ---- the approval gate: first run, same hash, changed hash ----
$m = WriteManifest $null $allTools $false $false
$threw = Throws { Test-RealApproval -Manifest $m -MacroSha (Sha $macro) }
Check "a first run without owner approval is refused" $threw "no throw"
$m = WriteManifest (Sha $macro) $allTools $false $false
$ok = (Throws { Test-RealApproval -Manifest $m -MacroSha (Sha $macro) }) -eq $false
Check "an approved unchanged macro proceeds" $ok "threw"
$threw = Throws { Test-RealApproval -Manifest $m -MacroSha (Sha $src) }
Check "a changed macro needs re-approval" $threw "no throw"

# ---- acceptance: real designs REQUIRE mass_vs_reference=pass ----
$ok = (Test-RealAcceptance -Checks @("check:mass_vs_reference=pass", "check:bodies=pass")) -eq ""
Check "mass_vs_reference=pass is accepted" $ok "reason given"
Check "mass_vs_reference=skipped is refused (real designs always compare)" ((Test-RealAcceptance -Checks @("check:mass_vs_reference=skipped")) -ne "") "accepted"
Check "mass_vs_reference=fail is refused" ((Test-RealAcceptance -Checks @("check:mass_vs_reference=fail")) -ne "") "accepted"
Check "an absent mass entry is refused" ((Test-RealAcceptance -Checks @("check:tree_count=pass")) -ne "") "accepted"
Check "a failed companion check is refused" ((Test-RealAcceptance -Checks @("check:mass_vs_reference=pass", "check:placement=fail")) -ne "") "accepted"

# ---- tooling pins: anything not pinned is refused ----
$m = WriteManifest (Sha $macro) @("tools/seat-run.ps1", "tools/run-macro.vbs", "tools/check-part.vbs") $false $false
$threw = Throws { Test-RealToolingPins -Manifest $m -Root $scratch }
Check "unpinned tooling is refused" $threw "no throw"
$m = WriteManifest (Sha $macro) $allTools $false $false
$pins = Test-RealToolingPins -Manifest $m -Root $scratch
Check "pinned tooling verifies" ($pins.Count -eq 4) "count=$($pins.Count)"

if ($failures -gt 0) {
    Write-Host "$failures test(s) failed"
    exit 1
}
Write-Host "all seat-run-real tests passed"
exit 0
