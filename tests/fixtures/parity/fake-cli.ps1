$command = if ($args.Count -gt 0) { [string]$args[0] } else { '' }
$script:cliArgs = [object[]]$args

function Get-Option([string]$Name) {
    $index = [Array]::IndexOf($script:cliArgs, $Name)
    if ($index -ge 0 -and $index + 1 -lt $script:cliArgs.Count) { return [string]$script:cliArgs[$index + 1] }
    return $null
}

function Write-RuntimeJournal($Value) {
    $path = Get-Option '--runtime-journal'
    if ($path) {
        [IO.File]::WriteAllText($path, ($Value | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    }
}

if ($command -eq '--version') {
    Write-Output 'ibcmd-rs fake 1.0.0'
    $global:LASTEXITCODE = 0
    return
}

if ($args.Count -gt 1 -and $args[1] -eq '--help') {
    Write-Output "$command fake help"
    $global:LASTEXITCODE = 0
    return
}

if ($command -eq 'dump-sources') {
    $runtimeCall = [ordered]@{
        executable=(Get-Option '--ibcmd')
        arguments=@(
            'infobase','config','export','--dbms=MSSQLServer',
            ('--db-server=' + (Get-Option '--db-server')),
            ('--db-name=' + (Get-Option '--db-name')),
            '--force','C:\fake-native-export'
        )
        started_unix_ms=1
        ended_unix_ms=2
        status='failed'
        exit_code=23
        timed_out=$false
        exception='fake native export failure'
    }
    if ($env:PARITY_FAKE_CAPTURE) {
        $capture = [ordered]@{
            db_user_present = -not [string]::IsNullOrEmpty([Environment]::GetEnvironmentVariable('IBCMD_DB_USR', 'Process'))
            db_password_present = -not [string]::IsNullOrEmpty([Environment]::GetEnvironmentVariable('IBCMD_DB_PSW', 'Process'))
        }
        [IO.File]::WriteAllText(
            $env:PARITY_FAKE_CAPTURE,
            ($capture | ConvertTo-Json),
            [System.Text.UTF8Encoding]::new($false)
        )
    }
    if ($env:PARITY_FAKE_MODE -eq 'throw') {
        $runtimeCall.exit_code = $null
        $runtimeCall.exception = 'fake terminating export failure'
        Write-RuntimeJournal ([ordered]@{ protocol_version=1; runtime_call=$runtimeCall })
        throw 'fake terminating export failure'
    }
    if ($env:PARITY_FAKE_MODE -eq 'leak-exit') {
        Write-RuntimeJournal ([ordered]@{ protocol_version=1; runtime_call=$runtimeCall })
        Write-Output "IBCMD_DB_PSW=$env:IBCMD_DB_PSW --db-pwd=$env:IBCMD_DB_PSW"
        $global:LASTEXITCODE = 23
        return
    }
    if ($env:PARITY_FAKE_MODE -in @('candidate-exit', 'candidate-stale-exit', 'candidate-outer-exit')) {
        $runtimeCall.status = 'passed'
        $runtimeCall.exit_code = 0
        $runtimeCall.exception = $null
        Write-RuntimeJournal ([ordered]@{ protocol_version=1; runtime_call=$runtimeCall })
        Write-Output 'fake native export success'
        $global:LASTEXITCODE = 0
        return
    }
    if ($env:PARITY_FAKE_MODE -eq 'native-outer-exit') {
        $runtimeCall.status = 'passed'
        $runtimeCall.exit_code = 0
        $runtimeCall.exception = $null
        Write-RuntimeJournal ([ordered]@{ protocol_version=1; runtime_call=$runtimeCall })
        Write-Error 'fake outer failure after nested ibcmd passed' -ErrorAction Continue
        $global:LASTEXITCODE = 31
        return
    }
    if ($env:PARITY_FAKE_MODE -eq 'native-stale-exit') {
        $runtimeCall.status = 'running'
        $runtimeCall.ended_unix_ms = $null
        $runtimeCall.exit_code = $null
        $runtimeCall.exception = $null
        Write-RuntimeJournal ([ordered]@{ protocol_version=1; runtime_call=$runtimeCall })
        $global:LASTEXITCODE = 32
        return
    }
    Write-RuntimeJournal ([ordered]@{ protocol_version=1; runtime_call=$runtimeCall })
    Write-Error 'fake native export failure' -ErrorAction Continue
    $global:LASTEXITCODE = 23
    return
}

if ($command -eq 'mssql-dump-config' -and $env:PARITY_FAKE_MODE -eq 'candidate-exit') {
    Write-RuntimeJournal ([ordered]@{
        protocol_version=1
        status='failed'
        server=(Get-Option '--server')
        database=(Get-Option '--database')
        started_unix_ms=1
        ended_unix_ms=2
        exception='fake candidate export failure'
        calls=@()
    })
    Write-Error 'fake candidate export failure' -ErrorAction Continue
    $global:LASTEXITCODE = 29
    return
}

if ($command -eq 'mssql-dump-config' -and $env:PARITY_FAKE_MODE -eq 'candidate-stale-exit') {
    Write-RuntimeJournal ([ordered]@{
        protocol_version=1
        status='running'
        server=(Get-Option '--server')
        database=(Get-Option '--database')
        started_unix_ms=3
        ended_unix_ms=$null
        exception=$null
        calls=@([ordered]@{
            executable=(Get-Option '--sqlcmd')
            arguments=@('-S',(Get-Option '--server'),'-Q',('<query-sha256:' + ('a' * 64) + '>'))
            started_unix_ms=4
            ended_unix_ms=$null
            status='running'
            exit_code=$null
            timed_out=$false
            exception=$null
        })
    })
    $global:LASTEXITCODE = 33
    return
}

if ($command -eq 'mssql-dump-config' -and $env:PARITY_FAKE_MODE -eq 'candidate-outer-exit') {
    Write-RuntimeJournal ([ordered]@{
        protocol_version=1
        status='passed'
        server=(Get-Option '--server')
        database=(Get-Option '--database')
        started_unix_ms=3
        ended_unix_ms=8
        exception=$null
        calls=@(
            [ordered]@{
                executable=(Get-Option '--sqlcmd')
                arguments=@('-S',(Get-Option '--server'),'-Q',('<query-sha256:' + ('a' * 64) + '>'))
                started_unix_ms=4; ended_unix_ms=5; status='passed'; exit_code=0
                timed_out=$false; exception=$null
            },
            [ordered]@{
                executable=(Get-Option '--bcp-executable')
                arguments=@(('<query-sha256:' + ('b' * 64) + '>'),'queryout','C:\fake-rows.bcp','-S',(Get-Option '--server'),'-T')
                started_unix_ms=6; ended_unix_ms=7; status='passed'; exit_code=0
                timed_out=$false; exception=$null
            }
        )
    })
    Write-Error 'fake outer failure after candidate journal passed' -ErrorAction Continue
    $global:LASTEXITCODE = 34
    return
}

Write-Output "$command fake success"
$global:LASTEXITCODE = 0
