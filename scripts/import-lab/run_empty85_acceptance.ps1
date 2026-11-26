# An 8.5 БСП tree into a natively created EMPTY 8.5 infobase with the default flags (base-free, the guard on), then the
# platform's apply and export. Lab only (ibcmd_rs_04_import_*).
param([string]$Exe, [string]$Db, [string]$Tag, [string]$Tree = 'F:\ibcmd\lab\v85\ibcmd_rs_bsp_85_src_20260922_20260922_bsp85_r2\native', [switch]$SkipCreate)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. "$here\native.ps1"
Set-NativePlatform '8.5' -NoUser
Assert-LabDb $Db
$lab = $script:Lab
if ((Get-PSDrive F).Free -lt 25GB) { throw 'F: needs at least 25 GB free' }
foreach ($artifact in @("$lab\out\empty85-$Tag.json", "$lab\out\import-$Tag-own.json", "$lab\out\export\${Tag}_own", "$lab\out\export\${Tag}_own.diff.json")) {
    if (Test-Path -LiteralPath $artifact) { throw "acceptance artifact already exists: $artifact" }
}
$report = [ordered]@{ database = $Db; tree = $Tree; exe = $Exe; platform = '8.5' }
if (-not $SkipCreate) {
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus empty -Name $Db -Track import -Purpose "8.5 empty infobase for the palette colors guard fix" | Select-Object -Last 1
    if ($LASTEXITCODE -ne 0) { throw 'empty database creation failed' }
    $data = "$lab\ibdata\$Db"
    New-Item -ItemType Directory -Force $data | Out-Null
    if (-not (Test-Path "$lab\logs\empty.txt")) { New-Item -ItemType File "$lab\logs\empty.txt" | Out-Null }
    $res = @{}
    Invoke-WithNativeLock {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $args = @('infobase', 'create', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Db", "--data=$data", '--locale=ru_RU')
        $p = Start-Process -FilePath $script:Ibcmd -ArgumentList $args -NoNewWindow -PassThru `
            -RedirectStandardOutput "$lab\logs\native-create-$Tag.out.txt" -RedirectStandardError "$lab\logs\native-create-$Tag.err.txt" `
            -RedirectStandardInput "$lab\logs\empty.txt"
        if (-not $p.WaitForExit(1200 * 1000)) { $p.Kill(); $res.Exit = -999 } else { $res.Exit = $p.ExitCode }
        $res.Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1)
    }
    "native create: exit=$($res.Exit) $($res.Seconds)s"
    $report.create = $res
    if ($res.Exit -ne 0) { exit 1 }
}
$oi = Invoke-OursImport -Db $Db -Tree $Tree -Tag "$Tag-own" -Exe $Exe
"own import: exit=$($oi.Exit) rows=$($oi.Rows) $($oi.Seconds)s mode=$($oi.Mode)"
$oi.Tail
$report.import = $oi
if ($oi.Exit -ne 0) { exit 2 }
$rep = Get-Content "$lab\out\import-$Tag-own.json" -Raw | ConvertFrom-Json
"guard: $($rep.verification | ConvertTo-Json -Compress)"
$report.verification = $rep.verification
if (-not $rep.verification -or $rep.verification.checked_files -le 0 -or $rep.verification.checked_files -ne $rep.verification.identical_files) { throw 'stage guard did not pass' }
$a = Invoke-NativeApply -Db $Db -Dynamic disable -Tag "$Tag-own"
"native apply: exit=$($a.Exit) $($a.Seconds)s $($a.Tail)"
$report.apply = $a
if ($a.Exit -ne 0) { exit 3 }
$out = "$lab\out\export\${Tag}_own"
$e = Invoke-NativeExport -Db $Db -OutDir $out -Tag "$Tag-own"
"export: exit=$($e.Exit) $($e.Seconds)s"
$report.export = $e
if ($e.Exit -ne 0) { throw 'native export failed' }
$diff = "$lab\out\export\${Tag}_own.diff.json"
& $Exe source-diff -o $diff $Tree $out 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'source-diff failed' }
$d = Get-Content $diff -Raw | ConvertFrom-Json
"source-diff tree vs native export: $($d.summary | ConvertTo-Json -Compress)"
$others = @($d.differences | Where-Object { $_.status -ne 'unchanged' -and $_.path -ne 'ConfigDumpInfo.xml' })
$others | Select-Object -First 30 | ForEach-Object { "  {0,-10} {1}" -f $_.status, $_.path }
"export equals the tree (ConfigDumpInfo.xml aside): $($others.Count -eq 0)"
$report.diff_summary = $d.summary
$report.differing_files = @($others | ForEach-Object { $_.path })
$dumpInfo = python "$here\compare_dump_info.py" "$Tree\ConfigDumpInfo.xml" "$out\ConfigDumpInfo.xml" | ConvertFrom-Json
if ($LASTEXITCODE -notin @(0, 1) -or -not $dumpInfo) { throw 'dump-info comparison failed' }
$report.dump_info = $dumpInfo
$report.equal_except_config_version = ($others.Count -eq 0 -and $dumpInfo.equal_except_config_version)
$report | ConvertTo-Json -Depth 8 | Set-Content "$lab\out\empty85-$Tag.json" -Encoding UTF8
if (-not $report.equal_except_config_version) { throw 'native export differs from the source tree' }
