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

function ConvertTo-HashtableValue {
    param($Value)

    if ($null -eq $Value -or $Value -is [string] -or $Value.GetType().IsValueType) {
        return $Value
    }
    if ($Value -is [System.Collections.IDictionary]) {
        $result = @{}
        foreach ($key in $Value.Keys) {
            $result[[string]$key] = ConvertTo-HashtableValue $Value[$key]
        }
        return $result
    }
    if ($Value -is [System.Management.Automation.PSCustomObject]) {
        $result = @{}
        foreach ($property in $Value.PSObject.Properties) {
            $result[$property.Name] = ConvertTo-HashtableValue $property.Value
        }
        return $result
    }
    if ($Value -is [System.Collections.IEnumerable]) {
        $items = @($Value | ForEach-Object { ConvertTo-HashtableValue $_ })
        return ,$items
    }
    return $Value
}

function Read-JsonObject {
    param([Parameter(Mandatory = $true)] [string]$Path)

    try {
        $parsed = (Get-Content -LiteralPath $Path -Raw -Encoding utf8) | ConvertFrom-Json
        $value = ConvertTo-HashtableValue $parsed
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

$coverageRouteDefinitions = [ordered]@{
    'com._1c.g5.v8.dt.debug.model.core|class' = 'other'
    'com._1c.g5.v8.dt.dcs.expressions.model|class' = 'dcs'
    'com._1c.g5.v8.dt.dcs.model.appearancetemplate|class' = 'dcs'
    'com._1c.g5.v8.dt.dcs.model.areaTemplate|class' = 'dcs'
    'com._1c.g5.v8.dt.dcs.model.common|class' = 'dcs'
    'com._1c.g5.v8.dt.dcs.model.core|class' = 'dcs'
    'com._1c.g5.v8.dt.dcs.model.dbcopies|class' = 'dcs'
    'com._1c.g5.v8.dt.dcs.model.schema|class' = 'dcs'
    'com._1c.g5.v8.dt.dcs.model.settings|class' = 'dcs'
    'com._1c.g5.v8.dt.ql.dcs.model|class' = 'dcs'
    'com._1c.g5.v8.dt.form.layout.model.calculation.context|class' = 'forms'
    'com._1c.g5.v8.dt.form.layout.model.calculation.context|interface' = 'forms'
    'com._1c.g5.v8.dt.form.layout.model.description|class' = 'forms'
    'com._1c.g5.v8.dt.form.layout.model.description|interface' = 'forms'
    'com._1c.g5.v8.dt.form.layout.model.generation.context|class' = 'forms'
    'com._1c.g5.v8.dt.form.layout.model.generation.context|interface' = 'forms'
    'com._1c.g5.v8.dt.form.layout.model.transformation.context|class' = 'forms'
    'com._1c.g5.v8.dt.form.layout.model.transformation.context|interface' = 'forms'
    'com._1c.g5.v8.dt.form.mapping.model|class' = 'forms'
    'com._1c.g5.v8.dt.form.mapping.model|interface' = 'forms'
    'com._1c.g5.v8.dt.form.model|class' = 'forms'
    'com._1c.g5.v8.dt.form.model|interface' = 'forms'
    'com._1c.g5.v8.dt.aggregates.model|class' = 'other'
    'com._1c.g5.v8.dt.bp.scheme.model|class' = 'other'
    'com._1c.g5.v8.dt.bsl.model|class' = 'other'
    'com._1c.g5.v8.dt.cai.model|class' = 'other'
    'com._1c.g5.v8.dt.chart.model.timescale|class' = 'other'
    'com._1c.g5.v8.dt.chart.model|class' = 'other'
    'com._1c.g5.v8.dt.cmi.model.deriveddata|class' = 'other'
    'com._1c.g5.v8.dt.cmi.model|class' = 'other'
    'com._1c.g5.v8.dt.compare.model|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.area|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.attach|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.base.data|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.breakpoints|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.bsl.exceptions|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.calculations|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.dbgui.commands|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.foreground.data|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.measure|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.rdbg.request.response|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.rte.filter|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.rte.info|class' = 'other'
    'com._1c.g5.v8.dt.debug.model.virtual|class' = 'other'
    'com._1c.g5.v8.dt.dendrogram.model|class' = 'other'
    'com._1c.g5.v8.dt.ganttchart.model|class' = 'other'
    'com._1c.g5.v8.dt.geographicalschema.model|class' = 'other'
    'com._1c.g5.v8.dt.hpwa.model|class' = 'other'
    'com._1c.g5.v8.dt.lcore.model|class' = 'other'
    'com._1c.g5.v8.dt.mcore|class' = 'other'
    'com._1c.g5.v8.dt.mcore|interface' = 'other'
    'com._1c.g5.v8.dt.planner.model|class' = 'other'
    'com._1c.g5.v8.dt.platform.model|class' = 'other'
    'com._1c.g5.v8.dt.platform.services.model|class' = 'other'
    'com._1c.g5.v8.dt.ql.model|class' = 'other'
    'com._1c.g5.v8.dt.right.ql.model|class' = 'other'
    'com._1c.g5.v8.dt.right.templates.model|class' = 'other'
    'com._1c.g5.v8.dt.rights.model|class' = 'other'
    'com._1c.g5.v8.dt.scc.model|class' = 'other'
    'com._1c.g5.v8.dt.scc.model|interface' = 'other'
    'com._1c.g5.v8.dt.schedule.model|class' = 'other'
    'com._1c.g5.v8.dt.style.model|class' = 'other'
    'com._1c.g5.v8.dt.supply.settings.model|class' = 'other'
    'com._1c.g5.v8.dt.supply.settings.model|interface' = 'other'
    'com._1c.g5.v8.dt.v8help.model|class' = 'other'
    'com._1c.g5.v8.dt.ws.wsdefinitions.model|class' = 'other'
    'com._1c.g5.v8.dt.xdto.model|class' = 'other'
    'com._1c.g5.v8.dt.xdto.type.model|class' = 'other'
}
$CoverageRoutes = [System.Collections.Generic.Dictionary[string, string]]::new(
    [System.StringComparer]::Ordinal
)
foreach ($route in $coverageRouteDefinitions.GetEnumerator()) {
    $CoverageRoutes.Add([string]$route.Key, [string]$route.Value)
}

function Get-CoverageFamily {
    param(
        [Parameter(Mandatory = $true)] [string]$PackageName,
        [Parameter(Mandatory = $true)] [string]$ClassifierKind
    )

    $route = "$PackageName|$ClassifierKind"
    if (-not $CoverageRoutes.ContainsKey($route)) {
        throw "No canonical coverage route for package '$PackageName' / classifier kind '$ClassifierKind'."
    }
    return [string]$CoverageRoutes[$route]
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

function New-TypedCoverageEntry {
    param(
        [Parameter(Mandatory = $true)] [System.Collections.IDictionary]$Feature,
        [Parameter(Mandatory = $true)] [System.Collections.IDictionary]$Mapping
    )

    return [ordered]@{
        key = [ordered]@{
            namespaceUri = $Feature.namespaceUri
            classifier = $Feature.classifier
            feature = $Feature.feature
        }
        family = $Feature.family
        status = 'typed'
        canonicalType = $Mapping.canonicalType
        canonicalField = $Mapping.canonicalField
        opaquePlacement = $null
        diagnosticCode = $null
        evidence = [ordered]@{
            status = 'verified'
            kind = 'canonical-code-inspection-and-platform-evidence'
            sources = @(
                'crates/ibcmd-core/src/dcs.rs',
                $Mapping.policy,
                $Feature.resource
            ) | Sort-Object
            note = 'Typed canonical field with bounded constructor/deserialization validation and profile-gated XML emission.'
        }
    }
}

$settingsNamespaceUri = 'http://g5.1c.ru/v8/dt/data-composition-system/settings'
$typedCoverageDefinitions = @(
    @{ classifier = 'DataCompositionSettings'; feature = 'selection'; canonicalType = 'DcsSettings'; canonicalField = 'selection'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-selection-evidence.json' },
    @{ classifier = 'DataCompositionSettings'; feature = 'order'; canonicalType = 'DcsSettings'; canonicalField = 'order'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-order-evidence.json' },
    @{ classifier = 'DataCompositionSettings'; feature = 'filter'; canonicalType = 'DcsSettings'; canonicalField = 'filter'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-filter-evidence.json' },
    @{ classifier = 'DataCompositionSettings'; feature = 'conditionalAppearance'; canonicalType = 'DcsSettings'; canonicalField = 'conditional_appearance'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-conditional-appearance-evidence.json' },
    @{ classifier = 'DataCompositionSelectedFields'; feature = 'items'; canonicalType = 'DcsSelection'; canonicalField = 'items'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-selection-evidence.json' },
    @{ classifier = 'DataCompositionSelectedField'; feature = 'field'; canonicalType = 'DcsSelectedField'; canonicalField = 'field'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-selection-evidence.json' },
    @{ classifier = 'DataCompositionOrder'; feature = 'items'; canonicalType = 'DcsOrder'; canonicalField = 'items'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-order-evidence.json' },
    @{ classifier = 'DataCompositionOrder'; feature = 'viewMode'; canonicalType = 'DcsOrder'; canonicalField = 'view_mode'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-order-evidence.json' },
    @{ classifier = 'DataCompositionOrder'; feature = 'userSettingID'; canonicalType = 'DcsOrder'; canonicalField = 'user_setting_id'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-order-evidence.json' },
    @{ classifier = 'DataCompositionOrderItem'; feature = 'field'; canonicalType = 'DcsOrderField'; canonicalField = 'field'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-order-evidence.json' },
    @{ classifier = 'DataCompositionOrderItem'; feature = 'orderType'; canonicalType = 'DcsOrderField'; canonicalField = 'order_type'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-order-evidence.json' },
    @{ classifier = 'DataCompositionOrderItem'; feature = 'use'; canonicalType = 'DcsOrderField'; canonicalField = 'use_value'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-order-evidence.json' },
    @{ classifier = 'DataCompositionFilter'; feature = 'items'; canonicalType = 'DcsFilter'; canonicalField = 'items'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-filter-evidence.json' },
    @{ classifier = 'DataCompositionFilter'; feature = 'viewMode'; canonicalType = 'DcsFilter'; canonicalField = 'view_mode'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-filter-evidence.json' },
    @{ classifier = 'DataCompositionFilter'; feature = 'userSettingID'; canonicalType = 'DcsFilter'; canonicalField = 'user_setting_id'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-filter-evidence.json' },
    @{ classifier = 'DataCompositionFilterItem'; feature = 'comparisonType'; canonicalType = 'DcsFilterComparison'; canonicalField = 'comparison_type'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-filter-evidence.json' },
    @{ classifier = 'DataCompositionFilterItem'; feature = 'left'; canonicalType = 'DcsFilterComparison'; canonicalField = 'field'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-filter-evidence.json' },
    @{ classifier = 'DataCompositionFilterItem'; feature = 'right'; canonicalType = 'DcsFilterComparison'; canonicalField = 'right'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-filter-evidence.json' },
    @{ classifier = 'DataCompositionConditionalAppearance'; feature = 'items'; canonicalType = 'DcsConditionalAppearance'; canonicalField = 'items'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-conditional-appearance-evidence.json' },
    @{ classifier = 'DataCompositionConditionalAppearance'; feature = 'viewMode'; canonicalType = 'DcsConditionalAppearance'; canonicalField = 'view_mode'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-conditional-appearance-evidence.json' },
    @{ classifier = 'DataCompositionConditionalAppearance'; feature = 'userSettingID'; canonicalType = 'DcsConditionalAppearance'; canonicalField = 'user_setting_id'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-conditional-appearance-evidence.json' },
    @{ classifier = 'DataCompositionConditionalAppearanceItem'; feature = 'selection'; canonicalType = 'DcsConditionalAppearanceItem'; canonicalField = 'selected_field'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-conditional-appearance-evidence.json' },
    @{ classifier = 'DataCompositionConditionalAppearanceItem'; feature = 'filter'; canonicalType = 'DcsConditionalAppearanceItem'; canonicalField = 'filter'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-conditional-appearance-evidence.json' },
    @{ classifier = 'DataCompositionConditionalAppearanceItem'; feature = 'appearance'; canonicalType = 'DcsConditionalAppearanceItem'; canonicalField = 'appearance'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-conditional-appearance-evidence.json' },
    @{ namespaceUri = 'http://g5.1c.ru/v8/dt/form'; classifier = 'Form'; feature = 'conditionalAppearance'; canonicalType = 'DcsConditionalAppearance'; canonicalField = 'conditional_appearance'; policy = 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-form-attributes-conditional-appearance-evidence.json' }
)
$typedCoverageMappings = [System.Collections.Generic.Dictionary[string, object]]::new(
    [System.StringComparer]::Ordinal
)
foreach ($mapping in $typedCoverageDefinitions) {
    $namespaceUri = if ($mapping.ContainsKey('namespaceUri')) { $mapping.namespaceUri } else { $settingsNamespaceUri }
    $id = Get-KeyId -NamespaceUri $namespaceUri -Classifier $mapping.classifier -Feature $mapping.feature
    $typedCoverageMappings.Add($id, $mapping)
}

$input = Read-JsonObject -Path $InputFeatureSemantics
$inputSource = Get-RequiredValue -Object $input -Name 'source' -Context 'feature semantics corpus'
if (-not ($inputSource -is [System.Collections.IDictionary])) {
    throw 'feature semantics corpus source must be an object.'
}
$inputPackages = @(Get-RequiredValue -Object $input -Name 'packages' -Context 'feature semantics corpus')

$features = [System.Collections.Generic.Dictionary[string, object]]::new(
    [System.StringComparer]::Ordinal
)
foreach ($package in $inputPackages) {
    if (-not ($package -is [System.Collections.IDictionary])) {
        throw 'feature semantics package must be an object.'
    }
    $resource = Get-RequiredText -Object $package -Name 'resource' -Context 'feature semantics package'
    $packageName = Get-RequiredText -Object $package -Name 'packageName' -Context 'feature semantics package'
    $namespaceUri = Get-RequiredText -Object $package -Name 'namespaceUri' -Context 'feature semantics package'
    $classifiers = @(Get-RequiredValue -Object $package -Name 'classifiers' -Context 'feature semantics package')

    foreach ($classifier in $classifiers) {
        if (-not ($classifier -is [System.Collections.IDictionary])) {
            throw 'feature semantics classifier must be an object.'
        }
        $classifierName = Get-RequiredText -Object $classifier -Name 'name' -Context 'feature semantics classifier'
        $classifierKind = Get-RequiredText -Object $classifier -Name 'kind' -Context 'feature semantics classifier'
        $declaredFeatures = @(Get-RequiredValue -Object $classifier -Name 'features' -Context 'feature semantics classifier')
        if ($declaredFeatures.Count -eq 0) {
            continue
        }
        $family = Get-CoverageFamily -PackageName $packageName -ClassifierKind $classifierKind
        foreach ($feature in $declaredFeatures) {
            if (-not ($feature -is [System.Collections.IDictionary])) {
                throw 'feature semantics feature must be an object.'
            }
            $featureName = Get-RequiredText -Object $feature -Name 'name' -Context 'feature semantics feature'
            $featureKind = Get-RequiredText -Object $feature -Name 'kind' -Context 'feature semantics feature'
            $id = Get-KeyId -NamespaceUri $namespaceUri -Classifier $classifierName -Feature $featureName
            if ($features.ContainsKey($id)) {
                throw "Feature semantics corpus contains duplicate key '$namespaceUri / $classifierName / $featureName'."
            }
            $features[$id] = [ordered]@{
                namespaceUri = $namespaceUri
                classifier = $classifierName
                feature = $featureName
                family = $family
                package = $packageName
                classifierKind = $classifierKind
                featureKind = $featureKind
                resource = $resource
            }
        }
    }
}

$existingEntries = [System.Collections.Generic.Dictionary[string, object]]::new(
    [System.StringComparer]::Ordinal
)
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
    if ($typedCoverageMappings.ContainsKey($id)) {
        $entries += New-TypedCoverageEntry -Feature $feature -Mapping $typedCoverageMappings[$id]
    }
    elseif ($existingEntries.ContainsKey($id) -and -not (Test-BootstrapPlaceholder -Entry $existingEntries[$id])) {
        $preserved = $existingEntries[$id]
        $preserved.family = $feature.family
        $entries += $preserved
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

$familyOrder = @('metadata', 'forms', 'dcs', 'mxl', 'common', 'other')
$familyAggregates = @(
    foreach ($family in $familyOrder) {
        $familyEntries = @($entries | Where-Object { $_.family -eq $family })
        [ordered]@{
            family = $family
            entries = $familyEntries.Count
            typed = @($familyEntries | Where-Object { $_.status -eq 'typed' }).Count
            opaqueLossless = @($familyEntries | Where-Object { $_.status -eq 'opaque-lossless' }).Count
            unsupported = @($familyEntries | Where-Object { $_.status -eq 'unsupported' }).Count
            platformOnly = @($familyEntries | Where-Object { $_.status -eq 'platform-only' }).Count
        }
    }
)

$familyOrdinals = [System.Collections.Generic.Dictionary[string, int]]::new(
    [System.StringComparer]::Ordinal
)
for ($index = 0; $index -lt $familyOrder.Count; $index++) {
    $familyOrdinals[$familyOrder[$index]] = $index
}
$classifierKindOrdinals = [System.Collections.Generic.Dictionary[string, int]]::new(
    [System.StringComparer]::Ordinal
)
$classifierKindOrdinals.Add('class', 0)
$classifierKindOrdinals.Add('interface', 1)
$classifierKindOrdinals.Add('enum', 2)
$classifierKindOrdinals.Add('datatype', 3)
$featureKindOrdinals = [System.Collections.Generic.Dictionary[string, int]]::new(
    [System.StringComparer]::Ordinal
)
$featureKindOrdinals.Add('attribute', 0)
$featureKindOrdinals.Add('reference', 1)
$featureKindOrdinals.Add('containment', 2)
$backlogGroups = [System.Collections.Generic.Dictionary[string, object]]::new(
    [System.StringComparer]::Ordinal
)
foreach ($entry in $entries) {
    if ($entry.status -ne 'unsupported' -or $entry.diagnosticCode -ne 'schema.unmapped') {
        continue
    }
    $id = Get-KeyId -NamespaceUri $entry.key.namespaceUri -Classifier $entry.key.classifier -Feature $entry.key.feature
    $feature = $features[$id]
    $groupId = "$($familyOrdinals[$feature.family])$([char]0x1f)$($feature.package)$([char]0x1f)$($classifierKindOrdinals[$feature.classifierKind])$([char]0x1f)$($featureKindOrdinals[$feature.featureKind])"
    if (-not $backlogGroups.ContainsKey($groupId)) {
        $backlogGroups[$groupId] = [ordered]@{
            familyOrdinal = $familyOrdinals[$feature.family]
            classifierKindOrdinal = $classifierKindOrdinals[$feature.classifierKind]
            featureKindOrdinal = $featureKindOrdinals[$feature.featureKind]
            rule = 'unsupported/schema.unmapped'
            family = $feature.family
            package = $feature.package
            classifierKind = $feature.classifierKind
            featureKind = $feature.featureKind
            features = 0
        }
    }
    $backlogGroups[$groupId].features++
}
$orderedBacklogGroupIds = [string[]]@($backlogGroups.Keys)
[System.Array]::Sort($orderedBacklogGroupIds, [System.StringComparer]::Ordinal)
$migrationBacklog = @(
    foreach ($groupId in $orderedBacklogGroupIds) {
        $group = $backlogGroups[$groupId]
        [ordered]@{
            rule = $group.rule
            family = $group.family
            package = $group.package
            classifierKind = $group.classifierKind
            featureKind = $group.featureKind
            features = $group.features
        }
    }
)

$corpus = [ordered]@{
    schemaVersion = 1
    source = [ordered]@{
        product = Get-RequiredText -Object $inputSource -Name 'product' -Context 'feature semantics source'
        release = Get-RequiredText -Object $inputSource -Name 'release' -Context 'feature semantics source'
        derivation = 'deterministic canonical implementation coverage bootstrap; placeholders are not EDT semantics'
    }
    summary = $summary
    familyAggregates = $familyAggregates
    migrationBacklog = $migrationBacklog
    entries = $entries
}

$json = ($corpus | ConvertTo-Json -Depth 16).Replace("`r`n", "`n") + "`n"
$indentLevel = 0
$json = (($json.Split("`n") | ForEach-Object {
    $trimmed = $_.TrimStart()
    if ($trimmed.StartsWith('}') -or $trimmed.StartsWith(']')) {
        $indentLevel--
    }
    $line = (' ' * (2 * $indentLevel)) + $trimmed.Replace('":  ', '": ')
    if ($trimmed.EndsWith('{') -or $trimmed.EndsWith('[')) {
        $indentLevel++
    }
    $line
}) -join "`n")
if ($json -match '(?i)(?:^|[^A-Za-z0-9_])[A-Z]:[\\/]|\\\\[^\\]|file:') {
    throw 'Refusing to write non-portable absolute path to canonical coverage.'
}

$outputPath = [System.IO.Path]::GetFullPath($OutputCoverage)
$outputParent = [System.IO.Path]::GetDirectoryName($outputPath)
[System.IO.Directory]::CreateDirectory($outputParent) | Out-Null
[System.IO.File]::WriteAllText($outputPath, $json, [System.Text.UTF8Encoding]::new($false))
Write-Host "Generated $($entries.Count) canonical coverage entries at $outputPath"
