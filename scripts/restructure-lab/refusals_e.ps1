# 12.6 check 11 for S1-E on a real database: three edits of the staged rows of a FRESH twin of case e4 (the staged state, nothing applied),
# each followed by a dry run of `mssql-config-apply --allow-restructure s1`, which must refuse with the reason named, and put back.
#   pwsh -NoProfile -File refusals_e.ps1 -Database ibcmd_rs_04_ddl_s2_e4_ref [-Exe <ibcmd-rs.exe>] [-Out <file>]
# The rows: f6ed8faa-... is the catalog _ДемоПодразделения, b31bdfd7-... the document _ДемоОтпускаСотрудников of case e4.
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [string]$Exe = 'F:\ibcmd\lab\04\restructure\bin\ibcmd-rs-e.exe',
    [string]$Out = 'F:\ibcmd\lab\04\restructure\out\refusals_e4.txt'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
if ($Database -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only (got $Database)" }
$kit = $PSScriptRoot
$doc = 'b31bdfd7-8d3a-44cd-869e-0c75a62ab1d5'
$common = @('mssql-config-apply', '--platform-profile', 'platform-8.3.27.2214', '--database', $Database, '--allow-restructure', 's1', '--allow-non-lab', '--dry-run')
$nl = "`r`n"
$cases = @(
    @{ Name = 'a stored tabular section is renamed (its generated types change: the check refuses it)'; Row = $doc;
       Old = ('"Сотрудники",' + $nl + '{1,"ru","Сотрудники"}'); New = ('"СотрудникиПереименованы",' + $nl + '{1,"ru","Сотрудники"}'); Expect = 'tabular-section-outside-s1' },
    @{ Name = 'the attribute of a new tabular section gets a composite type (no field for it yet)'; Row = $doc;
       Old = ('"ДемоДата",' + $nl + '{1,"ru","ДемоДата"},"",0,0,00000000-0000-0000-0000-000000000000,0},' + $nl + '{"Pattern",' + $nl + '{"D","D"}');
       New = ('"ДемоДата",' + $nl + '{1,"ru","ДемоДата"},"",0,0,00000000-0000-0000-0000-000000000000,0},' + $nl + '{"Pattern",' + $nl + '{"D","D"},' + $nl + '{"S",10,1}');
       Expect = 'not mapped to fields yet' },
    @{ Name = 'the type of an attribute of a stored tabular section changes (the check refuses it)'; Row = $doc;
       Old = ('"КоличествоДней",' + $nl + '{1,"ru","Количество дней"},"",0,0,00000000-0000-0000-0000-000000000000,0},' + $nl + '{"Pattern",' + $nl + '{"N",3,0,1}');
       New = ('"КоличествоДней",' + $nl + '{1,"ru","Количество дней"},"",0,0,00000000-0000-0000-0000-000000000000,0},' + $nl + '{"Pattern",' + $nl + '{"N",4,0,1}');
       Expect = 'tabular-section-outside-s1' }
)
"S1-E, check 11 on $Database ($(Get-Date -Format s))" | Set-Content $Out -Encoding UTF8
foreach ($case in $cases) {
    "" | Add-Content $Out -Encoding UTF8
    "== $($case.Name)" | Add-Content $Out -Encoding UTF8
    python "$kit\tamper_stage.py" $Database $case.Row replace $case.Old $case.New 2>&1 | Add-Content $Out -Encoding UTF8
    try {
        $text = & $Exe @common --report "$env:TEMP\refusal_e.json" 2>&1 | Out-String
        $exit = $LASTEXITCODE
    } finally {
        python "$kit\tamper_stage.py" $Database $case.Row replace $case.New $case.Old 2>&1 | Add-Content $Out -Encoding UTF8
    }
    "dry run exit $exit" | Add-Content $Out -Encoding UTF8
    $reasons = @()
    if (Test-Path "$env:TEMP\refusal_e.json") {
        $report = Get-Content "$env:TEMP\refusal_e.json" -Raw -Encoding UTF8 | ConvertFrom-Json
        $reasons = @($report.gate.blockers | Where-Object { $_.row -eq '' } | ForEach-Object { $_.reason })
        Remove-Item "$env:TEMP\refusal_e.json"
    }
    $reasons | ForEach-Object { "  blocker: $_" } | Add-Content $Out -Encoding UTF8
    $ok = ($exit -ne 0) -and (($reasons -join ' ').Contains($case.Expect))
    "refused with the reason `"$($case.Expect)`": $ok" | Add-Content $Out -Encoding UTF8
}
Get-Content $Out
