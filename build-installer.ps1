# OrbitConvert Windows Installer Build Script (EXE + MSI)
param (
    [switch]$SkipBuild,
    [switch]$ExeOnly,
    [switch]$MsiOnly
)

$ErrorActionPreference = "Stop"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "   OrbitConvert Windows Installer Build Pipeline (EXE & MSI)" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $rootDir

# 1. Ensure Cargo & Compiler are in PATH
$cargoBin = "$env:USERPROFILE\.cargo\bin"
$winlibsBin = "C:\Users\REYMARK\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$innoBin = "C:\Users\REYMARK\AppData\Local\Programs\Inno Setup 6"
$wixBin = "C:\Program Files\WiX Toolset v7.0\bin"
$env:PATH = "$cargoBin;$winlibsBin;$innoBin;$wixBin;$env:PATH"

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

$buildExe = (-not $MsiOnly)
$buildMsi = (-not $ExeOnly)
$distDir = Join-Path $rootDir "installer\dist"
if (-not (Test-Path $distDir)) {
    New-Item -ItemType Directory -Path $distDir | Out-Null
}

# 3. Compile EXE Installer via Inno Setup
if ($buildExe) {
    Write-Host "`n[2/3] Compiling Setup EXE with Inno Setup..." -ForegroundColor Yellow
    $isccPath = Get-Command "iscc.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
    if (-not $isccPath) {
        $candidate = "C:\Users\REYMARK\AppData\Local\Programs\Inno Setup 6\ISCC.exe"
        if (Test-Path $candidate) {
            $isccPath = $candidate
        } else {
            throw "Inno Setup compiler (ISCC.exe) not found. Please install via: winget install JRSoftware.InnoSetup"
        }
    }
    $issScript = Join-Path $rootDir "installer\OrbitConvert-Setup.iss"
    & $isccPath $issScript
    if ($LASTEXITCODE -ne 0) {
        throw "Inno Setup compiler returned error code $LASTEXITCODE."
    }
}

# 4. Compile MSI Installer via WiX Toolset
if ($buildMsi) {
    Write-Host "`n[2b/3] Compiling Setup MSI with WiX Toolset v7..." -ForegroundColor Yellow
    $wixPath = Get-Command "wix.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
    if (-not $wixPath) {
        $candidateWix = "C:\Program Files\WiX Toolset v7.0\bin\wix.exe"
        if (Test-Path $candidateWix) {
            $wixPath = $candidateWix
        } else {
            throw "WiX compiler (wix.exe) not found. Please install via: winget install WiXToolset.WiXCLI"
        }
    }

    $installerDir = Join-Path $rootDir "installer"
    Push-Location $installerDir
    try {
        & $wixPath build -arch x64 OrbitConvert.wxs -ext WixToolset.UI.wixext -o dist\OrbitConvert-v0.1.0-Windows-x64.msi -pdbtype none
        if ($LASTEXITCODE -ne 0) {
            throw "WiX Toolset compiler returned error code $LASTEXITCODE."
        }
    } finally {
        Pop-Location
    }
}

# 5. Output Verification
Write-Host "`n[3/3] Build Completed Successfully!" -ForegroundColor Green
Write-Host "Artifacts in installer\dist:" -ForegroundColor White
Get-ChildItem -Path $distDir -Include "*.exe", "*.msi" -Recurse | ForEach-Object {
    Write-Host "  -> $($_.Name) ($([Math]::Round($_.Length / 1MB, 2)) MB)" -ForegroundColor Cyan
}
Write-Host "============================================================" -ForegroundColor Cyan
