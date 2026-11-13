# seat-run.ps1 - run one console command and capture its TRUE exit code.
# Dot-source this from the seat scripts; it never touches SolidWorks.
#
# Why this exists: Windows PowerShell 5.1's Start-Process -PassThru can
# surface a $null ExitCode after WaitForExit(ms), which silently turned the
# exit-code gate into a coin flip (the 2026-11 seat run failed six items on
# "exited " with an empty code). This starts the process directly through
# System.Diagnostics.Process, drains both output streams concurrently, and
# returns a record with ExitCode, TimedOut, StdOut and StdErr. A gate on
# ExitCode is only sound when an unreadable code is visible as failure.

function Quote-SeatArg {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value)
    # ProcessStartInfo.Arguments is one command-line string on .NET
    # Framework. Quote only when needed - cmd's own /c switch is happier
    # unquoted - and then the Windows way: double the backslashes that
    # precede an embedded quote, escape the quote, double trailing
    # backslashes before the closing quote.
    if ($Value -ne '' -and $Value -notmatch '[\s"]') { return $Value }
    $escaped = $Value -replace '(\\*)"', '$1$1\"' -replace '(\\+)$', '$1$1'
    return '"' + $escaped + '"'
}

function Invoke-SeatCommand {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$ArgumentList,
        [int]$TimeoutSeconds = 300
    )
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath
    $psi.Arguments = (($ArgumentList | ForEach-Object { Quote-SeatArg $_ }) -join ' ')
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true

    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi
    $result = [ordered]@{ ExitCode = $null; TimedOut = $false; StdOut = ''; StdErr = '' }
    try {
        $null = $proc.Start()
        # Drain both streams concurrently: a chatty child must never block
        # on a full pipe while we wait for its exit.
        $outTask = $proc.StandardOutput.ReadToEndAsync()
        $errTask = $proc.StandardError.ReadToEndAsync()
        if (-not $proc.WaitForExit($TimeoutSeconds * 1000)) {
            # Killing the runner does NOT stop work already executing
            # inside SolidWorks; callers must say so and stop the run.
            $result.TimedOut = $true
            try { $proc.Kill() } catch { }
        } else {
            $proc.WaitForExit()   # flush async output processing
            $result.ExitCode = $proc.ExitCode
        }
        # Bounded collection: after a kill the pipes close at EOF, but
        # never trust that and block forever.
        if ($outTask.Wait(5000)) { $result.StdOut = $outTask.Result }
        if ($errTask.Wait(5000)) { $result.StdErr = $errTask.Result }
    } finally {
        $proc.Dispose()
    }
    return [pscustomobject]$result
}

function Move-SeatEvidenceAside {
    # Move a possibly-stale evidence file aside, never delete it: the owner's
    # seat runs no delete commands. The name is collision-resistant (UTC
    # timestamp plus a random fragment) or an explicit test suffix; nothing
    # is ever overwritten. Any failure throws, so a caller that archives
    # before running fails closed BEFORE the macro is invoked. Returns the
    # archive path, or $null when the source was absent.
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$Suffix = ""
    )
    if (-not (Test-Path -LiteralPath $Path)) { return $null }
    if ($Suffix -eq "") {
        $stamp = Get-Date -Format "yyyyMMddTHHmmss"
        $unique = [Guid]::NewGuid().ToString("N").Substring(0, 8)
        $Suffix = "$stamp-$unique"
    }
    $dest = "$Path.prev-$Suffix"
    if (Test-Path -LiteralPath $dest) {
        throw "refusing to overwrite existing archive: $dest"
    }
    Move-Item -LiteralPath $Path -Destination $dest
    if (Test-Path -LiteralPath $Path) {
        throw "failed to move evidence aside: $Path"
    }
    return $dest
}
