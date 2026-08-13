param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath,

    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
)

$ErrorActionPreference = 'Stop'
$fixtureRelativePath = 'tests/fixtures/native-evidence/8.3.27.2214/task-basic'
$fixtureRoot = Join-Path $RepositoryRoot $fixtureRelativePath
$manifestPath = Join-Path $fixtureRoot 'manifest.json'

function Get-Sha256Hex {
    param([byte[]]$Bytes)

    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        return -join ($algorithm.ComputeHash($Bytes) | ForEach-Object { $_.ToString('x2') })
    } finally {
        $algorithm.Dispose()
    }
}

function Get-FileSha256Hex {
    param([string]$Path)

    return Get-Sha256Hex ([IO.File]::ReadAllBytes($Path))
}

function Get-ByteSlice {
    param(
        [byte[]]$Bytes,
        [int]$Offset,
        [int]$Length
    )

    $slice = New-Object byte[] $Length
    [Array]::Copy($Bytes, $Offset, $slice, 0, $Length)
    return ,$slice
}

function Resolve-FixturePath {
    param(
        [string]$RelativePath,
        [string]$Root = $fixtureRoot
    )

    $platformPath = $RelativePath.Replace('/', [IO.Path]::DirectorySeparatorChar)
    return Join-Path $Root $platformPath
}

function Assert-Equal {
    param(
        $Actual,
        $Expected,
        [string]$Label
    )

    if ($Actual -ne $Expected) {
        throw "$Label mismatch: expected '$Expected', got '$Actual'."
    }
}

function Assert-FileEvidence {
    param(
        $Evidence,
        [string]$Root = $fixtureRoot
    )

    $path = Resolve-FixturePath $Evidence.path $Root
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Evidence file is missing: $($Evidence.path)"
    }
    $item = Get-Item -LiteralPath $path
    Assert-Equal $item.Length ([long]$Evidence.size) "$($Evidence.path) size"
    Assert-Equal (Get-FileSha256Hex $path) $Evidence.sha256 "$($Evidence.path) SHA-256"
}

function Get-Base64EvidenceBytes {
    param(
        $Evidence,
        [string]$Root
    )

    $path = Resolve-FixturePath $Evidence.path $Root
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Base64 evidence file is missing: $($Evidence.path)"
    }
    $item = Get-Item -LiteralPath $path
    if ($null -ne $Evidence.encoded_size) {
        Assert-Equal $item.Length ([long]$Evidence.encoded_size) "$($Evidence.path) encoded size"
    }
    if ($null -ne $Evidence.encoded_sha256) {
        Assert-Equal (Get-FileSha256Hex $path) $Evidence.encoded_sha256 "$($Evidence.path) encoded SHA-256"
    }
    $encoded = Get-Content -LiteralPath $path -Raw -Encoding ASCII
    $decoded = [Convert]::FromBase64String(($encoded -replace '\s', ''))
    $expectedSize = if ($null -ne $Evidence.decoded_size) { $Evidence.decoded_size } else { $Evidence.size }
    Assert-Equal $decoded.Length ([long]$expectedSize) "$($Evidence.path) decoded size"
    $expectedSha = if ($null -ne $Evidence.decoded_sha256) { $Evidence.decoded_sha256 } else { $Evidence.sha256 }
    Assert-Equal (Get-Sha256Hex $decoded) $expectedSha "$($Evidence.path) decoded SHA-256"
    return ,$decoded
}

function Get-Utf8XmlFragmentBytes {
    param(
        [string]$Path,
        [string]$OpeningTag,
        [string]$ClosingTag
    )

    $text = [Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($Path))
    $start = $text.IndexOf($OpeningTag, [StringComparison]::Ordinal)
    if ($start -lt 0) {
        throw "XML fragment opening tag '$OpeningTag' is missing from: $Path"
    }
    $end = $text.IndexOf($ClosingTag, $start, [StringComparison]::Ordinal)
    if ($end -lt 0) {
        throw "XML fragment closing tag '$ClosingTag' is missing from: $Path"
    }
    $fragment = $text.Substring($start, $end - $start + $ClosingTag.Length)
    return ,[Text.Encoding]::UTF8.GetBytes($fragment)
}

function Get-XmlDocumentFromBytes {
    param([byte[]]$Bytes)

    $document = New-Object Xml.XmlDocument
    $document.PreserveWhitespace = $true
    $stream = New-Object IO.MemoryStream(,($Bytes))
    try {
        $document.Load($stream)
    } finally {
        $stream.Dispose()
    }
    return ,$document
}

function Assert-RawNativeEvidenceFixture {
    param(
        [string]$RelativePath,
        [int]$Issue,
        [string]$Kind,
        [string]$Uuid
    )

    $root = Join-Path $RepositoryRoot $RelativePath
    $fixtureManifestPath = Join-Path $root 'manifest.json'
    if (-not (Test-Path -LiteralPath $fixtureManifestPath -PathType Leaf)) {
        throw "$Kind evidence manifest is missing: $fixtureManifestPath"
    }
    $fixtureManifest = Get-Content -LiteralPath $fixtureManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Equal $fixtureManifest.schema_version 1 "$Kind manifest schema version"
    Assert-Equal $fixtureManifest.issue $Issue "$Kind issue"
    Assert-Equal $fixtureManifest.evidence.platform_version '8.3.27.2214' "$Kind platform version"
    Assert-Equal $fixtureManifest.evidence.source_version '2.20' "$Kind source version"
    Assert-Equal $fixtureManifest.object.kind $Kind "$Kind object kind"
    Assert-Equal $fixtureManifest.object.uuid $Uuid "$Kind UUID"
    Assert-FileEvidence $fixtureManifest.object.raw_entry.packed $root
    Assert-FileEvidence $fixtureManifest.object.raw_entry.unpacked $root
    Assert-FileEvidence $fixtureManifest.object.native_xml $root
    if ($null -ne $fixtureManifest.seed) {
        $seedPath = Resolve-FixturePath $fixtureManifest.seed.definition_path $root
        if (-not (Test-Path -LiteralPath $seedPath -PathType Leaf)) {
            throw "$Kind seed definition is missing: $($fixtureManifest.seed.definition_path)"
        }
        Assert-Equal (Get-Item -LiteralPath $seedPath).Length ([long]$fixtureManifest.seed.definition_size) "$Kind seed size"
        Assert-Equal (Get-FileSha256Hex $seedPath) $fixtureManifest.seed.definition_sha256 "$Kind seed SHA-256"
    }

    $packedPath = Resolve-FixturePath $fixtureManifest.object.raw_entry.packed.path $root
    $unpackedPath = Resolve-FixturePath $fixtureManifest.object.raw_entry.unpacked.path $root
    $compressedStream = New-Object IO.MemoryStream(,([IO.File]::ReadAllBytes($packedPath)))
    $decompressedStream = New-Object IO.MemoryStream
    $deflateStream = New-Object IO.Compression.DeflateStream(
        $compressedStream,
        [IO.Compression.CompressionMode]::Decompress
    )
    try {
        $deflateStream.CopyTo($decompressedStream)
    } finally {
        $deflateStream.Dispose()
        $compressedStream.Dispose()
    }
    try {
        $decompressed = $decompressedStream.ToArray()
    } finally {
        $decompressedStream.Dispose()
    }
    Assert-Equal $decompressed.Length ([long]$fixtureManifest.object.raw_entry.unpacked.size) "$Kind decoded size"
    Assert-Equal (Get-Sha256Hex $decompressed) $fixtureManifest.object.raw_entry.unpacked.sha256 "$Kind decoded SHA-256"
    Assert-Equal (Get-Sha256Hex ([IO.File]::ReadAllBytes($unpackedPath))) (Get-Sha256Hex $decompressed) "$Kind packed/unpacked pair"
}

function Assert-RawNativeEvidenceCorpus {
    param(
        [string]$RelativePath,
        [int]$Issue,
        [hashtable]$ExpectedObjects
    )

    $root = Join-Path $RepositoryRoot $RelativePath
    $fixtureManifestPath = Join-Path $root 'manifest.json'
    if (-not (Test-Path -LiteralPath $fixtureManifestPath -PathType Leaf)) {
        throw "Native evidence corpus manifest is missing: $fixtureManifestPath"
    }
    $fixtureManifest = Get-Content -LiteralPath $fixtureManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Equal $fixtureManifest.schema_version 1 'corpus manifest schema version'
    Assert-Equal $fixtureManifest.issue $Issue 'corpus issue'
    Assert-Equal $fixtureManifest.evidence.platform_version '8.3.27.2214' 'corpus platform version'
    Assert-Equal $fixtureManifest.evidence.source_version '2.20' 'corpus source version'

    $seedPath = Resolve-FixturePath $fixtureManifest.seed.definition_path $root
    if (-not (Test-Path -LiteralPath $seedPath -PathType Leaf)) {
        throw "Corpus seed definition is missing: $($fixtureManifest.seed.definition_path)"
    }
    Assert-Equal (Get-Item -LiteralPath $seedPath).Length ([long]$fixtureManifest.seed.definition_size) 'corpus seed size'
    Assert-Equal (Get-FileSha256Hex $seedPath) $fixtureManifest.seed.definition_sha256 'corpus seed SHA-256'

    foreach ($kind in $ExpectedObjects.Keys) {
        $matches = @($fixtureManifest.objects | Where-Object { $_.kind -eq $kind })
        Assert-Equal $matches.Count 1 "$kind corpus object count"
        $object = $matches[0]
        Assert-Equal $object.uuid $ExpectedObjects[$kind] "$kind corpus UUID"
        Assert-FileEvidence $object.raw_entry.packed $root
        Assert-FileEvidence $object.raw_entry.unpacked $root
        Assert-FileEvidence $object.native_xml $root

        $packedPath = Resolve-FixturePath $object.raw_entry.packed.path $root
        $unpackedPath = Resolve-FixturePath $object.raw_entry.unpacked.path $root
        $compressedStream = New-Object IO.MemoryStream(,([IO.File]::ReadAllBytes($packedPath)))
        $decompressedStream = New-Object IO.MemoryStream
        $deflateStream = New-Object IO.Compression.DeflateStream(
            $compressedStream,
            [IO.Compression.CompressionMode]::Decompress
        )
        try {
            $deflateStream.CopyTo($decompressedStream)
        } finally {
            $deflateStream.Dispose()
            $compressedStream.Dispose()
        }
        try {
            $decompressed = $decompressedStream.ToArray()
        } finally {
            $decompressedStream.Dispose()
        }
        Assert-Equal $decompressed.Length ([long]$object.raw_entry.unpacked.size) "$kind corpus decoded size"
        Assert-Equal (Get-Sha256Hex $decompressed) $object.raw_entry.unpacked.sha256 "$kind corpus decoded SHA-256"
        Assert-Equal (Get-Sha256Hex ([IO.File]::ReadAllBytes($unpackedPath))) (Get-Sha256Hex $decompressed) "$kind corpus packed/unpacked pair"
    }
    Assert-Equal (@($fixtureManifest.objects).Count) $ExpectedObjects.Count 'corpus object count'
}

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Native evidence manifest is missing: $manifestPath"
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $manifest.schema_version 1 'manifest schema version'
Assert-Equal $manifest.evidence.platform_version '8.3.27.2214' 'platform version'
Assert-Equal $manifest.evidence.source_version '2.20' 'source version'

$seedPath = Resolve-FixturePath $manifest.seed.definition_path
if (-not (Test-Path -LiteralPath $seedPath -PathType Leaf)) {
    throw "Seed definition is missing: $($manifest.seed.definition_path)"
}
$seedItem = Get-Item -LiteralPath $seedPath
Assert-Equal $seedItem.Length ([long]$manifest.seed.definition_size) 'seed definition size'
Assert-Equal (Get-FileSha256Hex $seedPath) $manifest.seed.definition_sha256 'seed definition SHA-256'

$encodedCfPath = Resolve-FixturePath $manifest.configuration_cf.path
if (-not (Test-Path -LiteralPath $encodedCfPath -PathType Leaf)) {
    throw "Encoded CF is missing: $($manifest.configuration_cf.path)"
}
$encodedCfItem = Get-Item -LiteralPath $encodedCfPath
Assert-Equal $encodedCfItem.Length ([long]$manifest.configuration_cf.encoded_size) 'encoded CF size'
Assert-Equal (Get-FileSha256Hex $encodedCfPath) $manifest.configuration_cf.encoded_sha256 'encoded CF SHA-256'

$encodedCf = Get-Content -LiteralPath $encodedCfPath -Raw -Encoding ASCII
$cfBytes = [Convert]::FromBase64String(($encodedCf -replace '\s', ''))
Assert-Equal $cfBytes.Length ([long]$manifest.configuration_cf.decoded_size) 'decoded CF size'
Assert-Equal (Get-Sha256Hex $cfBytes) $manifest.configuration_cf.decoded_sha256 'decoded CF SHA-256'

foreach ($object in @($manifest.objects)) {
    $rawPath = Resolve-FixturePath $object.raw_entry.path
    if (-not (Test-Path -LiteralPath $rawPath -PathType Leaf)) {
        throw "Raw evidence is missing: $($object.raw_entry.path)"
    }
    $rawItem = Get-Item -LiteralPath $rawPath
    Assert-Equal $rawItem.Length ([long]$object.raw_entry.unpacked_size) "$($object.raw_entry.path) size"
    Assert-Equal (Get-FileSha256Hex $rawPath) $object.raw_entry.unpacked_sha256 "$($object.raw_entry.path) SHA-256"
    foreach ($output in @($object.native_outputs)) {
        Assert-FileEvidence $output
    }
}

Assert-RawNativeEvidenceFixture `
    -RelativePath 'tests/fixtures/native-evidence/8.3.27.2214/task-assignee' `
    -Issue 317 `
    -Kind 'Task' `
    -Uuid '3ad08f4a-6202-4099-b6cc-bc116e6731a0'
Assert-RawNativeEvidenceFixture `
    -RelativePath 'tests/fixtures/native-evidence/8.3.27.2214/business-process-duty' `
    -Issue 282 `
    -Kind 'BusinessProcess' `
    -Uuid 'dad11c2e-08fc-4a6b-8829-8be6c64c15fc'
Assert-RawNativeEvidenceFixture `
    -RelativePath 'tests/fixtures/native-evidence/8.3.27.2214/chart-of-characteristic-types' `
    -Issue 282 `
    -Kind 'ChartOfCharacteristicTypes' `
    -Uuid 'd003f1f8-d632-4f80-adad-af1583998864'
Assert-RawNativeEvidenceCorpus `
    -RelativePath 'tests/fixtures/native-evidence/8.3.27.2214/register-generated-types' `
    -Issue 282 `
    -ExpectedObjects @{
        AccountingRegister = '8b6ea484-0164-4c68-a0cb-175a31c56186'
        CalculationRegister = '5ad20ecf-0375-4218-b348-0534286973a5'
        ChartOfAccounts = '7671ada0-5cde-47a2-b49e-8de67818fb10'
        ChartOfCalculationTypes = '8c132029-d49c-49db-b12b-64519b64d755'
    }

$dcsFixtureRoot = Join-Path $RepositoryRoot 'tests/fixtures/native-evidence/8.3.27.2214/dcs-core'
$dcsManifestPath = Join-Path $dcsFixtureRoot 'manifest.json'
if (-not (Test-Path -LiteralPath $dcsManifestPath -PathType Leaf)) {
    throw "DCS evidence manifest is missing: $dcsManifestPath"
}
$dcsManifest = Get-Content -LiteralPath $dcsManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsManifest.schema_version 1 'DCS manifest schema version'
Assert-Equal $dcsManifest.issue 283 'DCS issue'
Assert-Equal $dcsManifest.evidence.platform_version '8.3.27.2214' 'DCS platform version'
Assert-Equal $dcsManifest.evidence.source_version '2.20' 'DCS source version'
foreach ($definition in @(
    $dcsManifest.seed.report_definition,
    $dcsManifest.seed.dcs_definition,
    $dcsManifest.seed.generated_template
)) {
    Assert-FileEvidence $definition $dcsFixtureRoot
}
Assert-FileEvidence $dcsManifest.template.raw_entry.packed $dcsFixtureRoot
Assert-FileEvidence $dcsManifest.template.raw_entry.unpacked $dcsFixtureRoot
Assert-FileEvidence $dcsManifest.template.native_xml $dcsFixtureRoot

$dcsPackedPath = Resolve-FixturePath $dcsManifest.template.raw_entry.packed.path $dcsFixtureRoot
$dcsUnpackedPath = Resolve-FixturePath $dcsManifest.template.raw_entry.unpacked.path $dcsFixtureRoot
$dcsCompressedStream = New-Object IO.MemoryStream(,([IO.File]::ReadAllBytes($dcsPackedPath)))
$dcsDecompressedStream = New-Object IO.MemoryStream
$dcsDeflateStream = New-Object IO.Compression.DeflateStream(
    $dcsCompressedStream,
    [IO.Compression.CompressionMode]::Decompress
)
try {
    $dcsDeflateStream.CopyTo($dcsDecompressedStream)
} finally {
    $dcsDeflateStream.Dispose()
    $dcsCompressedStream.Dispose()
}
try {
    $dcsDecompressed = $dcsDecompressedStream.ToArray()
} finally {
    $dcsDecompressedStream.Dispose()
}
Assert-Equal $dcsDecompressed.Length ([long]$dcsManifest.template.raw_entry.unpacked.size) 'DCS decoded size'
Assert-Equal (Get-Sha256Hex $dcsDecompressed) $dcsManifest.template.raw_entry.unpacked.sha256 'DCS decoded SHA-256'
Assert-Equal (Get-Sha256Hex ([IO.File]::ReadAllBytes($dcsUnpackedPath))) (Get-Sha256Hex $dcsDecompressed) 'DCS packed/unpacked pair'
Assert-Equal ([BitConverter]::ToUInt32($dcsDecompressed, 0)) ([uint32]$dcsManifest.proven_shape.header_marker) 'DCS header marker'
Assert-Equal ([BitConverter]::ToUInt32($dcsDecompressed, 4)) ([uint32]$dcsManifest.proven_shape.settings_document_count) 'DCS settings document count'
Assert-Equal ([BitConverter]::ToUInt64($dcsDecompressed, 8)) ([uint64]$dcsManifest.proven_shape.stored_document_lengths[0]) 'DCS first document length'
Assert-Equal ([BitConverter]::ToUInt64($dcsDecompressed, 16)) ([uint64]$dcsManifest.proven_shape.stored_document_lengths[1]) 'DCS second document length'

$dcsSelectionRoot = Join-Path $RepositoryRoot 'tests/fixtures/native-evidence/8.3.27.2214/dcs-selection-auto'
$dcsSelectionManifestPath = Join-Path $dcsSelectionRoot 'manifest.json'
if (-not (Test-Path -LiteralPath $dcsSelectionManifestPath -PathType Leaf)) {
    throw "DCS selection evidence manifest is missing: $dcsSelectionManifestPath"
}
$dcsSelectionManifest = Get-Content -LiteralPath $dcsSelectionManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsSelectionManifest.schema_version 1 'DCS selection manifest schema version'
Assert-Equal $dcsSelectionManifest.issue 283 'DCS selection issue'
Assert-Equal $dcsSelectionManifest.evidence.platform_version '8.3.27.2214' 'DCS selection platform version'
Assert-Equal $dcsSelectionManifest.evidence.platform_line '8.3.27' 'DCS selection platform line'
Assert-Equal $dcsSelectionManifest.evidence.source_version '2.20' 'DCS selection source version'
Assert-FileEvidence $dcsSelectionManifest.seed.patch $dcsSelectionRoot
Assert-FileEvidence $dcsSelectionManifest.template.raw_entry.packed $dcsSelectionRoot
Assert-FileEvidence $dcsSelectionManifest.template.raw_entry.unpacked $dcsSelectionRoot
Assert-FileEvidence $dcsSelectionManifest.template.native_xml $dcsSelectionRoot

$dcsSelectionPolicyPath = Join-Path $RepositoryRoot 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-selection-evidence.json'
if (-not (Test-Path -LiteralPath $dcsSelectionPolicyPath -PathType Leaf)) {
    throw "DCS selection policy evidence is missing: $dcsSelectionPolicyPath"
}
$dcsSelectionPolicy = Get-Content -LiteralPath $dcsSelectionPolicyPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsSelectionPolicy.source.fixtureId $dcsSelectionManifest.fixture_id 'DCS selection policy fixture binding'
Assert-Equal $dcsSelectionPolicy.source.platformVersion $dcsSelectionManifest.evidence.platform_version 'DCS selection policy platform binding'
Assert-Equal $dcsSelectionPolicy.source.sourceVersion $dcsSelectionManifest.evidence.source_version 'DCS selection policy source binding'
Assert-Equal $dcsSelectionPolicy.source.rawBodySha256 $dcsSelectionManifest.template.raw_entry.unpacked.sha256 'DCS selection policy body binding'
Assert-Equal $dcsSelectionPolicy.source.nativeXmlSha256 $dcsSelectionManifest.template.native_xml.sha256 'DCS selection policy XML binding'
Assert-Equal $dcsSelectionPolicy.source.roundTrips 2 'DCS selection policy round-trip count'

$dcsSelectionPackedPath = Resolve-FixturePath $dcsSelectionManifest.template.raw_entry.packed.path $dcsSelectionRoot
$dcsSelectionUnpackedPath = Resolve-FixturePath $dcsSelectionManifest.template.raw_entry.unpacked.path $dcsSelectionRoot
$dcsSelectionCompressedStream = New-Object IO.MemoryStream(,([IO.File]::ReadAllBytes($dcsSelectionPackedPath)))
$dcsSelectionDecompressedStream = New-Object IO.MemoryStream
$dcsSelectionDeflateStream = New-Object IO.Compression.DeflateStream(
    $dcsSelectionCompressedStream,
    [IO.Compression.CompressionMode]::Decompress
)
try {
    $dcsSelectionDeflateStream.CopyTo($dcsSelectionDecompressedStream)
} finally {
    $dcsSelectionDeflateStream.Dispose()
    $dcsSelectionCompressedStream.Dispose()
}
try {
    $dcsSelectionDecompressed = $dcsSelectionDecompressedStream.ToArray()
} finally {
    $dcsSelectionDecompressedStream.Dispose()
}
Assert-Equal $dcsSelectionDecompressed.Length ([long]$dcsSelectionManifest.template.raw_entry.unpacked.size) 'DCS selection decoded size'
Assert-Equal (Get-Sha256Hex $dcsSelectionDecompressed) $dcsSelectionManifest.template.raw_entry.unpacked.sha256 'DCS selection decoded SHA-256'
Assert-Equal (Get-FileSha256Hex $dcsSelectionUnpackedPath) (Get-Sha256Hex $dcsSelectionDecompressed) 'DCS selection packed/unpacked pair'
Assert-Equal ([BitConverter]::ToUInt32($dcsSelectionDecompressed, 0)) ([uint32]$dcsSelectionManifest.proven_shape.header_marker) 'DCS selection header marker'
Assert-Equal ([BitConverter]::ToUInt32($dcsSelectionDecompressed, 4)) ([uint32]$dcsSelectionManifest.proven_shape.settings_document_count) 'DCS selection settings document count'
$dcsSelectionFirstLength = [int]$dcsSelectionManifest.proven_shape.stored_document_lengths[0]
$dcsSelectionSecondLength = [int]$dcsSelectionManifest.proven_shape.stored_document_lengths[1]
Assert-Equal ([BitConverter]::ToUInt64($dcsSelectionDecompressed, 8)) ([uint64]$dcsSelectionFirstLength) 'DCS selection first document length'
Assert-Equal ([BitConverter]::ToUInt64($dcsSelectionDecompressed, 16)) ([uint64]$dcsSelectionSecondLength) 'DCS selection second document length'
$dcsSelectionThirdLength = $dcsSelectionDecompressed.Length - 24 - $dcsSelectionFirstLength - $dcsSelectionSecondLength
Assert-Equal $dcsSelectionThirdLength ([int]$dcsSelectionManifest.proven_shape.trailing_document_length) 'DCS selection trailing document length'
$dcsSelectionDocuments = @(
    Get-ByteSlice $dcsSelectionDecompressed 24 $dcsSelectionFirstLength
    Get-ByteSlice $dcsSelectionDecompressed (24 + $dcsSelectionFirstLength) $dcsSelectionSecondLength
    Get-ByteSlice $dcsSelectionDecompressed (24 + $dcsSelectionFirstLength + $dcsSelectionSecondLength) $dcsSelectionThirdLength
)
for ($index = 0; $index -lt $dcsSelectionDocuments.Count; $index++) {
    Assert-Equal (Get-Sha256Hex $dcsSelectionDocuments[$index]) $dcsSelectionManifest.proven_shape.document_sha256[$index] "DCS selection document $($index + 1) SHA-256"
}
Assert-Equal (Get-Sha256Hex $dcsSelectionDocuments[0]) (Get-Sha256Hex (Get-ByteSlice $dcsDecompressed 24 ([int]$dcsManifest.proven_shape.stored_document_lengths[0]))) 'DCS selection/base schema document equality'
$dcsBaseThirdOffset = 24 + [int]$dcsManifest.proven_shape.stored_document_lengths[0] + [int]$dcsManifest.proven_shape.stored_document_lengths[1]
$dcsBaseThirdLength = $dcsDecompressed.Length - $dcsBaseThirdOffset
Assert-Equal (Get-Sha256Hex $dcsSelectionDocuments[2]) (Get-Sha256Hex (Get-ByteSlice $dcsDecompressed $dcsBaseThirdOffset $dcsBaseThirdLength)) 'DCS selection/base trailing document equality'

$dcsEncodedCfPath = Resolve-FixturePath $dcsManifest.configuration_cf.path $dcsFixtureRoot
if (-not (Test-Path -LiteralPath $dcsEncodedCfPath -PathType Leaf)) {
    throw "Encoded DCS CF is missing: $($dcsManifest.configuration_cf.path)"
}
Assert-Equal (Get-Item -LiteralPath $dcsEncodedCfPath).Length ([long]$dcsManifest.configuration_cf.encoded_size) 'encoded DCS CF size'
Assert-Equal (Get-FileSha256Hex $dcsEncodedCfPath) $dcsManifest.configuration_cf.encoded_sha256 'encoded DCS CF SHA-256'
$dcsEncodedCf = Get-Content -LiteralPath $dcsEncodedCfPath -Raw -Encoding ASCII
$dcsCfBytes = [Convert]::FromBase64String(($dcsEncodedCf -replace '\s', ''))
Assert-Equal $dcsCfBytes.Length ([long]$dcsManifest.configuration_cf.decoded_size) 'decoded DCS CF size'
Assert-Equal (Get-Sha256Hex $dcsCfBytes) $dcsManifest.configuration_cf.decoded_sha256 'decoded DCS CF SHA-256'

$dcsDataParametersRoot = Join-Path $RepositoryRoot 'tests/fixtures/native-evidence/8.3.27.2214/dcs-data-parameters-source-owned'
$dcsDataParametersManifestPath = Join-Path $dcsDataParametersRoot 'manifest.json'
if (-not (Test-Path -LiteralPath $dcsDataParametersManifestPath -PathType Leaf)) {
    throw "DCS dataParameters evidence manifest is missing: $dcsDataParametersManifestPath"
}
$dcsDataParametersManifest = Get-Content -LiteralPath $dcsDataParametersManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsDataParametersManifest.schema_version 1 'DCS dataParameters manifest schema version'
Assert-Equal $dcsDataParametersManifest.fixture_id '8.3.27.2214-xml-2.20-dcs-data-parameters-source-owned' 'DCS dataParameters fixture ID'
Assert-Equal $dcsDataParametersManifest.issue 283 'DCS dataParameters issue'
Assert-Equal $dcsDataParametersManifest.contract 'dcs-recognized-untyped-source-owned-passthrough' 'DCS dataParameters contract'
Assert-Equal $dcsDataParametersManifest.evidence.platform_line '8.3.27' 'DCS dataParameters platform line'
Assert-Equal $dcsDataParametersManifest.evidence.platform_version '8.3.27.2214' 'DCS dataParameters exact platform provenance'
Assert-Equal $dcsDataParametersManifest.evidence.source_version '2.20' 'DCS dataParameters source version'
Assert-Equal $dcsDataParametersManifest.evidence.database_locale 'ru_RU' 'DCS dataParameters database locale'
Assert-Equal $dcsDataParametersManifest.rounds.template_equal_between_rounds $true 'DCS dataParameters Template round equality'
Assert-Equal $dcsDataParametersManifest.rounds.packed_equal_between_rounds $true 'DCS dataParameters packed round equality'
Assert-Equal $dcsDataParametersManifest.rounds.unpacked_equal_between_rounds $true 'DCS dataParameters unpacked round equality'
Assert-Equal $dcsDataParametersManifest.rounds.round_1_template_sha256 $dcsDataParametersManifest.rounds.round_2_template_sha256 'DCS dataParameters Template round hashes'
Assert-Equal $dcsDataParametersManifest.rounds.round_1_packed_sha256 $dcsDataParametersManifest.rounds.round_2_packed_sha256 'DCS dataParameters packed round hashes'
Assert-Equal $dcsDataParametersManifest.rounds.round_1_unpacked_sha256 $dcsDataParametersManifest.rounds.round_2_unpacked_sha256 'DCS dataParameters unpacked round hashes'
Assert-FileEvidence $dcsDataParametersManifest.seed.delta $dcsDataParametersRoot

$dcsDataParametersCfBytes = Get-Base64EvidenceBytes $dcsDataParametersManifest.configuration_cf $dcsDataParametersRoot
$dcsDataParametersTemplateBytes = Get-Base64EvidenceBytes $dcsDataParametersManifest.template.native_xml $dcsDataParametersRoot
$dcsDataParametersPackedBytes = Get-Base64EvidenceBytes $dcsDataParametersManifest.template.raw_entry.packed $dcsDataParametersRoot
$dcsDataParametersUnpackedBytes = Get-Base64EvidenceBytes $dcsDataParametersManifest.template.raw_entry.unpacked $dcsDataParametersRoot
Assert-Equal (Get-Sha256Hex $dcsDataParametersCfBytes) $dcsDataParametersManifest.rounds.round_2_cf.sha256 'DCS dataParameters retained round-2 CF SHA-256'
Assert-Equal (Get-Sha256Hex $dcsDataParametersTemplateBytes) $dcsDataParametersManifest.rounds.round_1_template_sha256 'DCS dataParameters retained Template/round-1 binding'
Assert-Equal (Get-Sha256Hex $dcsDataParametersTemplateBytes) $dcsDataParametersManifest.rounds.round_2_template_sha256 'DCS dataParameters retained Template/round-2 binding'
Assert-Equal (Get-Sha256Hex $dcsDataParametersPackedBytes) $dcsDataParametersManifest.rounds.round_1_packed_sha256 'DCS dataParameters retained packed/round-1 binding'
Assert-Equal (Get-Sha256Hex $dcsDataParametersPackedBytes) $dcsDataParametersManifest.rounds.round_2_packed_sha256 'DCS dataParameters retained packed/round-2 binding'
Assert-Equal (Get-Sha256Hex $dcsDataParametersUnpackedBytes) $dcsDataParametersManifest.rounds.round_1_unpacked_sha256 'DCS dataParameters retained unpacked/round-1 binding'
Assert-Equal (Get-Sha256Hex $dcsDataParametersUnpackedBytes) $dcsDataParametersManifest.rounds.round_2_unpacked_sha256 'DCS dataParameters retained unpacked/round-2 binding'

$dcsDataParametersCompressedStream = New-Object IO.MemoryStream(,($dcsDataParametersPackedBytes))
$dcsDataParametersDecompressedStream = New-Object IO.MemoryStream
$dcsDataParametersDeflateStream = New-Object IO.Compression.DeflateStream(
    $dcsDataParametersCompressedStream,
    [IO.Compression.CompressionMode]::Decompress
)
try {
    $dcsDataParametersDeflateStream.CopyTo($dcsDataParametersDecompressedStream)
} finally {
    $dcsDataParametersDeflateStream.Dispose()
    $dcsDataParametersCompressedStream.Dispose()
}
try {
    $dcsDataParametersDecompressed = $dcsDataParametersDecompressedStream.ToArray()
} finally {
    $dcsDataParametersDecompressedStream.Dispose()
}
Assert-Equal (Get-Sha256Hex $dcsDataParametersDecompressed) (Get-Sha256Hex $dcsDataParametersUnpackedBytes) 'DCS dataParameters packed/unpacked pair'
Assert-Equal ([BitConverter]::ToUInt32($dcsDataParametersUnpackedBytes, 0)) ([uint32]$dcsDataParametersManifest.proven_shape.header_marker) 'DCS dataParameters header marker'
Assert-Equal ([BitConverter]::ToUInt32($dcsDataParametersUnpackedBytes, 4)) ([uint32]$dcsDataParametersManifest.proven_shape.settings_document_count) 'DCS dataParameters settings document count'
$dcsDataParametersFirstLength = [int]$dcsDataParametersManifest.proven_shape.stored_document_lengths[0]
$dcsDataParametersSecondLength = [int]$dcsDataParametersManifest.proven_shape.stored_document_lengths[1]
Assert-Equal ([BitConverter]::ToUInt64($dcsDataParametersUnpackedBytes, 8)) ([uint64]$dcsDataParametersFirstLength) 'DCS dataParameters first document length'
Assert-Equal ([BitConverter]::ToUInt64($dcsDataParametersUnpackedBytes, 16)) ([uint64]$dcsDataParametersSecondLength) 'DCS dataParameters second document length'
$dcsDataParametersThirdLength = $dcsDataParametersUnpackedBytes.Length - 24 - $dcsDataParametersFirstLength - $dcsDataParametersSecondLength
Assert-Equal $dcsDataParametersThirdLength ([int]$dcsDataParametersManifest.proven_shape.trailing_document_length) 'DCS dataParameters trailing document length'
$dcsDataParametersDocuments = @(
    Get-ByteSlice $dcsDataParametersUnpackedBytes 24 $dcsDataParametersFirstLength
    Get-ByteSlice $dcsDataParametersUnpackedBytes (24 + $dcsDataParametersFirstLength) $dcsDataParametersSecondLength
    Get-ByteSlice $dcsDataParametersUnpackedBytes (24 + $dcsDataParametersFirstLength + $dcsDataParametersSecondLength) $dcsDataParametersThirdLength
)
for ($index = 0; $index -lt $dcsDataParametersDocuments.Count; $index++) {
    Assert-Equal (Get-Sha256Hex $dcsDataParametersDocuments[$index]) $dcsDataParametersManifest.proven_shape.document_sha256[$index] "DCS dataParameters document $($index + 1) SHA-256"
}
$dcsBaseFirstLength = [int]$dcsManifest.proven_shape.stored_document_lengths[0]
$dcsBaseSecondLength = [int]$dcsManifest.proven_shape.stored_document_lengths[1]
$dcsBaseThirdOffset = 24 + $dcsBaseFirstLength + $dcsBaseSecondLength
$dcsBaseThirdLength = $dcsDecompressed.Length - $dcsBaseThirdOffset
Assert-Equal (Get-Sha256Hex $dcsDataParametersDocuments[0]) (Get-Sha256Hex (Get-ByteSlice $dcsDecompressed 24 $dcsBaseFirstLength)) 'DCS dataParameters/base schema document equality'
Assert-Equal (Get-Sha256Hex $dcsDataParametersDocuments[2]) (Get-Sha256Hex (Get-ByteSlice $dcsDecompressed $dcsBaseThirdOffset $dcsBaseThirdLength)) 'DCS dataParameters/base trailing document equality'

$dcsDataParametersTemplateText = [Text.Encoding]::UTF8.GetString($dcsDataParametersTemplateBytes)
$dcsDataParametersTemplateDocument = New-Object Xml.XmlDocument
$dcsDataParametersTemplateDocument.PreserveWhitespace = $true
$dcsDataParametersTemplateStream = New-Object IO.MemoryStream(,($dcsDataParametersTemplateBytes))
try {
    $dcsDataParametersTemplateDocument.Load($dcsDataParametersTemplateStream)
} finally {
    $dcsDataParametersTemplateStream.Dispose()
}
$dcsDataParametersNamespaces = New-Object Xml.XmlNamespaceManager($dcsDataParametersTemplateDocument.NameTable)
$dcsDataParametersNamespaces.AddNamespace('dcs', 'http://v8.1c.ru/8.1/data-composition-system/schema')
$dcsDataParametersNamespaces.AddNamespace('dcsset', 'http://v8.1c.ru/8.1/data-composition-system/settings')
$dcsDataParametersNamespaces.AddNamespace('dcscor', 'http://v8.1c.ru/8.1/data-composition-system/core')
$dcsDataParametersNamespaces.AddNamespace('xsi', 'http://www.w3.org/2001/XMLSchema-instance')
$dcsDataParametersNode = $dcsDataParametersTemplateDocument.SelectSingleNode(
    '/dcs:DataCompositionSchema/dcs:settingsVariant/dcsset:settings/dcsset:dataParameters',
    $dcsDataParametersNamespaces
)
if ($null -eq $dcsDataParametersNode) {
    throw 'DCS dataParameters direct Settings child is missing from retained native Template.xml.'
}
$dcsDataParametersSettingsNode = $dcsDataParametersNode.ParentNode
$dcsDataParametersChildOrder = @(
    $dcsDataParametersSettingsNode.ChildNodes |
        Where-Object { $_.NodeType -eq [Xml.XmlNodeType]::Element } |
        ForEach-Object { $_.LocalName }
)
Assert-Equal ($dcsDataParametersChildOrder -join ',') ($dcsDataParametersManifest.proven_shape.root_settings_child_order -join ',') 'DCS dataParameters direct Settings child order'
$dcsDataParametersItems = @($dcsDataParametersNode.SelectNodes('dcscor:item', $dcsDataParametersNamespaces))
Assert-Equal $dcsDataParametersItems.Count 1 'DCS dataParameters item count'
$dcsDataParametersItem = $dcsDataParametersItems[0]
Assert-Equal $dcsDataParametersItem.NamespaceURI 'http://v8.1c.ru/8.1/data-composition-system/core' 'DCS dataParameters item namespace'
Assert-Equal $dcsDataParametersItem.GetAttribute('type', 'http://www.w3.org/2001/XMLSchema-instance') 'dcsset:SettingsParameterValue' 'DCS dataParameters item xsi:type'
$dcsDataParametersParameter = $dcsDataParametersItem.SelectSingleNode('dcscor:parameter', $dcsDataParametersNamespaces)
$dcsDataParametersValue = $dcsDataParametersItem.SelectSingleNode('dcscor:value', $dcsDataParametersNamespaces)
Assert-Equal $dcsDataParametersParameter.InnerText $dcsDataParametersManifest.proven_shape.parameter 'DCS dataParameters parameter'
Assert-Equal $dcsDataParametersValue.InnerText $dcsDataParametersManifest.proven_shape.value 'DCS dataParameters value'
Assert-Equal $dcsDataParametersValue.GetAttribute('type', 'http://www.w3.org/2001/XMLSchema-instance') 'xs:string' 'DCS dataParameters value xsi:type'
Assert-Equal $dcsDataParametersManifest.proven_shape.data_parameters_qname '{http://v8.1c.ru/8.1/data-composition-system/settings}dataParameters' 'DCS dataParameters expanded QName'
Assert-Equal $dcsDataParametersManifest.proven_shape.item_qname '{http://v8.1c.ru/8.1/data-composition-system/core}item' 'DCS dataParameters item expanded QName'
Assert-Equal $dcsDataParametersManifest.proven_shape.item_type_qname '{http://v8.1c.ru/8.1/data-composition-system/settings}SettingsParameterValue' 'DCS dataParameters item type expanded QName'
Assert-Equal $dcsDataParametersManifest.proven_shape.value_type_qname '{http://www.w3.org/2001/XMLSchema}string' 'DCS dataParameters value type expanded QName'
$dcsDataParametersFragmentStart = $dcsDataParametersTemplateText.IndexOf('<dcsset:dataParameters>', [StringComparison]::Ordinal)
$dcsDataParametersFragmentEndTag = '</dcsset:dataParameters>'
$dcsDataParametersFragmentEnd = $dcsDataParametersTemplateText.IndexOf($dcsDataParametersFragmentEndTag, $dcsDataParametersFragmentStart, [StringComparison]::Ordinal)
if ($dcsDataParametersFragmentStart -lt 0 -or $dcsDataParametersFragmentEnd -lt 0) {
    throw 'DCS dataParameters exact fragment is missing from retained native Template.xml.'
}
$dcsDataParametersFragmentText = $dcsDataParametersTemplateText.Substring(
    $dcsDataParametersFragmentStart,
    $dcsDataParametersFragmentEnd - $dcsDataParametersFragmentStart + $dcsDataParametersFragmentEndTag.Length
)
$dcsDataParametersFragmentBytes = [Text.Encoding]::UTF8.GetBytes($dcsDataParametersFragmentText)
Assert-Equal $dcsDataParametersFragmentBytes.Length ([long]$dcsDataParametersManifest.template.data_parameters_fragment.decoded_size) 'DCS dataParameters fragment size'
Assert-Equal (Get-Sha256Hex $dcsDataParametersFragmentBytes) $dcsDataParametersManifest.template.data_parameters_fragment.sha256 'DCS dataParameters fragment SHA-256'

$dcsFeatureSemanticsPath = Join-Path $RepositoryRoot 'crates/ibcmd-schema/data/edt-2025.2.3-feature-semantics.json'
$dcsCanonicalCoveragePath = Join-Path $RepositoryRoot 'crates/ibcmd-schema/data/edt-2025.2.3-canonical-coverage.json'
$dcsFeatureSemantics = Get-Content -LiteralPath $dcsFeatureSemanticsPath -Raw -Encoding UTF8 | ConvertFrom-Json
$dcsCanonicalCoverage = Get-Content -LiteralPath $dcsCanonicalCoveragePath -Raw -Encoding UTF8 | ConvertFrom-Json
$dcsDataParametersFeatures = @(
    $dcsFeatureSemantics.packages |
        ForEach-Object { $_.classifiers } |
        Where-Object { $_.name -eq $dcsDataParametersManifest.classification.xcore_classifier } |
        ForEach-Object { $_.features } |
        Where-Object { $_.name -eq $dcsDataParametersManifest.classification.xcore_feature }
)
Assert-Equal $dcsDataParametersFeatures.Count 1 'DCS dataParameters Xcore feature cardinality'
Assert-Equal $dcsDataParametersFeatures[0].kind $dcsDataParametersManifest.classification.xcore_kind 'DCS dataParameters Xcore feature kind'
Assert-Equal $dcsDataParametersFeatures[0].modelType $dcsDataParametersManifest.classification.xcore_model_type 'DCS dataParameters Xcore model type'
$dcsDataParametersCoverageEntries = @(
    $dcsCanonicalCoverage.entries |
        Where-Object {
            $_.key.classifier -eq $dcsDataParametersManifest.classification.xcore_classifier -and
            $_.key.feature -eq $dcsDataParametersManifest.classification.xcore_feature
        }
)
Assert-Equal $dcsDataParametersCoverageEntries.Count 1 'DCS dataParameters canonical coverage cardinality'
Assert-Equal $dcsDataParametersCoverageEntries[0].status $dcsDataParametersManifest.classification.canonical_coverage_status 'DCS dataParameters canonical coverage status'
Assert-Equal $dcsDataParametersCoverageEntries[0].diagnosticCode $dcsDataParametersManifest.classification.canonical_diagnostic_code 'DCS dataParameters canonical diagnostic code'
Assert-Equal $dcsDataParametersCoverageEntries[0].opaquePlacement $null 'DCS dataParameters canonical opaque-placement absence'
Assert-Equal $dcsCanonicalCoverage.summary.opaqueLossless 0 'canonical coverage opaque-lossless count'

$dcsMultiVariantRoot = Join-Path $RepositoryRoot 'tests/fixtures/native-evidence/8.3.27.2214/dcs-multi-variant-envelope'
$dcsMultiVariantManifestPath = Join-Path $dcsMultiVariantRoot 'manifest.json'
if (-not (Test-Path -LiteralPath $dcsMultiVariantManifestPath -PathType Leaf)) {
    throw "DCS multi-variant evidence manifest is missing: $dcsMultiVariantManifestPath"
}
$dcsMultiVariantManifest = Get-Content -LiteralPath $dcsMultiVariantManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsMultiVariantManifest.schema_version 1 'DCS multi-variant manifest schema version'
Assert-Equal $dcsMultiVariantManifest.fixture_id '8.3.27.2214-xml-2.20-dcs-multi-variant-envelope' 'DCS multi-variant fixture ID'
Assert-Equal $dcsMultiVariantManifest.issue 283 'DCS multi-variant issue'
Assert-Equal $dcsMultiVariantManifest.contract 'dcs-schema-template-envelope-positional-settings-v1' 'DCS multi-variant contract'
Assert-Equal $dcsMultiVariantManifest.evidence.platform_line '8.3.27' 'DCS multi-variant platform line'
Assert-Equal $dcsMultiVariantManifest.evidence.platform_version '8.3.27.2214' 'DCS multi-variant exact platform provenance'
Assert-Equal $dcsMultiVariantManifest.evidence.source_version '2.20' 'DCS multi-variant source version'
Assert-Equal $dcsMultiVariantManifest.evidence.database_locale 'ru_RU' 'DCS multi-variant database locale'
Assert-Equal $dcsMultiVariantManifest.rounds.selected_native_tree_inventory_equal_between_rounds $true 'DCS multi-variant selected-tree inventory equality'
Assert-Equal (@($dcsMultiVariantManifest.rounds.selected_native_tree_inventory).Count) 5 'DCS multi-variant selected-tree inventory cardinality'
Assert-Equal $dcsMultiVariantManifest.rounds.template_equal_between_rounds $true 'DCS multi-variant Template round equality'
Assert-Equal $dcsMultiVariantManifest.rounds.packed_equal_between_rounds $true 'DCS multi-variant packed round equality'
Assert-Equal $dcsMultiVariantManifest.rounds.unpacked_equal_between_rounds $true 'DCS multi-variant unpacked round equality'
Assert-Equal $dcsMultiVariantManifest.rounds.round_1_template_sha256 $dcsMultiVariantManifest.rounds.round_2_template_sha256 'DCS multi-variant Template round hashes'
Assert-Equal $dcsMultiVariantManifest.rounds.round_1_packed_sha256 $dcsMultiVariantManifest.rounds.round_2_packed_sha256 'DCS multi-variant packed round hashes'
Assert-Equal $dcsMultiVariantManifest.rounds.round_1_unpacked_sha256 $dcsMultiVariantManifest.rounds.round_2_unpacked_sha256 'DCS multi-variant unpacked round hashes'
Assert-FileEvidence $dcsMultiVariantManifest.seed $dcsMultiVariantRoot

$dcsMultiVariantCfBytes = Get-Base64EvidenceBytes $dcsMultiVariantManifest.configuration_cf $dcsMultiVariantRoot
$dcsMultiVariantTemplateBytes = Get-Base64EvidenceBytes $dcsMultiVariantManifest.template.native_xml $dcsMultiVariantRoot
$dcsMultiVariantPackedBytes = Get-Base64EvidenceBytes $dcsMultiVariantManifest.template.raw_entry.packed $dcsMultiVariantRoot
$dcsMultiVariantUnpackedBytes = Get-Base64EvidenceBytes $dcsMultiVariantManifest.template.raw_entry.unpacked $dcsMultiVariantRoot
Assert-Equal (Get-Sha256Hex $dcsMultiVariantCfBytes) $dcsMultiVariantManifest.rounds.round_2_cf.sha256 'DCS multi-variant retained round-2 CF SHA-256'
Assert-Equal $dcsMultiVariantCfBytes.Length ([long]$dcsMultiVariantManifest.rounds.round_2_cf.size) 'DCS multi-variant retained round-2 CF size'
Assert-Equal (Get-Sha256Hex $dcsMultiVariantTemplateBytes) $dcsMultiVariantManifest.rounds.round_1_template_sha256 'DCS multi-variant retained Template/round-1 binding'
Assert-Equal (Get-Sha256Hex $dcsMultiVariantTemplateBytes) $dcsMultiVariantManifest.rounds.round_2_template_sha256 'DCS multi-variant retained Template/round-2 binding'
Assert-Equal (Get-Sha256Hex $dcsMultiVariantPackedBytes) $dcsMultiVariantManifest.rounds.round_1_packed_sha256 'DCS multi-variant retained packed/round-1 binding'
Assert-Equal (Get-Sha256Hex $dcsMultiVariantPackedBytes) $dcsMultiVariantManifest.rounds.round_2_packed_sha256 'DCS multi-variant retained packed/round-2 binding'
Assert-Equal (Get-Sha256Hex $dcsMultiVariantUnpackedBytes) $dcsMultiVariantManifest.rounds.round_1_unpacked_sha256 'DCS multi-variant retained unpacked/round-1 binding'
Assert-Equal (Get-Sha256Hex $dcsMultiVariantUnpackedBytes) $dcsMultiVariantManifest.rounds.round_2_unpacked_sha256 'DCS multi-variant retained unpacked/round-2 binding'

$dcsMultiVariantCompressedStream = New-Object IO.MemoryStream(,($dcsMultiVariantPackedBytes))
$dcsMultiVariantDecompressedStream = New-Object IO.MemoryStream
$dcsMultiVariantDeflateStream = New-Object IO.Compression.DeflateStream(
    $dcsMultiVariantCompressedStream,
    [IO.Compression.CompressionMode]::Decompress
)
try {
    $dcsMultiVariantDeflateStream.CopyTo($dcsMultiVariantDecompressedStream)
} finally {
    $dcsMultiVariantDeflateStream.Dispose()
    $dcsMultiVariantCompressedStream.Dispose()
}
try {
    $dcsMultiVariantDecompressed = $dcsMultiVariantDecompressedStream.ToArray()
} finally {
    $dcsMultiVariantDecompressedStream.Dispose()
}
Assert-Equal (Get-Sha256Hex $dcsMultiVariantDecompressed) (Get-Sha256Hex $dcsMultiVariantUnpackedBytes) 'DCS multi-variant packed/unpacked pair'
Assert-Equal ([BitConverter]::ToUInt32($dcsMultiVariantUnpackedBytes, 0)) ([uint32]$dcsMultiVariantManifest.proven_shape.header_marker) 'DCS multi-variant header marker'
$dcsMultiVariantSettingsCount = [int][BitConverter]::ToUInt32($dcsMultiVariantUnpackedBytes, 4)
Assert-Equal $dcsMultiVariantSettingsCount ([int]$dcsMultiVariantManifest.proven_shape.settings_document_count) 'DCS multi-variant external Settings document count'
$dcsMultiVariantHeaderBytes = 8 + (8 * ($dcsMultiVariantSettingsCount + 1))
Assert-Equal $dcsMultiVariantHeaderBytes ([int]$dcsMultiVariantManifest.proven_shape.header_bytes) 'DCS multi-variant header size'
Assert-Equal $dcsMultiVariantManifest.proven_shape.body_layout 'dcs-schema-positional-settings-v1' 'DCS multi-variant body layout'
Assert-Equal (@($dcsMultiVariantManifest.proven_shape.stored_document_lengths).Count) ($dcsMultiVariantSettingsCount + 1) 'DCS multi-variant stored-length count'
Assert-Equal (@($dcsMultiVariantManifest.proven_shape.document_roles).Count) ($dcsMultiVariantSettingsCount + 2) 'DCS multi-variant document-role count'
Assert-Equal (@($dcsMultiVariantManifest.proven_shape.document_sha256).Count) ($dcsMultiVariantSettingsCount + 2) 'DCS multi-variant document-hash count'
Assert-Equal (@($dcsMultiVariantManifest.proven_shape.variant_names).Count) $dcsMultiVariantSettingsCount 'DCS multi-variant variant-name count'
Assert-Equal (@($dcsMultiVariantManifest.proven_shape.document_roles) -join ',') 'PrimarySchemaFile,Settings[0],Settings[1],TerminalSchemaFile' 'DCS multi-variant document roles'

$dcsMultiVariantDocuments = @()
$dcsMultiVariantDocumentOffset = $dcsMultiVariantHeaderBytes
for ($index = 0; $index -lt $dcsMultiVariantSettingsCount + 1; $index++) {
    $storedLength = [int]$dcsMultiVariantManifest.proven_shape.stored_document_lengths[$index]
    Assert-Equal ([BitConverter]::ToUInt64($dcsMultiVariantUnpackedBytes, 8 + (8 * $index))) ([uint64]$storedLength) "DCS multi-variant stored document $($index + 1) length"
    $dcsMultiVariantDocuments += ,(Get-ByteSlice $dcsMultiVariantUnpackedBytes $dcsMultiVariantDocumentOffset $storedLength)
    $dcsMultiVariantDocumentOffset += $storedLength
}
$dcsMultiVariantTrailingLength = $dcsMultiVariantUnpackedBytes.Length - $dcsMultiVariantDocumentOffset
Assert-Equal $dcsMultiVariantTrailingLength ([int]$dcsMultiVariantManifest.proven_shape.trailing_document_length) 'DCS multi-variant trailing document length'
$dcsMultiVariantDocuments += ,(Get-ByteSlice $dcsMultiVariantUnpackedBytes $dcsMultiVariantDocumentOffset $dcsMultiVariantTrailingLength)
Assert-Equal ($dcsMultiVariantHeaderBytes + ($dcsMultiVariantManifest.proven_shape.stored_document_lengths | Measure-Object -Sum).Sum + $dcsMultiVariantTrailingLength) $dcsMultiVariantUnpackedBytes.Length 'DCS multi-variant complete framing'
for ($index = 0; $index -lt $dcsMultiVariantDocuments.Count; $index++) {
    Assert-Equal (Get-Sha256Hex $dcsMultiVariantDocuments[$index]) $dcsMultiVariantManifest.proven_shape.document_sha256[$index] "DCS multi-variant document $($index + 1) SHA-256"
    Assert-Equal ([BitConverter]::ToString($dcsMultiVariantDocuments[$index], 0, 3)) 'EF-BB-BF' "DCS multi-variant document $($index + 1) UTF-8 BOM"
}
Assert-Equal (Get-Sha256Hex $dcsMultiVariantDocuments[1]) (Get-Sha256Hex (Get-ByteSlice $dcsDecompressed (24 + $dcsBaseFirstLength) $dcsBaseSecondLength)) 'DCS multi-variant/base primary Settings equality'
Assert-Equal (Get-Sha256Hex $dcsMultiVariantDocuments[3]) (Get-Sha256Hex (Get-ByteSlice $dcsDecompressed $dcsBaseThirdOffset $dcsBaseThirdLength)) 'DCS multi-variant/base terminal SchemaFile equality'

$dcsMultiVariantStorageDocument = Get-XmlDocumentFromBytes $dcsMultiVariantDocuments[0]
$dcsMultiVariantStorageNamespaces = New-Object Xml.XmlNamespaceManager($dcsMultiVariantStorageDocument.NameTable)
$dcsMultiVariantStorageNamespaces.AddNamespace('dcs', 'http://v8.1c.ru/8.1/data-composition-system/schema')
$dcsMultiVariantStorageNamespaces.AddNamespace('dcsset', 'http://v8.1c.ru/8.1/data-composition-system/settings')
$dcsMultiVariantStorageVariants = @($dcsMultiVariantStorageDocument.SelectNodes('/SchemaFile/dcs:dataCompositionSchema/dcs:settingsVariant', $dcsMultiVariantStorageNamespaces))
Assert-Equal $dcsMultiVariantStorageVariants.Count $dcsMultiVariantSettingsCount 'DCS multi-variant storage shell count'

$dcsMultiVariantTemplateDocument = Get-XmlDocumentFromBytes $dcsMultiVariantTemplateBytes
$dcsMultiVariantTemplateNamespaces = New-Object Xml.XmlNamespaceManager($dcsMultiVariantTemplateDocument.NameTable)
$dcsMultiVariantTemplateNamespaces.AddNamespace('dcs', 'http://v8.1c.ru/8.1/data-composition-system/schema')
$dcsMultiVariantTemplateNamespaces.AddNamespace('dcsset', 'http://v8.1c.ru/8.1/data-composition-system/settings')
$dcsMultiVariantTemplateNamespaces.AddNamespace('v8', 'http://v8.1c.ru/8.1/data/core')
$dcsMultiVariantTemplateNamespaces.AddNamespace('xsi', 'http://www.w3.org/2001/XMLSchema-instance')
$dcsMultiVariantSourceVariants = @($dcsMultiVariantTemplateDocument.SelectNodes('/dcs:DataCompositionSchema/dcs:settingsVariant', $dcsMultiVariantTemplateNamespaces))
Assert-Equal $dcsMultiVariantSourceVariants.Count $dcsMultiVariantSettingsCount 'DCS multi-variant source variant count'
Assert-Equal $dcsMultiVariantManifest.proven_shape.source_variant_order_matches_external_settings_order $true 'DCS multi-variant positional-binding claim'

for ($index = 0; $index -lt $dcsMultiVariantSettingsCount; $index++) {
    $expectedName = $dcsMultiVariantManifest.proven_shape.variant_names[$index]
    $storageVariant = $dcsMultiVariantStorageVariants[$index]
    $storageChildren = @($storageVariant.ChildNodes | Where-Object { $_.NodeType -eq [Xml.XmlNodeType]::Element })
    Assert-Equal (($storageChildren | ForEach-Object { $_.LocalName }) -join ',') 'name,presentation' "DCS multi-variant storage shell $($index + 1) child order"
    Assert-Equal $storageChildren[0].NamespaceURI 'http://v8.1c.ru/8.1/data-composition-system/settings' "DCS multi-variant storage name $($index + 1) namespace"
    Assert-Equal $storageChildren[0].InnerText $expectedName "DCS multi-variant storage name $($index + 1)"

    $sourceVariant = $dcsMultiVariantSourceVariants[$index]
    $sourceChildren = @($sourceVariant.ChildNodes | Where-Object { $_.NodeType -eq [Xml.XmlNodeType]::Element })
    Assert-Equal (($sourceChildren | ForEach-Object { $_.LocalName }) -join ',') 'name,presentation,settings' "DCS multi-variant source variant $($index + 1) child order"
    Assert-Equal $sourceChildren[0].NamespaceURI 'http://v8.1c.ru/8.1/data-composition-system/settings' "DCS multi-variant source name $($index + 1) namespace"
    Assert-Equal $sourceChildren[0].InnerText $expectedName "DCS multi-variant source name $($index + 1)"
    Assert-Equal $sourceChildren[2].NamespaceURI 'http://v8.1c.ru/8.1/data-composition-system/settings' "DCS multi-variant source settings $($index + 1) namespace"
    $presentationLanguage = $sourceChildren[1].SelectSingleNode('v8:item/v8:lang', $dcsMultiVariantTemplateNamespaces)
    $presentationContent = $sourceChildren[1].SelectSingleNode('v8:item/v8:content', $dcsMultiVariantTemplateNamespaces)
    Assert-Equal $presentationLanguage.InnerText 'ru' "DCS multi-variant presentation $($index + 1) language"
    Assert-Equal $presentationContent.InnerText $expectedName "DCS multi-variant presentation $($index + 1) content"

    $externalSettingsDocument = Get-XmlDocumentFromBytes $dcsMultiVariantDocuments[$index + 1]
    Assert-Equal $externalSettingsDocument.DocumentElement.LocalName 'Settings' "DCS multi-variant external Settings $($index + 1) root"
    Assert-Equal $externalSettingsDocument.DocumentElement.NamespaceURI 'http://v8.1c.ru/8.1/data-composition-system/settings' "DCS multi-variant external Settings $($index + 1) namespace"
    $externalSettingsChildren = @($externalSettingsDocument.DocumentElement.ChildNodes | Where-Object { $_.NodeType -eq [Xml.XmlNodeType]::Element })
    $sourceSettingsChildren = @($sourceChildren[2].ChildNodes | Where-Object { $_.NodeType -eq [Xml.XmlNodeType]::Element })
    Assert-Equal (($sourceSettingsChildren | ForEach-Object { $_.LocalName }) -join ',') (($externalSettingsChildren | ForEach-Object { $_.LocalName }) -join ',') "DCS multi-variant Settings $($index + 1) child-order binding"

    $externalNamespaces = New-Object Xml.XmlNamespaceManager($externalSettingsDocument.NameTable)
    $externalNamespaces.AddNamespace('dcsset', 'http://v8.1c.ru/8.1/data-composition-system/settings')
    $externalNamespaces.AddNamespace('xsi', 'http://www.w3.org/2001/XMLSchema-instance')
    $sourceSelectionItems = @($sourceChildren[2].SelectNodes('dcsset:selection/dcsset:item', $dcsMultiVariantTemplateNamespaces))
    $externalSelectionItems = @($externalSettingsDocument.DocumentElement.SelectNodes('dcsset:selection/dcsset:item', $externalNamespaces))
    Assert-Equal $sourceSelectionItems.Count $externalSelectionItems.Count "DCS multi-variant Settings $($index + 1) selection cardinality"
    for ($itemIndex = 0; $itemIndex -lt $sourceSelectionItems.Count; $itemIndex++) {
        $sourceType = $sourceSelectionItems[$itemIndex].GetAttribute('type', 'http://www.w3.org/2001/XMLSchema-instance').Split(':')[-1]
        $externalType = $externalSelectionItems[$itemIndex].GetAttribute('type', 'http://www.w3.org/2001/XMLSchema-instance').Split(':')[-1]
        Assert-Equal $sourceType $externalType "DCS multi-variant Settings $($index + 1) selection item $($itemIndex + 1) type"
        $sourceField = $sourceSelectionItems[$itemIndex].SelectSingleNode('dcsset:field', $dcsMultiVariantTemplateNamespaces)
        $externalField = $externalSelectionItems[$itemIndex].SelectSingleNode('dcsset:field', $externalNamespaces)
        $sourceFieldText = if ($null -eq $sourceField) { '' } else { $sourceField.InnerText }
        $externalFieldText = if ($null -eq $externalField) { '' } else { $externalField.InnerText }
        Assert-Equal $sourceFieldText $externalFieldText "DCS multi-variant Settings $($index + 1) selection item $($itemIndex + 1) field"
    }
}

$dcsMultiVariantTerminalDocument = Get-XmlDocumentFromBytes $dcsMultiVariantDocuments[3]
$dcsMultiVariantTerminalNamespaces = New-Object Xml.XmlNamespaceManager($dcsMultiVariantTerminalDocument.NameTable)
$dcsMultiVariantTerminalNamespaces.AddNamespace('dcs', 'http://v8.1c.ru/8.1/data-composition-system/schema')
$dcsMultiVariantTerminalSchema = $dcsMultiVariantTerminalDocument.SelectSingleNode('/SchemaFile/dcs:dataCompositionSchema', $dcsMultiVariantTerminalNamespaces)
if ($null -eq $dcsMultiVariantTerminalSchema) {
    throw 'DCS multi-variant terminal SchemaFile has no dataCompositionSchema child.'
}
Assert-Equal (@($dcsMultiVariantTerminalSchema.ChildNodes | Where-Object { $_.NodeType -eq [Xml.XmlNodeType]::Element }).Count) 0 'DCS multi-variant terminal SchemaFile emptiness'
Assert-Equal $dcsMultiVariantManifest.proven_shape.terminal_schema_file_is_empty $true 'DCS multi-variant terminal SchemaFile claim'

$dcsMultiVariantPolicyPath = Join-Path $RepositoryRoot 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-schema-template-envelope-evidence.json'
if (-not (Test-Path -LiteralPath $dcsMultiVariantPolicyPath -PathType Leaf)) {
    throw "DCS schema-template envelope policy evidence is missing: $dcsMultiVariantPolicyPath"
}
$dcsMultiVariantPolicy = Get-Content -LiteralPath $dcsMultiVariantPolicyPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsMultiVariantPolicy.schemaVersion 1 'DCS schema-template envelope policy schema version'
Assert-Equal $dcsMultiVariantPolicy.contract '8.3.27-xml-2.20-dcs-schema-template-envelope-v1' 'DCS schema-template envelope policy contract'
Assert-Equal $dcsMultiVariantPolicy.source.product '1C:Enterprise Platform' 'DCS schema-template envelope policy product'
Assert-Equal $dcsMultiVariantPolicy.source.release '8.3.27 / XML 2.20' 'DCS schema-template envelope policy release'
Assert-Equal $dcsMultiVariantPolicy.fixture.fixtureId $dcsMultiVariantManifest.fixture_id 'DCS schema-template envelope fixture binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.platformLine $dcsMultiVariantManifest.evidence.platform_line 'DCS schema-template envelope platform-line binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.platformVersion $dcsMultiVariantManifest.evidence.platform_version 'DCS schema-template envelope platform-version binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.sourceVersion $dcsMultiVariantManifest.evidence.source_version 'DCS schema-template envelope source-version binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.ibcmdSha256 $dcsMultiVariantManifest.evidence.ibcmd_sha256 'DCS schema-template envelope ibcmd binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.extractorIdentity $dcsMultiVariantManifest.evidence.extractor.identity 'DCS schema-template envelope extractor identity binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.extractorSha256 $dcsMultiVariantManifest.evidence.extractor.sha256 'DCS schema-template envelope extractor binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.seedSha256 $dcsMultiVariantManifest.seed.sha256 'DCS schema-template envelope seed binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.round1CfSha256 $dcsMultiVariantManifest.rounds.round_1_cf.sha256 'DCS schema-template envelope round-1 CF binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.round2CfSha256 $dcsMultiVariantManifest.rounds.round_2_cf.sha256 'DCS schema-template envelope round-2 CF binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.round1TemplateSha256 $dcsMultiVariantManifest.rounds.round_1_template_sha256 'DCS schema-template envelope round-1 Template binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.round2TemplateSha256 $dcsMultiVariantManifest.rounds.round_2_template_sha256 'DCS schema-template envelope round-2 Template binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.round1PackedSha256 $dcsMultiVariantManifest.rounds.round_1_packed_sha256 'DCS schema-template envelope round-1 packed binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.round2PackedSha256 $dcsMultiVariantManifest.rounds.round_2_packed_sha256 'DCS schema-template envelope round-2 packed binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.round1UnpackedSha256 $dcsMultiVariantManifest.rounds.round_1_unpacked_sha256 'DCS schema-template envelope round-1 unpacked binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.round2UnpackedSha256 $dcsMultiVariantManifest.rounds.round_2_unpacked_sha256 'DCS schema-template envelope round-2 unpacked binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.configurationEncodedSha256 $dcsMultiVariantManifest.configuration_cf.encoded_sha256 'DCS schema-template envelope encoded CF binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.configurationDecodedSha256 $dcsMultiVariantManifest.configuration_cf.decoded_sha256 'DCS schema-template envelope decoded CF binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.rawPackedEncodedSha256 $dcsMultiVariantManifest.template.raw_entry.packed.encoded_sha256 'DCS schema-template envelope encoded packed binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.rawPackedDecodedSha256 $dcsMultiVariantManifest.template.raw_entry.packed.decoded_sha256 'DCS schema-template envelope decoded packed binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.rawUnpackedEncodedSha256 $dcsMultiVariantManifest.template.raw_entry.unpacked.encoded_sha256 'DCS schema-template envelope encoded unpacked binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.rawUnpackedDecodedSha256 $dcsMultiVariantManifest.template.raw_entry.unpacked.decoded_sha256 'DCS schema-template envelope decoded unpacked binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.nativeXmlEncodedSha256 $dcsMultiVariantManifest.template.native_xml.encoded_sha256 'DCS schema-template envelope encoded native XML binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.nativeXmlDecodedSha256 $dcsMultiVariantManifest.template.native_xml.decoded_sha256 'DCS schema-template envelope decoded native XML binding'
Assert-Equal $dcsMultiVariantPolicy.fixture.roundTrips 2 'DCS schema-template envelope round-trip count'
Assert-Equal $dcsMultiVariantPolicy.policy.schemaNamespace 'http://v8.1c.ru/8.1/data-composition-system/schema' 'DCS schema-template envelope schema namespace'
Assert-Equal $dcsMultiVariantPolicy.policy.settingsNamespace 'http://v8.1c.ru/8.1/data-composition-system/settings' 'DCS schema-template envelope settings namespace'
Assert-Equal $dcsMultiVariantPolicy.policy.sourceRootQname '{http://v8.1c.ru/8.1/data-composition-system/schema}DataCompositionSchema' 'DCS schema-template envelope source root QName'
Assert-Equal $dcsMultiVariantPolicy.policy.sourceSettingsVariantQname '{http://v8.1c.ru/8.1/data-composition-system/schema}settingsVariant' 'DCS schema-template envelope source variant QName'
Assert-Equal $dcsMultiVariantPolicy.policy.sourceInlineSettingsQname '{http://v8.1c.ru/8.1/data-composition-system/settings}settings' 'DCS schema-template envelope source settings QName'
Assert-Equal $dcsMultiVariantPolicy.policy.nativeSchemaFileQname '{}SchemaFile' 'DCS schema-template envelope native SchemaFile QName'
Assert-Equal $dcsMultiVariantPolicy.policy.nativeSchemaQname '{http://v8.1c.ru/8.1/data-composition-system/schema}dataCompositionSchema' 'DCS schema-template envelope native schema QName'
Assert-Equal $dcsMultiVariantPolicy.policy.nativeSettingsQname '{http://v8.1c.ru/8.1/data-composition-system/settings}Settings' 'DCS schema-template envelope native Settings QName'
Assert-Equal $dcsMultiVariantPolicy.policy.headerMarker $dcsMultiVariantManifest.proven_shape.header_marker 'DCS schema-template envelope header marker binding'
Assert-Equal $dcsMultiVariantPolicy.policy.settingsCountOffsetBytes 4 'DCS schema-template envelope settings-count offset'
Assert-Equal $dcsMultiVariantPolicy.policy.settingsCountEncoding 'little-endian-u32' 'DCS schema-template envelope settings-count encoding'
Assert-Equal $dcsMultiVariantPolicy.policy.storedLengthsOffsetBytes 8 'DCS schema-template envelope stored-length offset'
Assert-Equal $dcsMultiVariantPolicy.policy.storedLengthWidthBytes 8 'DCS schema-template envelope stored-length width'
Assert-Equal $dcsMultiVariantPolicy.policy.storedLengthEncoding 'little-endian-u64' 'DCS schema-template envelope stored-length encoding'
Assert-Equal $dcsMultiVariantPolicy.policy.minimumAttestedSettingsVariants 1 'DCS schema-template envelope minimum attested variant count'
Assert-Equal $dcsMultiVariantPolicy.policy.maximumAttestedSettingsVariants 2 'DCS schema-template envelope maximum attested variant count'
Assert-Equal (@($dcsMultiVariantPolicy.policy.storedLengthRoles) -join ',') 'PrimarySchemaFile,Settings[*]' 'DCS schema-template envelope stored-length roles'
Assert-Equal (@($dcsMultiVariantPolicy.policy.documentRoles) -join ',') 'PrimarySchemaFile,Settings[*],TerminalSchemaFile' 'DCS schema-template envelope document roles'
Assert-Equal $dcsMultiVariantPolicy.policy.storedLengthsCover 'primary-schema-file-and-each-settings-document' 'DCS schema-template envelope stored-length coverage'
Assert-Equal $dcsMultiVariantPolicy.policy.terminalDocumentFraming 'remaining-bytes' 'DCS schema-template envelope terminal framing'
Assert-Equal $dcsMultiVariantPolicy.policy.documentEncoding 'utf-8-with-bom' 'DCS schema-template envelope document encoding'
Assert-Equal $dcsMultiVariantPolicy.policy.settingsBinding 'direct-settings-variant-order' 'DCS schema-template envelope settings binding'
Assert-Equal $dcsMultiVariantPolicy.policy.sourceVariantPlacement 'direct-root-child' 'DCS schema-template envelope source placement'
Assert-Equal $dcsMultiVariantPolicy.policy.terminalSchemaFileShape 'empty-data-composition-schema' 'DCS schema-template envelope terminal shape'
Assert-Equal $dcsMultiVariantPolicy.twoVariantShape.settingsDocumentCount $dcsMultiVariantManifest.proven_shape.settings_document_count 'DCS schema-template envelope two-variant settings count'
Assert-Equal $dcsMultiVariantPolicy.twoVariantShape.headerBytes $dcsMultiVariantManifest.proven_shape.header_bytes 'DCS schema-template envelope two-variant header size'
Assert-Equal (@($dcsMultiVariantPolicy.twoVariantShape.storedDocumentLengths) -join ',') (@($dcsMultiVariantManifest.proven_shape.stored_document_lengths) -join ',') 'DCS schema-template envelope two-variant stored lengths'
Assert-Equal $dcsMultiVariantPolicy.twoVariantShape.trailingDocumentLength $dcsMultiVariantManifest.proven_shape.trailing_document_length 'DCS schema-template envelope two-variant trailing length'
Assert-Equal (@($dcsMultiVariantPolicy.twoVariantShape.documentRoles) -join ',') (@($dcsMultiVariantManifest.proven_shape.document_roles) -join ',') 'DCS schema-template envelope two-variant roles'
Assert-Equal (@($dcsMultiVariantPolicy.twoVariantShape.documentSha256) -join ',') (@($dcsMultiVariantManifest.proven_shape.document_sha256) -join ',') 'DCS schema-template envelope two-variant document hashes'
Assert-Equal (@($dcsMultiVariantPolicy.twoVariantShape.variantNames) -join ',') (@($dcsMultiVariantManifest.proven_shape.variant_names) -join ',') 'DCS schema-template envelope two-variant names'
Assert-Equal $dcsMultiVariantPolicy.twoVariantShape.sourceVariantOrderMatchesExternalSettingsOrder $dcsMultiVariantManifest.proven_shape.source_variant_order_matches_external_settings_order 'DCS schema-template envelope positional-binding claim'
Assert-Equal $dcsMultiVariantPolicy.twoVariantShape.terminalSchemaFileIsEmpty $dcsMultiVariantManifest.proven_shape.terminal_schema_file_is_empty 'DCS schema-template envelope terminal-shape claim'
Assert-Equal (@($dcsMultiVariantPolicy.provenClaims) -join "`n") (@($dcsMultiVariantManifest.proven_claims) -join "`n") 'DCS schema-template envelope proven-claims binding'
Assert-Equal (@($dcsMultiVariantPolicy.nonClaims) -join "`n") (@($dcsMultiVariantManifest.non_claims) -join "`n") 'DCS schema-template envelope non-claims binding'

$dcsOrderRoot = Join-Path $RepositoryRoot 'tests/fixtures/native-evidence/8.3.27.2214/dcs-order'
$dcsOrderManifestPath = Join-Path $dcsOrderRoot 'manifest.json'
if (-not (Test-Path -LiteralPath $dcsOrderManifestPath -PathType Leaf)) {
    throw "DCS order evidence manifest is missing: $dcsOrderManifestPath"
}
$dcsOrderManifest = Get-Content -LiteralPath $dcsOrderManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsOrderManifest.schema_version 1 'DCS order manifest schema version'
Assert-Equal $dcsOrderManifest.issue 283 'DCS order issue'
Assert-Equal $dcsOrderManifest.fixture_id '8.3.27.2214-xml-2.20-dcs-order' 'DCS order fixture ID'
Assert-Equal $dcsOrderManifest.evidence.platform_line '8.3.27' 'DCS order platform line'
Assert-Equal $dcsOrderManifest.evidence.platform_version '8.3.27.2214' 'DCS order exact platform provenance'
Assert-Equal $dcsOrderManifest.evidence.source_version '2.20' 'DCS order source version'
Assert-Equal $dcsOrderManifest.standalone.round_trips 2 'DCS order standalone round-trip count'
foreach ($rowName in @('owner', 'metadata', 'body')) {
    Assert-Equal (@($dcsOrderManifest.form.raw_rows.$rowName.observed_in_rounds) -join ',') 'retained_round_1,fresh_round_2' "DCS order $rowName observed rounds"
}
foreach ($nativeFile in @($dcsOrderManifest.form.native_files)) {
    Assert-Equal (@($nativeFile.observed_in_rounds) -join ',') 'retained_round_1,fresh_round_2' "DCS order $($nativeFile.path) observed rounds"
}
foreach ($fragment in @(
    $dcsOrderManifest.form.minimal_public_fragments.storage_order,
    $dcsOrderManifest.form.minimal_public_fragments.embedded_order,
    $dcsOrderManifest.form.minimal_public_fragments.metadata_only_order
)) {
    $fragmentPath = Resolve-FixturePath $fragment.path $dcsOrderRoot
    if (-not (Test-Path -LiteralPath $fragmentPath -PathType Leaf)) {
        throw "DCS order fragment is missing: $($fragment.path)"
    }
    $encoded = Get-Content -LiteralPath $fragmentPath -Raw -Encoding ASCII
    $decoded = [Convert]::FromBase64String(($encoded -replace '\s', ''))
    Assert-Equal $decoded.Length ([long]$fragment.decoded_size) "$($fragment.path) decoded size"
    Assert-Equal (Get-Sha256Hex $decoded) $fragment.sha256 "$($fragment.path) SHA-256"
}

$dcsOrderPolicyPath = Join-Path $RepositoryRoot 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-order-evidence.json'
if (-not (Test-Path -LiteralPath $dcsOrderPolicyPath -PathType Leaf)) {
    throw "DCS order policy evidence is missing: $dcsOrderPolicyPath"
}
$dcsOrderPolicy = Get-Content -LiteralPath $dcsOrderPolicyPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsOrderPolicy.schemaVersion 1 'DCS order policy schema version'
Assert-Equal $dcsOrderPolicy.contract $dcsOrderManifest.contract 'DCS order policy contract binding'
Assert-Equal $dcsOrderPolicy.source.product '1C:Enterprise Platform' 'DCS order contract product'
Assert-Equal $dcsOrderPolicy.source.release '8.3.27 / XML 2.20' 'DCS order component contract'
Assert-Equal $dcsOrderPolicy.sources.platformLine $dcsOrderManifest.evidence.platform_line 'DCS order policy platform-line binding'
Assert-Equal $dcsOrderPolicy.sources.sourceVersion $dcsOrderManifest.evidence.source_version 'DCS order policy source-version binding'
Assert-Equal $dcsOrderPolicy.sources.ibcmdSha256 $dcsOrderManifest.evidence.ibcmd_sha256 'DCS order policy ibcmd binding'
Assert-Equal $dcsOrderPolicy.sources.standalone.fixtureId $dcsManifest.fixture_id 'DCS order standalone fixture binding'
Assert-Equal $dcsOrderPolicy.sources.standalone.release $dcsOrderManifest.evidence.platform_version 'DCS order standalone release binding'
Assert-Equal $dcsOrderPolicy.sources.standalone.roundTrips $dcsOrderManifest.standalone.round_trips 'DCS order standalone round-trip binding'
Assert-Equal $dcsOrderPolicy.sources.form.fixtureId $dcsOrderManifest.fixture_id 'DCS order Form fixture binding'
Assert-Equal $dcsOrderPolicy.sources.form.release $dcsOrderManifest.evidence.platform_version 'DCS order Form release binding'
Assert-Equal $dcsOrderPolicy.sources.form.roundTrips 2 'DCS order Form round-trip binding'
Assert-Equal $dcsOrderPolicy.sources.standalone.rawBodySha256 $dcsOrderManifest.standalone.unpacked_body_sha256 'DCS order standalone body binding'
Assert-Equal $dcsOrderPolicy.sources.standalone.nativeXmlSha256 $dcsOrderManifest.standalone.native_template_sha256 'DCS order standalone XML binding'
Assert-Equal $dcsOrderPolicy.sources.form.rawBodySha256 $dcsOrderManifest.form.raw_rows.body.unpacked_sha256 'DCS order Form body binding'
$dcsOrderNativeForm = @($dcsOrderManifest.form.native_files | Where-Object { $_.path -like '*/Ext/Form.xml' })
Assert-Equal $dcsOrderNativeForm.Count 1 'DCS order native Form XML count'
Assert-Equal $dcsOrderPolicy.sources.form.nativeXmlSha256 $dcsOrderNativeForm[0].sha256 'DCS order Form XML binding'
Assert-Equal $dcsOrderPolicy.sources.form.storageOrderSha256 $dcsOrderManifest.form.minimal_public_fragments.storage_order.sha256 'DCS order storage fragment binding'
Assert-Equal $dcsOrderPolicy.sources.form.embeddedOrderSha256 $dcsOrderManifest.form.minimal_public_fragments.embedded_order.sha256 'DCS order embedded fragment binding'
Assert-Equal $dcsOrderPolicy.sources.formMetadataOnly.fragmentSha256 $dcsOrderManifest.form.supplemental_metadata_only_observation.metadata_only.sha256 'DCS order metadata-only binding'
Assert-Equal (@($dcsOrderPolicy.policy.supportedOrderTypes) -join ',') 'Asc,Desc' 'DCS order emission directions'
Assert-Equal $dcsOrderPolicy.sources.unicaDesc.repositoryRevision $dcsOrderManifest.form.cross_evidence.unica_desc.revision 'DCS order Unica revision binding'
Assert-Equal (@($dcsOrderPolicy.policy.supportedViewModes) -join ',') 'Normal' 'DCS order emission view modes'
Assert-Equal $dcsOrderPolicy.policy.maxEmittedItems 1 'DCS order emission cardinality'
Assert-Equal $dcsOrderPolicy.policy.storageRecordTypeUuid '11743ff3-2db3-4cfc-9404-90ed8209437f' 'DCS order storage UUID'

$dcsFilterRoot = Join-Path $RepositoryRoot 'tests/fixtures/native-evidence/8.3.27.2214/dcs-filter'
$dcsFilterManifestPath = Join-Path $dcsFilterRoot 'manifest.json'
if (-not (Test-Path -LiteralPath $dcsFilterManifestPath -PathType Leaf)) {
    throw "DCS filter evidence manifest is missing: $dcsFilterManifestPath"
}
$dcsFilterManifest = Get-Content -LiteralPath $dcsFilterManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsFilterManifest.schema_version 1 'DCS filter manifest schema version'
Assert-Equal $dcsFilterManifest.issue 283 'DCS filter issue'
Assert-Equal $dcsFilterManifest.fixture_id '8.3.27.2214-xml-2.20-dcs-filter' 'DCS filter fixture ID'
Assert-Equal $dcsFilterManifest.evidence.platform_line '8.3.27' 'DCS filter platform line'
Assert-Equal $dcsFilterManifest.evidence.platform_version '8.3.27.2214' 'DCS filter platform version'
Assert-Equal $dcsFilterManifest.evidence.source_version '2.20' 'DCS filter source version'
Assert-Equal $dcsFilterManifest.comparison_cohort.selected_native_equal_between_rounds $true 'DCS filter comparison native equality'
Assert-Equal $dcsFilterManifest.metadata_only_cohort.selected_native_equal_between_rounds $true 'DCS filter metadata-only native equality'

foreach ($seed in @($dcsFilterManifest.seed.objects_definition, $dcsFilterManifest.seed.dcs_definition)) {
    $seedPath = Resolve-FixturePath $seed.path $dcsFilterRoot
    Assert-Equal (Get-Item -LiteralPath $seedPath).Length ([long]$seed.size) "$($seed.path) size"
    Assert-Equal (Get-FileSha256Hex $seedPath) $seed.sha256 "$($seed.path) SHA-256"
}
$dcsFilterComparisonCfBytes = Get-Base64EvidenceBytes $dcsFilterManifest.comparison_cohort.configuration_cf $dcsFilterRoot
$dcsFilterMetadataCfBytes = Get-Base64EvidenceBytes $dcsFilterManifest.metadata_only_cohort.configuration_cf $dcsFilterRoot
foreach ($fragment in @(
    $dcsFilterManifest.comparison_cohort.form.native_xml,
    $dcsFilterManifest.comparison_cohort.form.storage_filter,
    $dcsFilterManifest.comparison_cohort.form.embedded_filter,
    $dcsFilterManifest.comparison_cohort.standalone.native_xml,
    $dcsFilterManifest.comparison_cohort.standalone.filter_fragment,
    $dcsFilterManifest.metadata_only_cohort.native_xml,
    $dcsFilterManifest.metadata_only_cohort.embedded_filter
)) {
    $null = Get-Base64EvidenceBytes $fragment $dcsFilterRoot
}

$dcsFilterPolicyPath = Join-Path $RepositoryRoot 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-filter-evidence.json'
if (-not (Test-Path -LiteralPath $dcsFilterPolicyPath -PathType Leaf)) {
    throw "DCS filter policy evidence is missing: $dcsFilterPolicyPath"
}
$dcsFilterPolicy = Get-Content -LiteralPath $dcsFilterPolicyPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsFilterPolicy.schemaVersion 1 'DCS filter policy schema version'
Assert-Equal $dcsFilterPolicy.contract $dcsFilterManifest.contract 'DCS filter policy contract binding'
Assert-Equal $dcsFilterPolicy.source.product '1C:Enterprise Platform' 'DCS filter contract product'
Assert-Equal $dcsFilterPolicy.sources.platformLine $dcsFilterManifest.evidence.platform_line 'DCS filter platform-line binding'
Assert-Equal $dcsFilterPolicy.sources.sourceVersion $dcsFilterManifest.evidence.source_version 'DCS filter source-version binding'
Assert-Equal $dcsFilterPolicy.sources.ibcmdSha256 $dcsFilterManifest.evidence.ibcmd_sha256 'DCS filter ibcmd binding'
Assert-Equal $dcsFilterPolicy.sources.comparison.fixtureId $dcsFilterManifest.fixture_id 'DCS filter comparison fixture binding'
Assert-Equal $dcsFilterPolicy.sources.comparison.release $dcsFilterManifest.evidence.platform_version 'DCS filter comparison release binding'
Assert-Equal $dcsFilterPolicy.sources.comparison.formRawBodySha256 $dcsFilterManifest.comparison_cohort.form.body_row.unpacked_sha256 'DCS filter Form body binding'
Assert-Equal $dcsFilterPolicy.sources.comparison.formNativeXmlSha256 $dcsFilterManifest.comparison_cohort.form.native_xml.sha256 'DCS filter Form XML binding'
Assert-Equal $dcsFilterPolicy.sources.comparison.formStorageFilterSha256 $dcsFilterManifest.comparison_cohort.form.storage_filter.sha256 'DCS filter storage binding'
Assert-Equal $dcsFilterPolicy.sources.comparison.formEmbeddedFilterSha256 $dcsFilterManifest.comparison_cohort.form.embedded_filter.sha256 'DCS filter embedded binding'
Assert-Equal $dcsFilterPolicy.sources.comparison.standaloneRawBodySha256 $dcsFilterManifest.comparison_cohort.standalone.body_row.unpacked_sha256 'DCS filter standalone body binding'
Assert-Equal $dcsFilterPolicy.sources.comparison.standaloneNativeXmlSha256 $dcsFilterManifest.comparison_cohort.standalone.native_xml.sha256 'DCS filter standalone XML binding'
Assert-Equal $dcsFilterPolicy.sources.comparison.standaloneFilterSha256 $dcsFilterManifest.comparison_cohort.standalone.filter_fragment.sha256 'DCS filter standalone fragment binding'
Assert-Equal $dcsFilterPolicy.sources.metadataOnly.formRawBodySha256 $dcsFilterManifest.metadata_only_cohort.form_body_row.unpacked_sha256 'DCS filter metadata-only body binding'
Assert-Equal $dcsFilterPolicy.sources.metadataOnly.formNativeXmlSha256 $dcsFilterManifest.metadata_only_cohort.native_xml.sha256 'DCS filter metadata-only XML binding'
Assert-Equal $dcsFilterPolicy.sources.metadataOnly.formEmbeddedFilterSha256 $dcsFilterManifest.metadata_only_cohort.embedded_filter.sha256 'DCS filter metadata-only fragment binding'
Assert-Equal $dcsFilterPolicy.policy.comparisonStorageRecordTypeUuid $dcsFilterManifest.comparison_cohort.form.storage_record_type_uuid 'DCS filter storage UUID binding'
Assert-Equal $dcsFilterPolicy.policy.metadataOnlyStorageRepresentation 'Filter-property-absent-when-AutoSaveUserSettings-true' 'DCS filter metadata-only storage policy'
Assert-Equal (@($dcsFilterPolicy.policy.supportedComparisonTypes) -join ',') 'Equal' 'DCS filter comparison token'
Assert-Equal (@($dcsFilterPolicy.policy.supportedRightTypes) -join ',') 'string' 'DCS filter right type'
Assert-Equal $dcsFilterPolicy.policy.maxEmittedItems 1 'DCS filter emission cardinality'

$dcsTypeIdRoot = Join-Path $RepositoryRoot 'tests/fixtures/native-evidence/8.3.27.2214/dcs-typeid-reference'
$dcsTypeIdManifest = Get-Content -LiteralPath (Join-Path $dcsTypeIdRoot 'manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsTypeIdManifest.schema_version 1 'DCS TypeId manifest schema version'
Assert-Equal $dcsTypeIdManifest.fixture_id '8.3.27.2214-xml-2.20-dcs-typeid-reference' 'DCS TypeId fixture ID'
Assert-Equal $dcsTypeIdManifest.platform.version '8.3.27.2214' 'DCS TypeId platform version'
Assert-Equal $dcsTypeIdManifest.platform.database_locale 'ru_RU' 'DCS TypeId database locale'
Assert-Equal $dcsTypeIdManifest.rounds.native_template_equal $true 'DCS TypeId native equality'
Assert-Equal $dcsTypeIdManifest.rounds.packed_body_equal $true 'DCS TypeId packed equality'
Assert-Equal $dcsTypeIdManifest.rounds.unpacked_body_equal $true 'DCS TypeId unpacked equality'
foreach ($artifact in @(
    $dcsTypeIdManifest.retained.configuration,
    $dcsTypeIdManifest.retained.native_template,
    $dcsTypeIdManifest.retained.packed_body,
    $dcsTypeIdManifest.retained.unpacked_body
)) {
    $null = Get-Base64EvidenceBytes $artifact $dcsTypeIdRoot
}
Assert-FileEvidence ([pscustomobject]@{ path=$dcsTypeIdManifest.seed.template_path; size=$dcsTypeIdManifest.seed.template_size; sha256=$dcsTypeIdManifest.seed.template_sha256 }) $dcsTypeIdRoot
Assert-FileEvidence ([pscustomobject]@{ path=$dcsTypeIdManifest.seed.objects_path; size=$dcsTypeIdManifest.seed.objects_size; sha256=$dcsTypeIdManifest.seed.objects_sha256 }) $dcsTypeIdRoot
Assert-Equal $dcsTypeIdManifest.mapping.storage_value '488c0ffa-ef24-480c-a420-3bd2736317f9' 'DCS TypeId storage mapping'
Assert-Equal $dcsTypeIdManifest.mapping.source_value 'CatalogRef.FilterProbe' 'DCS TypeId source mapping'
Assert-Equal $dcsTypeIdManifest.compiler_acceptance.candidate_cf_sha256 '485b71e5e42622a563c0551241b464b219e8c939b2dd285b4babfc231e28d377' 'DCS TypeId compiler candidate'
Assert-Equal $dcsTypeIdManifest.compiler_acceptance.compiled_body_sha256 '099c803efaa22422914bb32f7a8214cf1bd4c2c7b2c73ec155b8b6f73f173f45' 'DCS TypeId compiled body'
Assert-Equal $dcsTypeIdManifest.compiler_acceptance.reexported_native_template_sha256 $dcsTypeIdManifest.rounds.native_template_sha256 'DCS TypeId compiler re-export equality'
Assert-Equal $dcsTypeIdManifest.compiler_acceptance.semantic_token_retained $dcsTypeIdManifest.mapping.source_value 'DCS TypeId compiler semantic retention'

$dcsInnerPolicyPath = Join-Path $RepositoryRoot 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-inner-schema-evidence.json'
$dcsInnerPolicy = Get-Content -LiteralPath $dcsInnerPolicyPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsInnerPolicy.sources.typeIdReference.fixtureId $dcsTypeIdManifest.fixture_id 'DCS inner policy TypeId fixture binding'
Assert-Equal $dcsInnerPolicy.sources.typeIdReference.nativeXmlSha256 $dcsTypeIdManifest.rounds.native_template_sha256 'DCS inner policy TypeId native binding'
Assert-Equal $dcsInnerPolicy.sources.typeIdReference.packedBodySha256 $dcsTypeIdManifest.rounds.packed_body_sha256 'DCS inner policy TypeId packed binding'
Assert-Equal $dcsInnerPolicy.sources.typeIdReference.unpackedBodySha256 $dcsTypeIdManifest.rounds.unpacked_body_sha256 'DCS inner policy TypeId unpacked binding'
Assert-Equal $dcsInnerPolicy.policy.referenceStorageTypeId $dcsTypeIdManifest.mapping.storage_value 'DCS inner policy TypeId value binding'
Assert-Equal $dcsInnerPolicy.policy.referenceSourceQualifiedName $dcsTypeIdManifest.mapping.source_value 'DCS inner policy reference QName binding'

$dcsConditionalRoot = Join-Path $RepositoryRoot 'tests/fixtures/native-evidence/8.3.27.2214/dcs-conditional-appearance'
$dcsConditionalManifestPath = Join-Path $dcsConditionalRoot 'manifest.json'
if (-not (Test-Path -LiteralPath $dcsConditionalManifestPath -PathType Leaf)) {
    throw "DCS conditional-appearance evidence manifest is missing: $dcsConditionalManifestPath"
}
$dcsConditionalManifest = Get-Content -LiteralPath $dcsConditionalManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsConditionalManifest.schema_version 1 'DCS conditional-appearance manifest schema version'
Assert-Equal $dcsConditionalManifest.issue 283 'DCS conditional-appearance issue'
Assert-Equal $dcsConditionalManifest.fixture_id '8.3.27.2214-xml-2.20-dcs-conditional-appearance' 'DCS conditional-appearance fixture ID'
Assert-Equal $dcsConditionalManifest.evidence.platform_line '8.3.27' 'DCS conditional-appearance platform line'
Assert-Equal $dcsConditionalManifest.evidence.platform_version '8.3.27.2214' 'DCS conditional-appearance platform version'
Assert-Equal $dcsConditionalManifest.evidence.source_version '2.20' 'DCS conditional-appearance source version'
Assert-Equal $dcsConditionalManifest.rounds.selected_native_equal_between_rounds $true 'DCS conditional-appearance native equality'
Assert-Equal $dcsConditionalManifest.rounds.packed_rows_equal_between_rounds $true 'DCS conditional-appearance packed-row equality'
Assert-Equal $dcsConditionalManifest.rounds.unpacked_rows_equal_between_rounds $true 'DCS conditional-appearance unpacked-row equality'
foreach ($seed in @($dcsConditionalManifest.seed.objects_definition, $dcsConditionalManifest.seed.dcs_definition)) {
    $seedPath = Resolve-FixturePath $seed.path $dcsConditionalRoot
    Assert-Equal (Get-Item -LiteralPath $seedPath).Length ([long]$seed.size) "$($seed.path) size"
    Assert-Equal (Get-FileSha256Hex $seedPath) $seed.sha256 "$($seed.path) SHA-256"
}
$dcsConditionalCfBytes = Get-Base64EvidenceBytes $dcsConditionalManifest.configuration_cf $dcsConditionalRoot
foreach ($fragment in @(
    $dcsConditionalManifest.form.native_xml,
    $dcsConditionalManifest.form.embedded_conditional_appearance,
    $dcsConditionalManifest.form.storage_conditional_appearance,
    $dcsConditionalManifest.standalone.native_xml,
    $dcsConditionalManifest.standalone.conditional_appearance
)) {
    $null = Get-Base64EvidenceBytes $fragment $dcsConditionalRoot
}

$dcsConditionalPolicyPath = Join-Path $RepositoryRoot 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-conditional-appearance-evidence.json'
if (-not (Test-Path -LiteralPath $dcsConditionalPolicyPath -PathType Leaf)) {
    throw "DCS conditional-appearance policy evidence is missing: $dcsConditionalPolicyPath"
}
$dcsConditionalPolicy = Get-Content -LiteralPath $dcsConditionalPolicyPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsConditionalPolicy.schemaVersion 1 'DCS conditional-appearance policy schema version'
Assert-Equal $dcsConditionalPolicy.contract $dcsConditionalManifest.contract 'DCS conditional-appearance contract binding'
Assert-Equal $dcsConditionalPolicy.source.product '1C:Enterprise Platform' 'DCS conditional-appearance contract product'
Assert-Equal $dcsConditionalPolicy.sources.platformLine $dcsConditionalManifest.evidence.platform_line 'DCS conditional-appearance platform-line binding'
Assert-Equal $dcsConditionalPolicy.sources.sourceVersion $dcsConditionalManifest.evidence.source_version 'DCS conditional-appearance source-version binding'
Assert-Equal $dcsConditionalPolicy.sources.ibcmdSha256 $dcsConditionalManifest.evidence.ibcmd_sha256 'DCS conditional-appearance ibcmd binding'
Assert-Equal $dcsConditionalPolicy.sources.comparison.fixtureId $dcsConditionalManifest.fixture_id 'DCS conditional-appearance fixture binding'
Assert-Equal $dcsConditionalPolicy.sources.comparison.roundTrips 2 'DCS conditional-appearance round-trip binding'
Assert-Equal $dcsConditionalPolicy.sources.comparison.formRawBodySha256 $dcsConditionalManifest.form.body_row.unpacked_sha256 'DCS conditional-appearance Form body binding'
Assert-Equal $dcsConditionalPolicy.sources.comparison.formNativeXmlSha256 $dcsConditionalManifest.form.native_xml.sha256 'DCS conditional-appearance Form XML binding'
Assert-Equal $dcsConditionalPolicy.sources.comparison.formStorageSha256 $dcsConditionalManifest.form.storage_conditional_appearance.sha256 'DCS conditional-appearance storage binding'
Assert-Equal $dcsConditionalPolicy.sources.comparison.formEmbeddedSha256 $dcsConditionalManifest.form.embedded_conditional_appearance.sha256 'DCS conditional-appearance embedded binding'
Assert-Equal $dcsConditionalPolicy.sources.comparison.standaloneRawBodySha256 $dcsConditionalManifest.standalone.body_row.unpacked_sha256 'DCS conditional-appearance standalone body binding'
Assert-Equal $dcsConditionalPolicy.sources.comparison.standaloneNativeXmlSha256 $dcsConditionalManifest.standalone.native_xml.sha256 'DCS conditional-appearance standalone XML binding'
Assert-Equal $dcsConditionalPolicy.sources.comparison.standaloneFragmentSha256 $dcsConditionalManifest.standalone.conditional_appearance.sha256 'DCS conditional-appearance standalone fragment binding'
Assert-Equal $dcsConditionalPolicy.sources.metadataOnly.formEmbeddedSha256 $dcsConditionalManifest.form.metadata_only_baseline.embedded_fragment_sha256 'DCS conditional-appearance metadata-only binding'
Assert-Equal $dcsConditionalPolicy.policy.storagePropertyName $dcsConditionalManifest.form.storage_property_name 'DCS conditional-appearance storage property binding'
Assert-Equal $dcsConditionalPolicy.policy.storageRecordTypeUuid $dcsConditionalManifest.form.storage_record_type_uuid 'DCS conditional-appearance storage UUID binding'
Assert-Equal (@($dcsConditionalPolicy.policy.supportedParameters) -join ',') 'TextColor' 'DCS conditional-appearance parameter cohort'
Assert-Equal (@($dcsConditionalPolicy.policy.supportedValues) -join ',') 'WebRed' 'DCS conditional-appearance value cohort'
Assert-Equal $dcsConditionalPolicy.policy.maxEmittedItems 1 'DCS conditional-appearance emission cardinality'

$dcsFormAttributesRoot = Join-Path $RepositoryRoot 'tests/fixtures/native-evidence/8.3.27.2214/dcs-form-attributes-conditional-appearance'
$dcsFormAttributesManifestPath = Join-Path $dcsFormAttributesRoot 'manifest.json'
if (-not (Test-Path -LiteralPath $dcsFormAttributesManifestPath -PathType Leaf)) {
    throw "DCS Form Attributes conditional-appearance manifest is missing: $dcsFormAttributesManifestPath"
}
$dcsFormAttributesManifest = Get-Content -LiteralPath $dcsFormAttributesManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsFormAttributesManifest.schema_version 1 'DCS Form Attributes manifest schema version'
Assert-Equal $dcsFormAttributesManifest.issue 283 'DCS Form Attributes issue'
Assert-Equal $dcsFormAttributesManifest.fixture_id '8.3.27.2214-xml-2.20-dcs-form-attributes-conditional-appearance' 'DCS Form Attributes fixture ID'
Assert-Equal $dcsFormAttributesManifest.evidence.platform_version '8.3.27.2214' 'DCS Form Attributes platform version'
Assert-Equal $dcsFormAttributesManifest.evidence.source_version '2.20' 'DCS Form Attributes source version'
Assert-Equal $dcsFormAttributesManifest.evidence.database_locale 'ru_RU' 'DCS Form Attributes database locale'
Assert-Equal $dcsFormAttributesManifest.rounds.selected_native_equal_between_rounds $true 'DCS Form Attributes selected-native equality'
Assert-Equal $dcsFormAttributesManifest.rounds.packed_rows_equal_between_rounds $true 'DCS Form Attributes packed-row equality'
Assert-Equal $dcsFormAttributesManifest.rounds.unpacked_rows_equal_between_rounds $true 'DCS Form Attributes unpacked-row equality'
$dcsFormAttributesCfBytes = Get-Base64EvidenceBytes $dcsFormAttributesManifest.configuration_cf $dcsFormAttributesRoot
foreach ($fragment in @(
    $dcsFormAttributesManifest.seed.probe_form,
    $dcsFormAttributesManifest.form.native_xml,
    $dcsFormAttributesManifest.form.wrapper,
    $dcsFormAttributesManifest.form.storage_settings,
    $dcsFormAttributesManifest.form.absent_baseline.empty_storage_settings
)) {
    $null = Get-Base64EvidenceBytes $fragment $dcsFormAttributesRoot
}
$dcsFormAttributesRulePath = Resolve-FixturePath $dcsFormAttributesManifest.seed.rule.path $dcsFormAttributesRoot
Assert-Equal (Get-Item -LiteralPath $dcsFormAttributesRulePath).Length ([long]$dcsFormAttributesManifest.seed.rule.size) 'DCS Form Attributes seed rule size'
Assert-Equal (Get-FileSha256Hex $dcsFormAttributesRulePath) $dcsFormAttributesManifest.seed.rule.sha256 'DCS Form Attributes seed rule SHA-256'

$dcsFormAttributesPolicyPath = Join-Path $RepositoryRoot 'crates/ibcmd-schema/data/platform-8.3.27-xml-2.20-dcs-form-attributes-conditional-appearance-evidence.json'
if (-not (Test-Path -LiteralPath $dcsFormAttributesPolicyPath -PathType Leaf)) {
    throw "DCS Form Attributes conditional-appearance policy is missing: $dcsFormAttributesPolicyPath"
}
$dcsFormAttributesPolicy = Get-Content -LiteralPath $dcsFormAttributesPolicyPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-Equal $dcsFormAttributesPolicy.schemaVersion 1 'DCS Form Attributes policy schema version'
Assert-Equal $dcsFormAttributesPolicy.contract $dcsFormAttributesManifest.contract 'DCS Form Attributes policy contract binding'
Assert-Equal $dcsFormAttributesPolicy.bodyContract $dcsFormAttributesManifest.body_contract 'DCS Form Attributes body contract binding'
Assert-Equal $dcsFormAttributesPolicy.sources.comparison.fixtureId $dcsFormAttributesManifest.fixture_id 'DCS Form Attributes fixture binding'
Assert-Equal $dcsFormAttributesPolicy.sources.comparison.release $dcsFormAttributesManifest.evidence.platform_version 'DCS Form Attributes release binding'
Assert-Equal $dcsFormAttributesPolicy.sources.comparison.databaseLocale $dcsFormAttributesManifest.evidence.database_locale 'DCS Form Attributes locale binding'
Assert-Equal $dcsFormAttributesPolicy.sources.comparison.formRawBodySha256 $dcsFormAttributesManifest.form.body_row.unpacked_sha256 'DCS Form Attributes raw-body binding'
Assert-Equal $dcsFormAttributesPolicy.sources.comparison.formNativeXmlSha256 $dcsFormAttributesManifest.form.native_xml.sha256 'DCS Form Attributes native-XML binding'
Assert-Equal $dcsFormAttributesPolicy.sources.comparison.wrapperSha256 $dcsFormAttributesManifest.form.wrapper.sha256 'DCS Form Attributes wrapper binding'
Assert-Equal $dcsFormAttributesPolicy.sources.comparison.storageSettingsSha256 $dcsFormAttributesManifest.form.storage_settings.sha256 'DCS Form Attributes storage binding'
Assert-Equal $dcsFormAttributesPolicy.sources.absent.fixtureId $dcsFormAttributesManifest.form.absent_baseline.fixture_id 'DCS Form Attributes absent fixture binding'
Assert-Equal $dcsFormAttributesPolicy.sources.absent.formRawBodySha256 $dcsFormAttributesManifest.form.absent_baseline.body_unpacked_sha256 'DCS Form Attributes absent body binding'
Assert-Equal $dcsFormAttributesPolicy.sources.absent.formNativeXmlSha256 $dcsFormAttributesManifest.form.absent_baseline.native_form_sha256 'DCS Form Attributes absent XML binding'
Assert-Equal $dcsFormAttributesPolicy.policy.storageRecordTypeUuid $null 'DCS Form Attributes storage UUID absence'
Assert-Equal $dcsFormAttributesPolicy.policy.storageEnvelope $dcsFormAttributesManifest.proven_shape.storage_envelope 'DCS Form Attributes storage-envelope binding'
Assert-Equal $dcsFormAttributesPolicy.policy.storageContainerMarker $dcsFormAttributesManifest.form.storage_outer_tail.container_marker 'DCS Form Attributes container marker'
Assert-Equal $dcsFormAttributesPolicy.policy.storageAbsentContainerMarker $dcsFormAttributesManifest.form.storage_outer_tail.absent_container_marker 'DCS Form Attributes absent container marker'
Assert-Equal (@($dcsFormAttributesPolicy.policy.storageInactiveMarker) -join ',') '0,0' 'DCS Form Attributes inactive marker'
Assert-Equal (@($dcsFormAttributesPolicy.policy.storageActiveMarker) -join ',') '0,1' 'DCS Form Attributes active marker'
Assert-Equal (@($dcsFormAttributesPolicy.policy.storageFieldOrder) -join ',') 'selection,filter' 'DCS Form Attributes descriptor field order'
Assert-Equal (@($dcsFormAttributesPolicy.policy.storageSelectionTypeIndexes) -join ',') '26,9' 'DCS Form Attributes selection type indexes'
Assert-Equal (@($dcsFormAttributesPolicy.policy.storageFilterTypeIndexes) -join ',') '26' 'DCS Form Attributes filter type indexes'
Assert-Equal $dcsFormAttributesPolicy.policy.absenceRepresentation 'wrapper-absent-empty-settings-tail-present' 'DCS Form Attributes absence representation'
Assert-Equal (@($dcsFormAttributesManifest.form.storage_outer_tail.active_marker) -join ',') '0,1' 'DCS Form Attributes manifest active marker'
Assert-Equal (@($dcsFormAttributesManifest.form.storage_outer_tail.field_order) -join ',') 'selection,filter' 'DCS Form Attributes manifest field order'
Assert-Equal (@($dcsFormAttributesManifest.form.storage_outer_tail.selection_type_indexes) -join ',') '26,9' 'DCS Form Attributes manifest selection type indexes'
Assert-Equal (@($dcsFormAttributesManifest.form.storage_outer_tail.filter_type_indexes) -join ',') '26' 'DCS Form Attributes manifest filter type indexes'
Assert-Equal $dcsFormAttributesPolicy.policy.maxEmittedItems 1 'DCS Form Attributes emission cardinality'

if (-not [IO.Path]::IsPathRooted($BinaryPath)) {
    $BinaryPath = Join-Path $RepositoryRoot $BinaryPath
}
if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
    throw "ibcmd-rs binary is missing: $BinaryPath"
}

$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("ibcmd-rs-native-evidence-" + [Guid]::NewGuid().ToString('N'))
$cfPath = Join-Path $temporaryRoot 'configuration.cf'
$outputRoot = Join-Path $temporaryRoot 'export'
$dcsCfPath = Join-Path $temporaryRoot 'dcs-configuration.cf'
$dcsOutputRoot = Join-Path $temporaryRoot 'dcs-export'
$dcsDataParametersCfPath = Join-Path $temporaryRoot 'dcs-data-parameters-configuration.cf'
$dcsDataParametersOutputRoot = Join-Path $temporaryRoot 'dcs-data-parameters-export'
$dcsMultiVariantCfPath = Join-Path $temporaryRoot 'dcs-multi-variant-configuration.cf'
$dcsMultiVariantOutputRoot = Join-Path $temporaryRoot 'dcs-multi-variant-export'
$dcsFilterComparisonCfPath = Join-Path $temporaryRoot 'dcs-filter-comparison.cf'
$dcsFilterComparisonOutputRoot = Join-Path $temporaryRoot 'dcs-filter-comparison-export'
$dcsFilterMetadataCfPath = Join-Path $temporaryRoot 'dcs-filter-metadata-only.cf'
$dcsFilterMetadataOutputRoot = Join-Path $temporaryRoot 'dcs-filter-metadata-only-export'
$dcsConditionalCfPath = Join-Path $temporaryRoot 'dcs-conditional-appearance.cf'
$dcsConditionalOutputRoot = Join-Path $temporaryRoot 'dcs-conditional-appearance-export'
$dcsFormAttributesCfPath = Join-Path $temporaryRoot 'dcs-form-attributes-conditional-appearance.cf'
$dcsFormAttributesOutputRoot = Join-Path $temporaryRoot 'dcs-form-attributes-conditional-appearance-export'
$dcsFormAttributesCandidateCfPath = Join-Path $temporaryRoot 'dcs-form-attributes-compiler-candidate.cf'
$dcsFormAttributesCandidateFormPath = Join-Path $temporaryRoot 'dcs-form-attributes-compiler-input.xml'
$dcsFormAttributesCandidateOutputRoot = Join-Path $temporaryRoot 'dcs-form-attributes-compiler-export'
$stderrPath = Join-Path $temporaryRoot 'stderr.txt'
[IO.Directory]::CreateDirectory($temporaryRoot) | Out-Null

try {
    [IO.File]::WriteAllBytes($cfPath, $cfBytes)
    $stdout = & $BinaryPath cf export --source-version 2.20 $cfPath $outputRoot 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        $stderr = if (Test-Path -LiteralPath $stderrPath) {
            Get-Content -LiteralPath $stderrPath -Raw
        } else {
            ''
        }
        throw "CF export failed with exit code $LASTEXITCODE. $stderr"
    }

    $report = ($stdout -join [Environment]::NewLine) | ConvertFrom-Json
    Assert-Equal $report.ok $true 'CF export status'
    Assert-Equal $report.export.storage.failed 0 'failed storage entries'

    foreach ($object in @($manifest.objects)) {
        foreach ($expected in @($object.native_outputs)) {
            $candidateRelativePath = $expected.path -replace '^native/', ''
            $candidatePath = Join-Path $outputRoot ($candidateRelativePath.Replace('/', [IO.Path]::DirectorySeparatorChar))
            if (-not (Test-Path -LiteralPath $candidatePath -PathType Leaf)) {
                throw "Exported evidence output is missing: $candidateRelativePath"
            }
            $candidateItem = Get-Item -LiteralPath $candidatePath
            Assert-Equal $candidateItem.Length ([long]$expected.size) "$candidateRelativePath exported size"
            Assert-Equal (Get-FileSha256Hex $candidatePath) $expected.sha256 "$candidateRelativePath exported SHA-256"
        }
    }

    [IO.File]::WriteAllBytes($dcsCfPath, $dcsCfBytes)
    $dcsStdout = & $BinaryPath cf export --source-version 2.20 $dcsCfPath $dcsOutputRoot 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        $stderr = if (Test-Path -LiteralPath $stderrPath) {
            Get-Content -LiteralPath $stderrPath -Raw
        } else {
            ''
        }
        throw "DCS CF export failed with exit code $LASTEXITCODE. $stderr"
    }
    $dcsReport = ($dcsStdout -join [Environment]::NewLine) | ConvertFrom-Json
    Assert-Equal $dcsReport.ok $true 'DCS CF export status'
    Assert-Equal $dcsReport.export.storage.failed 0 'failed DCS storage entries'
    $dcsCandidateRelativePath = $dcsManifest.template.native_xml.path -replace '^native/', ''
    $dcsCandidatePath = Join-Path $dcsOutputRoot ($dcsCandidateRelativePath.Replace('/', [IO.Path]::DirectorySeparatorChar))
    if (-not (Test-Path -LiteralPath $dcsCandidatePath -PathType Leaf)) {
        throw "Exported DCS evidence output is missing: $dcsCandidateRelativePath"
    }
    Assert-Equal (Get-Item -LiteralPath $dcsCandidatePath).Length ([long]$dcsManifest.template.native_xml.size) "$dcsCandidateRelativePath exported size"
    Assert-Equal (Get-FileSha256Hex $dcsCandidatePath) $dcsManifest.template.native_xml.sha256 "$dcsCandidateRelativePath exported SHA-256"

    [IO.File]::WriteAllBytes($dcsDataParametersCfPath, $dcsDataParametersCfBytes)
    $dcsDataParametersStdout = & $BinaryPath cf export --source-version 2.20 $dcsDataParametersCfPath $dcsDataParametersOutputRoot 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        $stderr = if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath -Raw } else { '' }
        throw "DCS dataParameters CF export failed with exit code $LASTEXITCODE. $stderr"
    }
    $dcsDataParametersReport = ($dcsDataParametersStdout -join [Environment]::NewLine) | ConvertFrom-Json
    Assert-Equal $dcsDataParametersReport.ok $true 'DCS dataParameters CF export status'
    Assert-Equal $dcsDataParametersReport.export.storage.failed 0 'DCS dataParameters failed storage entries'
    $dcsDataParametersCandidatePath = Join-Path $dcsDataParametersOutputRoot ($dcsDataParametersManifest.template.native_xml.export_path.Replace('/', [IO.Path]::DirectorySeparatorChar))
    if (-not (Test-Path -LiteralPath $dcsDataParametersCandidatePath -PathType Leaf)) {
        throw "Exported DCS dataParameters Template is missing: $($dcsDataParametersManifest.template.native_xml.export_path)"
    }
    Assert-Equal (Get-Item -LiteralPath $dcsDataParametersCandidatePath).Length ([long]$dcsDataParametersManifest.template.native_xml.decoded_size) 'DCS dataParameters exported Template size'
    Assert-Equal (Get-FileSha256Hex $dcsDataParametersCandidatePath) $dcsDataParametersManifest.template.native_xml.decoded_sha256 'DCS dataParameters exported Template SHA-256'
    $dcsDataParametersExportedFragment = Get-Utf8XmlFragmentBytes $dcsDataParametersCandidatePath '<dcsset:dataParameters>' '</dcsset:dataParameters>'
    Assert-Equal $dcsDataParametersExportedFragment.Length ([long]$dcsDataParametersManifest.template.data_parameters_fragment.decoded_size) 'DCS dataParameters exported fragment size'
    Assert-Equal (Get-Sha256Hex $dcsDataParametersExportedFragment) $dcsDataParametersManifest.template.data_parameters_fragment.sha256 'DCS dataParameters exported fragment SHA-256'
    $verifyDcsDataParameters = & $BinaryPath cf verify $dcsDataParametersCfPath --compression raw-deflate `
        --element $dcsDataParametersManifest.template.body_key `
        --expect-sha256 "$($dcsDataParametersManifest.template.body_key)=$($dcsDataParametersManifest.template.raw_entry.unpacked.decoded_sha256)" 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        throw "DCS dataParameters raw-row verification failed: $((Get-Content -LiteralPath $stderrPath -Raw))"
    }
    Assert-Equal ((($verifyDcsDataParameters -join [Environment]::NewLine) | ConvertFrom-Json).ok) $true 'DCS dataParameters raw row'

    [IO.File]::WriteAllBytes($dcsMultiVariantCfPath, $dcsMultiVariantCfBytes)
    $dcsMultiVariantStdout = & $BinaryPath cf export --source-version 2.20 $dcsMultiVariantCfPath $dcsMultiVariantOutputRoot 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        $stderr = if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath -Raw } else { '' }
        throw "DCS multi-variant CF export failed with exit code $LASTEXITCODE. $stderr"
    }
    $dcsMultiVariantReport = ($dcsMultiVariantStdout -join [Environment]::NewLine) | ConvertFrom-Json
    Assert-Equal $dcsMultiVariantReport.ok $true 'DCS multi-variant CF export status'
    Assert-Equal $dcsMultiVariantReport.export.storage.failed 0 'DCS multi-variant failed storage entries'
    $dcsMultiVariantCandidatePath = Join-Path $dcsMultiVariantOutputRoot ($dcsMultiVariantManifest.template.native_xml.export_path.Replace('/', [IO.Path]::DirectorySeparatorChar))
    if (-not (Test-Path -LiteralPath $dcsMultiVariantCandidatePath -PathType Leaf)) {
        throw "Exported DCS multi-variant Template is missing: $($dcsMultiVariantManifest.template.native_xml.export_path)"
    }
    Assert-Equal (Get-Item -LiteralPath $dcsMultiVariantCandidatePath).Length ([long]$dcsMultiVariantManifest.template.native_xml.decoded_size) 'DCS multi-variant exported Template size'
    Assert-Equal (Get-FileSha256Hex $dcsMultiVariantCandidatePath) $dcsMultiVariantManifest.template.native_xml.decoded_sha256 'DCS multi-variant exported Template SHA-256'
    $verifyDcsMultiVariant = & $BinaryPath cf verify $dcsMultiVariantCfPath --compression raw-deflate `
        --element $dcsMultiVariantManifest.template.body_key `
        --expect-sha256 "$($dcsMultiVariantManifest.template.body_key)=$($dcsMultiVariantManifest.template.raw_entry.unpacked.decoded_sha256)" 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        throw "DCS multi-variant raw-row verification failed: $((Get-Content -LiteralPath $stderrPath -Raw))"
    }
    Assert-Equal ((($verifyDcsMultiVariant -join [Environment]::NewLine) | ConvertFrom-Json).ok) $true 'DCS multi-variant raw row'

    [IO.File]::WriteAllBytes($dcsFilterComparisonCfPath, $dcsFilterComparisonCfBytes)
    $dcsFilterComparisonStdout = & $BinaryPath cf export --source-version 2.20 $dcsFilterComparisonCfPath $dcsFilterComparisonOutputRoot 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        $stderr = if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath -Raw } else { '' }
        throw "DCS filter comparison CF export failed with exit code $LASTEXITCODE. $stderr"
    }
    $dcsFilterComparisonReport = ($dcsFilterComparisonStdout -join [Environment]::NewLine) | ConvertFrom-Json
    Assert-Equal $dcsFilterComparisonReport.ok $true 'DCS filter comparison export status'
    Assert-Equal $dcsFilterComparisonReport.export.storage.failed 0 'DCS filter comparison failed storage entries'
    $comparisonFormPath = Join-Path $dcsFilterComparisonOutputRoot 'Catalogs\FilterProbe\Forms\ListForm\Ext\Form.xml'
    $comparisonFilterBytes = Get-Utf8XmlFragmentBytes $comparisonFormPath '<dcsset:filter>' '</dcsset:filter>'
    Assert-Equal $comparisonFilterBytes.Length ([long]$dcsFilterManifest.comparison_cohort.form.embedded_filter.decoded_size) 'DCS filter comparison embedded fragment exported size'
    Assert-Equal (Get-Sha256Hex $comparisonFilterBytes) $dcsFilterManifest.comparison_cohort.form.embedded_filter.sha256 'DCS filter comparison embedded fragment exported SHA-256'

    $comparisonTemplatePath = Join-Path $dcsFilterComparisonOutputRoot 'Reports\FilterProbeReport\Templates\MainSchema\Ext\Template.xml'
    Assert-Equal (Get-Item -LiteralPath $comparisonTemplatePath).Length ([long]$dcsFilterManifest.comparison_cohort.standalone.native_xml.decoded_size) 'DCS filter standalone Template exported size'
    Assert-Equal (Get-FileSha256Hex $comparisonTemplatePath) $dcsFilterManifest.comparison_cohort.standalone.native_xml.sha256 'DCS filter standalone Template exported SHA-256'
    $verifyComparison = & $BinaryPath cf verify $dcsFilterComparisonCfPath --compression raw-deflate `
        --element $dcsFilterManifest.comparison_cohort.form.body_key `
        --element $dcsFilterManifest.comparison_cohort.standalone.body_key `
        --expect-sha256 "$($dcsFilterManifest.comparison_cohort.form.body_key)=$($dcsFilterManifest.comparison_cohort.form.body_row.unpacked_sha256)" `
        --expect-sha256 "$($dcsFilterManifest.comparison_cohort.standalone.body_key)=$($dcsFilterManifest.comparison_cohort.standalone.body_row.unpacked_sha256)" 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        throw "DCS filter comparison raw-row verification failed: $((Get-Content -LiteralPath $stderrPath -Raw))"
    }
    Assert-Equal ((($verifyComparison -join [Environment]::NewLine) | ConvertFrom-Json).ok) $true 'DCS filter comparison raw rows'

    [IO.File]::WriteAllBytes($dcsFilterMetadataCfPath, $dcsFilterMetadataCfBytes)
    $dcsFilterMetadataStdout = & $BinaryPath cf export --source-version 2.20 $dcsFilterMetadataCfPath $dcsFilterMetadataOutputRoot 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        $stderr = if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath -Raw } else { '' }
        throw "DCS filter metadata-only CF export failed with exit code $LASTEXITCODE. $stderr"
    }
    $dcsFilterMetadataReport = ($dcsFilterMetadataStdout -join [Environment]::NewLine) | ConvertFrom-Json
    Assert-Equal $dcsFilterMetadataReport.ok $true 'DCS filter metadata-only export status'
    Assert-Equal $dcsFilterMetadataReport.export.storage.failed 0 'DCS filter metadata-only failed storage entries'
    $metadataCandidatePath = Join-Path $dcsFilterMetadataOutputRoot 'Catalogs\FilterProbe\Forms\ListForm\Ext\Form.xml'
    $metadataFilterBytes = Get-Utf8XmlFragmentBytes $metadataCandidatePath '<dcsset:filter>' '</dcsset:filter>'
    Assert-Equal $metadataFilterBytes.Length ([long]$dcsFilterManifest.metadata_only_cohort.embedded_filter.decoded_size) 'DCS filter metadata-only embedded fragment exported size'
    Assert-Equal (Get-Sha256Hex $metadataFilterBytes) $dcsFilterManifest.metadata_only_cohort.embedded_filter.sha256 'DCS filter metadata-only embedded fragment exported SHA-256'
    $verifyMetadata = & $BinaryPath cf verify $dcsFilterMetadataCfPath --compression raw-deflate `
        --element $dcsFilterManifest.comparison_cohort.form.body_key `
        --expect-sha256 "$($dcsFilterManifest.comparison_cohort.form.body_key)=$($dcsFilterManifest.metadata_only_cohort.form_body_row.unpacked_sha256)" 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        throw "DCS filter metadata-only raw-row verification failed: $((Get-Content -LiteralPath $stderrPath -Raw))"
    }
    Assert-Equal ((($verifyMetadata -join [Environment]::NewLine) | ConvertFrom-Json).ok) $true 'DCS filter metadata-only raw row'

    [IO.File]::WriteAllBytes($dcsConditionalCfPath, $dcsConditionalCfBytes)
    $dcsConditionalStdout = & $BinaryPath cf export --source-version 2.20 $dcsConditionalCfPath $dcsConditionalOutputRoot 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        $stderr = if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath -Raw } else { '' }
        throw "DCS conditional-appearance CF export failed with exit code $LASTEXITCODE. $stderr"
    }
    $dcsConditionalReport = ($dcsConditionalStdout -join [Environment]::NewLine) | ConvertFrom-Json
    Assert-Equal $dcsConditionalReport.ok $true 'DCS conditional-appearance export status'
    Assert-Equal $dcsConditionalReport.export.storage.failed 0 'DCS conditional-appearance failed storage entries'
    foreach ($expected in @($dcsConditionalManifest.selected_native_files)) {
        if ($expected.path -eq 'Catalogs/FilterProbe/Forms/ListForm/Ext/Form.xml') {
            continue
        }
        $candidatePath = Join-Path $dcsConditionalOutputRoot ($expected.path.Replace('/', [IO.Path]::DirectorySeparatorChar))
        if (-not (Test-Path -LiteralPath $candidatePath -PathType Leaf)) {
            throw "Exported DCS conditional-appearance output is missing: $($expected.path)"
        }
        Assert-Equal (Get-Item -LiteralPath $candidatePath).Length ([long]$expected.size) "$($expected.path) exported size"
        Assert-Equal (Get-FileSha256Hex $candidatePath) $expected.sha256 "$($expected.path) exported SHA-256"
    }
    $conditionalFormPath = Join-Path $dcsConditionalOutputRoot 'Catalogs\FilterProbe\Forms\ListForm\Ext\Form.xml'
    $conditionalEmbeddedBytes = Get-Utf8XmlFragmentBytes $conditionalFormPath '<dcsset:conditionalAppearance>' '</dcsset:conditionalAppearance>'
    Assert-Equal $conditionalEmbeddedBytes.Length ([long]$dcsConditionalManifest.form.embedded_conditional_appearance.decoded_size) 'DCS conditional-appearance embedded fragment exported size'
    Assert-Equal (Get-Sha256Hex $conditionalEmbeddedBytes) $dcsConditionalManifest.form.embedded_conditional_appearance.sha256 'DCS conditional-appearance embedded fragment exported SHA-256'
    $conditionalTemplatePath = Join-Path $dcsConditionalOutputRoot 'Reports\FilterProbeReport\Templates\MainSchema\Ext\Template.xml'
    $conditionalStandaloneBytes = Get-Utf8XmlFragmentBytes $conditionalTemplatePath '<dcsset:conditionalAppearance>' '</dcsset:conditionalAppearance>'
    Assert-Equal $conditionalStandaloneBytes.Length ([long]$dcsConditionalManifest.standalone.conditional_appearance.decoded_size) 'DCS conditional-appearance standalone fragment exported size'
    Assert-Equal (Get-Sha256Hex $conditionalStandaloneBytes) $dcsConditionalManifest.standalone.conditional_appearance.sha256 'DCS conditional-appearance standalone fragment exported SHA-256'
    $verifyConditional = & $BinaryPath cf verify $dcsConditionalCfPath --compression raw-deflate `
        --element $dcsConditionalManifest.form.body_key `
        --element $dcsConditionalManifest.standalone.body_key `
        --expect-sha256 "$($dcsConditionalManifest.form.body_key)=$($dcsConditionalManifest.form.body_row.unpacked_sha256)" `
        --expect-sha256 "$($dcsConditionalManifest.standalone.body_key)=$($dcsConditionalManifest.standalone.body_row.unpacked_sha256)" 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        throw "DCS conditional-appearance raw-row verification failed: $((Get-Content -LiteralPath $stderrPath -Raw))"
    }
    Assert-Equal ((($verifyConditional -join [Environment]::NewLine) | ConvertFrom-Json).ok) $true 'DCS conditional-appearance raw rows'

    [IO.File]::WriteAllBytes($dcsFormAttributesCfPath, $dcsFormAttributesCfBytes)
    $dcsFormAttributesStdout = & $BinaryPath cf export --source-version 2.20 $dcsFormAttributesCfPath $dcsFormAttributesOutputRoot 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        $stderr = if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath -Raw } else { '' }
        throw "DCS Form Attributes CF export failed with exit code $LASTEXITCODE. $stderr"
    }
    $dcsFormAttributesReport = ($dcsFormAttributesStdout -join [Environment]::NewLine) | ConvertFrom-Json
    Assert-Equal $dcsFormAttributesReport.ok $true 'DCS Form Attributes export status'
    Assert-Equal $dcsFormAttributesReport.export.storage.failed 0 'DCS Form Attributes failed storage entries'
    foreach ($expected in @($dcsFormAttributesManifest.selected_native_files)) {
        $candidatePath = Join-Path $dcsFormAttributesOutputRoot ($expected.path.Replace('/', [IO.Path]::DirectorySeparatorChar))
        if (-not (Test-Path -LiteralPath $candidatePath -PathType Leaf)) {
            throw "Exported DCS Form Attributes output is missing: $($expected.path)"
        }
        Assert-Equal (Get-Item -LiteralPath $candidatePath).Length ([long]$expected.size) "$($expected.path) exported size"
        Assert-Equal (Get-FileSha256Hex $candidatePath) $expected.sha256 "$($expected.path) exported SHA-256"
    }
    $dcsFormAttributesFormPath = Join-Path $dcsFormAttributesOutputRoot 'Catalogs\FilterProbe\Forms\ListForm\Ext\Form.xml'
    $dcsFormAttributesWrapper = Get-Utf8XmlFragmentBytes $dcsFormAttributesFormPath '<ConditionalAppearance>' '</ConditionalAppearance>'
    Assert-Equal $dcsFormAttributesWrapper.Length ([long]$dcsFormAttributesManifest.form.wrapper.decoded_size) 'DCS Form Attributes wrapper exported size'
    Assert-Equal (Get-Sha256Hex $dcsFormAttributesWrapper) $dcsFormAttributesManifest.form.wrapper.sha256 'DCS Form Attributes wrapper exported SHA-256'
    $verifyDcsFormAttributes = & $BinaryPath cf verify $dcsFormAttributesCfPath --compression raw-deflate `
        --element $dcsFormAttributesManifest.form.metadata_key `
        --element $dcsFormAttributesManifest.form.body_key `
        --expect-sha256 "$($dcsFormAttributesManifest.form.metadata_key)=$($dcsFormAttributesManifest.form.metadata_row.unpacked_sha256)" `
        --expect-sha256 "$($dcsFormAttributesManifest.form.body_key)=$($dcsFormAttributesManifest.form.body_row.unpacked_sha256)" 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        throw "DCS Form Attributes raw-row verification failed: $((Get-Content -LiteralPath $stderrPath -Raw))"
    }
    Assert-Equal ((($verifyDcsFormAttributes -join [Environment]::NewLine) | ConvertFrom-Json).ok) $true 'DCS Form Attributes raw rows'

    $dcsFormAttributesCompilerInput = Get-Base64EvidenceBytes $dcsFormAttributesManifest.form.native_xml $dcsFormAttributesRoot
    [IO.File]::WriteAllBytes($dcsFormAttributesCandidateFormPath, $dcsFormAttributesCompilerInput)
    $overlayBinding = "$($dcsFormAttributesManifest.form.body_key)=$dcsFormAttributesCandidateFormPath"
    $overlayResult = & $BinaryPath cf overlay $dcsConditionalCfPath $dcsFormAttributesCandidateCfPath `
        --source-version 2.20 `
        --form-xml $overlayBinding 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        throw "DCS Form Attributes offline compiler overlay failed: $((Get-Content -LiteralPath $stderrPath -Raw))"
    }
    $overlayReport = ($overlayResult -join [Environment]::NewLine) | ConvertFrom-Json
    Assert-Equal $overlayReport.ok $true 'DCS Form Attributes compiler overlay status'
    Assert-Equal (Get-Item -LiteralPath $dcsFormAttributesCandidateCfPath).Length ([long]$dcsFormAttributesManifest.compiler_acceptance.candidate_cf_size) 'DCS Form Attributes compiler candidate size'
    $verifyDcsFormAttributesCandidate = & $BinaryPath cf verify $dcsFormAttributesCandidateCfPath --compression raw-deflate `
        --element $dcsFormAttributesManifest.form.body_key `
        --expect-sha256 "$($dcsFormAttributesManifest.form.body_key)=$($dcsFormAttributesManifest.compiler_acceptance.body_unpacked_sha256)" 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        throw "DCS Form Attributes compiler candidate verification failed: $((Get-Content -LiteralPath $stderrPath -Raw))"
    }
    Assert-Equal ((($verifyDcsFormAttributesCandidate -join [Environment]::NewLine) | ConvertFrom-Json).ok) $true 'DCS Form Attributes compiler candidate raw body'
    $candidateExport = & $BinaryPath cf export --source-version 2.20 $dcsFormAttributesCandidateCfPath $dcsFormAttributesCandidateOutputRoot 2> $stderrPath
    if ($LASTEXITCODE -ne 0) {
        throw "DCS Form Attributes compiler candidate export failed: $((Get-Content -LiteralPath $stderrPath -Raw))"
    }
    Assert-Equal ((($candidateExport -join [Environment]::NewLine) | ConvertFrom-Json).ok) $true 'DCS Form Attributes compiler candidate export status'
    $candidateFormPath = Join-Path $dcsFormAttributesCandidateOutputRoot 'Catalogs\FilterProbe\Forms\ListForm\Ext\Form.xml'
    $candidateWrapper = Get-Utf8XmlFragmentBytes $candidateFormPath '<ConditionalAppearance>' '</ConditionalAppearance>'
    Assert-Equal (Get-Sha256Hex $candidateWrapper) $dcsFormAttributesManifest.compiler_acceptance.platform_native_wrapper_sha256 'DCS Form Attributes compiler candidate wrapper SHA-256'
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}

Write-Output 'Native evidence verification passed: 8.3.27.2214 / XML 2.20 / Task + BusinessProcess + DCS selection/order/filter/conditionalAppearance/dataParameters source-owned/multi-variant envelope/Form Attributes wrapper + ChartOfCharacteristicTypes + register and plan generated types.'
