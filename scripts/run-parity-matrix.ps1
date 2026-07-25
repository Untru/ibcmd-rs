[CmdletBinding()]
param(
    [string]$UtDbName = 'ut_ibcmd',
    [string]$BspDbName = 'bsp',
    [string]$DbServer = 'localhost',
    [string]$DbUser = 'sa',
    [switch]$IntegratedAuth,
    [string]$LabRoot = 'E:\ibcmd_lab\parity',
    [string]$RunId = (Get-Date -Format 'yyyyMMdd_HHmmss'),
    [string]$ExePath = '',
    [string]$IbcmdPath = '',
    [string]$SqlcmdExecutable = '',
    [string]$BcpExecutable = '',
    [ValidateRange(1, 86400)][int]$NativeTimeoutSec = 900,
    [ValidateSet('2.20', '2.21')][string]$SourceVersion = '2.20',
    [ValidateSet('full', 'scoped')][string]$Scope = 'full',
    [string[]]$PathPrefix = @(),
    [switch]$RequireCompleteRootMetadata
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-UtcNow { (Get-Date).ToUniversalTime().ToString('o') }

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

function Write-AtomicJson {
    param([string]$Path, [System.Collections.IDictionary]$Object)
    $json = $Object | ConvertTo-Json -Depth 20
    $null = $json | ConvertFrom-Json -ErrorAction Stop
    foreach ($secretName in @('IBCMD_DB_PSW', 'IBCMD_USER_PSW', 'SQLCMDPASSWORD')) {
        if ($json.IndexOf($secretName, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw 'Refusing to write secret-bearing matrix manifest content.'
        }
        $secretValue = [Environment]::GetEnvironmentVariable($secretName, 'Process')
        if (-not [string]::IsNullOrEmpty($secretValue) -and $json.Contains($secretValue)) {
            throw 'Refusing to write secret-bearing matrix manifest content.'
        }
    }
    $tmp = "$Path.$([guid]::NewGuid().ToString('N')).tmp"
    [IO.File]::WriteAllText($tmp, $json, [System.Text.UTF8Encoding]::new($false))
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

function Get-NormalizedExecutablePath {
    param([string]$Path, [string]$Label)
    if ([string]::IsNullOrWhiteSpace($Path) -or -not [IO.Path]::IsPathRooted($Path)) {
        throw "$Label must be an absolute resolved executable path."
    }
    return [IO.Path]::GetFullPath($Path)
}

function Assert-NoReparsePointComponent {
    param([string]$Root, [string]$Target, [string]$Label)
    $rootFull = [IO.Path]::GetFullPath($Root)
    $targetFull = [IO.Path]::GetFullPath($Target)
    $prefix = $rootFull.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $targetFull.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "$Label escapes its child run directory."
    }
    $rootItem = Get-Item -LiteralPath $rootFull -Force -ErrorAction Stop
    if (($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "$Label child root is a reparse point."
    }
    $relative = $targetFull.Substring($prefix.Length)
    $current = $rootFull
    foreach ($component in @($relative -split '[\\/]' | Where-Object { -not [string]::IsNullOrEmpty($_) })) {
        $current = Join-Path $current $component
        $item = Get-Item -LiteralPath $current -Force -ErrorAction Stop
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "$Label crosses a reparse point: $current"
        }
    }
}

function Read-ValidChildManifest {
    param(
        [string]$Path,
        [string]$ExpectedScope,
        [string]$ExpectedCandidatePath,
        [string]$ExpectedSourceVersion,
        [string]$ExpectedServer,
        [string]$ExpectedDatabase
    )
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Missing child manifest: $Path" }
    $child = Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json -ErrorAction Stop
    if ([int]$child.protocol_version -ne 2) { throw "Child manifest protocol is not supported: $Path" }
    if ($child.status -ne 'passed') { throw "Child manifest is not successful: $Path" }
    if ($child.scope -ne $ExpectedScope) { throw "Child manifest scope '$($child.scope)' does not match expected '$ExpectedScope': $Path" }
    if ([string]::IsNullOrWhiteSpace([string]$child.database.name) -or
        -not [StringComparer]::Ordinal.Equals([string]$child.database.name, $ExpectedDatabase)) {
        throw "Child manifest database does not match expected '$ExpectedDatabase': $Path"
    }
    if ([string]::IsNullOrWhiteSpace([string]$child.database.server) -or
        -not [StringComparer]::Ordinal.Equals([string]$child.database.server, $ExpectedServer)) {
        throw "Child manifest database server does not match expected '$ExpectedServer': $Path"
    }
    if ([string]::IsNullOrWhiteSpace([string]$child.finished_utc)) { throw "Child manifest is unfinished: $Path" }
    foreach ($property in @('git_sha', 'xml_version', 'source_version')) {
        if ([string]::IsNullOrWhiteSpace([string]$child.$property)) { throw "Child manifest misses ${property}: $Path" }
    }
    if ([string]$child.git_sha -notmatch '^[0-9a-fA-F]{40,64}$') {
        throw "Child manifest has invalid candidate Git SHA: $Path"
    }
    if ([string]$child.xml_version -cne [string]$child.source_version) {
        throw "Child manifest XML/source versions differ: $Path"
    }
    if ([string]$child.source_version -cne $ExpectedSourceVersion) {
        throw "Child manifest source version '$($child.source_version)' does not match requested '$ExpectedSourceVersion': $Path"
    }
    if ($child.database_fingerprint.unchanged -ne $true) {
        throw "Child manifest has no unchanged database fingerprint: $Path"
    }
    foreach ($side in @('before', 'after')) {
        if ([string]$child.database_fingerprint.$side.status -cne 'passed') {
            throw "Child manifest database fingerprint ${side} is not passed: $Path"
        }
    }
    if (@($child.steps).Count -eq 0 -or @($child.steps | Where-Object { $_.status -ne 'passed' }).Count -ne 0) {
        throw "Child manifest has incomplete or failed steps: $Path"
    }
    if ($ExpectedScope -eq 'full' -and [string]$child.repository.status -cne 'clean') {
        throw "Full child manifest does not describe a clean candidate repository: $Path"
    }
    foreach ($tool in @('candidate', 'native_ibcmd', 'sqlcmd', 'bcp')) {
        if ([string]$child.tools.$tool.status -cne 'passed') {
            throw "Child manifest has no passed ${tool} identity probe: $Path"
        }
        if ([string]::IsNullOrWhiteSpace([string]$child.tools.$tool.version)) {
            throw "Child manifest misses resolved ${tool} version: $Path"
        }
        if ([string]$child.tools.$tool.sha256 -notmatch '^[0-9a-fA-F]{64}$') {
            throw "Child manifest misses valid ${tool} SHA-256: $Path"
        }
    }
    if ([string]$child.tools.candidate.capability_probe_status -cne 'passed') {
        throw "Child manifest candidate capability probe set is not passed: $Path"
    }
    $capabilityProbes = @($child.tools.candidate.capability_probes)
    if ($capabilityProbes.Count -ne 6 -or @($capabilityProbes | Where-Object {
        [string]$_.status -cne 'passed' -or [int]$_.exit_code -ne 0 -or
        [string]::IsNullOrWhiteSpace([string]$_.ended_utc) -or @($_.arguments).Count -ne 2
    }).Count -ne 0) {
        throw "Child manifest candidate capability probe journal is incomplete: $Path"
    }
    if ([string]$child.nested_runtime_calls.status -cne 'passed' -or
        [string]$child.nested_runtime_calls.candidate_subprocess_journal_status -cne 'passed' -or
        [int]$child.nested_runtime_calls.sqlcmd_calls -le 0 -or
        [int]$child.nested_runtime_calls.bcp_calls -le 0) {
        throw "Child manifest has no complete nested native/sqlcmd/bcp runtime journals: $Path"
    }
    if ([string]::IsNullOrWhiteSpace([string]$child.nested_runtime_calls.ended_utc)) {
        throw "Child manifest nested runtime journal verification is unfinished: $Path"
    }
    $expectedCandidate = Get-NormalizedExecutablePath -Path $ExpectedCandidatePath -Label 'Expected candidate path'
    $actualCandidate = Get-NormalizedExecutablePath -Path ([string]$child.tools.candidate.path) -Label 'Child candidate path'
    if (-not [StringComparer]::OrdinalIgnoreCase.Equals($actualCandidate, $expectedCandidate)) {
        throw "Child manifest candidate path does not match the orchestrator executable identity: $Path"
    }
    [void](Get-NormalizedExecutablePath -Path ([string]$child.tools.native_ibcmd.path) -Label 'Child native ibcmd path')
    [void](Get-NormalizedExecutablePath -Path ([string]$child.tools.sqlcmd.path) -Label 'Child sqlcmd path')
    [void](Get-NormalizedExecutablePath -Path ([string]$child.tools.bcp.path) -Label 'Child bcp path')
    if (-not $child.artifacts.matrix) { throw "Child manifest misses matrix artifact: $Path" }
    if ([string]$child.artifact_sha256.matrix -notmatch '^[0-9a-fA-F]{64}$') {
        throw "Child manifest misses matrix artifact SHA-256: $Path"
    }
    $childRoot = [IO.Path]::GetFullPath((Split-Path -Parent $Path))
    foreach ($nestedArtifact in @(
        [ordered]@{ path=[string]$child.nested_runtime_calls.native_report; sha=[string]$child.nested_runtime_calls.native_report_sha256; label='native runtime report' },
        [ordered]@{ path=[string]$child.nested_runtime_calls.candidate_manifest; sha=[string]$child.nested_runtime_calls.candidate_manifest_sha256; label='candidate subprocess journal' }
    )) {
        if ([string]::IsNullOrWhiteSpace($nestedArtifact.path) -or [IO.Path]::IsPathRooted($nestedArtifact.path)) {
            throw "Child $($nestedArtifact.label) path must be a non-empty relative path: $Path"
        }
        if ($nestedArtifact.sha -notmatch '^[0-9a-fA-F]{64}$') {
            throw "Child $($nestedArtifact.label) SHA-256 is invalid: $Path"
        }
        $nestedPath = [IO.Path]::GetFullPath((Join-Path $childRoot $nestedArtifact.path))
        $childPrefix = $childRoot.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        if (-not $nestedPath.StartsWith($childPrefix, [StringComparison]::OrdinalIgnoreCase) -or
            -not (Test-Path -LiteralPath $nestedPath -PathType Leaf)) {
            throw "Child $($nestedArtifact.label) is missing or escapes its run directory: $Path"
        }
        Assert-NoReparsePointComponent -Root $childRoot -Target $nestedPath -Label "Child $($nestedArtifact.label)"
        if (
            -not [StringComparer]::OrdinalIgnoreCase.Equals((Get-FileSha256 -Path $nestedPath), $nestedArtifact.sha)) {
            throw "Child $($nestedArtifact.label) has a mismatched SHA-256: $Path"
        }
    }
    $matrixRelative = [string]$child.artifacts.matrix
    if ([IO.Path]::IsPathRooted($matrixRelative)) { throw "Child matrix artifact path must be relative: $Path" }
    $childMatrixPath = [IO.Path]::GetFullPath((Join-Path $childRoot $matrixRelative))
    $childPrefix = $childRoot.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $childMatrixPath.StartsWith($childPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Child matrix artifact escapes its run directory: $Path"
    }
    if (-not (Test-Path -LiteralPath $childMatrixPath -PathType Leaf)) {
        throw "Child matrix artifact is missing: $childMatrixPath"
    }
    Assert-NoReparsePointComponent -Root $childRoot -Target $childMatrixPath -Label 'Child matrix artifact'
    if (-not [StringComparer]::OrdinalIgnoreCase.Equals(
        (Get-FileSha256 -Path $childMatrixPath),
        [string]$child.artifact_sha256.matrix
    )) {
        throw "Child matrix artifact SHA-256 does not match its manifest: $Path"
    }
    return $child
}

function Assert-CompatibleChildManifests {
    param([object[]]$Children, [string]$ExpectedScope)
    if ($Children.Count -ne 2) { throw "Exactly two validated child manifests are required before merge; found $($Children.Count)." }
    $databaseNames = @($Children | ForEach-Object { ([string]$_.database).Trim() })
    if (@($databaseNames | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -ne 0 -or
        [StringComparer]::OrdinalIgnoreCase.Equals($databaseNames[0], $databaseNames[1])) {
        throw 'Validated child manifests must represent two distinct database names. Merge blocked.'
    }
    $reference = $Children[0]
    if ([string]::IsNullOrWhiteSpace([string]$reference.server)) {
        throw 'Validated child manifests must carry an exact database server identity. Merge blocked.'
    }
    $fields = @(
        'git_sha',
        'xml_version',
        'source_version',
        'candidate_version',
        'candidate_sha256',
        'native_ibcmd_version',
        'native_ibcmd_sha256',
        'sqlcmd_version',
        'sqlcmd_sha256',
        'bcp_version',
        'bcp_sha256'
    )
    foreach ($child in $Children | Select-Object -Skip 1) {
        if (-not [StringComparer]::Ordinal.Equals([string]$child.server, [string]$reference.server)) {
            throw "Child manifest mismatch for server: '$($reference.database)' != '$($child.database)'. Merge blocked."
        }
        foreach ($field in $fields) {
            if (-not [StringComparer]::Ordinal.Equals([string]$child.$field, [string]$reference.$field)) {
                throw "Child manifest mismatch for ${field}: '$($reference.database)' != '$($child.database)'. Merge blocked."
            }
        }
        if (-not [StringComparer]::OrdinalIgnoreCase.Equals(
            [string]$child.candidate_path,
            [string]$reference.candidate_path
        )) {
            throw "Child manifest mismatch for candidate_path under absolute-normalized-ordinal-ignore-case policy: '$($reference.database)' != '$($child.database)'. Merge blocked."
        }
        if (-not [StringComparer]::OrdinalIgnoreCase.Equals(
            [string]$child.native_ibcmd_path,
            [string]$reference.native_ibcmd_path
        )) {
            throw "Child manifest mismatch for native_ibcmd_path under absolute-normalized-ordinal-ignore-case policy: '$($reference.database)' != '$($child.database)'. Merge blocked."
        }
        foreach ($pathField in @('sqlcmd_path', 'bcp_path')) {
            if (-not [StringComparer]::OrdinalIgnoreCase.Equals(
                [string]$child.$pathField,
                [string]$reference.$pathField
            )) {
                throw "Child manifest mismatch for $pathField under absolute-normalized-ordinal-ignore-case policy: '$($reference.database)' != '$($child.database)'. Merge blocked."
            }
        }
    }
    return [ordered]@{
        status = 'passed'
        compared_children = $Children.Count
        scope = $ExpectedScope
        release_proof = ($ExpectedScope -eq 'full')
        candidate_path_identity_policy = 'absolute-normalized-ordinal-ignore-case'
        native_ibcmd_path_identity_policy = 'absolute-normalized-ordinal-ignore-case'
        sql_client_path_identity_policy = 'absolute-normalized-ordinal-ignore-case'
        distinct_databases = $true
        server = $reference.server
        git_sha = $reference.git_sha
        xml_version = $reference.xml_version
        source_version = $reference.source_version
        candidate_path = $reference.candidate_path
        candidate_version = $reference.candidate_version
        candidate_sha256 = $reference.candidate_sha256
        native_ibcmd_path = $reference.native_ibcmd_path
        native_ibcmd_version = $reference.native_ibcmd_version
        native_ibcmd_sha256 = $reference.native_ibcmd_sha256
        sqlcmd_path = $reference.sqlcmd_path
        sqlcmd_version = $reference.sqlcmd_version
        sqlcmd_sha256 = $reference.sqlcmd_sha256
        bcp_path = $reference.bcp_path
        bcp_version = $reference.bcp_version
        bcp_sha256 = $reference.bcp_sha256
    }
}

function Complete-FailedStep {
    param(
        [System.Collections.IDictionary]$Step,
        [System.Management.Automation.ErrorRecord]$ErrorRecord,
        [System.Collections.IDictionary]$Manifest,
        [string]$ManifestPath
    )
    $Step.status = 'failed'
    $Step.ended_utc = Get-UtcNow
    $Step.exit_code = -1
    $Step.exception = Protect-SensitiveText $ErrorRecord.Exception.Message
    Write-AtomicJson -Path $ManifestPath -Object $Manifest
}

if ($RunId -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$' -or $RunId.Contains('..')) {
    throw "RunId must be 1-128 safe characters and not contain '..'."
}
if (@($PathPrefix | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -ne 0) {
    throw 'PathPrefix entries must be non-empty, non-whitespace strings.'
}
if ($Scope -eq 'full' -and $PathPrefix.Count -ne 0) {
    throw "Scope 'full' requires an empty PathPrefix. Use -Scope scoped for a partial comparison."
}
if ($Scope -eq 'scoped' -and $PathPrefix.Count -eq 0) {
    throw "Scope 'scoped' requires at least one PathPrefix."
}
if ($RequireCompleteRootMetadata -and $Scope -ne 'full') {
    throw "RequireCompleteRootMetadata is available only for Scope 'full'."
}
if ([string]::IsNullOrWhiteSpace($UtDbName) -or [string]::IsNullOrWhiteSpace($BspDbName) -or
    [StringComparer]::OrdinalIgnoreCase.Equals($UtDbName.Trim(), $BspDbName.Trim())) {
    throw 'UtDbName and BspDbName must be distinct non-empty database names (case-insensitive).'
}

$runner = Join-Path $PSScriptRoot 'export-ibcmd-vs-ours.ps1'
if (-not (Test-Path -LiteralPath $runner)) { throw "Missing runner: $runner" }
if ([string]::IsNullOrWhiteSpace($ExePath)) {
    $ExePath = Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\ibcmd-rs.exe'
}
$ExePath = [IO.Path]::GetFullPath($ExePath)

$matrixRoot = Join-Path $LabRoot ("matrix_{0}" -f $RunId)
if (Test-Path -LiteralPath $matrixRoot) { throw "Matrix directory already exists and is immutable: $matrixRoot" }
New-Item -ItemType Directory -Path $matrixRoot | Out-Null
$logsRoot = Join-Path $matrixRoot 'logs'
New-Item -ItemType Directory -Path $logsRoot | Out-Null

$steps = [System.Collections.ArrayList]::new()
$manifestPath = Join-Path $matrixRoot 'parity-matrix-manifest.json'
$resultClass = 'diagnostic'
$matrixManifest = [ordered]@{
    protocol_version = 2
    run_id = $RunId
    created_utc = Get-UtcNow
    status = 'running'
    scope = $Scope
    result_class = $resultClass
    release_gate_requested = ($Scope -eq 'full' -and $RequireCompleteRootMetadata)
    release_eligible = $false
    parity_zero = $null
    child_manifests = @()
    steps = $steps
    artifacts = [ordered]@{}
}
Write-AtomicJson -Path $manifestPath -Object $matrixManifest

try {
    $children = [System.Collections.ArrayList]::new()
    $matrixPaths = [System.Collections.ArrayList]::new()
    foreach ($database in @(
        [ordered]@{ id='ut'; name=$UtDbName },
        [ordered]@{ id='bsp'; name=$BspDbName }
    )) {
        $childRunId = "{0}_{1}" -f $RunId, $database.id
        $runDirectory = Join-Path $LabRoot ("{0}_{1}" -f ($database.name -replace '[^A-Za-z0-9_.-]', '_'), $childRunId)
        $childManifestPath = Join-Path $runDirectory 'parity-manifest.json'
        $childLog = Join-Path $logsRoot ("child-{0}.log" -f $database.id)
        $childStep = [ordered]@{
            name = "child-$($database.id)"
            database = $database.name
            status = 'running'
            started_utc = Get-UtcNow
            ended_utc = $null
            exit_code = $null
            exception = $null
            log = "logs/child-$($database.id).log"
            artifacts = @($childManifestPath)
        }
        [void]$steps.Add($childStep)
        Write-AtomicJson -Path $manifestPath -Object $matrixManifest

        $params = @{
            DbName=$database.name
            DbServer=$DbServer
            DbUser=$DbUser
            LabRoot=$LabRoot
            RunId=$childRunId
            SourceVersion=$SourceVersion
            Scope=$Scope
            PathPrefix=$PathPrefix
            ExePath=$ExePath
            NativeTimeoutSec=$NativeTimeoutSec
        }
        if ($IntegratedAuth -or [string]::IsNullOrWhiteSpace($DbUser)) { $params.IntegratedAuth = $true }
        if ($IbcmdPath) { $params.IbcmdPath = $IbcmdPath }
        if ($SqlcmdExecutable) { $params.SqlcmdExecutable = $SqlcmdExecutable }
        if ($BcpExecutable) { $params.BcpExecutable = $BcpExecutable }
        if ($RequireCompleteRootMetadata) { $params.RequireCompleteRootMetadata = $true }

        try {
            & $runner @params *>&1 |
                ForEach-Object { Protect-SensitiveText $_ } |
                Tee-Object -FilePath $childLog |
                Write-Host
            if ($LASTEXITCODE -ne 0) {
                throw "Parity run failed for database '$($database.name)' with exit code $LASTEXITCODE"
            }
            $child = Read-ValidChildManifest -Path $childManifestPath -ExpectedScope $Scope `
                -ExpectedCandidatePath $ExePath -ExpectedSourceVersion $SourceVersion `
                -ExpectedServer $DbServer -ExpectedDatabase $database.name
            $childStep.status = 'passed'
            $childStep.ended_utc = Get-UtcNow
            $childStep.exit_code = 0
            [void]$children.Add([ordered]@{
                database = $database.name
                server = $child.database.server
                manifest = $childManifestPath
                git_sha = $child.git_sha
                xml_version = $child.xml_version
                source_version = $child.source_version
                candidate_path = (Get-NormalizedExecutablePath -Path ([string]$child.tools.candidate.path) -Label 'Child candidate path')
                candidate_version = $child.tools.candidate.version
                native_ibcmd_version = $child.tools.native_ibcmd.version
                native_ibcmd_path = (Get-NormalizedExecutablePath -Path ([string]$child.tools.native_ibcmd.path) -Label 'Child native ibcmd path')
                native_ibcmd_sha256 = $child.tools.native_ibcmd.sha256
                candidate_sha256 = $child.tools.candidate.sha256
                sqlcmd_path = (Get-NormalizedExecutablePath -Path ([string]$child.tools.sqlcmd.path) -Label 'Child sqlcmd path')
                sqlcmd_version = $child.tools.sqlcmd.version
                sqlcmd_sha256 = $child.tools.sqlcmd.sha256
                bcp_path = (Get-NormalizedExecutablePath -Path ([string]$child.tools.bcp.path) -Label 'Child bcp path')
                bcp_version = $child.tools.bcp.version
                bcp_sha256 = $child.tools.bcp.sha256
                child_log = $childLog
            })
            [void]$matrixPaths.Add((Join-Path $runDirectory ([string]$child.artifacts.matrix)))
            $matrixManifest.child_manifests = @($children)
            Write-AtomicJson -Path $manifestPath -Object $matrixManifest
        } catch {
            Complete-FailedStep -Step $childStep -ErrorRecord $_ -Manifest $matrixManifest -ManifestPath $manifestPath
            throw
        }
    }

    $matrixManifest.child_compatibility = Assert-CompatibleChildManifests -Children @($children) -ExpectedScope $Scope
    Write-AtomicJson -Path $manifestPath -Object $matrixManifest
    foreach ($path in $matrixPaths) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing child matrix: $path" }
    }

    $matrixJson = Join-Path $matrixRoot 'parity-matrix.json'
    $matrixMarkdown = Join-Path $matrixRoot 'parity-matrix.md'
    $mergeLog = Join-Path $logsRoot 'merge.log'
    $mergeArgs = @('source-diff-matrix-merge') + @($matrixPaths) + @('--output', $matrixJson, '--markdown', $matrixMarkdown)
    $mergeStep = [ordered]@{
        name = 'merge'
        status = 'running'
        started_utc = Get-UtcNow
        ended_utc = $null
        exit_code = $null
        exception = $null
        log = 'logs/merge.log'
        arguments = $mergeArgs
        artifacts = @('parity-matrix.json', 'parity-matrix.md')
    }
    [void]$steps.Add($mergeStep)
    Write-AtomicJson -Path $manifestPath -Object $matrixManifest
    try {
        & $ExePath @mergeArgs *>&1 |
            ForEach-Object { Protect-SensitiveText $_ } |
            Tee-Object -FilePath $mergeLog |
            Write-Host
        $mergeStep.exit_code = $LASTEXITCODE
        $mergeStep.ended_utc = Get-UtcNow
        if ($LASTEXITCODE -ne 0) {
            $mergeStep.status = 'failed'
            Write-AtomicJson -Path $manifestPath -Object $matrixManifest
            throw "source-diff-matrix-merge failed with exit code $LASTEXITCODE"
        }
        $mergeStep.status = 'passed'
        $matrixManifest.artifacts = [ordered]@{
            matrix = 'parity-matrix.json'
            markdown = 'parity-matrix.md'
            merge_log = 'logs/merge.log'
        }
        $mergedMatrix = Get-Content -Raw -LiteralPath $matrixJson | ConvertFrom-Json -ErrorAction Stop
        $mergedRuns = @($mergedMatrix.runs)
        $matrixManifest.parity_zero = (
            $mergedRuns.Count -eq 2 -and
            @($mergedRuns | Where-Object {
                -not $_.full -or
                [int64]$_.raw_summary.different -ne 0 -or
                [int64]$_.raw_summary.left_only -ne 0 -or
                [int64]$_.raw_summary.right_only -ne 0
            }).Count -eq 0
        )
        $matrixManifest.release_eligible = (
            $Scope -eq 'full' -and
            $RequireCompleteRootMetadata -and
            $matrixManifest.child_compatibility.status -eq 'passed' -and
            $matrixManifest.child_compatibility.release_proof -eq $true -and
            $matrixManifest.child_compatibility.distinct_databases -eq $true -and
            $matrixManifest.parity_zero
        )
        $matrixManifest.result_class = if ($matrixManifest.release_eligible) { 'release' } else { 'diagnostic' }
        $resultClass = $matrixManifest.result_class
        $matrixManifest.artifact_sha256 = [ordered]@{
            matrix = (Get-FileSha256 -Path $matrixJson)
            markdown = (Get-FileSha256 -Path $matrixMarkdown)
            merge_log = (Get-FileSha256 -Path $mergeLog)
            child_manifests = @($children | ForEach-Object {
                [ordered]@{
                    database = $_.database
                    manifest = (Get-FileSha256 -Path $_.manifest)
                    log = (Get-FileSha256 -Path $_.child_log)
                }
            })
        }
        $matrixManifest.status = 'passed'
        Write-AtomicJson -Path $manifestPath -Object $matrixManifest
    } catch {
        if ($mergeStep.status -eq 'running') {
            Complete-FailedStep -Step $mergeStep -ErrorRecord $_ -Manifest $matrixManifest -ManifestPath $manifestPath
        }
        throw
    }
} catch {
    $matrixManifest.status = 'failed'
    $matrixManifest.failure = Protect-SensitiveText $_.Exception.Message
    throw
} finally {
    $matrixManifest.finished_utc = Get-UtcNow
    Write-AtomicJson -Path $manifestPath -Object $matrixManifest
}

Write-Host "Two-database parity matrix completed: $matrixRoot ($resultClass)"
