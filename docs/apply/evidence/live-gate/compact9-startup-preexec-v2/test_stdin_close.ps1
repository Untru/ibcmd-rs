$ErrorActionPreference='Stop'
. "$PSScriptRoot/compact9-tools/live/process.ps1"
function Assert-Compact9RuntimeClosure {}
function Get-LiveChildReceiptRoot {return $null}
$script:saved=$null;$script:disposed=$false;$script:signals=0
function Save-LiveUncertainChild($State){$script:saved=$State}
function New-LiveBoundedProcess($StartInfo){
 if(!$StartInfo.RedirectStandardInput){throw 'stdin not redirected'}
 $reader=[pscustomobject]@{};$reader|Add-Member ScriptMethod ReadToEndAsync {return [pscustomobject]@{mock_pipe=$true}}
 $input=[pscustomobject]@{};$input|Add-Member ScriptMethod Close {throw 'injected stdin Close failure'}
 $p=[pscustomobject]@{Id=123;StartTime=[datetime]::UtcNow;HasExited=$false;StandardInput=$input;StandardOutput=$reader;StandardError=$reader}
 $p|Add-Member ScriptMethod Start {return $true}
 $p|Add-Member ScriptMethod Dispose {$script:disposed=$true}
 $p|Add-Member ScriptMethod Kill {$script:signals++}
 return $p
}
$refused=$false
try{Invoke-LiveBounded 'fixed.exe' @('fixed') 1}catch{if($_.Exception.Message -notmatch 'LIVE_CHILD_EXECUTION_UNCERTAIN'){throw};$refused=$true}
if(!$refused -or !$script:LiveBoundedUncertainChild -or !$script:saved -or $script:saved.pid -ne 123 -or $script:saved.execution_state -cne 'unknown_running' -or !$script:disposed -or $script:signals){throw 'Close fault lost live child authority'}
Write-Output 'PASS stdin Close fault retains original started child/receipt uncertainty; no real process/filesystem/SQL/signals'
