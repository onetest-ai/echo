# Rust Conventions

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: atomic writes, meta.json and state-file locking, call stamps, version bump

## Files, locks and releases

### Rules
- R1. Replace a file another thread or process may read with `paths::write_atomic` (own temp file, then rename), not `fs::write` (a reader would see it half written). Evidence: `paths::write_atomic`
- R2. Change an existing call's `meta.json` through `calls::update_meta` (lock `<call>/.meta.lock`, re-read, `write_atomic`); code that keeps its own copy holds `calls::lock_meta` while writing, as `recorder::write_final_meta` does. Evidence: `calls::update_meta`
- R3. Change a shared JSON state file through its `update` helper: take the lock, re-read the file inside it, write atomically (two writers never undo each other). Evidence: `plugins::Registry.update`
- R4. Take cross-process locks with `app::FileLock` (`try_take` / `try_take_shared`): `flock` on Unix, share modes on Windows, released by the OS when the holder dies. Evidence: `app::FileLock`
- R5. A call is named by its folder stamp (`calls::stamp_of`) across renames; find it again with `calls::by_stamp`, which returns `None` rather than guess when two folders share a stamp. Evidence: `calls::by_stamp`
- R6. Release by bumping the version in `Cargo.toml` `[workspace.package]`, `Cargo.lock` and `plugin/.claude-plugin/plugin.json` together in one `Echo X.Y.Z` commit (the release commits 23f9d33 and be64ae3 touch exactly these). Evidence: `Cargo.toml::version`

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| changing a call's metadata | `calls::update_meta` | no covering test |
| adding a JSON settings or registry file | `plugins::Registry.update` | `plugins::install_update_and_remove_from_a_local_marketplace` |
