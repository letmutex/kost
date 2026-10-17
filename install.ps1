<#
.SYNOPSIS
    Installs or updates kost on Windows.

.DESCRIPTION
    PowerShell installation script for kost:
    1. Fetches latest release from GitHub.
    2. Checks local vs remote version, skipping if already up to date.
    3. Downloads archive and verifies SHA256 hash against release checksum.
    4. Extracts and installs binary to target directory.
    5. Cleans up archive and temporary files.
    6. Verifies PATH environment and notifies user if addition is needed.

.EXAMPLE
    irm https://raw.githubusercontent.com/letmutex/kost/main/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -InstallDir "C:\tools\kost" -Force
#>

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$InstallDir,

    [Parameter(Position = 1)]
    [string]$Version,

    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# Ensure TLS 1.2+ is enabled
[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor [System.Net.SecurityProtocolType]::Tls12

# Resolve configuration from parameters or environment variables
$repo = if ($env:KOST_REPO) { $env:KOST_REPO } else { "letmutex/kost" }

if (-not $Version -and $env:KOST_VERSION) {
    $Version = $env:KOST_VERSION
}

if (-not $Force -and ($env:KOST_FORCE -eq '1' -or $env:KOST_FORCE -eq 'true')) {
    $Force = $true
}

if (-not $InstallDir) {
    if ($env:KOST_INSTALL_DIR) {
        $InstallDir = $env:KOST_INSTALL_DIR
    } elseif ($env:LOCALAPPDATA) {
        $InstallDir = Join-Path $env:LOCALAPPDATA "kost\bin"
    } else {
        $InstallDir = Join-Path $HOME ".kost\bin"
    }
}

# 1. Fetch latest release version if not specified (unauthenticated)
$tag = $Version
if (-not $tag) {
    Write-Host "Fetching latest release information for $repo..."
    $headers = @{ "User-Agent" = "kost-installer" }

    try {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -Headers $headers -UseBasicParsing
        $tag = $release.tag_name
    } catch {
        # Fallback to HTTP redirect if GitHub API is rate-limited or unauthenticated
        try {
            $req = [System.Net.HttpWebRequest]::Create("https://github.com/$repo/releases/latest")
            $req.AllowAutoRedirect = $false
            $req.UserAgent = "kost-installer"
            $resp = $req.GetResponse()
            $location = $resp.GetResponseHeader("Location")
            $resp.Close()
            if ($location -match '/releases/tag/([^/]+)$') {
                $tag = $Matches[1]
            }
        } catch {
            throw "Failed to fetch latest release info from GitHub: $($_.Exception.Message)"
        }
    }

    if (-not $tag) {
        throw "Unable to determine latest release tag for $repo."
    }
}

# Detect architecture
$isArm64 = ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') -or ($env:PROCESSOR_ARCHITEW6432 -eq 'ARM64')
if (-not $isArm64) {
    try {
        $isArm64 = ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq [System.Runtime.InteropServices.Architecture]::Arm64)
    } catch {}
}

$assetSuffix = if ($isArm64) { "windows-arm64.zip" } else { "windows-x64.zip" }
$archiveName = "kost-$tag-$assetSuffix"
$hashName = "$archiveName.sha256"
$downloadUrl = "https://github.com/$repo/releases/download/$tag/$archiveName"
$hashUrl = "https://github.com/$repo/releases/download/$tag/$hashName"

# 2. Check local and remote version, skip if latest version is installed
$targetBinary = Join-Path $InstallDir "kost.exe"
$existingKost = $null
if (Test-Path -Path $targetBinary) {
    $existingKost = $targetBinary
} else {
    $cmd = Get-Command kost -ErrorAction SilentlyContinue
    if ($cmd) {
        $existingKost = $cmd.Source
    }
}

$localVersion = $null
$isUpdate = $false

if ($existingKost) {
    try {
        $rawVersion = (& $existingKost --version 2>$null) -join "`n"
        if ($rawVersion -match '([0-9]+\.[0-9]+\.[0-9]+[a-zA-Z0-9\.\-]*)') {
            $localVersion = $Matches[1]
            $isUpdate = $true
        }
    } catch {}
}

$cleanLocal = if ($localVersion) { ($localVersion -replace '^v', '').Trim() } else { $null }
$cleanRemote = ($tag -replace '^v', '').Trim()
if ($cleanLocal -and ($cleanLocal -eq $cleanRemote) -and (-not $Force)) {
    Write-Host "kost is already up to date ($tag). Skipping installation." -ForegroundColor Green
    return
}

# 5. Temporary directory setup with guaranteed cleanup in finally block
$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("kost-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

try {
    $tempArchive = Join-Path $tempDir $archiveName
    $tempHash = Join-Path $tempDir $hashName

    # 3. Download archive and verify hash (no auth headers)
    Write-Host "Downloading $archiveName..."
    Invoke-WebRequest -Uri $downloadUrl -OutFile $tempArchive -UseBasicParsing

    Write-Host "Downloading checksum $hashName..."
    Invoke-WebRequest -Uri $hashUrl -OutFile $tempHash -UseBasicParsing

    $hashContent = (Get-Content -Path $tempHash -Raw).Trim()
    $expectedHash = ($hashContent -split '\s+')[0].ToLowerInvariant()

    Write-Host "Verifying SHA256 checksum..."
    $actualHash = (Get-FileHash -Path $tempArchive -Algorithm SHA256).Hash.ToLowerInvariant()

    if ($expectedHash -ne $actualHash) {
        throw "SHA256 checksum mismatch`nExpected: $expectedHash`nActual:   $actualHash"
    }
    Write-Host "Checksum verified successfully." -ForegroundColor Green

    # 4. Extract to install
    Write-Host "Extracting archive..."
    $extractedDir = Join-Path $tempDir "extracted"
    Expand-Archive -Path $tempArchive -DestinationPath $extractedDir -Force

    $binaryPath = Join-Path $extractedDir "kost.exe"
    if (-not (Test-Path -Path $binaryPath)) {
        $found = Get-ChildItem -Path $extractedDir -Filter "kost.exe" -Recurse | Select-Object -First 1
        if ($found) {
            $binaryPath = $found.FullName
        } else {
            throw "kost.exe not found in extracted archive."
        }
    }

    if (-not (Test-Path -Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Safe replacement of existing binary (handle file locks)
    if (Test-Path -Path $targetBinary) {
        try {
            Copy-Item -Path $binaryPath -Destination $targetBinary -Force
        } catch {
            $oldBinary = Join-Path $InstallDir "kost.exe.old"
            if (Test-Path -Path $oldBinary) {
                Remove-Item -Path $oldBinary -Force -ErrorAction SilentlyContinue
            }
            Move-Item -Path $targetBinary -Destination $oldBinary -Force
            Copy-Item -Path $binaryPath -Destination $targetBinary -Force
        }
    } else {
        Copy-Item -Path $binaryPath -Destination $targetBinary -Force
    }
} finally {
    # 5. Cleanup archive and temporary files
    if (Test-Path -Path $tempDir) {
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# 6. Check PATH and print add install path to path env or update success message
$resolvedPath = Resolve-Path -Path $InstallDir -ErrorAction SilentlyContinue
$normalizedInstallDir = if ($resolvedPath) { $resolvedPath.Path.TrimEnd('\', '/') } else { $InstallDir.TrimEnd('\', '/') }

$pathEntries = ($env:Path -split [System.IO.Path]::PathSeparator) | ForEach-Object {
    $_.Trim().Trim('"').TrimEnd('\', '/')
}

$inPath = $false
foreach ($entry in $pathEntries) {
    if ($entry -and ($entry.Equals($normalizedInstallDir, [System.StringComparison]::OrdinalIgnoreCase))) {
        $inPath = $true
        break
    }
}

if ($inPath) {
    if ($isUpdate) {
        Write-Host "Successfully updated kost to $tag" -ForegroundColor Green
    } else {
        Write-Host "Successfully installed kost $tag to $targetBinary" -ForegroundColor Green
    }
    Write-Host "Run 'kost --help' to get started." -ForegroundColor Cyan
} else {
    if ($isUpdate) {
        Write-Host "Successfully updated kost to $tag" -ForegroundColor Green
    } else {
        Write-Host "Successfully installed kost $tag to $targetBinary" -ForegroundColor Green
    }
    Write-Host ""
    Write-Host "NOTICE: '$InstallDir' is not in your PATH environment variable." -ForegroundColor Yellow
    Write-Host "To run kost from the terminal, add it to your PATH:"
    Write-Host ""
    Write-Host "  To add permanently to your User PATH (PowerShell):" -ForegroundColor Cyan
    Write-Host "    `$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')"
    Write-Host "    [Environment]::SetEnvironmentVariable('Path', `"`$userPath;$InstallDir`", 'User')"
    Write-Host ""
    Write-Host "  To add to the current session only:" -ForegroundColor Cyan
    Write-Host "    `$env:Path += `";$InstallDir`""
    Write-Host ""
}
