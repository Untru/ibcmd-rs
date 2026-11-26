# capture.ps1 (snapshots + XE trace + diff) around one apply.ps1 / act.ps1 run, with hooks for the session matrix.
#   pwsh -NoProfile -File cap-apply.ps1 -Database <db> -Kind apply -Src <tag> -Mode <mode> -Run <run> -Tag <tag> [-TailLog <path>]
#        [-DataChecksum full|counts|none] [-ArmTxn <label>] [-StartNew <label,label>] [-ArmLazy <label>]
#   pwsh -NoProfile -File cap-apply.ps1 -Database <db> -Kind act -Mode <mode> -Run <run> -Tag <tag>            (activate the staged ConfigSave)
# -ArmTxn <label>: right before the command, create obs\<label>.go so the txn observer opens its 60 s server transaction,
#                  and wait until its journal shows txn-begin (+3 s).
# -StartNew a,b:   right after the command (before the after-snapshot) launch NEW observer clients with these labels.
# -ArmLazy <label>: ARMS nothing before; right after the command creates obs\<label>.go so the lazy observer touches the module for the first time.
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [ValidateSet('apply', 'act')][string]$Kind = 'apply',
    [string]$Src = '',
    [Parameter(Mandatory = $true)][string]$Mode,
    [Parameter(Mandatory = $true)][string]$Run,
    [Parameter(Mandatory = $true)][string]$Tag,
    [string]$TailLog = '',
    [ValidateSet('full', 'counts', 'none')][string]$DataChecksum = 'full',
    [string]$BeforeSnapshot = '',
    [string]$ArmTxn = '',
    [string[]]$StartNew = @(),
    [string]$ArmLazy = ''
)
$kit = 'F:\ibcmd\src\ibcmd-rs-m-simple\scripts\apply-trace'
$lab = 'F:\ibcmd\lab\05\online'
if ($Kind -eq 'apply') {
    $argsList = @('-NoProfile', '-File', "$lab\tools\apply.ps1", '-Database', $Database, '-Src', $Src, '-Mode', $Mode, '-Run', $Run, '-Tag', $Tag)
} else {
    $argsList = @('-NoProfile', '-File', "$lab\tools\act.ps1", '-Database', $Database, '-Mode', $Mode, '-Run', $Run, '-Tag', $Tag)
}
if ($TailLog) { $argsList += @('-TailLog', $TailLog) }
$cmd = { & pwsh @argsList }.GetNewClosure()
$capArgs = @{
    Database = $Database; Tag = $Tag; Track = 'online'
    OutRoot = "$lab\runs\$Run\cap"; Command = $cmd; TimeoutMinutes = 20
    DataChecksum = $DataChecksum; Note = "#344 $Mode $Kind $Src"
}
if ($BeforeSnapshot) { $capArgs['BeforeSnapshot'] = $BeforeSnapshot }
if ($ArmTxn) {
    $capArgs['BeforeCommand'] = {
        $go = "$lab\obs\$ArmTxn.go"
        Set-Content -LiteralPath $go -Value 'go' -Encoding ascii
        $journal = "$lab\obs\$ArmTxn.log"
        $deadline = (Get-Date).AddSeconds(30)
        while ((Get-Date) -lt $deadline) {
            if ((Test-Path -LiteralPath $journal) -and (Select-String -LiteralPath $journal -Pattern '\|txn-begin\|' -Quiet)) { break }
            Start-Sleep -Milliseconds 300
        }
        Start-Sleep -Seconds 3
        Write-Host "armed txn observer $ArmTxn at $([DateTime]::UtcNow.ToString('o'))"
    }.GetNewClosure()
}
if ($StartNew.Count -or $ArmLazy) {
    $capArgs['AfterCommand'] = {
        if ($ArmLazy) {
            Set-Content -LiteralPath "$lab\obs\$ArmLazy.go" -Value 'go' -Encoding ascii
            Write-Host "armed lazy observer $ArmLazy at $([DateTime]::UtcNow.ToString('o'))"
        }
        foreach ($label in $StartNew) {
            Start-Process -FilePath 'pwsh.exe' -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', "`"$lab\tools\obs-start.ps1`"", '-Database', $Database, '-Label', $label, '-TimeoutSec', '300')
            Write-Host "launched NEW observer $label at $([DateTime]::UtcNow.ToString('o'))"
        }
    }.GetNewClosure()
}
& (Join-Path $kit 'capture.ps1') @capArgs
