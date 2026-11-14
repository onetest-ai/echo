# Git Workflow — reference

The repository, commands and files the git rules rely on. It changes with the code and the scripts, in the same PR. The
rules are in `.ai-run/guides/standards/git-workflow.md`.

## Repository

`onetest-ai/echo` on GitHub (`update::REPO`), worked with `gh`.

## Branch types

`feat`, `fix`, `refactor`, `chore`, `ci`, `docs`, `rules`.

## The Map line

- `python3 tests/tools/feature-map.py changes origin/main` prints the PR's `Map:` line (`feature-map.py::changes`).
- When it changes no behaviour: `Map: no behaviour change — <why>`; a rules-only PR: `Map: no behaviour change — rules`.
- `python3 tests/tools/feature-map.py check` checks the map.

## Rewriting history

`git push --force-with-lease`.

## Release

- What's new: the version's entry goes at the top of `crates/echoctl/whats-new.json`, 1 to 6 of the release's most useful
  changes, written as `crates/echoctl/WHATS-NEW.md` says; `cargo test` fails without it (`whats_new`).
- Version files: `.ai-run/reference/development/rust-conventions.md` (Version files).
- macOS: `scripts/package-macos.sh`, notarized with the App Store Connect API key (`APPLE_API_KEY`, `APPLE_API_KEY_ID`,
  `APPLE_API_ISSUER`).
- Publishing: `gh release create vX.Y.Z --target main` with the dmg.
- Windows: the `windows-release.yml` workflow runs on a published release and attaches the MSI and zip.
- Users update through Echo's own updater (`update::check`).
