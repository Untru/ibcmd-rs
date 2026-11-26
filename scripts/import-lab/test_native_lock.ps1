# No SQL or 1C commands: verify the native wrapper executes its body once, with/without heavy locking.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\native.ps1"
$temp = Join-Path $script:Lab ('lock-test-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temp | Out-Null
$script:Lock = Join-Path $temp 'fake-lock.ps1'
$env:IMPORT_LOCK_TEST_LOG = Join-Path $temp 'calls.txt'
@'
param($Action, $Track, $Name = 'heavy', $TimeoutMin)
$log = $env:IMPORT_LOCK_TEST_LOG
Add-Content -LiteralPath $log -Value "$Action $Name $Track"
if (@(Get-Content -LiteralPath $log).Count -gt 8) { exit 1 }
'@ | Set-Content -LiteralPath $script:Lock
try {
    foreach ($heavy in @($false, $true)) {
        if (Test-Path $env:IMPORT_LOCK_TEST_LOG) { Remove-Item -LiteralPath $env:IMPORT_LOCK_TEST_LOG }
        Use-HeavyLab $heavy
        $result = @{ calls = 0 }
        Invoke-WithNativeLock { $result.calls++ }
        if ($result.calls -ne 1) { throw "body ran $($result.calls) times" }
        $calls = @(Get-Content -LiteralPath $env:IMPORT_LOCK_TEST_LOG)
        $expected = if ($heavy) { @('acquire heavy import', 'acquire native import', 'release native import', 'release heavy import') }
                    else { @('acquire native import', 'release native import') }
        if (($calls -join '|') -ne ($expected -join '|')) { throw "wrong lock order: $calls" }
        Remove-Item -LiteralPath $env:IMPORT_LOCK_TEST_LOG
        $caught = $false
        try { Invoke-WithNativeLock { throw 'expected body failure' } }
        catch {
            if ($_.Exception.Message -ne 'expected body failure') { throw }
            $caught = $true
        }
        $calls = @(Get-Content -LiteralPath $env:IMPORT_LOCK_TEST_LOG)
        if (-not $caught -or ($calls -join '|') -ne ($expected -join '|')) { throw 'failure did not release both locks' }
    }
    'native lock harness: both paths passed'
} finally {
    Remove-Item Env:IMPORT_LOCK_TEST_LOG
    # The generated temporary directory is always beneath the import lab.
    if (([IO.Path]::GetFullPath($temp)).StartsWith([IO.Path]::GetFullPath($script:Lab) + '\')) {
        Remove-Item -LiteralPath $temp -Recurse -Force
    }
}
