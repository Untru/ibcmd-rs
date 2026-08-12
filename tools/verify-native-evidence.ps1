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
    param([string]$RelativePath)

    $platformPath = $RelativePath.Replace('/', [IO.Path]::DirectorySeparatorChar)
    return Join-Path $fixtureRoot $platformPath
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
    param($Evidence)

    $path = Resolve-FixturePath $Evidence.path
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Evidence file is missing: $($Evidence.path)"
    }
    $item = Get-Item -LiteralPath $path
    Assert-Equal $item.Length ([long]$Evidence.size) "$($Evidence.path) size"
    Assert-Equal (Get-FileSha256Hex $path) $Evidence.sha256 "$($Evidence.path) SHA-256"
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

Write-Output 'Native evidence verification passed: 8.3.27.2214 / XML 2.20 / Task.CorpusTask.'
