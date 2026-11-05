# Git Workflow

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: branch names, commit subjects, merge policy and their recovery steps; exact gate commands live in `.ai-run/guides/quality-gates.md`, the MR adapter in `.ai-run/guides/project.md`

## Branch Naming Convention

- base branch: main
- pattern: short free-form feature name (e.g. `brain-rework`, `fix-plugin-settings`); no prefix

### Rules
- R1. Feature work goes on a named branch (`brain-rework`, `plugins`, `obsidian-a`) merged to `main` through a GitHub pull request with a merge commit (`Merge pull request #75 from onetest-ai/brain-rework`); small commits also land on `main` directly (convention from history; no branch protection evidence in the repository).

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
