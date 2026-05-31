---
name: work-on-issue
description: Implement a GitHub Issue end-to-end. Reads the issue, plans implementation with the user, creates a branch, implements with CI checks, records progress on the issue, and invokes create-pull-request when done.
---

Implement a GitHub Issue from start to PR. Follow every step below in order.

## Step 1: Select the issue

If the user provided an issue number (e.g. `/work-on-issue 42`), use it. Otherwise, fetch open issues with `gh issue list --state open` and present them for the user to choose.

Read the full issue body with `gh issue view <number> --json body --jq '.body'`.

## Step 2: Detect and report previous progress

Check for existing work on this issue:

1. Check if a branch matching `<prefix>/<issue-number>-*` exists: `git branch --list '*<issue-number>*'`
2. Check if a PR already exists: `gh pr list --head '*<issue-number>*'`
3. Check issue comments for progress logs: `gh issue view <number> --comments`

Report findings to the user:
- If a branch exists: "Branch `feature/42-add-foo` already exists with N commits. Resume from there?"
- If a PR exists: "PR #N is already open. What would you like to do?"
- If nothing found: "No previous work found. Starting fresh."

## Step 3: Investigate and plan

### 3a: Investigate the codebase

Read the Issue spec thoroughly. Then investigate the relevant code — read the files mentioned in `Scope`, trace dependencies, understand the current implementation. Do NOT ask questions that can be answered by reading the Issue, code, or research.

### 3b: Plan implementation

Based on the investigation, create an implementation plan:
- List specific files to create or modify
- Describe the changes for each file
- Estimate the total line count

### 3c: Check PR split necessity

If the estimated changes exceed ~400 lines, propose splitting into multiple PRs. Present the split plan to the user. Each sub-task should be independently mergeable.

If a split is agreed upon, post it as an Issue comment:
```
## PR Split Plan

1. **PR 1: <title>** — <scope> (~N lines)
2. **PR 2: <title>** — <scope> (~N lines)
```

### 3d: Discuss with the user

Present the plan and discuss thoroughly with the user until shared understanding is reached. Resolve every question — do not proceed with ambiguity. One topic at a time.

## Step 4: Create branch and start implementation

After the user approves the plan:

1. Ensure you are on `develop` and up to date: `git checkout develop && git pull`
2. Create the branch following CLAUDE.md naming: `git checkout -b <prefix>/<issue-number>-<description>`
   - Prefix: `feat`, `fix`, `refactor`, `docs`, `chore` (based on issue type)
   - Description: short kebab-case summary

**Important**: Unless the user explicitly specifies otherwise, always branch from `develop` and target `develop` for PRs. Never branch from or target `main`.

## Step 5: Implement

Implement the changes according to the plan. After each meaningful unit of work:

1. **Run light CI** before committing:
   ```bash
   cargo check --workspace
   cargo clippy --workspace --all-targets
   ```

2. If CI fails, attempt auto-fix:
   - `cargo fmt --all` for formatting issues
   - Fix clippy warnings
   - Retry up to 3 times. If still failing, report to the user.

3. **Commit** with conventional commit format per CLAUDE.md: `type(scope): description`

4. **Post progress** as an Issue comment for significant milestones:
   ```
   gh issue comment <number> --body "<progress update>"
   ```

## Step 6: Update the Issue

As implementation proceeds:
- **Spec changes**: If the plan changes during implementation (e.g. a different approach is needed), update the Issue body with `gh issue edit <number> --body-file <file>`
- **Progress logs**: Post Issue comments at key milestones

## Step 7: Invoke create-pull-request

When implementation is complete, invoke `/create-pull-request` to run full CI and create the PR.

## Error recovery

If the session is interrupted, running `/work-on-issue <number>` again will detect the existing branch and PR via Step 2, allowing the user to resume.
