$ErrorActionPreference='Stop'
$root=$PSScriptRoot;$lab='F:/ibcmd/lab/05/wave3/load';$co='F:/ibcmd/lab/05/wave3/coordinator'
. "$root/runtime_closure.ps1"
function Pin([string]$Path){
 $full=Assert-Compact9RuntimePath $Path;$m=Get-Item -LiteralPath $full -Force
 if($m -isnot [IO.FileInfo]){throw 'ordinary frozen file required'}
 [ordered]@{file=$full;bytes=$m.Length;sha256=(Get-FileHash -LiteralPath $full).Hash}
}
function NewJson([string]$Path,$Value){
 $bytes=[Text.UTF8Encoding]::new($false).GetBytes(($Value|ConvertTo-Json -Depth 20)+"`r`n")
 $f=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);try{$f.Write($bytes)}finally{$f.Dispose()}
}
$prior=Read-Compact9RuntimeJson "$co/compact8-runtime-frozen-v1.json"
$binary="$co/final-packaging-v5/ibcmd-rs-7aebba15-release.exe"
$producer=Read-Compact9RuntimeJson "$co/final-packaging-v5/binary-provenance.json"
if($producer.source_head -cne '7aebba150e20c9b47b5956f4ca09e6cc62b28bef' -or !$producer.source_clean -or !$producer.targeted_packaging2_pass -or !$producer.nested_stdin_smoke_pass -or $producer.original16_run_pass -ne $false -or $producer.sha256 -cne '44513F752CB946644C856B69D740189A045452682989C6E7FC955A59035D0E38'){throw 'audited producer relation changed'}
if((Get-FileHash -LiteralPath $binary).Hash -cne $producer.sha256){throw 'audited release byte identity'}
$baseline=Read-Compact9RuntimeJson "$root/copy-baseline.json"
foreach($copy in $baseline.copies){
 if((Get-FileHash -LiteralPath $copy.source).Hash -cne $copy.source_sha256){throw 'copy baseline source changed'}
 if($copy.creation_sha256 -cnotmatch '^[0-9A-F]{64}$'){throw 'original creation SHA missing'}
 $copy|Add-Member NoteProperty sha256 (Get-FileHash -LiteralPath $copy.file).Hash
}
$baseline.scope='Fresh9 V2: original139 route copies plus diagnostic293 seam and caller120 only; existing guarded measured5485 archive/nonce/protocol/current audited release unchanged. Original139 and all prior proofs unchanged.'
NewJson "$root/copy-provenance-final.json" $baseline
$excluded=@('review_packaging_v5.ps1','review_source_certificate.py','test_packaging_stdin_close_red.ps1','prepare_tests.py','copy-baseline.json')
$paths=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach($f in Get-ChildItem -LiteralPath $root -Recurse -File -Force){if($f.Name -notin $excluded){[void]$paths.Add($f.FullName)}}
foreach($f in Get-ChildItem -LiteralPath "$lab/src/f5-9" -Recurse -File -Force){[void]$paths.Add($f.FullName)}
# Pin actual shared helpers/effective executables and original controls; old copy closure remains deep-preserved below.
foreach($e in $prior.files){
 if($e.file -notmatch '\\compact8-runtime-v1\\' -and $e.file -notmatch '\\src\\f5-8\\' -and $e.file -notmatch 'ibcmd-rs-fbf2d743-debug.exe'){
  [void]$paths.Add([IO.Path]::GetFullPath($e.file))
 }
}
[void]$paths.Add([IO.Path]::GetFullPath("$lab/compact7_closure_v2.ps1"))
[void]$paths.Add([IO.Path]::GetFullPath("$lab/compact8-prior-archive-census-v1.json"))
foreach($p in @('private-lib.ps1','private-start.ps1','test_private_startup_deadline.ps1')){[void]$paths.Add("F:\ibcmd\src\ibcmd-rs-05-load-wave3\scripts\apply-lab\cluster\$p")}
[void]$paths.Add("$co/private-startup-deadline-v1-tests.json")
foreach($e in (Read-Compact9RuntimeJson "$co/final-packaging-v5-frozen.json").files){[void]$paths.Add([IO.Path]::GetFullPath($(if($e.file){$e.file}else{$e.path})))}
foreach($f in Get-ChildItem -LiteralPath "$co/final-packaging-v5" -File -Force){[void]$paths.Add($f.FullName)}
foreach($p in @('final-packaging-v5-frozen.json','final-combined-v3-frozen.json','final-combined-v3/outcome.json','final-combined-v3/summary.txt')){[void]$paths.Add([IO.Path]::GetFullPath("$co/$p"))}
$preserved=@($prior.preserved)+@(
 [ordered]@{file="$co/compact8-runtime-frozen-v1.json";kind='closure';members=132},
 [ordered]@{file="$lab/compact8-runtime-archive-refusal-proof.json";kind='raw';members=18},
 [ordered]@{file="$co/compact9-runtime-frozen-v1.json";kind='closure';members=139}
)
foreach($p in $preserved){$pin=Pin $p.file;$p.file=$pin.file;$p.bytes=$pin.bytes;$p.sha256=$pin.sha256;[void]$paths.Add($pin.file)}
$files=@($paths|Sort-Object|ForEach-Object {Pin $_})
if($files.Count -lt 70 -or $files.Count -gt 160){throw "closure file budget $($files.Count)"}
$commands=@('pwsh','python','sqlcmd','git'|ForEach-Object {[ordered]@{name=$_;path=(Get-Command $_ -CommandType Application|Select-Object -First 1).Source}})
$exe=@($prior.executables)
foreach($e in $exe){$pin=Pin $e.file;if(!($files.file -contains $pin.file)){throw 'effective executable outside file closure'};$e.file=$pin.file}
$environment=@('PATHEXT','PYTHONHOME','PYTHONPATH','PYTHONUSERBASE','PYTHONNOUSERSITE'|ForEach-Object {[ordered]@{name=$_;value=[Environment]::GetEnvironmentVariable($_)}})
$manifest=[ordered]@{format=3;scope='Fresh compact9 V2 caller120 + diagnostic293 only, original139 preserved. Runtime WITHHELD pending root+peer and explicit scheduling; same uncreated fresh scope/current audited release. No warm readiness/zero-error claim.';runtime_executed=$false;controller_source_parent='29397db06ab1138591af6eadafba74fcbf200c53';database='ibcmd_rs_05_load_w3_compact9_20261002';acceptance_source_status='CURRENT_COMBINED_REVIEWED';binary_source=$producer.source_head;binary_sha256=$producer.sha256;binary_file=[IO.Path]::GetFullPath($binary);binary_profile='release';producer_verification=$producer.verification;original16_run_pass=$false;composite_original14_plus_targeted2=$true;unique_fifo_track='load9-983ad8f3eae1';commands=$commands;path_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($env:PATH)));selection_environment=$environment;executables=$exe;files=$files;preserved=$preserved;validation='entry before runtime allocation; after worker/heavy grants; immediately before each bounded/direct child spawn; deep after checkpoint/final. Archive requires confirmed parent proof/raw nonce lease/current parent identity; readonly measured5485 tree/4888directories/597files and all613 bytes; streaming discovery65536 bound before child metadata/push; no archive replay/moves.'}
NewJson "$co/compact9-runtime-frozen-v2.json" $manifest
$env:IBCMD_RS_COMPACT9_RUNTIME_MANIFEST_SHA=(Get-FileHash -LiteralPath "$co/compact9-runtime-frozen-v2.json").Hash
Assert-Compact9RuntimeClosure -RetainedEvidence
"PASS full9 runtime freeze $($files.Count) files/$($exe.Count) effective EXEs + deep original retained inputs; no acquires/SQL/native/lifecycle"
"MANIFEST $env:IBCMD_RS_COMPACT9_RUNTIME_MANIFEST_SHA"
