$ErrorActionPreference = "Stop"
$Version = if ($env:VERSION) { $env:VERSION } else { (Select-String -Path "Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value }
$Target = if ($env:TARGET) { $env:TARGET } else { "x86_64-pc-windows-gnu" }
$Dist = "dist/release/windows-$Target"

New-Item -ItemType Directory -Force -Path "crates/client-ui/assets" | Out-Null
if (Get-Command magick -ErrorAction SilentlyContinue) {
  magick "crates/client-ui/assets/logo.png" -define icon:auto-resize=256,128,64,48,32,16 "crates/client-ui/assets/logo.ico"
}

cargo build --release -p client-ui --target $Target
Remove-Item -Recurse -Force $Dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Dist | Out-Null
Copy-Item "target/$Target/release/client-ui.exe" "$Dist/rsmsg.exe"
Copy-Item "crates/client-ui/locales" "$Dist/locales" -Recurse

$DllNames = @("libgcc_s_seh-1.dll", "libstdc++-6.dll", "libwinpthread-1.dll")
$SearchDirs = @($env:WINDOWS_EXTRA_DLL_DIR, "$env:USERPROFILE\.cargo\bin", "C:\msys64\mingw64\bin") | Where-Object { $_ -and (Test-Path $_) }
foreach ($Dll in $DllNames) {
  foreach ($Dir in $SearchDirs) {
    $Path = Join-Path $Dir $Dll
    if (Test-Path $Path) { Copy-Item $Path $Dist; break }
  }
}

if (Get-Command makensis -ErrorAction SilentlyContinue) {
  makensis /DVERSION=$Version /DDIST_DIR=$Dist "scripts/release/windows-installer.nsi"
  Get-FileHash "$Dist/rsmsg-setup-$Version-x86_64.exe" -Algorithm SHA256 | ForEach-Object { "$($_.Hash.ToLower())  rsmsg-setup-$Version-x86_64.exe" } | Set-Content "$Dist/rsmsg-setup-$Version-x86_64.exe.sha256"
}
Get-FileHash "$Dist/rsmsg.exe" -Algorithm SHA256 | ForEach-Object { "$($_.Hash.ToLower())  rsmsg.exe" } | Set-Content "$Dist/rsmsg.exe.sha256"
