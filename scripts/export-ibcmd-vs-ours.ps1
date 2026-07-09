param(
    [string]$DbName = "ut_ibcmd",
    [string]$DbServer = "localhost",
    [string]$DbUser = "sa",
    [string]$LabRoot = "E:\ibcmd_lab\060726",
    [string]$SourceVersion = "2.20",
    [string]$IbcmdPath = "",
    [switch]$SkipIbcmd
)

$ErrorActionPreference = "Stop"

function Invoke-Step {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Message,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Action
    )

    Write-Host "==> $Message"
    & $Action
}

function Assert-ToolExitCode {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ToolName
    )

    if ($LASTEXITCODE -ne 0) {
        throw "$ToolName failed with exit code $LASTEXITCODE"
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $repoRoot "target\release\ibcmd-rs.exe"
if (-not (Test-Path $exe)) {
    throw "Missing executable: $exe. Build release first with: cargo build --release"
}

if (-not $env:IBCMD_DB_PSW) {
    throw "Environment variable IBCMD_DB_PSW is not set"
}

$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$runRoot = Join-Path $LabRoot "full_compare_$stamp"
$ibcmdRoot = Join-Path $runRoot "ibcmd"
$oursDumpRoot = Join-Path $runRoot "ibcmd_rs_dump"
$oursSourceRoot = Join-Path $runRoot "ibcmd_rs_source_only"
$diffPath = Join-Path $runRoot "diff_full_source_only.json"
$reportPath = Join-Path $runRoot "report.json"
$stderrPath = Join-Path $runRoot "stderr.txt"

New-Item -ItemType Directory -Force -Path $runRoot | Out-Null

if (-not $SkipIbcmd) {
    Invoke-Step "Exporting baseline with ibcmd" {
        $args = @(
            "dump-sources",
            "--dbms", "MSSQLServer",
            "--db-server", $DbServer,
            "--db-name", $DbName,
            "--db-user", $DbUser,
            "-o", $ibcmdRoot,
            "--overwrite"
        )
        if ($IbcmdPath) {
            $args += @("--ibcmd", $IbcmdPath)
        }
        & $exe @args
        Assert-ToolExitCode "dump-sources"
    }
}

Invoke-Step "Exporting current tree with ibcmd-rs" {
    & $exe mssql-dump-config `
        --database $DbName `
        --server $DbServer `
        --sql-user $DbUser `
        -o $oursDumpRoot `
        --overwrite `
        --inflate `
        --extract-module-text `
        --extract-metadata-xml `
        --source-version $SourceVersion `
        --no-binary-rows `
        1> $reportPath `
        2> $stderrPath
    Assert-ToolExitCode "mssql-dump-config"
}

Invoke-Step "Copying source-only tree" {
    robocopy `
        $oursDumpRoot `
        $oursSourceRoot `
        /E `
        /XD Config_inflated Config_raw ConfigSave_inflated ConfigSave_raw `
        /XF manifest.json *.json | Out-Null

    if ($LASTEXITCODE -gt 7) {
        throw "robocopy failed with exit code $LASTEXITCODE"
    }
}

if (-not $SkipIbcmd) {
    Invoke-Step "Building source diff" {
        & $exe source-diff -o $diffPath $ibcmdRoot $oursSourceRoot
        Assert-ToolExitCode "source-diff"
    }
}

Write-Host ""
Write-Host "Run root: $runRoot"
if (-not $SkipIbcmd) {
    Write-Host "ibcmd baseline: $ibcmdRoot"
    Write-Host "diff json: $diffPath"
}
Write-Host "ibcmd-rs dump: $oursDumpRoot"
Write-Host "ibcmd-rs source-only: $oursSourceRoot"
Write-Host "report: $reportPath"
