param([Parameter(Mandatory)][ValidateSet('register','unregister','list')][string]$Action,[string]$Database='')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\lib.ps1"
if(!$script:PrivateContext){throw 'private registration requires exact wave3 root'}
$state=Read-State;if(!$state -or !$state.cluster){throw 'ready private state required'}
Private-RequireLease $state;Private-RequireListeners $state
$snapshot=Private-Snapshot $state 'before-registration'
if($Action -eq 'list'){$snapshot.names;return}
Private-RequireNames @($Database)
if($Action -eq 'register' -and $Database -in $snapshot.names){throw 'registration already exists; inspect exact binding instead of overwriting'}
if($Action -eq 'unregister' -and $Database -notin $snapshot.names){throw 'registration absent; no mutation'}
$arguments=@('-NoProfile','-File','F:\ibcmd\lab\04\tools\register-ib.ps1',$Action,'-Database',$Database,'-Platform','8.3.27','-Track',$script:PrivateContext.track,'-Cluster','worker')
# The shared helper alone handles credentials. They are never read here.
$result=Invoke-LiveBounded pwsh $arguments 60
$path=Join-Path $script:Logs ($Action+'-'+$Database+'-'+(Get-Date -Format yyyyMMddHHmmssfff)+'.json')
$result|ConvertTo-Json|Set-Content -LiteralPath $path
if($result.ExitCode){throw 'shared registration helper failed; retained result must be inspected'}
$after=Private-Snapshot (Read-State) 'after-registration'
$wanted=if($Action -eq 'register'){@($snapshot.names)+@($Database)}else{@($snapshot.names|Where-Object{$_ -cne $Database})}
if((@($wanted|Sort-Object) -join "`n") -cne (@($after.names|Sort-Object) -join "`n")){throw 'registration result differs; no further lifecycle write'}
$result.Stdout
