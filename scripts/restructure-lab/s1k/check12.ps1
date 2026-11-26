# S1-K check 12 (docs/apply/restructuring.md 12.6): a failure inside the transaction of OUR apply takes everything back.
# Runs the script our apply generates for the case (the dry run's `--script-output`, out\<case>\script_dry.sql: the text the real run
# executes) on a FRESH twin of the staged state (out\<case>\staged.bak: the way back the drop-in `config apply --recovery-backup`
# takes just before it writes, a full backup of the staged state) with a THROW injected as the last statement before COMMIT, and
# compares the digest of the database before and after.
#
#   pwsh -NoProfile -File check12.ps1 -Case b1 [-Keep]
#
# The database is ibcmd_rs_05_ext_s1k_<case>_inj; it is dropped at the end unless -Keep. Writes out\<case>\check12.txt.
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [switch]$Keep
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
. "$PSScriptRoot\digest.ps1"
$lab = if ($env:S1K_LAB) { $env:S1K_LAB } else { 'F:\ibcmd\lab\05\ext\s1k' }
$o = "$lab\out\$Case"
$own = "ibcmd_rs_05_ext_s1k_${Case}_own"
$inj = "ibcmd_rs_05_ext_s1k_${Case}_inj"
$script = "$o\script_dry.sql"
$backup = "$o\staged.bak"
foreach ($f in $script, $backup) { if (-not (Test-Path -LiteralPath $f)) { throw "missing $f (run twin_case.ps1 first)" } }

if (-not (Test-LabDatabase $inj)) {
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak $backup -Name $inj -Track ext -Purpose "S1-K check 12, case $Case (injected failure)" | Select-Object -Last 1
}
$text = Get-Content -LiteralPath $script -Raw -Encoding UTF8
if ($text -notmatch [regex]::Escape("USE [$own];")) { throw "the script does not start with USE [$own];" }
$text = $text.Replace("USE [$own];", "USE [$inj];")
if ($text -notmatch "(\r?\n)COMMIT TRANSACTION;(\r?\n)END TRY") { throw 'no COMMIT before END TRY in the script' }
$text = $text -replace "(\r?\n)COMMIT TRANSACTION;(\r?\n)END TRY", "`$1THROW 51999, N'injected failure before COMMIT', 1;`$1COMMIT TRANSACTION;`$2END TRY"
$injected = "$o\script_injected.sql"
[IO.File]::WriteAllText($injected, $text, (New-Object Text.UTF8Encoding($true)))

"before:"
$before = Get-LabDigest $inj
$before
$sw = [Diagnostics.Stopwatch]::StartNew()
$run = sqlcmd -S localhost -E -C -f 65001 -d $inj -i $injected 2>&1
# sp_rename warns about every renamed object; the error of the THROW is the line that matters
$said = (($run | ForEach-Object { "$_" }) | Where-Object { $_ -notmatch '^Внимание|^Caution' -and $_.Trim() }) -join ' | '
$verdict = "the script ran for {0:n1} s and said: {1}" -f $sw.Elapsed.TotalSeconds, $said
$verdict
if ($said -notmatch 'injected failure before COMMIT') { throw 'the injected THROW was not reached' }
"after:"
$after = Get-LabDigest $inj
$after
$same = (($before -join "`n") -eq ($after -join "`n"))
"digest unchanged: $same"
@($verdict) + @('before:') + $before + @('after:') + $after + @("digest unchanged: $same") | Set-Content -LiteralPath "$o\check12.txt" -Encoding UTF8
Remove-Item -LiteralPath $injected -Force
if (-not $Keep) {
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\drop-lab-dbs.ps1 -Track ext -Names $inj -MinIdleMinutes 0 -Execute | Select-Object -Last 2
}
if (-not $same) { exit 1 }
