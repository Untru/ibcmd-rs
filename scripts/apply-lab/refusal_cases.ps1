# Lists the own apply must refuse as a whole (#393), on a disposable clone that holds our removal stage
# (form ВсеЗаметки and template ДатыПасха removed): the original list of 11 names is changed, the apply is planned
# with --dry-run, the refusal is read, and the original list is put back.
#   pwsh -NoProfile -File refusal_cases.ps1 -Database ibcmd_rs_04_apply_fdr_20260930 -Exe <ibcmd-rs.exe>
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^ibcmd_rs_04_apply_')][string]$Database,
    [Parameter(Mandatory = $true)][string]$Exe
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONIOENCODING = 'utf-8'
$lab = 'F:\ibcmd\lab\04\apply'
$tools = $PSScriptRoot
$form = '8a7546f4-bfc9-4732-bf60-43a41e2c8753'
$template = '0384ec55-dc18-40ae-ae83-7defc28d8f3f'
$alias = 'ab132638-5188-470d-9432-de85f2b2c7d8_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b'
$alias2 = 'a627e390-8fad-4a95-afe6-674f54813188_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b'
$versions = 'versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b'
function Names($extra, $skip) {
    $all = @($template, "$template.0", $form, "$form.0", "$form.1", $alias, "$alias.0", $alias2, "$alias2.0", 'DynamicallyUpdated', $versions)
    $all = $all | Where-Object { $skip -notcontains $_ }
    $pairs = @($all | ForEach-Object { '"' + $_ + '",0' }) + @($extra)
    "$($pairs.Count)," + ($pairs -join ',')
}
$cases = [ordered]@{
    'control: the whole list of the native import'                       = @{ extra = @(); skip = @() }
    'an attribute id (flag 1) next to the form: no gate judges it'      = @{ extra = @('"c1a2b3d4-0000-4000-8000-000000000001",1'); skip = @() }
    'a body row of an object that stays (00149051-....0)'                = @{ extra = @('"00149051-cb5d-404b-9bc7-6347f3f29bf6.0",0'); skip = @() }
    'a body of the removed form left out of the list (.1)'              = @{ extra = @(); skip = @("$form.1") }
    'a row Config does not hold (.7)'                                    = @{ extra = @("`"$form.7`",0"); skip = @() }
    'only some of the rows of the dynamic update'                        = @{ extra = @(); skip = @($alias2, "$alias2.0") }
}
foreach ($case in $cases.Keys) {
    $text = Names $cases[$case].extra $cases[$case].skip
    & python "$PSScriptRoot\stage_tools.py" drop-deleted $Database | Out-Null
    & python "$PSScriptRoot\stage_tools.py" add-deleted $Database $text | Out-Null
    $out = & $Exe mssql-config-apply --platform-profile platform-8.3.27.2214 --database $Database --dry-run 2>&1
    $code = $LASTEXITCODE
    $line = ($out | Where-Object { $_ -match '"refused"|"reason"|"executed"|"dry_run"' } | Select-Object -First 2) -join ' '
    "{0}`n   exit={1} {2}" -f $case, $code, ($line -replace '\s+', ' ')
    if ($code -ne 0) { $reason = ($out | Where-Object { $_ -match '"reason"' } | Select-Object -First 1); "   " + ($reason -replace '\s+', ' ').Trim() }
}
& python "$PSScriptRoot\stage_tools.py" drop-deleted $Database | Out-Null
& python "$PSScriptRoot\stage_tools.py" add-deleted $Database (Names @() @()) | Out-Null
'original list put back'
