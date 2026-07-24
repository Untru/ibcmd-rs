[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$CanonicalDiffPath,

    [Parameter(Mandatory = $true)]
    [string]$NativeRoot,

    [Parameter(Mandatory = $true)]
    [string]$BaselineCandidateRoot,

    [Parameter(Mandatory = $true)]
    [string]$AfterCandidateRoot,

    [Parameter(Mandatory = $true)]
    [string]$NativeRunId,

    [Parameter(Mandatory = $true)]
    [string]$BaselineRunId,

    [Parameter(Mandatory = $true)]
    [string]$AfterRunId,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$AfterCommit,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string[]]$SelectedPath,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string[]]$RawLayoutRoot,

    [Parameter(Mandatory = $true)]
    [string]$OutputJson,

    [Parameter(Mandatory = $true)]
    [string]$OutputMarkdown,

    [int]$ExpectedLeftOnlyItemSignatures = 209,
    [int]$ExpectedMissingCommandMultiset = 211,
    [int]$ExpectedMissingFileCount = 57,
    [int]$ExpectedSelectedPositiveCount = 3,
    [int]$ExpectedSelectedAbsentCount = 1,
    [int]$ExpectedRawProbeCount = 4,
    [int]$ExpectedRawItemCount = 17,
    [string[]]$ExpectedRawKinds = @('0', '1', '3', '4', '5'),

    [switch]$VerifyOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)]
        [bool]$Condition,

        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Get-NormalizedAbsolutePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $candidate = $Path.Trim()
    if ($candidate.StartsWith('\\?\', [StringComparison]::Ordinal)) {
        $candidate = $candidate.Substring(4)
    }
    [IO.Path]::GetFullPath($candidate).TrimEnd(
        [IO.Path]::DirectorySeparatorChar,
        [IO.Path]::AltDirectorySeparatorChar
    )
}

function Get-FileSha256 {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-BytesSha256 {
    param(
        [Parameter(Mandatory = $true)]
        [byte[]]$Bytes
    )

    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        ([Convert]::ToHexString($algorithm.ComputeHash($Bytes))).ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
    }
}

function Read-XmlDocument {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $document = [Xml.XmlDocument]::new()
    $document.PreserveWhitespace = $true
    try {
        $document.Load($Path)
    }
    catch {
        throw "Cannot parse XML '$Path': $($_.Exception.Message)"
    }
    $document
}

function Get-NavigationPanelCommands {
    param(
        [Parameter(Mandatory = $true)]
        [Xml.XmlDocument]$Document,

        [Parameter(Mandatory = $true)]
        [string]$RelativePath
    )

    $items = @($Document.SelectNodes(
        "/*[local-name()='Form']" +
        "/*[local-name()='CommandInterface']" +
        "/*[local-name()='NavigationPanel']" +
        "/*[local-name()='Item']"
    ))
    $commands = [Collections.Generic.List[string]]::new()
    foreach ($item in $items) {
        $commandNode = $item.SelectSingleNode("./*[local-name()='Command']")
        Assert-Condition ($null -ne $commandNode) (
            "NavigationPanel Item without Command: $RelativePath"
        )
        $command = $commandNode.InnerText
        Assert-Condition (-not [String]::IsNullOrWhiteSpace($command)) (
            "NavigationPanel Item with an empty Command: $RelativePath"
        )
        $commands.Add($command)
    }
    $commands.ToArray()
}

function Find-BytePattern {
    param(
        [Parameter(Mandatory = $true)]
        [byte[]]$Bytes,

        [Parameter(Mandatory = $true)]
        [byte[]]$Pattern,

        [int]$StartIndex = 0
    )

    if ($Pattern.Length -eq 0 -or $Bytes.Length -lt $Pattern.Length) {
        return -1
    }
    $lastStart = $Bytes.Length - $Pattern.Length
    for ($index = $StartIndex; $index -le $lastStart; $index++) {
        $matches = $true
        for ($offset = 0; $offset -lt $Pattern.Length; $offset++) {
            if ($Bytes[$index + $offset] -ne $Pattern[$offset]) {
                $matches = $false
                break
            }
        }
        if ($matches) {
            return $index
        }
    }
    -1
}

function Get-NavigationPanelFragment {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $bytes = [IO.File]::ReadAllBytes($Path)
    $startPattern = [Text.Encoding]::UTF8.GetBytes('<NavigationPanel>')
    $endPattern = [Text.Encoding]::UTF8.GetBytes('</NavigationPanel>')
    $start = Find-BytePattern -Bytes $bytes -Pattern $startPattern
    if ($start -lt 0) {
        return [pscustomobject][ordered]@{
            present = $false
            bytes = [byte[]]@()
        }
    }
    $lineStart = $start
    while ($lineStart -gt 0 -and $bytes[$lineStart - 1] -ne 10) {
        $lineStart--
    }
    for ($index = $lineStart; $index -lt $start; $index++) {
        Assert-Condition (
            $bytes[$index] -eq 9 -or $bytes[$index] -eq 32
        ) "Non-whitespace data before NavigationPanel on its line: $Path"
    }
    $start = $lineStart
    $endStart = Find-BytePattern -Bytes $bytes -Pattern $endPattern `
        -StartIndex ($start + $startPattern.Length)
    Assert-Condition ($endStart -ge 0) "Unclosed NavigationPanel fragment: $Path"
    $endExclusive = $endStart + $endPattern.Length
    if (
        $endExclusive + 1 -lt $bytes.Length -and
        $bytes[$endExclusive] -eq 13 -and
        $bytes[$endExclusive + 1] -eq 10
    ) {
        $endExclusive += 2
    }
    elseif ($endExclusive -lt $bytes.Length -and $bytes[$endExclusive] -eq 10) {
        $endExclusive += 1
    }
    Assert-Condition (
        (Find-BytePattern -Bytes $bytes -Pattern $startPattern -StartIndex $endExclusive) -lt 0
    ) "More than one NavigationPanel fragment: $Path"

    $length = $endExclusive - $start
    $fragment = [byte[]]::new($length)
    [Buffer]::BlockCopy($bytes, $start, $fragment, 0, $length)
    [pscustomobject][ordered]@{
        present = $true
        bytes = $fragment
    }
}

function Split-1CBracedFields {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text,

        [Parameter(Mandatory = $true)]
        [string]$Context
    )

    $value = $Text.Trim()
    Assert-Condition ($value.StartsWith('{', [StringComparison]::Ordinal)) (
        "Expected a braced value: $Context"
    )
    $fields = [Collections.Generic.List[string]]::new()
    $depth = 0
    $inString = $false
    $fieldStart = 1
    $rootEnd = -1
    for ($index = 0; $index -lt $value.Length; $index++) {
        $character = $value[$index]
        if ($inString) {
            if ($character -eq '"') {
                if ($index + 1 -lt $value.Length -and $value[$index + 1] -eq '"') {
                    $index++
                }
                else {
                    $inString = $false
                }
            }
            continue
        }
        if ($character -eq '"') {
            $inString = $true
            continue
        }
        if ($character -eq '{') {
            $depth++
            continue
        }
        if ($character -eq '}') {
            $depth--
            Assert-Condition ($depth -ge 0) "Unbalanced braces: $Context"
            if ($depth -eq 0) {
                $fields.Add($value.Substring($fieldStart, $index - $fieldStart).Trim())
                $rootEnd = $index
                break
            }
            continue
        }
        if ($character -eq ',' -and $depth -eq 1) {
            $fields.Add($value.Substring($fieldStart, $index - $fieldStart).Trim())
            $fieldStart = $index + 1
        }
    }
    Assert-Condition (-not $inString) "Unclosed string: $Context"
    Assert-Condition ($rootEnd -ge 0) "Unclosed braced value: $Context"
    Assert-Condition (
        [String]::IsNullOrWhiteSpace($value.Substring($rootEnd + 1))
    ) "Trailing data after a braced value: $Context"
    $fields.ToArray()
}

function ConvertTo-SingleQuotedPowerShellLiteral {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    "'" + $Value.Replace("'", "''") + "'"
}

function Write-EvidenceFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Content,

        [Parameter(Mandatory = $true)]
        [bool]$Verify
    )

    $normalizedContent = $Content + [Environment]::NewLine
    if ($Verify) {
        Assert-Condition (Test-Path -LiteralPath $Path -PathType Leaf) (
            "Evidence file does not exist: $Path"
        )
        $actual = [IO.File]::ReadAllText((Get-NormalizedAbsolutePath $Path))
        Assert-Condition ($actual -ceq $normalizedContent) "Stale evidence file: $Path"
        return
    }
    $absolutePath = Get-NormalizedAbsolutePath $Path
    $parent = Split-Path -Parent $absolutePath
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        New-Item -ItemType Directory -Path $parent | Out-Null
    }
    [IO.File]::WriteAllText(
        $absolutePath,
        $normalizedContent,
        [Text.UTF8Encoding]::new($false)
    )
}

$canonicalDiffPathNormalized = Get-NormalizedAbsolutePath $CanonicalDiffPath
$nativeRootNormalized = Get-NormalizedAbsolutePath $NativeRoot
$baselineCandidateRootNormalized = Get-NormalizedAbsolutePath $BaselineCandidateRoot
$afterCandidateRootNormalized = Get-NormalizedAbsolutePath $AfterCandidateRoot
$rawLayoutRootsNormalized = @(
    $RawLayoutRoot | ForEach-Object { Get-NormalizedAbsolutePath $_ }
)

Assert-Condition (Test-Path -LiteralPath $canonicalDiffPathNormalized -PathType Leaf) (
    "Canonical diff does not exist: $canonicalDiffPathNormalized"
)
foreach ($root in @(
    $nativeRootNormalized,
    $baselineCandidateRootNormalized,
    $afterCandidateRootNormalized
) + $rawLayoutRootsNormalized) {
    Assert-Condition (Test-Path -LiteralPath $root -PathType Container) (
        "Input root does not exist: $root"
    )
}

$canonicalDiff = Get-Content -LiteralPath $canonicalDiffPathNormalized -Raw |
    ConvertFrom-Json -Depth 100
$canonicalNativeRoot = Get-NormalizedAbsolutePath ([string]$canonicalDiff.left_root
)
$canonicalCandidateRoot = Get-NormalizedAbsolutePath ([string]$canonicalDiff.right_root
)
Assert-Condition (
    $canonicalNativeRoot.Equals($nativeRootNormalized, [StringComparison]::OrdinalIgnoreCase)
) 'Canonical diff left_root contradicts NativeRoot'
Assert-Condition (
    $canonicalCandidateRoot.Equals(
        $baselineCandidateRootNormalized,
        [StringComparison]::OrdinalIgnoreCase
    )
) 'Canonical diff right_root contradicts BaselineCandidateRoot'

$changedForms = @(
    $canonicalDiff.differences | Where-Object {
        $_.kind -eq 'form' -and
        $_.status -eq 'different' -and
        ([string]$_.path).EndsWith('/Ext/Form.xml', [StringComparison]::Ordinal)
    }
)
Assert-Condition ($changedForms.Count -gt 0) 'Canonical diff has no changed form pairs'

$leftOnlyItemSignatures = 0
$actualMissingCommandMultiset = 0
$leftOnlyFiles = [Collections.Generic.HashSet[string]]::new(
    [StringComparer]::Ordinal
)
$missingCommandFiles = [Collections.Generic.HashSet[string]]::new(
    [StringComparer]::Ordinal
)

foreach ($entry in $changedForms) {
    $relativePath = [string]$entry.path
    $platformRelativePath = $relativePath.Replace(
        '/',
        [IO.Path]::DirectorySeparatorChar
    )
    $nativePath = Join-Path $nativeRootNormalized $platformRelativePath
    $candidatePath = Join-Path $baselineCandidateRootNormalized $platformRelativePath
    Assert-Condition (Test-Path -LiteralPath $nativePath -PathType Leaf) (
        "Native form is missing: $relativePath"
    )
    Assert-Condition (Test-Path -LiteralPath $candidatePath -PathType Leaf) (
        "Baseline candidate form is missing: $relativePath"
    )

    $nativeCommands = @(
        Get-NavigationPanelCommands `
            -Document (Read-XmlDocument $nativePath) `
            -RelativePath $relativePath
    )
    $candidateCommands = @(
        Get-NavigationPanelCommands `
            -Document (Read-XmlDocument $candidatePath) `
            -RelativePath $relativePath
    )

    $leftOnlyInFile = [Math]::Max(
        $nativeCommands.Count - $candidateCommands.Count,
        0
    )
    $leftOnlyItemSignatures += $leftOnlyInFile
    if ($leftOnlyInFile -gt 0) {
        $null = $leftOnlyFiles.Add($relativePath)
    }

    $candidateMultiset = @{}
    foreach ($command in $candidateCommands) {
        if (-not $candidateMultiset.ContainsKey($command)) {
            $candidateMultiset[$command] = 0
        }
        $candidateMultiset[$command] = [int]$candidateMultiset[$command] + 1
    }
    $missingInFile = 0
    foreach ($command in $nativeCommands) {
        if (
            $candidateMultiset.ContainsKey($command) -and
            [int]$candidateMultiset[$command] -gt 0
        ) {
            $candidateMultiset[$command] = [int]$candidateMultiset[$command] - 1
        }
        else {
            $missingInFile++
        }
    }
    if ($leftOnlyInFile -gt 0) {
        $actualMissingCommandMultiset += $missingInFile
        if ($missingInFile -gt 0) {
            $null = $missingCommandFiles.Add($relativePath)
        }
    }
}

Assert-Condition (
    $leftOnlyItemSignatures -eq $ExpectedLeftOnlyItemSignatures
) (
    "Left-only NavigationPanel Item signature count is " +
    "$leftOnlyItemSignatures; expected $ExpectedLeftOnlyItemSignatures"
)
Assert-Condition (
    $actualMissingCommandMultiset -eq $ExpectedMissingCommandMultiset
) (
    "Actual missing NavigationPanel command multiset is " +
    "$actualMissingCommandMultiset; expected $ExpectedMissingCommandMultiset"
)
Assert-Condition ($leftOnlyFiles.Count -eq $ExpectedMissingFileCount) (
    "Files with left-only NavigationPanel Items: $($leftOnlyFiles.Count); " +
    "expected $ExpectedMissingFileCount"
)
Assert-Condition ($missingCommandFiles.Count -eq $ExpectedMissingFileCount) (
    "Files with missing NavigationPanel commands: $($missingCommandFiles.Count); " +
    "expected $ExpectedMissingFileCount"
)

$selectedResults = [Collections.Generic.List[object]]::new()
$selectedPositiveCount = 0
$selectedAbsentCount = 0
foreach ($relativePath in $SelectedPath) {
    Assert-Condition (-not [IO.Path]::IsPathRooted($relativePath)) (
        "Selected path must be relative: $relativePath"
    )
    $platformRelativePath = $relativePath.Replace(
        '/',
        [IO.Path]::DirectorySeparatorChar
    )
    $nativePath = Join-Path $nativeRootNormalized $platformRelativePath
    $afterPath = Join-Path $afterCandidateRootNormalized $platformRelativePath
    Assert-Condition (Test-Path -LiteralPath $nativePath -PathType Leaf) (
        "Selected native form is missing: $relativePath"
    )
    Assert-Condition (Test-Path -LiteralPath $afterPath -PathType Leaf) (
        "Selected after form is missing: $relativePath"
    )

    $nativeFragment = Get-NavigationPanelFragment $nativePath
    $afterFragment = Get-NavigationPanelFragment $afterPath
    Assert-Condition ($nativeFragment.present -eq $afterFragment.present) (
        "NavigationPanel presence mismatch after patch: $relativePath"
    )
    if ($nativeFragment.present) {
        $selectedPositiveCount++
        $nativeHash = Get-BytesSha256 $nativeFragment.bytes
        $afterHash = Get-BytesSha256 $afterFragment.bytes
        Assert-Condition (
            $nativeFragment.bytes.Length -eq $afterFragment.bytes.Length
        ) "NavigationPanel byte length mismatch after patch: $relativePath"
        Assert-Condition ($nativeHash -ceq $afterHash) (
            "NavigationPanel SHA256 mismatch after patch: $relativePath"
        )
    }
    else {
        $selectedAbsentCount++
        $nativeHash = $null
        $afterHash = $null
    }
    $selectedResults.Add([pscustomobject][ordered]@{
        path = $relativePath.Replace('\', '/')
        present = [bool]$nativeFragment.present
        native = [ordered]@{
            bytes = [int]$nativeFragment.bytes.Length
            sha256 = $nativeHash
        }
        after_candidate = [ordered]@{
            bytes = [int]$afterFragment.bytes.Length
            sha256 = $afterHash
        }
    })
}
Assert-Condition ($selectedPositiveCount -eq $ExpectedSelectedPositiveCount) (
    "Selected positive fragments: $selectedPositiveCount; " +
    "expected $ExpectedSelectedPositiveCount"
)
Assert-Condition ($selectedAbsentCount -eq $ExpectedSelectedAbsentCount) (
    "Selected absent fragments: $selectedAbsentCount; " +
    "expected $ExpectedSelectedAbsentCount"
)

$rawKindNames = [ordered]@{
    '0' = 'standard-or-object-reference'
    '1' = 'register-open-by-recorder'
    '2' = 'create-based-on'
    '3' = 'open-by-value-or-command'
    '4' = 'catalog-or-register-open-by-value'
    '5' = 'register-open-by-value'
}
$rawKindCounts = @{}
$rawProbeCount = 0
$rawItemCount = 0
$seenRawFiles = [Collections.Generic.HashSet[string]]::new(
    [StringComparer]::OrdinalIgnoreCase
)
foreach ($rawRoot in $rawLayoutRootsNormalized) {
    $rawFiles = @(
        Get-ChildItem -LiteralPath $rawRoot -File -Recurse |
            Where-Object { $_.Name.EndsWith('.0__part0.txt', [StringComparison]::Ordinal) }
    )
    Assert-Condition ($rawFiles.Count -gt 0) "No form raw layouts under: $rawRoot"
    foreach ($rawFile in $rawFiles) {
        $rawPath = Get-NormalizedAbsolutePath $rawFile.FullName
        Assert-Condition ($seenRawFiles.Add($rawPath)) "Duplicate raw probe: $rawPath"
        $rawProbeCount++
        $rootFields = @(
            Split-1CBracedFields `
                -Text ([IO.File]::ReadAllText($rawPath)) `
                -Context "form root in raw probe $rawProbeCount"
        )
        Assert-Condition ($rootFields.Count -ge 7) (
            "Form root is too short in raw probe $rawProbeCount"
        )
        Assert-Condition ($rootFields[0].Trim() -eq '4') (
            "Unexpected form root marker in raw probe $rawProbeCount"
        )
        $navigationFields = @(
            Split-1CBracedFields `
                -Text $rootFields[6] `
                -Context "NavigationPanel container in raw probe $rawProbeCount"
        )
        Assert-Condition ($navigationFields.Count -ge 2) (
            "NavigationPanel container is too short in raw probe $rawProbeCount"
        )
        Assert-Condition ($navigationFields[0].Trim() -eq '0') (
            "Unexpected NavigationPanel wrapper in raw probe $rawProbeCount"
        )
        $declaredItemCount = 0
        Assert-Condition (
            [int]::TryParse($navigationFields[1].Trim(), [ref]$declaredItemCount)
        ) "Invalid NavigationPanel item count in raw probe $rawProbeCount"
        Assert-Condition ($declaredItemCount -eq $navigationFields.Count - 2) (
            "NavigationPanel declared/actual count mismatch in raw probe $rawProbeCount"
        )
        for ($itemIndex = 0; $itemIndex -lt $declaredItemCount; $itemIndex++) {
            $rawItemCount++
            $itemFields = @(
                Split-1CBracedFields `
                    -Text $navigationFields[$itemIndex + 2] `
                    -Context (
                        "NavigationPanel item $itemIndex in raw probe $rawProbeCount"
                    )
            )
            Assert-Condition ($itemFields.Count -eq 9) (
                "Unexpected NavigationPanel item width in raw probe $rawProbeCount"
            )
            Assert-Condition ($itemFields[0].Trim() -eq '3') (
                "Unexpected NavigationPanel item wrapper in raw probe $rawProbeCount"
            )
            Assert-Condition ($itemFields[4].Trim() -in @('0', '1')) (
                "Unexpected NavigationPanel item type in raw probe $rawProbeCount"
            )
            Assert-Condition ($itemFields[7].Trim() -in @('0', '1')) (
                "Unexpected NavigationPanel visibility flag in raw probe $rawProbeCount"
            )
            $commandFields = @(
                Split-1CBracedFields `
                    -Text $itemFields[2] `
                    -Context (
                        "NavigationPanel command $itemIndex in raw probe $rawProbeCount"
                    )
            )
            Assert-Condition ($commandFields.Count -ge 1) (
                "Empty NavigationPanel command in raw probe $rawProbeCount"
            )
            $kind = $commandFields[0].Trim()
            Assert-Condition ($rawKindNames.Contains($kind)) (
                "Unmatched NavigationPanel command kind '$kind' in raw probe $rawProbeCount"
            )
            if ($kind -eq '0') {
                Assert-Condition ($commandFields.Count -in @(1, 2)) (
                    "Unexpected command width for kind '0' in raw probe $rawProbeCount"
                )
            }
            else {
                Assert-Condition ($commandFields.Count -eq 2) (
                    "Unexpected command width for kind '$kind' in raw probe $rawProbeCount"
                )
            }
            if (-not $rawKindCounts.ContainsKey($kind)) {
                $rawKindCounts[$kind] = 0
            }
            $rawKindCounts[$kind] = [int]$rawKindCounts[$kind] + 1
        }
    }
}
Assert-Condition ($rawProbeCount -eq $ExpectedRawProbeCount) (
    "Raw probe count is $rawProbeCount; expected $ExpectedRawProbeCount"
)
Assert-Condition ($rawItemCount -eq $ExpectedRawItemCount) (
    "Raw NavigationPanel item count is $rawItemCount; expected $ExpectedRawItemCount"
)
foreach ($expectedKind in $ExpectedRawKinds) {
    Assert-Condition ($rawKindCounts.ContainsKey($expectedKind)) (
        "Expected raw command kind '$expectedKind' was not observed"
    )
}
$unexpectedObservedKinds = @(
    $rawKindCounts.Keys | Where-Object { $_ -notin $ExpectedRawKinds }
)
Assert-Condition ($unexpectedObservedKinds.Count -eq 0) (
    "Unexpected observed raw command kinds: $($unexpectedObservedKinds -join ', ')"
)

$rawKindResults = @(
    $rawKindCounts.GetEnumerator() |
        Sort-Object { [int]$_.Key } |
        ForEach-Object {
            [pscustomobject][ordered]@{
                kind = [string]$_.Key
                classification = [string]$rawKindNames[[string]$_.Key]
                count = [int]$_.Value
            }
        }
)

$report = [ordered]@{
    schema_version = 1
    status = 'passed'
    issue = 242
    inputs = [ordered]@{
        canonical_diff = [ordered]@{
            path = $canonicalDiffPathNormalized
            sha256 = Get-FileSha256 $canonicalDiffPathNormalized
        }
        native = [ordered]@{
            root = $nativeRootNormalized
            run_id = $NativeRunId
        }
        baseline_candidate = [ordered]@{
            root = $baselineCandidateRootNormalized
            run_id = $BaselineRunId
        }
        after_candidate = [ordered]@{
            root = $afterCandidateRootNormalized
            run_id = $AfterRunId
            commit = $AfterCommit.ToLowerInvariant()
        }
        raw_layout_roots = $rawLayoutRootsNormalized
    }
    baseline_navigation_panel = [ordered]@{
        changed_form_pairs = $changedForms.Count
        actual_missing_multiset_scope = 'changed forms with left-only NavigationPanel Items'
        left_only_item_signature_count = $leftOnlyItemSignatures
        actual_missing_command_multiset_count = $actualMissingCommandMultiset
        files_with_left_only_items = $leftOnlyFiles.Count
        files_with_missing_commands = $missingCommandFiles.Count
    }
    raw_layout_probe = [ordered]@{
        probe_files = $rawProbeCount
        navigation_panel_items = $rawItemCount
        command_kinds = $rawKindResults
    }
    selected_fragments = $selectedResults.ToArray()
    assertions = [ordered]@{
        baseline_counts_match = $true
        raw_layout_is_well_formed_and_fully_classified = $true
        selected_native_after_fragments_are_byte_exact = $true
        selected_positive_count = $selectedPositiveCount
        selected_absent_count = $selectedAbsentCount
    }
}

$json = $report | ConvertTo-Json -Depth 20
$markdown = [Collections.Generic.List[string]]::new()
$markdown.Add('# NavigationPanel: evidence для #242')
$markdown.Add('')
$markdown.Add(
    'Статус: **PASS**. Анализатор читает входные выгрузки и raw-layouts; ' +
    'изменяет только явно заданные файлы evidence.'
)
$markdown.Add('')
$markdown.Add('## Входы')
$markdown.Add('')
$markdown.Add('| Набор | Run ID / commit | Путь |')
$markdown.Add('|---|---|---|')
$markdown.Add(
    "| Native | ``$NativeRunId`` | ``$nativeRootNormalized`` |"
)
$markdown.Add(
    "| Candidate до исправления | ``$BaselineRunId`` | " +
    "``$baselineCandidateRootNormalized`` |"
)
$markdown.Add(
    "| Candidate после исправления (выборочный) | ``$AfterRunId`` / " +
    "``$($AfterCommit.ToLowerInvariant())`` | ``$afterCandidateRootNormalized`` |"
)
$markdown.Add(
    "| Canonical diff | SHA256 ``$(Get-FileSha256 $canonicalDiffPathNormalized)`` | " +
    "``$canonicalDiffPathNormalized`` |"
)
$markdown.Add(
    "| Raw probes | $rawProbeCount файла | " +
    "``$($rawLayoutRootsNormalized -join '``; ``')`` |"
)
$markdown.Add('')
$markdown.Add('## Измерения')
$markdown.Add('')
$markdown.Add(
    "- Изменённых пар Form.xml: **$($changedForms.Count)**."
)
$markdown.Add(
    "- Left-only сигнатур ``Form/CommandInterface/NavigationPanel/Item``: " +
    "**$leftOnlyItemSignatures** в **$($leftOnlyFiles.Count)** файлах."
)
$markdown.Add(
    "- Реально отсутствующих команд по мультимножеству ``Command``: " +
    "**$actualMissingCommandMultiset** в **$($missingCommandFiles.Count)** файлах " +
    'с left-only Item.'
)
$markdown.Add('')
$markdown.Add('## Классификация raw-layout')
$markdown.Add('')
$markdown.Add('| Kind | Класс | Количество |')
$markdown.Add('|---:|---|---:|')
foreach ($kindResult in $rawKindResults) {
    $markdown.Add(
        "| $($kindResult.kind) | ``$($kindResult.classification)`` | " +
        "$($kindResult.count) |"
    )
}
$markdown.Add('')
$markdown.Add(
    "Проверено **$rawItemCount** элементов. Неизвестных, битых или " +
    'неучтённых вариантов нет.'
)
$markdown.Add('')
$markdown.Add('## Выборочная проверка после исправления')
$markdown.Add('')
$markdown.Add('| Относительный путь | Состояние | Native, bytes / SHA256 | After, bytes / SHA256 |')
$markdown.Add('|---|---|---|---|')
foreach ($result in $selectedResults) {
    $state = if ($result.present) { 'совпадает побайтно' } else { 'отсутствует в обоих' }
    $nativeDigest = if ($null -eq $result.native.sha256) {
        '0 / —'
    }
    else {
        "$($result.native.bytes) / ``$($result.native.sha256)``"
    }
    $afterDigest = if ($null -eq $result.after_candidate.sha256) {
        '0 / —'
    }
    else {
        "$($result.after_candidate.bytes) / ``$($result.after_candidate.sha256)``"
    }
    $markdown.Add(
        "| ``$($result.path)`` | $state | $nativeDigest | $afterDigest |"
    )
}
$markdown.Add('')
$markdown.Add(
    'Сохранены только относительные пути, длины и SHA256 фрагментов; ' +
    'XML и UUID объектов в evidence не включены.'
)
$markdown.Add('')
$markdown.Add('## Воспроизведение')
$markdown.Add('')
$markdown.Add('```powershell')
$selectedLiterals = @(
    $SelectedPath | ForEach-Object { ConvertTo-SingleQuotedPowerShellLiteral $_ }
)
$rawRootLiterals = @(
    $rawLayoutRootsNormalized |
        ForEach-Object { ConvertTo-SingleQuotedPowerShellLiteral $_ }
)
$markdown.Add('$selected = @(' + ($selectedLiterals -join ', ') + ')')
$markdown.Add('$rawRoots = @(' + ($rawRootLiterals -join ', ') + ')')
$markdown.Add('& .\scripts\measure-form-navigation-parity.ps1 `')
$markdown.Add(
    '  -CanonicalDiffPath ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $canonicalDiffPathNormalized) + ' `'
)
$markdown.Add(
    '  -NativeRoot ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $nativeRootNormalized) + ' `'
)
$markdown.Add(
    '  -BaselineCandidateRoot ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $baselineCandidateRootNormalized) + ' `'
)
$markdown.Add(
    '  -AfterCandidateRoot ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $afterCandidateRootNormalized) + ' `'
)
$markdown.Add(
    '  -NativeRunId ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $NativeRunId) + ' `'
)
$markdown.Add(
    '  -BaselineRunId ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $BaselineRunId) + ' `'
)
$markdown.Add(
    '  -AfterRunId ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $AfterRunId) + ' `'
)
$markdown.Add(
    '  -AfterCommit ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $AfterCommit.ToLowerInvariant()) + ' `'
)
$markdown.Add('  -SelectedPath $selected -RawLayoutRoot $rawRoots `')
$markdown.Add(
    '  -OutputJson ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $OutputJson) +
    ' `'
)
$markdown.Add(
    '  -OutputMarkdown ' +
    (ConvertTo-SingleQuotedPowerShellLiteral $OutputMarkdown)
)
$markdown.Add('```')
$markdown.Add('')
$markdown.Add(
    'Команда пересчитывает оба счётчика и evidence. Для проверки уже ' +
    'зафиксированных файлов добавьте ``-VerifyOnly``; любое расхождение ' +
    'или необработанный raw-элемент завершает процесс с ненулевым кодом.'
)

Write-EvidenceFile `
    -Path $OutputJson `
    -Content $json `
    -Verify ([bool]$VerifyOnly)
Write-EvidenceFile `
    -Path $OutputMarkdown `
    -Content ($markdown -join [Environment]::NewLine) `
    -Verify ([bool]$VerifyOnly)

Write-Host (
    "PASS: left-only=$leftOnlyItemSignatures, " +
    "missing-multiset=$actualMissingCommandMultiset, " +
    "raw-items=$rawItemCount, selected=$($selectedResults.Count)"
)
