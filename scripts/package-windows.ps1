$ErrorActionPreference = "Stop"

$RootDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DistDir = Join-Path $RootDir "dist"

$CargoToml = Get-Content -Raw (Join-Path $RootDir "Cargo.toml")
if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    throw "Could not read version from Cargo.toml"
}

$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $RootDir "target" }
cargo build --locked --release --manifest-path (Join-Path $RootDir "Cargo.toml")
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed with exit code $LASTEXITCODE"
}

$StageDir = Join-Path $DistDir "stage"
$ZipPath = Join-Path $DistDir "block-clock-windows-x64.zip"

if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
if (Test-Path $ZipPath) { Remove-Item -Force $ZipPath }

New-Item -ItemType Directory -Force -Path $StageDir | Out-Null
Copy-Item (Join-Path $TargetDir "release\block-clock.exe") (Join-Path $StageDir "block-clock.exe")
Copy-Item (Join-Path $RootDir "README.md") (Join-Path $StageDir "README.md")
Copy-Item (Join-Path $RootDir "LICENSE") (Join-Path $StageDir "LICENSE")

Compress-Archive -Path "$StageDir\*" -DestinationPath $ZipPath -Force
Remove-Item -Recurse -Force $StageDir
Write-Host "Packaged: $ZipPath"

# Packaging MSIX for Microsoft Store
$MsixPath = Join-Path $DistDir "Block.Clock_${Version}_x64.msix"
if (Test-Path $MsixPath) { Remove-Item -Force $MsixPath }

$MsixStageDir = Join-Path $DistDir "msix-stage"
if (Test-Path $MsixStageDir) { Remove-Item -Recurse -Force $MsixStageDir }
New-Item -ItemType Directory -Force -Path $MsixStageDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $MsixStageDir "Assets") | Out-Null

Copy-Item (Join-Path $TargetDir "release\block-clock.exe") (Join-Path $MsixStageDir "block-clock.exe")
Copy-Item (Join-Path $RootDir "resources\msix\Assets\*") (Join-Path $MsixStageDir "Assets") -Recurse

$Publisher = if ($env:MSIX_PUBLISHER) { $env:MSIX_PUBLISHER } else { "CN=F9A61C2C-016B-4569-AF3E-D8FB4CBCB150" }
$ManifestContent = Get-Content -Raw (Join-Path $RootDir "resources\msix\Package.appxmanifest.xml")
$ManifestContent = $ManifestContent.Replace('__APP_VERSION__', "${Version}.0")
$ManifestContent = $ManifestContent.Replace('__PUBLISHER__', $Publisher)
Set-Content -Path (Join-Path $MsixStageDir "AppxManifest.xml") -Value $ManifestContent -Encoding UTF8

$MakeAppxCandidates = @(
    (Get-Command "MakeAppx.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue),
    (Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\MakeAppx.exe" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -ExpandProperty FullName -First 1),
    (Get-ChildItem "C:\Program Files\Windows Kits\10\bin\*\x64\MakeAppx.exe" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -ExpandProperty FullName -First 1)
) | Where-Object { $_ -and (Test-Path $_) }

$MakeAppxPath = $MakeAppxCandidates | Select-Object -First 1
if (-not $MakeAppxPath) {
    throw "MakeAppx.exe not found. Windows SDK is required to build the MSIX package."
}

& $MakeAppxPath pack /d $MsixStageDir /p $MsixPath /o /nv
if ($LASTEXITCODE -ne 0) {
    throw "MakeAppx.exe failed with exit code $LASTEXITCODE"
}
Write-Host "Packaged: $MsixPath"
Remove-Item -Recurse -Force $MsixStageDir
