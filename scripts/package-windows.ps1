# Build Echo for Windows beta testers: release build -> dist\echo-<version>-windows-x64.msi
# (per-user installer: Start menu, PATH, start at login; packaging/windows/echo.wxs) and the
# same files as a .zip (unzip to %LOCALAPPDATA%\Programs\echo, `echoctl.exe app autostart on`,
# double-click echoctl.exe).
#
# Needs: Rust (MSVC), Visual Studio Build Tools (C++; CMake comes with them), LLVM
# (LIBCLANG_PATH) and the Vulkan SDK (VULKAN_SDK) - build machine only. The zip runs on any
# x64 Windows 10 2004+: whisper.cpp is linked in (AVX2, static C runtime, .cargo/config.toml)
# and uses the GPU through Vulkan when a driver offers one, else the CPU.
# TODO: Authenticode signing (signtool) once a certificate is available.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value

if (-not $env:VULKAN_SDK) { throw "VULKAN_SDK is not set - install the Vulkan SDK (winget install KhronosGroup.VulkanSDK)" }
# CMake lives inside the Build Tools when it isn't on PATH.
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    $vs = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath
    $env:PATH = "$vs\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin;$env:PATH"
}
# A short target dir: ggml's Vulkan shader generator is a nested CMake project whose MSBuild
# tracking logs (~220 chars below the target dir) exceed MAX_PATH (260) under
# target\release\build\... (error FTK1011); %LOCALAPPDATA%\echo-build is already 258 for a
# 4-letter user name. $env:ECHO_BUILD_DIR overrides it (keep it short).
$target = if ($env:ECHO_BUILD_DIR) { $env:ECHO_BUILD_DIR } else { Join-Path $env:SystemDrive "echo-build" }
cargo build --release -p echoctl --features vulkan --target-dir $target
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$stage = Join-Path $env:TEMP "echo-$version"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
Copy-Item "$target\release\echoctl.exe" $stage
Copy-Item README.md $stage
Copy-Item LICENSE.md $stage

# The Vulkan loader next to the exe: echoctl.exe imports vulkan-1.dll, which GPU drivers install
# but a machine without one (VM, bare office PC) lacks - Windows would refuse to start Echo.
# With the loader present and no driver, whisper.cpp quietly runs on the CPU.
$vkVersion = Split-Path $env:VULKAN_SDK -Leaf
$rtZip = Join-Path $env:TEMP "VulkanRT-$vkVersion.zip"
$rtDir = Join-Path $env:TEMP "VulkanRT-$vkVersion"
Invoke-WebRequest "https://sdk.lunarg.com/sdk/download/$vkVersion/windows/VulkanRT-X64-$vkVersion-Components.zip" -OutFile $rtZip
Remove-Item -Recurse -Force $rtDir -ErrorAction SilentlyContinue
Expand-Archive $rtZip -DestinationPath $rtDir
$loader = Get-ChildItem $rtDir -Recurse -Filter vulkan-1.dll | Where-Object { $_.Directory.Name -eq "x64" } | Select-Object -First 1
$sig = Get-AuthenticodeSignature $loader.FullName
if ($sig.Status -ne "Valid" -or $sig.SignerCertificate.Subject -notmatch 'CN="LunarG, Inc\."') {
    throw "vulkan-1.dll is not signed by LunarG ($($sig.Status))"
}
Copy-Item $loader.FullName $stage
Copy-Item (Get-ChildItem $rtDir -Recurse -Filter VulkanRT-License.txt | Select-Object -First 1).FullName "$stage\VULKAN-RUNTIME-LICENSE.txt"

# The plugin travels with the app (`echoctl notes` runs Claude Code with --plugin-dir on it).
robocopy plugin "$stage\plugin" /E /XD bin node_modules "plugin\.claude-plugin\types" /XF *.test.ts | Out-Null
python scripts/third-party.py --texts "$stage\THIRD-PARTY-LICENSES.txt"
if ($LASTEXITCODE -ne 0) { throw "third-party.py failed" }
New-Item -ItemType Directory dist -Force | Out-Null
$zip = "dist\echo-$version-windows-x64.zip"
# Windows' bsdtar, not Compress-Archive: PowerShell 5.1 writes backslash separators, which
# other unzip tools take as part of the file name (plugin\scripts\...).
# By full path: Git's GNU tar (first on PATH in Git Bash) can't write zips.
Remove-Item $zip -ErrorAction SilentlyContinue
$entries = (Get-ChildItem $stage).Name
& "$env:SystemRoot\System32\tar.exe" -a -c -f $zip -C $stage @entries
if ($LASTEXITCODE -ne 0) { throw "creating $zip failed" }
Write-Output $zip

# The installer (packaging/windows/echo.wxs): the same files, per user, no admin rights.
# WiX 5 (MS-RL; later versions add a maintenance-fee EULA), either installed (admin):
#   winget install WiXToolset.WiXCLI --version 5.0.2.0
# or, without admin rights, the NuGet tool package run on any .NET >= 6 runtime:
#   unzip https://www.nuget.org/api/v2/package/wix/5.0.2 to %LOCALAPPDATA%\wix5-tool
$wix = @(Get-Command wix -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source)
if (-not $wix) { $wix = @(Get-ChildItem "$env:ProgramFiles\WiX Toolset v5*\bin\wix.exe" -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName) }
if (-not $wix -and (Test-Path "$env:LOCALAPPDATA\wix5-tool\tools\net6.0\any\wix.dll")) {
    $env:DOTNET_ROLL_FORWARD = "Major"
    $wix = @("dotnet", "$env:LOCALAPPDATA\wix5-tool\tools\net6.0\any\wix.dll")
}
if (-not $wix) { throw "WiX 5 is not installed - winget install WiXToolset.WiXCLI --version 5.0.2.0" }
$msi = "dist\echo-$version-windows-x64.msi"
$msiVersion = ($version -split '-')[0] # MSI versions are numeric only
$wixArgs = @($wix | Select-Object -Skip 1) + @("build", "packaging\windows\echo.wxs", "-arch", "x64", "-d", "Version=$msiVersion", "-d", "Stage=$stage", "-o", $msi)
& $wix[0] @wixArgs
if ($LASTEXITCODE -ne 0) { throw "building $msi failed" }
Remove-Item "dist\echo-$version-windows-x64.wixpdb" -ErrorAction SilentlyContinue
Write-Output $msi
