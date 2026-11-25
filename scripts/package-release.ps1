<#
.SYNOPSIS
    Build and package the Detent release zip for Windows.

.DESCRIPTION
    scripts/package-release.ps1 -Version <tag> [-NoBuild]

    <tag> is the release tag (v0.1.0) or any label (main-abc1234). Produces:
      dist\detent-<tag>-<target>.zip
      dist\detent-<tag>-<target>.zip.sha256   (sha256sum format: "<hex>  <file name>")
    with <target> = x86_64-pc-windows-msvc. The zip holds one directory,
    detent-<tag>-<target>\, with detentctl.exe (the preview: device settings only),
    install.ps1, uninstall.ps1, README.md and LICENSE.
    Linux and macOS: scripts/package-release.sh. Contract: docs/dev/release.md.

.NOTES
    ASCII-only source: avoids PowerShell 5.1 UTF-8-without-BOM parse errors.
#>

param(
    [Parameter(Mandatory = $true)][string]$Version,
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $RepoRoot

if ($Version.StartsWith('-')) { throw "usage: package-release.ps1 -Version <tag> [-NoBuild]" }

switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { $Target = 'x86_64-pc-windows-msvc' }
    'ARM64' { $Target = 'aarch64-pc-windows-msvc' }
    default { throw "unsupported architecture $($env:PROCESSOR_ARCHITECTURE)" }
}

if (-not $NoBuild) {
    cargo build --release --locked -p detentctl
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
}

$Name  = "detent-$Version-$Target"
$Dist  = Join-Path $RepoRoot 'dist'
$Stage = Join-Path $Dist $Name
$Zip   = Join-Path $Dist "$Name.zip"
$Sha   = "$Zip.sha256"

foreach ($p in @($Stage, $Zip, $Sha)) {
    if (Test-Path $p) { Remove-Item -Recurse -Force $p }
}
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

Copy-Item 'target\release\detentctl.exe' (Join-Path $Stage 'detentctl.exe')
Copy-Item 'scripts\install.ps1'          (Join-Path $Stage 'install.ps1')
Copy-Item 'scripts\uninstall.ps1'        (Join-Path $Stage 'uninstall.ps1')
foreach ($f in @('README.md', 'LICENSE')) {
    if (Test-Path $f) { Copy-Item $f (Join-Path $Stage $f) }
}

# -Path <dir> (not <dir>\*) keeps the top-level directory in the archive, like the tarballs.
Compress-Archive -Path $Stage -DestinationPath $Zip

$hash = (Get-FileHash -Algorithm SHA256 $Zip).Hash.ToLower()
# LF and ASCII, so `sha256sum -c` reads it on any OS.
[IO.File]::WriteAllText($Sha, "$hash  $Name.zip`n", [Text.Encoding]::ASCII)
Remove-Item -Recurse -Force $Stage

Write-Host "dist\$Name.zip"
Write-Host "dist\$Name.zip.sha256"
