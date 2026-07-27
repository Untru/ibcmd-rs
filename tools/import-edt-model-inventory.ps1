param(
    [Parameter(Mandatory = $true)]
    [string]$InputInventory,

    [Parameter(Mandatory = $true)]
    [string]$OutputInventory,

    [string]$EdtRelease = "2025.2.3+30"
)

$ErrorActionPreference = "Stop"

$source = Get-Content -LiteralPath $InputInventory -Raw | ConvertFrom-Json
$bundles = @(
    foreach ($entry in $source) {
        $jarName = [System.IO.Path]::GetFileName([string]$entry.jar)
        $prefix = [string]$entry.bundle + "_"
        $version = if ($jarName.StartsWith($prefix) -and $jarName.EndsWith(".jar")) {
            $jarName.Substring($prefix.Length, $jarName.Length - $prefix.Length - 4)
        } else {
            $null
        }

        [ordered]@{
            symbolicName = [string]$entry.bundle
            version = $version
            modelTypes = @($entry.modelTypes | Sort-Object -Unique)
            importers = @($entry.importers | Sort-Object -Unique)
            exporters = @($entry.exporters | Sort-Object -Unique)
        }
    }
)
$bundles = @($bundles | Sort-Object { $_.symbolicName })

$modelTypeCount = @($bundles | ForEach-Object { $_.modelTypes }).Count
$importerCount = @($bundles | ForEach-Object { $_.importers }).Count
$exporterCount = @($bundles | ForEach-Object { $_.exporters }).Count

$snapshot = [ordered]@{
    schemaVersion = 1
    source = [ordered]@{
        product = "1C:EDT"
        release = $EdtRelease
        derivation = "class-name inventory; no JAR, bytecode, native library, or absolute path"
    }
    summary = [ordered]@{
        bundles = $bundles.Count
        modelTypes = $modelTypeCount
        importers = $importerCount
        exporters = $exporterCount
    }
    bundles = $bundles
}

$json = $snapshot | ConvertTo-Json -Depth 8
$json = $json.Replace("`r`n", "`n") + "`n"
$outputPath = [System.IO.Path]::GetFullPath($OutputInventory)
$parent = [System.IO.Path]::GetDirectoryName($outputPath)
[System.IO.Directory]::CreateDirectory($parent) | Out-Null
[System.IO.File]::WriteAllText(
    $outputPath,
    $json,
    [System.Text.UTF8Encoding]::new($false)
)

Write-Output "Wrote $outputPath"
Write-Output "bundles=$($bundles.Count) modelTypes=$modelTypeCount importers=$importerCount exporters=$exporterCount"
