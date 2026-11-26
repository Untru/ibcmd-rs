<#
.SYNOPSIS
Extracts declarative feature semantics from EDT Xcore resources.

.DESCRIPTION
The importer reads the JAR locations from a raw EDT model inventory and streams
model/*.xcore entries with tar. It writes only portable, derived metadata; source
text, archive bytes, class bytes, and absolute paths are never copied to output.

The default scope is the first form-model vertical slice. Use -Scope with one or
more portable resource names/wildcards and -Bundle with exact symbolic names or
wildcards to expand the research import.

.EXAMPLE
.\tools\import-edt-xcore-semantics.ps1 `
    -InputInventory 'C:\research\edt-models\inventory.json' `
    -OutputSemantics "$env:TEMP\edt-form-semantics.json"

.EXAMPLE
.\tools\import-edt-xcore-semantics.ps1 `
    -InputInventory 'C:\research\edt-models\inventory.json' `
    -OutputSemantics "$env:TEMP\edt-all-xcore-semantics.json" `
    -Bundle '*' -Scope 'model/*.xcore'
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$InputInventory,

    [Parameter(Mandatory = $true)]
    [Alias("OutputCorpus", "OutputJson", "OutputPath")]
    [string]$OutputSemantics,

    [string[]]$Scope = @("model/Form.xcore"),

    [string[]]$Bundle = @("com._1c.g5.v8.dt.form.model"),

    [Alias("RejectReportPath")]
    [string]$RejectReport,

    [Alias("Release")]
    [string]$EdtRelease = "2025.2.3+30"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-DelimiterDelta {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string]$Text,

        [Parameter(Mandatory = $true)]
        [char]$Open,

        [Parameter(Mandatory = $true)]
        [char]$Close
    )

    $delta = 0
    $quote = [char]0
    $escaped = $false

    foreach ($character in $Text.ToCharArray()) {
        if ($quote -ne [char]0) {
            if ($escaped) {
                $escaped = $false
            } elseif ($character -eq '\') {
                $escaped = $true
            } elseif ($character -eq $quote) {
                $quote = [char]0
            }
            continue
        }

        if ($character -eq '"' -or $character -eq "'") {
            $quote = $character
        } elseif ($character -eq $Open) {
            $delta++
        } elseif ($character -eq $Close) {
            $delta--
        }
    }

    return $delta
}

function Remove-XcoreComments {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text,

        [Parameter(Mandatory = $true)]
        [string]$Resource
    )

    $builder = [System.Text.StringBuilder]::new($Text.Length)
    $state = "code"
    $quote = [char]0
    $escaped = $false
    $index = 0

    :sourceCharacters while ($index -lt $Text.Length) {
        $character = $Text[$index]
        $next = if ($index + 1 -lt $Text.Length) {
            $Text[$index + 1]
        } else {
            [char]0
        }

        switch ($state) {
            "line-comment" {
                if ($character -eq "`r" -or $character -eq "`n") {
                    [void]$builder.Append($character)
                    $state = "code"
                } else {
                    [void]$builder.Append(' ')
                }
                $index++
                continue sourceCharacters
            }
            "block-comment" {
                if ($character -eq '*' -and $next -eq '/') {
                    [void]$builder.Append("  ")
                    $index += 2
                    $state = "code"
                    continue sourceCharacters
                }

                if ($character -eq "`r" -or $character -eq "`n") {
                    [void]$builder.Append($character)
                } else {
                    [void]$builder.Append(' ')
                }
                $index++
                continue sourceCharacters
            }
        }

        if ($quote -ne [char]0) {
            [void]$builder.Append($character)
            if ($escaped) {
                $escaped = $false
            } elseif ($character -eq '\') {
                $escaped = $true
            } elseif ($character -eq $quote) {
                $quote = [char]0
            }
            $index++
            continue
        }

        if ($character -eq '"' -or $character -eq "'") {
            $quote = $character
            [void]$builder.Append($character)
            $index++
        } elseif ($character -eq '/' -and $next -eq '/') {
            [void]$builder.Append("  ")
            $index += 2
            $state = "line-comment"
        } elseif ($character -eq '/' -and $next -eq '*') {
            [void]$builder.Append("  ")
            $index += 2
            $state = "block-comment"
        } else {
            [void]$builder.Append($character)
            $index++
        }
    }

    if ($state -eq "block-comment") {
        throw "Unterminated block comment in $Resource"
    }
    if ($quote -ne [char]0) {
        throw "Unterminated string literal in $Resource"
    }

    return $builder.ToString()
}

function ConvertFrom-XcoreStringLiteral {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Literal,

        [Parameter(Mandatory = $true)]
        [string]$Context
    )

    if ($Literal.Length -lt 2 -or $Literal[0] -ne '"' -or
        $Literal[$Literal.Length - 1] -ne '"') {
        return $Literal
    }

    $builder = [System.Text.StringBuilder]::new()
    $index = 1
    while ($index -lt $Literal.Length - 1) {
        $character = $Literal[$index]
        if ($character -ne '\') {
            [void]$builder.Append($character)
            $index++
            continue
        }

        if ($index + 1 -ge $Literal.Length - 1) {
            throw "Invalid escape sequence in $Context"
        }
        $escaped = $Literal[$index + 1]
        switch ($escaped) {
            '"' { [void]$builder.Append('"'); $index += 2 }
            "'" { [void]$builder.Append("'"); $index += 2 }
            '\' { [void]$builder.Append('\'); $index += 2 }
            'b' { [void]$builder.Append("`b"); $index += 2 }
            'f' { [void]$builder.Append("`f"); $index += 2 }
            'n' { [void]$builder.Append("`n"); $index += 2 }
            'r' { [void]$builder.Append("`r"); $index += 2 }
            't' { [void]$builder.Append("`t"); $index += 2 }
            'u' {
                if ($index + 5 -ge $Literal.Length) {
                    throw "Incomplete Unicode escape in $Context"
                }
                $digits = $Literal.Substring($index + 2, 4)
                if ($digits -notmatch '^[0-9A-Fa-f]{4}$') {
                    throw "Invalid Unicode escape \u$digits in $Context"
                }
                [void]$builder.Append([char][Convert]::ToInt32($digits, 16))
                $index += 6
            }
            default {
                throw "Unsupported escape sequence \$escaped in $Context"
            }
        }
    }

    return $builder.ToString()
}

function Read-XcoreTypeToken {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text,

        [Parameter(Mandatory = $true)]
        [string]$Context
    )

    $angleDepth = 0
    $index = 0
    while ($index -lt $Text.Length) {
        $character = $Text[$index]
        if ($character -eq '<') {
            $angleDepth++
        } elseif ($character -eq '>') {
            $angleDepth--
            if ($angleDepth -lt 0) {
                throw "Unbalanced generic type in $Context"
            }
        } elseif ([char]::IsWhiteSpace($character) -and $angleDepth -eq 0) {
            break
        }
        $index++
    }

    if ($angleDepth -ne 0) {
        throw "Unbalanced generic type in $Context"
    }
    if ($index -eq 0 -or $index -eq $Text.Length) {
        return $null
    }

    return [ordered]@{
        token = $Text.Substring(0, $index)
        rest = $Text.Substring($index).TrimStart()
    }
}

function Get-XcoreMultiplicity {
    param(
        [Parameter(Mandatory = $true)]
        [string]$TypeToken,

        [Parameter(Mandatory = $true)]
        [string]$Context
    )

    $modelType = $TypeToken
    $multiplicity = $null
    if ($TypeToken -match '^(?<type>.+)\[(?<multiplicity>[^\]]*)\]$') {
        $modelType = $Matches["type"]
        $multiplicity = $Matches["multiplicity"]
    } elseif ($TypeToken.Contains("[") -or $TypeToken.Contains("]")) {
        throw "Unsupported or unbalanced multiplicity '$TypeToken' in $Context"
    }

    if ([string]::IsNullOrWhiteSpace($modelType)) {
        throw "Missing model type in $Context"
    }

    if ($null -eq $multiplicity) {
        return [ordered]@{
            modelType = $modelType
            lowerBound = 0
            upperBound = 1
        }
    }

    switch ($multiplicity) {
        "" {
            return [ordered]@{
                modelType = $modelType
                lowerBound = 0
                upperBound = $null
            }
        }
        "1" {
            return [ordered]@{
                modelType = $modelType
                lowerBound = 1
                upperBound = 1
            }
        }
        "1..*" {
            return [ordered]@{
                modelType = $modelType
                lowerBound = 1
                upperBound = $null
            }
        }
        "0..*" {
            return [ordered]@{
                modelType = $modelType
                lowerBound = 0
                upperBound = $null
            }
        }
        default {
            if ($multiplicity -match '^(?<bound>\d+)$') {
                $bound = [int]$Matches["bound"]
                return [ordered]@{
                    modelType = $modelType
                    lowerBound = $bound
                    upperBound = $bound
                }
            }
            if ($multiplicity -match '^(?<lower>\d+)\.\.(?<upper>\d+)$') {
                $lower = [int]$Matches["lower"]
                $upper = [int]$Matches["upper"]
                if ($lower -gt $upper) {
                    throw "Invalid Xcore multiplicity '[$multiplicity]' in $Context"
                }
                return [ordered]@{
                    modelType = $modelType
                    lowerBound = $lower
                    upperBound = $upper
                }
            }
            throw "Unsupported Xcore multiplicity '[$multiplicity]' in $Context"
        }
    }
}

function Parse-XcoreFeature {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Line,

        [Parameter(Mandatory = $true)]
        [string]$Resource,

        [Parameter(Mandatory = $true)]
        [int]$LineNumber
    )

    $context = "$Resource`:$LineNumber"
    $working = $Line.Trim()
    $kindMarker = $null
    $qualifiers = [System.Collections.Generic.List[string]]::new()
    $knownModifiers = @(
        "contains",
        "container",
        "refers",
        "transient",
        "unsettable",
        "unique",
        "derived"
    )

    while ($working -match '^(?<modifier>[A-Za-z_][A-Za-z0-9_]*)\s+(?<rest>.+)$' -and
        $knownModifiers -contains $Matches["modifier"]) {
        $modifier = $Matches["modifier"]
        $working = $Matches["rest"].TrimStart()
        if ($modifier -in @("contains", "container", "refers")) {
            if ($null -ne $kindMarker) {
                throw "Conflicting feature kind modifiers in $context"
            }
            $kindMarker = $modifier
            if ($modifier -eq "container") {
                $qualifiers.Add("container")
            }
        } else {
            $qualifiers.Add($modifier)
        }
    }

    $typeParts = Read-XcoreTypeToken -Text $working -Context $context
    if ($null -eq $typeParts) {
        return $null
    }

    $typeToken = [string]$typeParts.token
    $rest = [string]$typeParts.rest
    if ($rest -notmatch '^(?<name>\^?[A-Za-z_][A-Za-z0-9_]*)(?<tail>.*)$') {
        return $null
    }

    $name = $Matches["name"].TrimStart('^')
    $tail = $Matches["tail"]
    if ($tail.TrimStart().StartsWith("(")) {
        return $null
    }

    $allowedTail = $tail.Trim()
    if ($allowedTail -match '^opposite\s+\^?[A-Za-z_][A-Za-z0-9_]*\s*(?<remainder>.*)$') {
        $allowedTail = $Matches["remainder"].Trim()
    }

    $defaultValue = $null
    if ($allowedTail.StartsWith("=")) {
        $defaultMatch = [regex]::Match(
            $allowedTail,
            '^\s*=\s*(?<literal>"(?:\\.|[^"\\])*"|[^\s{}]+)(?<remainder>.*)$'
        )
        if (-not $defaultMatch.Success) {
            throw "Malformed explicit default in $context"
        }
        $literal = $defaultMatch.Groups["literal"].Value
        $defaultValue = ConvertFrom-XcoreStringLiteral -Literal $literal -Context $context
        $allowedTail = $defaultMatch.Groups["remainder"].Value.Trim()
    }

    if ($allowedTail -ne "" -and
        $allowedTail -notmatch '^(?:get|set)\b' -and
        $allowedTail -notmatch '^\{') {
        return $null
    }

    $multiplicity = Get-XcoreMultiplicity -TypeToken $typeToken -Context $context
    $kind = switch ($kindMarker) {
        "contains" { "containment" }
        "container" { "reference" }
        "refers" { "reference" }
        default { "attribute" }
    }

    return [ordered]@{
        name = $name
        kind = $kind
        qualifiers = @($qualifiers | Sort-Object -Unique)
        modelType = [string]$multiplicity.modelType
        lowerBound = [int]$multiplicity.lowerBound
        upperBound = if ($null -eq $multiplicity.upperBound) {
            $null
        } else {
            [int]$multiplicity.upperBound
        }
        defaultValue = $defaultValue
        modelEvidence = [ordered]@{
            kind = "xcore"
            status = "verified"
            sources = @($Resource)
            note = "structural feature declaration in $Resource"
        }
        xml = [ordered]@{
            qname = $null
            order = $null
            emitDefault = $null
            versionGate = [ordered]@{
                status = "pending"
            }
            delegate = [ordered]@{
                status = "pending"
            }
            evidence = [ordered]@{
                kind = "writer-inspection"
                status = "pending"
                note = "XML behaviour has not been verified"
            }
        }
    }
}

function Parse-XcoreResource {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text,

        [Parameter(Mandatory = $true)]
        [string]$BundleName,

        [Parameter(Mandatory = $true)]
        [string]$Resource
    )

    $clean = Remove-XcoreComments -Text $Text -Resource $Resource

    $packageMatches = [regex]::Matches(
        $clean,
        '(?m)^\s*package\s+(?<name>\^?[A-Za-z_][A-Za-z0-9_]*' +
            '(?:\.\^?[A-Za-z_][A-Za-z0-9_]*)*)\s*$'
    )
    if ($packageMatches.Count -ne 1) {
        throw "$Resource must contain exactly one package declaration; found $($packageMatches.Count)"
    }
    $packageName = $packageMatches[0].Groups["name"].Value.Replace("^", "")

    $ecoreMatches = [regex]::Matches($clean, '(?s)@Ecore\s*\((?<arguments>.*?)\)')
    if ($ecoreMatches.Count -ne 1) {
        throw "$Resource must contain exactly one @Ecore annotation; found $($ecoreMatches.Count)"
    }
    $namespaceMatches = [regex]::Matches(
        $ecoreMatches[0].Groups["arguments"].Value,
        '\bnsURI\s*=\s*(?<literal>"(?:\\.|[^"\\])*")'
    )
    if ($namespaceMatches.Count -ne 1) {
        throw "$Resource @Ecore annotation must contain exactly one nsURI"
    }
    $namespaceUri = ConvertFrom-XcoreStringLiteral `
        -Literal $namespaceMatches[0].Groups["literal"].Value `
        -Context "$Resource @Ecore(nsURI)"
    if ([string]::IsNullOrWhiteSpace($namespaceUri)) {
        throw "$Resource has an empty @Ecore nsURI"
    }

    $classifiers = [System.Collections.Generic.List[object]]::new()
    $lines = $clean -split "\r?\n"
    $currentClassifier = $null
    $braceDepth = 0
    $annotationParenDepth = 0
    $operationParenDepth = 0
    $dataTypeHook = $null
    $dataTypeHookDepth = 0
    $dataTypeHooksAllowed = $false

    for ($lineIndex = 0; $lineIndex -lt $lines.Count; $lineIndex++) {
        $lineNumber = $lineIndex + 1
        $line = $lines[$lineIndex]
        $trimmed = $line.Trim()

        if ($null -ne $dataTypeHook) {
            $hookBraceDelta = Get-DelimiterDelta -Text $line -Open '{' -Close '}'
            if ($dataTypeHookDepth -eq 0) {
                if ($trimmed -eq "") {
                    continue
                }
                if ($trimmed -ne "{") {
                    throw "Expected datatype $dataTypeHook body in " +
                        "$Resource`:$lineNumber"
                }
            }
            $dataTypeHookDepth += $hookBraceDelta
            if ($dataTypeHookDepth -lt 0) {
                throw "Unbalanced datatype $dataTypeHook body in " +
                    "$Resource`:$lineNumber"
            }
            if ($dataTypeHookDepth -eq 0) {
                $dataTypeHook = $null
            }
            continue
        }

        if ($annotationParenDepth -gt 0) {
            $annotationParenDepth += Get-DelimiterDelta `
                -Text $line -Open '(' -Close ')'
            if ($annotationParenDepth -lt 0) {
                throw "Unbalanced annotation parentheses in $Resource`:$lineNumber"
            }
            continue
        }

        if ($operationParenDepth -gt 0) {
            $operationParenDepth += Get-DelimiterDelta `
                -Text $line -Open '(' -Close ')'
            if ($operationParenDepth -lt 0) {
                throw "Unbalanced operation parentheses in $Resource`:$lineNumber"
            }
            $braceDepth += Get-DelimiterDelta -Text $line -Open '{' -Close '}'
            if ($braceDepth -lt 0) {
                throw "Unbalanced braces in $Resource`:$lineNumber"
            }
            continue
        }

        if ($trimmed.StartsWith("@")) {
            $annotationParenDepth = Get-DelimiterDelta `
                -Text $line -Open '(' -Close ')'
            if ($annotationParenDepth -lt 0) {
                throw "Unbalanced annotation parentheses in $Resource`:$lineNumber"
            }
            continue
        }

        if ($null -eq $currentClassifier) {
            if ($trimmed -eq "" -or
                $trimmed -match '^(?:package|import)\b' -or
                $trimmed -match '^annotation\s+"(?:\\.|[^"\\])*"\s+as\s+[A-Za-z_][A-Za-z0-9_]*$') {
                continue
            }

            $dataTypeMatch = [regex]::Match(
                $trimmed,
                '^type\s+(?<name>\^?[A-Za-z_][A-Za-z0-9_]*)\s+wraps\s+' +
                    '[A-Za-z_$][A-Za-z0-9_.$]*(?:<[^{}]+>)?(?:\[\])*$'
            )
            if ($dataTypeMatch.Success) {
                $classifiers.Add([ordered]@{
                    name = $dataTypeMatch.Groups["name"].Value.TrimStart('^')
                    kind = "datatype"
                    features = @()
                })
                $dataTypeHooksAllowed = $true
                continue
            }

            if ($dataTypeHooksAllowed -and
                $trimmed -match '^(?<hook>create|convert)\s*(?<brace>\{)?$') {
                $dataTypeHook = $Matches["hook"]
                $dataTypeHookDepth = Get-DelimiterDelta `
                    -Text $line -Open '{' -Close '}'
                if ($dataTypeHookDepth -lt 0 -or $dataTypeHookDepth -gt 1) {
                    throw "Unsupported datatype $dataTypeHook body in " +
                        "$Resource`:$lineNumber"
                }
                if ($Matches["brace"] -eq "{" -and $dataTypeHookDepth -eq 0) {
                    $dataTypeHook = $null
                }
                continue
            }

            $classifierMatch = [regex]::Match(
                $trimmed,
                '^(?:(?<abstract>abstract)\s+)?(?<kind>class|interface|enum)\s+' +
                    '(?<name>\^?[A-Za-z_][A-Za-z0-9_]*)(?:<[^{}]+>)?(?:\s+.*)?$'
            )
            if (-not $classifierMatch.Success) {
                throw "Unsupported top-level Xcore syntax in $Resource`:${lineNumber}: $trimmed"
            }
            $dataTypeHooksAllowed = $false

            $classifierKind = $classifierMatch.Groups["kind"].Value
            $currentClassifier = [ordered]@{
                name = $classifierMatch.Groups["name"].Value.TrimStart('^')
                kind = $classifierKind
                features = [System.Collections.Generic.List[object]]::new()
            }
            $classifiers.Add($currentClassifier)
            $braceDepth = Get-DelimiterDelta -Text $line -Open '{' -Close '}'
            if ($braceDepth -lt 0 -or $braceDepth -gt 1) {
                throw "Unsupported classifier braces in $Resource`:$lineNumber"
            }
            continue
        }

        $depthBefore = $braceDepth
        $braceDelta = Get-DelimiterDelta -Text $line -Open '{' -Close '}'

        if ($depthBefore -eq 0) {
            if ($trimmed -eq "") {
                continue
            }
            if ($trimmed -eq "{}") {
                $currentClassifier.features = @(
                    $currentClassifier.features |
                        Sort-Object `
                            { [string]$_.name },
                            { [string]$_.kind },
                            { [string]$_.modelType }
                )
                $currentClassifier = $null
                continue
            }
            $headerFragment = $trimmed
            if ($headerFragment.EndsWith("{")) {
                $headerFragment = $headerFragment.Substring(
                    0,
                    $headerFragment.Length - 1
                ).Trim()
            }
            $validHeaderFragment = $headerFragment -eq "" -or
                $headerFragment -match '^[A-Za-z_$?][A-Za-z0-9_.$?<>,\s]*,?$'
            if (-not $validHeaderFragment -or
                $braceDelta -lt 0 -or $braceDelta -gt 1) {
                throw "Expected classifier body in $Resource`:$lineNumber, found: $trimmed"
            }
            $braceDepth += $braceDelta
            continue
        }

        if ($depthBefore -eq 1 -and $trimmed -ne "" -and $trimmed -ne "}") {
            if ($trimmed -match '^(?:get|set)\b' -or $trimmed -eq "{") {
                $braceDepth += $braceDelta
                continue
            }

            if ($trimmed -match '^(?:(?:readonly|volatile|transient)\s+)*op\b') {
                $operationParenDepth = Get-DelimiterDelta `
                    -Text $line -Open '(' -Close ')'
                if ($operationParenDepth -lt 0) {
                    throw "Unbalanced operation parentheses in $Resource`:$lineNumber"
                }
                $braceDepth += $braceDelta
                continue
            }

            if ($currentClassifier.kind -eq "enum") {
                if ($trimmed -ne "," -and $trimmed -notmatch (
                    '^\^?[A-Za-z_][A-Za-z0-9_]*' +
                    '(?:\s+as\s+"(?:\\.|[^"\\])*")?' +
                    '\s*(?:=\s*-?\d+)?\s*,?$'
                )) {
                    throw "Unsupported enum literal syntax in $Resource`:${lineNumber}: $trimmed"
                }
            } else {
                $feature = Parse-XcoreFeature `
                    -Line $line -Resource $Resource -LineNumber $lineNumber
                if ($null -eq $feature) {
                    throw "Unsupported structural feature syntax in $Resource`:${lineNumber}: $trimmed"
                }
                $currentClassifier.features.Add($feature)
            }
        }

        $braceDepth += $braceDelta
        if ($braceDepth -lt 0) {
            throw "Unbalanced braces in $Resource`:$lineNumber"
        }
        if ($braceDepth -eq 0) {
            $currentClassifier.features = @(
                $currentClassifier.features |
                    Sort-Object `
                        { [string]$_.name },
                        { [string]$_.kind },
                        { [string]$_.modelType }
            )
            $currentClassifier = $null
        }
    }

    if ($null -ne $currentClassifier -or $braceDepth -ne 0) {
        throw "Unterminated classifier body in $Resource"
    }
    if ($annotationParenDepth -ne 0 -or $operationParenDepth -ne 0) {
        throw "Unterminated annotation or operation signature in $Resource"
    }

    $duplicateClassifiers = @(
        $classifiers |
            Group-Object { [string]$_.name } |
            Where-Object Count -gt 1
    )
    if ($duplicateClassifiers.Count -gt 0) {
        throw "Duplicate classifier '$($duplicateClassifiers[0].Name)' in $Resource"
    }
    foreach ($classifier in $classifiers) {
        $duplicateFeatures = @(
            $classifier.features |
                Group-Object { [string]$_.name } |
                Where-Object Count -gt 1
        )
        if ($duplicateFeatures.Count -gt 0) {
            throw "Duplicate feature '$($duplicateFeatures[0].Name)' in " +
                "$Resource classifier $($classifier.name)"
        }
    }

    return [ordered]@{
        bundle = $BundleName
        resource = $Resource
        packageName = $packageName
        namespaceUri = $namespaceUri
        classifiers = @(
            $classifiers |
                Sort-Object { [string]$_.name }, { [string]$_.kind }
        )
    }
}

function New-XcoreRejection {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BundleName,

        [Parameter(Mandatory = $true)]
        [string]$Resource,

        [Parameter(Mandatory = $true)]
        [System.Exception]$Exception
    )

    $message = $Exception.Message
    $lineNumber = $null
    $resourceLinePattern = [regex]::Escape($Resource) + ':(?<line>\d+)'
    if ($message -match $resourceLinePattern) {
        $lineNumber = [int]$Matches["line"]
    }

    $production = "resource"
    $reason = "parser rejected resource"
    switch -Regex ($message) {
        '^Unsupported top-level Xcore syntax' {
            $production = "classifier-declaration"
            $reason = "unsupported top-level declaration"
            break
        }
        '^Unsupported structural feature syntax' {
            $production = "structural-feature"
            $reason = "unsupported structural feature declaration"
            break
        }
        '^Unsupported enum literal syntax' {
            $production = "enum-literal"
            $reason = "unsupported enum literal declaration"
            break
        }
        '^Unsupported Xcore multiplicity' {
            $production = "multiplicity"
            $reason = "unsupported feature multiplicity"
            if ($message -match "multiplicity '(?<value>\[[^']+\])'") {
                $reason += " $($Matches['value'])"
            }
            break
        }
        'package declaration' {
            $production = "package-declaration"
            $reason = "missing or ambiguous package declaration"
            break
        }
        '@Ecore|nsURI' {
            $production = "package-annotation"
            $reason = "missing or ambiguous Ecore namespace annotation"
            break
        }
        'comment|string literal|escape sequence|Unicode escape' {
            $production = "lexical"
            $reason = "unsupported or unterminated lexical construct"
            break
        }
        'annotation parentheses' {
            $production = "annotation"
            $reason = "unsupported or unterminated annotation"
            break
        }
        'operation parentheses' {
            $production = "operation"
            $reason = "unsupported or unterminated operation signature"
            break
        }
        'explicit default' {
            $production = "explicit-default"
            $reason = "unsupported explicit default"
            break
        }
        'generic type|model type' {
            $production = "model-type"
            $reason = "unsupported model type"
            break
        }
        'classifier body|classifier braces|braces|Unterminated classifier' {
            $production = "classifier-body"
            $reason = "unsupported or unbalanced classifier body"
            break
        }
        'Duplicate classifier|Duplicate feature' {
            $production = "semantic-identity"
            $reason = "duplicate local semantic identity"
            break
        }
        '^tar failed to read' {
            $production = "archive-read"
            $reason = "archive tool could not stream selected resource"
            break
        }
    }

    return [ordered]@{
        bundle = $BundleName
        resource = $Resource
        production = $production
        line = $lineNumber
        reason = if ($null -eq $lineNumber) {
            $reason
        } else {
            "$reason at line $lineNumber"
        }
    }
}

if ([string]::IsNullOrWhiteSpace($EdtRelease)) {
    throw "EdtRelease must not be empty"
}
if ($Scope.Count -eq 0 -or $Bundle.Count -eq 0) {
    throw "Scope and Bundle must contain at least one selector"
}
foreach ($selector in $Scope) {
    if ([string]::IsNullOrWhiteSpace($selector) -or
        $selector.Contains('\') -or
        $selector.StartsWith('/') -or
        $selector -match '(^|/)\.\.($|/)') {
        throw "Scope must use portable model-relative paths: '$selector'"
    }
}

$inventoryPath = [System.IO.Path]::GetFullPath($InputInventory)
if (-not [System.IO.File]::Exists($inventoryPath)) {
    throw "Inventory does not exist: $inventoryPath"
}
$inventory = @(Get-Content -LiteralPath $inventoryPath -Raw | ConvertFrom-Json)
if ($inventory.Count -eq 0) {
    throw "Inventory is empty: $inventoryPath"
}

$duplicateBundles = @(
    $inventory |
        Where-Object { $null -ne $_.bundle } |
        Group-Object { [string]$_.bundle } |
        Where-Object Count -gt 1
)
if ($duplicateBundles.Count -gt 0) {
    throw "Raw inventory contains duplicate bundle '$($duplicateBundles[0].Name)'"
}

$selectedBundles = @(
    $inventory |
        Where-Object {
            $entryName = [string]$_.bundle
            @($Bundle | Where-Object { $entryName -like $_ }).Count -gt 0
        } |
        Sort-Object { [string]$_.bundle }
)
if ($selectedBundles.Count -eq 0) {
    throw "No inventory bundles matched: $($Bundle -join ', ')"
}
$unmatchedBundles = @(
    $Bundle |
        Where-Object {
            $selector = $_
            @(
                $inventory |
                    Where-Object { ([string]$_.bundle) -like $selector }
            ).Count -eq 0
        }
)
if ($unmatchedBundles.Count -gt 0) {
    throw "Bundle selectors matched no inventory entries: $($unmatchedBundles -join ', ')"
}

$packages = [System.Collections.Generic.List[object]]::new()
$processedResources = [System.Collections.Generic.List[object]]::new()
$rejectedResources = [System.Collections.Generic.List[object]]::new()
$selectedResourceCount = 0
$matchedScopes = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::Ordinal
)
$releaseSegment = "1c-edt-$EdtRelease-"

foreach ($inventoryEntry in $selectedBundles) {
    $bundleName = [string]$inventoryEntry.bundle
    if ([string]::IsNullOrWhiteSpace($bundleName)) {
        throw "Raw inventory entry has an empty bundle name"
    }
    if ($null -eq $inventoryEntry.jar -or
        [string]::IsNullOrWhiteSpace([string]$inventoryEntry.jar)) {
        throw "Raw inventory entry '$bundleName' has no JAR path"
    }

    $jarPath = [System.IO.Path]::GetFullPath([string]$inventoryEntry.jar)
    if (-not [System.IO.Path]::IsPathRooted([string]$inventoryEntry.jar)) {
        throw "Raw inventory JAR path for '$bundleName' is not absolute"
    }
    if (-not [System.IO.File]::Exists($jarPath)) {
        throw "Inventory JAR does not exist for '$bundleName': $jarPath"
    }
    $jarName = [System.IO.Path]::GetFileName($jarPath)
    if (-not $jarName.StartsWith("$bundleName`_", [System.StringComparison]::Ordinal) -or
        -not $jarName.EndsWith(".jar", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Inventory JAR '$jarName' does not match bundle '$bundleName'"
    }
    if ($jarPath.IndexOf(
            $releaseSegment,
            [System.StringComparison]::OrdinalIgnoreCase
        ) -lt 0) {
        throw "Inventory JAR for '$bundleName' does not prove EDT release " +
            "'$EdtRelease' (expected path segment '$releaseSegment')"
    }

    $archiveEntries = @(& tar -tf $jarPath 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "tar failed to list Xcore resources for '$bundleName': " +
            ($archiveEntries -join "`n")
    }
    $xcoreResources = @(
        $archiveEntries |
            ForEach-Object { ([string]$_).Replace('\', '/') } |
            Where-Object { $_ -match '^model/[A-Za-z0-9_.\-/]+\.xcore$' } |
            Sort-Object -Unique
    )
    if ($xcoreResources.Count -eq 0) {
        continue
    }

    $selectedResources = @(
        $xcoreResources |
            Where-Object {
                $resourceName = $_
                @($Scope | Where-Object { $resourceName -like $_ }).Count -gt 0
            }
    )
    if ($selectedResources.Count -eq 0) {
        continue
    }
    foreach ($resource in $selectedResources) {
        $selectedResourceCount++
        foreach ($selector in $Scope) {
            if ($resource -like $selector) {
                [void]$matchedScopes.Add($selector)
            }
        }

        try {
            $sourceLines = @(& tar -xOf $jarPath $resource 2>&1)
            if ($LASTEXITCODE -ne 0) {
                throw "tar failed to read selected Xcore resource"
            }
            $sourceText = $sourceLines -join "`n"
            $parsedPackage = Parse-XcoreResource `
                -Text $sourceText `
                -BundleName $bundleName `
                -Resource $resource
            $packages.Add($parsedPackage)
            $processedResources.Add([ordered]@{
                bundle = $bundleName
                resource = $resource
                packageName = [string]$parsedPackage.packageName
                namespaceUri = [string]$parsedPackage.namespaceUri
                classifiers = @($parsedPackage.classifiers).Count
                features = @(
                    $parsedPackage.classifiers |
                        ForEach-Object { $_.features }
                ).Count
            })
        } catch {
            $rejectedResources.Add(
                (New-XcoreRejection `
                    -BundleName $bundleName `
                    -Resource $resource `
                    -Exception $_.Exception)
            )
        } finally {
            $sourceLines = $null
            $sourceText = $null
        }
    }
}

$unmatchedScopes = @($Scope | Where-Object { -not $matchedScopes.Contains($_) })
if ($unmatchedScopes.Count -gt 0) {
    throw "Scope selectors matched no Xcore resources: $($unmatchedScopes -join ', ')"
}
if ($selectedResourceCount -eq 0) {
    throw "The selected inventory/scope discovered no Xcore resources"
}
if ($selectedResourceCount -ne
    ($processedResources.Count + $rejectedResources.Count)) {
    throw "Internal resource accounting mismatch"
}

$packages = @(
    $packages |
        Sort-Object `
            { [string]$_.bundle },
            { [string]$_.resource },
            { [string]$_.packageName },
            { [string]$_.namespaceUri }
)
$processedResources = @(
    $processedResources |
        Sort-Object `
            { [string]$_.bundle },
            { [string]$_.resource }
)
$rejectedResources = @(
    $rejectedResources |
        Sort-Object `
            { [string]$_.bundle },
            { [string]$_.resource },
            { [string]$_.production }
)
$classifierCount = @($packages | ForEach-Object { $_.classifiers }).Count
$features = @(
    $packages |
        ForEach-Object { $_.classifiers } |
        ForEach-Object { $_.features }
)
$featureCount = $features.Count
$attributeCount = @($features | Where-Object { $_.kind -eq "attribute" }).Count
$referenceCount = @($features | Where-Object { $_.kind -eq "reference" }).Count
$containmentCount = @($features | Where-Object { $_.kind -eq "containment" }).Count
$defaultCount = @($features | Where-Object { $null -ne $_.defaultValue }).Count

$corpus = [ordered]@{
    schemaVersion = 1
    source = [ordered]@{
        product = "1C:EDT"
        release = $EdtRelease
        derivation = "declarative Xcore feature semantics streamed from inventory-selected JAR resources with tar; no source, archive, class bytes, or absolute paths"
    }
    summary = [ordered]@{
        packages = $packages.Count
        classifiers = $classifierCount
        features = $featureCount
    }
    packages = $packages
}

$json = $corpus | ConvertTo-Json -Depth 16
$json = $json.Replace("`r`n", "`n") + "`n"
if ($json -match '(?i)(?:[A-Z]:\\|\\\\[^\\]|file:/)') {
    throw "Refusing to write non-portable absolute path to the corpus"
}

$outputPath = [System.IO.Path]::GetFullPath($OutputSemantics)
$outputParent = [System.IO.Path]::GetDirectoryName($outputPath)
[System.IO.Directory]::CreateDirectory($outputParent) | Out-Null
[System.IO.File]::WriteAllText(
    $outputPath,
    $json,
    [System.Text.UTF8Encoding]::new($false)
)

Write-Output "Wrote $outputPath"
Write-Output (
    "packages=$($packages.Count) classifiers=$classifierCount features=$featureCount " +
    "attributes=$attributeCount references=$referenceCount " +
    "containments=$containmentCount explicitDefaults=$defaultCount"
)
Write-Output (
    "selectedResources=$selectedResourceCount " +
    "processedResources=$($processedResources.Count) " +
    "rejectedResources=$($rejectedResources.Count)"
)

if (-not [string]::IsNullOrWhiteSpace($RejectReport)) {
    $clusters = @(
        $rejectedResources |
            Group-Object { [string]$_.production } |
            ForEach-Object {
                [ordered]@{
                    production = $_.Name
                    count = $_.Count
                }
            } |
            Sort-Object { [string]$_.production }
    )
    $report = [ordered]@{
        schemaVersion = 1
        source = [ordered]@{
            product = "1C:EDT"
            release = $EdtRelease
            derivation = "deterministic Xcore discovery accounting; no source, archive, class bytes, or absolute paths"
        }
        summary = [ordered]@{
            selected = $selectedResourceCount
            processed = $processedResources.Count
            rejected = $rejectedResources.Count
        }
        clusters = $clusters
        processed = $processedResources
        rejected = $rejectedResources
    }
    $reportJson = $report | ConvertTo-Json -Depth 12
    $reportJson = $reportJson.Replace("`r`n", "`n") + "`n"
    if ($reportJson -match '(?i)(?:[A-Z]:\\|\\\\[^\\]|file:/)') {
        throw "Refusing to write non-portable absolute path to reject report"
    }

    $rejectReportPath = [System.IO.Path]::GetFullPath($RejectReport)
    $rejectReportParent = [System.IO.Path]::GetDirectoryName($rejectReportPath)
    [System.IO.Directory]::CreateDirectory($rejectReportParent) | Out-Null
    [System.IO.File]::WriteAllText(
        $rejectReportPath,
        $reportJson,
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "Wrote $rejectReportPath"
    foreach ($cluster in $clusters) {
        Write-Output "reject.$($cluster.production)=$($cluster.count)"
    }
} elseif ($rejectedResources.Count -gt 0) {
    Write-Warning (
        "$($rejectedResources.Count) resources were rejected; " +
        "use -RejectReport to persist deterministic details"
    )
}
