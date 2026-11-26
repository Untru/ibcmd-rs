$ErrorActionPreference='Stop'
$root=$PSScriptRoot;$lab='F:/ibcmd/lab/05/wave3/load';$co='F:/ibcmd/lab/05/wave3/coordinator'
. "$root/runtime_closure.ps1"
function Pin([string]$Path){
 $full=Assert-Compact8RuntimePath $Path;$m=Get-Item -LiteralPath $full -Force
 if($m -isnot [IO.FileInfo]){throw 'ordinary frozen file required'}
 [ordered]@{file=$full;bytes=$m.Length;sha256=(Get-FileHash -LiteralPath $full).Hash}
}
function NewJson([string]$Path,$Value){
 $bytes=[Text.UTF8Encoding]::new($false).GetBytes(($Value|ConvertTo-Json -Depth 20)+"`r`n")
 $f=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);try{$f.Write($bytes)}finally{$f.Dispose()}
}
$prior=Read-Compact8RuntimeJson "$co/compact7-runtime-frozen-v3.json"
$binary="$co/final-packaging-v5/ibcmd-rs-7aebba15-release.exe"
$producer=Read-Compact8RuntimeJson "$co/final-packaging-v5/binary-provenance.json"
if($producer.source_head -cne '7aebba150e20c9b47b5956f4ca09e6cc62b28bef' -or !$producer.source_clean -or !$producer.targeted_packaging2_pass -or !$producer.nested_stdin_smoke_pass -or $producer.original16_run_pass -ne $false -or $producer.sha256 -cne '44513F752CB946644C856B69D740189A045452682989C6E7FC955A59035D0E38'){throw 'audited producer relation changed'}
if((Get-FileHash -LiteralPath $binary).Hash -cne $producer.sha256){throw 'audited release byte identity'}
$baseline=Read-Compact8RuntimeJson "$root/copy-baseline.json"
foreach($copy in $baseline.copies){
 if((Get-FileHash -LiteralPath $copy.source).Hash -cne $copy.source_sha256){throw 'copy baseline source changed'}
 $copy|Add-Member NoteProperty creation_sha256 $copy.sha256
 $copy.sha256=(Get-FileHash -LiteralPath $copy.file).Hash
}
$baseline.scope='Full8 copied successor: case8 routes; current audited release authority; unique nonce worker/raw acquire proof; explicit CIM microsecond precision; valid closed stdin with original PID/pipe ownership before Close; readonly archived613 admission, no replay. Original sources unchanged.'
NewJson "$root/copy-provenance-final.json" $baseline
$excluded=@('review_packaging_v5.ps1','review_source_certificate.py','test_packaging_stdin_close_red.ps1','prepare_tests.py','copy-baseline.json')
$paths=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach($f in Get-ChildItem -LiteralPath $root -Recurse -File -Force){if($f.Name -notin $excluded){[void]$paths.Add($f.FullName)}}
foreach($f in Get-ChildItem -LiteralPath "$lab/src/f5-8" -Recurse -File -Force){[void]$paths.Add($f.FullName)}
# Pin actual shared helpers/effective executables and original controls; old copy closure remains deep-preserved below.
foreach($e in $prior.files){
 if($e.file -notmatch '\\compact7(?:-|_|\\)' -and $e.file -notmatch '\\src\\f5-7\\' -and $e.file -notmatch 'ibcmd-rs-fbf2d743-debug.exe'){
  [void]$paths.Add([IO.Path]::GetFullPath($e.file))
 }
}
[void]$paths.Add([IO.Path]::GetFullPath("$lab/compact7_closure_v2.ps1"))
foreach($e in (Read-Compact8RuntimeJson "$co/final-packaging-v5-frozen.json").files){[void]$paths.Add([IO.Path]::GetFullPath($(if($e.file){$e.file}else{$e.path})))}
foreach($f in Get-ChildItem -LiteralPath "$co/final-packaging-v5" -File -Force){[void]$paths.Add($f.FullName)}
foreach($p in @('final-packaging-v5-frozen.json','final-combined-v3-frozen.json','final-combined-v3/outcome.json','final-combined-v3/summary.txt')){[void]$paths.Add([IO.Path]::GetFullPath("$co/$p"))}
$preserved=@($prior.preserved)+@(
 [ordered]@{file="$co/compact7-runtime-frozen-v3.json";kind='closure';members=100},
 [ordered]@{file="$lab/compact7-runtime-guard-refusal-proof.json";kind='raw';members=17},
 [ordered]@{file="$co/compact8-archive-preparation-frozen-v1.json";kind='closure';members=10},
 [ordered]@{file="$co/final-combined-v3-frozen.json";kind='closure';members=40},
 [ordered]@{file="$co/final-packaging-v5-frozen.json";kind='closure';members=14}
)
foreach($p in $preserved){$pin=Pin $p.file;$p.file=$pin.file;$p.bytes=$pin.bytes;$p.sha256=$pin.sha256;[void]$paths.Add($pin.file)}
$files=@($paths|Sort-Object|ForEach-Object {Pin $_})
if($files.Count -lt 70 -or $files.Count -gt 160){throw "closure file budget $($files.Count)"}
$commands=@('pwsh','python','sqlcmd','git'|ForEach-Object {[ordered]@{name=$_;path=(Get-Command $_ -CommandType Application|Select-Object -First 1).Source}})
$exe=@($prior.executables)
foreach($e in $exe){$pin=Pin $e.file;if(!($files.file -contains $pin.file)){throw 'effective executable outside file closure'};$e.file=$pin.file}
$environment=@('PATHEXT','PYTHONHOME','PYTHONPATH','PYTHONUSERBASE','PYTHONNOUSERSITE'|ForEach-Object {[ordered]@{name=$_;value=[Environment]::GetEnvironmentVariable($_)}})
$manifest=[ordered]@{format=3;scope='ONE fresh compact8 lifecycle. Runtime WITHHELD pending root+independent PREEXEC and explicit scheduling. No warm admission/zero-error/readiness claim. All old freezes retained.';runtime_executed=$false;controller_source_parent='c6ea62201b4e8af73d6a8c4401186b7b79808b29';database='ibcmd_rs_05_load_w3_compact8_20261002';acceptance_source_status='CURRENT_COMBINED_REVIEWED';binary_source=$producer.source_head;binary_sha256=$producer.sha256;binary_file=[IO.Path]::GetFullPath($binary);binary_profile='release';producer_verification=$producer.verification;original16_run_pass=$false;composite_original14_plus_targeted2=$true;unique_fifo_track='load8-f8bd7d065a2c';commands=$commands;path_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($env:PATH)));selection_environment=$environment;executables=$exe;files=$files;preserved=$preserved;validation='entry before runtime allocation; after worker/heavy grants; immediately before each bounded/direct child spawn; deep after checkpoint/final. Archive requires confirmed parent proof/raw nonce lease/current parent identity; readonly prior613 archive, no moves.'}
NewJson "$co/compact8-runtime-frozen-v1.json" $manifest
$env:IBCMD_RS_COMPACT8_RUNTIME_MANIFEST_SHA=(Get-FileHash -LiteralPath "$co/compact8-runtime-frozen-v1.json").Hash
Assert-Compact8RuntimeClosure -RetainedEvidence
"PASS full8 runtime freeze $($files.Count) files/$($exe.Count) effective EXEs + deep original retained inputs; no acquires/SQL/native/lifecycle"
"MANIFEST $env:IBCMD_RS_COMPACT8_RUNTIME_MANIFEST_SHA"
