Describe 'EDT corpus governance validator' {
    BeforeAll {
        $repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
        $validatorPath = Join-Path $repositoryRoot 'tools\validate-edt-corpus.ps1'
        $hostExecutableName = if ($PSVersionTable.PSEdition -eq 'Core') {
            'pwsh.exe'
        } else {
            'powershell.exe'
        }
        $hostExecutable = Join-Path $PSHOME $hostExecutableName
        if (-not (Test-Path -LiteralPath $hostExecutable -PathType Leaf)) {
            throw "PowerShell host executable not found: $hostExecutable"
        }
    }

    It 'accepts portable repository-relative paths and rejects hostile absolute paths' {
        $output = & $hostExecutable -NoLogo -NoProfile -File $validatorPath `
            -RepositoryRoot $repositoryRoot -SelfTest 2>&1

        if ($LASTEXITCODE -ne 0) {
            throw "EDT corpus governance self-tests failed with exit code $LASTEXITCODE."
        }
        if (($output -join [Environment]::NewLine) -notmatch
            'EDT corpus governance self-tests passed\.') {
            throw 'EDT corpus governance self-tests did not report success.'
        }
    }

    It 'accepts the tracked cleansed corpus' {
        $output = & $hostExecutable -NoLogo -NoProfile -File $validatorPath `
            -RepositoryRoot $repositoryRoot 2>&1

        if ($LASTEXITCODE -ne 0) {
            throw "EDT corpus validation failed with exit code $LASTEXITCODE."
        }
        if (($output -join [Environment]::NewLine) -notmatch
            'EDT-derived corpus governance validation passed\.') {
            throw 'EDT corpus validation did not report success.'
        }
    }
}
