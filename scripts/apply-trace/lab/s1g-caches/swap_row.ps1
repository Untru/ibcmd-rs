# Puts a stored (deflated) cache row into a trace-track lab database and gives it a new guid in `siVersions`,
# as the apply does for every row it rewrites.
#   pwsh -NoProfile -File swap_row.ps1 -Database ibcmd_rs_05_trace_x -Row c4629235-4823-4320-b8b5-1d08f4c6d612.si -Blob <file.deflated>
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Row,
    [Parameter(Mandatory = $true)][string]$Blob
)
$ErrorActionPreference = 'Stop'
$k = Split-Path -Parent $MyInvocation.MyCommand.Path
& pwsh -NoProfile -File "$k\dbrow.ps1" put -Database $Database -Name $Row -Blob $Blob
if ($LASTEXITCODE -ne 0) { throw 'put failed' }
$tmp = Join-Path $env:TEMP "siversions-$Database.bin"
& pwsh -NoProfile -File "$k\dbrow.ps1" dump -Database $Database -Name siVersions -Out $tmp
if ($LASTEXITCODE -ne 0) { throw 'dump failed' }
$text = [IO.File]::ReadAllText($tmp)
$guid = [guid]::NewGuid().ToString()
$pattern = '("' + [regex]::Escape($Row) + '",)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}'
$updated = [regex]::Replace($text, $pattern, ('${1}' + $guid))
if ($updated -eq $text) { throw "siVersions has no entry for $Row" }
[IO.File]::WriteAllText($tmp, $updated, [Text.UTF8Encoding]::new($true))
& pwsh -NoProfile -File "$k\dbrow.ps1" put -Database $Database -Name siVersions -Blob $tmp
if ($LASTEXITCODE -ne 0) { throw 'put siVersions failed' }
Remove-Item -LiteralPath $tmp -ErrorAction SilentlyContinue
"$Row -> new siVersions guid $guid"
