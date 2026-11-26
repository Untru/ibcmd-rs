function Get-Compact8ConfirmedReceipts([string]$Path,[string]$Label) {
 $count=0;$seenAttempts=@{}
 foreach($line in (Get-Content -LiteralPath $Path)){
  $cells=$line -split '\|',7
  if($cells.Count -ne 7){throw 'malformed journal, not workload proof'}
  if($cells[1] -cne $Label){throw 'cohort journal label changed'}
  if($cells[2] -cne 'operation'){continue}
  $fields=@{};$known=@('attempt','client_ms','committed','doc_uuid','doc_ms','report_ok','report_rows','report_ms')
  foreach($match in [regex]::Matches($cells[6],'(?:^|;)([A-Za-z_][A-Za-z0-9_-]*)=([^;]*)')){
   $key=$match.Groups[1].Value;$value=$match.Groups[2].Value
   if($key -cnotin $known){continue}
   if($fields.ContainsKey($key)){throw "duplicate receipt field $key"};$fields[$key]=$value
  }
  if(!$fields.ContainsKey('attempt') -or $fields.attempt -cnotmatch '^\d+$' -or !$fields.ContainsKey('committed') -or $fields.committed -cnotin @('0','1','unknown')){throw 'invalid operation receipt attempt/commit'}
  if($seenAttempts.ContainsKey($fields.attempt)){throw 'duplicate operation receipt attempt'};$seenAttempts[$fields.attempt]=$true
  if($fields.ContainsKey('report_ok') -and $fields.report_ok -cnotin @('0','1')){throw 'invalid report_ok receipt value'}
  if($fields.committed -ceq '1'){
   if(!$fields.ContainsKey('report_ok') -or !$fields.ContainsKey('doc_uuid') -or $fields.doc_uuid -cnotmatch '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'){throw 'committed receipt lacks exact report/UUID'}
   if($fields.report_ok -ceq '1'){$count++}
  }
 }
 return $count
}
function Compare-Compact8Storage([string]$Lab,[string]$Prefix,[string]$Before,[string]$After) {
 $a=@(Get-Content -LiteralPath "$Lab/snapshots/$Prefix-$Before-storage.txt");$b=@(Get-Content -LiteralPath "$Lab/snapshots/$Prefix-$After-storage.txt")
 $proof=[ordered]@{before_snapshot=$Before;after_snapshot=$After;before_sha256=(Get-FileHash "$Lab/snapshots/$Prefix-$Before-storage.txt").Hash;after_sha256=(Get-FileHash "$Lab/snapshots/$Prefix-$After-storage.txt").Hash;tables=[ordered]@{}}
 foreach($table in @('Config','ConfigSave','Params','Files')){
  $beforeRows=@($a|Where-Object{$_.StartsWith($table+'|')});$afterRows=@($b|Where-Object{$_.StartsWith($table+'|')})
  $equal=($beforeRows -join "`n") -ceq ($afterRows -join "`n")
  $proof.tables[$table]=@{exact_all_headers_and_data_sha_equal=$equal;before_rows=$beforeRows.Count;after_rows=$afterRows.Count;difference=@(if(!$equal){@{before=$beforeRows;after=$afterRows}})}
 }
 $path="$Lab/logs/$Prefix-$Before-$After-storage-comparison.json";if(Test-Path -LiteralPath $path){throw 'comparison proof exists'};$proof|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $path
 foreach($table in @('Config','ConfigSave','Params')){if(!$proof.tables[$table].exact_all_headers_and_data_sha_equal){throw "$table changed during refusal/repeat; preserve proof, do not claim unchanged"}}
 # Files is captured and its actual difference retained; operational changes do not become exact-equality claims.
}

