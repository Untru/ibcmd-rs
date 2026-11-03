param(
    [string]$RepositoryRoot = (Join-Path $PSScriptRoot '..'),
    [string]$OutputDirectory,
    [switch]$SelfTest
)
$ErrorActionPreference = 'Stop'

function Get-RootLibTestExecutable($Messages) {
    $executables = @($Messages | ForEach-Object {
        $item = $_ | ConvertFrom-Json -ErrorAction Stop
        if ($item.reason -eq 'compiler-artifact' -and $item.profile.test -eq $true -and
            $item.target.name -eq 'ibcmd_rs' -and ($item.target.kind -contains 'lib') -and $item.executable) {
            $item.executable
        }
    })
    if ($executables.Count -ne 1) { throw 'expected exactly one root lib test executable' }
    return $executables[0]
}

function Get-TrackedSourceState {
    $commit = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'source commit lookup failed' }
    $tree = (& git rev-parse 'HEAD^{tree}').Trim()
    if ($LASTEXITCODE -ne 0) { throw 'source tree lookup failed' }
    & git diff --quiet --ignore-submodules=none --exit-code --
    $worktreeExit = $LASTEXITCODE
    & git diff --cached --quiet --ignore-submodules=none --exit-code --
    $indexExit = $LASTEXITCODE
    return [pscustomobject]@{ Commit = $commit; Tree = $tree; WorktreeExit = $worktreeExit; IndexExit = $indexExit }
}

function Assert-TrackedSourceState($Actual, $Expected) {
    if ($Actual.Commit -cnotmatch '^[a-f0-9]{40}$' -or $Actual.Tree -cnotmatch '^[a-f0-9]{40}$' -or
        $Actual.WorktreeExit -ne 0 -or $Actual.IndexExit -ne 0 -or
        $Actual.Commit -cne $Expected.Commit -or $Actual.Tree -cne $Expected.Tree) {
        throw 'Windows lib-test packaging requires unchanged HEAD and clean tracked worktree/index; commit changes before Windows packaging'
    }
}

function Assert-CopyHash([string]$SourceHash, [string]$CopyHash) {
    if ($SourceHash -cnotmatch '^[a-f0-9]{64}$' -or $SourceHash -cne $CopyHash) {
        throw 'packaged executable hash differs from source'
    }
}

function New-BuildReceipt($Commit, $Tree, $Rust, $Hash) {
    return [ordered]@{
        source_commit = $Commit
        source_tree = $Tree
        rustc = $Rust
        executable = 'ibcmd_rs-lib-test.exe'
        executable_sha256 = $Hash
        no_default_features = $true
        profile = 'test'
        test_filter = 'server::sql_import::transaction::tests::public_sql_held_capture_rolls_back_and_closes_dedicated_session'
        sql_executed_in_ci = $false
    }
}

function Assert-BuildReceipt($Receipt, $Commit, $Tree, $Rust, $Hash) {
    $expected = New-BuildReceipt $Commit $Tree $Rust $Hash
    $names = @($Receipt.PSObject.Properties.Name)
    if ($Commit -cnotmatch '^[a-f0-9]{40}$' -or $Tree -cnotmatch '^[a-f0-9]{40}$' -or
        $Hash -cnotmatch '^[a-f0-9]{64}$' -or $Rust -cnotmatch '^rustc .+' -or
        $names.Count -ne $expected.Count) { throw 'invalid build receipt' }
    foreach ($key in $expected.Keys) {
        $actual = $Receipt.$key
        if ($null -eq $actual -or $actual.GetType() -ne $expected[$key].GetType() -or
            $actual -cne $expected[$key]) { throw 'invalid build receipt' }
    }
}

function Assert-Rejected($Action, $Reason) {
    try { & $Action } catch {
        if ($_.Exception.Message -ceq $Reason) { return }
        throw
    }
    throw "expected refusal: $Reason"
}

function Invoke-SelfTest {
    $artifact = '{"reason":"compiler-artifact","profile":{"test":true},"target":{"name":"ibcmd_rs","kind":["lib"]},"executable":"synthetic.exe"}'
    if ((Get-RootLibTestExecutable @($artifact)) -cne 'synthetic.exe') { throw 'artifact selection failed' }
    $reason = 'expected exactly one root lib test executable'
    Assert-Rejected { Get-RootLibTestExecutable @() } $reason
    Assert-Rejected { Get-RootLibTestExecutable @($artifact, $artifact) } $reason
    Assert-Rejected { Get-RootLibTestExecutable @($artifact.Replace('"lib"', '"bin"')) } $reason
    Assert-Rejected { Get-RootLibTestExecutable @($artifact.Replace('"test":true', '"test":false')) } $reason
    $commit = 'a' * 40; $tree = 'b' * 40; $rust = 'rustc synthetic'; $hash = 'c' * 64
    $clean = [pscustomobject]@{ Commit = $commit; Tree = $tree; WorktreeExit = 0; IndexExit = 0 }
    Assert-TrackedSourceState $clean $clean
    foreach ($change in @(@{ WorktreeExit = 1 }, @{ IndexExit = 1 }, @{ Commit = ('d' * 40) }, @{ Tree = ('e' * 40) }, @{ WorktreeExit = 2 })) {
        $changed = $clean | ConvertTo-Json | ConvertFrom-Json
        foreach ($key in $change.Keys) { $changed.$key = $change[$key] }
        Assert-Rejected { Assert-TrackedSourceState $changed $clean } 'Windows lib-test packaging requires unchanged HEAD and clean tracked worktree/index; commit changes before Windows packaging'
    }
    $json = (New-BuildReceipt $commit $tree $rust $hash) | ConvertTo-Json
    Assert-BuildReceipt ($json | ConvertFrom-Json) $commit $tree $rust $hash
    foreach ($key in @('source_commit', 'source_tree', 'rustc', 'executable', 'executable_sha256', 'no_default_features', 'profile', 'test_filter', 'sql_executed_in_ci')) {
        $receipt = $json | ConvertFrom-Json
        $receipt.$key = if ($receipt.$key -is [bool]) { -not $receipt.$key } else { 'wrong' }
        Assert-Rejected { Assert-BuildReceipt $receipt $commit $tree $rust $hash } 'invalid build receipt'
    }
    $receipt = $json | ConvertFrom-Json; $receipt.sql_executed_in_ci = 'False'
    Assert-Rejected { Assert-BuildReceipt $receipt $commit $tree $rust $hash } 'invalid build receipt'
    $receipt = $json | ConvertFrom-Json; $receipt.PSObject.Properties.Remove('source_tree')
    Assert-Rejected { Assert-BuildReceipt $receipt $commit $tree $rust $hash } 'invalid build receipt'
    # Retain failed controls or an explicit KEEP1; clean only this fresh tiny fixture.
    $fixtureRoot = if ($OutputDirectory) { $OutputDirectory } else { [IO.Path]::GetTempPath() }
    $directory = Join-Path $fixtureRoot ('windows-lib-package-selftest-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $directory | Out-Null
    $source = Join-Path $directory 'source.exe'; $copy = Join-Path $directory 'copy.exe'
    $passed = $false
    try {
        [IO.File]::WriteAllBytes($source, [byte[]]@(1, 2, 3, 4))
        Copy-Item -LiteralPath $source -Destination $copy
        $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()
        Assert-CopyHash $sourceHash (Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash.ToLowerInvariant()
        [IO.File]::WriteAllBytes($copy, [byte[]]@(1, 2, 3, 5))
        Assert-Rejected { Assert-CopyHash $sourceHash (Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash.ToLowerInvariant() } 'packaged executable hash differs from source'
        $passed = $true
    } finally {
        if ($passed -and $env:KEEP1 -cne '1') {
            Remove-Item -LiteralPath $source, $copy
            # No recursive removal: unexpected contents stay available for inspection.
            Remove-Item -LiteralPath $directory
        } else {
            Write-Host "Retained packaging self-test fixture: $directory"
        }
    }
    Write-Host 'Windows root lib packaging self-tests passed.'
}

if ($SelfTest) { Invoke-SelfTest; return }
if (-not $IsWindows) { throw 'Windows root lib packaging requires Windows' }
Push-Location $RepositoryRoot
try {
    $sourceState = Get-TrackedSourceState
    Assert-TrackedSourceState $sourceState $sourceState
    $env:CARGO_NET_OFFLINE = 'true'
    $messages = @(& cargo test --locked -p ibcmd-rs --lib --no-default-features --no-run --message-format=json)
    if ($LASTEXITCODE -ne 0) { throw 'lib test compilation failed' }
    $source = Get-RootLibTestExecutable $messages
    $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()
    $commit = $sourceState.Commit; $tree = $sourceState.Tree
    $rust = (& rustc --version).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'compiler version lookup failed' }
    if (-not $OutputDirectory) {
        $OutputDirectory = Join-Path 'target/offline-evidence' ('windows-root-lib-test-' + [guid]::NewGuid().ToString('N'))
    }
    New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
    $executable = Join-Path $OutputDirectory 'ibcmd_rs-lib-test.exe'
    Copy-Item -LiteralPath $source -Destination $executable
    $hash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant()
    Assert-CopyHash $sourceHash $hash
    # Recheck the source too: a concurrent rebuild must not be silently packaged.
    Assert-CopyHash $sourceHash (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()
    $receipt = New-BuildReceipt $commit $tree $rust $hash
    $receiptPath = Join-Path $OutputDirectory 'build-receipt.json'
    $receipt | ConvertTo-Json | Set-Content -LiteralPath $receiptPath -Encoding utf8
    Assert-BuildReceipt (Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json) $commit $tree $rust $hash
    Assert-TrackedSourceState (Get-TrackedSourceState) $sourceState
    Write-Host "Windows root lib test packaged: $OutputDirectory"
} finally { Pop-Location }
