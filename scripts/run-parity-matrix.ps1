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
    [switch]$RequireCompleteRootMetadata,
    [switch]$RequireCompleteSourceAssets
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
        $jsonLiteral = [string](ConvertTo-Json -InputObject $secretValue -Compress)
        if ($jsonLiteral.Length -ge 2 -and $jsonLiteral[0] -eq '"' -and $jsonLiteral[$jsonLiteral.Length - 1] -eq '"') {
            $jsonEscapedValue = $jsonLiteral.Substring(1, $jsonLiteral.Length - 2)
            if (-not [string]::IsNullOrEmpty($jsonEscapedValue)) {
                $text = $text.Replace($jsonEscapedValue, '<redacted>')
            }
        }
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
        '(?i)(--(?:db-pwd|sql-pwd|password|pwd)(?:-env)?)(?:=|\s+)(?:"(?:\\.|[^"\\])*"|(?:\\.|[^\s"\\])+)',
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

function ConvertFrom-WindowsExtendedLengthPath {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) { return $Path }

    $extendedPrefix = '\\?\'
    $extendedUncPrefix = '\\?\UNC\'
    foreach ($unsupportedPrefix in @('\\.\', '\??\', '\\??\')) {
        if ($Path.StartsWith($unsupportedPrefix, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Unsupported Windows device/NT executable path namespace: $Path"
        }
    }
    if (-not $Path.StartsWith($extendedPrefix, [StringComparison]::Ordinal)) {
        return $Path
    }
    if ($Path.IndexOf('/') -ge 0) {
        throw "Invalid Windows extended-length executable path: $Path"
    }

    $relativeComponents = @()
    $converted = $null
    if ($Path.StartsWith($extendedUncPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        $tail = $Path.Substring($extendedUncPrefix.Length)
        $components = @([regex]::Split($tail, '\\'))
        if ($components.Count -lt 3 -or
            [string]::IsNullOrEmpty($components[0]) -or
            [string]::IsNullOrEmpty($components[1])) {
            throw "Malformed Windows extended-length UNC executable path: $Path"
        }
        $relativeComponents = $components
        $converted = '\\' + $tail
    } elseif ($Path.Length -ge 8 -and
        $Path.StartsWith($extendedPrefix, [StringComparison]::Ordinal) -and
        [char]::IsLetter($Path[4]) -and $Path[5] -eq ':' -and $Path[6] -eq '\') {
        $relative = $Path.Substring(7)
        $relativeComponents = @([regex]::Split($relative, '\\'))
        $converted = $Path.Substring($extendedPrefix.Length)
    } else {
        throw "Unsupported Windows extended-length executable path namespace: $Path"
    }

    foreach ($component in $relativeComponents) {
        if ([string]::IsNullOrEmpty($component) -or
            $component -eq '.' -or $component -eq '..' -or
            $component.EndsWith('.', [StringComparison]::Ordinal) -or
            $component.EndsWith(' ', [StringComparison]::Ordinal) -or
            $component.IndexOf(':') -ge 0 -or
            [Management.Automation.WildcardPattern]::ContainsWildcardCharacters($component)) {
            throw "Ambiguous Windows extended-length executable path component: $Path"
        }
    }
    return $converted
}

function Get-NormalizedExecutablePath {
    param([string]$Path, [string]$Label)
    $identityPath = ConvertFrom-WindowsExtendedLengthPath $Path
    if ([string]::IsNullOrWhiteSpace($identityPath) -or $identityPath.IndexOf('/') -ge 0 -or
        [Management.Automation.WildcardPattern]::ContainsWildcardCharacters($identityPath)) {
        throw "$Label must be an absolute resolved executable path."
    }
    $driveRooted = $identityPath.Length -ge 3 -and
        [char]::IsLetter($identityPath[0]) -and $identityPath[1] -eq ':' -and $identityPath[2] -eq '\'
    $uncRooted = $identityPath.StartsWith('\\', [StringComparison]::Ordinal)
    if (-not ($driveRooted -or $uncRooted)) {
        throw "$Label must be an absolute resolved executable path."
    }
    $components = if ($driveRooted) {
        @([regex]::Split($identityPath.Substring(3), '\\'))
    } else {
        @([regex]::Split($identityPath.Substring(2), '\\'))
    }
    if ($components.Count -lt $(if ($driveRooted) { 1 } else { 3 })) {
        throw "$Label has an incomplete drive or UNC route."
    }
    foreach ($component in $components) {
        if ([string]::IsNullOrEmpty($component) -or
            $component -eq '.' -or $component -eq '..' -or
            $component.EndsWith('.', [StringComparison]::Ordinal) -or
            $component.EndsWith(' ', [StringComparison]::Ordinal) -or
            $component.IndexOf(':') -ge 0) {
            throw "$Label contains an ambiguous path component."
        }
    }

    $fullPath = [IO.Path]::GetFullPath($identityPath)
    $root = [IO.Path]::GetPathRoot($fullPath)
    $rootItem = Get-Item -LiteralPath $root -Force -ErrorAction Stop
    if (($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "$Label root is a reparse point."
    }
    $current = $root
    $relative = $fullPath.Substring($root.Length)
    $item = $null
    foreach ($component in @([regex]::Split($relative, '\\'))) {
        $current = Join-Path $current $component
        $item = Get-Item -LiteralPath $current -Force -ErrorAction Stop
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "$Label crosses a reparse point: $current"
        }
    }
    if ($null -eq $item -or $item.PSIsContainer) { throw "$Label must be an executable file path." }
    return $fullPath
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

function Test-NonNegativeBoundedInteger {
    param([AllowNull()]$Value)
    if ($null -eq $Value) { return $false }
    if ($Value.GetType().FullName -notin @(
        'System.Byte',
        'System.SByte',
        'System.Int16',
        'System.UInt16',
        'System.Int32',
        'System.UInt32',
        'System.Int64',
        'System.UInt64'
    )) {
        return $false
    }
    try {
        $number = [decimal]$Value
        return $number -ge 0 -and $number -le [decimal][int64]::MaxValue
    } catch {
        return $false
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
    if ([int]$child.protocol_version -ne 3) { throw "Child manifest protocol is not supported for release: $Path" }
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
        [ordered]@{ path=[string]$child.nested_runtime_calls.candidate_manifest; sha=[string]$child.nested_runtime_calls.candidate_manifest_sha256; label='candidate subprocess journal' },
        [ordered]@{ path=[string]$child.artifacts.candidate_dump_manifest; sha=[string]$child.artifact_sha256.candidate_dump_manifest; label='candidate source asset manifest' },
        [ordered]@{ path=[string]$child.artifacts.raw_diff; sha=[string]$child.artifact_sha256.raw_diff; label='raw diff artifact' }
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
    $sourceEvidence = $child.source_assets
    $sourceReport = $sourceEvidence.report
    if ([string]$sourceEvidence.evidence_manifest -cne [string]$child.artifacts.candidate_dump_manifest -or
        -not [StringComparer]::OrdinalIgnoreCase.Equals(
            [string]$sourceEvidence.evidence_manifest_sha256,
            [string]$child.artifact_sha256.candidate_dump_manifest
        )) {
        throw "Child source asset evidence reference does not match its artifact binding: $Path"
    }
    if ($null -eq $sourceReport -or [int]$sourceReport.schema_version -ne 1) {
        throw "Child source asset completeness evidence is missing or unsupported: $Path"
    }
    if ([string]$sourceReport.scope -cne $ExpectedScope) {
        throw "Child source asset completeness scope does not match the requested scope: $Path"
    }
    $sourceCounts = [ordered]@{}
    foreach ($count in @('expected', 'emitted', 'opaque', 'missing', 'opaque_property_count')) {
        if ($sourceReport.PSObject.Properties.Name -notcontains $count -or
            -not (Test-NonNegativeBoundedInteger $sourceReport.$count)) {
            throw "Child source asset completeness count '$count' is missing, negative, non-integral, or out of range: $Path"
        }
        $sourceCounts[$count] = [decimal]$sourceReport.$count
    }
    foreach ($reason in @($sourceReport.reasons.PSObject.Properties)) {
        if (-not (Test-NonNegativeBoundedInteger $reason.Value)) {
            throw "Child source asset reason count '$($reason.Name)' is negative, non-integral, or out of range: $Path"
        }
    }
    $dispositionCount = $sourceCounts.emitted + $sourceCounts.opaque + $sourceCounts.missing
    if ($dispositionCount -gt [decimal][int64]::MaxValue -or
        $sourceCounts.expected -ne $dispositionCount) {
        throw "Child source asset completeness count invariant is violated: $Path"
    }
    if ($sourceCounts.opaque_property_count -lt $sourceCounts.opaque) {
        throw "Child source asset opaque property count is invalid: $Path"
    }
    $derivedSourceStatus = if ($sourceReport.candidate_set_complete -ne $true) {
        'unknown'
    } elseif ($sourceCounts.opaque -eq 0 -and $sourceCounts.missing -eq 0) {
        'complete'
    } else {
        'partial'
    }
    if ([string]$sourceReport.status -cne $derivedSourceStatus) {
        throw "Child source asset completeness status disagrees with its counts: $Path"
    }
    $sourceComplete = (
        [string]$sourceReport.scope -ceq 'full' -and
        $sourceReport.candidate_set_complete -eq $true -and
        [string]$sourceReport.status -ceq 'complete' -and
        $sourceCounts.opaque -eq 0 -and
        $sourceCounts.missing -eq 0
    )
    if ($sourceEvidence.complete -ne $sourceComplete) {
        throw "Child source asset completeness summary disagrees with its report: $Path"
    }
    if (-not $sourceComplete -and (
        $child.release_eligible -eq $true -or [string]$child.result_class -cne 'diagnostic'
    )) {
        throw "Partial child source assets must remain diagnostic and release-ineligible: $Path"
    }
    if ($null -eq $child.source_asset_gate -or
        $child.source_asset_gate.requested -notin @($true, $false) -or
        $child.source_asset_gate.passed -notin @($true, $false)) {
        throw "Child source asset strict-gate attestation is missing or invalid: $Path"
    }
    if ($child.source_asset_gate.passed -eq $true -and (
        $child.source_asset_gate.requested -ne $true -or -not $sourceComplete
    )) {
        throw "Child source asset strict-gate attestation is incoherent: $Path"
    }
    $rawDiffPath = [IO.Path]::GetFullPath((Join-Path $childRoot ([string]$child.artifacts.raw_diff)))
    $rawDiff = Get-Content -Raw -LiteralPath $rawDiffPath | ConvertFrom-Json -ErrorAction Stop
    if ([string]::IsNullOrWhiteSpace([string]$rawDiff.left_root) -or
        [string]::IsNullOrWhiteSpace([string]$rawDiff.right_root) -or
        $rawDiff.PSObject.Properties.Name -notcontains 'differences') {
        throw "Child raw diff artifact has an invalid schema: $Path"
    }
    $rawCounts = [ordered]@{}
    foreach ($count in @('left_only', 'right_only', 'different', 'unchanged')) {
        if ($rawDiff.summary.PSObject.Properties.Name -notcontains $count -or
            -not (Test-NonNegativeBoundedInteger $rawDiff.summary.$count)) {
            throw "Child raw diff summary count '$count' is missing, negative, non-integral, or out of range: $Path"
        }
        $rawCounts[$count] = [decimal]$rawDiff.summary.$count
    }
    $computedRawCounts = [ordered]@{ left_only=0; right_only=0; different=0; unchanged=0 }
    foreach ($difference in @($rawDiff.differences)) {
        $status = [string]$difference.status
        if ($status -notin @('left_only', 'right_only', 'different', 'unchanged')) {
            throw "Child raw diff artifact contains an invalid difference status: $Path"
        }
        $computedRawCounts[$status] = [decimal]$computedRawCounts[$status] + 1
        if ($computedRawCounts[$status] -gt [decimal][int64]::MaxValue) {
            throw "Child raw diff artifact contains too many differences: $Path"
        }
    }
    foreach ($count in $rawCounts.Keys) {
        if ($rawCounts[$count] -ne $computedRawCounts[$count]) {
            throw "Child raw diff summary count '$count' disagrees with its difference records: $Path"
        }
    }
    foreach ($count in @('different', 'left_only', 'right_only')) {
        if ($child.raw_parity.PSObject.Properties.Name -notcontains $count -or
            -not (Test-NonNegativeBoundedInteger $child.raw_parity.$count) -or
            [decimal]$child.raw_parity.$count -ne $rawCounts[$count]) {
            throw "Child raw parity count '$count' is missing, invalid, or disagrees with the hashed raw diff: $Path"
        }
    }
    $rawParityZero = (
        $rawCounts.different -eq 0 -and
        $rawCounts.left_only -eq 0 -and
        $rawCounts.right_only -eq 0
    )
    if ($child.raw_parity.zero -ne $rawParityZero) {
        throw "Child raw parity zero attestation disagrees with its counts: $Path"
    }
    if ($child.release_eligible -eq $true -and (
        [string]$child.result_class -cne 'release' -or
        $child.source_asset_gate.requested -ne $true -or
        $child.source_asset_gate.passed -ne $true -or
        -not $sourceComplete -or
        -not $rawParityZero
    )) {
        throw "Child release classification lacks strict source-assets and zero-parity attestation: $Path"
    }
    if ($child.release_eligible -ne $true -and [string]$child.result_class -cne 'diagnostic') {
        throw "Release-ineligible child must be classified diagnostic: $Path"
    }
    $candidateDumpManifestPath = [IO.Path]::GetFullPath((Join-Path $childRoot ([string]$child.artifacts.candidate_dump_manifest)))
    $candidateDump = Get-Content -Raw -LiteralPath $candidateDumpManifestPath | ConvertFrom-Json -ErrorAction Stop
    if (-not [StringComparer]::Ordinal.Equals([string]$candidateDump.server, $ExpectedServer) -or
        -not [StringComparer]::Ordinal.Equals([string]$candidateDump.database, $ExpectedDatabase)) {
        throw "Child hashed candidate manifest database identity does not match the requested server/database: $Path"
    }
    if ($null -eq $candidateDump.source_assets -or
        [string]$candidateDump.source_assets.status -cne [string]$sourceReport.status -or
        [int64]$candidateDump.source_assets.expected -ne [int64]$sourceReport.expected -or
        [int64]$candidateDump.source_assets.emitted -ne [int64]$sourceReport.emitted -or
        [int64]$candidateDump.source_assets.opaque -ne [int64]$sourceReport.opaque -or
        [int64]$candidateDump.source_assets.missing -ne [int64]$sourceReport.missing) {
        throw "Child source asset report does not match the hashed candidate manifest: $Path"
    }
    $candidateSourceJson = $candidateDump.source_assets | ConvertTo-Json -Depth 20 -Compress
    $childSourceJson = $sourceReport | ConvertTo-Json -Depth 20 -Compress
    if (-not [StringComparer]::Ordinal.Equals($candidateSourceJson, $childSourceJson)) {
        throw "Child source asset report payload is not identical to the hashed candidate manifest: $Path"
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
    $childMatrix = Get-Content -Raw -LiteralPath $childMatrixPath | ConvertFrom-Json -ErrorAction Stop
    if (-not (Test-NonNegativeBoundedInteger $childMatrix.schema_version) -or
        [decimal]$childMatrix.schema_version -ne 1) {
        throw "Child matrix artifact schema version is unsupported: $Path"
    }
    $matrixRuns = @($childMatrix.runs)
    if ($matrixRuns.Count -ne 1) {
        throw "Child matrix artifact must contain exactly one parity run: $Path"
    }
    $matrixRun = $matrixRuns[0]
    if (-not [StringComparer]::Ordinal.Equals([string]$matrixRun.database, $ExpectedDatabase) -or
        -not [StringComparer]::Ordinal.Equals([string]$matrixRun.run_id, [string]$child.run_id) -or
        -not [StringComparer]::OrdinalIgnoreCase.Equals([string]$matrixRun.git_sha, [string]$child.git_sha) -or
        $matrixRun.full -ne ($ExpectedScope -eq 'full') -or
        -not [StringComparer]::Ordinal.Equals([string]$matrixRun.left_root, [string]$rawDiff.left_root) -or
        -not [StringComparer]::Ordinal.Equals([string]$matrixRun.right_root, [string]$rawDiff.right_root)) {
        throw "Child matrix parity run identity or scope disagrees with the child/raw-diff evidence: $Path"
    }
    foreach ($count in @('left_only', 'right_only', 'different', 'unchanged')) {
        if ($matrixRun.raw_summary.PSObject.Properties.Name -notcontains $count -or
            -not (Test-NonNegativeBoundedInteger $matrixRun.raw_summary.$count) -or
            [decimal]$matrixRun.raw_summary.$count -ne $rawCounts[$count]) {
            throw "Child matrix raw summary count '$count' disagrees with the hashed raw diff: $Path"
        }
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
    $sourceAssetsComplete = @($Children | Where-Object { $_.source_assets_complete -ne $true }).Count -eq 0
    $childrenReleaseAttested = @($Children | Where-Object {
        $_.result_class -cne 'release' -or
        $_.release_eligible -ne $true -or
        $_.source_asset_gate_requested -ne $true -or
        $_.source_asset_gate_passed -ne $true -or
        $_.raw_parity_zero -ne $true
    }).Count -eq 0
    return [ordered]@{
        status = 'passed'
        compared_children = $Children.Count
        scope = $ExpectedScope
        release_proof = ($ExpectedScope -eq 'full' -and $sourceAssetsComplete -and $childrenReleaseAttested)
        source_assets_complete = $sourceAssetsComplete
        children_release_attested = $childrenReleaseAttested
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
if ($RequireCompleteSourceAssets -and $Scope -ne 'full') {
    throw "RequireCompleteSourceAssets is available only for Scope 'full'."
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
    protocol_version = 3
    run_id = $RunId
    created_utc = Get-UtcNow
    status = 'running'
    scope = $Scope
    result_class = $resultClass
    release_gate_requested = ($Scope -eq 'full' -and $RequireCompleteRootMetadata)
    release_eligible = $false
    source_asset_gate = [ordered]@{ requested=[bool]$RequireCompleteSourceAssets; passed=$false }
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
        if ($RequireCompleteSourceAssets) { $params.RequireCompleteSourceAssets = $true }

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
                source_assets_complete = $child.source_assets.complete
                source_asset_gate_requested = $child.source_asset_gate.requested
                source_asset_gate_passed = $child.source_asset_gate.passed
                raw_parity_zero = $child.raw_parity.zero
                result_class = $child.result_class
                release_eligible = $child.release_eligible
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
    $matrixManifest.source_asset_gate.passed = (
        $matrixManifest.source_asset_gate.requested -eq $true -and
        $matrixManifest.child_compatibility.source_assets_complete -eq $true -and
        @($children | Where-Object {
            $_.source_asset_gate_requested -ne $true -or $_.source_asset_gate_passed -ne $true
        }).Count -eq 0
    )
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
            $matrixManifest.child_compatibility.source_assets_complete -eq $true -and
            $matrixManifest.child_compatibility.children_release_attested -eq $true -and
            $matrixManifest.source_asset_gate.requested -eq $true -and
            $matrixManifest.source_asset_gate.passed -eq $true -and
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
