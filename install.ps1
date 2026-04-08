# install.ps1 — One-line installer for indiana_business_dir prebuilt binaries on Windows
# Usage: Invoke-RestMethod -Uri https://raw.githubusercontent.com/millerjes37/indiana_business_dir/main/install.ps1 | Invoke-Expression

$ErrorActionPreference = "Stop"

$Repo = "millerjes37/indiana_business_dir"
$ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"

function Info($msg) {
    Write-Host "[install] $msg" -ForegroundColor Green
}

function Warn($msg) {
    Write-Host "[install] $msg" -ForegroundColor Yellow
}

function Error($msg) {
    Write-Host "[install] $msg" -ForegroundColor Red
    exit 1
}

# Detect architecture
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture
switch ($Arch) {
    "X64" { $Target = "x86_64-pc-windows-msvc" }
    default { Error "Unsupported architecture: $Arch. This installer only supports x86_64 on Windows." }
}

Info "Detected platform: $Target"

# Fetch latest release tag
Info "Fetching latest release from GitHub..."
try {
    $Response = Invoke-RestMethod -Uri $ApiUrl -UseBasicParsing
    $Tag = $Response.tag_name
    if ($Tag -match '^v') {
        $Version = $Tag.Substring(1)
    } else {
        $Version = $Tag
    }
} catch {
    Error "Could not determine latest release tag. GitHub API may be rate-limited or unavailable."
}

Info "Latest release: $Tag"

$ArchiveName = "indiana_business_dir-$Tag-$Target.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/$Tag/$ArchiveName"

# Determine install directories
$InstallDir = if ($env:INSTALL_DIR) { $env:INSTALL_DIR } else { "$env:LOCALAPPDATA\Microsoft\WindowsApps" }
$DataDir = if ($env:DATA_DIR) { $env:DATA_DIR } else { "$env:LOCALAPPDATA\indiana_business_dir" }

Info "Install directory: $InstallDir"
Info "Data directory: $DataDir"

# Create directories
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

# Download to temp
$TempDir = [System.IO.Path]::GetTempPath() + [System.Guid]::NewGuid().ToString()
New-Item -ItemType Directory -Force -Path $TempDir | Out-Null

$ArchivePath = Join-Path $TempDir $ArchiveName

Info "Downloading $ArchiveName..."
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ArchivePath -UseBasicParsing
} catch {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $TempDir
    Error "Failed to download $ArchiveName from $DownloadUrl"
}

# Extract
Info "Extracting archive..."
try {
    Expand-Archive -Path $ArchivePath -DestinationPath $TempDir -Force
} catch {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $TempDir
    Error "Failed to extract archive."
}

# The archive contains a single directory: indiana_business_dir/
$ExtractedDir = Join-Path $TempDir "indiana_business_dir"
if (-not (Test-Path $ExtractedDir)) {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $TempDir
    Error "Archive did not contain expected 'indiana_business_dir' directory."
}

# Move contents to DataDir
Info "Installing to $DataDir..."
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $DataDir
Move-Item -Path $ExtractedDir -Destination $DataDir

# Install Node dependencies
Info "Installing Node.js dependencies (this may take a minute)..."
Set-Location $DataDir
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    Warn "npm was not found in your PATH. Please install Node.js 18+ and run 'npm install' in $DataDir manually."
} else {
    npm install --silent 2>$null
}

# Copy binary to install dir
$BinaryName = "indiana_business_dir.exe"
$BinaryPath = Join-Path $DataDir $BinaryName
$DestPath = Join-Path $InstallDir $BinaryName

if (-not (Test-Path $BinaryPath)) {
    Error "Binary not found after extraction: $BinaryPath"
}

Info "Copying binary to $DestPath..."
Copy-Item -Path $BinaryPath -Destination $DestPath -Force

# Clean up temp
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $TempDir

# Check if install dir is on PATH
$PathDirs = ($env:PATH -split ';') | ForEach-Object { $_.TrimEnd('\') }
$NormalizedInstallDir = $InstallDir.TrimEnd('\')
if ($PathDirs -notcontains $NormalizedInstallDir) {
    Warn "$InstallDir is not in your PATH."
    Warn "You can add it by running the following command in an Administrator PowerShell:"
    Warn "  [Environment]::SetEnvironmentVariable('Path', `$env:Path + ';$InstallDir', 'User')"
    Warn "Or use the full path to run the binary: $DestPath"
}

Info "Installation complete!"
Info "Run '$BinaryName --help' to get started."
