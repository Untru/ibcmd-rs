[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Join-Path $PSScriptRoot '..'),
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:AbsolutePathPatterns = @(
    '(?i)(?:^|[^\p{L}\p{N}_])[A-Z]:[\\/]',
    '(?i)\\\\[^\\\s]+\\[^\s]+',
    '(?m)(?<![:/])//[^/\s]+/[^\s]+',
    '(?i)(?<![\p{L}\p{N}_])file:(?:/{1,3})?',
    '(?m)(?<![\p{L}\p{N}_.:/-])/(?![/\s])'
)

function Test-StringContainsAbsolutePath {
    param([Parameter(Mandatory)] [AllowEmptyString()] [string]$Value)

    foreach ($pattern in $script:AbsolutePathPatterns) {
        if ($Value -match $pattern) {
            return $true
        }
    }
    return $false
}

function Get-NormalizedRelativePath {
    param(
        [Parameter(Mandatory)] [string]$BasePath,
        [Parameter(Mandatory)] [string]$Path
    )

    $baseFullPath = [System.IO.Path]::GetFullPath($BasePath)
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $getRelativePath = [System.IO.Path].GetMethod(
        'GetRelativePath',
        [type[]]@([string], [string])
    )
    if ($null -ne $getRelativePath) {
        return ([string]$getRelativePath.Invoke($null, @($baseFullPath, $fullPath))).Replace('\', '/')
    }

    $trimmedBasePath = $baseFullPath.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $baseUri = [System.Uri]::new($trimmedBasePath + [System.IO.Path]::DirectorySeparatorChar)
    $pathUri = [System.Uri]::new($fullPath)
    return [System.Uri]::UnescapeDataString(
        $baseUri.MakeRelativeUri($pathUri).OriginalString
    ).Replace('\', '/')
}

function Get-TrackedFiles {
    param([Parameter(Mandatory)] [string]$Root)

    $git = Get-Command git -ErrorAction SilentlyContinue
    if ($null -eq $git) {
        throw 'git is required to validate the tracked EDT-derived corpus.'
    }

    $trackedFiles = @(& git -C $Root ls-files)
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to list tracked files in '$Root'."
    }

    return @($trackedFiles | ForEach-Object { Join-Path $Root $_ })
}

function Get-TrackedEdtCorpusFiles {
    param(
        [Parameter(Mandatory)] [string]$Root,
        [Parameter(Mandatory)] [string[]]$TrackedFiles
    )

    $corpusRoot = Join-Path $Root 'crates/ibcmd-schema/data'
    return @($TrackedFiles | Where-Object {
        (Get-NormalizedRelativePath -BasePath $Root -Path $_).StartsWith(
            'crates/ibcmd-schema/data/', [System.StringComparison]::Ordinal)
    })
}

function Get-FixtureFiles {
    param([Parameter(Mandatory)] [string]$FixturePath)

    if (-not (Test-Path -LiteralPath $FixturePath -PathType Container)) {
        throw "Fixture directory '$FixturePath' does not exist."
    }

    return @(Get-ChildItem -LiteralPath $FixturePath -File -Recurse | ForEach-Object { $_.FullName })
}

function Assert-PayloadFileIsAllowed {
    param(
        [Parameter(Mandatory)] [string]$Path,
        [Parameter(Mandatory)] [string]$DisplayPath
    )

    $forbiddenExtensions = @('.jar', '.class', '.dll', '.so', '.dylib', '.xcore')
    $extension = [System.IO.Path]::GetExtension($Path).ToLowerInvariant()
    if ($forbiddenExtensions -contains $extension) {
        throw "$DisplayPath is a forbidden proprietary binary or Xcore payload ($extension)."
    }

    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $header = [byte[]]::new(8)
        $bytesRead = $stream.Read($header, 0, $header.Length)
    }
    finally {
        $stream.Dispose()
    }

    $signatures = @(
        @{ Name = 'ZIP/JAR'; Bytes = [byte[]](0x50, 0x4B, 0x03, 0x04) },
        @{ Name = 'ZIP/JAR'; Bytes = [byte[]](0x50, 0x4B, 0x05, 0x06) },
        @{ Name = 'ZIP/JAR'; Bytes = [byte[]](0x50, 0x4B, 0x07, 0x08) },
        @{ Name = 'Java class or Mach-O universal binary'; Bytes = [byte[]](0xCA, 0xFE, 0xBA, 0xBE) },
        @{ Name = 'Windows PE'; Bytes = [byte[]](0x4D, 0x5A) },
        @{ Name = 'ELF'; Bytes = [byte[]](0x7F, 0x45, 0x4C, 0x46) },
        @{ Name = 'Mach-O'; Bytes = [byte[]](0xFE, 0xED, 0xFA, 0xCE) },
        @{ Name = 'Mach-O'; Bytes = [byte[]](0xFE, 0xED, 0xFA, 0xCF) },
        @{ Name = 'Mach-O'; Bytes = [byte[]](0xCE, 0xFA, 0xED, 0xFE) },
        @{ Name = 'Mach-O'; Bytes = [byte[]](0xCF, 0xFA, 0xED, 0xFE) }
    )

    foreach ($signature in $signatures) {
        if ($bytesRead -lt $signature.Bytes.Length) {
            continue
        }

        $matches = $true
        for ($index = 0; $index -lt $signature.Bytes.Length; $index++) {
            if ($header[$index] -ne $signature.Bytes[$index]) {
                $matches = $false
                break
            }
        }

        if ($matches) {
            throw "$DisplayPath has a forbidden binary signature ($($signature.Name))."
        }
    }
}

function Assert-TrackedFileIsSafe {
    param(
        [Parameter(Mandatory)] [string]$Path,
        [Parameter(Mandatory)] [string]$Root,
        [Parameter(Mandatory)] [string]$DisplayPath
    )

    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "$DisplayPath is a symlink or reparse-point file."
    }

    $rootFullPath = [System.IO.Path]::GetFullPath($Root).TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
    $fileFullPath = [System.IO.Path]::GetFullPath($Path)
    $rootPrefix = $rootFullPath + [System.IO.Path]::DirectorySeparatorChar
    if (-not $fileFullPath.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$DisplayPath resolves outside the repository root."
    }
}

function Assert-TextContainsPortablePaths {
    param(
        [Parameter(Mandatory)] [string]$Text,
        [Parameter(Mandatory)] [string]$DisplayPath
    )

    if (Test-StringContainsAbsolutePath -Value $Text) {
        throw "$DisplayPath contains an absolute drive, UNC, POSIX, or file URI path."
    }
}

function Test-PortableEvidenceSource {
    param([AllowNull()] $Value)

    if (-not ($Value -is [string]) -or [string]::IsNullOrWhiteSpace($Value)) {
        return $false
    }

    return $Value -notmatch '(?i)(?:^[A-Z]:[\\/]|^\\\\|^file:|^/)'
}

function ConvertTo-Hashtable {
    param([AllowNull()] $Value)

    if ($null -eq $Value -or $Value -is [string]) {
        return $Value
    }

    if ($Value -is [System.Collections.IDictionary]) {
        $table = [ordered]@{}
        foreach ($key in $Value.Keys) {
            $table[$key] = ConvertTo-Hashtable -Value $Value[$key]
        }
        return $table
    }

    if ($Value -is [System.Management.Automation.PSCustomObject]) {
        $table = [ordered]@{}
        foreach ($property in $Value.PSObject.Properties) {
            $table[$property.Name] = ConvertTo-Hashtable -Value $property.Value
        }
        return $table
    }

    if ($Value -is [System.Collections.IEnumerable]) {
        $items = @()
        foreach ($item in $Value) {
            $items += ,(ConvertTo-Hashtable -Value $item)
        }
        return ,$items
    }

    return $Value
}

function Assert-DecodedJsonValueContainsPortablePaths {
    param(
        [AllowNull()] [AllowEmptyString()] $Value,
        [Parameter(Mandatory)] [string]$DisplayPath,
        [string]$JsonPath = '$'
    )

    if ($Value -is [string]) {
        if (Test-StringContainsAbsolutePath -Value $Value) {
            throw "$DisplayPath contains an absolute drive, UNC, POSIX, or file URI path at $JsonPath."
        }
        return
    }

    if ($Value -is [System.Collections.IDictionary]) {
        $propertyIndex = 0
        foreach ($key in $Value.Keys) {
            Assert-DecodedJsonValueContainsPortablePaths `
                -Value ([string]$key) `
                -DisplayPath $DisplayPath `
                -JsonPath "$JsonPath.<key[$propertyIndex]>"
            Assert-DecodedJsonValueContainsPortablePaths `
                -Value $Value[$key] `
                -DisplayPath $DisplayPath `
                -JsonPath "$JsonPath.<value[$propertyIndex]>"
            $propertyIndex++
        }
        return
    }

    if ($Value -is [System.Collections.IEnumerable]) {
        $index = 0
        foreach ($item in $Value) {
            Assert-DecodedJsonValueContainsPortablePaths `
                -Value $item `
                -DisplayPath $DisplayPath `
                -JsonPath "$JsonPath[$index]"
            $index++
        }
    }
}

function Test-HasVerifiedProvenance {
    param(
        [Parameter(Mandatory)] [System.Collections.IDictionary]$Fact,
        [Parameter(Mandatory)] [bool]$AllowWriterRuleFallback
    )

    if ($Fact.Contains('sources')) {
        $sources = $Fact['sources']
        if (-not ($sources -is [System.Collections.IEnumerable]) -or $sources -is [string]) {
            return $false
        }

        $sourceCount = 0
        foreach ($source in $sources) {
            if (-not (Test-PortableEvidenceSource -Value $source)) {
                return $false
            }
            $sourceCount++
        }
        return $sourceCount -gt 0
    }

    if ($AllowWriterRuleFallback -and $Fact.Contains('kind') -and $Fact.Contains('note') -and
        $Fact.Keys.Count -eq 3 -and
        -not [string]::IsNullOrWhiteSpace([string]$Fact['kind']) -and
        -not [string]::IsNullOrWhiteSpace([string]$Fact['note'])) {
        return $true
    }

    return $false
}

function Assert-VerifiedFactsHaveProvenance {
    param(
        [AllowNull()] $Value,
        [Parameter(Mandatory)] [string]$DisplayPath,
        [Parameter(Mandatory)] [bool]$IsWriterRuleDocument,
        [string]$JsonPath = '$'
    )

    if ($null -eq $Value) {
        return
    }

    if ($Value -is [System.Collections.IDictionary]) {
        if ($Value.Contains('status') -and ([string]$Value['status']).Equals('verified', [System.StringComparison]::OrdinalIgnoreCase)) {
            $allowWriterRuleFallback = $IsWriterRuleDocument -and $JsonPath -match '^\$\.rules\[\d+\]\.evidence$'
            if (-not (Test-HasVerifiedProvenance -Fact $Value -AllowWriterRuleFallback $allowWriterRuleFallback)) {
                throw "$DisplayPath contains a verified fact without provenance."
            }
        }

        foreach ($key in $Value.Keys) {
            Assert-VerifiedFactsHaveProvenance -Value $Value[$key] -DisplayPath $DisplayPath -IsWriterRuleDocument $IsWriterRuleDocument -JsonPath "$JsonPath.$key"
        }
        return
    }

    if ($Value -is [System.Collections.IEnumerable] -and -not ($Value -is [string])) {
        for ($index = 0; $index -lt $Value.Count; $index++) {
            Assert-VerifiedFactsHaveProvenance -Value $Value[$index] -DisplayPath $DisplayPath -IsWriterRuleDocument $IsWriterRuleDocument -JsonPath "$JsonPath[$index]"
        }
    }
}

function Assert-EdtCorpusFile {
    param(
        [Parameter(Mandatory)] [string]$Path,
        [Parameter(Mandatory)] [string]$DisplayPath
    )

    Assert-PayloadFileIsAllowed -Path $Path -DisplayPath $DisplayPath

    $extension = [System.IO.Path]::GetExtension($Path).ToLowerInvariant()
    if ($extension -ne '.json') {
        throw "$DisplayPath is not an approved cleansed JSON corpus file."
    }

    $text = Get-Content -LiteralPath $Path -Raw
    Assert-TextContainsPortablePaths -Text $text -DisplayPath $DisplayPath

    try {
        $convertFromJsonParameters = @{}
        $convertFromJsonCommand = Get-Command ConvertFrom-Json
        if ($convertFromJsonCommand.Parameters.ContainsKey('AsHashtable')) {
            $convertFromJsonParameters['AsHashtable'] = $true
        }
        if ($convertFromJsonCommand.Parameters.ContainsKey('Depth')) {
            $convertFromJsonParameters['Depth'] = 100
        }
        $document = ConvertTo-Hashtable -Value (
            $text | ConvertFrom-Json @convertFromJsonParameters
        )
    }
    catch {
        throw "$DisplayPath is not valid JSON: $($_.Exception.Message)"
    }

    Assert-DecodedJsonValueContainsPortablePaths -Value $document -DisplayPath $DisplayPath

    if (-not ($document -is [System.Collections.IDictionary])) {
        throw "$DisplayPath must contain a JSON object."
    }
    if (-not $document.Contains('schemaVersion')) {
        throw "$DisplayPath is missing schemaVersion."
    }
    if (-not $document.Contains('source') -or -not ($document['source'] -is [System.Collections.IDictionary])) {
        throw "$DisplayPath is missing versioned source metadata."
    }

    $source = $document['source']
    foreach ($field in @('product', 'release', 'derivation')) {
        if (-not $source.Contains($field) -or [string]::IsNullOrWhiteSpace([string]$source[$field])) {
            throw "$DisplayPath is missing source.$field metadata."
        }
    }

    $isWriterRuleDocument = $DisplayPath -match '^crates/ibcmd-schema/data/edt-.+-writer-rules\.json$'
    Assert-VerifiedFactsHaveProvenance -Value $document -DisplayPath $DisplayPath -IsWriterRuleDocument $isWriterRuleDocument
}

function Invoke-EdtCorpusValidation {
    param(
        [Parameter(Mandatory)] [string[]]$Files,
        [Parameter(Mandatory)] [string]$DisplayRoot,
        [switch]$CheckPayloadOnly
    )

    if ($Files.Count -eq 0) {
        throw "No EDT-derived corpus files were found under '$DisplayRoot'."
    }

    foreach ($file in $Files | Sort-Object) {
        $displayPath = Get-NormalizedRelativePath -BasePath $DisplayRoot -Path $file
        Assert-TrackedFileIsSafe -Path $file -Root $DisplayRoot -DisplayPath $displayPath
        Assert-PayloadFileIsAllowed -Path $file -DisplayPath $displayPath
        if (-not $CheckPayloadOnly) {
            Assert-EdtCorpusFile -Path $file -DisplayPath $displayPath
        }
    }
}

function Invoke-SelfTest {
    param([Parameter(Mandatory)] [string]$Root)

    $fixtureRoot = Join-Path $Root 'tests/fixtures/schema-governance'
    $cases = @(
        @{ Name = 'valid'; MustPass = $true },
        @{ Name = 'legitimate-profile-identifiers'; MustPass = $true },
        @{ Name = 'absolute-path'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'unc-path'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'file-uri'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'posix-path'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'escaped-posix-path'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'escaped-unc-path'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'escaped-file-uri'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'forward-unc-path'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'escaped-absolute-key'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'delimited-escaped-posix-path'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'delimited-escaped-unc-path'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'delimited-escaped-file-uri'; MustPass = $false; ExpectedError = 'contains an absolute drive, UNC, POSIX, or file URI path' },
        @{ Name = 'missing-provenance'; MustPass = $false },
        @{ Name = 'fake-provenance'; MustPass = $false }
    )

    foreach ($case in $cases) {
        $casePath = Join-Path $fixtureRoot $case.Name
        $passed = $false
        try {
            $files = Get-FixtureFiles -FixturePath $casePath
            $payloadOnly = $case.ContainsKey('PayloadOnly') -and [bool]$case.PayloadOnly
            if ($payloadOnly) {
                Invoke-EdtCorpusValidation -Files $files -DisplayRoot $casePath -CheckPayloadOnly
            }
            else {
                Invoke-EdtCorpusValidation -Files $files -DisplayRoot $casePath
            }
            $passed = $true
        }
        catch {
            if ($case.MustPass) {
                throw "Self-test '$($case.Name)' unexpectedly failed: $($_.Exception.Message)"
            }
            if ($case.ContainsKey('ExpectedError') -and
                $_.Exception.Message -notmatch [regex]::Escape([string]$case.ExpectedError)) {
                throw "Self-test '$($case.Name)' failed through an unexpected branch: $($_.Exception.Message)"
            }
        }

        if ($case.MustPass -ne $passed) {
            throw "Self-test '$($case.Name)' did not produce the expected result."
        }
    }

    $payloadDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("ibcmd-governance-payload-" + [guid]::NewGuid())
    [System.IO.Directory]::CreateDirectory($payloadDirectory) | Out-Null
    try {
        $payloadPath = Join-Path $payloadDirectory 'edt-proprietary.payload'
        [System.IO.File]::WriteAllBytes($payloadPath, [byte[]](0x4D, 0x5A, 0x00))
        $payloadRejected = $false
        try {
            Invoke-EdtCorpusValidation -Files @($payloadPath) -DisplayRoot $payloadDirectory -CheckPayloadOnly
        }
        catch {
            if ($_.Exception.Message -notmatch [regex]::Escape('forbidden binary signature')) {
                throw "Self-test 'forbidden-payload' failed through an unexpected branch: $($_.Exception.Message)"
            }
            $payloadRejected = $true
        }
        if (-not $payloadRejected) {
            throw "Self-test 'forbidden-payload' did not produce the expected result."
        }
    }
    finally {
        [System.IO.Directory]::Delete($payloadDirectory, $true)
    }

    $temporaryDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("ibcmd-governance-" + [guid]::NewGuid())
    [System.IO.Directory]::CreateDirectory($temporaryDirectory) | Out-Null
    try {
        $targetPath = Join-Path $temporaryDirectory 'target.json'
        $linkPath = Join-Path $temporaryDirectory 'linked.json'
        [System.IO.File]::WriteAllText($targetPath, '{}', [System.Text.UTF8Encoding]::new($false))
        $createSymbolicLink = [System.IO.File].GetMethod(
            'CreateSymbolicLink',
            [type[]]@([string], [string])
        )
        if ($null -eq $createSymbolicLink) {
            Write-Host 'Symlink self-test skipped: symbolic links are unsupported.'
        }
        else {
            try {
                [System.IO.File]::CreateSymbolicLink($linkPath, $targetPath) | Out-Null
                $rejected = $false
                try {
                    Invoke-EdtCorpusValidation -Files @($linkPath) -DisplayRoot $temporaryDirectory -CheckPayloadOnly
                }
                catch {
                    $rejected = $true
                }
                if (-not $rejected) {
                    throw 'Self-test symlink was not rejected.'
                }
            }
            catch [System.PlatformNotSupportedException] {
                Write-Host 'Symlink self-test skipped: symbolic links are unsupported.'
            }
            catch [System.UnauthorizedAccessException] {
                Write-Host 'Symlink self-test skipped: symbolic-link permission is unavailable.'
            }
            catch [System.IO.IOException] {
                Write-Host 'Symlink self-test skipped: symbolic-link creation is unavailable.'
            }
        }
    }
    finally {
        [System.IO.Directory]::Delete($temporaryDirectory, $true)
    }

    Write-Host 'EDT corpus governance self-tests passed.'
}

$resolvedRoot = [System.IO.Path]::GetFullPath($RepositoryRoot)
if ($SelfTest) {
    Invoke-SelfTest -Root $resolvedRoot
    exit 0
}

$trackedFiles = Get-TrackedFiles -Root $resolvedRoot
Invoke-EdtCorpusValidation -Files $trackedFiles -DisplayRoot $resolvedRoot -CheckPayloadOnly
Invoke-EdtCorpusValidation -Files (Get-TrackedEdtCorpusFiles -Root $resolvedRoot -TrackedFiles $trackedFiles) -DisplayRoot $resolvedRoot
Write-Host 'EDT-derived corpus governance validation passed.'
