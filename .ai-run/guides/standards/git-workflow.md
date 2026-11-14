# Git Workflow

**Project**: Echo · **Scope**: repository, branches, commits, reviews, merges, releases

**Reference**: `.ai-run/reference/standards/git-workflow.md` — the repository, commands, variables and files these rules
rely on (`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Rules
- R1. Issues and pull requests live in Echo's GitHub repository and go through `gh`. An issue is `#N`, no ticket keys. Evidence: `update::REPO` (ref: Repository)
- R2. Branch from `main` as `<type>/<kebab-case-description>`. A `rules/` branch changes only the rules (the guides, `AGENTS.md`), and its PR title starts with `Rules:`. (ref: Branch types)
- R3. A commit subject is a plain sentence saying what changed, with an optional `Area:` prefix (`Memos: record yourself, no call`); a release commit is `Echo X.Y.Z`. No ticket key, no conventional-commit type.
- R4. A PR references its issue (`Closes #N`), carries the reasoning and the alternatives weighed, and the `Map:` line for its changes; the functional map's check passes before push. Evidence: `feature-map.py::changes` (ref: The Map line)
- R5. A PR is merged into `main` with a merge commit by its owner, without asking, once an independent review (a fresh reviewer given the code) has nothing open and the gates and its functional cases pass. A `Rules:` PR is merged only on the user's yes.
- R6. Rewrite pushed history only on your own branch, with a lease; resolve rebase conflicts file by file, never by taking one side wholesale. (ref: Rewriting history)
- R7. Cut a release in this order:
  1. A version-bump PR `Echo X.Y.Z` (the bump: rust-conventions R6) that adds the version's What's new entry, in the user's words; merged after the release's test bar (quality-gates R5) passes on it.
  2. The macOS package from that commit, signed and notarized.
  3. A GitHub release from `main` with the dmg, published at once, never a draft; Windows packages are attached by the workflow the published release starts.
  4. Users update through Echo's own updater.

  Evidence: `windows-release.yml::windows`, `update::check`, `whats_new` (ref: Release)
- R8. A review reports its findings, each checked against the code (real, speculative, or not a bug) and ranked; it changes no code until the user decides what to fix.
