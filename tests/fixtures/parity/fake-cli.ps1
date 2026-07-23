$command = if ($args.Count -gt 0) { [string]$args[0] } else { '' }

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
        throw 'fake terminating export failure'
    }
    if ($env:PARITY_FAKE_MODE -eq 'leak-exit') {
        Write-Output "IBCMD_DB_PSW=$env:IBCMD_DB_PSW --db-pwd=$env:IBCMD_DB_PSW"
        $global:LASTEXITCODE = 23
        return
    }
    Write-Error 'fake native export failure' -ErrorAction Continue
    $global:LASTEXITCODE = 23
    return
}

Write-Output "$command fake success"
$global:LASTEXITCODE = 0
