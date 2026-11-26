param(
    [Parameter(Mandatory = $true)]
    [string]$InputInventory,

    [Parameter(Mandatory = $true)]
    [string]$OutputFeatures,

    [string]$EdtRelease = "2025.2.3+30"
)

$ErrorActionPreference = "Stop"

function Parse-JavapString {
    param(
        [string[]]$Lines,
        [string]$Name
    )

    $pattern = '^\s+public static final java\.lang\.String ' +
        [regex]::Escape($Name) + ' = "(.*)";$'
    foreach ($line in $Lines) {
        if ($line -match $pattern) {
            return $Matches[1]
        }
    }
    return $null
}

$inventory = Get-Content -LiteralPath $InputInventory -Raw | ConvertFrom-Json
$packages = [System.Collections.Generic.List[object]]::new()

foreach ($bundle in ($inventory | Sort-Object bundle)) {
    $packageClasses = @(
        $bundle.modelTypes |
            Where-Object {
                $_ -match '\.[A-Za-z0-9]+Package$' -and
                $_ -notmatch '\.impl\.'
            } |
            Sort-Object -Unique
    )

    foreach ($packageClass in $packageClasses) {
        $lines = @(& javap -classpath ([string]$bundle.jar) -constants $packageClass 2>&1)
        if ($LASTEXITCODE -ne 0) {
            throw "javap failed for $packageClass in $($bundle.bundle): $($lines -join "`n")"
        }

        $integerConstants = [ordered]@{}
        foreach ($line in $lines) {
            if ($line -match '^\s+public static final int ([A-Z0-9_]+) = (-?\d+);$') {
                $integerConstants[$Matches[1]] = [int]$Matches[2]
            }
        }

        $classifierTokens = @(
            $integerConstants.Keys |
                Where-Object {
                    $_ -notmatch '__' -and
                    $_ -notmatch '_(FEATURE|OPERATION)_COUNT$'
                } |
                Sort-Object { $integerConstants[$_] }, { $_ }
        )

        $classifiers = [System.Collections.Generic.List[object]]::new()
        foreach ($token in $classifierTokens) {
            $featurePrefix = $token + "__"
            $operationPrefix = $token + "___"
            $features = [System.Collections.Generic.List[object]]::new()
            $operations = [System.Collections.Generic.List[object]]::new()

            foreach ($name in $integerConstants.Keys) {
                if ($name.StartsWith($operationPrefix)) {
                    $operations.Add([ordered]@{
                        token = $name.Substring($operationPrefix.Length)
                        id = $integerConstants[$name]
                    })
                } elseif ($name.StartsWith($featurePrefix)) {
                    $features.Add([ordered]@{
                        token = $name.Substring($featurePrefix.Length)
                        id = $integerConstants[$name]
                    })
                }
            }

            $featureCountName = $token + "_FEATURE_COUNT"
            $operationCountName = $token + "_OPERATION_COUNT"
            $classifiers.Add([ordered]@{
                token = $token
                id = $integerConstants[$token]
                featureCount = if ($integerConstants.Contains($featureCountName)) {
                    $integerConstants[$featureCountName]
                } else {
                    $null
                }
                operationCount = if ($integerConstants.Contains($operationCountName)) {
                    $integerConstants[$operationCountName]
                } else {
                    $null
                }
                features = @($features | Sort-Object id, token)
                operations = @($operations | Sort-Object id, token)
            })
        }

        $packages.Add([ordered]@{
            bundle = [string]$bundle.bundle
            packageClass = [string]$packageClass
            name = Parse-JavapString -Lines $lines -Name "eNAME"
            namespaceUri = Parse-JavapString -Lines $lines -Name "eNS_URI"
            namespacePrefix = Parse-JavapString -Lines $lines -Name "eNS_PREFIX"
            classifiers = @($classifiers)
        })
    }
}

$classifierCount = @($packages | ForEach-Object { $_.classifiers }).Count
$featureCount = @(
    $packages |
        ForEach-Object { $_.classifiers } |
        ForEach-Object { $_.features }
).Count
$operationCount = @(
    $packages |
        ForEach-Object { $_.classifiers } |
        ForEach-Object { $_.operations }
).Count

$snapshot = [ordered]@{
    schemaVersion = 1
    source = [ordered]@{
        product = "1C:EDT"
        release = $EdtRelease
        derivation = "public EPackage integer constants extracted with javap; no method code or binary artifact"
    }
    summary = [ordered]@{
        packages = $packages.Count
        classifiers = $classifierCount
        features = $featureCount
        operations = $operationCount
    }
    packages = @($packages | Sort-Object packageClass)
}

$json = $snapshot | ConvertTo-Json -Depth 12
$json = $json.Replace("`r`n", "`n") + "`n"
$outputPath = [System.IO.Path]::GetFullPath($OutputFeatures)
$parent = [System.IO.Path]::GetDirectoryName($outputPath)
[System.IO.Directory]::CreateDirectory($parent) | Out-Null
[System.IO.File]::WriteAllText(
    $outputPath,
    $json,
    [System.Text.UTF8Encoding]::new($false)
)

Write-Output "Wrote $outputPath"
Write-Output "packages=$($packages.Count) classifiers=$classifierCount features=$featureCount operations=$operationCount"
