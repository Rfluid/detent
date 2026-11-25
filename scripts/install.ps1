<#
.SYNOPSIS
    Install (or update) detentctl on Windows.

.DESCRIPTION
    Remote install, latest release:
        irm https://raw.githubusercontent.com/Rfluid/detent/main/scripts/install.ps1 | iex
    Pin a release:
        $env:DETENT_VERSION = 'v0.1.0'; irm https://raw.githubusercontent.com/Rfluid/detent/main/scripts/install.ps1 | iex
    With options:
        & ([scriptblock]::Create((irm https://raw.githubusercontent.com/Rfluid/detent/main/scripts/install.ps1))) -Version v0.1.0 -DryRun

    Windows gets a preview: detentctl reads and changes the mouse's settings. The
    background service, remapping and gestures are Linux-only today.

    Where the files come from, detected automatically (mirror of scripts/install.sh):
      * bundle  : run from an extracted release zip (detentctl.exe next to this script).
      * source  : run from a git checkout with cargo on PATH: cargo build -p detentctl.
      * release : anything else (irm | iex): download detent-<tag>-<target>.zip and its
                  .zip.sha256 from GitHub Releases and verify the checksum.

    detentctl.exe and uninstall.ps1 go to %LOCALAPPDATA%\Programs\Detent, which is added
    to the user PATH. Running it again updates. Ends with platform notes and an opt-in
    sponsor prompt.

    Environment:
      DETENT_VERSION            Same as -Version.
      DETENT_NO_SPONSOR_PROMPT  Skip the closing sponsor prompt (also skipped when CI is
                                set or the session isn't interactive).

.PARAMETER Version
    Release tag to install (e.g. v0.1.0). Default: the latest release.

.PARAMETER DryRun
    Print the detected platform, the URLs and the install directory; change nothing.

.NOTES
    Keep the closing notes and sponsor prompt in step with install.sh (repo root) and
    scripts/install.sh. Asset contract: docs/dev/release.md.
    ASCII-only source: avoids PowerShell 5.1 UTF-8-without-BOM parse errors.
#>

param(
    [string]$Version,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

# PowerShell 5.1 defaults to TLS 1.0/1.1 on older Windows, which GitHub refuses.
try {
    [Net.ServicePointManager]::SecurityProtocol =
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {}

$Repo           = 'Rfluid/detent'
$ReleaseApiUrl  = "https://api.github.com/repos/$Repo/releases/latest"
$ReleaseBaseUrl = "https://github.com/$Repo/releases"
$IssuesUrl      = "https://github.com/$Repo/issues?q=is%3Aissue+is%3Aopen+label%3A"
$HelpWantedUrl  = "${IssuesUrl}%22help+wanted%22"
$SponsorUrl     = 'https://github.com/sponsors/Rfluid'
$PixUrl         = 'https://livepix.gg/rfluid'
$InstallDir     = Join-Path $env:LOCALAPPDATA 'Programs\Detent'

# Empty under `irm | iex`: then there is no bundle or checkout to install from.
$ScriptPath = $MyInvocation.MyCommand.Path

function Get-HostTarget {
    # x64 build on ARM64 too: Windows 11 on ARM runs it under emulation.
    switch ($env:PROCESSOR_ARCHITECTURE) {
        'AMD64' { return 'x86_64-pc-windows-msvc' }
        'ARM64' {
            Write-Warning 'No native ARM64 build yet; installing the x64 build (runs under emulation).'
            return 'x86_64-pc-windows-msvc'
        }
        default { throw "no Detent release for PROCESSOR_ARCHITECTURE=$($env:PROCESSOR_ARCHITECTURE)" }
    }
}

function Resolve-LatestVersion {
    $headers = @{
        'User-Agent' = 'detent-installer'   # required by api.github.com
        'Accept'     = 'application/vnd.github+json'
    }
    try {
        $resp = Invoke-RestMethod -Uri $ReleaseApiUrl -Headers $headers -UseBasicParsing
    } catch {
        throw "could not find a published release at $ReleaseApiUrl ($($_.Exception.Message))"
    }
    if (-not $resp.tag_name) { throw "GitHub returned no tag_name from $ReleaseApiUrl" }
    return $resp.tag_name
}

function Test-Sha256 {
    param([string]$File, [string]$ShaFile)
    $expected = (Get-Content $ShaFile -Raw).Trim().Split()[0].ToLower()
    $actual   = (Get-FileHash -Algorithm SHA256 $File).Hash.ToLower()
    if ($expected -ne $actual) {
        throw "SHA256 mismatch for $(Split-Path -Leaf $File) (expected $expected, got $actual); nothing was installed"
    }
}

function Add-ToUserPath {
    param([string]$Dir)
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $parts = @()
    if ($userPath) { $parts = $userPath -split ';' | Where-Object { $_ } }
    if ($parts -notcontains $Dir) {
        [Environment]::SetEnvironmentVariable('Path', (($parts + $Dir) -join ';'), 'User')
        Write-Host "> Added $Dir to your user PATH (new terminals pick it up)"
    }
    if (($env:Path -split ';') -notcontains $Dir) { $env:Path = "$env:Path;$Dir" }
}

function Write-ClosingNotes {
    Write-Host ''
    Write-Host "This is a preview: detentctl can read and change your mouse's settings. The"
    Write-Host "background service, remapping and gestures are open issues: ${IssuesUrl}platform%3Awindows"
    Write-Host ''
    Write-Host "Detent is early and community-built. Open issues come with ready-to-run agent prompts: $HelpWantedUrl"
}

function Invoke-SponsorPrompt {
    if ($env:DETENT_NO_SPONSOR_PROMPT -or $env:CI) { return }
    Write-Host ''
    Write-Host 'Detent is free and built by one person. If it helps you, please consider'
    Write-Host "sponsoring: $SponsorUrl"
    Write-Host "Prefer Pix (BRL)? $PixUrl"
    $interactive = [Environment]::UserInteractive -and -not [Console]::IsInputRedirected
    if (-not $interactive) { return }
    $answer = Read-Host 'Open the sponsor page now? [y/N]'
    if ($answer -match '^(y|yes)$') {
        try { Start-Process $SponsorUrl } catch { Write-Host "Open this link in your browser: $SponsorUrl" }
    }
}

function Install-Detent {
    if (-not $Version) { $Version = $env:DETENT_VERSION }
    if ($Version -and -not $Version.StartsWith('v')) { $Version = "v$Version" }

    $scriptDir = if ($ScriptPath) { Split-Path -Parent $ScriptPath } else { $null }
    $repoRoot  = if ($scriptDir) { Split-Path -Parent $scriptDir } else { $null }

    if ($scriptDir -and (Test-Path (Join-Path $scriptDir 'detentctl.exe'))) {
        $mode = 'bundle'
    } elseif ($repoRoot -and (Test-Path (Join-Path $repoRoot 'Cargo.toml')) -and
              (Get-Command cargo -ErrorAction SilentlyContinue) -and -not $Version) {
        $mode = 'source'
    } else {
        $mode = 'release'
    }

    $cleanup = $null
    if ($mode -eq 'release') {
        $target = Get-HostTarget
        if (-not $Version) {
            Write-Host '> Resolving the latest release...'
            if ($DryRun) {
                try { $Version = Resolve-LatestVersion } catch { $Version = '<latest>' }
            } else {
                $Version = Resolve-LatestVersion
            }
        }
        $base = "detent-$Version-$target"
        $url  = "$ReleaseBaseUrl/download/$Version/$base.zip"
        if ($DryRun) {
            Write-Host "mode:        release"
            Write-Host "arch:        $($env:PROCESSOR_ARCHITECTURE)"
            Write-Host "target:      $target"
            Write-Host "version:     $Version"
            Write-Host "archive:     $url"
            Write-Host "checksum:    $url.sha256"
            Write-Host "install dir: $InstallDir (added to the user PATH)"
            Write-ClosingNotes
            return
        }
        Write-Host "> Installing Detent $Version ($target)"
        $tmp = Join-Path ([IO.Path]::GetTempPath()) ("detent-install-" + [Guid]::NewGuid())
        New-Item -ItemType Directory -Force -Path $tmp | Out-Null
        $cleanup = $tmp
        try {
            $zip = Join-Path $tmp "$base.zip"
            Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $zip
            Invoke-WebRequest -UseBasicParsing -Uri "$url.sha256" -OutFile "$zip.sha256"
            Test-Sha256 -File $zip -ShaFile "$zip.sha256"
            Write-Host '> Checksum verified'
            Expand-Archive -Path $zip -DestinationPath $tmp -Force
            $stage = Join-Path $tmp $base
            if (-not (Test-Path (Join-Path $stage 'detentctl.exe'))) { $stage = $tmp }   # flat zip
        } catch {
            Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
            throw
        }
    } elseif ($mode -eq 'bundle') {
        $stage = $scriptDir
    } else {
        if ($DryRun) {
            Write-Host "mode:        source ($repoRoot)"
            Write-Host "would run:   cargo build --release --locked -p detentctl"
            Write-Host "install dir: $InstallDir (added to the user PATH)"
            Write-ClosingNotes
            return
        }
        Write-Host '> Building detentctl (release)...'
        Push-Location $repoRoot
        try {
            cargo build --release --locked -p detentctl
            if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
        } finally { Pop-Location }
        $stage = Join-Path $repoRoot 'target\release'
    }

    if ($DryRun) {
        Write-Host "mode:        $mode ($stage)"
        Write-Host "install dir: $InstallDir (added to the user PATH)"
        Write-ClosingNotes
        return
    }

    try {
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        $exe = Join-Path $InstallDir 'detentctl.exe'
        Copy-Item -Force (Join-Path $stage 'detentctl.exe') $exe
        # Downloaded files carry the internet zone mark; this one was checksum-verified.
        try { Unblock-File -Path $exe } catch {}
        $uninstall = if ($mode -eq 'source') { Join-Path $repoRoot 'scripts\uninstall.ps1' } else { Join-Path $stage 'uninstall.ps1' }
        if (Test-Path $uninstall) {
            Copy-Item -Force $uninstall (Join-Path $InstallDir 'uninstall.ps1')
        }
        Write-Host "> Installed detentctl.exe to $InstallDir"
        Add-ToUserPath -Dir $InstallDir
    } finally {
        if ($cleanup) { Remove-Item -Recurse -Force $cleanup -ErrorAction SilentlyContinue }
    }

    Write-Host ''
    Write-Host 'Detent installed. Try it with: detentctl devices   (then: detentctl settings list)'
    Write-Host "If Windows SmartScreen blocks the unsigned binary, choose 'More info' -> 'Run anyway'."
    Write-ClosingNotes
    Invoke-SponsorPrompt
}

Install-Detent
