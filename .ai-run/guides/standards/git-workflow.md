# Git Workflow

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: branch names, commit subjects, merge policy and their recovery steps; exact gate commands live in `.ai-run/guides/quality-gates.md`, the MR adapter in `.ai-run/guides/project.md`

## Branch Naming Convention

- base branch: main
- pattern: `<type>/<kebab-case-description>`

| Type | Use for | Example |
|---|---|---|
| `feat/` | new features | `feat/memo-threads` |
| `fix/` | bug fixes | `fix/plugin-settings` |
| `refactor/` | restructuring without behaviour change | `refactor/brain-store` |
| `chore/` | maintenance, dependencies, releases prep | `chore/update-deps` |
| `ci/` | CI and packaging scripts | `ci/windows-release` |
| `docs/` | documentation and guides | `docs/ai-guides` |

### Rules
- R1. Branch from `main` as `<type>/<kebab-case-description>` with a type from the table above, and merge back to `main` through a GitHub pull request with a merge commit. Older branches (`brain-rework`, `plugins`) predate this pattern; don't rename them. No ticket prefix: the tracker is GitHub Issues, so reference an issue in the PR body (`Closes #N`).

## Commit Message Format

```
Area: Plain sentence saying what changed
```

Examples: `Memos: record yourself, no call; …` · `Echo X.Y.Z` (a release). The `Area:` prefix is optional.

### Rules
- R1. Commit subjects are a plain sentence saying what changed ("Memos: record yourself, no call; …"), with an optional `Area:` prefix; no ticket key and no conventional-commit type (convention from `git log`, no hook enforces it).

## Merge Strategy

### Rules
- R1. Merge into `main` by merge commit. Evidence: recorded user decision
- R2. A release is a commit `Echo X.Y.Z` on `main` plus a published GitHub release; publishing triggers `.github/workflows/windows-release.yml`, the only CI, which builds and uploads the Windows MSI and zip (nothing builds on push or pull request).

## Anti-Patterns

| Do not | Do instead | Why |
|---|---|---|
| Force-push a shared branch with `--force` | `git push --force-with-lease` after a rebase, and only on your own branch | a teammate's commits survive |

## Troubleshooting

| Issue | Fix |
|---|---|
| Branch was cut from the wrong base | `git rebase --onto main <old-base> <branch>`, then `git push --force-with-lease` |
| Rebase conflicts on `main` | resolve file by file, `git add`, `git rebase --continue`; `git rebase --abort` to restart; never resolve by taking one side wholesale |
