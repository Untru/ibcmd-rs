$ErrorActionPreference='Stop'
$tests=@('test_archive_identity.ps1','test_archive_worker.ps1','test_archive_context.ps1','test_prior_archive.ps1','test_private_nonce.ps1','test_stdin_close.ps1','compact8-tools/live/test_process.ps1','test_compact8_locks.ps1','test_compact8_acquire_v2.ps1','test_compact8_safety_v2.ps1','test_compact8_receipts.ps1','test_compact8_header_v2.ps1','test_compact8_close.ps1','test_compact8_postconditions.ps1','test_compact8_invalid_control_v2.ps1')
foreach($test in $tests){
 & 'C:/Program Files/PowerShell/7/pwsh.exe' -NoProfile -File "$PSScriptRoot/$test"
 if($LASTEXITCODE){throw "pure suite failed $test"}
}
'PASS all prepared copied8 pure suites; no FIFO/native/SQL/registry/signals/deletes'
