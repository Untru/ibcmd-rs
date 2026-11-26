$ErrorActionPreference='Stop'
function Assert-Compact9RuntimeClosure {}
. 'F:/ibcmd/lab/05/wave3/load/compact9-runtime-v1/compact9_receipts.ps1'
$lab='F:/ibcmd/lab/05/wave3/load';$path="$lab/compact9-receipt-test-input.txt"
if(Test-Path -LiteralPath $path){throw 'fresh test file required'}
try {
 $base='63890000000000|test|operation|12|OLD|OLD|attempt=1;client_ms=1;committed=1;doc_uuid=11111111-1111-1111-1111-111111111111;doc_ms=1;report_ok=1;report_rows=1;report_ms=1'
 $base|Set-Content -LiteralPath $path
 if((Get-Compact9ConfirmedReceipts $path test) -ne 1){throw 'valid receipt rejected'}
 foreach($value in @('10','1extra','0extra')){
  $base.Replace('report_ok=1',"report_ok=$value")|Set-Content -LiteralPath $path
  $refused=$false;try{[void](Get-Compact9ConfirmedReceipts $path test)}catch{$refused=$true};if(!$refused){throw "accepted malformed$value"};"PASS invalid$value refusal"
 }
 foreach($suffix in @(';report_ok=1',';committed=1',';attempt=1')){
  ($base+$suffix)|Set-Content -LiteralPath $path
  $refused=$false;try{[void](Get-Compact9ConfirmedReceipts $path test)}catch{$refused=$true};if(!$refused){throw 'accepted duplicate'};"PASS duplicate$suffix refusal"
 }
 $base.Replace('report_ok=1','report_ok=0')|Set-Content -LiteralPath $path;if((Get-Compact9ConfirmedReceipts $path test) -ne 0){throw 'failed report accepted'}
 $base.Replace('committed=1','committed=0')|Set-Content -LiteralPath $path;if((Get-Compact9ConfirmedReceipts $path test) -ne 0){throw 'uncommitted accepted'}
 'PASS valid1, malformed3, duplicate3, report0/commit0 excluded; no process/native/SQL starts'
}finally{Remove-Item -LiteralPath $path}
