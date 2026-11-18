# Rust Conventions — reference

The helpers and files the conventions rely on. It changes with the code, in the same PR. The rules are in
`.ai-run/guides/development/rust-conventions.md`.

## Writes and locks

- Atomic replace: `paths::write_atomic`.
- A new file only where none is (never replacing one, one of racing writers wins): `paths::place_new` — an exclusive
  rename (`renamex_np(RENAME_EXCL)` on macOS, `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on Windows); on a
  volume without it a hard link; with neither (exFAT and FAT on macOS, SMB shares without hard links) the name taken
  new (`O_EXCL`), then the file renamed over it — empty at the name for that moment.
- A call's `meta.json`: `calls::update_meta`; code that keeps its own copy holds `calls::lock_meta` while writing.
- A shared JSON state file's `update` helper locks, re-reads inside the lock, and writes atomically
  (`plugins::Registry.update`).
- Cross-process locks: `app::FileLock` (`try_take` / `try_take_shared`).

## Call stamps

A call is named by its folder stamp (`calls::stamp_of`) and found with `calls::by_stamp`, which returns `None` when two
folders share a stamp.

Every new call folder (a recording, a memo, a Teams transcript import) is made by `recorder::call_dir`: under
the lock `<home>/.echo/new-call.lock` (`recorder::new_call_lock`, also taken by `calls::restore`) it takes the first
second at or after the call's start that no folder in `<home>/calls` or the trash has as its stamp, and creates the
folder with `create_dir` (one that appeared meanwhile moves it a second on). The lock is waited for 2 s at most; when it
can't be had or opened, that is logged and the call goes on without it (a fresh folder still, its stamp possibly shared). A new call's stamp is therefore unique and never changes; `meta.json`'s `started_at` keeps the
real start, which may be a second or two before the stamp. Calls made before this have no migration: two that share
a stamp stay found by neither. The test-data makers pick stamps the same way (`make_data.py` `write_call`,
`recordings.py` `free_folder`).

## Version files

A version bump changes `Cargo.toml` `[workspace.package]` `version`, `Cargo.lock` and `plugin/.claude-plugin/plugin.json`.

## Exemplars

| When you are | Exemplar |
|---|---|
| changing a call's metadata | `calls::update_meta` |
| adding a JSON settings or registry file | `plugins::Registry.update` |
