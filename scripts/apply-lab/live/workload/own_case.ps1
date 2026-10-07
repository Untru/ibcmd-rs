param([ValidateRange(1,5)][int]$Case=1)
$ErrorActionPreference='Stop';$env:RAYON_NUM_THREADS='4'
$wt='F:/ibcmd/src/ibcmd-rs-05-load-wave3';$lab='F:/ibcmd/lab/05/wave3/load';$db='ibcmd_rs_05_load_w3_work_20261001';$bin='F:/ibcmd/lab/05/wave2/coordinator/ibcmd-rs-wave2-final.exe';$tag="own$Case"
. "$wt/scripts/apply-lab/live/process.ps1"
if((Get-FileHash $bin).Hash -ne '639C04D7180EB6F8904084698BD9AB7BA24E4F50D8547FCA0A99350D5B135E44'){throw 'binary provenance mismatch'}
function Run($label,$exe,[string[]]$argv,$deadline){
if(Test-Path "$lab/logs/$label.result.json"){throw 'fresh command label required'}
@{source_head='6a7cbbf60e70d35c03767e0ad06c1afa4915e6e7';executable=$exe;arguments=$argv;deadline=$deadline;started_utc=[DateTime]::UtcNow.ToString('o');epf_sha=(Get-FileHash "$lab/observer/IbcmdRsObserver.epf").Hash}|ConvertTo-Json -Depth 5|Set-Content "$lab/logs/$label.command.json" -Encoding utf8
$sw=[Diagnostics.Stopwatch]::StartNew();$r=Invoke-LiveBounded $exe $argv $deadline;$sw.Stop();@{exit_code=$r.ExitCode;stdout=$r.Stdout;stderr=$r.Stderr;elapsed_ms=$sw.ElapsedMilliseconds;ended_utc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json -Depth 5|Set-Content "$lab/logs/$label.result.json" -Encoding utf8
return $r
}
$r=Run "$tag-import" pwsh @('-NoProfile','-File',"$wt/scripts/apply-lab/live/native_load.ps1",'-Action','import','-Database',$db,'-LabRoot',$lab,'-Label',"$tag-native-import",'-Tree',"$lab/src/$tag",'-TimeoutSec','120') 760
if($r.ExitCode){throw 'native partial import failed'}
$r=Run "$tag-staged" pwsh @('-NoProfile','-File',"$lab/snapshot.ps1",'-Label',"$tag-staged") 120
if($r.ExitCode){throw 'snapshot failed'}
$argv=@('infobase','config','apply','--dbms=MSSQLServer','--db-server=localhost',"--db-name=$db",'--user=Администратор','--platform=8.3.27.2214','--force','--dynamic=force',"--report=$lab/logs/$tag-apply-report.json")
$r=Run "$tag-apply" $bin $argv 90
if($r.ExitCode){throw "own apply refused/failed; preserved $tag result"}
$r=Run "$tag-after" pwsh @('-NoProfile','-File',"$lab/snapshot.ps1",'-Label',"$tag-after") 120
if($r.ExitCode){throw 'after snapshot failed'}
$r=Run "$tag-repeat" $bin @('infobase','config','apply','--dbms=MSSQLServer','--db-server=localhost',"--db-name=$db",'--platform=8.3.27.2214','--force','--dynamic=force',"--report=$lab/logs/$tag-repeat-report.json") 90
if($r.ExitCode){throw 'repeat refused/failed'}
$r=Run "$tag-repeat-after" pwsh @('-NoProfile','-File',"$lab/snapshot.ps1",'-Label',"$tag-repeat-after") 120
if($r.ExitCode){throw 'repeat snapshot failed'}
$before=Get-Content "$lab/snapshots/$tag-after-storage.txt" -Raw;$after=Get-Content "$lab/snapshots/$tag-repeat-after-storage.txt" -Raw
if($before -cne $after){throw 'repeat changed measured full configuration storage'}
$r=Run "$tag-new-start" pwsh @('-NoProfile','-File',"$wt/scripts/apply-lab/live/obs.ps1",'start','-Database',$db,'-Label',"$tag-new",'-Mode','poll','-LabRoot',$lab,'-Srvr','localhost:2541','-TimeoutSec','45') 60
if($r.ExitCode){throw 'new cohort did not open'}
"PASS $tag dynamic publication + repeated full storage unchanged; new cohort journal retained"
