# Run after each project() in whisper.cpp (CMAKE_PROJECT_INCLUDE, set in whisper.cmake): the static C
# runtime (/MT) in every configuration, as Rust links it (+crt-static, .cargo/config.toml).
#
# whisper-rs-sys always builds the Release configuration, but the cmake crate puts cc's flags (-MT)
# only into the configuration of the cargo profile: RelWithDebInfo for a debug build. Release then
# kept CMake's default /MD, and a debug `cargo build` / `cargo test` failed to link (LNK2019 on
# __imp_fgetc, __imp__aligned_malloc, …). whisper.cpp asks for CMake 3.14, before
# CMAKE_MSVC_RUNTIME_LIBRARY, so the flags themselves are rewritten.
foreach(lang C CXX)
  foreach(config DEBUG RELEASE MINSIZEREL RELWITHDEBINFO)
    set(v CMAKE_${lang}_FLAGS_${config})
    string(REPLACE "/MDd" "/MTd" ${v} "${${v}}")
    string(REPLACE "/MD" "/MT" ${v} "${${v}}")
  endforeach()
endforeach()
