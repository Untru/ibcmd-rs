[CmdletBinding()]
param(
    [string]$UtDbName = "ut_ibcmd",
    [string]$BspDbName = "bsp",
    [string]$DbServer = "localhost",
    [string]$DbUser = "sa",
    [string]$LabRoot = "E:\ibcmd_lab\parity",
    [string]$RunId = (Get-Date -Format "yyyyMMdd_HHmmss"),
    [string]$ExePath = "",
    [string]$IbcmdPath = "",
    [ValidateSet("2.20", "2.21")][string]$SourceVersion = "2.20",
    [ValidateSet("full", "scoped")][string]$Scope = "full",
    [string[]]$PathPrefix = @(),
    [switch]$RequireCompleteRootMetadata
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$runner = Join-Path $PSScriptRoot "export-ibcmd-vs-ours.ps1"
if (-not (Test-Path -LiteralPath $runner)) { throw "Missing runner: $runner" }
if ($RunId -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$' -or $RunId.Contains('..')) {
    throw "RunId must be 1-128 safe characters (A-Z, a-z, 0-9, dot, underscore, hyphen), start with an alphanumeric character, and not contain '..'."
}
if (@($PathPrefix | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -ne 0) {
    throw "PathPrefix entries must be non-empty, non-whitespace strings."
}
if ($Scope -eq "full" -and $PathPrefix.Count -ne 0) {
    throw "Scope 'full' requires an empty PathPrefix. Use -Scope scoped for a partial comparison."
}
if ($Scope -eq "scoped" -and $PathPrefix.Count -eq 0) {
    throw "Scope 'scoped' requires at least one PathPrefix."
}
if ($RequireCompleteRootMetadata -and $Scope -ne "full") {
    throw "RequireCompleteRootMetadata is available only for Scope 'full'."
}
if ([string]::IsNullOrWhiteSpace($ExePath)) {
    $repoRoot = Split-Path -Parent $PSScriptRoot
    $ExePath = Join-Path $repoRoot "target\release\ibcmd-rs.exe"
}
$ExePath = [IO.Path]::GetFullPath($ExePath)

$matrixRoot = Join-Path $LabRoot ("matrix_{0}" -f $RunId)
if (Test-Path -LiteralPath $matrixRoot) { throw "Matrix directory already exists and is immutable: $matrixRoot" }
New-Item -ItemType Directory -Path $matrixRoot | Out-Null

$matrixPaths = [System.Collections.ArrayList]::new()
foreach ($database in @(
    [ordered]@{ id="ut"; name=$UtDbName },
    [ordered]@{ id="bsp"; name=$BspDbName }
)) {
    $childRunId = "{0}_{1}" -f $RunId, $database.id
    $params = @{
        DbName=$database.name; DbServer=$DbServer; DbUser=$DbUser; LabRoot=$LabRoot
        RunId=$childRunId; SourceVersion=$SourceVersion; Scope=$Scope; PathPrefix=$PathPrefix; ExePath=$ExePath
    }
    if ($IbcmdPath) { $params.IbcmdPath = $IbcmdPath }
    if ($RequireCompleteRootMetadata) { $params.RequireCompleteRootMetadata = $true }
    & $runner @params
    if ($LASTEXITCODE -ne 0) { throw "Parity run failed for database '$($database.name)' with exit code $LASTEXITCODE" }
    $runDirectory = Join-Path $LabRoot ("{0}_{1}" -f ($database.name -replace '[^A-Za-z0-9_.-]', '_'), $childRunId)
    [void]$matrixPaths.Add((Join-Path $runDirectory "matrix.json"))
}

$mergeArgs = @("source-diff-matrix-merge") + @($matrixPaths) + @("--output", (Join-Path $matrixRoot "parity-matrix.json"), "--markdown", (Join-Path $matrixRoot "parity-matrix.md"))
& $ExePath @mergeArgs
if ($LASTEXITCODE -ne 0) { throw "source-diff-matrix-merge failed with exit code $LASTEXITCODE" }

Write-Host "Two-database parity matrix completed: $matrixRoot"
