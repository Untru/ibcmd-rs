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
Assert-RawNativeEvidenceCorpus `
    -RelativePath 'tests/fixtures/native-evidence/8.3.27.2214/register-generated-types' `
    -Issue 282 `
    -ExpectedObjects @{
        AccountingRegister = '8b6ea484-0164-4c68-a0cb-175a31c56186'
        CalculationRegister = '5ad20ecf-0375-4218-b348-0534286973a5'
        ChartOfAccounts = '7671ada0-5cde-47a2-b49e-8de67818fb10'
        ChartOfCalculationTypes = '8c132029-d49c-49db-b12b-64519b64d755'
    }

if (-not [IO.Path]::IsPathRooted($BinaryPath)) {
    $BinaryPath = Join-Path $RepositoryRoot $BinaryPath
}
if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
    throw "ibcmd-rs binary is missing: $BinaryPath"
}

$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("ibcmd-rs-native-evidence-" + [Guid]::NewGuid().ToString('N'))
$cfPath = Join-Path $temporaryRoot 'configuration.cf'
$outputRoot = Join-Path $temporaryRoot 'export'
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
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}

Write-Output 'Native evidence verification passed: 8.3.27.2214 / XML 2.20 / Task + BusinessProcess + register and plan generated types.'
