# Git Workflow

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: repository, branches, commits, reviews, merges, releases

## Rules
- R1. The repository is `onetest-ai/echo` on GitHub; issues and pull requests go through `gh`. An issue is `#N`, no ticket keys. Evidence: `update::REPO`
- R2. Branch from `main` as `<type>/<kebab-case-description>`, type one of `feat`, `fix`, `refactor`, `chore`, `ci`, `docs`, `rules`. A `rules/` branch changes only the rules (the guides, `AGENTS.md`), and its PR title starts with `Rules:`.
- R3. A commit subject is a plain sentence saying what changed, with an optional `Area:` prefix (`Memos: record yourself, no call`); a release commit is `Echo X.Y.Z`. No ticket key, no conventional-commit type.
- R4. A PR references its issue (`Closes #N`), carries the reasoning and the alternatives weighed, and the `Map:` line that `python3 tests/tools/feature-map.py changes origin/main` prints (`Map: no behaviour change — <why>` when it changes none; a rules-only PR: `Map: no behaviour change — rules`); `python3 tests/tools/feature-map.py check` passes before push. Evidence: `feature-map.py::changes`
- R5. A PR is merged into `main` with a merge commit by its owner, without asking, once an independent review (a fresh reviewer given the code) has nothing open and the gates and its functional cases pass. A `Rules:` PR is merged only on the user's yes.
- R6. Rewrite pushed history only on your own branch, with `git push --force-with-lease`; resolve rebase conflicts file by file, never by taking one side wholesale.
- R7. Cut a release in this order:
  1. A version-bump PR `Echo X.Y.Z` (the bump: rust-conventions R6), merged after the release's test bar (quality-gates R5) passes on it.
  2. `scripts/package-macos.sh` from that commit, notarized with the App Store Connect API key (`APPLE_API_KEY`, `APPLE_API_KEY_ID`, `APPLE_API_ISSUER`).
  3. `gh release create vX.Y.Z --target main` with the dmg, published at once, never a draft. The Windows workflow runs only on a published release and attaches the MSI and zip.
  4. Users update through Echo's own updater.

  Evidence: `windows-release.yml::windows`, `update::check`
- R8. A review reports its findings, each checked against the code (real, speculative, or not a bug) and ranked; it changes no code until the user decides what to fix.
