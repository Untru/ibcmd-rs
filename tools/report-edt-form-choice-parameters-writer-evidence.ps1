<#
.SYNOPSIS
Extracts fail-closed production evidence for InputFieldExtInfo.choiceParameters.

.DESCRIPTION
Validates one exact EDT release with javap and cross-validates the result against
the committed raw slot-27/native Form.xml slice. No JAR, bytecode, or machine
path is retained in the portable report.
#>
[CmdletBinding(DefaultParameterSetName = 'Extract')]
param(
    [Parameter(Mandatory, ParameterSetName = 'Extract')] [string]$EdtRoot,
    [Parameter(ParameterSetName = 'Extract')] [string]$EdtRelease = '2025.2.3+30',
    [string]$LiveFixture,
    [Parameter(Mandatory, ParameterSetName = 'Extract')] [string]$RawSource,
    [Parameter(Mandatory, ParameterSetName = 'Extract')] [string]$NativeSource,
    [Parameter(ParameterSetName = 'Extract')] [string]$OutputReport,
    [Parameter(Mandatory, ParameterSetName = 'SelfTest')] [switch]$SelfTest,
    [Parameter(ParameterSetName = 'Extract')] [switch]$VerifyDeterminism
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($LiveFixture)) {
    $scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
    $LiveFixture = Join-Path $scriptDirectory '..\tests\fixtures\form_choice_parameters_slot27_live.json'
}

$script:ExpectedNativeSource = '"DataProcessors/\u0423\u043f\u0440\u0430\u0432\u043b\u0435\u043d\u0438\u0435\u041f\u0440\u043e\u0434\u0430\u0436\u0430\u043c\u0438\u041d\u0430Ozon/Forms/\u041d\u0430\u0441\u0442\u0440\u043e\u0439\u043a\u0438\u0418\u043d\u0442\u0435\u0433\u0440\u0430\u0446\u0438\u0438/Ext/Form.xml"' | ConvertFrom-Json
$script:ExpectedItemStatus = '"\u041e\u0442\u0431\u043e\u0440.\u0421\u0442\u0430\u0442\u0443\u0441"' | ConvertFrom-Json
$script:ExpectedItemOperation = '"\u041e\u0442\u0431\u043e\u0440.\u0425\u043e\u0437\u044f\u0439\u0441\u0442\u0432\u0435\u043d\u043d\u0430\u044f\u041e\u043f\u0435\u0440\u0430\u0446\u0438\u044f"' | ConvertFrom-Json
$script:ExpectedItemDeletionMark = '"\u041e\u0442\u0431\u043e\u0440.\u041f\u043e\u043c\u0435\u0442\u043a\u0430\u0423\u0434\u0430\u043b\u0435\u043d\u0438\u044f"' | ConvertFrom-Json
$script:ExpectedValueSale = '"\u0420\u0435\u0430\u043b\u0438\u0437\u0430\u0446\u0438\u044f\u041a\u043b\u0438\u0435\u043d\u0442\u0443"' | ConvertFrom-Json
$script:ExpectedValueCommission = '"\u041f\u0435\u0440\u0435\u0434\u0430\u0447\u0430\u041d\u0430\u041a\u043e\u043c\u0438\u0441\u0441\u0438\u044e"' | ConvertFrom-Json

function Assert-Equal {
    param([object]$Actual, [object]$Expected, [string]$Label)
    if ($Actual -cne $Expected) { throw "$Label differs: '$Actual' != '$Expected'." }
}

function Assert-OrderedText {
    param([string]$Text, [string[]]$Patterns, [string]$Label)
    $offset = 0
    foreach ($pattern in $Patterns) {
        $match = [regex]::Match($Text.Substring($offset), $pattern)
        if (-not $match.Success) { throw "$Label is missing ordered pattern '$pattern'." }
        $offset += $match.Index + $match.Length
    }
}

function Get-ExactBundle {
    param([string]$Plugins, [string]$SymbolicName, [string]$Version)
    $matches = @(Get-ChildItem -LiteralPath $Plugins -File -Filter "${SymbolicName}_${Version}.jar")
    if ($matches.Count -ne 1) { throw "Expected exactly one ${SymbolicName}_${Version}.jar." }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($matches[0].FullName)
    try {
        $entry = $archive.GetEntry('META-INF/MANIFEST.MF')
        if ($null -eq $entry) { throw "$SymbolicName has no bundle manifest." }
        $reader = [System.IO.StreamReader]::new($entry.Open())
        try { $manifest = $reader.ReadToEnd() } finally { $reader.Dispose() }
    } finally { $archive.Dispose() }
    $manifest = $manifest -replace "`r?`n ", ''
    $manifestSymbolicName = [regex]::Match($manifest, '(?m)^Bundle-SymbolicName:\s*([^;\r\n]+)').Groups[1].Value
    $manifestVersion = [regex]::Match($manifest, '(?m)^Bundle-Version:\s*([^\r\n]+)').Groups[1].Value
    Assert-Equal $manifestSymbolicName $SymbolicName "$SymbolicName manifest symbolic name"
    Assert-Equal $manifestVersion $Version "$SymbolicName manifest version"
    return $matches[0].FullName
}

function Get-ExactConfigValue {
    param([string]$Text, [string]$Key)
    $matches = @($Text -split "`r?`n" | Where-Object { $_ -match ('^' + [regex]::Escape($Key) + '=') })
    if ($matches.Count -ne 1) { throw "Expected exactly one '$Key' identity entry." }
    return $matches[0].Substring($Key.Length + 1)
}

function Invoke-ExactJavap {
    param([string]$Jar, [string]$Class)
    $output = @(& javap -classpath $Jar -v -p -c -constants $Class 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "javap failed for ${Class}: $($output -join "`n")" }
    return ($output -join "`n")
}

function Get-TextSha256 {
    param([string]$Text)
    $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($Text)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = $algorithm.ComputeHash($bytes)
    } finally {
        $algorithm.Dispose()
    }
    return (($hash | ForEach-Object { $_.ToString('x2') }) -join '')
}

function Get-FileSha256 {
    param([string]$Path)
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

function Get-Utf8FileText {
    param([string]$Path)
    $resolved = (Resolve-Path -LiteralPath $Path).Path
    return [IO.File]::ReadAllText($resolved, [Text.UTF8Encoding]::new($false))
}

function ConvertTo-CanonicalJson {
    param([object]$Value)
    return ($Value | ConvertTo-Json -Depth 20 -Compress)
}

function Get-XmlSignature {
    param([System.Xml.XmlElement]$Element)
    $attributes = @($Element.Attributes | Where-Object {
        $_.NamespaceURI -ne 'http://www.w3.org/2000/xmlns/'
    } | ForEach-Object {
        "{{{0}}}{1}={2}" -f $_.NamespaceURI, $_.LocalName, $_.Value
    } | Sort-Object)
    $children = [System.Collections.Generic.List[string]]::new()
    foreach ($child in $Element.ChildNodes) {
        if ($child -is [System.Xml.XmlElement]) {
            $children.Add((Get-XmlSignature $child))
        } elseif ($child -is [System.Xml.XmlText] -and -not [string]::IsNullOrWhiteSpace($child.Value)) {
            $children.Add("text=$($child.Value)")
        }
    }
    return "{{$($Element.NamespaceURI)}}$($Element.LocalName)[$($attributes -join '|')]($($children -join ','))"
}

function Assert-LiveFixtureObject {
    param([object]$Fixture)
    Assert-Equal $Fixture.source.rawRow '34accda9-6211-4bc3-be8d-e42a24260653.0' 'raw row'
    Assert-Equal $Fixture.source.rawSource 'candidate_dump/Config_inflated/34accda9-6211-4bc3-be8d-e42a24260653.0__part0.txt' 'raw source'
    Assert-Equal $Fixture.source.rawSourceSha256 '77a99cffaa0b5c81ccccafa3a5fa01dec56342b49d1cce2e56f97f28b62785b1' 'raw source SHA-256'
    Assert-Equal $Fixture.source.rawSlot 27 'raw slot'
    Assert-Equal $Fixture.source.nativeSource $script:ExpectedNativeSource 'native source'
    Assert-Equal $Fixture.source.nativeSourceSha256 '30cf0689522d6b74408da77426a178df282361f36d3787c0cfaf456c85cb8b03' 'native source SHA-256'
    Assert-Equal (Get-TextSha256 $Fixture.rawSlotValue) 'c72e9cce9f56cb12a59e0a09f563e840096d75e5c71bff790817f9f3fdbd5dd8' 'raw slot bytes'
    Assert-Equal (Get-TextSha256 $Fixture.expectedXml) 'a46f28788fb4994c398d775f045bf2e6ea58ccb1b97543562123234a03d9bac4' 'expected XML bytes'

    $wrapped = '<root xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">' + $Fixture.expectedXml + '</root>'
    [xml]$xml = $wrapped
    $choice = [System.Xml.XmlElement]$xml.DocumentElement.FirstChild
    Assert-Equal $choice.NamespaceURI 'http://v8.1c.ru/8.3/xcf/logform' 'ChoiceParameters namespace'
    Assert-Equal $choice.LocalName 'ChoiceParameters' 'ChoiceParameters QName'
    $items = @($choice.ChildNodes)
    Assert-Equal $items.Count 3 'choice parameter item count'
    Assert-Equal (($items | ForEach-Object { $_.GetAttribute('name') }) -join '|') (($script:ExpectedItemStatus, $script:ExpectedItemOperation, $script:ExpectedItemDeletionMark) -join '|') 'choice parameter item order'
    foreach ($item in $items) {
        Assert-Equal $item.NamespaceURI 'http://v8.1c.ru/8.2/managed-application/core' 'app:item namespace'
        Assert-Equal $item.LocalName 'item' 'app:item QName'
        Assert-Equal $item.GetAttributeNode('name').NamespaceURI '' 'name attribute namespace'
        $value = [System.Xml.XmlElement]$item.FirstChild
        Assert-Equal $value.NamespaceURI 'http://v8.1c.ru/8.2/managed-application/core' 'app:value namespace'
        Assert-Equal $value.LocalName 'value' 'app:value QName'
        Assert-Equal $value.GetAttribute('type', 'http://www.w3.org/2001/XMLSchema-instance') 'FormChoiceListDesTimeValue' 'outer value xsi:type'
        Assert-Equal $value.ChildNodes[0].LocalName 'Presentation' 'Presentation-before-Value order'
        Assert-Equal $value.ChildNodes[0].NamespaceURI 'http://v8.1c.ru/8.3/xcf/logform' 'Presentation namespace'
        Assert-Equal $value.ChildNodes[1].LocalName 'Value' 'scalar Value order'
        Assert-Equal $value.ChildNodes[1].NamespaceURI 'http://v8.1c.ru/8.3/xcf/logform' 'scalar Value namespace'
    }
    $fixed = [System.Xml.XmlElement]$items[1].FirstChild.ChildNodes[1]
    Assert-Equal $fixed.GetAttribute('type', 'http://www.w3.org/2001/XMLSchema-instance') 'v8:FixedArray' 'FixedArray xsi:type'
    Assert-Equal $fixed.ChildNodes.Count 2 'FixedArray member count'
    foreach ($member in $fixed.ChildNodes) {
        Assert-Equal $member.NamespaceURI 'http://v8.1c.ru/8.1/data/core' 'v8:Value namespace'
        Assert-Equal $member.LocalName 'Value' 'v8:Value QName'
        Assert-Equal $member.GetAttribute('type', 'http://www.w3.org/2001/XMLSchema-instance') 'FormChoiceListDesTimeValue' 'FixedArray member xsi:type'
        Assert-Equal (($member.ChildNodes | ForEach-Object LocalName) -join '|') 'Presentation|Value' 'FixedArray member order'
    }
    Assert-Equal $items[0].FirstChild.ChildNodes[1].GetAttribute('type', 'http://www.w3.org/2001/XMLSchema-instance') 'xr:DesignTimeRef' 'design-time-ref xsi:type'
    Assert-Equal $items[2].FirstChild.ChildNodes[1].GetAttribute('type', 'http://www.w3.org/2001/XMLSchema-instance') 'xs:boolean' 'boolean xsi:type'
    return $choice
}

function Test-LiveFixture {
    param([string]$Path)
    Assert-Equal (Get-FileSha256 $Path) '05e4ef14ae7e3de0b2cc7d1b46e042be6ec70df629c57355036c5c7e58148bf7' 'fixture SHA-256'
    $fixture = Get-Utf8FileText $Path | ConvertFrom-Json
    [void](Assert-LiveFixtureObject $fixture)
    return $fixture
}

function Test-LiveSources {
    param([object]$Fixture, [string]$RawPath, [string]$NativePath)
    Assert-Equal (Get-FileSha256 $RawPath) $Fixture.source.rawSourceSha256 'raw source file SHA-256'
    Assert-Equal (Get-FileSha256 $NativePath) $Fixture.source.nativeSourceSha256 'native source file SHA-256'
    $raw = (Get-Utf8FileText $RawPath).Replace("`r`n", "`n")
    if (-not $raw.Contains($Fixture.rawSlotValue)) {
        throw 'The exact committed slot-27 value is absent from the hash-bound raw source.'
    }
    [xml]$native = Get-Utf8FileText $NativePath
    $root = $native.DocumentElement
    foreach ($mapping in ([ordered]@{
        '' = 'http://v8.1c.ru/8.3/xcf/logform'
        app = 'http://v8.1c.ru/8.2/managed-application/core'
        v8 = 'http://v8.1c.ru/8.1/data/core'
        xr = 'http://v8.1c.ru/8.3/xcf/readable'
        xs = 'http://www.w3.org/2001/XMLSchema'
        xsi = 'http://www.w3.org/2001/XMLSchema-instance'
    }).GetEnumerator()) {
        $actual = if ($mapping.Key -eq '') { $root.GetAttribute('xmlns') } else { $root.GetAttribute("xmlns:$($mapping.Key)") }
        Assert-Equal $actual $mapping.Value "native namespace prefix '$($mapping.Key)'"
    }
    $expectedChoice = Assert-LiveFixtureObject $Fixture
    $expectedSignature = Get-XmlSignature $expectedChoice
    $matches = @($native.SelectNodes('//*[local-name()="ChoiceParameters"]') | Where-Object {
        (Get-XmlSignature $_) -eq $expectedSignature
    })
    Assert-Equal $matches.Count 1 'exact native ChoiceParameters structural match'
}

function New-Evidence {
    $root = (Resolve-Path -LiteralPath $EdtRoot).Path
    Assert-Equal (Split-Path $root -Leaf) '1c-edt-2025.2.3+30-x86_64' 'EDT root'
    Assert-Equal $EdtRelease '2025.2.3+30' 'EDT release'
    $config = Get-Utf8FileText (Join-Path $root 'configuration\config.ini')
    Assert-Equal (Get-ExactConfigValue $config 'product.version') '2025.2.3' 'product version'
    Assert-Equal (Get-ExactConfigValue $config 'eclipse.buildId') '2025.2.3.30' 'build ID'
    Assert-Equal (Get-ExactConfigValue $config 'eclipse.product') 'com._1c.g5.v8.dt.product.application.rcp' 'product ID'
    Assert-Equal (Get-ExactConfigValue $config 'eclipse.application') 'org.eclipse.ui.ide.workbench' 'application ID'
    $plugins = Join-Path $root 'plugins'
    $core = Get-ExactBundle $plugins 'com._1c.g5.v8.dt.export.xml' '13.0.100.v202602241426'
    $formExport = Get-ExactBundle $plugins 'com._1c.g5.v8.dt.form.export.xml' '10.1.0.v202602241426'
    $formModel = Get-ExactBundle $plugins 'com._1c.g5.v8.dt.form.model' '14.0.0.v202602241426'
    [void](Get-ExactBundle $plugins 'com._1c.g5.v8.dt.mcore' '8.6.0.v202602241426')

    $order = Invoke-ExactJavap $formExport 'com._1c.g5.v8.dt.form.export.xml.writer.ExtInfoWriter$ExtInfoFeatureOrderProvider'
    Assert-OrderedText $order @(
        '431:\s+getstatic.+INPUT_FIELD_EXT_INFO__CHOICE_PARAMETER_LINKS',
        '439:\s+getstatic.+INPUT_FIELD_EXT_INFO__CHOICE_PARAMETERS',
        '447:\s+getstatic.+INPUT_FIELD_EXT_INFO__AVAILABLE_TYPES'
    ) 'InputFieldExtInfo feature order'
    $extInfo = Invoke-ExactJavap $formExport 'com._1c.g5.v8.dt.form.export.xml.writer.ExtInfoWriter'
    Assert-OrderedText $extInfo @(
        'protected void writeExtInfoFeatures',
        '65:\s+aload\s+8',
        '67:\s+instanceof.+EReference',
        '70:\s+ifne\s+77',
        '73:\s+iconst_1',
        '77:\s+iconst_0',
        '80:\s+invokevirtual.+FormSmartFeatureWriter\.write'
    ) 'EReference writeDefault=false call site'

    $model = Invoke-ExactJavap $formModel 'com._1c.g5.v8.dt.form.model.impl.FormPackageImpl'
    Assert-OrderedText $model @(
        '19690:\s+invokevirtual.+getInputFieldExtInfo_ChoiceParameters',
        '19695:\s+invokeinterface.+CommonPackage\.getChoiceParameter',
        '19701:\s+ldc_w.+String choiceParameters',
        '19705:\s+iconst_0',
        '19706:\s+iconst_m1'
    ) 'choiceParameters model initialization'

    $writer = Invoke-ExactJavap $core 'com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter'
    Assert-OrderedText $writer @(
        'getElementQName', 'writeStartElement', 'java/util/List\.iterator',
        'IXmlElements\$APP\.ITEM', 'String name', 'Strings\.nullToEmpty',
        'writeAttribute', 'CHOICE_PARAMETER__VALUE', 'iconst_1',
        'ISpecifiedElementWriter\.write', 'writeEndElement'
    ) 'ChoiceParameterWriter'
    Assert-OrderedText $writer @(
        '149:\s+invokevirtual.+writeEmptyElement',
        '152:\s+goto\s+167',
        '155:\s+aload_0',
        '167:\s+return'
    ) 'ChoiceParameterWriter empty-collection branch'

    $smart = Invoke-ExactJavap $formExport 'com._1c.g5.v8.dt.form.export.xml.writer.FormSmartFeatureWriter'
    Assert-OrderedText $smart @('CommonPackage\$Literals\.CHOICE_PARAMETER', 'ChoiceParameterWriter') 'writer delegation'
    $names = Invoke-ExactJavap $formExport 'com._1c.g5.v8.dt.internal.form.export.xml.FormFeatureNameProvider'
    Assert-OrderedText $names @('CHOICE_PARAMETER__VALUE', 'IXmlElements\$APP\.VALUE') 'choice parameter value QName'
    Assert-OrderedText $names @('FIXED_ARRAY_VALUE__VALUES', 'IFormXmlElements\$V8\.VALUE') 'fixed-array member QName'
    $fixture = Test-LiveFixture $LiveFixture
    Test-LiveSources $fixture $RawSource $NativeSource

    return [ordered]@{
        schemaVersion = 1
        source = [ordered]@{
            product = '1C:EDT'; release = '2025.2.3+30'
            rootIdentity = [ordered]@{ leaf = '1c-edt-2025.2.3+30-x86_64'; productVersion = '2025.2.3'; buildId = '2025.2.3.30' }
            validatedBundles = @(
                [ordered]@{ symbolicName = 'com._1c.g5.v8.dt.export.xml'; version = '13.0.100.v202602241426' },
                [ordered]@{ symbolicName = 'com._1c.g5.v8.dt.form.export.xml'; version = '10.1.0.v202602241426' },
                [ordered]@{ symbolicName = 'com._1c.g5.v8.dt.form.model'; version = '14.0.0.v202602241426' },
                [ordered]@{ symbolicName = 'com._1c.g5.v8.dt.mcore'; version = '8.6.0.v202602241426' }
            )
            derivation = 'Exact javap -v -p -c -constants assertions over the named release bundles, cross-validated against one current raw slot-27/native Form.xml pair; no EDT code or machine path retained.'
            invocation = 'tools/report-edt-form-choice-parameters-writer-evidence.ps1'
        }
        scope = [ordered]@{ disposition = 'production-emission-evidence'; productionEmission = $true }
        verifiedFacts = [ordered]@{
            model = [ordered]@{ modelType = 'InputFieldExtInfo'; feature = 'choiceParameters'; lowerBound = 0; upperBound = -1; ownerQName = '{http://v8.1c.ru/8.3/xcf/logform}ChoiceParameters' }
            ownerOrder = [ordered]@{ predecessorQName = '{http://v8.1c.ru/8.3/xcf/logform}ChoiceParameterLinks'; featureQName = '{http://v8.1c.ru/8.3/xcf/logform}ChoiceParameters'; successorQName = '{http://v8.1c.ru/8.3/xcf/logform}AvailableTypes' }
            writer = [ordered]@{
                delegate = 'com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter'; emptyCollection = 'omit-when-write-default-false'
                item = [ordered]@{ itemQName = '{http://v8.1c.ru/8.2/managed-application/core}item'; nameAttributeQName = '{}name'; valueQName = '{http://v8.1c.ru/8.2/managed-application/core}value'; valueXsiType = 'FormChoiceListDesTimeValue'; valueOrder = @('presentation','value'); presentationQName = '{http://v8.1c.ru/8.3/xcf/logform}Presentation'; scalarValueQName = '{http://v8.1c.ru/8.3/xcf/logform}Value'; booleanXsiType = 'xs:boolean'; designTimeRefXsiType = 'xr:DesignTimeRef' }
                fixedArray = [ordered]@{ xsiType = 'v8:FixedArray'; itemQName = '{http://v8.1c.ru/8.1/data/core}Value'; itemXsiType = 'FormChoiceListDesTimeValue'; itemOrder = @('presentation','value') }
            }
            liveSlot27 = [ordered]@{ fixture = 'tests/fixtures/form_choice_parameters_slot27_live.json'; fixtureSha256 = '05e4ef14ae7e3de0b2cc7d1b46e042be6ec70df629c57355036c5c7e58148bf7'; rawRow = '34accda9-6211-4bc3-be8d-e42a24260653.0'; rawSource = 'candidate_dump/Config_inflated/34accda9-6211-4bc3-be8d-e42a24260653.0__part0.txt'; rawSourceSha256 = '77a99cffaa0b5c81ccccafa3a5fa01dec56342b49d1cce2e56f97f28b62785b1'; rawSlot = 27; nativeSource = $script:ExpectedNativeSource; nativeSourceSha256 = '30cf0689522d6b74408da77426a178df282361f36d3787c0cfaf456c85cb8b03'; itemNamesInOrder = @($script:ExpectedItemStatus, $script:ExpectedItemOperation, $script:ExpectedItemDeletionMark); valueKindsInOrder = @('U','FixedArray','B') }
        }
        missingKeys = @()
    }
}

if ($SelfTest) {
    $fixture = Test-LiveFixture $LiveFixture
    $mutations = [ordered]@{
        'raw-row' = { param($f) $f.source.rawRow = 'wrong' }
        'raw-source' = { param($f) $f.source.rawSource = 'wrong' }
        'raw-source-hash' = { param($f) $f.source.rawSourceSha256 = ('0' * 64) }
        'raw-slot' = { param($f) $f.source.rawSlot = 26 }
        'native-source' = { param($f) $f.source.nativeSource = 'wrong' }
        'native-source-hash' = { param($f) $f.source.nativeSourceSha256 = ('0' * 64) }
        'raw-U' = { param($f) $f.rawSlotValue = $f.rawSlotValue.Replace('{"U"}', '{"X"}') }
        'raw-FixedArray' = { param($f) $f.rawSlotValue = $f.rawSlotValue.Replace('4500381b-db30-4a10-9db4-990038032acf', '00000000-0000-0000-0000-000000000001') }
        'raw-B' = { param($f) $f.rawSlotValue = $f.rawSlotValue.Replace('{"B",0}', '{"B",1}') }
        'owner-qname' = { param($f) $f.expectedXml = $f.expectedXml.Replace('<ChoiceParameters>', '<Wrong>') }
        'app-item-qname' = { param($f) $f.expectedXml = $f.expectedXml.Replace('<app:item ', '<v8:item ') }
        'name-attribute' = { param($f) $f.expectedXml = $f.expectedXml.Replace(' name=', ' app:name=') }
        'app-value-qname' = { param($f) $f.expectedXml = $f.expectedXml.Replace('<app:value ', '<v8:value ') }
        'outer-xsi-type' = { param($f) $f.expectedXml = $f.expectedXml.Replace('xsi:type="FormChoiceListDesTimeValue"', 'xsi:type="Wrong"') }
        'presentation-qname' = { param($f) $f.expectedXml = $f.expectedXml.Replace('<Presentation/>', '<Wrong/>') }
        'presentation-value-order' = { param($f) $f.expectedXml = $f.expectedXml.Replace("<Presentation/>`r`n`t`t`t`t<Value", "<Value") }
        'scalar-value-qname' = { param($f) $f.expectedXml = $f.expectedXml.Replace('<Value xsi:type=', '<Wrong xsi:type=') }
        'boolean-xsi-type' = { param($f) $f.expectedXml = $f.expectedXml.Replace('xs:boolean', 'xs:string') }
        'design-ref-xsi-type' = { param($f) $f.expectedXml = $f.expectedXml.Replace('xr:DesignTimeRef', 'xs:string') }
        'fixed-array-xsi-type' = { param($f) $f.expectedXml = $f.expectedXml.Replace('v8:FixedArray', 'v8:Array') }
        'fixed-member-qname' = { param($f) $f.expectedXml = $f.expectedXml.Replace('<v8:Value ', '<app:value ') }
        'fixed-member-xsi-type' = { param($f) $f.expectedXml = $f.expectedXml.Replace('<v8:Value xsi:type="FormChoiceListDesTimeValue">', '<v8:Value xsi:type="Wrong">') }
        'fixed-member-order' = { param($f) $f.expectedXml = $f.expectedXml.Replace($script:ExpectedValueSale, $script:ExpectedValueCommission).Replace("$($script:ExpectedValueCommission)</Value>", 'Duplicate</Value>') }
        'item-order' = { param($f) $f.expectedXml = $f.expectedXml.Replace($script:ExpectedItemStatus, $script:ExpectedItemDeletionMark) }
    }
    foreach ($entry in $mutations.GetEnumerator()) {
        $mutated = $fixture | ConvertTo-Json -Depth 20 | ConvertFrom-Json
        & $entry.Value $mutated
        $rejected = $false
        try { [void](Assert-LiveFixtureObject $mutated) } catch { $rejected = $true }
        if (-not $rejected) { throw "Mutation '$($entry.Key)' did not fail closed." }
    }
    $canonicalProbe = [ordered]@{
        alpha = 1
        nested = [ordered]@{ beta = @($true, 'x') }
        empty = @()
    }
    Assert-Equal (ConvertTo-CanonicalJson $canonicalProbe) '{"alpha":1,"nested":{"beta":[true,"x"]},"empty":[]}' 'canonical JSON'
    Write-Output "SelfTest passed: fixture hash, canonical JSON, plus $($mutations.Count) independent fact mutations."
    return
}

$report = New-Evidence
$json = ConvertTo-CanonicalJson $report
if ($VerifyDeterminism) {
    $again = ConvertTo-CanonicalJson (New-Evidence)
    Assert-Equal $again $json 'deterministic report'
}
if ($OutputReport) {
    $outputPath = if ([System.IO.Path]::IsPathRooted($OutputReport)) {
        $OutputReport
    } else {
        Join-Path (Get-Location) $OutputReport
    }
    [System.IO.File]::WriteAllText($outputPath, $json + "`n", [System.Text.UTF8Encoding]::new($false))
} else {
    $json
}
