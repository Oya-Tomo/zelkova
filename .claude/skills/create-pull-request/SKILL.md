---
name: create-pull-request
description: Create a pull request with enforced full CI. Auto-generates PR title and body from the linked Issue and changes, runs all CI checks, auto-fixes failures, and creates the PR via gh.
---

Create a pull request for the current branch. Follow every step below in order.

**Important**: Unless the user explicitly specifies otherwise, always target `develop` as the base branch. Never target `main`.

## Step 1: Run full CI

Execute all four CI checks sequentially. These are mandatory — do not skip.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace --exclude zelkova-gui
cargo check --workspace
```

### On failure

If any check fails:

1. Read the error output carefully.
2. Attempt auto-fix:
   - **fmt**: Run `cargo fmt --all`
   - **clippy**: Fix lint warnings. If `cargo clippy --fix` is used, always run `cargo fmt --all` afterward (clippy --fix breaks formatting).
   - **test**: Fix failing tests.
   - **check**: Fix compilation errors.
3. Re-run the full CI suite.
4. Retry up to 3 times. If still failing after 3 attempts, report all errors to the user and stop.

## Step 2: Gather context

Collect information for the PR:

1. **Current branch**: `git branch --show-current`
2. **Linked Issue**: Extract the issue number from the branch name (pattern: `<prefix>/<number>-<description>`) or ask the user.
3. **Issue content**: `gh issue view <number> --json title,body --jq '.body'`
4. **Changes**: `git diff develop...HEAD` (or the appropriate base branch)
5. **Commits**: `git log develop..HEAD --oneline`
6. **Recent PR style**: `gh pr list --state merged --limit 5 --json title,body` to match existing conventions.

## Step 3: Generate PR title and body

### Title

Derive from the Issue title and CLAUDE.md conventions. Format: `type(scope): description`. Keep under 70 characters.

### Body

Use the following template:

```
## Summary

<2-3 bullet points summarizing what this PR does, based on the Issue and changes>

Closes #<issue-number>

## Test plan

<checklist of how to verify the changes — based on acceptance criteria from the Issue>
```

## Step 4: Create the PR

Push the branch and create the PR:

```bash
git push -u origin <branch-name>
gh pr create --base develop --title "<title>" --body "<body>"
```

## Step 5: Monitor CI

After creation, launch a monitor to watch CI checks until all pass or any fails:

```bash
gh pr checks <pr-number> --watch
```

Report the PR URL to the user. If any check fails, attempt auto-fix (see Step 1 "On failure"), push the fix, and re-monitor.
