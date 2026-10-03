$ErrorActionPreference='Stop'
$tests=@('test_archive_identity.ps1','test_archive_worker.ps1','test_archive_context.ps1','test_archive_tree.ps1','test_prior_archive.ps1','test_private_nonce.ps1','test_stdin_close.ps1','compact9-tools/live/test_process.ps1','test_compact9_locks.ps1','test_compact9_acquire_v2.ps1','test_compact9_safety_v2.ps1','test_compact9_receipts.ps1','test_compact9_header_v2.ps1','test_compact9_close.ps1','test_compact9_postconditions.ps1','test_compact9_invalid_control_v2.ps1')
foreach($test in $tests){
 & 'C:/Program Files/PowerShell/7/pwsh.exe' -NoProfile -File "$PSScriptRoot/$test"
 if($LASTEXITCODE){throw "pure suite failed $test"}
}
'PASS all 16 prepared copied9 pure suites; no FIFO/native/SQL/registry/signals/deletes'
