# Rust Conventions

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: atomic writes, meta.json and state-file locking, call stamps, version bump

## Files, locks and releases

### Rules
- R1. A file another thread or process may read is replaced with `paths::write_atomic`, never `fs::write`. Evidence: `paths::write_atomic`
- R2. An existing call's `meta.json` changes through `calls::update_meta`; code that keeps its own copy holds `calls::lock_meta` while writing. Evidence: `calls::update_meta`
- R3. A shared JSON state file changes through its `update` helper: lock, re-read inside the lock, write atomically. Evidence: `plugins::Registry.update`
- R4. Cross-process locks are `app::FileLock` (`try_take` / `try_take_shared`). Evidence: `app::FileLock`
- R5. A call is named by its folder stamp (`calls::stamp_of`) across renames and found with `calls::by_stamp`, which returns `None` when two folders share a stamp. Evidence: `calls::by_stamp`
- R6. A release bumps the version in `Cargo.toml` `[workspace.package]`, `Cargo.lock` and `plugin/.claude-plugin/plugin.json` together, in one `Echo X.Y.Z` commit and never inside a feature PR. Evidence: `Cargo.toml::version`

### Read first
| When you are | Exemplar |
|---|---|
| changing a call's metadata | `calls::update_meta` |
| adding a JSON settings or registry file | `plugins::Registry.update` |
