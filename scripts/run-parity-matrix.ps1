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

function Read-ValidChildManifest {
    param([string]$Path, [string]$ExpectedScope)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Missing child manifest: $Path" }
    $child = Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json -ErrorAction Stop
    if ($child.status -ne 'passed') { throw "Child manifest is not successful: $Path" }
    if ($child.scope -ne $ExpectedScope) { throw "Child manifest scope '$($child.scope)' does not match expected '$ExpectedScope': $Path" }
    foreach ($property in @('git_sha', 'xml_version')) {
        if ([string]::IsNullOrWhiteSpace([string]$child.$property)) { throw "Child manifest misses ${property}: $Path" }
    }
    if ([string]::IsNullOrWhiteSpace([string]$child.tools.native_ibcmd.version)) {
        throw "Child manifest misses resolved native ibcmd version: $Path"
    }
    foreach ($tool in @('candidate', 'native_ibcmd')) {
        if ([string]$child.tools.$tool.sha256 -notmatch '^[0-9a-fA-F]{64}$') {
            throw "Child manifest misses valid ${tool} SHA-256: $Path"
        }
    }
    if (-not $child.artifacts.matrix) { throw "Child manifest misses matrix artifact: $Path" }
    return $child
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
        }
        if ($IntegratedAuth -or [string]::IsNullOrWhiteSpace($DbUser)) { $params.IntegratedAuth = $true }
        if ($IbcmdPath) { $params.IbcmdPath = $IbcmdPath }
        if ($RequireCompleteRootMetadata) { $params.RequireCompleteRootMetadata = $true }

        try {
            & $runner @params *>&1 |
                ForEach-Object { Protect-SensitiveText $_ } |
                Tee-Object -FilePath $childLog |
                Write-Host
            if ($LASTEXITCODE -ne 0) {
                throw "Parity run failed for database '$($database.name)' with exit code $LASTEXITCODE"
            }
            $child = Read-ValidChildManifest -Path $childManifestPath -ExpectedScope $Scope
            $childStep.status = 'passed'
            $childStep.ended_utc = Get-UtcNow
            $childStep.exit_code = 0
            [void]$children.Add([ordered]@{
                database = $database.name
                manifest = $childManifestPath
                git_sha = $child.git_sha
                xml_version = $child.xml_version
                native_ibcmd_version = $child.tools.native_ibcmd.version
                native_ibcmd_sha256 = $child.tools.native_ibcmd.sha256
                candidate_sha256 = $child.tools.candidate.sha256
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

    $reference = $children[0]
    foreach ($child in $children | Select-Object -Skip 1) {
        foreach ($field in @('git_sha', 'xml_version', 'native_ibcmd_version', 'native_ibcmd_sha256', 'candidate_sha256')) {
            if ([string]$child.$field -ne [string]$reference.$field) {
                throw "Child manifest mismatch for ${field}: '$($reference.database)' != '$($child.database)'. Merge blocked."
            }
        }
    }
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
