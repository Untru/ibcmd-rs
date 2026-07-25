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
    [string]$SqlcmdExecutable = "",
    [string]$BcpExecutable = "",
    [ValidateRange(1, 86400)][int]$NativeTimeoutSec = 900,
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
    $secretNames = @('IBCMD_DB_PSW', 'IBCMD_USER_PSW', 'SQLCMDPASSWORD')
    $secretValues = @($secretNames |
        ForEach-Object { [Environment]::GetEnvironmentVariable($_, 'Process') } |
        Where-Object { -not [string]::IsNullOrEmpty($_) } |
        Sort-Object -Property @{ Expression = { $_.Length }; Descending = $true }, @{ Expression = { $_ }; Descending = $false } -Unique)
    foreach ($secretValue in $secretValues) {
        $text = $text.Replace($secretValue, '<redacted>')
    }
    foreach ($secretName in $secretNames) {
        $text = [regex]::Replace(
            $text,
            [regex]::Escape($secretName),
            '<redacted-environment>',
            [Text.RegularExpressions.RegexOptions]::IgnoreCase
        )
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
    param([string]$Path, [object]$Manifest)
    $json = $Manifest | ConvertTo-Json -Depth 20
    Assert-ManifestSafe -Json $json
    $tmp = "$Path.$([guid]::NewGuid().ToString('N')).tmp"
    $utf8 = [System.Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText($tmp, $json, $utf8)
    try {
        $attempt = 0
        while ($true) {
            $backup = $null
            try {
                if (Test-Path -LiteralPath $Path) {
                    $backup = "$Path.$([guid]::NewGuid().ToString('N')).bak"
                    [IO.File]::Replace($tmp, $Path, $backup, $true)
                } else {
                    [IO.File]::Move($tmp, $Path)
                }
                break
            } catch [IO.IOException] {
                $attempt++
                if ($attempt -ge 20) { throw }
                Start-Sleep -Milliseconds ([Math]::Min(200, 10 * $attempt))
            } catch [UnauthorizedAccessException] {
                $attempt++
                if ($attempt -ge 20) { throw }
                Start-Sleep -Milliseconds ([Math]::Min(200, 10 * $attempt))
            } finally {
                if ($null -ne $backup) {
                    Remove-Item -LiteralPath $backup -Force -ErrorAction SilentlyContinue
                }
            }
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
    }
}

function Register-ManifestTool {
    param(
        [string]$Name,
        [string]$Path,
        [string[]]$VersionArguments,
        [int[]]$AllowedExitCodes = @(0),
        [System.Collections.IDictionary]$Manifest,
        [string]$ManifestPath
    )
    $record = [ordered]@{
        status = 'running'
        started_utc = Get-UtcNow
        ended_utc = $null
        path = $Path
        version_arguments = ConvertTo-SanitizedArguments $VersionArguments
        version = $null
        sha256 = $null
        exception = $null
    }
    $Manifest.tools[$Name] = $record
    Write-ManifestAtomic -Path $ManifestPath -Manifest $Manifest
    try {
        $record.version = Protect-SensitiveText (Get-CommandVersion -Path $Path -Arguments $VersionArguments -AllowedExitCodes $AllowedExitCodes)
        $record.sha256 = Get-FileSha256 -Path $Path
        $record.status = 'passed'
    } catch {
        $record.status = 'failed'
        $record.exception = Protect-SensitiveText $_.Exception.Message
        throw
    } finally {
        $record.ended_utc = Get-UtcNow
        Write-ManifestAtomic -Path $ManifestPath -Manifest $Manifest
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

function Get-ExplicitOrDiscoveredApplicationPath {
    param([string]$ExplicitPath, [string]$Name)
    if ([string]::IsNullOrWhiteSpace($ExplicitPath)) {
        return Get-ApplicationPath $Name
    }
    $resolved = Get-Command -Name $ExplicitPath -CommandType Application,ExternalScript -ErrorAction Stop |
        Select-Object -First 1
    return [IO.Path]::GetFullPath($resolved.Source)
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
    param([string]$RepoRoot, [string]$GitPath)
    $lines = @(& $GitPath -C $RepoRoot status --porcelain=v1 --untracked-files=all)
    if ($LASTEXITCODE -ne 0) { throw "Cannot determine repository state in $RepoRoot" }
    return [ordered]@{
        status = if ($lines.Count -eq 0) { 'clean' } else { 'dirty' }
        dirty_entries = $lines.Count
        porcelain = @($lines)
        command = @('git', '-C', $RepoRoot, 'status', '--porcelain=v1', '--untracked-files=all')
        executable = $GitPath
        arguments = @('-C', $RepoRoot, 'status', '--porcelain=v1', '--untracked-files=all')
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

function Get-NormalizedExecutablePath {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) { throw 'Runtime journal executable path is empty.' }
    $resolved = Get-Command -Name $Path -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1
    return [IO.Path]::GetFullPath($resolved.Source)
}

function Assert-CompleteRuntimeCall {
    param(
        [Parameter(Mandatory = $true)]$Call,
        [Parameter(Mandatory = $true)][string]$ExpectedExecutable,
        [Parameter(Mandatory = $true)][string]$ExpectedServer,
        [Parameter(Mandatory = $true)][string]$ExpectedDatabase,
        [Parameter(Mandatory = $true)][ValidateSet('native_ibcmd', 'sqlcmd', 'bcp')][string]$Kind,
        [ValidateSet('running', 'passed', 'failed')][string]$ExpectedStatus = 'passed'
    )
    foreach ($property in @('executable', 'arguments', 'started_unix_ms', 'ended_unix_ms', 'status', 'exit_code', 'timed_out', 'exception')) {
        if ($Call.PSObject.Properties.Name -notcontains $property) { throw "Runtime journal $Kind call is missing '$property'." }
    }
    if ((Get-NormalizedExecutablePath ([string]$Call.executable)) -ne (Get-NormalizedExecutablePath $ExpectedExecutable)) {
        throw "Runtime journal $Kind executable does not match the registered tool."
    }
    if ([string]$Call.status -ne $ExpectedStatus) {
        throw "Runtime journal $Kind call status does not match expected '$ExpectedStatus'."
    }
    if ($Call.timed_out -isnot [bool]) {
        throw "Runtime journal $Kind timed_out is not a JSON boolean."
    }
    if ($null -ne $Call.exception -and $Call.exception -isnot [string]) {
        throw "Runtime journal $Kind exception is neither null nor a string."
    }
    if ([decimal]$Call.started_unix_ms -le 0) {
        throw "Runtime journal $Kind timestamps are incomplete."
    }
    $hasException = -not [string]::IsNullOrWhiteSpace([string]$Call.exception)
    $hasExitCode = $null -ne $Call.exit_code
    if ($ExpectedStatus -eq 'running') {
        if ($null -ne $Call.ended_unix_ms -or $hasExitCode -or [bool]$Call.timed_out -or $hasException) {
            throw "Runtime journal $Kind running call already contains terminal fields."
        }
    } else {
        if ($null -eq $Call.ended_unix_ms -or [decimal]$Call.ended_unix_ms -lt [decimal]$Call.started_unix_ms) {
            throw "Runtime journal $Kind timestamps are incomplete."
        }
        if ($ExpectedStatus -eq 'passed') {
            if (-not $hasExitCode -or [int]$Call.exit_code -ne 0 -or [bool]$Call.timed_out -or $hasException) {
                throw "Runtime journal $Kind passed call is not coherent."
            }
        } elseif (-not ([bool]$Call.timed_out -or ($hasExitCode -and [int]$Call.exit_code -ne 0) -or $hasException)) {
            throw "Runtime journal $Kind failed call has no timeout, nonzero exit, or exception reason."
        }
    }
    $arguments = @($Call.arguments | ForEach-Object { [string]$_ })
    if ($arguments.Count -eq 0 -or @($arguments | Where-Object { [string]::IsNullOrEmpty($_) }).Count -ne 0) {
        throw "Runtime journal $Kind arguments are incomplete."
    }
    for ($index = 0; $index -lt $arguments.Count; $index++) {
            if ($arguments[$index] -ceq '-P') {
            if ($index + 1 -ge $arguments.Count -or $arguments[$index + 1] -notmatch '^<password-source:[a-z-]+>$') {
                throw "Runtime journal $Kind password argument is not source-marked."
            }
        }
    }
    if ($Kind -eq 'native_ibcmd') {
        if ($arguments.Count -lt 3 -or $arguments[0] -ne 'infobase' -or $arguments[1] -ne 'config' -or $arguments[2] -ne 'export') {
            throw 'Native ibcmd runtime journal does not contain the exact export command.'
        }
        $serverArguments = @($arguments | Where-Object { $_.StartsWith('--db-server=', [StringComparison]::Ordinal) })
        $databaseArguments = @($arguments | Where-Object { $_.StartsWith('--db-name=', [StringComparison]::Ordinal) })
        if ($serverArguments.Count -ne 1 -or
            -not [StringComparer]::Ordinal.Equals($serverArguments[0].Substring('--db-server='.Length), $ExpectedServer) -or
            $databaseArguments.Count -ne 1 -or
            -not [StringComparer]::Ordinal.Equals($databaseArguments[0].Substring('--db-name='.Length), $ExpectedDatabase)) {
            throw 'Native ibcmd runtime journal database identity does not match the requested server/database.'
        }
        if (@($arguments | Where-Object { $_ -match '^--(?:db-pwd|password)=' -and $_ -notmatch '=<password-source:[a-z-]+>$' }).Count -ne 0) {
            throw 'Native ibcmd runtime journal contains an unmarked password.'
        }
    } elseif ($Kind -eq 'sqlcmd') {
        $queryEvidence = $false
        for ($index = 0; $index -lt $arguments.Count; $index++) {
            if ($arguments[$index] -ceq '-Q') {
                if ($index + 1 -ge $arguments.Count -or $arguments[$index + 1] -notmatch '^<query-sha256:[0-9a-f]{64}>$') {
                    throw 'sqlcmd runtime journal contains a raw or invalid inline query.'
                }
                $queryEvidence = $true
            } elseif ($arguments[$index] -ceq '-i') {
                if ($index + 1 -ge $arguments.Count -or [string]::IsNullOrWhiteSpace($arguments[$index + 1])) {
                    throw 'sqlcmd runtime journal omits its temporary query path.'
                }
                $queryEvidence = $true
            }
        }
        if (-not $queryEvidence) { throw 'sqlcmd runtime journal has no query evidence.' }
    } else {
        if ($arguments.Count -lt 3 -or $arguments[0] -notmatch '^<query-sha256:[0-9a-f]{64}>$' -or $arguments[1] -ne 'queryout' -or [string]::IsNullOrWhiteSpace($arguments[2])) {
            throw 'bcp runtime journal does not contain a query marker and explicit output path.'
        }
    }
    if ($Kind -in @('sqlcmd', 'bcp')) {
        $serverValues = [System.Collections.Generic.List[string]]::new()
        for ($index = 0; $index -lt $arguments.Count; $index++) {
            if ($arguments[$index] -ceq '-S') {
                if ($index + 1 -ge $arguments.Count) { throw "$Kind runtime journal omits the -S value." }
                $serverValues.Add($arguments[$index + 1])
            }
        }
        if ($serverValues.Count -ne 1 -or -not [StringComparer]::Ordinal.Equals($serverValues[0], $ExpectedServer)) {
            throw "Runtime journal $Kind server does not match the requested server."
        }
    }
}

function Import-VerifiedRuntimeEvidence {
    param(
        [string]$NativeJournalPath,
        [string]$CandidateJournalPath,
        [string]$NativeIbcmdPath,
        [string]$SqlcmdPath,
        [string]$BcpPath,
        [string]$ExpectedServer,
        [string]$ExpectedDatabase
    )
    if (-not (Test-Path -LiteralPath $NativeJournalPath -PathType Leaf)) { throw 'Native export runtime journal is missing.' }
    if (-not (Test-Path -LiteralPath $CandidateJournalPath -PathType Leaf)) { throw 'Candidate subprocess journal is missing.' }

    $nativeJson = Get-Content -Raw -LiteralPath $NativeJournalPath
    Assert-ManifestSafe -Json $nativeJson
    $nativeJournal = $nativeJson | ConvertFrom-Json -ErrorAction Stop
    if ([int]$nativeJournal.protocol_version -ne 1 -or $nativeJournal.PSObject.Properties.Name -notcontains 'runtime_call') {
        throw 'Native export runtime journal protocol is invalid.'
    }
    Assert-CompleteRuntimeCall -Call $nativeJournal.runtime_call -ExpectedExecutable $NativeIbcmdPath `
        -ExpectedServer $ExpectedServer -ExpectedDatabase $ExpectedDatabase -Kind native_ibcmd

    $candidateJson = Get-Content -Raw -LiteralPath $CandidateJournalPath
    Assert-ManifestSafe -Json $candidateJson
    $candidateJournal = $candidateJson | ConvertFrom-Json -ErrorAction Stop
    if ([int]$candidateJournal.protocol_version -ne 1 -or [string]$candidateJournal.status -ne 'passed') {
        throw 'Candidate subprocess journal protocol/status is invalid.'
    }
    if (-not [StringComparer]::Ordinal.Equals([string]$candidateJournal.server, $ExpectedServer) -or
        -not [StringComparer]::Ordinal.Equals([string]$candidateJournal.database, $ExpectedDatabase)) {
        throw 'Candidate subprocess journal database identity does not match the requested server/database.'
    }
    $calls = @($candidateJournal.calls)
    if ($calls.Count -eq 0) { throw 'Candidate subprocess journal is empty.' }
    $sqlcmdCalls = 0
    $bcpCalls = 0
    foreach ($call in $calls) {
        $runtimeExecutable = Get-NormalizedExecutablePath ([string]$call.executable)
        if ($runtimeExecutable -eq (Get-NormalizedExecutablePath $SqlcmdPath)) {
            Assert-CompleteRuntimeCall -Call $call -ExpectedExecutable $SqlcmdPath `
                -ExpectedServer $ExpectedServer -ExpectedDatabase $ExpectedDatabase -Kind sqlcmd
            $sqlcmdCalls++
        } elseif ($runtimeExecutable -eq (Get-NormalizedExecutablePath $BcpPath)) {
            Assert-CompleteRuntimeCall -Call $call -ExpectedExecutable $BcpPath `
                -ExpectedServer $ExpectedServer -ExpectedDatabase $ExpectedDatabase -Kind bcp
            $bcpCalls++
        } else {
            throw "Candidate subprocess journal contains an unregistered executable: $($call.executable)"
        }
    }
    if ($sqlcmdCalls -eq 0 -or $bcpCalls -eq 0) { throw 'Candidate subprocess journal must contain passed sqlcmd and bcp calls.' }

    return [ordered]@{
        status='passed'
        native_report='logs/native-runtime.json'
        native_report_sha256=(Get-FileSha256 -Path $NativeJournalPath)
        native_call=$nativeJournal.runtime_call
        candidate_manifest='logs/candidate-runtime.json'
        candidate_manifest_sha256=(Get-FileSha256 -Path $CandidateJournalPath)
        candidate_subprocess_journal_status='passed'
        candidate_calls=$calls
        sqlcmd_calls=$sqlcmdCalls
        bcp_calls=$bcpCalls
    }
}

function Import-FailedRuntimeEvidence {
    param(
        [ValidateSet('native', 'candidate')][string]$JournalKind,
        [string]$JournalPath,
        [string]$NativeIbcmdPath,
        [string]$SqlcmdPath,
        [string]$BcpPath,
        [string]$ExpectedServer,
        [string]$ExpectedDatabase,
        [ValidateSet('failed', 'terminal')][string]$ExpectedStatus = 'failed'
    )
    if (-not (Test-Path -LiteralPath $JournalPath -PathType Leaf)) {
        throw "Failed $JournalKind runtime journal is missing."
    }
    $json = Get-Content -Raw -LiteralPath $JournalPath
    Assert-ManifestSafe -Json $json
    $journal = $json | ConvertFrom-Json -ErrorAction Stop
    if ([int]$journal.protocol_version -ne 1) { throw "Failed $JournalKind runtime journal protocol is invalid." }
    if ($JournalKind -eq 'native') {
        Assert-CompleteRuntimeCall -Call $journal.runtime_call -ExpectedExecutable $NativeIbcmdPath `
            -ExpectedServer $ExpectedServer -ExpectedDatabase $ExpectedDatabase -Kind native_ibcmd -ExpectedStatus failed
        return [ordered]@{
            status='failed'
            native_report='logs/native-runtime.json'
            native_report_sha256=(Get-FileSha256 -Path $JournalPath)
            native_call=$journal.runtime_call
        }
    }
    $actualStatus = [string]$journal.status
    if ($ExpectedStatus -eq 'terminal') {
        if ($actualStatus -notin @('passed', 'failed')) {
            throw 'Candidate journal is not finalized with a terminal status.'
        }
    } elseif ($actualStatus -ne 'failed') {
        throw 'Candidate failure journal is not finalized as failed.'
    }
    if (-not [StringComparer]::Ordinal.Equals([string]$journal.server, $ExpectedServer) -or
        -not [StringComparer]::Ordinal.Equals([string]$journal.database, $ExpectedDatabase)) {
        throw 'Candidate failure journal database identity does not match the requested server/database.'
    }
    $sqlcmdCalls = 0
    $bcpCalls = 0
    foreach ($call in @($journal.calls)) {
        $runtimeExecutable = Get-NormalizedExecutablePath ([string]$call.executable)
        $kind = if ($runtimeExecutable -eq (Get-NormalizedExecutablePath $SqlcmdPath)) {
            $sqlcmdCalls++; 'sqlcmd'
        } elseif ($runtimeExecutable -eq (Get-NormalizedExecutablePath $BcpPath)) {
            $bcpCalls++; 'bcp'
        } else {
            throw "Candidate failure journal contains an unregistered executable: $($call.executable)"
        }
        $expected = [string]$call.status
        if ($expected -notin @('passed', 'failed')) { throw 'Candidate failure journal contains an incomplete call.' }
        if ($actualStatus -eq 'passed' -and $expected -ne 'passed') {
            throw 'Passed candidate journal contains a non-passed call.'
        }
        Assert-CompleteRuntimeCall -Call $call -ExpectedExecutable ([string]$call.executable) `
            -ExpectedServer $ExpectedServer -ExpectedDatabase $ExpectedDatabase -Kind $kind -ExpectedStatus $expected
    }
    if ($actualStatus -eq 'passed' -and ($sqlcmdCalls -eq 0 -or $bcpCalls -eq 0)) {
        throw 'Passed candidate journal must contain passed sqlcmd and bcp calls.'
    }
    return [ordered]@{
        status=$actualStatus
        candidate_manifest='logs/candidate-runtime.json'
        candidate_manifest_sha256=(Get-FileSha256 -Path $JournalPath)
        candidate_subprocess_journal_status=$actualStatus
        candidate_calls=@($journal.calls)
        sqlcmd_calls=$sqlcmdCalls
        bcp_calls=$bcpCalls
    }
}

function Import-NativeRuntimeEvidence {
    param(
        [string]$JournalPath,
        [string]$NativeIbcmdPath,
        [string]$ExpectedServer,
        [string]$ExpectedDatabase,
        [ValidateSet('passed', 'failed', 'terminal')][string]$ExpectedStatus
    )
    if (-not (Test-Path -LiteralPath $JournalPath -PathType Leaf)) { throw 'Native runtime journal is missing.' }
    $json = Get-Content -Raw -LiteralPath $JournalPath
    Assert-ManifestSafe -Json $json
    $journal = $json | ConvertFrom-Json -ErrorAction Stop
    if ([int]$journal.protocol_version -ne 1) { throw 'Native runtime journal protocol is invalid.' }
    $actualStatus = [string]$journal.runtime_call.status
    if ($ExpectedStatus -eq 'terminal') {
        if ($actualStatus -notin @('passed', 'failed')) { throw 'Native runtime journal is not terminal.' }
    } elseif ($actualStatus -ne $ExpectedStatus) {
        throw "Native runtime journal call status does not match expected '$ExpectedStatus'."
    }
    Assert-CompleteRuntimeCall -Call $journal.runtime_call -ExpectedExecutable $NativeIbcmdPath `
        -ExpectedServer $ExpectedServer -ExpectedDatabase $ExpectedDatabase -Kind native_ibcmd -ExpectedStatus $actualStatus
    return [ordered]@{
        native_report='logs/native-runtime.json'
        native_report_sha256=(Get-FileSha256 -Path $JournalPath)
        native_call_status=$actualStatus
        native_call=$journal.runtime_call
    }
}

function Complete-StaleRuntimeJournal {
    param(
        [ValidateSet('native', 'candidate')][string]$JournalKind,
        [string]$JournalPath,
        [string]$NativeIbcmdPath,
        [string]$SqlcmdPath,
        [string]$BcpPath,
        [string]$ExpectedServer,
        [string]$ExpectedDatabase
    )
    if (-not (Test-Path -LiteralPath $JournalPath -PathType Leaf)) { return $false }
    $originalJson = Get-Content -Raw -LiteralPath $JournalPath
    Assert-ManifestSafe -Json $originalJson
    $journal = $originalJson | ConvertFrom-Json -ErrorAction Stop
    if ([int]$journal.protocol_version -ne 1) { throw "Stale $JournalKind runtime journal protocol is invalid." }
    $ended = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $reason = 'supervisor observed producer exit while runtime journal was still running'

    if ($JournalKind -eq 'native') {
        if ([string]$journal.runtime_call.status -ne 'running') { return $false }
        Assert-CompleteRuntimeCall -Call $journal.runtime_call -ExpectedExecutable $NativeIbcmdPath `
            -ExpectedServer $ExpectedServer -ExpectedDatabase $ExpectedDatabase -Kind native_ibcmd -ExpectedStatus running
        $journal.runtime_call.status = 'failed'
        $journal.runtime_call.ended_unix_ms = $ended
        $journal.runtime_call.exit_code = $null
        $journal.runtime_call.timed_out = $false
        $journal.runtime_call.exception = $reason
    } else {
        if (-not [StringComparer]::Ordinal.Equals([string]$journal.server, $ExpectedServer) -or
            -not [StringComparer]::Ordinal.Equals([string]$journal.database, $ExpectedDatabase)) {
            throw 'Stale candidate journal database identity does not match the requested server/database.'
        }
        if ([string]$journal.status -ne 'running') { return $false }
        foreach ($call in @($journal.calls)) {
            $runtimeExecutable = Get-NormalizedExecutablePath ([string]$call.executable)
            $kind = if ($runtimeExecutable -eq (Get-NormalizedExecutablePath $SqlcmdPath)) {
                'sqlcmd'
            } elseif ($runtimeExecutable -eq (Get-NormalizedExecutablePath $BcpPath)) {
                'bcp'
            } else {
                throw "Stale candidate journal contains an unregistered executable: $($call.executable)"
            }
            $status = [string]$call.status
            if ($status -notin @('running', 'passed', 'failed')) {
                throw 'Stale candidate journal contains an invalid call status.'
            }
            Assert-CompleteRuntimeCall -Call $call -ExpectedExecutable ([string]$call.executable) `
                -ExpectedServer $ExpectedServer -ExpectedDatabase $ExpectedDatabase -Kind $kind -ExpectedStatus $status
            if ($status -eq 'running') {
                $call.status = 'failed'
                $call.ended_unix_ms = $ended
                $call.exit_code = $null
                $call.timed_out = $false
                $call.exception = $reason
            }
        }
        $journal.status = 'failed'
        $journal.ended_unix_ms = $ended
        $journal.exception = $reason
    }

    $recovery = [ordered]@{
        kind='stale-running'
        recovered_unix_ms=$ended
        original_sha256=(Get-FileSha256 -Path $JournalPath)
    }
    $journal | Add-Member -NotePropertyName supervisor_recovery -NotePropertyValue $recovery -Force
    $currentJson = Get-Content -Raw -LiteralPath $JournalPath
    if (-not [StringComparer]::Ordinal.Equals($currentJson, $originalJson)) {
        throw "Stale $JournalKind runtime journal changed during supervisor recovery."
    }
    Write-ManifestAtomic -Path $JournalPath -Manifest $journal
    return $true
}

function Invoke-ParityStep {
    param(
        [string]$Name, [string]$Tool, [string]$Executable, [string]$LogPath, [string[]]$Arguments, [string[]]$Artifacts,
        [scriptblock]$Action, [System.Collections.ArrayList]$Steps, [System.Collections.IDictionary]$Manifest, [string]$ManifestPath
    )
    $record = [ordered]@{
        name=$Name
        tool=$Tool
        executable=$Executable
        status='running'
        started_utc=(Get-UtcNow)
        ended_utc=$null
        exit_code=$null
        exception=$null
        log=(Split-Path $LogPath -Leaf)
        arguments=(ConvertTo-SanitizedArguments $Arguments)
        artifacts=@($Artifacts)
    }
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
$sqlcmdPath = $null
$primaryFailure = $false
$manifest = [ordered]@{
    protocol_version=2; run_id=$RunId; scope=$Scope; created_utc=(Get-UtcNow); status='initializing'; git_sha=$null; xml_version=$SourceVersion; source_version=$SourceVersion
    database=[ordered]@{ name=$DbName; server=$DbServer; auth_mode=$authMode; user=$manifestDbUser; password_source=$manifestPasswordSource }
    tools=[ordered]@{}; layout=[ordered]@{ native='native'; candidate_dump='candidate_dump'; candidate='candidate' }; path_prefixes=@($PathPrefix); steps=$steps; artifacts=[ordered]@{}
}
# This is intentionally the first persistent action after directory creation: every later external command is journaled.
Write-ManifestAtomic -Path $manifestPath -Manifest $manifest

try {
    $manifest.status = 'running'
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest

    $gitPath = Get-ApplicationPath 'git'
    Register-ManifestTool -Name 'git' -Path $gitPath -VersionArguments @('--version') -Manifest $manifest -ManifestPath $manifestPath
    $manifest.repository_probe = [ordered]@{
        status='running'
        started_utc=(Get-UtcNow)
        ended_utc=$null
        exception=$null
        commands=@(
            [ordered]@{ executable=$gitPath; arguments=@('-C', $repoRoot, 'rev-parse', 'HEAD') },
            [ordered]@{ executable=$gitPath; arguments=@('-C', $repoRoot, 'status', '--porcelain=v1', '--untracked-files=all') }
        )
    }
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    try {
        $manifest.git_sha = (& $gitPath -C $repoRoot rev-parse HEAD).Trim()
        if ($LASTEXITCODE -ne 0) { throw "Cannot determine git SHA in $repoRoot" }
        $manifest.repository = Get-RepositoryState -RepoRoot $repoRoot -GitPath $gitPath
        $manifest.repository_probe.status = 'passed'
    } catch {
        $manifest.repository_probe.status = 'failed'
        $manifest.repository_probe.exception = Protect-SensitiveText $_.Exception.Message
        throw
    } finally {
        $manifest.repository_probe.ended_utc = Get-UtcNow
        Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    }

    Register-ManifestTool -Name 'candidate' -Path $ExePath -VersionArguments @('--version') -Manifest $manifest -ManifestPath $manifestPath
    $resolvedIbcmd = Get-ResolvedIbcmdPath $IbcmdPath
    $resolvedIbcmdFile = Get-Command -Name $resolvedIbcmd -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1
    Register-ManifestTool -Name 'native_ibcmd' -Path $resolvedIbcmdFile.Source -VersionArguments @('--version') -Manifest $manifest -ManifestPath $manifestPath
    $resolvedIbcmd = $resolvedIbcmdFile.Source
    $sqlcmdPath = Get-ExplicitOrDiscoveredApplicationPath -ExplicitPath $SqlcmdExecutable -Name 'sqlcmd'
    Register-ManifestTool -Name 'sqlcmd' -Path $sqlcmdPath -VersionArguments @('-?') -Manifest $manifest -ManifestPath $manifestPath
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
    try {
        $beforeFingerprint = Get-DatabaseFingerprint -SqlcmdPath $sqlcmdPath -Server $DbServer -Database $DbName -UseIntegratedAuth $IntegratedAuth -SqlUser $DbUser
        $manifest.database_fingerprint.before = $beforeFingerprint
        $manifest.database_fingerprint.before.status = 'passed'
    } catch {
        $manifest.database_fingerprint.before.status = 'failed'
        $manifest.database_fingerprint.before.ended_utc = Get-UtcNow
        $manifest.database_fingerprint.before.exception = Protect-SensitiveText $_.Exception.Message
    }
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest

    if (-not [string]::IsNullOrWhiteSpace($BcpExecutable)) {
        $bcpPath = Get-ExplicitOrDiscoveredApplicationPath -ExplicitPath $BcpExecutable -Name 'bcp'
    } else {
        $bcpPath = Join-Path (Split-Path -Parent $sqlcmdPath) 'bcp.exe'
        if (-not (Test-Path -LiteralPath $bcpPath -PathType Leaf)) { $bcpPath = Get-ApplicationPath 'bcp' }
    }
    $robocopyPath = Get-ApplicationPath 'robocopy'
    Register-ManifestTool -Name 'bcp' -Path $bcpPath -VersionArguments @('-v') -Manifest $manifest -ManifestPath $manifestPath
    Register-ManifestTool -Name 'robocopy' -Path $robocopyPath -VersionArguments @('/?') -AllowedExitCodes @(16) -Manifest $manifest -ManifestPath $manifestPath
    $manifest.tools.candidate.capability_probe_status = 'running'
    $manifest.tools.candidate.capability_probe_started_utc = Get-UtcNow
    $manifest.tools.candidate.capability_probe_ended_utc = $null
    $manifest.tools.candidate.capability_probe_arguments = @(
        @($Cli.NativeExport, '--help'),
        @($Cli.CandidateExport, '--help'),
        @($Cli.Diff, '--help'),
        @($Cli.Signatures, '--help'),
        @($Cli.Matrix, '--help'),
        @($Cli.MatrixMerge, '--help')
    )
    $manifest.tools.candidate.capability_probes = [System.Collections.ArrayList]::new()
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    try {
        foreach ($command in @($Cli.NativeExport, $Cli.CandidateExport, $Cli.Diff, $Cli.Signatures, $Cli.Matrix, $Cli.MatrixMerge)) {
            $probe = [ordered]@{
                executable=$ExePath
                arguments=@($command, '--help')
                status='running'
                started_utc=(Get-UtcNow)
                ended_utc=$null
                exit_code=$null
                exception=$null
            }
            [void]$manifest.tools.candidate.capability_probes.Add($probe)
            Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
            $previousPreference = $ErrorActionPreference
            $ErrorActionPreference = 'Continue'
            try {
                $help = & $ExePath $command '--help' 2>&1 | Out-String
                $probe.exit_code = $LASTEXITCODE
                if ($probe.exit_code -ne 0 -or $help -notmatch [regex]::Escape($command)) {
                    throw "Required command '$command' is unavailable in $ExePath. Build it with: cargo build --release --features platform-oracle"
                }
                $probe.status = 'passed'
            } catch {
                if ($null -eq $probe.exit_code) { $probe.exit_code = -1 }
                $probe.status = 'failed'
                $probe.exception = Protect-SensitiveText $_.Exception.Message
                throw
            } finally {
                $ErrorActionPreference = $previousPreference
                $probe.ended_utc = Get-UtcNow
                Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
            }
        }
        $manifest.tools.candidate.capability_probe_status = 'passed'
    } catch {
        $manifest.tools.candidate.capability_probe_status = 'failed'
        $manifest.tools.candidate.capability_probe_exception = Protect-SensitiveText $_.Exception.Message
        throw
    } finally {
        $manifest.tools.candidate.capability_probe_ended_utc = Get-UtcNow
        Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    }

    $nativeRuntimeJournalPath = Join-Path $logsRoot 'native-runtime.json'
    $candidateRuntimeJournalPath = Join-Path $logsRoot 'candidate-runtime.json'
    $manifest.nested_runtime_calls = [ordered]@{
        status='running'
        started_utc=(Get-UtcNow)
        ended_utc=$null
        exception=$null
        native_report='logs/native-runtime.json'
        candidate_manifest='logs/candidate-runtime.json'
    }
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest

    $nativeArgs = @($Cli.NativeExport, '--dbms', 'MSSQLServer', '--db-server', $DbServer, '--db-name', $DbName, '-o', $nativeRoot, '--overwrite', '--ibcmd', $resolvedIbcmd, '--timeout-sec', [string]$NativeTimeoutSec, '--runtime-journal', $nativeRuntimeJournalPath)
    if (-not $IntegratedAuth) { $nativeArgs += @('--db-user', $DbUser, '--db-pwd-env', 'IBCMD_DB_PSW') }
    $nativeAction = { & $ExePath @nativeArgs }
    try {
        if ($IntegratedAuth) {
            Invoke-ParityStep -Name 'native-export' -Tool 'candidate' -Executable $ExePath -LogPath (Join-Path $logsRoot 'native-export.log') -Arguments $nativeArgs -Artifacts @('native', 'logs/native-runtime.json') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { Invoke-WithoutSqlCredentialEnvironment $nativeAction }
        } else {
            Invoke-ParityStep -Name 'native-export' -Tool 'candidate' -Executable $ExePath -LogPath (Join-Path $logsRoot 'native-export.log') -Arguments $nativeArgs -Artifacts @('native', 'logs/native-runtime.json') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action $nativeAction
        }
        $nativeEvidence = Import-NativeRuntimeEvidence -JournalPath $nativeRuntimeJournalPath `
            -NativeIbcmdPath $resolvedIbcmd -ExpectedServer $DbServer -ExpectedDatabase $DbName -ExpectedStatus passed
        foreach ($key in $nativeEvidence.Keys) { $manifest.nested_runtime_calls[$key] = $nativeEvidence[$key] }
        Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    } catch {
        $nativeError = $_
        try {
            $null = Complete-StaleRuntimeJournal -JournalKind native -JournalPath $nativeRuntimeJournalPath `
                -NativeIbcmdPath $resolvedIbcmd -SqlcmdPath $sqlcmdPath -BcpPath $bcpPath `
                -ExpectedServer $DbServer -ExpectedDatabase $DbName
            $nativeEvidence = Import-NativeRuntimeEvidence -JournalPath $nativeRuntimeJournalPath `
                -NativeIbcmdPath $resolvedIbcmd -ExpectedServer $DbServer -ExpectedDatabase $DbName -ExpectedStatus terminal
            foreach ($key in $nativeEvidence.Keys) { $manifest.nested_runtime_calls[$key] = $nativeEvidence[$key] }
        } catch {
            $manifest.nested_runtime_calls.exception = Protect-SensitiveText $_.Exception.Message
        }
        $manifest.nested_runtime_calls.status = 'failed'
        $manifest.nested_runtime_calls.ended_utc = Get-UtcNow
        Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
        throw $nativeError
    }

    $candidateArgs = @($Cli.CandidateExport, '--database', $DbName, '--server', $DbServer, '--sqlcmd', $sqlcmdPath, '--bcp-executable', $bcpPath, '--runtime-journal', $candidateRuntimeJournalPath, '-o', $candidateDumpRoot, '--overwrite', '--inflate', '--extract-module-text', '--extract-metadata-xml', '--source-version', $SourceVersion, '--no-binary-rows')
    if (-not $IntegratedAuth) { $candidateArgs += @('--sql-user', $DbUser, '--sql-pwd-env', 'IBCMD_DB_PSW') }
    if ($RequireCompleteRootMetadata) { $candidateArgs += '--require-complete-root-metadata' }
    try {
        Invoke-ParityStep -Name 'candidate-export' -Tool 'candidate' -Executable $ExePath -LogPath (Join-Path $logsRoot 'candidate-export.log') -Arguments $candidateArgs -Artifacts @('candidate_dump/manifest.json', 'logs/candidate-runtime.json') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & $ExePath @candidateArgs }
    } catch {
        $candidateError = $_
        try {
            $null = Complete-StaleRuntimeJournal -JournalKind candidate -JournalPath $candidateRuntimeJournalPath `
                -NativeIbcmdPath $resolvedIbcmd -SqlcmdPath $sqlcmdPath -BcpPath $bcpPath `
                -ExpectedServer $DbServer -ExpectedDatabase $DbName
            $candidateEvidence = Import-FailedRuntimeEvidence -JournalKind candidate `
                -JournalPath $candidateRuntimeJournalPath -SqlcmdPath $sqlcmdPath -BcpPath $bcpPath `
                -ExpectedServer $DbServer -ExpectedDatabase $DbName -ExpectedStatus terminal
            foreach ($key in $candidateEvidence.Keys) { $manifest.nested_runtime_calls[$key] = $candidateEvidence[$key] }
        } catch {
            $manifest.nested_runtime_calls.exception = Protect-SensitiveText $_.Exception.Message
        }
        $manifest.nested_runtime_calls.status = 'failed'
        $manifest.nested_runtime_calls.ended_utc = Get-UtcNow
        Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
        throw $candidateError
    }
    try {
        $runtimeEvidence = Import-VerifiedRuntimeEvidence `
            -NativeJournalPath $nativeRuntimeJournalPath `
            -CandidateJournalPath $candidateRuntimeJournalPath `
            -NativeIbcmdPath $resolvedIbcmd -SqlcmdPath $sqlcmdPath -BcpPath $bcpPath `
            -ExpectedServer $DbServer -ExpectedDatabase $DbName
        foreach ($key in $runtimeEvidence.Keys) { $manifest.nested_runtime_calls[$key] = $runtimeEvidence[$key] }
    } catch {
        $manifest.nested_runtime_calls.status = 'failed'
        $manifest.nested_runtime_calls.exception = Protect-SensitiveText $_.Exception.Message
        throw
    } finally {
        $manifest.nested_runtime_calls.ended_utc = Get-UtcNow
        Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    }

    $roboArgs = @($candidateDumpRoot, $candidateRoot, '/E', '/XD', 'Config_inflated', 'Config_raw', 'ConfigSave_inflated', 'ConfigSave_raw', '/XF', 'manifest.json', '*.json')
    Invoke-ParityStep -Name 'candidate-source-layout' -Tool 'robocopy' -Executable $robocopyPath -LogPath (Join-Path $logsRoot 'candidate-source-layout.log') -Arguments $roboArgs -Artifacts @('candidate') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & $robocopyPath @roboArgs | Out-Host; if ($LASTEXITCODE -le 7) { $global:LASTEXITCODE = 0 } }

    $diffPath = Join-Path $runRoot 'raw-diff.json'; $diffArgs = @($Cli.Diff, '-o', $diffPath)
    foreach ($prefix in $PathPrefix) { $diffArgs += @('--path-prefix', $prefix) }; $diffArgs += @($nativeRoot, $candidateRoot)
    Invoke-ParityStep -Name 'raw-diff' -Tool 'candidate' -Executable $ExePath -LogPath (Join-Path $logsRoot 'raw-diff.log') -Arguments $diffArgs -Artifacts @('raw-diff.json') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & $ExePath @diffArgs }
    $manifest.tree_summaries = [ordered]@{
        native = Get-TreeSummaryFromDiff -DiffPath $diffPath -Side left
        candidate = Get-TreeSummaryFromDiff -DiffPath $diffPath -Side right
    }
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    $signaturesPath = Join-Path $runRoot 'signatures.json'; $signatureArgs = @($Cli.Signatures, '-o', $signaturesPath, $diffPath)
    Invoke-ParityStep -Name 'diff-signatures' -Tool 'candidate' -Executable $ExePath -LogPath (Join-Path $logsRoot 'diff-signatures.log') -Arguments $signatureArgs -Artifacts @('signatures.json') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & $ExePath @signatureArgs }
    $matrixPath = Join-Path $runRoot 'matrix.json'; $matrixMarkdownPath = Join-Path $runRoot 'matrix.md'; $matrixScopeArg = if ($Scope -eq 'full') { '--full' } else { '--scoped' }
    $matrixArgs = @($Cli.Matrix, $diffPath, '--database', $DbName, '--run-id', $RunId, '--git-sha', $manifest.git_sha, $matrixScopeArg, '--output', $matrixPath, '--markdown', $matrixMarkdownPath)
    Invoke-ParityStep -Name 'parity-matrix' -Tool 'candidate' -Executable $ExePath -LogPath (Join-Path $logsRoot 'parity-matrix.log') -Arguments $matrixArgs -Artifacts @('matrix.json', 'matrix.md') -Steps $steps -Manifest $manifest -ManifestPath $manifestPath -Action { & $ExePath @matrixArgs }
    if ($Scope -eq 'full' -and $manifest.repository.status -ne 'clean') { throw 'Full release parity requires a clean Git repository.' }
    $manifest.artifacts = [ordered]@{ raw_diff='raw-diff.json'; signatures='signatures.json'; matrix='matrix.json'; markdown='matrix.md' }
    $manifest.artifact_sha256 = [ordered]@{
        raw_diff=(Get-FileSha256 -Path $diffPath)
        signatures=(Get-FileSha256 -Path $signaturesPath)
        matrix=(Get-FileSha256 -Path $matrixPath)
        markdown=(Get-FileSha256 -Path $matrixMarkdownPath)
    }
    $manifest.status='finalizing'
} catch {
    $primaryFailure = $true
    $manifest.status='failed'; $manifest.failure=(Protect-SensitiveText $_.Exception.Message); throw
} finally {
    # A failed export is still evidence about whether it changed the configuration
    # storage.  This best-effort snapshot must never replace the original failure.
    if ($manifest.Contains('database_fingerprint') -and -not [string]::IsNullOrWhiteSpace($sqlcmdPath)) {
        $manifest.database_fingerprint.after = [ordered]@{
            status='running'
            started_utc=(Get-UtcNow)
            executable=$sqlcmdPath
            arguments=@()
        }
        try {
            $afterFingerprintCommand = Get-DatabaseFingerprintCommand -SqlcmdPath $sqlcmdPath -Server $DbServer -Database $DbName -UseIntegratedAuth $IntegratedAuth -SqlUser $DbUser
            $manifest.database_fingerprint.after.executable = $afterFingerprintCommand.executable
            $manifest.database_fingerprint.after.arguments = ConvertTo-SanitizedArguments $afterFingerprintCommand.arguments
            Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
            $afterFingerprint = Get-DatabaseFingerprint -SqlcmdPath $sqlcmdPath -Server $DbServer -Database $DbName -UseIntegratedAuth $IntegratedAuth -SqlUser $DbUser
            $manifest.database_fingerprint.after = $afterFingerprint
            $manifest.database_fingerprint.after.status = 'passed'
        } catch {
            $manifest.database_fingerprint.after.status = 'failed'
            $manifest.database_fingerprint.after.ended_utc = Get-UtcNow
            $manifest.database_fingerprint.after.exception = Protect-SensitiveText $_.Exception.Message
        }
    }

    $integrityFailure = $null
    if (-not $primaryFailure) {
        if (-not $manifest.Contains('nested_runtime_calls') -or $manifest.nested_runtime_calls.status -ne 'passed') {
            $integrityFailure = 'Nested runtime journals are unavailable or incomplete; run is invalid.'
        } elseif ($manifest.database_fingerprint.before.status -ne 'passed') {
            $integrityFailure = 'Database fingerprint before export is unavailable; run is invalid.'
        } elseif ($null -eq $manifest.database_fingerprint.after -or $manifest.database_fingerprint.after.status -ne 'passed') {
            $integrityFailure = 'Database fingerprint after export is unavailable; run is invalid.'
        } else {
            $manifest.database_fingerprint.unchanged = ($manifest.database_fingerprint.before.sha256 -eq $manifest.database_fingerprint.after.sha256)
            if (-not $manifest.database_fingerprint.unchanged) { $integrityFailure = 'Database configuration storage changed during parity export.' }
        }
        if ($null -ne $integrityFailure) {
            $manifest.status = 'failed'
            $manifest.failure = $integrityFailure
        }
    }
    $manifest.finished_utc=Get-UtcNow
    if (-not $primaryFailure -and $null -eq $integrityFailure) {
        $manifest.status = 'passed'
    }
    Write-ManifestAtomic -Path $manifestPath -Manifest $manifest
    if ($null -ne $integrityFailure) { throw $integrityFailure }
}

Write-Host "Parity run completed: $runRoot"
