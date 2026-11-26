# The twin protocol for all seven cases with the final binary, on fresh own twins (ibcmd_rs_05_trace_f<N>_own), from the
# staged backups. The native twins are the ones twin_stage.ps1 made. Steps: run (checks 2-6, 10), reg, extra (7, 9), inject (12).
param([string]$Cases = 'n1:tw1,n2:tw2,n3:tw3c,n4:tw4,n5:tw5,n6:tw6,n7:tw7', [string]$Steps = 'run,reg,extra', [string]$Suffix = '', [string]$LabelPrefix = 'f', [string]$Exe = 'F:\ibcmd\lab\05\s1g\bin\ibcmd-rs-f.exe')
$CaseList = $Cases -split '[,\s]+' | Where-Object { $_ }
$StepList = $Steps -split '[,\s]+' | Where-Object { $_ }
"cases=[$($CaseList -join '|')] steps=[$($StepList -join '|')]"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:DDL_LAB = 'F:\ibcmd\lab\05\s1g\store'; $env:PYTHONUTF8 = '1'
$kit = 'F:\ibcmd\src\ibcmd-rs-m-simple\scripts\restructure-lab'
$mine = 'F:\ibcmd\src\ibcmd-rs-m-simple\scripts\apply-trace\lab\s1g-caches'
$exe = $Exe
foreach ($c in $CaseList) {
    $n, $tag = $c -split ':'
    $nat = "ibcmd_rs_05_trace_${tag}_nat"; $st = "ibcmd_rs_05_trace_${tag}_st"; $own = "ibcmd_rs_05_trace_${LabelPrefix}${n}${Suffix}_own"
    $bak = "F:\ibcmd\lab\05\s1g\bak\${n}_staged.bak"
    $label = "$LabelPrefix$n"
    if ($StepList -contains 'run') {
        $there = "$(sqlcmd -S localhost -E -C -h -1 -W -Q "SET NOCOUNT ON; SELECT ISNULL(DB_ID(N'$own'), 0)")".Trim()
        if ($there -eq '0') {
            pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak $bak -Name $own -Track trace -Purpose "S1-F final run of $n with the final binary" | Select-Object -Last 1
        } else {
            $staged = "$(sqlcmd -S localhost -E -C -h -1 -W -d $own -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave")".Trim()
            if ($staged -eq '0') { throw "$own has been applied already" }
            "reusing the staged $own"
        }
        Set-Location $kit
        try {
            & "$kit\twin_run.ps1" -Case $label -Nat $nat -Own $own -Base $st -StagedLabel "${n}_staged" -Exe $exe -NatLabel nat_after -Skip export, session, noop *> "F:\ibcmd\lab\05\s1g\logs\twin_run_$label.log"
        } catch {
            "twin_run failed: $_" | Add-Content "F:\ibcmd\lab\05\s1g\logs\twin_run_$label.log"
        }
    }
    if ($StepList -contains 'reg') {
        python F:\ibcmd\lab\05\s1g\kit\reg_cmp.py $nat $own $st > "F:\ibcmd\lab\05\s1g\store\out\${label}_reg.txt" 2>&1
    }
    foreach ($x in 'noop', 'session') {
        if ($StepList -contains $x) {
            pwsh -NoProfile -File "$mine\twin_extra.ps1" -Nat $nat -Own $own -Label $label -Steps $x *> "F:\ibcmd\lab\05\s1g\logs\twin_extra_${label}_$x.log"
        }
    }
    if ($StepList -contains 'extra') {
        pwsh -NoProfile -File "$mine\twin_extra.ps1" -Nat $nat -Own $own -Label $label -Steps noop,session *> "F:\ibcmd\lab\05\s1g\logs\twin_extra_$label.log"
    }
    if ($StepList -contains 'export') {
        pwsh -NoProfile -File "$mine\twin_extra.ps1" -Nat $nat -Own $own -Label $label -Steps export *> "F:\ibcmd\lab\05\s1g\logs\twin_extra_${label}_export.log"
    }
    if ($StepList -contains 'inject') {
        pwsh -NoProfile -File "$mine\twin_inject.ps1" -Case $n -Run "F:\ibcmd\lab\05\s1g\store\out\run_$label" -Own $own -To "ibcmd_rs_05_trace_f${n}_inj" *> "F:\ibcmd\lab\05\s1g\logs\twin_inject_$label.log"
    }
}
