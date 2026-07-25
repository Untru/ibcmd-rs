<#
.SYNOPSIS
Builds a deterministic canonical-coverage bootstrap from FeatureSemantics JSON.

.DESCRIPTION
Every exact namespace/classifier/feature key receives an explicit mapping. New
keys are deliberately emitted as unsupported implementation-coverage
placeholders; this generator does not infer EDT semantics.
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [Alias('InputCorpus', 'InputJson', 'InputPath')]
    [string]$InputFeatureSemantics,

    [Parameter(Mandatory = $true)]
    [Alias('OutputCorpus', 'OutputJson', 'OutputPath')]
    [string]$OutputCoverage,

    [Alias('ExistingCorpus', 'ExistingJson', 'ExistingPath')]
    [string]$ExistingCoverage
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-JsonObject {
    param([Parameter(Mandatory = $true)] [string]$Path)

    try {
        $value = (Get-Content -LiteralPath $Path -Raw -Encoding utf8) |
            ConvertFrom-Json -AsHashtable -Depth 100
    }
    catch {
        throw "Unable to read JSON '$Path': $($_.Exception.Message)"
    }

    if (-not ($value -is [System.Collections.IDictionary])) {
        throw "JSON document '$Path' must contain an object."
    }
    return $value
}

function Get-RequiredValue {
    param(
        [Parameter(Mandatory = $true)] [System.Collections.IDictionary]$Object,
        [Parameter(Mandatory = $true)] [string]$Name,
        [Parameter(Mandatory = $true)] [string]$Context
    )

    if (-not $Object.Contains($Name) -or $null -eq $Object[$Name]) {
        throw "$Context is missing '$Name'."
    }
    return $Object[$Name]
}

function Get-RequiredText {
    param(
        [Parameter(Mandatory = $true)] [System.Collections.IDictionary]$Object,
        [Parameter(Mandatory = $true)] [string]$Name,
        [Parameter(Mandatory = $true)] [string]$Context
    )

    $value = [string](Get-RequiredValue -Object $Object -Name $Name -Context $Context)
    if ([string]::IsNullOrWhiteSpace($value)) {
        throw "$Context has an empty '$Name'."
    }
    return $value
}

function Get-KeyId {
    param(
        [Parameter(Mandatory = $true)] [string]$NamespaceUri,
        [Parameter(Mandatory = $true)] [string]$Classifier,
        [Parameter(Mandatory = $true)] [string]$Feature
    )

    return "$NamespaceUri$([char]0x1f)$Classifier$([char]0x1f)$Feature"
}

function Test-PortablePath {
    param(
        [Parameter(Mandatory = $true)] [string]$Path,
        [Parameter(Mandatory = $true)] [string]$Context
    )

    if ($Path -match '(?i)(?:^[A-Z]:[\\/]|^\\\\|^file:)') {
        throw "$Context must be a portable repository resource, not '$Path'."
    }
}

function Get-CoverageFamily {
    param(
        [Parameter(Mandatory = $true)] [string]$Bundle,
        [Parameter(Mandatory = $true)] [string]$NamespaceUri
    )

    $identity = "$Bundle`n$NamespaceUri".ToLowerInvariant()
    if ($identity -match '(?:^|[./:_-])forms?(?:[./:_-]|$)') {
        return 'forms'
    }
    if ($identity -match '(?:^|[./:_-])metadata(?:[./:_-]|$)|(?:^|[./:_-])md(?:[./:_-]|$)') {
        return 'metadata'
    }
    if ($identity -match '(?:^|[./:_-])dcs(?:[./:_-]|$)|data[-._]?composition') {
        return 'dcs'
    }
    if ($identity -match '(?:^|[./:_-])mxl(?:[./:_-]|$)|spreadsheet|table[-._]?document') {
        return 'mxl'
    }
    if ($identity -match '(?:^|[./:_-])common(?:[./:_-]|$)|(?:^|[./:_-])core(?:[./:_-]|$)') {
        return 'common'
    }
    return 'other'
}

function Test-BootstrapPlaceholder {
    param([Parameter(Mandatory = $true)] [System.Collections.IDictionary]$Entry)

    if (-not $Entry.Contains('status') -or $Entry['status'] -ne 'unsupported') {
        return $false
    }
    if (-not $Entry.Contains('diagnosticCode') -or $Entry['diagnosticCode'] -ne 'schema.unmapped') {
        return $false
    }
    if (-not $Entry.Contains('evidence') -or -not ($Entry['evidence'] -is [System.Collections.IDictionary])) {
        return $false
    }
    return $Entry['evidence'].Contains('kind') -and $Entry['evidence']['kind'] -eq 'coverage-bootstrap'
}

function ConvertTo-NormalizedCoverageEntry {
    param([Parameter(Mandatory = $true)] [System.Collections.IDictionary]$Entry)

    $key = Get-RequiredValue -Object $Entry -Name 'key' -Context 'coverage entry'
    if (-not ($key -is [System.Collections.IDictionary])) {
        throw 'coverage entry key must be an object.'
    }
    $evidence = Get-RequiredValue -Object $Entry -Name 'evidence' -Context 'coverage entry'
    if (-not ($evidence -is [System.Collections.IDictionary])) {
        throw 'coverage entry evidence must be an object.'
    }

    $sources = @()
    if ($evidence.Contains('sources') -and $null -ne $evidence['sources']) {
        $sources = @($evidence['sources'] | ForEach-Object { [string]$_ } | Sort-Object)
    }

    return [ordered]@{
        key = [ordered]@{
            namespaceUri = Get-RequiredText -Object $key -Name 'namespaceUri' -Context 'coverage key'
            classifier = Get-RequiredText -Object $key -Name 'classifier' -Context 'coverage key'
            feature = Get-RequiredText -Object $key -Name 'feature' -Context 'coverage key'
        }
        family = Get-RequiredText -Object $Entry -Name 'family' -Context 'coverage entry'
        status = Get-RequiredText -Object $Entry -Name 'status' -Context 'coverage entry'
        canonicalType = if ($Entry.Contains('canonicalType')) { $Entry['canonicalType'] } else { $null }
        canonicalField = if ($Entry.Contains('canonicalField')) { $Entry['canonicalField'] } else { $null }
        opaquePlacement = if ($Entry.Contains('opaquePlacement')) { $Entry['opaquePlacement'] } else { $null }
        diagnosticCode = if ($Entry.Contains('diagnosticCode')) { $Entry['diagnosticCode'] } else { $null }
        evidence = [ordered]@{
            status = Get-RequiredText -Object $evidence -Name 'status' -Context 'coverage evidence'
            kind = Get-RequiredText -Object $evidence -Name 'kind' -Context 'coverage evidence'
            sources = $sources
            note = if ($evidence.Contains('note')) { $evidence['note'] } else { $null }
        }
    }
}

function New-BootstrapCoverageEntry {
    param(
        [Parameter(Mandatory = $true)] [string]$NamespaceUri,
        [Parameter(Mandatory = $true)] [string]$Classifier,
        [Parameter(Mandatory = $true)] [string]$Feature,
        [Parameter(Mandatory = $true)] [string]$Family,
        [Parameter(Mandatory = $true)] [string]$Resource
    )

    Test-PortablePath -Path $Resource -Context 'feature semantics resource'
    return [ordered]@{
        key = [ordered]@{
            namespaceUri = $NamespaceUri
            classifier = $Classifier
            feature = $Feature
        }
        family = $Family
        status = 'unsupported'
        canonicalType = $null
        canonicalField = $null
        opaquePlacement = $null
        diagnosticCode = 'schema.unmapped'
        evidence = [ordered]@{
            status = 'verified'
            kind = 'coverage-bootstrap'
            sources = @($Resource, 'tools/generate-canonical-coverage.ps1')
            note = 'Implementation coverage placeholder; not EDT semantics.'
        }
    }
}

$input = Read-JsonObject -Path $InputFeatureSemantics
$inputSource = Get-RequiredValue -Object $input -Name 'source' -Context 'feature semantics corpus'
if (-not ($inputSource -is [System.Collections.IDictionary])) {
    throw 'feature semantics corpus source must be an object.'
}
$inputPackages = @(Get-RequiredValue -Object $input -Name 'packages' -Context 'feature semantics corpus')

$features = @{}
foreach ($package in $inputPackages) {
    if (-not ($package -is [System.Collections.IDictionary])) {
        throw 'feature semantics package must be an object.'
    }
    $bundle = Get-RequiredText -Object $package -Name 'bundle' -Context 'feature semantics package'
    $resource = Get-RequiredText -Object $package -Name 'resource' -Context 'feature semantics package'
    $namespaceUri = Get-RequiredText -Object $package -Name 'namespaceUri' -Context 'feature semantics package'
    $family = Get-CoverageFamily -Bundle $bundle -NamespaceUri $namespaceUri
    $classifiers = @(Get-RequiredValue -Object $package -Name 'classifiers' -Context 'feature semantics package')

    foreach ($classifier in $classifiers) {
        if (-not ($classifier -is [System.Collections.IDictionary])) {
            throw 'feature semantics classifier must be an object.'
        }
        $classifierName = Get-RequiredText -Object $classifier -Name 'name' -Context 'feature semantics classifier'
        $declaredFeatures = @(Get-RequiredValue -Object $classifier -Name 'features' -Context 'feature semantics classifier')
        foreach ($feature in $declaredFeatures) {
            if (-not ($feature -is [System.Collections.IDictionary])) {
                throw 'feature semantics feature must be an object.'
            }
            $featureName = Get-RequiredText -Object $feature -Name 'name' -Context 'feature semantics feature'
            $id = Get-KeyId -NamespaceUri $namespaceUri -Classifier $classifierName -Feature $featureName
            if ($features.ContainsKey($id)) {
                throw "Feature semantics corpus contains duplicate key '$namespaceUri / $classifierName / $featureName'."
            }
            $features[$id] = [ordered]@{
                namespaceUri = $namespaceUri
                classifier = $classifierName
                feature = $featureName
                family = $family
                resource = $resource
            }
        }
    }
}

$existingEntries = @{}
if (-not [string]::IsNullOrWhiteSpace($ExistingCoverage)) {
    $existing = Read-JsonObject -Path $ExistingCoverage
    $entries = @(Get-RequiredValue -Object $existing -Name 'entries' -Context 'existing coverage corpus')
    foreach ($entry in $entries) {
        if (-not ($entry -is [System.Collections.IDictionary])) {
            throw 'existing coverage entry must be an object.'
        }
        $normalized = ConvertTo-NormalizedCoverageEntry -Entry $entry
        $key = $normalized.key
        $id = Get-KeyId -NamespaceUri $key.namespaceUri -Classifier $key.classifier -Feature $key.feature
        if ($existingEntries.ContainsKey($id)) {
            throw "Existing coverage corpus contains duplicate key '$($key.namespaceUri) / $($key.classifier) / $($key.feature)'."
        }
        if (-not $features.ContainsKey($id)) {
            throw "Existing coverage corpus contains stale key '$($key.namespaceUri) / $($key.classifier) / $($key.feature)'."
        }
        $existingEntries[$id] = $normalized
    }
}

$entries = @()
foreach ($id in $features.Keys) {
    $feature = $features[$id]
    if ($existingEntries.ContainsKey($id) -and -not (Test-BootstrapPlaceholder -Entry $existingEntries[$id])) {
        $entries += $existingEntries[$id]
    }
    else {
        $entries += New-BootstrapCoverageEntry -NamespaceUri $feature.namespaceUri -Classifier $feature.classifier -Feature $feature.feature -Family $feature.family -Resource $feature.resource
    }
}
$entries = @($entries | Sort-Object @{ Expression = { [string]$_.key.namespaceUri }; Ascending = $true }, @{ Expression = { [string]$_.key.classifier }; Ascending = $true }, @{ Expression = { [string]$_.key.feature }; Ascending = $true })

$summary = [ordered]@{
    entries = $entries.Count
    typed = @($entries | Where-Object { $_.status -eq 'typed' }).Count
    opaqueLossless = @($entries | Where-Object { $_.status -eq 'opaque-lossless' }).Count
    unsupported = @($entries | Where-Object { $_.status -eq 'unsupported' }).Count
    platformOnly = @($entries | Where-Object { $_.status -eq 'platform-only' }).Count
}

$corpus = [ordered]@{
    schemaVersion = 1
    source = [ordered]@{
        product = Get-RequiredText -Object $inputSource -Name 'product' -Context 'feature semantics source'
        release = Get-RequiredText -Object $inputSource -Name 'release' -Context 'feature semantics source'
        derivation = 'deterministic canonical implementation coverage bootstrap; placeholders are not EDT semantics'
    }
    summary = $summary
    entries = $entries
}

$json = ($corpus | ConvertTo-Json -Depth 16).Replace("`r`n", "`n") + "`n"
if ($json -match '(?i)(?:^|[^A-Za-z0-9_])[A-Z]:[\\/]|\\\\[^\\]|file:') {
    throw 'Refusing to write non-portable absolute path to canonical coverage.'
}

$outputPath = [System.IO.Path]::GetFullPath($OutputCoverage)
$outputParent = [System.IO.Path]::GetDirectoryName($outputPath)
[System.IO.Directory]::CreateDirectory($outputParent) | Out-Null
[System.IO.File]::WriteAllText($outputPath, $json, [System.Text.UTF8Encoding]::new($false))
Write-Host "Generated $($entries.Count) canonical coverage entries at $outputPath"
