[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$DbName,
    [string]$DbServer = "localhost",
    [string]$DbUser = "sa",
    [string]$LabRoot = "E:\ibcmd_lab\parity",
    [string]$RunId = (Get-Date -Format "yyyyMMdd_HHmmss"),
    [string]$ExePath = "",
    [string]$IbcmdPath = "",
    [ValidateSet("2.20", "2.21")][string]$SourceVersion = "2.20",
    [ValidateSet("full", "scoped")][string]$Scope = "full",
    [string[]]$PathPrefix = @()
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# Keep CLI spelling in one place.  A platform-oracle build supplies dump-sources;
# changing its future flags only requires changing this map, not the run protocol.
$Cli = [ordered]@{
    NativeExport = "dump-sources"
    CandidateExport = "mssql-dump-config"
    Diff = "source-diff"
    Signatures = "source-diff-signatures"
    Matrix = "source-diff-matrix"
    MatrixMerge = "source-diff-matrix-merge"
}

function Get-RepoSha {
    param([string]$RepoRoot)
    $sha = & git -C $RepoRoot rev-parse HEAD 2>$null
    if ($LASTEXITCODE -ne 0) { throw "Cannot determine git SHA in $RepoRoot" }
    return $sha.Trim()
}

function Test-CliCommand {
    param([string]$Exe, [string]$Command)
    # A missing subcommand is expected during preflight; do not let native stderr
    # bypass the actionable platform-oracle build hint below.
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $help = & $Exe $Command "--help" 2>&1 | Out-String
        return ($LASTEXITCODE -eq 0 -and $help -match [regex]::Escape($Command))
    } finally {
        $ErrorActionPreference = $previousPreference
    }
}

function Invoke-ParityStep {
    param([string]$Name, [string]$LogPath, [scriptblock]$Action, [System.Collections.ArrayList]$Steps)
    $started = (Get-Date).ToUniversalTime().ToString("o")
    & $Action *>&1 | Tee-Object -FilePath $LogPath | Write-Host
    $exitCode = $LASTEXITCODE
    $ended = (Get-Date).ToUniversalTime().ToString("o")
    [void]$Steps.Add([ordered]@{ name=$Name; started_utc=$started; ended_utc=$ended; exit_code=$exitCode; log=(Split-Path $LogPath -Leaf) })
    if ($exitCode -ne 0) { throw "$Name failed with exit code $exitCode (see $LogPath)" }
}

function Write-Manifest {
    param([string]$Path, [hashtable]$Manifest)
    # Deliberately never include process environment, --sql-pwd, or IBCMD_DB_PSW.
    $Manifest | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $Path -Encoding utf8
}

$repoRoot = Split-Path -Parent $PSScriptRoot
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
if ([string]::IsNullOrWhiteSpace($ExePath)) { $ExePath = Join-Path $repoRoot "target\release\ibcmd-rs.exe" }
$ExePath = [IO.Path]::GetFullPath($ExePath)
if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
    throw "Missing executable: $ExePath. Build it with: cargo build --release --features platform-oracle"
}
foreach ($command in @($Cli.NativeExport, $Cli.CandidateExport, $Cli.Diff, $Cli.Signatures, $Cli.Matrix, $Cli.MatrixMerge)) {
    if (-not (Test-CliCommand -Exe $ExePath -Command $command)) {
        throw "Required command '$command' is unavailable in $ExePath. Build it with: cargo build --release --features platform-oracle"
    }
}
if (-not $env:IBCMD_DB_PSW) { throw "IBCMD_DB_PSW must be set for SQL authentication (its value is never recorded)." }

$safeDb = ($DbName -replace '[^A-Za-z0-9_.-]', '_')
$runRoot = Join-Path $LabRoot ("{0}_{1}" -f $safeDb, $RunId)
if (Test-Path -LiteralPath $runRoot) { throw "Run directory already exists and is immutable: $runRoot" }

$nativeRoot = Join-Path $runRoot "native"
$candidateDumpRoot = Join-Path $runRoot "candidate_dump"
$candidateRoot = Join-Path $runRoot "candidate"
$logsRoot = Join-Path $runRoot "logs"
New-Item -ItemType Directory -Path $runRoot, $logsRoot -ErrorAction Stop | Out-Null

$steps = [System.Collections.ArrayList]::new()
$manifestPath = Join-Path $runRoot "parity-manifest.json"
$manifest = [ordered]@{
    protocol_version = 1
    run_id = $RunId
    scope = $Scope
    created_utc = (Get-Date).ToUniversalTime().ToString("o")
    git_sha = Get-RepoSha -RepoRoot $repoRoot
    database = [ordered]@{ name=$DbName; server=$DbServer; user=$DbUser; password_source="env:IBCMD_DB_PSW (redacted)" }
    source_version = $SourceVersion
    executable = $ExePath
    layout = [ordered]@{ native="native"; candidate_dump="candidate_dump"; candidate="candidate" }
    commands = [ordered]@{
        native_export = $Cli.NativeExport; candidate_export = $Cli.CandidateExport; diff = $Cli.Diff; signatures = $Cli.Signatures
        matrix = $Cli.Matrix; matrix_merge = $Cli.MatrixMerge
    }
    path_prefixes = @($PathPrefix)
    steps = $steps
    status = "running"
}
Write-Manifest -Path $manifestPath -Manifest $manifest

try {
    $nativeArgs = @($Cli.NativeExport, "--dbms", "MSSQLServer", "--db-server", $DbServer, "--db-name", $DbName, "--db-user", $DbUser, "-o", $nativeRoot, "--overwrite")
    if ($IbcmdPath) { $nativeArgs += @("--ibcmd", $IbcmdPath) }
    Invoke-ParityStep -Name "native-export" -LogPath (Join-Path $logsRoot "native-export.log") -Steps $steps -Action { & $ExePath @nativeArgs }

    $candidateArgs = @($Cli.CandidateExport, "--database", $DbName, "--server", $DbServer, "--sql-user", $DbUser, "--sql-pwd-env", "IBCMD_DB_PSW", "-o", $candidateDumpRoot, "--overwrite", "--inflate", "--extract-module-text", "--extract-metadata-xml", "--source-version", $SourceVersion, "--no-binary-rows")
    if ($Scope -eq "full") {
        $candidateArgs += "--require-complete-root-metadata"
    }
    Invoke-ParityStep -Name "candidate-export" -LogPath (Join-Path $logsRoot "candidate-export.log") -Steps $steps -Action { & $ExePath @candidateArgs }

    # Copy only reconstructed source, never the raw storage payload or generated JSON.
    $roboArgs = @($candidateDumpRoot, $candidateRoot, "/E", "/XD", "Config_inflated", "Config_raw", "ConfigSave_inflated", "ConfigSave_raw", "/XF", "manifest.json", "*.json")
    Invoke-ParityStep -Name "candidate-source-layout" -LogPath (Join-Path $logsRoot "candidate-source-layout.log") -Steps $steps -Action {
        & robocopy @roboArgs | Out-Host
        if ($LASTEXITCODE -le 7) { $global:LASTEXITCODE = 0 }
    }

    $diffPath = Join-Path $runRoot "raw-diff.json"
    $diffArgs = @($Cli.Diff, "-o", $diffPath)
    foreach ($prefix in $PathPrefix) { $diffArgs += @("--path-prefix", $prefix) }
    $diffArgs += @($nativeRoot, $candidateRoot)
    Invoke-ParityStep -Name "raw-diff" -LogPath (Join-Path $logsRoot "raw-diff.log") -Steps $steps -Action { & $ExePath @diffArgs }

    $signaturesPath = Join-Path $runRoot "signatures.json"
    Invoke-ParityStep -Name "diff-signatures" -LogPath (Join-Path $logsRoot "diff-signatures.log") -Steps $steps -Action { & $ExePath $Cli.Signatures "-o" $signaturesPath $diffPath }

    $matrixPath = Join-Path $runRoot "matrix.json"
    $matrixMarkdownPath = Join-Path $runRoot "matrix.md"
    $matrixScopeArg = if ($Scope -eq "full") { "--full" } else { "--scoped" }
    $matrixArgs = @($Cli.Matrix, $diffPath, "--database", $DbName, "--run-id", $RunId, "--git-sha", $manifest.git_sha, $matrixScopeArg, "--output", $matrixPath, "--markdown", $matrixMarkdownPath)
    Invoke-ParityStep -Name "parity-matrix" -LogPath (Join-Path $logsRoot "parity-matrix.log") -Steps $steps -Action { & $ExePath @matrixArgs }
    $manifest.status = "passed"
} catch {
    $manifest.status = "failed"
    $manifest.failure = $_.Exception.Message
    throw
} finally {
    $manifest.finished_utc = (Get-Date).ToUniversalTime().ToString("o")
    Write-Manifest -Path $manifestPath -Manifest $manifest
}

Write-Host "Parity run completed: $runRoot"
