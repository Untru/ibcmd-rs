# The acceptance of #393 on the merged build: OUR import (drop-in `infobase config import`, default flags) of a tree
# into a fresh clone, OUR apply (drop-in `infobase config apply`), and the platform's apply of the same stage on a twin.
#   pwsh -NoProfile -File final_acceptance.ps1 -Exe <ibcmd-rs.exe> -Tree <dir> -Suffix <name>
# Databases ibcmd_rs_04_apply_<Suffix>o (ours) and ibcmd_rs_04_apply_<Suffix>n (native twin of the same stage).
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Tree,
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-z0-9]{2,8}$')][string]$Suffix
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\apply'
$ours = "ibcmd_rs_04_apply_${Suffix}o_20260930"
$native = "ibcmd_rs_04_apply_${Suffix}n_20260930"
$log = "$lab\logs\final_$Suffix.log"
function Say($text) { $line = "$(Get-Date -Format s) $text"; $line | Tee-Object -FilePath $log -Append }
$env:DDL_LAB = "$lab\ddlkit"; $env:PYTHONIOENCODING = 'utf-8'
$kit = (Resolve-Path (Join-Path $PSScriptRoot '..\restructure-lab')).Path

Say "restore $ours"
& pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bsp8327 -Name $ours -Track apply -Purpose "#393 final acceptance, ours" 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
$data = "$lab\ibdata\$ours"; New-Item -ItemType Directory -Force $data | Out-Null
$sw = [Diagnostics.Stopwatch]::StartNew()
$o = & $Exe infobase config import --dbms=MSSQLServer --db-server=localhost --db-name=$ours --data=$data --report="$lab\out\import-final-$Suffix.json" $Tree 2>&1
Say "our import: exit $LASTEXITCODE, $([math]::Round($sw.Elapsed.TotalSeconds,1)) s ; $($o | Select-Object -Last 1)"
if ($LASTEXITCODE -ne 0) { exit 3 }
Push-Location $kit; python snapshot.py $ours staged 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }; Pop-Location
sqlcmd -S localhost -E -C -b -Q "BACKUP DATABASE [$ours] TO DISK = N'$lab\bak\final_${Suffix}_staged.bak' WITH COPY_ONLY, COMPRESSION, INIT" | Select-Object -Last 1 | ForEach-Object { Say $_ }
Say "restore $native (the same stage)"
& pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak "$lab\bak\final_${Suffix}_staged.bak" -Name $native -Track apply -Purpose "#393 final acceptance, native twin" 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }

$sw.Restart()
$o = & $Exe infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=$ours --data=$data --force --dynamic=disable 2>&1
Say "our apply (drop-in): exit $LASTEXITCODE, $([math]::Round($sw.Elapsed.TotalSeconds,1)) s ; $(($o | Select-Object -Last 3) -join ' | ')"
Push-Location $kit; python snapshot.py $ours own_after 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }; Pop-Location

$r = & pwsh -NoProfile -File "$PSScriptRoot\native_cmd.ps1" -Database $native -Tag "final_${Suffix}_apply" -Command "infobase config apply --force --dynamic=disable" -TimeoutSec 3000 -LockTimeoutMin 180 2>&1
Say "native apply on the twin: $($r | Select-Object -First 1)"
Push-Location $kit; python snapshot.py $native nat_after 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }; Pop-Location

$r = & pwsh -NoProfile -File "$PSScriptRoot\native_cmd.ps1" -Database $ours -Tag "final_${Suffix}_reapply" -Command "infobase config apply --force --dynamic=disable" -TimeoutSec 3000 -LockTimeoutMin 180 2>&1
Say "native apply again on our result: $(($r | Select-Object -Skip 1 -First 3) -join ' | ')"
$export = "$lab\out\export\final_$Suffix"
if (-not (Test-Path $export)) { & pwsh -NoProfile -File "$PSScriptRoot\native_cmd.ps1" -Database $ours -Tag "final_${Suffix}_export" -Command "infobase config export $($export -replace '\\','/')" -TimeoutSec 3000 2>&1 | Select-Object -First 1 | ForEach-Object { Say "native export: $_" } }
& $Exe source-diff -o "$export.diff.json" $Tree $export 2>&1 | Out-Null
$d = Get-Content "$export.diff.json" -Raw -Encoding UTF8 | ConvertFrom-Json
Say ("export against the tree: left_only {0}, right_only {1}, different {2} ({3}), unchanged {4}" -f $d.summary.left_only, $d.summary.right_only, $d.summary.different, (($d.differences | Where-Object { $_.status -ne 'unchanged' } | ForEach-Object { $_.path }) -join ', '), $d.summary.unchanged)
Push-Location $kit
Say 'snapdiff native -> ours (Config, Params, tables):'
python snapdiff.py $native nat_after $ours own_after --max 8 2>&1 | Where-Object { $_ -match '^(==|added|removed|changed|  |Config|Params|ConfigSave|\(|DBSchema)' -and $_ -notmatch '^    [mM+-] ' } | ForEach-Object { Say $_ }
python si_diff.py $native nat_after $ours own_after --max-lines 2 2>&1 | Select-Object -Last 2 | ForEach-Object { Say $_ }
Pop-Location
Say 'done'
