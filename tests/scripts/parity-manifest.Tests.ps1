$ErrorActionPreference = "Stop"

Describe "Parity protocol scripts" {
    $repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $runner = Join-Path $repo "scripts\export-ibcmd-vs-ours.ps1"
    $matrix = Join-Path $repo "scripts\run-parity-matrix.ps1"
    $fakeCli = Join-Path $repo "tests\fixtures\parity\fake-cli.ps1"

    It "parses under PowerShell without executing an export" {
        foreach ($path in @($runner, $matrix)) {
            $tokens = $null; $errors = $null
            [void][System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors)
            @($errors).Count | Should Be 0
        }
    }

    It "uses immutable native/candidate layout and redacts the password" {
        $source = Get-Content -Raw $runner
        ($source.Contains("Join-Path `$runRoot 'native'")) | Should Be $true
        ($source.Contains("Join-Path `$runRoot 'candidate_dump'")) | Should Be $true
        ($source.Contains("Join-Path `$runRoot 'candidate'")) | Should Be $true
        ($source -match 'Run directory already exists and is immutable') | Should Be $true
        ($source.Contains('password_source=$manifestPasswordSource')) | Should Be $true
        ($source -match '--sql-pwd\s+\$env:IBCMD_DB_PSW') | Should Be $false
        ($source -match 'Write-ManifestAtomic -Path \$manifestPath -Manifest \$manifest') | Should Be $true
        ($source.Contains("[IO.File]::Replace(`$tmp, `$Path, `$backup")) | Should Be $true
        ($source -match 'ConvertTo-SanitizedArguments') | Should Be $true
    }

    It "runs UT and BSP and labels scoped output as diagnostic" {
        $source = Get-Content -Raw $matrix
        ($source -match "id='ut'") | Should Be $true
        ($source -match "id='bsp'") | Should Be $true
        ($source.Contains("[string]`$BspDbName = 'bsp'")) | Should Be $true
        ($source -match 'source-diff-matrix-merge') | Should Be $true
        ($source -match 'Read-ValidChildManifest') | Should Be $true
        ($source -match "native_ibcmd_sha256.*candidate_sha256") | Should Be $true
        ($source -match 'release_eligible = \$false') | Should Be $true
        ($source -match '(?s)\$RequireCompleteRootMetadata.*\$matrixManifest.parity_zero') | Should Be $true
    }

    It "hashes every executable and keeps SQL passwords out of process arguments" {
        $source = Get-Content -Raw $runner
        foreach ($tool in @('git', 'candidate', 'native_ibcmd', 'sqlcmd', 'bcp', 'robocopy')) {
            ($source -match "(?s)tools\.${tool}.*?sha256") | Should Be $true
        }
        ($source -match "@\('-U', \`$SqlUser, '-P'") | Should Be $false
        ($source -match "SetEnvironmentVariable\('SQLCMDPASSWORD'") | Should Be $true
    }

    It "uses the CLI matrix commands instead of writing a summary-only matrix" {
        $source = Get-Content -Raw $runner
        ($source -match 'Matrix = "source-diff-matrix"') | Should Be $true
        ($source -match 'MatrixMerge = "source-diff-matrix-merge"') | Should Be $true
        ($source -match "Invoke-ParityStep -Name 'parity-matrix'") | Should Be $true
        ($source -match "'--require-complete-root-metadata'") | Should Be $true
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

    It "supports integrated authentication without password environment references in executed arguments" {
        $source = Get-Content -Raw $runner
        ($source -match '\[switch\]\$IntegratedAuth') | Should Be $true
        ($source.Contains('auth_mode=$authMode')) | Should Be $true
        ($source.Contains("if (-not `$IntegratedAuth) { `$nativeArgs += @('--db-user', `$DbUser, '--db-pwd-env', 'IBCMD_DB_PSW') }")) | Should Be $true
        ($source.Contains("if (-not `$IntegratedAuth) { `$candidateArgs += @('--sql-user', `$DbUser, '--sql-pwd-env', 'IBCMD_DB_PSW') }")) | Should Be $true
        $matrixSource = Get-Content -Raw $matrix
        ($matrixSource.Contains('[switch]$IntegratedAuth')) | Should Be $true
        ($matrixSource.Contains('$params.IntegratedAuth = $true')) | Should Be $true
    }

    It "persists a failed native step and clears inherited SQL credentials" {
        $lab = Join-Path $TestDrive "runtime-failure"
        $capturePath = Join-Path $TestDrive "runtime-failure-auth.json"
        $savedUser = $env:IBCMD_DB_USR
        $savedPassword = $env:IBCMD_DB_PSW
        $savedCapture = $env:PARITY_FAKE_CAPTURE
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:IBCMD_DB_USR = "must-not-leak"
            $env:IBCMD_DB_PSW = "must-not-leak-secret"
            $env:PARITY_FAKE_CAPTURE = $capturePath
            $env:PARITY_FAKE_MODE = "exit"
            & powershell -NoProfile -ExecutionPolicy Bypass -File $runner `
                -DbName missing_runtime_probe -IntegratedAuth -LabRoot $lab -RunId probe `
                -ExePath $fakeCli -IbcmdPath $fakeCli *> $null
            $LASTEXITCODE | Should Not Be 0
        } finally {
            $env:IBCMD_DB_USR = $savedUser
            $env:IBCMD_DB_PSW = $savedPassword
            $env:PARITY_FAKE_CAPTURE = $savedCapture
            $env:PARITY_FAKE_MODE = $savedMode
        }

        $manifestPath = Join-Path $lab "missing_runtime_probe_probe\parity-manifest.json"
        (Test-Path -LiteralPath $manifestPath) | Should Be $true
        $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
        $manifest.status | Should Be "failed"
        @($manifest.steps).Count | Should Be 1
        $manifest.steps[0].name | Should Be "native-export"
        $manifest.steps[0].status | Should Be "failed"
        $manifest.steps[0].ended_utc | Should Not BeNullOrEmpty
        $manifest.steps[0].exit_code | Should Be 23
        $manifest.database.auth_mode | Should Be "integrated"
        $manifest.tools.sqlcmd.version | Should Not BeNullOrEmpty
        $manifest.tools.bcp.version | Should Not BeNullOrEmpty
        $manifest.tools.sqlcmd.sha256 | Should Match "^[0-9a-f]{64}$"
        $manifest.tools.bcp.sha256 | Should Match "^[0-9a-f]{64}$"
        ((Get-Content -Raw -LiteralPath $manifestPath) -match 'must-not-leak|IBCMD_DB_PSW') | Should Be $false

        $capture = Get-Content -Raw -LiteralPath $capturePath | ConvertFrom-Json
        $capture.db_user_present | Should Be $false
        $capture.db_password_present | Should Be $false
    }

    It "persists exception details for a terminating step failure" {
        $lab = Join-Path $TestDrive "runtime-exception"
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:PARITY_FAKE_MODE = "throw"
            & powershell -NoProfile -ExecutionPolicy Bypass -File $runner `
                -DbName missing_exception_probe -IntegratedAuth -LabRoot $lab -RunId probe `
                -ExePath $fakeCli -IbcmdPath $fakeCli *> $null
            $LASTEXITCODE | Should Not Be 0
        } finally {
            $env:PARITY_FAKE_MODE = $savedMode
        }

        $manifestPath = Join-Path $lab "missing_exception_probe_probe\parity-manifest.json"
        $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
        $manifest.status | Should Be "failed"
        $manifest.steps[0].status | Should Be "failed"
        $manifest.steps[0].ended_utc | Should Not BeNullOrEmpty
        $manifest.steps[0].exit_code | Should Be -1
        $manifest.steps[0].exception | Should Match "fake terminating export failure"
    }

    It "redacts SQL-auth secret names and values from manifests and logs" {
        $lab = Join-Path $TestDrive "runtime-sql-redaction"
        $savedPassword = $env:IBCMD_DB_PSW
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:IBCMD_DB_PSW = "manifest-redaction-probe-secret"
            $env:PARITY_FAKE_MODE = "leak-exit"
            & powershell -NoProfile -ExecutionPolicy Bypass -File $runner `
                -DbName missing_sql_redaction_probe -DbUser fake_user `
                -LabRoot $lab -RunId probe -ExePath $fakeCli -IbcmdPath $fakeCli *> $null
            $LASTEXITCODE | Should Not Be 0
        } finally {
            $env:IBCMD_DB_PSW = $savedPassword
            $env:PARITY_FAKE_MODE = $savedMode
        }

        $runRoot = Join-Path $lab "missing_sql_redaction_probe_probe"
        $manifestText = Get-Content -Raw -LiteralPath (Join-Path $runRoot "parity-manifest.json")
        $logText = Get-Content -Raw -LiteralPath (Join-Path $runRoot "logs\native-export.log")
        (($manifestText + $logText) -match 'manifest-redaction-probe-secret|IBCMD_DB_PSW') | Should Be $false
        ($logText -match '<redacted>') | Should Be $true
        $manifest = $manifestText | ConvertFrom-Json
        (@($manifest.database_fingerprint.before.arguments) -contains '-P') | Should Be $false
    }

    It "journals a failed child run in the top-level diagnostic manifest" {
        $lab = Join-Path $TestDrive "matrix-runtime-failure"
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:PARITY_FAKE_MODE = "exit"
            & powershell -NoProfile -ExecutionPolicy Bypass -File $matrix `
                -UtDbName missing_matrix_ut -BspDbName missing_matrix_bsp `
                -IntegratedAuth -LabRoot $lab -RunId orchestrator_probe `
                -ExePath $fakeCli -IbcmdPath $fakeCli `
                -Scope scoped -PathPrefix "__manifest_probe__" *> $null
            $LASTEXITCODE | Should Not Be 0
        } finally {
            $env:PARITY_FAKE_MODE = $savedMode
        }

        $manifestPath = Join-Path $lab "matrix_orchestrator_probe\parity-matrix-manifest.json"
        $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
        $manifest.status | Should Be "failed"
        $manifest.scope | Should Be "scoped"
        $manifest.result_class | Should Be "diagnostic"
        $manifest.release_eligible | Should Be $false
        @($manifest.steps).Count | Should Be 1
        $manifest.steps[0].name | Should Be "child-ut"
        $manifest.steps[0].status | Should Be "failed"
        $manifest.steps[0].ended_utc | Should Not BeNullOrEmpty
        $manifest.steps[0].log | Should Be "logs/child-ut.log"
        @($manifest.steps[0].artifacts).Count | Should Be 1
    }
}
