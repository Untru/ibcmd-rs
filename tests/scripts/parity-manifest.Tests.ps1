$ErrorActionPreference = "Stop"

Describe "Parity protocol scripts" {
    $repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $runner = Join-Path $repo "scripts\export-ibcmd-vs-ours.ps1"
    $matrix = Join-Path $repo "scripts\run-parity-matrix.ps1"
    $fakeCli = Join-Path $repo "tests\fixtures\parity\fake-cli.ps1"
    $fakeSqlcmd = Join-Path $repo "tests\fixtures\parity\sqlcmd.cmd"
    $fakeBcp = Join-Path $repo "tests\fixtures\parity\bcp.cmd"

    function Invoke-ExpectedFailureScript {
        param([string]$ScriptPath, [object[]]$Arguments)
        $previousPreference = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -File $ScriptPath @Arguments *> $null
            return [int]$LASTEXITCODE
        } finally {
            $ErrorActionPreference = $previousPreference
        }
    }

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
        ($source -match 'function Register-ManifestTool') | Should Be $true
        ($source -match "status = 'running'.*?version_arguments" -or $source -match "(?s)status = 'running'.*?version_arguments") | Should Be $true
        ($source -match '(?s)tool=\$Tool.*?executable=\$Executable.*?arguments=\(ConvertTo-SanitizedArguments \$Arguments\)') | Should Be $true
    }

    It "runs UT and BSP and labels scoped output as diagnostic" {
        $source = Get-Content -Raw $matrix
        ($source -match "id='ut'") | Should Be $true
        ($source -match "id='bsp'") | Should Be $true
        ($source.Contains("[string]`$BspDbName = 'bsp'")) | Should Be $true
        ($source -match 'source-diff-matrix-merge') | Should Be $true
        ($source -match 'Read-ValidChildManifest') | Should Be $true
        ($source -match "native_ibcmd_sha256") | Should Be $true
        ($source -match "candidate_sha256") | Should Be $true
        ($source -match 'release_eligible = \$false') | Should Be $true
        ($source -match '(?s)\$RequireCompleteRootMetadata.*\$matrixManifest.parity_zero') | Should Be $true
    }

    It "hashes every executable and keeps SQL passwords out of process arguments" {
        $source = Get-Content -Raw $runner
        foreach ($tool in @('git', 'candidate', 'native_ibcmd', 'sqlcmd', 'bcp', 'robocopy')) {
            ($source -match "Register-ManifestTool -Name '${tool}'") | Should Be $true
        }
        ($source -match '(?s)function Register-ManifestTool.*?\$record\.sha256 = Get-FileSha256') | Should Be $true
        ($source -match 'function Get-FileSha256') | Should Be $true
        ($source -match 'Get-FileHash') | Should Be $false
        ((Get-Content -Raw $matrix) -match 'Get-FileHash') | Should Be $false
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
        (Invoke-ExpectedFailureScript $runner @('-DbName','test','-LabRoot',$lab,'-RunId','valid_full','-Scope','full','-PathPrefix','Catalogs')) | Should Not Be 0
        (Invoke-ExpectedFailureScript $runner @('-DbName','test','-LabRoot',$lab,'-RunId','valid_scoped','-Scope','scoped')) | Should Not Be 0
        (Invoke-ExpectedFailureScript $matrix @('-LabRoot',$lab,'-RunId','strict_scoped','-Scope','scoped','-PathPrefix','Catalogs','-RequireCompleteRootMetadata')) | Should Not Be 0
        (Test-Path $lab) | Should Be $false
    }

    It "rejects unsafe RunId before creating a run directory" {
        $lab = Join-Path $TestDrive "runid"
        (Invoke-ExpectedFailureScript $runner @('-DbName','test','-LabRoot',$lab,'-RunId','../escape')) | Should Not Be 0
        (Invoke-ExpectedFailureScript $matrix @('-LabRoot',$lab,'-RunId','bad\\path')) | Should Not Be 0
        (Test-Path $lab) | Should Be $false
    }

    It "rejects the same normalized database for UT and BSP before creating runs" {
        $lab = Join-Path $TestDrive 'same-database'
        (Invoke-ExpectedFailureScript $matrix @(
            '-UtDbName','ReleaseDb','-BspDbName','  releasedb  ','-IntegratedAuth',
            '-LabRoot',$lab,'-RunId','same_db'
        )) | Should Not Be 0
        (Test-Path -LiteralPath $lab) | Should Be $false
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
        ($source.Contains("[ValidateRange(1, 86400)][int]`$NativeTimeoutSec = 900")) | Should Be $true
        ($source.Contains("'--timeout-sec', [string]`$NativeTimeoutSec")) | Should Be $true
        $matrixSource = Get-Content -Raw $matrix
        ($matrixSource.Contains('[switch]$IntegratedAuth')) | Should Be $true
        ($matrixSource.Contains('$params.IntegratedAuth = $true')) | Should Be $true
        ($matrixSource.Contains('NativeTimeoutSec=$NativeTimeoutSec')) | Should Be $true
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
            (Invoke-ExpectedFailureScript $runner @(
                '-DbName','missing_runtime_probe','-IntegratedAuth','-LabRoot',$lab,'-RunId','probe',
                '-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,'-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp
            )) | Should Not Be 0
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
        foreach ($tool in @('git', 'candidate', 'native_ibcmd', 'sqlcmd', 'bcp', 'robocopy')) {
            $manifest.tools.$tool.status | Should Be "passed"
            $manifest.tools.$tool.started_utc | Should Not BeNullOrEmpty
            $manifest.tools.$tool.ended_utc | Should Not BeNullOrEmpty
            $manifest.tools.$tool.version | Should Not BeNullOrEmpty
            @($manifest.tools.$tool.version_arguments).Count | Should BeGreaterThan 0
            $manifest.tools.$tool.sha256 | Should Match "^[0-9a-f]{64}$"
        }
        $expectedVersionArguments = @{
            git = @('--version')
            candidate = @('--version')
            native_ibcmd = @('--version')
            sqlcmd = @('-?')
            bcp = @('-v')
            robocopy = @('/?')
        }
        foreach ($tool in $expectedVersionArguments.Keys) {
            (@($manifest.tools.$tool.version_arguments) -join "`0") |
                Should Be (@($expectedVersionArguments[$tool]) -join "`0")
        }
        $manifest.repository_probe.status | Should Be "passed"
        @($manifest.repository_probe.commands).Count | Should Be 2
        $manifest.repository_probe.commands[0].executable | Should Be $manifest.tools.git.path
        (@($manifest.repository_probe.commands[0].arguments) -join "`0") |
            Should Be (@('-C', $repo, 'rev-parse', 'HEAD') -join "`0")
        $manifest.tools.candidate.capability_probe_status | Should Be "passed"
        @($manifest.tools.candidate.capability_probe_arguments).Count | Should Be 6
        @($manifest.tools.candidate.capability_probes).Count | Should Be 6
        foreach ($probe in @($manifest.tools.candidate.capability_probes)) {
            $probe.executable | Should Be $fakeCli
            @($probe.arguments).Count | Should Be 2
            $probe.arguments[1] | Should Be '--help'
            $probe.status | Should Be 'passed'
            $probe.exit_code | Should Be 0
            $probe.started_utc | Should Not BeNullOrEmpty
            $probe.ended_utc | Should Not BeNullOrEmpty
        }
        $manifest.steps[0].tool | Should Be "candidate"
        $manifest.steps[0].executable | Should Be $fakeCli
        $expectedNativeArguments = @(
            'dump-sources', '--dbms', 'MSSQLServer', '--db-server', 'localhost',
            '--db-name', 'missing_runtime_probe', '-o',
            (Join-Path $lab 'missing_runtime_probe_probe\native'),
            '--overwrite', '--ibcmd', $fakeCli, '--timeout-sec', '900',
            '--runtime-journal', (Join-Path $lab 'missing_runtime_probe_probe\logs\native-runtime.json')
        )
        (@($manifest.steps[0].arguments) -join "`0") | Should Be ($expectedNativeArguments -join "`0")
        $manifest.nested_runtime_calls.status | Should Be 'failed'
        $manifest.nested_runtime_calls.native_call.status | Should Be 'failed'
        $manifest.nested_runtime_calls.native_report_sha256 | Should Match '^[0-9a-f]{64}$'
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
            (Invoke-ExpectedFailureScript $runner @(
                '-DbName','missing_exception_probe','-IntegratedAuth','-LabRoot',$lab,'-RunId','probe',
                '-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,'-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp
            )) | Should Not Be 0
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

    It "retains a passed nested ibcmd call when the outer native export step fails later" {
        $lab = Join-Path $TestDrive "native-outer-failure"
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:PARITY_FAKE_MODE = "native-outer-exit"
            (Invoke-ExpectedFailureScript $runner @(
                '-DbName','outer_failure_probe','-IntegratedAuth','-LabRoot',$lab,'-RunId','probe',
                '-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,'-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp
            )) | Should Not Be 0
        } finally {
            $env:PARITY_FAKE_MODE = $savedMode
        }

        $runRoot = Join-Path $lab "outer_failure_probe_probe"
        $manifest = Get-Content -Raw -LiteralPath (Join-Path $runRoot "parity-manifest.json") | ConvertFrom-Json
        $manifest.status | Should Be "failed"
        $manifest.steps[0].status | Should Be "failed"
        $manifest.steps[0].exit_code | Should Be 31
        $manifest.nested_runtime_calls.status | Should Be "failed"
        $manifest.nested_runtime_calls.native_call_status | Should Be "passed"
        $manifest.nested_runtime_calls.native_call.status | Should Be "passed"
        $manifest.nested_runtime_calls.native_report_sha256 | Should Match "^[0-9a-f]{64}$"
    }

    It "conservatively finalizes and ingests a stale running native journal after producer exit" {
        $lab = Join-Path $TestDrive "native-stale-recovery"
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:PARITY_FAKE_MODE = "native-stale-exit"
            (Invoke-ExpectedFailureScript $runner @(
                '-DbName','stale_runtime_probe','-IntegratedAuth','-LabRoot',$lab,'-RunId','probe',
                '-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,'-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp
            )) | Should Not Be 0
        } finally {
            $env:PARITY_FAKE_MODE = $savedMode
        }

        $runRoot = Join-Path $lab "stale_runtime_probe_probe"
        $manifest = Get-Content -Raw -LiteralPath (Join-Path $runRoot "parity-manifest.json") | ConvertFrom-Json
        $journal = Get-Content -Raw -LiteralPath (Join-Path $runRoot "logs\native-runtime.json") | ConvertFrom-Json
        $manifest.status | Should Be "failed"
        $manifest.status | Should Not Be "passed"
        $manifest.steps[0].status | Should Be "failed"
        $manifest.nested_runtime_calls.native_call.status | Should Be "failed"
        $manifest.nested_runtime_calls.native_call.exception | Should Match "supervisor observed producer exit"
        $manifest.nested_runtime_calls.native_report_sha256 | Should Match "^[0-9a-f]{64}$"
        $journal.runtime_call.status | Should Be "failed"
        $journal.supervisor_recovery.kind | Should Be "stale-running"
        $journal.supervisor_recovery.original_sha256 | Should Match "^[0-9a-f]{64}$"
    }

    It "conservatively finalizes and ingests stale running candidate calls after producer exit" {
        $lab = Join-Path $TestDrive "candidate-stale-recovery"
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:PARITY_FAKE_MODE = "candidate-stale-exit"
            (Invoke-ExpectedFailureScript $runner @(
                '-DbName','candidate_stale_probe','-IntegratedAuth','-LabRoot',$lab,'-RunId','probe',
                '-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,'-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp
            )) | Should Not Be 0
        } finally {
            $env:PARITY_FAKE_MODE = $savedMode
        }

        $runRoot = Join-Path $lab "candidate_stale_probe_probe"
        $manifest = Get-Content -Raw -LiteralPath (Join-Path $runRoot "parity-manifest.json") | ConvertFrom-Json
        $journal = Get-Content -Raw -LiteralPath (Join-Path $runRoot "logs\candidate-runtime.json") | ConvertFrom-Json
        $manifest.status | Should Be "failed"
        $manifest.steps[1].status | Should Be "failed"
        $manifest.steps[1].exit_code | Should Be 33
        $manifest.nested_runtime_calls.candidate_subprocess_journal_status | Should Be "failed"
        @($manifest.nested_runtime_calls.candidate_calls).Count | Should Be 1
        $manifest.nested_runtime_calls.candidate_calls[0].status | Should Be "failed"
        $manifest.nested_runtime_calls.candidate_manifest_sha256 | Should Match "^[0-9a-f]{64}$"
        $journal.status | Should Be "failed"
        $journal.calls[0].status | Should Be "failed"
        $journal.calls[0].exception | Should Match "supervisor observed producer exit"
        $journal.supervisor_recovery.kind | Should Be "stale-running"
    }

    It "retains a passed candidate journal when the candidate wrapper fails later" {
        $lab = Join-Path $TestDrive "candidate-outer-failure"
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:PARITY_FAKE_MODE = "candidate-outer-exit"
            (Invoke-ExpectedFailureScript $runner @(
                '-DbName','candidate_outer_probe','-IntegratedAuth','-LabRoot',$lab,'-RunId','probe',
                '-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,'-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp
            )) | Should Not Be 0
        } finally {
            $env:PARITY_FAKE_MODE = $savedMode
        }

        $runRoot = Join-Path $lab "candidate_outer_probe_probe"
        $manifest = Get-Content -Raw -LiteralPath (Join-Path $runRoot "parity-manifest.json") | ConvertFrom-Json
        $manifest.status | Should Be "failed"
        $manifest.steps[1].status | Should Be "failed"
        $manifest.steps[1].exit_code | Should Be 34
        $manifest.nested_runtime_calls.status | Should Be "failed"
        $manifest.nested_runtime_calls.candidate_subprocess_journal_status | Should Be "passed"
        @($manifest.nested_runtime_calls.candidate_calls).Count | Should Be 2
        @($manifest.nested_runtime_calls.candidate_calls | Where-Object { $_.status -ne 'passed' }).Count | Should Be 0
        $manifest.nested_runtime_calls.candidate_manifest_sha256 | Should Match "^[0-9a-f]{64}$"
    }

    It "rejects wrong runtime database identity and incoherent failed calls" {
        $tokens = $null; $errors = $null
        $ast = [Management.Automation.Language.Parser]::ParseFile($runner, [ref]$tokens, [ref]$errors)
        foreach ($name in @('Assert-ManifestSafe', 'Get-FileSha256', 'ConvertFrom-WindowsExtendedLengthPath', 'Get-NormalizedExecutablePath', 'Assert-CompleteRuntimeCall', 'Import-VerifiedRuntimeEvidence')) {
            $functionAst = $ast.Find({
                param($node)
                $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name
            }.GetNewClosure(), $true)
            Invoke-Expression $functionAst.Extent.Text
        }

        $server = 'runtime-db-host'
        $database = 'runtime_db'
        $nativePath = Join-Path $TestDrive 'identity-native.json'
        $candidatePath = Join-Path $TestDrive 'identity-candidate.json'
        $native = [ordered]@{
            protocol_version=1
            runtime_call=[ordered]@{
                executable=$fakeCli
                arguments=@('infobase','config','export',("--db-server=$server"),("--db-name=$database"),'--force','C:\out')
                started_unix_ms=1; ended_unix_ms=2; status='passed'; exit_code=0
                timed_out=$false; exception=$null
            }
        }
        $calls = @(
            [ordered]@{
                executable=$fakeSqlcmd
                arguments=@('-S',$server,'-Q',('<query-sha256:' + ('a' * 64) + '>'))
                started_unix_ms=3; ended_unix_ms=4; status='passed'; exit_code=0
                timed_out=$false; exception=$null
            },
            [ordered]@{
                executable=$fakeBcp
                arguments=@(('<query-sha256:' + ('b' * 64) + '>'),'queryout','C:\rows.bcp','-S',$server,'-T')
                started_unix_ms=5; ended_unix_ms=6; status='passed'; exit_code=0
                timed_out=$false; exception=$null
            }
        )
        $candidate = [ordered]@{
            protocol_version=1; status='passed'; server=$server; database=$database
            started_unix_ms=3; ended_unix_ms=6; exception=$null; calls=$calls
        }
        $native | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $nativePath -Encoding UTF8
        $candidate | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $candidatePath -Encoding UTF8
        $valid = Import-VerifiedRuntimeEvidence -NativeJournalPath $nativePath -CandidateJournalPath $candidatePath `
            -NativeIbcmdPath $fakeCli -SqlcmdPath $fakeSqlcmd -BcpPath $fakeBcp `
            -ExpectedServer $server -ExpectedDatabase $database
        $valid.status | Should Be 'passed'

        $wrongCandidate = Get-Content -Raw -LiteralPath $candidatePath | ConvertFrom-Json
        $wrongCandidate.database = 'wrong_db'
        [IO.File]::WriteAllText($candidatePath, ($wrongCandidate | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
        (Get-Content -Raw -LiteralPath $candidatePath | ConvertFrom-Json).database | Should Be 'wrong_db'
        $wrongDatabaseBlocked = $false
        try {
            $null = Import-VerifiedRuntimeEvidence -NativeJournalPath $nativePath -CandidateJournalPath $candidatePath `
                -NativeIbcmdPath $fakeCli -SqlcmdPath $fakeSqlcmd -BcpPath $fakeBcp `
                -ExpectedServer $server -ExpectedDatabase $database
        } catch { $wrongDatabaseBlocked = $true }
        $wrongDatabaseBlocked | Should Be $true

        $wrongCandidate.database = $database
        $wrongCandidate.calls[0].arguments[1] = 'wrong-server'
        [IO.File]::WriteAllText($candidatePath, ($wrongCandidate | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
        (Get-Content -Raw -LiteralPath $candidatePath | ConvertFrom-Json).calls[0].arguments[1] | Should Be 'wrong-server'
        $wrongServerBlocked = $false
        try {
            $null = Import-VerifiedRuntimeEvidence -NativeJournalPath $nativePath -CandidateJournalPath $candidatePath `
                -NativeIbcmdPath $fakeCli -SqlcmdPath $fakeSqlcmd -BcpPath $fakeBcp `
                -ExpectedServer $server -ExpectedDatabase $database
        } catch { $wrongServerBlocked = $true }
        $wrongServerBlocked | Should Be $true

        $incoherent = [pscustomobject]@{
            executable=$fakeCli
            arguments=@('infobase','config','export',("--db-server=$server"),("--db-name=$database"),'--force','C:\out')
            started_unix_ms=1; ended_unix_ms=2; status='failed'; exit_code=0
            timed_out=$false; exception=$null
        }
        $incoherentBlocked = $false
        try {
            Assert-CompleteRuntimeCall -Call $incoherent -ExpectedExecutable $fakeCli `
                -ExpectedServer $server -ExpectedDatabase $database -Kind native_ibcmd -ExpectedStatus failed
        } catch { $incoherentBlocked = $true }
        $incoherentBlocked | Should Be $true
    }

    It "normalizes only equivalent Windows extended executable paths" {
        $tokens = $null; $errors = $null
        $ast = [Management.Automation.Language.Parser]::ParseFile($runner, [ref]$tokens, [ref]$errors)
        foreach ($name in @('ConvertFrom-WindowsExtendedLengthPath', 'Get-NormalizedExecutablePath', 'Assert-CompleteRuntimeCall')) {
            $functionAst = $ast.Find({
                param($node)
                $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name
            }.GetNewClosure(), $true)
            Invoke-Expression $functionAst.Extent.Text
        }

        $normalPath = [IO.Path]::GetFullPath($fakeCli)
        $extendedPath = '\\?\' + $normalPath
        (ConvertFrom-WindowsExtendedLengthPath $extendedPath) | Should Be $normalPath
        (ConvertFrom-WindowsExtendedLengthPath '\\?\UNC\server\share\tool.exe') |
            Should Be '\\server\share\tool.exe'
        (Get-NormalizedExecutablePath $extendedPath) | Should Be (Get-NormalizedExecutablePath $normalPath)

        foreach ($invalidPath in @(
            '\\?\GLOBALROOT\Device\HarddiskVolume1\tool.exe',
            '\\?\Volume{11111111-1111-4111-8111-111111111111}\tool.exe',
            '\\.\C:\tool.exe',
            '\??\C:\tool.exe',
            '\\??\C:\tool.exe',
            '\\?\C:/tool.exe',
            '\\?\C:\dir\.\tool.exe',
            '\\?\C:\dir\..\tool.exe',
            '\\?\C:\dir.\tool.exe',
            '\\?\C:\dir \tool.exe',
            '\\?\C:\tool.exe:stream',
            '\\?\C:\dir\\tool.exe',
            '\\?\C:\dir\*.exe',
            '\\?\UNC\server\share',
            '\\?\UNC\\share\tool.exe',
            '\\?\UNC\server\\tool.exe',
            '\\?\UNC\server\share\..\tool.exe',
            '\\?\UNC\server\share\tool.exe:stream'
        )) {
            $invalidBlocked = $false
            try { $null = ConvertFrom-WindowsExtendedLengthPath $invalidPath }
            catch { $invalidBlocked = $true }
            if (-not $invalidBlocked) { throw "Accepted invalid Windows executable path: $invalidPath" }
        }
        foreach ($journalControlledPath in @('ibcmd.exe', (Join-Path $TestDrive '*.ps1'))) {
            $unqualifiedBlocked = $false
            try { $null = Get-NormalizedExecutablePath $journalControlledPath }
            catch { $unqualifiedBlocked = $true }
            if (-not $unqualifiedBlocked) { throw "Accepted unqualified/wildcard journal path: $journalControlledPath" }
        }
        $normalParent = Split-Path -Parent $normalPath
        $normalLeaf = Split-Path -Leaf $normalPath
        foreach ($ordinaryInvalidPath in @(
            ($normalPath + ':stream'),
            ($normalParent + '\.\' + $normalLeaf),
            ($normalParent + '\child\..\' + $normalLeaf),
            ($normalParent + '\\' + $normalLeaf),
            ($normalParent + '\trailing.\' + $normalLeaf),
            ($normalParent + '\trailing \' + $normalLeaf)
        )) {
            $ordinaryBlocked = $false
            try { $null = Get-NormalizedExecutablePath $ordinaryInvalidPath }
            catch { $ordinaryBlocked = $true }
            if (-not $ordinaryBlocked) { throw "Accepted ambiguous ordinary executable path: $ordinaryInvalidPath" }
        }

        function New-ValidNativeRuntimeCall([string]$Executable) {
            return [pscustomobject]@{
                executable=$Executable
                arguments=@('infobase','config','export','--db-server=runtime-host','--db-name=runtime-db','--force','C:\out')
                started_unix_ms=1; ended_unix_ms=2; status='passed'; exit_code=0
                timed_out=$false; exception=$null
            }
        }

        Assert-CompleteRuntimeCall -Call (New-ValidNativeRuntimeCall $extendedPath) `
            -ExpectedExecutable $normalPath -ExpectedServer runtime-host -ExpectedDatabase runtime-db -Kind native_ibcmd

        $differentPath = Join-Path $TestDrive 'different-runtime.ps1'
        Copy-Item -LiteralPath $fakeCli -Destination $differentPath
        $differentBlocked = $false
        try {
            Assert-CompleteRuntimeCall -Call (New-ValidNativeRuntimeCall $differentPath) `
                -ExpectedExecutable $normalPath -ExpectedServer runtime-host -ExpectedDatabase runtime-db -Kind native_ibcmd
        } catch { $differentBlocked = $true }
        $differentBlocked | Should Be $true

        $reparseRoot = Join-Path $TestDrive 'runtime-reparse'
        $targetRoot = Split-Path -Parent $fakeCli
        $linkType = if ($env:OS -eq 'Windows_NT') { 'Junction' } else { 'SymbolicLink' }
        $null = New-Item -ItemType $linkType -Path $reparseRoot -Target $targetRoot -ErrorAction Stop
        $reparsePath = Join-Path $reparseRoot (Split-Path -Leaf $fakeCli)
        $reparseBlocked = $false
        try {
            Assert-CompleteRuntimeCall -Call (New-ValidNativeRuntimeCall $reparsePath) `
                -ExpectedExecutable $normalPath -ExpectedServer runtime-host -ExpectedDatabase runtime-db -Kind native_ibcmd
        } catch { $reparseBlocked = $true }
        $reparseBlocked | Should Be $true
        $aliasIdentityBlocked = $false
        try {
            Assert-CompleteRuntimeCall -Call (New-ValidNativeRuntimeCall $reparsePath) `
                -ExpectedExecutable $reparsePath -ExpectedServer runtime-host -ExpectedDatabase runtime-db -Kind native_ibcmd
        } catch { $aliasIdentityBlocked = $true }
        $aliasIdentityBlocked | Should Be $true
    }

    It "records an after fingerprint when candidate export fails" {
        $lab = Join-Path $TestDrive "candidate-fingerprint"
        $fixtureDir = Split-Path -Parent $fakeCli
        $savedPath = $env:PATH
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:PATH = "$fixtureDir;$savedPath"
            $env:PARITY_FAKE_MODE = "candidate-exit"
            (Invoke-ExpectedFailureScript $runner @(
                '-DbName','fingerprint_stub','-IntegratedAuth','-LabRoot',$lab,'-RunId','probe',
                '-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,'-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp
            )) | Should Not Be 0
        } finally {
            $env:PATH = $savedPath
            $env:PARITY_FAKE_MODE = $savedMode
        }

        $manifestPath = Join-Path $lab "fingerprint_stub_probe\parity-manifest.json"
        $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
        $manifest.status | Should Be "failed"
        @($manifest.steps).Count | Should Be 2
        $manifest.steps[1].name | Should Be "candidate-export"
        $manifest.steps[1].status | Should Be "failed"
        $manifest.steps[1].exit_code | Should Be 29
        $manifest.nested_runtime_calls.status | Should Be 'failed'
        $manifest.nested_runtime_calls.native_call.status | Should Be 'passed'
        $manifest.nested_runtime_calls.candidate_subprocess_journal_status | Should Be 'failed'
        $manifest.nested_runtime_calls.candidate_manifest_sha256 | Should Match '^[0-9a-f]{64}$'
        $manifest.database_fingerprint.after.status | Should Be "passed"
        $manifest.database_fingerprint.after.sha256 | Should Match "^[0-9a-f]{64}$"
        $manifest.database_fingerprint.after.ended_utc | Should Not BeNullOrEmpty
        $manifest.database_fingerprint.unchanged | Should Be $true
    }

    It "keeps a self-contained valid manifest when the process is interrupted mid-step" {
        $lab = Join-Path $TestDrive "runtime-interruption"
        $slowCli = Join-Path $TestDrive "slow-cli.ps1"
        @'
$command = if ($args.Count -gt 0) { [string]$args[0] } else { '' }
if ($command -eq '--version') { Write-Output 'slow fake 1.0.0'; $global:LASTEXITCODE = 0; return }
if ($args.Count -gt 1 -and $args[1] -eq '--help') { Write-Output "$command fake help"; $global:LASTEXITCODE = 0; return }
if ($command -eq 'dump-sources') { Start-Sleep -Seconds 30; $global:LASTEXITCODE = 0; return }
$global:LASTEXITCODE = 0
'@ | Set-Content -LiteralPath $slowCli -Encoding UTF8

        $fixtureDir = Split-Path -Parent $fakeCli
        $savedPath = $env:PATH
        $process = $null
        try {
            $env:PATH = "$fixtureDir;$savedPath"
            $arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$runner`" -DbName interrupt_probe -IntegratedAuth -LabRoot `"$lab`" -RunId probe -ExePath `"$slowCli`" -IbcmdPath `"$slowCli`" -SqlcmdExecutable `"$fakeSqlcmd`" -BcpExecutable `"$fakeBcp`""
            $process = Start-Process -FilePath "powershell" -ArgumentList $arguments -PassThru -WindowStyle Hidden
            $manifestPath = Join-Path $lab "interrupt_probe_probe\parity-manifest.json"
            $runningManifest = $null
            for ($attempt = 0; $attempt -lt 120; $attempt++) {
                if (Test-Path -LiteralPath $manifestPath) {
                    try {
                        $candidateManifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
                        if (@($candidateManifest.steps).Count -eq 1 -and $candidateManifest.steps[0].status -eq 'running') {
                            $runningManifest = $candidateManifest
                            break
                        }
                    } catch {
                        # Atomic replacement should make this transient at most.
                    }
                }
                if ($process.HasExited) { break }
                Start-Sleep -Milliseconds 100
            }
            if ($null -eq $runningManifest) {
                $early = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
                throw "Process exited before running native step: status=$($early.status); failure=$($early.failure)"
            }
            Stop-Process -Id $process.Id -Force
            $process.WaitForExit()

            $manifestText = Get-Content -Raw -LiteralPath $manifestPath
            $manifest = $manifestText | ConvertFrom-Json
            $manifest.status | Should Be "running"
            @($manifest.steps).Count | Should Be 1
            $manifest.steps[0].status | Should Be "running"
            $manifest.steps[0].tool | Should Be "candidate"
            $manifest.steps[0].executable | Should Be $slowCli
            $manifest.steps[0].ended_utc | Should BeNullOrEmpty
            $manifest.steps[0].exit_code | Should BeNullOrEmpty
            @($manifest.steps[0].arguments).Count | Should BeGreaterThan 0
            foreach ($tool in @('git', 'candidate', 'native_ibcmd', 'sqlcmd', 'bcp', 'robocopy')) {
                $manifest.tools.$tool.status | Should Be "passed"
                $manifest.tools.$tool.version | Should Not BeNullOrEmpty
                $manifest.tools.$tool.sha256 | Should Match "^[0-9a-f]{64}$"
            }
            ($manifestText -match 'IBCMD_DB_PSW|SQLCMDPASSWORD') | Should Be $false
        } finally {
            if ($null -ne $process -and -not $process.HasExited) {
                Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            }
            $env:PATH = $savedPath
        }
    }

    It "cannot persist passed when interrupted during the final database fingerprint" {
        $lab = Join-Path $TestDrive "final-fingerprint-interruption"
        $toolDir = Join-Path $TestDrive "final-fingerprint-tools"
        New-Item -ItemType Directory -Path $toolDir -Force | Out-Null
        $slowSqlcmd = Join-Path $toolDir 'sqlcmd.cmd'
        $counter = Join-Path $TestDrive 'fingerprint-counter.txt'
        @'
@echo off
if "%~1"=="-?" (
  echo slow sqlcmd 1.0
  exit /b 0
)
if exist "%PARITY_FP_COUNTER%" (
  powershell -NoProfile -Command "Start-Sleep -Seconds 30"
  exit /b 0
)
echo 1>"%PARITY_FP_COUNTER%"
echo Config file 0 1 deadbeef
echo ConfigSave file 0 1 deadbeef
exit /b 0
'@ | Set-Content -LiteralPath $slowSqlcmd -Encoding ASCII

        $successfulCli = Join-Path $TestDrive 'full-fake-cli.ps1'
        @'
$script:cliArgs = [object[]]$args
function Get-Option([string]$Name) {
    $index = [Array]::IndexOf($script:cliArgs, $Name)
    if ($index -ge 0 -and $index + 1 -lt $script:cliArgs.Count) { return [string]$script:cliArgs[$index + 1] }
    return $null
}
$command = if ($args.Count) { [string]$args[0] } else { '' }
if ($command -eq '--version') { Write-Output 'full fake 1.0'; $global:LASTEXITCODE=0; return }
if ($args.Count -gt 1 -and $args[1] -eq '--help') { Write-Output "$command fake help"; $global:LASTEXITCODE=0; return }
if ($command -eq 'dump-sources') {
    $output = Get-Option '-o'
    $ibcmd = Get-Option '--ibcmd'
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    'native' | Set-Content -LiteralPath (Join-Path $output 'source.txt')
    $report = [ordered]@{ runtime_call=[ordered]@{
        executable=$ibcmd
        arguments=@('infobase','config','export','--dbms=MSSQLServer',('--db-server=' + (Get-Option '--db-server')),('--db-name=' + (Get-Option '--db-name')),'--data=C:\temp\native-data','--force','C:\temp\native-export')
        started_unix_ms=1; ended_unix_ms=2; status='passed'; exit_code=0; timed_out=$false; exception=$null
    }}
    [IO.File]::WriteAllText((Get-Option '--runtime-journal'), ([ordered]@{protocol_version=1;runtime_call=$report.runtime_call} | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    Write-Output ($report | ConvertTo-Json -Depth 10 -Compress)
    $global:LASTEXITCODE=0
    return
}
if ($command -eq 'mssql-dump-config') {
    $output = Get-Option '-o'
    $sqlcmd = Get-Option '--sqlcmd'
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    $journal = @(
        [ordered]@{
            executable=$sqlcmd
            arguments=@('-C','-S','localhost','-s',"`t",'-w','65535','-y','0','-Y','0','-Q',('<query-sha256:' + ('d' * 64) + '>'))
            started_unix_ms=3; ended_unix_ms=4; status='passed'; exit_code=0; timed_out=$false; exception=$null
        },
        [ordered]@{
            executable=(Get-Option '--bcp-executable')
            arguments=@(('<query-sha256:' + ('e' * 64) + '>'),'queryout','C:\temp\rows.bcp','-S','localhost','-n','-u','-a','65535','-T')
            started_unix_ms=5; ended_unix_ms=6; status='passed'; exit_code=0; timed_out=$false; exception=$null
        }
    )
    [IO.File]::WriteAllText((Join-Path $output 'manifest.json'), ([ordered]@{database='test';tables=@();subprocess_journal=$journal} | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Get-Option '--runtime-journal'), ([ordered]@{protocol_version=1;status='passed';server=(Get-Option '--server');database=(Get-Option '--database');started_unix_ms=3;ended_unix_ms=6;exception=$null;calls=$journal} | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    Write-Output '{"candidate":"passed"}'
    $global:LASTEXITCODE=0
    return
}
if ($command -eq 'source-diff') {
    [IO.File]::WriteAllText((Get-Option '-o'), '{"differences":[]}', [Text.UTF8Encoding]::new($false))
} elseif ($command -eq 'source-diff-signatures') {
    [IO.File]::WriteAllText((Get-Option '-o'), '{}', [Text.UTF8Encoding]::new($false))
} elseif ($command -eq 'source-diff-matrix') {
    [IO.File]::WriteAllText((Get-Option '--output'), '{}', [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Get-Option '--markdown'), '# matrix', [Text.UTF8Encoding]::new($false))
}
Write-Output "$command fake success"
$global:LASTEXITCODE=0
'@ | Set-Content -LiteralPath $successfulCli -Encoding UTF8

        $fixtureDir = Split-Path -Parent $fakeCli
        $bcpPath = Join-Path $fixtureDir 'bcp.cmd'
        $savedPath = $env:PATH
        $savedCounter = $env:PARITY_FP_COUNTER
        $savedBcp = $env:PARITY_EXPECTED_BCP_PATH
        $process = $null
        try {
            $env:PATH = "$toolDir;$fixtureDir;$savedPath"
            $env:PARITY_FP_COUNTER = $counter
            $env:PARITY_EXPECTED_BCP_PATH = $bcpPath
            $arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$runner`" -DbName finalizing_probe -IntegratedAuth -LabRoot `"$lab`" -RunId probe -ExePath `"$successfulCli`" -IbcmdPath `"$successfulCli`" -SqlcmdExecutable `"$slowSqlcmd`" -BcpExecutable `"$fakeBcp`" -Scope scoped -PathPrefix __finalizing__"
            $process = Start-Process -FilePath powershell -ArgumentList $arguments -PassThru -WindowStyle Hidden
            $manifestPath = Join-Path $lab 'finalizing_probe_probe\parity-manifest.json'
            $finalizingManifest = $null
            for ($attempt = 0; $attempt -lt 300; $attempt++) {
                if (Test-Path -LiteralPath $manifestPath) {
                    try {
                        $candidateManifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
                        if ($candidateManifest.database_fingerprint.after.status -eq 'running') {
                            $finalizingManifest = $candidateManifest
                            break
                        }
                    } catch {}
                }
                if ($process.HasExited) { break }
                Start-Sleep -Milliseconds 100
            }
            if ($null -eq $finalizingManifest) {
                $early = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
                throw "Process exited before final fingerprint: status=$($early.status); failure=$($early.failure); nested=$($early.nested_runtime_calls.exception)"
            }
            if ($finalizingManifest.status -eq 'failed') {
                throw "Process failed before final fingerprint: failure=$($finalizingManifest.failure); nested=$($finalizingManifest.nested_runtime_calls.exception)"
            }
            $finalizingManifest.status | Should Be 'finalizing'
            Stop-Process -Id $process.Id -Force
            $process.WaitForExit()
            $persisted = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
            $persisted.status | Should Be 'finalizing'
            $persisted.status | Should Not Be 'passed'
            $persisted.database_fingerprint.before.status | Should Be 'passed'
            $persisted.database_fingerprint.after.status | Should Be 'running'
            $persisted.database_fingerprint.unchanged | Should BeNullOrEmpty
            $persisted.finished_utc | Should BeNullOrEmpty
            $persisted.nested_runtime_calls.status | Should Be 'passed'
        } finally {
            if ($null -ne $process -and -not $process.HasExited) {
                Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            }
            $env:PATH = $savedPath
            $env:PARITY_FP_COUNTER = $savedCounter
            $env:PARITY_EXPECTED_BCP_PATH = $savedBcp
        }
    }

    It "records an after fingerprint when tool discovery fails after sqlcmd" {
        $lab = Join-Path $TestDrive "early-tool-fingerprint"
        $fixtureDir = Split-Path -Parent $fakeCli
        $savedPath = $env:PATH
        $savedBcpMode = $env:PARITY_FAKE_BCP_MODE
        try {
            $env:PATH = "$fixtureDir;$savedPath"
            $env:PARITY_FAKE_BCP_MODE = "fail-version"
            (Invoke-ExpectedFailureScript $runner @(
                '-DbName','fingerprint_stub','-IntegratedAuth','-LabRoot',$lab,'-RunId','probe',
                '-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,'-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp
            )) | Should Not Be 0
        } finally {
            $env:PATH = $savedPath
            $env:PARITY_FAKE_BCP_MODE = $savedBcpMode
        }

        $manifestPath = Join-Path $lab "fingerprint_stub_probe\parity-manifest.json"
        $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
        $manifest.status | Should Be "failed"
        $manifest.failure | Should Match "Cannot read version"
        @($manifest.steps).Count | Should Be 0
        $manifest.tools.sqlcmd.status | Should Be "passed"
        $manifest.tools.sqlcmd.version | Should Not BeNullOrEmpty
        $manifest.tools.bcp.status | Should Be "failed"
        @($manifest.tools.bcp.version_arguments) | Should Be @('-v')
        $manifest.tools.bcp.ended_utc | Should Not BeNullOrEmpty
        $manifest.tools.bcp.exception | Should Match "Cannot read version"
        $manifest.database_fingerprint.before.status | Should Be "passed"
        $manifest.database_fingerprint.after.status | Should Be "passed"
        $manifest.database_fingerprint.after.sha256 | Should Match "^[0-9a-f]{64}$"
        $manifest.database_fingerprint.after.ended_utc | Should Not BeNullOrEmpty
        $manifest.database_fingerprint.unchanged | Should Be $true
    }

    It "redacts SQL-auth secret names and values from manifests and logs" {
        $lab = Join-Path $TestDrive "runtime-sql-redaction"
        $savedPassword = $env:IBCMD_DB_PSW
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:IBCMD_DB_PSW = "manifest-redaction-probe-secret"
            $env:PARITY_FAKE_MODE = "leak-exit"
            (Invoke-ExpectedFailureScript $runner @(
                '-DbName','missing_sql_redaction_probe','-DbUser','fake_user','-LabRoot',$lab,'-RunId','probe',
                '-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,'-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp
            )) | Should Not Be 0
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

    It "redacts overlapping values longest-first across every protected environment variable" {
        $saved = @{}
        foreach ($name in @('IBCMD_DB_PSW', 'IBCMD_USER_PSW', 'SQLCMDPASSWORD')) {
            $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
        }
        try {
            [Environment]::SetEnvironmentVariable('IBCMD_DB_PSW', 'overlap', 'Process')
            [Environment]::SetEnvironmentVariable('IBCMD_USER_PSW', 'overlap-long', 'Process')
            [Environment]::SetEnvironmentVariable('SQLCMDPASSWORD', 'overlap-long-longest', 'Process')
            foreach ($path in @($runner, $matrix)) {
                $tokens = $null; $errors = $null
                $ast = [Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors)
                $functionAst = $ast.Find({
                    param($node)
                    $node -is [Management.Automation.Language.FunctionDefinitionAst] -and
                    $node.Name -eq 'Protect-SensitiveText'
                }, $true)
                Invoke-Expression $functionAst.Extent.Text
                $protected = Protect-SensitiveText 'overlap-long-longest overlap-long overlap IBCMD_DB_PSW IBCMD_USER_PSW SQLCMDPASSWORD'
                $protected | Should Not Match 'overlap'
                $protected | Should Not Match 'IBCMD_DB_PSW|IBCMD_USER_PSW|SQLCMDPASSWORD'
                $protected | Should Match '<redacted>'
            }
        } finally {
            foreach ($name in $saved.Keys) {
                [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process')
            }
        }
    }

    It "redacts password options without corrupting JSON-looking log text" {
        foreach ($path in @($runner, $matrix)) {
            $tokens = $null; $errors = $null
            $ast = [Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors)
            $functionAst = $ast.Find({
                param($node)
                $node -is [Management.Automation.Language.FunctionDefinitionAst] -and
                $node.Name -eq 'Protect-SensitiveText'
            }, $true)
            Invoke-Expression $functionAst.Extent.Text

            $protectedJson = Protect-SensitiveText '{"x":"--db-pwd=token-aware-secret"}'
            $protectedJson | Should Not Match 'token-aware-secret'
            $parsed = $protectedJson | ConvertFrom-Json
            $parsed.x | Should Be '--db-pwd=<redacted>'

            $plain = Protect-SensitiveText 'failed --db-pwd="secret with spaces" retry'
            $plain | Should Not Match 'secret with spaces'
            $plain | Should Match 'failed --db-pwd=<redacted> retry'
        }
    }

    It "redacts exact JSON-escaped environment secret values" {
        $secretName = 'IBCMD_DB_PSW'
        $savedSecret = [Environment]::GetEnvironmentVariable($secretName, 'Process')
        $secret = 'alpha secret"omega'
        try {
            [Environment]::SetEnvironmentVariable($secretName, $secret, 'Process')
            foreach ($path in @($runner, $matrix)) {
                $tokens = $null; $errors = $null
                $ast = [Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors)
                $functionAst = $ast.Find({
                    param($node)
                    $node -is [Management.Automation.Language.FunctionDefinitionAst] -and
                    $node.Name -eq 'Protect-SensitiveText'
                }, $true)
                Invoke-Expression $functionAst.Extent.Text

                $serialized = ConvertTo-Json -InputObject ([ordered]@{ x = "--db-pwd=$secret" }) -Compress
                $protected = Protect-SensitiveText $serialized
                $protected | Should Not Match 'alpha|omega'
                $protected | Should Not Match ([regex]::Escape($secret))
                $parsed = $protected | ConvertFrom-Json
                $parsed.x | Should Be '--db-pwd=<redacted>'
            }
        } finally {
            [Environment]::SetEnvironmentVariable($secretName, $savedSecret, 'Process')
        }
    }

    It "validates complete child evidence and blocks every release identity mismatch before merge" {
        $tokens = $null; $errors = $null
        $ast = [Management.Automation.Language.Parser]::ParseFile($matrix, [ref]$tokens, [ref]$errors)
        foreach ($name in @('Get-FileSha256', 'ConvertFrom-WindowsExtendedLengthPath', 'Get-NormalizedExecutablePath', 'Assert-NoReparsePointComponent', 'Read-ValidChildManifest', 'Assert-CompatibleChildManifests')) {
            $functionAst = $ast.Find({
                param($node)
                $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name
            }.GetNewClosure(), $true)
            Invoke-Expression $functionAst.Extent.Text
        }

        function New-ChildEvidence {
            param([string]$Root, [string]$Scope, [string]$DatabaseName, [string]$Server = 'localhost')
            New-Item -ItemType Directory -Path (Join-Path $Root 'logs'), (Join-Path $Root 'candidate_dump') -Force | Out-Null
            $nativeReport = Join-Path $Root 'logs\native-runtime.json'
            $candidateManifest = Join-Path $Root 'logs\candidate-runtime.json'
            $matrixArtifact = Join-Path $Root 'matrix.json'
            '{"protocol_version":1,"runtime_call":{"status":"passed"}}' | Set-Content -LiteralPath $nativeReport -Encoding UTF8
            '{"protocol_version":1,"status":"passed","calls":[{"status":"passed"}]}' | Set-Content -LiteralPath $candidateManifest -Encoding UTF8
            '{"matrix":"valid"}' | Set-Content -LiteralPath $matrixArtifact -Encoding UTF8
            $manifest = [ordered]@{
                protocol_version=2; status='passed'; scope=$Scope; finished_utc='2026-07-25T00:00:00Z'
                git_sha=('a' * 40); xml_version='2.20'; source_version='2.20'
                database=[ordered]@{ name=$DatabaseName; server=$Server }
                repository=[ordered]@{ status=if ($Scope -eq 'full') { 'clean' } else { 'dirty' } }
                database_fingerprint=[ordered]@{
                    unchanged=$true
                    before=[ordered]@{ status='passed' }
                    after=[ordered]@{ status='passed' }
                }
                steps=@([ordered]@{ status='passed' })
                tools=[ordered]@{
                    candidate=[ordered]@{
                        status='passed'; path=$fakeCli; version='candidate 1.0'; sha256=('b' * 64)
                        capability_probe_status='passed'
                        capability_probes=@(1..6 | ForEach-Object {
                            [ordered]@{ status='passed'; exit_code=0; ended_utc='2026-07-25T00:00:00Z'; arguments=@("command-$_",'--help') }
                        })
                    }
                    native_ibcmd=[ordered]@{ status='passed'; path=$fakeCli; version='native 1.0'; sha256=('c' * 64) }
                    sqlcmd=[ordered]@{ status='passed'; path=$fakeCli; version='sqlcmd 1.0'; sha256=('d' * 64) }
                    bcp=[ordered]@{ status='passed'; path=$fakeCli; version='bcp 1.0'; sha256=('e' * 64) }
                }
                nested_runtime_calls=[ordered]@{
                    status='passed'; ended_utc='2026-07-25T00:00:00Z'
                    candidate_subprocess_journal_status='passed'; sqlcmd_calls=1; bcp_calls=1
                    native_report='logs/native-runtime.json'; native_report_sha256=(Get-FileSha256 $nativeReport)
                    candidate_manifest='logs/candidate-runtime.json'; candidate_manifest_sha256=(Get-FileSha256 $candidateManifest)
                }
                artifacts=[ordered]@{ matrix='matrix.json' }
                artifact_sha256=[ordered]@{ matrix=(Get-FileSha256 $matrixArtifact) }
            }
            $path = Join-Path $Root 'parity-manifest.json'
            $manifest | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $path -Encoding UTF8
            return $path
        }

        $fullPath = New-ChildEvidence -Root (Join-Path $TestDrive 'valid-full-child') -Scope full -DatabaseName ut
        $scopedPath = New-ChildEvidence -Root (Join-Path $TestDrive 'valid-scoped-child') -Scope scoped -DatabaseName bsp
        $full = Read-ValidChildManifest -Path $fullPath -ExpectedScope full -ExpectedCandidatePath $fakeCli -ExpectedSourceVersion '2.20' -ExpectedServer localhost -ExpectedDatabase ut
        $scoped = Read-ValidChildManifest -Path $scopedPath -ExpectedScope scoped -ExpectedCandidatePath $fakeCli -ExpectedSourceVersion '2.20' -ExpectedServer localhost -ExpectedDatabase bsp
        $full.status | Should Be 'passed'
        $scoped.status | Should Be 'passed'

        $base = [ordered]@{
            database='ut'; server=$full.database.server; git_sha=$full.git_sha; xml_version=$full.xml_version; source_version=$full.source_version
            candidate_path=$full.tools.candidate.path; candidate_version=$full.tools.candidate.version
            candidate_sha256=$full.tools.candidate.sha256; native_ibcmd_version=$full.tools.native_ibcmd.version
            native_ibcmd_path=$full.tools.native_ibcmd.path; native_ibcmd_sha256=$full.tools.native_ibcmd.sha256
            sqlcmd_path=$full.tools.sqlcmd.path; sqlcmd_version=$full.tools.sqlcmd.version; sqlcmd_sha256=$full.tools.sqlcmd.sha256
            bcp_path=$full.tools.bcp.path; bcp_version=$full.tools.bcp.version; bcp_sha256=$full.tools.bcp.sha256
        }
        $peer = [ordered]@{}
        foreach ($key in $base.Keys) { $peer[$key] = $base[$key] }
        $peer.database = 'bsp'
        $proof = Assert-CompatibleChildManifests -Children @($base, $peer) -ExpectedScope full
        $proof.release_proof | Should Be $true
        (Assert-CompatibleChildManifests -Children @($base, $peer) -ExpectedScope scoped).release_proof | Should Be $false
        $sameDatabasePeer = [ordered]@{}
        foreach ($key in $peer.Keys) { $sameDatabasePeer[$key] = $peer[$key] }
        $sameDatabasePeer.database = ' UT '
        $sameDatabaseBase = [ordered]@{}
        foreach ($key in $base.Keys) { $sameDatabaseBase[$key] = $base[$key] }
        $sameDatabaseBase.database = 'ut'
        $sameDbBlocked = $false
        try { $null = Assert-CompatibleChildManifests -Children @($sameDatabaseBase, $sameDatabasePeer) -ExpectedScope full }
        catch { $sameDbBlocked = $true }
        $sameDbBlocked | Should Be $true

        $differentServer = [ordered]@{}
        foreach ($key in $peer.Keys) { $differentServer[$key] = $peer[$key] }
        $differentServer.server = 'other-server'
        $serverCompatibilityBlocked = $false
        try { $null = Assert-CompatibleChildManifests -Children @($base, $differentServer) -ExpectedScope full }
        catch { $serverCompatibilityBlocked = $true }
        $serverCompatibilityBlocked | Should Be $true

        foreach ($field in @('git_sha', 'xml_version', 'source_version', 'candidate_version', 'candidate_sha256', 'native_ibcmd_version', 'native_ibcmd_sha256', 'candidate_path', 'native_ibcmd_path', 'sqlcmd_path', 'sqlcmd_version', 'sqlcmd_sha256', 'bcp_path', 'bcp_version', 'bcp_sha256')) {
            $mismatch = [ordered]@{}
            foreach ($key in $peer.Keys) { $mismatch[$key] = $peer[$key] }
            $mismatch[$field] = if ($field -match '_path$') { Join-Path $TestDrive 'different.exe' } else { 'different' }
            $blocked = $false
            try { $null = Assert-CompatibleChildManifests -Children @($base, $mismatch) -ExpectedScope full }
            catch { $blocked = $true }
            if (-not $blocked) { throw "Compatibility check accepted a $field mismatch." }
        }

        $invalid = Get-Content -Raw -LiteralPath $fullPath | ConvertFrom-Json
        $invalid.nested_runtime_calls.status = 'failed'
        $invalid | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $fullPath -Encoding UTF8
        $invalidBlocked = $false
        try { $null = Read-ValidChildManifest -Path $fullPath -ExpectedScope full -ExpectedCandidatePath $fakeCli -ExpectedSourceVersion '2.20' -ExpectedServer localhost -ExpectedDatabase ut }
        catch { $invalidBlocked = $true }
        $invalidBlocked | Should Be $true

        foreach ($wrongDatabase in @('UT', ' ut', 'ut ')) {
            $wrongPath = New-ChildEvidence -Root (Join-Path $TestDrive ("wrong-db-" + [guid]::NewGuid().ToString('N'))) -Scope full -DatabaseName $wrongDatabase
            $blocked = $false
            try { $null = Read-ValidChildManifest -Path $wrongPath -ExpectedScope full -ExpectedCandidatePath $fakeCli -ExpectedSourceVersion '2.20' -ExpectedServer localhost -ExpectedDatabase ut }
            catch { $blocked = $true }
            $blocked | Should Be $true
        }
        foreach ($wrongServer in @('', 'LOCALHOST', ' localhost', 'other-server')) {
            $wrongPath = New-ChildEvidence -Root (Join-Path $TestDrive ("wrong-server-" + [guid]::NewGuid().ToString('N'))) -Scope full -DatabaseName ut -Server $wrongServer
            $blocked = $false
            try { $null = Read-ValidChildManifest -Path $wrongPath -ExpectedScope full -ExpectedCandidatePath $fakeCli -ExpectedSourceVersion '2.20' -ExpectedServer localhost -ExpectedDatabase ut }
            catch { $blocked = $true }
            $blocked | Should Be $true
        }

        $escapeRoot = Join-Path $TestDrive 'matrix-escape-target'
        New-Item -ItemType Directory -Path $escapeRoot -Force | Out-Null
        $escapeMatrix = Join-Path $escapeRoot 'matrix.json'
        '{"outside":true}' | Set-Content -LiteralPath $escapeMatrix -Encoding UTF8
        $linkChildRoot = Join-Path $TestDrive 'reparse-child'
        $linkManifestPath = New-ChildEvidence -Root $linkChildRoot -Scope full -DatabaseName ut
        $linkPath = Join-Path $linkChildRoot 'matrix-link'
        $linkCreated = $false
        try {
            $linkType = if ($env:OS -eq 'Windows_NT') { 'Junction' } else { 'SymbolicLink' }
            $null = New-Item -ItemType $linkType -Path $linkPath -Target $escapeRoot -ErrorAction Stop
            $linkCreated = $true
        } catch {
            Write-Warning "Reparse-point containment test skipped: $($_.Exception.Message)"
        }
        if ($linkCreated) {
            $linked = Get-Content -Raw -LiteralPath $linkManifestPath | ConvertFrom-Json
            $linked.artifacts.matrix = 'matrix-link/matrix.json'
            $linked.artifact_sha256.matrix = Get-FileSha256 $escapeMatrix
            $linked | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $linkManifestPath -Encoding UTF8
            $reparseBlocked = $false
            try { $null = Read-ValidChildManifest -Path $linkManifestPath -ExpectedScope full -ExpectedCandidatePath $fakeCli -ExpectedSourceVersion '2.20' -ExpectedServer localhost -ExpectedDatabase ut }
            catch { $reparseBlocked = $true }
            $reparseBlocked | Should Be $true
        }
    }

    It "journals a failed child run in the top-level diagnostic manifest" {
        $lab = Join-Path $TestDrive "matrix-runtime-failure"
        $savedMode = $env:PARITY_FAKE_MODE
        try {
            $env:PARITY_FAKE_MODE = "exit"
            (Invoke-ExpectedFailureScript $matrix @(
                '-UtDbName','missing_matrix_ut','-BspDbName','missing_matrix_bsp','-IntegratedAuth',
                '-LabRoot',$lab,'-RunId','orchestrator_probe','-ExePath',$fakeCli,'-IbcmdPath',$fakeCli,
                '-SqlcmdExecutable',$fakeSqlcmd,'-BcpExecutable',$fakeBcp,'-Scope','scoped',
                '-PathPrefix','__manifest_probe__'
            )) | Should Not Be 0
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
