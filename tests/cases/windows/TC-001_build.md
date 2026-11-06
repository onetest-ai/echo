---
id: TC-001
title: The Windows build and package
priority: critical
type: functional
requirements: [F-WIN-01]
module: windows
run: manual
platform: windows
tags: [windows, build]
external_id: TC-WINOS-001
---

# TC-001: The Windows build and package

**Module:** Windows | **Priority:** Critical | **Type:** Functional

## Preconditions
- Windows 10 2004+ or 11, Teams (new) installed; the build is unsigned (updates are checked by SHA-256)
- Rust ≥ 1.90 (MSVC), VS Build Tools (C++), CMake, LLVM (`LIBCLANG_PATH`), Vulkan SDK (`VULKAN_SDK`), Python

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `cargo build --release -p echoctl`, then `scripts\package-windows.ps1` | whisper.cpp compiled in (Vulkan + AVX2) |
| 2 | List the zip | `echoctl.exe`, `vulkan-1.dll`, `VULKAN-RUNTIME-LICENSE.txt`, `README.md`, `LICENSE.txt`, `THIRD-PARTY-LICENSES.txt`, `plugin\` |
| 3 | `dumpbin /dependents echoctl.exe` | No `MSVCP140` / `VCRUNTIME140` |
| 4 | Run it on a PC without a GPU driver (or a VM) | It starts and transcribes on the CPU |

## Expected Final State
A self-contained Windows package that runs with or without a GPU.
