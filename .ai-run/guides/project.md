# Project Context

## Project Identity

| Field | Value | Source |
|---|---|---|
| Project name | Echo | README.md::Echo |
| Repository/package | echo | plugin/.claude-plugin/plugin.json::homepage |
| Project code/key | none | user decision |

## Work Item Tracker

| Field | Value |
|---|---|
| Provider | GitHub Issues |
| Key/prefix | none |

## Ticket Adapter

**Status**: configured
**Adapter**: gh CLI
**Lookup**: gh issue view <number> --repo onetest-ai/echo --json number,title,body,labels,state,comments
**Create**: gh issue create --repo onetest-ai/echo --title "<title>" --body-file <file>
**Output**: JSON from --json; issue URL on create
**Comment**: gh issue comment <number> --repo onetest-ai/echo --body-file <file>

## Source Control And Review

| Field | Value |
|---|---|
| Provider | GitHub |
| Repository remote | https://github.com/onetest-ai/echo |
| Default target branch | main |
| Review artifact type | PR |

## MR Adapter

**Status**: configured
**Adapter**: gh CLI
**Instructions**: Push the branch, then `gh pr create --repo onetest-ai/echo --base main --head <branch> --title "<subject>" --body-file <file>`; `gh pr view` / `gh pr checks` to follow it.

## Complexity Scoring

**Status**: not configured
**Field**: not configured
**Format**: not configured
