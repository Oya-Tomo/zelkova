---
name: create-issue
description: Create a GitHub Issue with spec-level detail. Includes duplicate check, related issue linking, sub-issue creation, and a grill-me phase to deeply resolve requirements before creation.
---

Create a GitHub Issue for this project. Follow every step below in order.

## Step 1: Gather initial context

If the user provided arguments (e.g. `/create-issue add math preview`), use that text as the initial requirement summary. If no arguments were given, ask the user to describe what they want to build or change.

## Step 2: Check for duplicate issues

Run `gh issue list --state open` and scan the results for duplicates. If a similar issue already exists, report it to the user with the issue number and title, then ask whether to proceed or stop.

## Step 3: Identify related issues

List open issues and identify potentially related ones. Present them to the user and ask which ones to link. Selected issues will be listed under `### Related Issues` in the template.

## Step 4: Grill-me phase

Interview the user relentlessly about the requirement until reaching shared understanding. Resolve every ambiguity — scope, behavior, edge cases, acceptance criteria. Provide your recommended answer for each question. Ask one question at a time.

Rules:
- Do NOT ask questions that can be answered by reading the Issue, code, or documentation. Investigate first.
- Go deep enough that reading the Issue alone is sufficient to start implementation.
- Cover: what to build, how it should behave, which crates/files are affected, the approach, and clear acceptance criteria.

## Step 5: Determine sub-issues

Ask the user if this issue should have sub-issues. If yes, create them after the parent issue is created.

## Step 6: Determine metadata

Auto-infer from the content:
- **Labels**: based on scope (crate name, type of work)
- **Branch prefix**: `feat`, `fix`, `refactor`, `docs`, `chore` (based on the nature of the change)

Present the inferred metadata to the user for confirmation before creating the issue.

## Step 7: Create the issue

Use `gh issue create` with the following fixed template. All naming conventions (title format, etc.) follow CLAUDE.md.

```
## Description

<concise summary of what and why>

## Spec (Confirmed)

### Scope (crates/files)

<list of crates and files that will be affected>

### Approach

<detailed implementation approach — deep enough that a developer can start coding without ambiguity>

### Acceptance Criteria

- [ ] <measurable, testable criterion>
- [ ] <measurable, testable criterion>

### Related Issues

<List of related issue links, or "None.">
```

The title must follow the project convention: `type(scope): description` (e.g. `feat(gui): add math preview`).

## Step 8: Create sub-issues (if applicable)

If sub-issues were agreed upon in Step 5, create each sub-issue first, then link them to the parent.

### GitHub Sub-Issues API

The `sub_issue_id` parameter requires the **database ID** (integer), NOT the issue number.

1. Get database ID:
   ```bash
   gh api repos/Oya-Tomo/zelkova/issues/<issue_number> --jq '.id'
   ```

2. Add sub-issue to parent:
   ```bash
   gh api --method POST repos/Oya-Tomo/zelkova/issues/<parent_number>/sub_issues \
     --input - <<< '{"sub_issue_id": <database_id>}'
   ```

3. Verify:
   ```bash
   gh api repos/Oya-Tomo/zelkova/issues/<parent_number>/sub_issues --jq '.[] | "#\(.number) \(.title)"'
   ```

Pitfalls:
- `sub_issue_id` as string → 422 validation error
- `gh issue add-sub-issues` does not exist in current gh CLI (v2.92)
- GET `/sub_issues` returns `[]` when none linked yet — not a 404
