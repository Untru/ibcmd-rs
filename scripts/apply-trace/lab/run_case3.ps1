# Case 3 "new database", steps 2..5 (step 1, native infobase create, was captured earlier):
#   2. our `infobase config import` of the whole reference tree into the created infobase (captured)
#   3. native `infobase config apply --force --dynamic=disable` (captured, heavy lock; retried once when it fails
#      with the known first-apply SDBL error on a fresh БСП database, the retry captured too)
#   4. native `infobase config export` into a fresh folder
#   5. ibcmd-rs source-diff against the reference tree
param(
    [string]$Database = 'ibcmd_rs_04_trace_c3_new',
    [string]$Platform = '8.3.27.2214',
    [string]$Ref = 'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native',
    [string]$User = 'Администратор',
    [string]$From = 'import'    # import | apply | export
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$kit = 'F:\ibcmd\src\ibcmd-rs-m-simple\scripts\apply-trace'
$lab = 'F:\ibcmd\lab\04\trace'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ours = 'F:\ibcmd\src\ibcmd-rs-m-simple\target\iter\ibcmd-rs.exe'
$ibcmdExe = "C:\Program Files\1cv8\$Platform\bin\ibcmd.exe"
$data = "$lab\ibdata\$Database"
New-Item -ItemType Directory -Force -Path $data | Out-Null
$common = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$data")
$capBase = @{ Database = $Database; OutRoot = "$lab\captures"; BlobStore = "$lab\blobs"; Track = 'trace' }
function Log($m) { Write-Host ('[{0}] {1}' -f (Get-Date -Format 'HH:mm:ss'), $m) }

$last = $null
if ($From -eq 'import') {
    Log 'step 2: our import of the whole tree'
    $rep = "$lab\captures\c3_ours_import_report.json"
    # our import uses 16 worker threads: heavy lock (the native lock is for native commands only)
    & pwsh -NoProfile -File $lock acquire trace -TimeoutMin 120
    if ($LASTEXITCODE -ne 0) { throw 'heavy lock not acquired' }
    try {
        $last = & (Join-Path $kit 'capture.ps1') @capBase -Tag c3-ours-import -TimeoutMinutes 30 `
            -Note 'case 3: our infobase config import of the whole reference tree into the freshly created (empty) infobase; base-free by itself' `
            -Exe $ours -ArgumentList (@('infobase', 'config', 'import') + $common + @('--platform=8.3.27', "--report=$rep", $Ref))
    } finally {
        & pwsh -NoProfile -File $lock release trace
    }
    Log "import capture: $last"
}
if ($From -in 'import', 'apply') {
    Log 'step 3: native apply (heavy lock)'
    & pwsh -NoProfile -File $lock acquire trace -TimeoutMin 120
    if ($LASTEXITCODE -ne 0) { throw 'heavy lock not acquired' }
    try {
        # a new infobase has no users: no --user (native ibcmd only prompts when users exist)
        $ap = @('infobase', 'config', 'apply') + $common + @('--force', '--dynamic=disable')
        # the native lock only for the native command itself (README, "Heavy operations and native writes")
        $nativeBefore = { & pwsh -NoProfile -File $lock acquire trace -Name native -TimeoutMin 120; if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' } }
        $nativeAfter = { & pwsh -NoProfile -File $lock release trace -Name native }
        $cap = & (Join-Path $kit 'capture.ps1') @capBase -Tag c3-native-apply -TimeoutMinutes 60 -BeforeCommand $nativeBefore -AfterCommand $nativeAfter `
            -Note 'case 3: native apply --dynamic=disable of the whole imported tree in the new infobase' -Exe $ibcmdExe -ArgumentList $ap
        $capDir = ($cap | Select-Object -Last 1)
        $meta = Get-Content -LiteralPath (Join-Path $capDir 'trace\trace-meta.json') -Raw | ConvertFrom-Json
        Log "apply exit code $($meta.exit_code) ($($meta.command_seconds) s)"
        if ($meta.exit_code -ne 0) {
            Log 'apply failed: one retry (same command, state kept)'
            $cap2 = & (Join-Path $kit 'capture.ps1') @capBase -Tag c3-native-apply-retry -TimeoutMinutes 60 -BeforeSnapshot (Join-Path $capDir 'after') -BeforeCommand $nativeBefore -AfterCommand $nativeAfter `
                -Note 'case 3: the retry of the native apply after the first apply failed' -Exe $ibcmdExe -ArgumentList $ap
            $capDir2 = ($cap2 | Select-Object -Last 1)
            Log "retry capture: $capDir2"
        }
    } finally {
        & pwsh -NoProfile -File $lock release trace
    }
}
Log 'step 4: native export'
$exp = "$lab\export\$Database"
if (Test-Path -LiteralPath $exp) { Remove-Item -Recurse -Force -LiteralPath $exp }
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $exp) | Out-Null
$sw = [Diagnostics.Stopwatch]::StartNew()
& $ibcmdExe infobase config export @common $exp *>&1 | Out-File -Encoding utf8 -LiteralPath "$lab\captures\c3_native_export.log"
Log "export exit $LASTEXITCODE in $([Math]::Round($sw.Elapsed.TotalSeconds, 1)) s"
Log 'step 5: source-diff against the reference tree'
& $ours source-diff -o "$lab\captures\c3_source_diff.json" $Ref $exp *>&1 | Out-File -Encoding utf8 -LiteralPath "$lab\captures\c3_source_diff.log"
Log "source-diff exit $LASTEXITCODE"
