# Bounded direct children. A timeout never grants authority over descendants.
if (!(Get-Variable LiveBoundedUncertainChild -Scope Script -ErrorAction SilentlyContinue)) {$script:LiveBoundedUncertainChild=$false}
function New-LiveBoundedProcess($StartInfo) {
    $process=[Diagnostics.Process]::new();$process.StartInfo=$StartInfo;return $process
}
function Get-LiveChildReceiptRoot {
    $root=$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT
    if (!$root) { return $null }
    if (![IO.Path]::IsPathFullyQualified($root)) { throw 'fully qualified child receipt root required' }
    $root=[IO.Path]::GetFullPath($root).TrimEnd('\')
    if ($root -cnotin @('F:\ibcmd\lab\05\wave3\load\compact9-child-receipts')) { throw 'unknown child receipt context' }
    for($probe=$root;$probe;$probe=[IO.Path]::GetDirectoryName($probe)) {
        if ((Test-Path -LiteralPath $probe) -and ((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'child receipt reparse ancestry' }
    }
    if (!(Test-Path -LiteralPath $root -PathType Container)) { throw 'child receipt directory missing' }
    return $root
}
function Save-LiveUncertainChild($State) {
    $root=Get-LiveChildReceiptRoot
    if (!$root) { return }
    $bytes=[Text.Encoding]::UTF8.GetBytes(($State|ConvertTo-Json -Depth 4 -Compress))
    if ($bytes.Length -gt 8192) { throw 'child receipt exceeds budget' }
    $file=[IO.File]::Open((Join-Path $root ('unknown-child-'+[guid]::NewGuid().ToString('N')+'.json')),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
    try {$file.Write($bytes);$file.Flush($true)} finally {$file.Dispose()}
}
function Test-LiveUncertainChild {
    if ($script:LiveBoundedUncertainChild) { return $true }
    $root=Get-LiveChildReceiptRoot
    if (!$root) { return $false }
    # Receipts are immutable unresolved failures. Only an explicit subsequent
    # owned-child exit/cleanup proof may release a retained lifecycle lease.
    return @(Get-ChildItem -LiteralPath $root -File).Count -gt 0
}
function Invoke-LiveBounded {
    param([string]$Executable, [string[]]$Arguments, [int]$TimeoutSeconds = 30)
    if ($TimeoutSeconds -le 0) { throw 'positive child deadline required' }
    [void](Get-LiveChildReceiptRoot)
    $psi = [Diagnostics.ProcessStartInfo]::new($Executable)
    $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true; $psi.RedirectStandardInput = $true
    foreach ($arg in $Arguments) { $psi.ArgumentList.Add($arg) }
    $p=New-LiveBoundedProcess $psi;$started=$false;$exitProved=$false;$spawn=$null;$timedOut=$false;$reapAttempted=$false;$pipesUnproved=$false
    $state=[ordered]@{execution_state='not_started';requested_executable=$Executable;arguments_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($Arguments|ConvertTo-Json -Compress))))}
    try {
        Assert-Compact9RuntimeClosure
        if (-not $p.Start()) { throw 'child process failed to start' }
        $started=$true;$state.pid=$p.Id;$state.handle_start_utc=$p.StartTime.ToUniversalTime().ToString('o')
        $pipesUnproved=$true;$stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync()
        $p.StandardInput.Close()
        # The original live process handle prevents PID reuse during initial
        # capture. Rapid exits need no signalling authority and still return.
        if (!$p.HasExited) {
            $candidate=Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)"
            if (!$p.HasExited) {
                if (!$candidate -or $candidate.ProcessId -ne $p.Id -or $candidate.ParentProcessId -ne $PID -or !$candidate.CreationDate -or !$candidate.CommandLine -or $candidate.ExecutablePath -cne $p.MainModule.FileName) { throw 'exact direct child identity unavailable; no signal permitted' }
                $spawn=$candidate;$state.born=$spawn.CreationDate.ToUniversalTime().ToString('o');$state.executable=$spawn.ExecutablePath;$state.parent_pid=$spawn.ParentProcessId;$state.command_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($spawn.CommandLine)))
            }
        }
        if (!$p.WaitForExit($TimeoutSeconds * 1000)) {
            $timedOut=$true
            if (!$p.HasExited) {
                $current=Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)"
                if (!$spawn -or !$current -or $current.ProcessId -ne $spawn.ProcessId -or $current.CreationDate.ToUniversalTime().Ticks -ne $spawn.CreationDate.ToUniversalTime().Ticks -or $current.ExecutablePath -cne $spawn.ExecutablePath -or $current.CommandLine -cne $spawn.CommandLine -or $current.ParentProcessId -ne $spawn.ParentProcessId) { throw 'direct child identity changed; no signal permitted' }
                $p.Kill() # Exactly this handle; never a process tree.
            }
            $reapAttempted=$true;$exitProved=$p.WaitForExit(5000)
            if (!$exitProved) { throw 'direct child reap exceeded5000ms; execution unresolved' }
            throw "owned direct child exceeded $TimeoutSeconds seconds: $Executable"
        }
        $exitProved=$true
        if (!$stdout.Wait(5000) -or !$stderr.Wait(5000)) { throw 'direct child output pipes exceeded5000ms' }
        $pipesUnproved=$false
        [pscustomobject]@{ExitCode=$p.ExitCode;Stdout=$stdout.GetAwaiter().GetResult();Stderr=$stderr.GetAwaiter().GetResult()}
    } catch {
        $state.failure=$_.Exception.Message
        throw
    } finally {
        if ($started -and !$exitProved -and !$reapAttempted -and $p.HasExited) { $exitProved=$p.WaitForExit(5000) }
        if ($started -and (!$exitProved -or $timedOut -or $pipesUnproved)) {
            # A direct handle exit does not prove that a timed-out wrapper left
            # no running descendants. Retain the lifecycle; never signal a tree.
            $script:LiveBoundedUncertainChild=$true;$state.execution_state=$(if(!$exitProved){'unknown_running'}elseif($timedOut){'timeout_descendants_unproved'}else{'pipe_timeout_descendants_unproved'});$state.direct_child_exit_proved=$exitProved;$state.captured_utc=[DateTime]::UtcNow.ToString('o')
            try {Save-LiveUncertainChild $state} catch {$state.receipt_error='publication refused; retain leases and original failure'}
            $p.Dispose()
            throw ('LIVE_CHILD_EXECUTION_UNCERTAIN '+($state|ConvertTo-Json -Depth 4 -Compress))
        }
        $p.Dispose()
    }
}
function Invoke-LiveRac {
    param([string]$Executable, [string[]]$Arguments)
    $result = Invoke-LiveBounded -Executable $Executable -Arguments $Arguments -TimeoutSeconds 5
    if ($result.ExitCode -ne 0) { throw "rac failed: $($result.Stderr)" }
    $result.Stdout -split '\r?\n'
}

. 'F:/ibcmd/lab/05/wave3/load/compact9-runtime-v1/compact9_locks.ps1'

. 'F:/ibcmd/lab/05/wave3/load/compact9-runtime-v1/runtime_child.ps1'
