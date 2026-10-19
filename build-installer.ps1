# OrbitConvert Windows Installer Build Script
param (
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "   OrbitConvert Windows Installer Build Pipeline" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $rootDir

# 1. Ensure Cargo & Compiler are in PATH
$cargoBin = "$env:USERPROFILE\.cargo\bin"
$winlibsBin = "C:\Users\REYMARK\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$innoBin = "C:\Users\REYMARK\AppData\Local\Programs\Inno Setup 6"
$env:PATH = "$cargoBin;$winlibsBin;$innoBin;$env:PATH"

# 2. Build optimized release binary
if (-not $SkipBuild) {
    Write-Host "`n[1/3] Building OrbitConvert optimized release binary..." -ForegroundColor Yellow
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo release build failed."
    }
} else {
    Write-Host "`n[1/3] Skipping cargo build as requested..." -ForegroundColor Yellow
}

# 3. Locate Inno Setup Compiler
$isccPath = Get-Command "iscc.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
if (-not $isccPath) {
    $candidate = "C:\Users\REYMARK\AppData\Local\Programs\Inno Setup 6\ISCC.exe"
    if (Test-Path $candidate) {
        $isccPath = $candidate
    } else {
        throw "Inno Setup compiler (ISCC.exe) not found. Please install via: winget install JRSoftware.InnoSetup"
    }
}

# 4. Compile Installer
Write-Host "`n[2/3] Compiling Windows Setup executable with Inno Setup..." -ForegroundColor Yellow
$issScript = Join-Path $rootDir "installer\OrbitConvert-Setup.iss"
& $isccPath $issScript

if ($LASTEXITCODE -ne 0) {
    throw "Inno Setup compiler returned error code $LASTEXITCODE."
}

# 5. Output Verification
$distDir = Join-Path $rootDir "installer\dist"
$setupExe = Get-ChildItem -Path $distDir -Filter "*.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1

Write-Host "`n[3/3] Build Completed Successfully!" -ForegroundColor Green
Write-Host "Installer created at:" -ForegroundColor White
Write-Host "  $($setupExe.FullName)" -ForegroundColor Cyan
Write-Host "File Size: $([Math]::Round($setupExe.Length / 1MB, 2)) MB" -ForegroundColor Gray
Write-Host "============================================================" -ForegroundColor Cyan
