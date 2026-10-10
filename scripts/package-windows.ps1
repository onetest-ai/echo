# Build Echo for Windows beta testers: release build → zip with echoctl.exe.
# Install: unzip to %LOCALAPPDATA%\Programs\echo, run `echoctl.exe app autostart on`
# and double-click echoctl.exe (or `echoctl app start`).
# TODO: Authenticode signing (signtool) once a certificate is available.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value
cargo build --release -p echoctl
$stage = Join-Path $env:TEMP "echo-$version"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
Copy-Item target\release\echoctl.exe $stage
Copy-Item README.md $stage
Copy-Item LICENSE.md $stage
# The plugin travels with the app (`echoctl notes` runs Claude Code with --plugin-dir on it).
robocopy plugin "$stage\plugin" /E /XD bin node_modules "plugin\.claude-plugin\types" /XF *.test.ts | Out-Null
python scripts/third-party.py --texts "$stage\THIRD-PARTY-LICENSES.txt"
New-Item -ItemType Directory dist -Force | Out-Null
$zip = "dist\echo-$version-windows-x64.zip"
Compress-Archive -Path "$stage\*" -DestinationPath $zip -Force
Write-Output $zip
