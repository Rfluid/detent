<#
.SYNOPSIS
    Uninstall detentctl on Windows (undoes scripts/install.ps1).

.DESCRIPTION
        irm https://raw.githubusercontent.com/Rfluid/detent/main/scripts/uninstall.ps1 | iex
    or run the copy the installer left next to detentctl.exe:
        & "$env:LOCALAPPDATA\Programs\Detent\uninstall.ps1"

    Removes %LOCALAPPDATA%\Programs\Detent and takes it off the user PATH. Your
    Detent config is kept.

.NOTES
    ASCII-only source: avoids PowerShell 5.1 UTF-8-without-BOM parse errors.
#>

$ErrorActionPreference = 'Stop'

$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\Detent'

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath) {
    $parts = $userPath -split ';' | Where-Object { $_ -and $_ -ne $InstallDir }
    $newPath = $parts -join ';'
    if ($newPath -ne $userPath) {
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        Write-Host "> Removed $InstallDir from your user PATH"
    }
}

if (Test-Path $InstallDir) {
    # This script may be running from inside $InstallDir; Windows lets a .ps1 be deleted
    # while PowerShell runs it (the text is already loaded).
    Remove-Item -Recurse -Force $InstallDir
    Write-Host "> Removed $InstallDir"
} else {
    Write-Host "> Nothing to remove at $InstallDir"
}

Write-Host 'Detent uninstalled. Your config was kept.'
