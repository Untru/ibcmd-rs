# Acceptance checks shared by the S1 combination runners. Every required result
# must be present; empty/missing/truncated output is a failure, not a pass.
function Assert-LabCommand([int]$ExitCode, [string]$Check) {
    if ($ExitCode -ne 0) { throw "$Check failed (exit $ExitCode)" }
}

function Assert-LabNoop([int]$ExitCode, [string]$Path) {
    Assert-LabCommand $ExitCode 'native follow-up apply'
    if (-not (Select-String -LiteralPath $Path -Pattern 'не требуется' -Quiet)) {
        throw 'native follow-up apply did not report a no-op'
    }
}

function Assert-LabExcept([string]$Path, [string[]]$Tables) {
    $expected = @($Tables | Where-Object { $_ } | Sort-Object -Unique)
    if ($expected.Count -eq 0) { throw 'no rebuilt tables were supplied for EXCEPT acceptance' }
    $seen = @{}
    foreach ($line in (Get-Content -LiteralPath $Path -Encoding UTF8)) {
        if (-not $line.Trim()) { continue }
        if ($line -notmatch '^(\S+)\s+rows (\d+) / (\d+)\s+only in A (\d+)\s+only in B (\d+)\s*$') {
            throw "EXCEPT output is incomplete or invalid: $line"
        }
        $table = $Matches[1]
        if ($table -notin $expected -or $seen.ContainsKey($table)) { throw "unexpected/duplicate EXCEPT table: $table" }
        if ($Matches[2] -ne $Matches[3] -or $Matches[4] -ne '0' -or $Matches[5] -ne '0') {
            throw "rebuilt-table data differs: $line"
        }
        $seen[$table] = $true
    }
    if ($seen.Count -ne $expected.Count) { throw 'EXCEPT output does not cover every rebuilt table' }
}

function Assert-LabConfig([string]$Path) {
    $lines = @(Get-Content -LiteralPath $Path -Encoding UTF8 | Where-Object { $_.Trim() })
    if ($lines.Count -ne 2 -or $lines[0] -ne '== only in A (0 rows)' -or $lines[1] -ne '== only in B (0 rows)') {
        throw 'Config EXCEPT acceptance differs or is incomplete'
    }
}

function Assert-LabExport([int]$ExitCode, [string]$Path, [switch]$AllowDifferences) {
    Assert-LabCommand $ExitCode 'source-diff'
    $report = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    $fields = @('left_only', 'right_only', 'different', 'unchanged')
    foreach ($field in $fields) {
        if ($null -eq $report.summary.$field -or $report.summary.$field -lt 0) { throw "source-diff has no valid $field count" }
    }
    if ($report.summary.left_only -ne 0 -or $report.summary.right_only -ne 0 -or $report.summary.unchanged -le 0) {
        throw 'native exports are missing files or have no equal files'
    }
    if (-not $AllowDifferences -and $report.summary.different -ne 0) { throw 'native exports differ' }
    $counts = @{left_only=0; right_only=0; different=0; unchanged=0}
    foreach ($entry in $report.differences) {
        if (-not $counts.ContainsKey($entry.status)) { throw 'source-diff contains an unknown status' }
        $counts[$entry.status]++
    }
    foreach ($field in $fields) {
        if ($counts[$field] -ne $report.summary.$field) { throw "source-diff $field summary disagrees with its entries" }
    }
}
