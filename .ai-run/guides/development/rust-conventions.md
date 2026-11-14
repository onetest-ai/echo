# Rust Conventions

**Project**: Echo · **Scope**: atomic writes, meta.json and state-file locking, call stamps, version bump

**Reference**: `.ai-run/reference/development/rust-conventions.md` — the helpers and files these rules rely on
(`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Files, locks and releases

### Rules
- R1. A file another thread or process may read is replaced atomically, never written in place. Evidence: `paths::write_atomic` (ref: Writes and locks)
- R2. An existing call's `meta.json` changes only through its update helper or under its lock. Evidence: `calls::update_meta` (ref: Writes and locks)
- R3. A shared JSON state file changes through its `update` helper: lock, re-read inside the lock, write atomically. Evidence: `plugins::Registry.update`
- R4. Cross-process locks are `app::FileLock`. Evidence: `app::FileLock`
- R5. A call is named by its folder stamp across renames, and a stamp two folders share finds neither. Evidence: `calls::by_stamp` (ref: Call stamps)
- R6. A release bumps every version file together, in one `Echo X.Y.Z` commit and never inside a feature PR. Evidence: `Cargo.toml::version` (ref: Version files)
