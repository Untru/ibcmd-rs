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
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}

Write-Output 'Native evidence verification passed: 8.3.27.2214 / XML 2.20 / Task + BusinessProcess + DCS selection + ChartOfCharacteristicTypes + register and plan generated types.'
