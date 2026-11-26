param([string]$Database, [string]$Mode = 'exclusive', [string]$Out = 'F:\ibcmd\lab\05\online\runs\online\act-dry')
. F:\ibcmd\lab\05\online\tools\lab-env.ps1
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$cluster = Get-ClusterId; $ib = Get-InfobaseId $Database
& $Tool mssql-activate-staged-main --platform-profile platform-8.3.27.2214 --sqlcmd-trust-cert --server localhost --database $Database `
  --mode $Mode --dry-run --allow-non-lab --script-output "$Out\dry.sql" --recovery-output "$Out\dry.recovery.json" `
  --rac $Rac --ras-endpoint $Ras --cluster-id $cluster --infobase-id $ib --infobase-user Администратор 2>&1 | Select-Object -First 30
"exit=$LASTEXITCODE"
