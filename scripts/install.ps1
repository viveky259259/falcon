# Falcon installer for Windows.
#
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/viveky259259/falcon/master/scripts/install.ps1 | iex
#
# Downloads the falcon / falcon-lsp / falcon-mcp binaries from a GitHub
# Release, verifies them against SHA256SUMS, installs them to
# $env:FALCON_INSTALL_DIR (default: %LOCALAPPDATA%\Programs\falcon\bin) and
# adds that folder to your user PATH. No Rust toolchain, npm or admin rights
# required. Works in Windows PowerShell 5.1 and PowerShell 7.
#
# Options (parameters when run as a file, or matching env vars with irm | iex):
#   -Version X.Y.Z    FALCON_VERSION          Install a specific release (default: latest)
#   -InstallDir DIR   FALCON_INSTALL_DIR      Where to put the binaries
#   -NoModifyPath     FALCON_NO_MODIFY_PATH=1 Don't add the folder to your user PATH

param(
    [string]$Version = $env:FALCON_VERSION,
    [string]$InstallDir = $env:FALCON_INSTALL_DIR,
    [switch]$NoModifyPath
)

$ErrorActionPreference = 'Stop'
# Invoke-WebRequest in Windows PowerShell 5.1 is very slow with its progress bar.
$ProgressPreference = 'SilentlyContinue'

function Write-Step([string]$Message) {
    Write-Host "==> $Message" -ForegroundColor Blue
}

# Throw rather than exit: under `irm | iex`, exit would close the user's shell.
function Stop-Install([string]$Message) {
    throw "falcon install: $Message"
}

function Install-Falcon {
    # GitHub requires TLS 1.2; Windows PowerShell 5.1 may not offer it by default.
    [Net.ServicePointManager]::SecurityProtocol =
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = $env:FALCON_RELEASE_REPO
    if (-not $repo) { $repo = 'viveky259259/falcon' }

    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) { $arch = $env:PROCESSOR_ARCHITEW6432 }
    if ($arch -ne 'AMD64') {
        Stop-Install "Windows $arch has no prebuilt release yet. Run 'cargo install falcon-flutter' instead."
    }
    $label = 'windows-x64'

    $installDir = $InstallDir
    if (-not $installDir) { $installDir = Join-Path $env:LOCALAPPDATA 'Programs\falcon\bin' }
    $modifyPath = -not ($NoModifyPath -or $env:FALCON_NO_MODIFY_PATH -eq '1')

    $ver = $Version
    if (-not $ver) {
        Write-Step 'Looking up the latest release...'
        try {
            $release = Invoke-RestMethod -UseBasicParsing `
                -Uri "https://api.github.com/repos/$repo/releases/latest" `
                -Headers @{ 'User-Agent' = 'falcon-installer' }
        } catch {
            Stop-Install "could not reach GitHub to resolve the latest version ($($_.Exception.Message)). Set FALCON_VERSION to skip this lookup."
        }
        $ver = [string]$release.tag_name
    }
    $ver = $ver.TrimStart('v')
    if (-not $ver) { Stop-Install 'could not determine the version to install.' }

    $archive = "falcon-$ver-$label.zip"
    $baseUrl = "https://github.com/$repo/releases/download/v$ver"
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("falcon-install-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp | Out-Null

    try {
        $archivePath = Join-Path $tmp $archive
        $sumsPath = Join-Path $tmp 'SHA256SUMS'
        # Local files instead of downloads, for testing this script in CI.
        if ($env:FALCON_LOCAL_ARCHIVE -and $env:FALCON_LOCAL_SHA256SUMS) {
            Copy-Item $env:FALCON_LOCAL_ARCHIVE $archivePath
            Copy-Item $env:FALCON_LOCAL_SHA256SUMS $sumsPath
        } else {
            foreach ($file in @($archive, 'SHA256SUMS')) {
                Write-Step "Downloading $file..."
                try {
                    Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/$file" `
                        -OutFile (Join-Path $tmp $file) `
                        -Headers @{ 'User-Agent' = 'falcon-installer' }
                } catch {
                    Stop-Install "download failed: $baseUrl/$file ($($_.Exception.Message))"
                }
            }
        }

        Write-Step 'Verifying checksum...'
        $expected = $null
        foreach ($line in Get-Content $sumsPath) {
            $parts = $line.Trim() -split '\s+', 2
            if ($parts.Count -eq 2 -and $parts[1].TrimStart('*') -eq $archive) {
                $expected = $parts[0].ToLowerInvariant()
                break
            }
        }
        if (-not $expected) { Stop-Install "SHA256SUMS in release v$ver has no entry for $archive." }
        $actual = (Get-FileHash -Algorithm SHA256 -Path $archivePath).Hash.ToLowerInvariant()
        if ($expected -ne $actual) {
            Stop-Install "checksum mismatch for $archive (expected $expected, got $actual)."
        }

        Write-Step "Installing to $installDir..."
        $extract = Join-Path $tmp 'extract'
        Expand-Archive -Path $archivePath -DestinationPath $extract -Force
        $staging = Join-Path $extract "falcon-$ver-$label"
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
        foreach ($bin in @('falcon', 'falcon-lsp', 'falcon-mcp')) {
            $source = Join-Path $staging "$bin.exe"
            if (-not (Test-Path $source -PathType Leaf)) {
                Stop-Install "release archive did not contain $bin.exe."
            }
            try {
                Copy-Item $source (Join-Path $installDir "$bin.exe") -Force
            } catch {
                Stop-Install "could not replace $bin.exe in $installDir; close any running Falcon (or your editor's Falcon extension) and try again."
            }
        }
        Write-Step "Installed falcon $ver to $installDir"
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    # @() keeps a one-entry PATH an array, so + appends instead of concatenating.
    $entries = @()
    if ($userPath) { $entries = @($userPath -split ';' | Where-Object { $_ }) }
    $onPath = @($entries | Where-Object { $_.TrimEnd('\') -ieq $installDir.TrimEnd('\') })
    if ($onPath.Count -gt 0) { $onPath = $true } else { $onPath = $false }
    if (-not $onPath) {
        if ($modifyPath) {
            [Environment]::SetEnvironmentVariable('Path', (($entries + $installDir) -join ';'), 'User')
            $env:Path = "$env:Path;$installDir"
            Write-Step "Added $installDir to your user PATH. Open a new terminal to use 'falcon' there."
        } else {
            Write-Warning "$installDir is not on your PATH. Add it to run 'falcon' from any folder."
        }
    }

    & (Join-Path $installDir 'falcon.exe') --version
}

Install-Falcon
