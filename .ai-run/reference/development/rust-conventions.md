# Rust Conventions — reference

The helpers and files the conventions rely on. It changes with the code, in the same PR. The rules are in
`.ai-run/guides/development/rust-conventions.md`.

## Writes and locks

- Atomic replace: `paths::write_atomic`.
- A new file only where none is (never replacing one, one of racing writers wins): `paths::place_new` — an exclusive
  rename (`renamex_np(RENAME_EXCL)` on macOS, `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on Windows); on a
  volume without it a hard link; with neither (exFAT, FAT, many SMB/NFS shares) the name taken new (`O_EXCL`), then
  the file renamed over it.
- A call's `meta.json`: `calls::update_meta`; code that keeps its own copy holds `calls::lock_meta` while writing.
- A shared JSON state file's `update` helper locks, re-reads inside the lock, and writes atomically
  (`plugins::Registry.update`).
- Cross-process locks: `app::FileLock` (`try_take` / `try_take_shared`).

## Call stamps

A call is named by its folder stamp (`calls::stamp_of`) and found with `calls::by_stamp`, which returns `None` when two
folders share a stamp.

## Version files

A version bump changes `Cargo.toml` `[workspace.package]` `version`, `Cargo.lock` and `plugin/.claude-plugin/plugin.json`.

## Exemplars

| When you are | Exemplar |
|---|---|
| changing a call's metadata | `calls::update_meta` |
| adding a JSON settings or registry file | `plugins::Registry.update` |
