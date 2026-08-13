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
$dcsFilterComparisonCfPath = Join-Path $temporaryRoot 'dcs-filter-comparison.cf'
$dcsFilterComparisonOutputRoot = Join-Path $temporaryRoot 'dcs-filter-comparison-export'
$dcsFilterMetadataCfPath = Join-Path $temporaryRoot 'dcs-filter-metadata-only.cf'
$dcsFilterMetadataOutputRoot = Join-Path $temporaryRoot 'dcs-filter-metadata-only-export'
$dcsConditionalCfPath = Join-Path $temporaryRoot 'dcs-conditional-appearance.cf'
$dcsConditionalOutputRoot = Join-Path $temporaryRoot 'dcs-conditional-appearance-export'
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
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}

Write-Output 'Native evidence verification passed: 8.3.27.2214 / XML 2.20 / Task + BusinessProcess + DCS selection/order/filter/conditionalAppearance + ChartOfCharacteristicTypes + register and plan generated types.'
