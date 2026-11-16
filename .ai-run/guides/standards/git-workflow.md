# Git Workflow

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: repository, branches, commits, merges, releases

## Rules
- R1. The repository is `onetest-ai/echo` on GitHub; issues and pull requests go through `gh`. An issue is `#N`, no ticket keys. Evidence: `gh repo view`
- R2. Branch from `main` as `<type>/<kebab-case-description>`, type one of `feat`, `fix`, `refactor`, `chore`, `ci`, `docs`. Evidence: `git branch -r`
- R3. A commit subject is a plain sentence saying what changed, with an optional `Area:` prefix (`Memos: record yourself, no call`); a release commit is `Echo X.Y.Z`. No ticket key, no conventional-commit type. Evidence: `git log`
- R4. A PR references its issue (`Closes #N`), carries the reasoning and the alternatives weighed, and the `Map:` line that `python3 tests/tools/feature-map.py changes main` prints (or `Map: no behaviour change — <why>`); `python3 tests/tools/feature-map.py check` passes before push. Evidence: `tests/tools/feature-map.py::changes`
- R5. Merge into `main` with a merge commit, after its review. Evidence: `gh pr merge --merge`
- R6. Rewrite pushed history only on your own branch, with `git push --force-with-lease`; resolve rebase conflicts file by file, never by taking one side wholesale. Evidence: `git push --force-with-lease`
- R7. A release is an `Echo X.Y.Z` commit on `main` plus a published GitHub release, after every `critical` functional case passed on that commit; publishing runs `.github/workflows/windows-release.yml` (the Windows MSI and zip). Evidence: `.github/workflows/windows-release.yml`
