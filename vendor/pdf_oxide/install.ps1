# Universal installer for pdf-oxide CLI (Windows)
# Usage: irm oxide.fyi/install.ps1 | iex
$ErrorActionPreference = "Stop"

$Repo = "yfedoseev/pdf_oxide"
$BinaryName = "pdf-oxide"
$InstallDir = if ($env:PDF_OXIDE_INSTALL_DIR) { $env:PDF_OXIDE_INSTALL_DIR } else { "$HOME\.pdf-oxide\bin" }

function Write-Info($msg) { Write-Host "  > $msg" -ForegroundColor Blue }
function Write-Err($msg) { Write-Host "  error: $msg" -ForegroundColor Red; exit 1 }

Write-Info "pdf-oxide installer"
Write-Info ""

# Get latest version
Write-Info "Fetching latest version..."
try {
    $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $Release.tag_name -replace '^v', ''
} catch {
    Write-Err "Could not determine latest version. Check https://github.com/$Repo/releases"
}

$Artifact = "pdf_oxide-windows-x86_64"
$Url = "https://github.com/$Repo/releases/download/v$Version/$Artifact-$Version.zip"

# Download
$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "pdf-oxide-install-$(Get-Random)"
New-Item -ItemType Directory -Force -Path $TmpDir | Out-Null
$ZipPath = Join-Path $TmpDir "archive.zip"

Write-Info "Downloading $BinaryName v$Version for Windows x86_64..."
try {
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing
} catch {
    Write-Err "Download failed: $_"
}

# Extract
Write-Info "Extracting..."
Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

$BinaryPath = Join-Path $TmpDir "$BinaryName.exe"
if (-not (Test-Path $BinaryPath)) {
    Write-Err "Binary '$BinaryName.exe' not found in archive"
}

# Install
Write-Info "Installing to $InstallDir..."
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Move-Item -Force -Path $BinaryPath -Destination (Join-Path $InstallDir "$BinaryName.exe")


# Add to PATH if not already present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$InstallDir;$UserPath", "User")
    Write-Info "Added $InstallDir to user PATH."
    Write-Info "Restart your terminal for PATH changes to take effect."
}

# Cleanup
Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue

Write-Info ""
Write-Info "Successfully installed $BinaryName v$Version to $InstallDir\$BinaryName.exe"
Write-Info "Run '$BinaryName --help' to get started."
