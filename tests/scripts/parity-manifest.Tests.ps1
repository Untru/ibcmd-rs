$ErrorActionPreference = "Stop"

Describe "Parity protocol scripts" {
    $repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $runner = Join-Path $repo "scripts\export-ibcmd-vs-ours.ps1"
    $matrix = Join-Path $repo "scripts\run-parity-matrix.ps1"

    It "parses under PowerShell without executing an export" {
        foreach ($path in @($runner, $matrix)) {
            $tokens = $null; $errors = $null
            [void][System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors)
            @($errors).Count | Should Be 0
        }
    }

    It "uses immutable native/candidate layout and redacts the password" {
        $source = Get-Content -Raw $runner
        ($source -match 'Join-Path \$runRoot "native"') | Should Be $true
        ($source -match 'Join-Path \$runRoot "candidate_dump"') | Should Be $true
        ($source -match 'Join-Path \$runRoot "candidate"') | Should Be $true
        ($source -match 'Run directory already exists and is immutable') | Should Be $true
        ($source -match 'password_source="env:IBCMD_DB_PSW \(redacted\)"') | Should Be $true
        ($source -match '--sql-pwd\s+\$env:IBCMD_DB_PSW') | Should Be $false
    }

    It "runs UT and BSP and labels scoped output as diagnostic" {
        $source = Get-Content -Raw $matrix
        ($source -match 'id="ut"') | Should Be $true
        ($source -match 'id="bsp"') | Should Be $true
        ($source -match '\[string\]\$BspDbName = "bsp"') | Should Be $true
        ($source -match 'source-diff-matrix-merge') | Should Be $true
    }

    It "uses the CLI matrix commands instead of writing a summary-only matrix" {
        $source = Get-Content -Raw $runner
        ($source -match 'Matrix = "source-diff-matrix"') | Should Be $true
        ($source -match 'MatrixMerge = "source-diff-matrix-merge"') | Should Be $true
        ($source -match 'Invoke-ParityStep -Name "parity-matrix"') | Should Be $true
        ($source -match '"--require-complete-root-metadata"') | Should Be $true
        ($source -match 'summary=\$summary') | Should Be $false
    }

    It "rejects inconsistent scope before creating a run directory" {
        $lab = Join-Path $TestDrive "scope"
        & powershell -NoProfile -ExecutionPolicy Bypass -File $runner -DbName test -LabRoot $lab -RunId valid_full -Scope full -PathPrefix "Catalogs" *> $null
        $LASTEXITCODE | Should Not Be 0
        & powershell -NoProfile -ExecutionPolicy Bypass -File $runner -DbName test -LabRoot $lab -RunId valid_scoped -Scope scoped *> $null
        $LASTEXITCODE | Should Not Be 0
        & powershell -NoProfile -ExecutionPolicy Bypass -File $matrix -LabRoot $lab -RunId strict_scoped -Scope scoped -PathPrefix "Catalogs" -RequireCompleteRootMetadata *> $null
        $LASTEXITCODE | Should Not Be 0
        (Test-Path $lab) | Should Be $false
    }

    It "rejects unsafe RunId before creating a run directory" {
        $lab = Join-Path $TestDrive "runid"
        & powershell -NoProfile -ExecutionPolicy Bypass -File $runner -DbName test -LabRoot $lab -RunId "../escape" *> $null
        $LASTEXITCODE | Should Not Be 0
        & powershell -NoProfile -ExecutionPolicy Bypass -File $matrix -LabRoot $lab -RunId "bad\\path" *> $null
        $LASTEXITCODE | Should Not Be 0
        (Test-Path $lab) | Should Be $false
    }

    It "rejects empty scoped prefixes in the single-database exporter before writes" {
        foreach ($prefix in @("", "   ")) {
            $lab = Join-Path $TestDrive ("export-prefix-" + [guid]::NewGuid().ToString("N"))
            $thrown = $false
            $message = ""
            try { & $runner -DbName test -LabRoot $lab -RunId valid_scoped -Scope scoped -PathPrefix $prefix }
            catch { $thrown = $true; $message = $_.Exception.Message }
            $thrown | Should Be $true
            ($message -match 'PathPrefix') | Should Be $true
            (Test-Path $lab) | Should Be $false
        }
    }

    It "rejects empty scoped prefixes in the two-database orchestrator before writes" {
        foreach ($prefix in @("", "   ")) {
            $lab = Join-Path $TestDrive ("matrix-prefix-" + [guid]::NewGuid().ToString("N"))
            $thrown = $false
            $message = ""
            try { & $matrix -LabRoot $lab -RunId valid_scoped -Scope scoped -PathPrefix $prefix }
            catch { $thrown = $true; $message = $_.Exception.Message }
            $thrown | Should Be $true
            ($message -match 'PathPrefix') | Should Be $true
            (Test-Path $lab) | Should Be $false
        }
    }
}
