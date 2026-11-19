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
foreach ($t in @("seat-run-real.ps1", "seat-run.ps1", "run-macro.vbs", "check-part.vbs", "check-part-logic.vbs", "mass-adapter.ps1")) {
    Set-Content -Path (Join-Path $scratch "tools\$t") -Value "synthetic tooling bytes"
}
git -C $scratch add -- "uploads/d1/tracked.bin" 2>$null | Out-Null

function Sha($p) { (Get-FileHash -Algorithm SHA256 -LiteralPath $p).Hash.ToLower() }
$allTools = @("tools/seat-run-real.ps1", "tools/seat-run.ps1", "tools/run-macro.vbs", "tools/check-part.vbs", "tools/check-part-logic.vbs", "tools/mass-adapter.ps1")

# A manifest with one EXTRA pinned file (a consumed fallback sidecar).
function WriteManifestExtra($approved, $toolsToPin, $extraPath, $extraSha) {
    $tooling = [ordered]@{}
    foreach ($t in $toolsToPin) { $tooling[$t] = Sha (Join-Path $scratch "tools\$($t -replace 'tools/', '')") }
    $files = [ordered]@{
        source = [ordered]@{ path = "uploads/d1/source.json"; sha256 = (Sha $src) }
        macro = [ordered]@{ path = "uploads/d1/macro.swb"; sha256 = (Sha $macro) }
        reference = [ordered]@{ path = "uploads/d1/ref.step"; sha256 = (Sha $ref) }
        expect = [ordered]@{ path = "uploads/d1/expect.txt"; sha256 = (Sha $exp) }
        "sidecar:$extraPath" = [ordered]@{ path = $extraPath; sha256 = $extraSha }
    }
    $m = [ordered]@{ design = "d1"; files = $files; tooling = $tooling }
    if ($approved) { $m["approved_macro_sha256"] = $approved }
    $m | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $scratch "runs\d1\manifest.json")
    return Read-RealManifest -Path (Join-Path $scratch "runs\d1\manifest.json")
}

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

# ---- acceptance: real designs REQUIRE the full verdict set, passing ----
$fullPass = @("check:mass=pass", "check:mass_vs_reference=pass", "check:step_reference=pass", "check:bodies=pass", "check:placement=pass", "check:ref_close=pass")
Check "the full passing verdict set is accepted" ((Test-RealAcceptance -Checks $fullPass) -eq "") "reason given"
Check "mass_vs_reference=skipped is refused (real designs always compare)" ((Test-RealAcceptance -Checks (@($fullPass | Where-Object { $_ -ne "check:mass_vs_reference=pass" }) + @("check:mass_vs_reference=skipped"))) -ne "") "accepted"
Check "mass_vs_reference=fail is refused" ((Test-RealAcceptance -Checks (@($fullPass | Where-Object { $_ -ne "check:mass_vs_reference=pass" }) + @("check:mass_vs_reference=fail"))) -ne "") "accepted"
Check "a missing companion verdict is refused" ((Test-RealAcceptance -Checks @("check:mass=pass", "check:mass_vs_reference=pass")) -ne "") "accepted"
Check "a duplicate verdict is refused" ((Test-RealAcceptance -Checks ($fullPass + @("check:mass=pass"))) -ne "") "accepted"
Check "a non-passing companion verdict is refused" ((Test-RealAcceptance -Checks (@($fullPass | Where-Object { $_ -ne "check:placement=pass" }) + @("check:placement=fail"))) -ne "") "accepted"
Check "a failed check elsewhere still refuses" ((Test-RealAcceptance -Checks ($fullPass + @("check:tree_count=fail"))) -ne "") "accepted"

# ---- the runtime report gate (finding 7) ----
Check "a complete ok report passes" ((Test-RealReport -Lines @('"command":"x"', '{"type":"summary","status":"ok"}')) -eq "") "reason given"
Check "a failed step refuses the run" ((Test-RealReport -Lines @('{"type":"step","status":"failed"}', '{"type":"summary","status":"ok"}')) -ne "") "accepted"
Check "a failed summary refuses the run" ((Test-RealReport -Lines @('"command":"x"', '{"type":"summary","status":"failed"}')) -ne "") "accepted"
Check "a truncated report refuses the run" ((Test-RealReport -Lines @('"command":"x"')) -ne "") "accepted"
Check "an empty report refuses the run" ((Test-RealReport -Lines @()) -ne "") "accepted"

# ---- tooling pins: the executed bytes, this runner included (finding 5) ----
$scriptDir = Join-Path $scratch "tools"
$m = WriteManifest (Sha $macro) @("tools/seat-run.ps1", "tools/run-macro.vbs", "tools/check-part.vbs", "tools/check-part-logic.vbs") $false $false
$threw = Throws { Test-RealToolingPins -Manifest $m -ScriptDir $scriptDir }
Check "unpinned tooling is refused (this runner included)" $threw "no throw"
$m = WriteManifest (Sha $macro) $allTools $false $false
$pins = Test-RealToolingPins -Manifest $m -ScriptDir $scriptDir
Check "pinned tooling verifies against the executed bytes" ($pins.Count -eq 6) "count=$($pins.Count)"
# A pin that matches the repo copy but not the executed bytes refuses.
Set-Content -Path (Join-Path $scratch "tools\run-macro.vbs") -Value "synthetic tooling bytes, changed after pinning"
$threw = Throws { Test-RealToolingPins -Manifest $m -ScriptDir $scriptDir }
Check "changed executed bytes are refused" $threw "no throw"
Set-Content -Path (Join-Path $scratch "tools\run-macro.vbs") -Value "synthetic tooling bytes"

# ---- zone and sidecar provenance (finding 5) ----
Set-Content -Path (Join-Path $scratch "runs\d1\leaked.txt") -Value "tracked output"
git -C $scratch add -- "runs/d1/leaked.txt" 2>$null | Out-Null
$threw = Throws { Test-RealZoneUntracked -Root $scratch -RelDir "runs/d1" }
Check "a tracked file in the output zone is refused" $threw "no throw"
git -C $scratch rm --cached --quiet -- "runs/d1/leaked.txt" 2>$null | Out-Null
$ok = (Throws { Test-RealZoneUntracked -Root $scratch -RelDir "runs/d1" }) -eq $false
Check "an untracked output zone passes" $ok "threw"
$nogit = Join-Path $env:TEMP ("seat real nogit " + [Guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Path $nogit -Force | Out-Null
$threw = Throws { Test-RealUntracked -Root $nogit -RelPath "anything.bin" }
Check "an unestablishable untracked check fails closed" $threw "no throw"
Set-Content -Path (Join-Path $scratch "uploads\d1\fallback-step-0001.step") -Value "synthetic sidecar bytes"
$m = WriteManifest (Sha $macro) $allTools $false $false
$threw = Throws { Test-RealInputPins -Manifest $m -DesignName "d1" -Root $scratch }
Check "an unpinned consumed sidecar is refused" $threw "no throw"
$m = WriteManifestExtra (Sha $macro) $allTools "uploads/d1/fallback-step-0001.step" (Sha (Join-Path $scratch "uploads\d1\fallback-step-0001.step"))
$inputs = Test-RealInputPins -Manifest $m -DesignName "d1" -Root $scratch
Check "a pinned consumed sidecar verifies" ($inputs.Count -eq 5) "count=$($inputs.Count)"

# ---- durable per-run records (finding 6) ----
$rec1 = New-RunRecord -RunsDir (Join-Path $scratch "runs\d1") -DesignName "d1" -Manifest $m -MacroSha (Sha $macro) -Verified @{}
Start-Sleep -Seconds 1
$rec2 = New-RunRecord -RunsDir (Join-Path $scratch "runs\d1") -DesignName "d1" -Manifest $m -MacroSha (Sha $macro) -Verified @{}
Check "two runs get distinct record files" ($rec1 -ne $rec2) "rec1=$rec1 rec2=$rec2"
Complete-RunRecord -Path $rec1 -Outcome "accepted"
$r1 = Get-Content -LiteralPath $rec1 -Raw | ConvertFrom-Json
$r2 = Get-Content -LiteralPath $rec2 -Raw | ConvertFrom-Json
Check "the completed record carries its outcome" ($r1.outcome -eq "accepted") "outcome=$($r1.outcome)"
Check "the later run never overwrites earlier evidence" ($r2.outcome -eq "started") "outcome=$($r2.outcome)"
Check "the record pins the verified bytes" ($r1.PSObject.Properties["verified"] -ne $null) "no verified block"

if ($failures -gt 0) {
    Write-Host "$failures test(s) failed"
    exit 1
}
Write-Host "all seat-run-real tests passed"
exit 0
