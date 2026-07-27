[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$NativeRoot,
    [Parameter(Mandatory = $true)][string]$EdtRoot,
    [Parameter(Mandatory = $true)][string]$OursRoot,
    [Parameter(Mandatory = $true)][string]$SourceVersion,
    [Parameter(Mandatory = $true)][string]$NativeToolVersion,
    [Parameter(Mandatory = $true)][string]$EdtToolVersion,
    [Parameter(Mandatory = $true)][string]$OursToolVersion,
    [Parameter(Mandatory = $true)][string]$Output,
    [Parameter(Mandatory = $true)][string]$Markdown,
    [string]$ExePath = "",
    [ValidateRange(1, 10000000)][int]$MaxFiles = 100000,
    [ValidateRange(1, [Int64]::MaxValue)][Int64]$MaxTotalBytes = 4294967296,
    [ValidateRange(1, [Int64]::MaxValue)][Int64]$MaxFileBytes = 536870912
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-ExistingDirectory([string]$Value, [string]$Name) {
    $resolved = [IO.Path]::GetFullPath($Value)
    if (-not (Test-Path -LiteralPath $resolved -PathType Container)) {
        throw "$Name must name an existing source-tree directory: $resolved"
    }
    return $resolved
}

function Resolve-OutputPath([string]$Value, [string]$Name) {
    $resolved = [IO.Path]::GetFullPath($Value)
    if (Test-Path -LiteralPath $resolved) { throw "$Name already exists and will not be overwritten: $resolved" }
    return $resolved
}

$repoRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($ExePath)) { $ExePath = Join-Path $repoRoot 'target\release\ibcmd-rs.exe' }
$ExePath = [IO.Path]::GetFullPath($ExePath)
if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) { throw "Missing executable: $ExePath" }

$native = Resolve-ExistingDirectory $NativeRoot 'NativeRoot'
$edt = Resolve-ExistingDirectory $EdtRoot 'EdtRoot'
$ours = Resolve-ExistingDirectory $OursRoot 'OursRoot'
$outputPath = Resolve-OutputPath $Output 'Output'
$markdownPath = Resolve-OutputPath $Markdown 'Markdown'
if ($outputPath -eq $markdownPath) { throw 'Output and Markdown must be different paths.' }
foreach ($root in @($native, $edt, $ours)) {
    if ($outputPath.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
        $markdownPath.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Reports must be outside immutable input tree: $root"
    }
}

# This is deliberately a read-only wrapper.  It does not locate or launch EDT/JVM;
# it only compares the three already-produced trees and records caller-supplied versions.
& $ExePath source-three-way-oracle `
    --native $native --edt $edt --ours $ours `
    --source-version $SourceVersion `
    --native-tool-version $NativeToolVersion `
    --edt-tool-version $EdtToolVersion `
    --ours-tool-version $OursToolVersion `
    --max-files $MaxFiles --max-total-bytes $MaxTotalBytes --max-file-bytes $MaxFileBytes `
    --output $outputPath --markdown $markdownPath
if ($LASTEXITCODE -ne 0) { throw "source-three-way-oracle failed with exit code $LASTEXITCODE" }
