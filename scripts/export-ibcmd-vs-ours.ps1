[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$DbName,
    [string]$DbServer = "localhost",
    [string]$DbUser = "sa",
    [switch]$IntegratedAuth,
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

$Cli = [ordered]@{
    NativeExport = "dump-sources"; CandidateExport = "mssql-dump-config"; Diff = "source-diff"
    Signatures = "source-diff-signatures"; Matrix = "source-diff-matrix"; MatrixMerge = "source-diff-matrix-merge"
}

function Get-UtcNow { (Get-Date).ToUniversalTime().ToString("o") }

function Protect-SensitiveText {
    param([AllowNull()]$Value)
    $text = [string]$Value
    foreach ($secretName in @('IBCMD_DB_PSW', 'IBCMD_USER_PSW', 'SQLCMDPASSWORD')) {
        $text = [regex]::Replace(
            $text,
            [regex]::Escape($secretName),
            '<redacted-environment>',
            [Text.RegularExpressions.RegexOptions]::IgnoreCase
        )
        $secretValue = [Environment]::GetEnvironmentVariable($secretName, 'Process')
        if (-not [string]::IsNullOrEmpty($secretValue)) {
            $text = $text.Replace($secretValue, '<redacted>')
        }
    }
    return [regex]::Replace(
        $text,
        '(?i)(--(?:db-pwd|sql-pwd|password|pwd)(?:-env)?)(?:=|\s+)(?:"[^"]*"|\S+)',
        '$1=<redacted>'
    )
}

function ConvertTo-SanitizedArguments {
    param([string[]]$Arguments)
    $result = [System.Collections.Generic.List[string]]::new()
    $redactNext = $false
    foreach ($argument in $Arguments) {
        if ($redactNext) { $result.Add("<redacted>"); $redactNext = $false; continue }
        if ($argument -match '^(--(?:(?:sql|db)-)?pwd|--password|--.*password)$') { $result.Add($argument); $redactNext = $true; continue }
        if ($argument -match '^(--(?:(?:sql|db)-)?pwd|--password)=') { $result.Add(($argument -replace '=.*$', '=<redacted>')); continue }
        if ($argument -match '^--(?:(?:sql|db)-)?pwd-env$') { $result.Add($argument); $redactNext = $true; continue }
        if ($argument -match '^--(?:(?:sql|db)-)?pwd-env=') { $result.Add(($argument -replace '=.*$', '=<redacted-environment>')); continue }
        if ($argument -eq '-P') { $result.Add($argument); $redactNext = $true; continue }
        if ($argument -eq 'IBCMD_DB_PSW') { $result.Add('<redacted-environment>'); continue }
        $result.Add($argument)
    }
    return @($result)
}

function Assert-ManifestSafe {
    param([string]$Json)
    # Option names are retained for reproducibility, but environment names and values are not.
    $forbidden = @('IBCMD_DB_PSW', 'IBCMD_USER_PSW', 'SQLCMDPASSWORD')
    foreach ($secretName in @('IBCMD_DB_PSW', 'IBCMD_USER_PSW', 'SQLCMDPASSWORD')) {
        $secretValue = [Environment]::GetEnvironmentVariable($secretName, 'Process')
        if (-not [string]::IsNullOrEmpty($secretValue)) { $forbidden += $secretValue }
    }
    foreach ($value in $forbidden) {
        if ($Json.IndexOf($value, [StringComparison]::OrdinalIgnoreCase) -ge 0) { throw "Refusing to write secret-bearing manifest content." }
    }
    $null = $Json | ConvertFrom-Json -ErrorAction Stop
}

function Write-ManifestAtomic {
    param([string]$Path, [System.Collections.IDictionary]$Manifest)
    $json = $Manifest | ConvertTo-Json -Depth 20
    Assert-ManifestSafe -Json $json
    $tmp = "$Path.$([guid]::NewGuid().ToString('N')).tmp"
    $utf8 = [System.Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText($tmp, $json, $utf8)
    try {
        if (Test-Path -LiteralPath $Path) {
            $backup = "$Path.$([guid]::NewGuid().ToString('N')).bak"
            [IO.File]::Replace($tmp, $Path, $backup, $true)
            Remove-Item -LiteralPath $backup -Force -ErrorAction SilentlyContinue
        } else {
            [IO.File]::Move($tmp, $Path)
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
    }
}

function Get-CommandVersion {
    param([string]$Path, [string[]]$Arguments = @('--version'), [int[]]$AllowedExitCodes = @(0))
    $previousPreference = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    try {
        $output = & $Path @Arguments 2>&1 | Out-String
        if ($AllowedExitCodes -notcontains $LASTEXITCODE) { throw "Cannot read version of '$Path' (exit $LASTEXITCODE)." }
        $lines = @($output -split "\r?\n" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -First 8)
        return ($lines -join ' | ')
    } finally { $ErrorActionPreference = $previousPreference }
}

function Get-ApplicationPath {
    param([string]$Name)
    $command = Get-Command -Name $Name -CommandType Application -ErrorAction Stop | Select-Object -First 1
    return $command.Source
}

function Get-Sha256Text {
    param([string]$Text)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes($Text)
        return (($sha.ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') }) -join '')
    } finally {
        $sha.Dispose()
    }
}

function Get-FileSha256 {
    param([string]$Path)
    $stream = [IO.File]::OpenRead($Path)
    try {
        $sha = [Security.Cryptography.SHA256]::Create()
        try {
            return (($sha.ComputeHash($stream) | ForEach-Object { $_.ToString('x2') }) -join '')
        } finally {
            $sha.Dispose()
        }
    } finally {
        $stream.Dispose()
    }
}

function Get-RepositoryState {
    param([string]$RepoRoot)
    $lines = @(& git -C $RepoRoot status --porcelain=v1 --untracked-files=all)
    if ($LASTEXITCODE -ne 0) { throw "Cannot determine repository state in $RepoRoot" }
    return [ordered]@{
        status = if ($lines.Count -eq 0) { 'clean' } else { 'dirty' }
        dirty_entries = $lines.Count
        porcelain = @($lines)
        command = @('git', '-C', $RepoRoot, 'status', '--porcelain=v1', '--untracked-files=all')
    }
}

function Get-DatabaseFingerprintCommand {
    param(
        [string]$SqlcmdPath,
        [string]$Server,
        [string]$Database,
        [bool]$UseIntegratedAuth,
        [string]$SqlUser
    )
    $escapedDatabase = $Database.Replace(']', ']]')
    $query = @"
SET NOCOUNT ON;
SELECT StorageTable, FileName, PartNo, DataSize,
       CONVERT(varchar(64), HASHBYTES('SHA2_256',
           CONVERT(varbinary(max), StorageTable) + 0x00 +
           CONVERT(varbinary(max), FileName) + 0x00 +
           ISNULL(CONVERT(varbinary(max), Creation), 0x) + 0x00 +
           ISNULL(CONVERT(varbinary(max), Modified), 0x) + 0x00 +
           ISNULL(CONVERT(varbinary(max), Attributes), 0x) + 0x00 +
           CONVERT(binary(8), CONVERT(bigint, PartNo)) +
           CONVERT(binary(8), CONVERT(bigint, DataSize)) +
           ISNULL(CONVERT(varbinary(max), BinaryData), 0x)), 2) AS RowHash
FROM (
    SELECT N'Config' AS StorageTable, FileName, Creation, Modified, Attributes, PartNo, DataSize, BinaryData
    FROM [$escapedDatabase].dbo.Config
    UNION ALL
    SELECT N'ConfigSave' AS StorageTable, FileName, Creation, Modified, Attributes, PartNo, DataSize, BinaryData
    FROM [$escapedDatabase].dbo.ConfigSave
) AS StorageRows
ORDER BY StorageTable, FileName, PartNo;
"@
    $arguments = @('-S', $Server, '-C', '-h', '-1', '-W', '-b')
    if ($UseIntegratedAuth) { $arguments += '-E' } else { $arguments += @('-U', $SqlUser) }
    $arguments += @('-Q', $query)
    return [ordered]@{ executable=$SqlcmdPath; arguments=$arguments; query=$query }
}

function Get-DatabaseFingerprint {
    param(
        [string]$SqlcmdPath,
        [string]$Server,
        [string]$Database,
        [bool]$UseIntegratedAuth,
        [string]$SqlUser
    )
    $startedUtc = Get-UtcNow
    $command = Get-DatabaseFingerprintCommand -SqlcmdPath $SqlcmdPath -Server $Server -Database $Database -UseIntegratedAuth $UseIntegratedAuth -SqlUser $SqlUser
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $savedSqlcmdPassword = [Environment]::GetEnvironmentVariable('SQLCMDPASSWORD', 'Process')
    try {
        if (-not $UseIntegratedAuth) {
            [Environment]::SetEnvironmentVariable('SQLCMDPASSWORD', $env:IBCMD_DB_PSW, 'Process')
        }
        $output = @(& $command.executable @($command.arguments) 2>&1)
        $exitCode = $LASTEXITCODE
    } finally {
        [Environment]::SetEnvironmentVariable('SQLCMDPASSWORD', $savedSqlcmdPassword, 'Process')
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) { throw "Database fingerprint query failed with exit code $exitCode." }
    $lines = @($output | ForEach-Object { [string]$_ } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $canonical = ($lines -join "`n")
    return [ordered]@{
        algorithm = 'sha256-per-row+sha256-manifest'
        tables = @('Config', 'ConfigSave')
        row_count = $lines.Count
        sha256 = Get-Sha256Text $canonical
        started_utc = $startedUtc
        ended_utc = Get-UtcNow
        executable = $SqlcmdPath
        arguments = ConvertTo-SanitizedArguments $command.arguments
    }
}

function Get-TreeSummaryFromDiff {
    param([string]$DiffPath, [ValidateSet('left', 'right')][string]$Side)
    $report = Get-Content -Raw -LiteralPath $DiffPath | ConvertFrom-Json -ErrorAction Stop
    $shaProperty = "${Side}_sha256"
    $sizeProperty = "${Side}_size_bytes"
    $entries = @($report.differences | Where-Object { $null -ne $_.$shaProperty } | Sort-Object path)
    $builder = [Text.StringBuilder]::new()
    [long]$totalBytes = 0
    foreach ($entry in $entries) {
        [void]$builder.Append([string]$entry.path).Append("`0").Append([string]$entry.$shaProperty).Append("`0").Append([string]$entry.$sizeProperty).Append("`n")
        $totalBytes += [long]$entry.$sizeProperty
    }
    return [ordered]@{
        algorithm = 'sha256(path+nul+file_sha256+nul+size+lf)'
        file_count = $entries.Count
        total_bytes = $totalBytes
        sha256 = Get-Sha256Text $builder.ToString()
    }
}

function Invoke-WithoutSqlCredentialEnvironment {
    param([scriptblock]$Action)
    $savedUser = [Environment]::GetEnvironmentVariable('IBCMD_DB_USR', 'Process')
    $savedPassword = [Environment]::GetEnvironmentVariable('IBCMD_DB_PSW', 'Process')
    try {
        [Environment]::SetEnvironmentVariable('IBCMD_DB_USR', $null, 'Process')
        [Environment]::SetEnvironmentVariable('IBCMD_DB_PSW', $null, 'Process')
        & $Action
        $wrappedExitCode = $LASTEXITCODE
    } finally {
        [Environment]::SetEnvironmentVariable('IBCMD_DB_USR', $savedUser, 'Process')
        [Environment]::SetEnvironmentVariable('IBCMD_DB_PSW', $savedPassword, 'Process')
    }
    $global:LASTEXITCODE = $wrappedExitCode
}

function Get-ResolvedIbcmdPath {
    param([string]$ExplicitPath)
    if ($ExplicitPath) { return [IO.Path]::GetFullPath($ExplicitPath) }
    if ($env:IBCMD_PATH -and (Test-Path -LiteralPath $env:IBCMD_PATH -PathType Leaf)) { return [IO.Path]::GetFullPath($env:IBCMD_PATH) }
    $preferred = 'C:\Program Files\1cv8\8.3.27.1989\bin\ibcmd.exe'
    if (Test-Path -LiteralPath $preferred -PathType Leaf) { return $preferred }
    $candidates = @()
    $programFilesRoots = @($env:ProgramFiles, ${env:ProgramFiles(x86)}) | Where-Object { $_ }
    foreach ($base in $programFilesRoots) {
        $root = Join-Path $base '1cv8'
        if (Test-Path -LiteralPath $root -PathType Container) {
            $candidates += Get-ChildItem -LiteralPath $root -Directory | ForEach-Object {
                $candidate = Join-Path $_.FullName 'bin\ibcmd.exe'
                if (Test-Path -LiteralPath $candidate -PathType Leaf) { $candidate }
            }
        }
    }
    if ($candidates.Count -gt 0) {
        return ($candidates | Sort-Object -Descending @{ Expression = {
            $versionName = Split-Path (Split-Path $_ -Parent) -Parent | Split-Path -Leaf
            try { [version]$versionName } catch { [version]'0.0' }
        } })[0]
    }
    return 'ibcmd'
}

function Test-CliCommand {
    param([string]$Exe, [string]$Command)
    $previousPreference = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    try {
        $help = & $Exe $Command "--help" 2>&1 | Out-String
        return ($LASTEXITCODE -eq 0 -and $help -match [regex]::Escape($Command))
    } finally { $ErrorActionPreference = $previousPreference }
}

function Invoke-ParityStep {
    param(
        [string]$Name, [string]$LogPath, [string[]]$Arguments, [string[]]$Artifacts,
        [scriptblock]$Action, [System.Collections.ArrayList]$Steps, [System.Collections.IDictionary]$Manifest, [string]$ManifestPath
    )
    $record = [ordered]@{ name=$Name; status='running'; started_utc=(Get-UtcNow); ended_utc=$null; exit_code=$null; exception=$null; log=(Split-Path $LogPath -Leaf); arguments=(ConvertTo-SanitizedArguments $Arguments); artifacts=@($Artifacts) }
    [void]$Steps.Add($record)
    Write-ManifestAtomic -Path $ManifestPath -Manifest $Manifest
    $previousPreference = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    $capturedError = $null
    $exitCode = $null
    try {
        & $Action *>&1 |
            ForEach-Object { Protect-SensitiveText $_ } |
            Tee-Object -FilePath $LogPath |
            Write-Host
        $exitCode = $LASTEXITCODE
    } catch {
        $capturedError = $_
        $record.exception = Protect-SensitiveText $_.Exception.Message
    } finally {
        $ErrorActionPreference = $previousPreference
        if ($null -eq $exitCode) { $exitCode = -1 }
        $record.ended_utc = Get-UtcNow
        $record.exit_code = $exitCode
        if ($null -ne $capturedError -or $exitCode -ne 0) { $record.status = 'failed' } else { $record.status = 'passed' }
        Write-ManifestAtomic -Path $ManifestPath -Manifest $Manifest
    }
    if ($null -ne $capturedError) { throw $capturedError }
    if ($exitCode -ne 0) { throw "$Name failed with exit code $exitCode (see $LogPath)" }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
if ($RunId -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$' -or $RunId.Contains('..')) { throw "RunId must be 1-128 safe characters (A-Z, a-z, 0-9, dot, underscore, hyphen), start with an alphanumeric character, and not contain '..'." }
if (@($PathPrefix | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -ne 0) { throw "PathPrefix entries must be non-empty, non-whitespace strings." }
if ($Scope -eq 'full' -and $PathPrefix.Count -ne 0) { throw "Scope 'full' requires an empty PathPrefix. Use -Scope scoped for a partial comparison." }
if ($Scope -eq 'scoped' -and $PathPrefix.Count -eq 0) { throw "Scope 'scoped' requires at least one PathPrefix." }
if ($RequireCompleteRootMetadata -and $Scope -ne 'full') { throw "RequireCompleteRootMetadata is available only for Scope 'full'." }
if ([string]::IsNullOrWhiteSpace($ExePath)) { $ExePath = Join-Path $repoRoot 'target\release\ibcmd-rs.exe' }
$ExePath = [IO.Path]::GetFullPath($ExePath)
if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) { throw "Missing executable: $ExePath. Build it with: cargo build --release --features platform-oracle" }
if ($IntegratedAuth -and -not [string]::IsNullOrWhiteSpace($DbUser) -and $DbUser -ne 'sa') { throw 'IntegratedAuth cannot be combined with a non-default DbUser. Pass -DbUser "".' }
if ([string]::IsNullOrWhiteSpace($DbUser)) { $IntegratedAuth = $true }
if (-not $IntegratedAuth -and -not $env:IBCMD_DB_PSW) { throw 'IBCMD_DB_PSW must be set for SQL authentication (its value is never recorded).' }
$authMode = if ($IntegratedAuth) { 'integrated' } else { 'sql' }
$manifestDbUser = if ($IntegratedAuth) { $null } else { $DbUser }
$manifestPasswordSource = if ($IntegratedAuth) { $null } else { 'environment (redacted)' }

$safeDb = ($DbName -replace '[^A-Za-z0-9_.-]', '_'); $runRoot = Join-Path $LabRoot ("{0}_{1}" -f $safeDb, $RunId)
if (Test-Path -LiteralPath $runRoot) { throw "Run directory already exists and is immutable: $runRoot" }
$nativeRoot = Join-Path $runRoot 'native'; $candidateDumpRoot = Join-Path $runRoot 'candidate_dump'; $candidateRoot = Join-Path $runRoot 'candidate'; $logsRoot = Join-Path $runRoot 'logs'
New-Item -ItemType Directory -Path $runRoot, $logsRoot -ErrorAction Stop | Out-Null
$steps = [System.Collections.ArrayList]::new(); $manifestPath = Join-Path $runRoot 'parity-manifest.json'
$manifest = [ordered]@{
    protocol_version=2; run_id=$RunId; scope=$Scope; created_utc=(Get-UtcNow); status='initializing'; git_sha=$null; xml_version=$SourceVersion; source_version=$SourceVersion
    database=[ordered]@{ name=$DbName; server=$DbServer; auth_mode=$authMode; user=$manifestDbUser; password_source=$manifestPasswordSource }
    tools=[ordered]@{}; layout=[ordered]@{ native='native'; candidate_dump='candidate_dump'; candidate='candidate' }; path_prefixes=@($PathPrefix); steps=$steps; artifacts=[ordered]@{}
}
# This is intentionally the first persistent action after directory creation: every later external command is journaled.
Write-ManifestAtomic -Path $manifestPath -Manifest $manifest

try {
    $gitPath = Get-ApplicationPath 'git'
    $manifest.git_sha = (& $gitPath -C $repoRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw "Cannot determine git SHA in $repoRoot" }
    $manifest.tools.git = [ordered]@{
        path=$gitPath
        version=((& $gitPath --version).Trim())
        sha256=(Get-FileSha256 -Path $gitPath)
    }
    $manifest.repository = Get-RepositoryState -RepoRoot $repoRoot
    $manifest.tools.candidate = [ordered]@{ path=$ExePath; version=(Get-CommandVersion $ExePath); sha256=(Get-FileSha256 -Path $ExePath) }
    $resolvedIbcmd = Get-ResolvedIbcmdPath $IbcmdPath
    $resolvedIbcmdFile = Get-Command -Name $resolvedIbcmd -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1
    $resolvedIbcmdHash = if (Test-Path -LiteralPath $resolvedIbcmdFile.Source -PathType Leaf) { Get-FileSha256 -Path $resolvedIbcmdFile.Source } else { $null }
    $manifest.tools.native_ibcmd = [ordered]@{ path=$resolvedIbcmdFile.Source; version=(Get-CommandVersion $resolvedIbcmdFile.Source); sha256=$resolvedIbcmdHash }
    $resolvedIbcmd = $resolvedIbcmdFile.Source
    $sqlcmdPath = Get-ApplicationPath 'sqlcmd'
    $bcpPath = Join-Path (Split-Path -Parent $sqlcmdPath) 'bcp.exe'
    if (-not (Test-Path -LiteralPath $bcpPath -PathType Leaf)) { $bcpPath = Get-ApplicationPath 'bcp' }
    $robocopyPath = Get-ApplicationPath 'robocopy'
    $manifest.tools.sqlcmd = [ordered]@{
        path=$sqlcmdPath
        version=(Get-CommandVersion $sqlcmdPath @('-?'))
        sha256=(Get-FileSha256 -Path $sqlcmdPath)
    }
    $manifest.tools.bcp = [ordered]@{
        path=$bcpPath
        version=(Get-CommandVersion $bcpPath @('-v'))
        sha256=(Get-FileSha256 -Path $bcpPath)
    }
    $manifest.tools.robocopy = [ordered]@{
        path=$robocopyPath
        version=(Get-CommandVersion $robocopyPath @('/?') @(16))
        sha256=(Get-FileSha256 -Path $robocopyPath)
    }
    $beforeFingerprintCommand = Get-DatabaseFingerprintCommand -SqlcmdPath $sqlcmdPath -Server $DbServer -Database $DbName -UseIntegratedAuth $IntegratedAuth -SqlUser $DbUser
    $manifest.database_fingerprint = [ordered]@{
        before = [ordered]@{
            status='running'
            started_utc=(Get-UtcNow)
            executable=$beforeFingerprintCommand.executable
            arguments=(ConvertTo-SanitizedArguments $beforeFingerprintCommand.arguments)
        }
        after = $null
        unchanged = $null
    }
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    foreach ($command in @($Cli.NativeExport, $Cli.CandidateExport, $Cli.Diff, $Cli.Signatures, $Cli.Matrix, $Cli.MatrixMerge)) {
        if (-not (Test-CliCommand -Exe $ExePath -Command $command)) { throw "Required command '$command' is unavailable in $ExePath. Build it with: cargo build --release --features platform-oracle" }
    }
    try {
        $beforeFingerprint = Get-DatabaseFingerprint -SqlcmdPath $sqlcmdPath -Server $DbServer -Database $DbName -UseIntegratedAuth $IntegratedAuth -SqlUser $DbUser
        $manifest.database_fingerprint.before = $beforeFingerprint
        $manifest.database_fingerprint.before.status = 'passed'
    } catch {
        $manifest.database_fingerprint.before.status = 'failed'
        $manifest.database_fingerprint.before.ended_utc = Get-UtcNow
        $manifest.database_fingerprint.before.exception = $_.Exception.Message
    }
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest

    $nativeArgs = @($Cli.NativeExport, '--dbms', 'MSSQLServer', '--db-server', $DbServer, '--db-name', $DbName, '-o', $nativeRoot, '--overwrite', '--ibcmd', $resolvedIbcmd)
    if (-not $IntegratedAuth) { $nativeArgs += @('--db-user', $DbUser, '--db-pwd-env', 'IBCMD_DB_PSW') }
    $nativeAction = { & $ExePath @nativeArgs }
    if ($IntegratedAuth) {
        Invoke-ParityStep -Name 'native-export' -LogPath (Join-Path $logsRoot 'native-export.log') -Arguments $nativeArgs -Artifacts @('native') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { Invoke-WithoutSqlCredentialEnvironment $nativeAction }
    } else {
        Invoke-ParityStep -Name 'native-export' -LogPath (Join-Path $logsRoot 'native-export.log') -Arguments $nativeArgs -Artifacts @('native') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action $nativeAction
    }

    $candidateArgs = @($Cli.CandidateExport, '--database', $DbName, '--server', $DbServer, '--sqlcmd', $sqlcmdPath, '-o', $candidateDumpRoot, '--overwrite', '--inflate', '--extract-module-text', '--extract-metadata-xml', '--source-version', $SourceVersion, '--no-binary-rows')
    if (-not $IntegratedAuth) { $candidateArgs += @('--sql-user', $DbUser, '--sql-pwd-env', 'IBCMD_DB_PSW') }
    if ($RequireCompleteRootMetadata) { $candidateArgs += '--require-complete-root-metadata' }
    Invoke-ParityStep -Name 'candidate-export' -LogPath (Join-Path $logsRoot 'candidate-export.log') -Arguments $candidateArgs -Artifacts @('candidate_dump/manifest.json') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & $ExePath @candidateArgs }

    $roboArgs = @($candidateDumpRoot, $candidateRoot, '/E', '/XD', 'Config_inflated', 'Config_raw', 'ConfigSave_inflated', 'ConfigSave_raw', '/XF', 'manifest.json', '*.json')
    Invoke-ParityStep -Name 'candidate-source-layout' -LogPath (Join-Path $logsRoot 'candidate-source-layout.log') -Arguments $roboArgs -Artifacts @('candidate') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & robocopy @roboArgs | Out-Host; if ($LASTEXITCODE -le 7) { $global:LASTEXITCODE = 0 } }

    $diffPath = Join-Path $runRoot 'raw-diff.json'; $diffArgs = @($Cli.Diff, '-o', $diffPath)
    foreach ($prefix in $PathPrefix) { $diffArgs += @('--path-prefix', $prefix) }; $diffArgs += @($nativeRoot, $candidateRoot)
    Invoke-ParityStep -Name 'raw-diff' -LogPath (Join-Path $logsRoot 'raw-diff.log') -Arguments $diffArgs -Artifacts @('raw-diff.json') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & $ExePath @diffArgs }
    $manifest.tree_summaries = [ordered]@{
        native = Get-TreeSummaryFromDiff -DiffPath $diffPath -Side left
        candidate = Get-TreeSummaryFromDiff -DiffPath $diffPath -Side right
    }
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    $signaturesPath = Join-Path $runRoot 'signatures.json'; $signatureArgs = @($Cli.Signatures, '-o', $signaturesPath, $diffPath)
    Invoke-ParityStep -Name 'diff-signatures' -LogPath (Join-Path $logsRoot 'diff-signatures.log') -Arguments $signatureArgs -Artifacts @('signatures.json') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & $ExePath @signatureArgs }
    $matrixPath = Join-Path $runRoot 'matrix.json'; $matrixMarkdownPath = Join-Path $runRoot 'matrix.md'; $matrixScopeArg = if ($Scope -eq 'full') { '--full' } else { '--scoped' }
    $matrixArgs = @($Cli.Matrix, $diffPath, '--database', $DbName, '--run-id', $RunId, '--git-sha', $manifest.git_sha, $matrixScopeArg, '--output', $matrixPath, '--markdown', $matrixMarkdownPath)
    Invoke-ParityStep -Name 'parity-matrix' -LogPath (Join-Path $logsRoot 'parity-matrix.log') -Arguments $matrixArgs -Artifacts @('matrix.json', 'matrix.md') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & $ExePath @matrixArgs }
    $afterFingerprintCommand = Get-DatabaseFingerprintCommand -SqlcmdPath $sqlcmdPath -Server $DbServer -Database $DbName -UseIntegratedAuth $IntegratedAuth -SqlUser $DbUser
    $manifest.database_fingerprint.after = [ordered]@{
        status='running'
        started_utc=(Get-UtcNow)
        executable=$afterFingerprintCommand.executable
        arguments=(ConvertTo-SanitizedArguments $afterFingerprintCommand.arguments)
    }
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    try {
        $afterFingerprint = Get-DatabaseFingerprint -SqlcmdPath $sqlcmdPath -Server $DbServer -Database $DbName -UseIntegratedAuth $IntegratedAuth -SqlUser $DbUser
        $manifest.database_fingerprint.after = $afterFingerprint
        $manifest.database_fingerprint.after.status = 'passed'
    } catch {
        $manifest.database_fingerprint.after.status = 'failed'
        $manifest.database_fingerprint.after.ended_utc = Get-UtcNow
        $manifest.database_fingerprint.after.exception = $_.Exception.Message
        throw
    }
    if ($manifest.database_fingerprint.before.status -ne 'passed') { throw 'Database fingerprint before export is unavailable; run is invalid.' }
    $manifest.database_fingerprint.unchanged = ($manifest.database_fingerprint.before.sha256 -eq $manifest.database_fingerprint.after.sha256)
    if (-not $manifest.database_fingerprint.unchanged) { throw 'Database configuration storage changed during parity export.' }
    if ($Scope -eq 'full' -and $manifest.repository.status -ne 'clean') { throw 'Full release parity requires a clean Git repository.' }
    $manifest.artifacts = [ordered]@{ raw_diff='raw-diff.json'; signatures='signatures.json'; matrix='matrix.json'; markdown='matrix.md' }
    $manifest.artifact_sha256 = [ordered]@{
        raw_diff=(Get-FileSha256 -Path $diffPath)
        signatures=(Get-FileSha256 -Path $signaturesPath)
        matrix=(Get-FileSha256 -Path $matrixPath)
        markdown=(Get-FileSha256 -Path $matrixMarkdownPath)
    }
    $manifest.status='passed'
} catch { $manifest.status='failed'; $manifest.failure=(Protect-SensitiveText $_.Exception.Message); throw
} finally { $manifest.finished_utc=Get-UtcNow; Write-ManifestAtomic -Path $manifestPath -Manifest $manifest }

Write-Host "Parity run completed: $runRoot"
