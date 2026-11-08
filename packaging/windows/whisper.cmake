# CMake toolchain file for whisper.cpp in Windows (MSVC) builds — set through
# CMAKE_TOOLCHAIN_FILE_x86_64_pc_windows_msvc in .cargo/config.toml, which only that target reads.
#
# - Portable CPU code: GGML_NATIVE would compile for the build machine (AVX-512 on a recent
#   Ryzen) and crash with "illegal instruction" elsewhere. AVX2 + FMA + F16C = every x86-64 PC
#   since ~2015 (Intel Haswell, AMD Excavator/Zen).
# - Optimized: with the Visual Studio generator the cmake crate replaces CMake's release flags
#   with cc's and drops every /O flag, so whisper.cpp would build with /Od (~10× slower).
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
set(GGML_AVX ON CACHE BOOL "" FORCE)
set(GGML_AVX2 ON CACHE BOOL "" FORCE)
set(GGML_FMA ON CACHE BOOL "" FORCE)
set(GGML_F16C ON CACHE BOOL "" FORCE)
#   Release only: CMake's compiler check builds in Debug, where /O2 clashes with /RTC1.
add_compile_options("$<$<AND:$<CONFIG:Release>,$<COMPILE_LANGUAGE:C,CXX>>:/O2>")
add_compile_definitions("$<$<CONFIG:Release>:NDEBUG>")
# - The static C runtime in every configuration, a debug build's too (whisper-crt.cmake).
set(CMAKE_PROJECT_INCLUDE "${CMAKE_CURRENT_LIST_DIR}/whisper-crt.cmake")
