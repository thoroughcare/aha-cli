---
name: update-branch
description: "Get onto the right branch for a piece of work and bring it up to
  date: find the existing branch for a ticket (or create it off the default
  branch using the repo's own naming convention when none exists yet), then
  rebase onto the base branch, resolving any merge conflicts intelligently.
  Optional \"manual rebase\" flag for interactive mode. TRIGGER when: user asks
  to rebase a branch, says \"the branch is behind main\", \"update this
  branch\", \"switch to the branch for <TICKET>\", \"start work on <TICKET>\",
  or types /git-workflow:update-branch [PR# or ticket]. SKIP when: the branch
  exists and is already up to date with its base (the skill detects this and
  stops), or the user wants to get back on the default branch itself (use
  update-main)."
---

# Update Branch

Get onto the branch for a piece of work and bring it up to date — finding or creating it as needed, then rebasing onto its base and resolving any conflicts. Argument: ticket ID, PR number, PR URL, or branch name. Optional flag: `manual rebase` to let the user drive the rebase interactively.

$ARGUMENTS

If no argument is provided, use the current branch.

---

## Phase 1: Identify the Branch

### 1a. Parse the argument
- **PR URL** (e.g., `https://github.com/<owner>/<repo>/pull/123`): extract PR number, then `gh pr view <NUMBER> --json headRefName,baseRefName`
- **PR number** (e.g., `123`): `gh pr view <NUMBER> --json headRefName,baseRefName`
- **Ticket ID** (e.g., `OW-815`): `gh pr list --search "OW-815" --state open --json headRefName,baseRefName,number`
- **Branch name**: use directly. Base branch is the PR's `baseRefName` when known, otherwise the repo's default branch.
- **No argument**: use current branch via `git branch --show-current`

Save both the **branch name** and the **base branch**.

Resolve the default branch from the remote rather than assuming `main` — sibling repos
often differ (`main` in one, `master` in another):

```bash
git remote show origin | sed -n 's/.*HEAD branch: //p'
```

### 1b. Check for `manual rebase` flag
If the arguments contain `manual rebase` (case-insensitive), set manual mode. Strip it from the branch/ticket parsing.

### 1c. For a ticket with no open PR, look for the branch by ticket ID
A ticket may already have a branch that has never been pushed or has no PR yet, so search
both the remote and local refs before concluding there isn't one — case-insensitively,
since branch conventions vary in casing:

```bash
git branch -r | grep -i "<TICKET-ID>"
git branch | grep -i "<TICKET-ID>"
```

If exactly one matches, that's the branch — continue to Phase 2. If several match, show
them and ask which to use rather than guessing. If none match, take Phase 1d.

### 1d. If no branch exists yet — create it off the default branch

**Use the repo's own branch-naming convention.** Read it from `AGENTS.md` / `CLAUDE.md`
(commonly a `${type}/${TICKET_ID}-${short-description}` shape, e.g.
`bugfix/OW-815-fix-timeout`), and match the casing and type vocabulary the repo documents.

**Don't use a tracker's auto-suggested branch name** (Linear's
`username/lowercase-ticket-…` form and its equivalents). Shared CI tooling frequently
parses the ticket ID out of the branch name in the repo's documented shape, and the
tracker default breaks that parsing.

Create it off a freshly-pulled default branch:

```bash
git switch <DEFAULT>
git pull
git switch -c <type>/<TICKET-ID>-<short-description>
```

A branch created this way is already current, so the rebase in Phase 3 is a no-op — Phase
3a detects that and you can go straight to reporting.

**If the user is already on a branch for this work, use it — don't create a second one.**
Confirm the current branch rather than assuming the ticket's suggested name; some
conventions deliberately deviate (a shared QE branch, a stacked PR's base), and renaming
or forking off a new branch splits the work across two.

---

## Phase 2: Switch to the Branch

If there is uncommitted work that would block the switch, stash it first and say so:

```bash
git stash push -m "wip"
```

Fetch and prune first, then switch:

```bash
git fetch -p origin
```

Clean up local branches whose remote tracking branch is gone — the upstream was deleted (typically after a merge), so the local copy is stale. This is pre-authorized; you don't need to ask. Be aware that `git branch -D` force-deletes even unmerged branches, so target only the `[gone]` ones and never the current branch or the default branch:

```bash
git branch -v | grep '\[gone\]'   # then: git branch -D <branch> for each gone branch
```

Then switch to the target branch:

```bash
git switch <BRANCH-NAME>
```

If the branch only exists on the remote, use `git switch --track origin/<BRANCH-NAME>`.

If the repo keeps tracked files deliberately modified in every working copy (local dev-file
overrides), set them aside before the switch and restore them from their canonical copies once
the rebase is done — see "Local dev-file overrides" in `update-main`'s Phase 2 for how to spot
the procedure and why restoring from the canonical copy matters. Never commit them; they show
as modified afterwards and that's expected.

---

## Phase 3: Update Base Branch and Rebase

**Rebase, not merge — including when the branch is shared.** Rebasing keeps history linear
and surfaces conflicts loudly at the point of update. An open PR, landed review comments, a
deployed review app, another dev's checkout, or a shared QE branch are none of them a reason
to reach for `git merge` to bring a branch up to date; they're a reason to tell the people
sharing it that you force-pushed. Where a repo documents a merge-based update flow instead,
follow the repo.

### 3a. Check if rebase is actually needed
```bash
git merge-base --is-ancestor origin/<BASE-BRANCH> HEAD
```
If the exit code is 0, the base is already an ancestor of HEAD — the branch is up to date. Tell the user there's nothing to rebase and stop.

### 3b: Manual rebase mode
If `manual rebase` flag is set:
1. Tell the user to start the rebase themselves:
   ```
   git rebase origin/<BASE-BRANCH>
   ```
2. Use `AskUserQuestion` with a selectable option (e.g., "I've resolved conflicts, continue") to wait
3. When the user signals they're done with a conflict:
   - Check `git status` for remaining conflicts
   - If conflicts remain, show which files still need resolution and wait again
   - If no conflicts, run `git rebase --continue`
   - Repeat until the rebase completes
4. Skip to Phase 5

### 3c: Automatic rebase mode (default)
```bash
git rebase origin/<BASE-BRANCH>
```
If the rebase succeeds with no conflicts, skip to Phase 5.

---

## Phase 4: Resolve Conflicts (automatic mode, only if conflicts exist)

If the rebase in 3c completed cleanly with no conflicts, skip this entire phase.

### 4a. Identify conflicted files
```bash
git diff --name-only --diff-filter=U
```

### 4b. For each conflicted file
1. Read the file to see the conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`)
2. Read the **original branch version** of the file to understand the intended changes:
   ```bash
   git show REBASE_HEAD:<FILE-PATH>
   ```
3. Read the **base branch version** to understand what's being rebased onto:
   ```bash
   git show origin/<BASE-BRANCH>:<FILE-PATH>
   ```
4. Resolve the conflict by merging intelligently:
   - Preserve every intended change from the original branch — carrying your work forward is the whole point of the rebase, so don't drop any of it
   - Integrate them cleanly with the base branch updates
   - If both sides changed the same lines, combine them logically (don't drop either side)
   - If the base branch renamed/moved something, adapt the original changes to match
5. Edit the file to remove all conflict markers with the resolved content
6. Stage the resolved file: `git add <FILE-PATH>`

### 4c. Continue the rebase
```bash
git rebase --continue
```
If more conflicts arise (multi-commit rebase), repeat 4a–4c.

### 4d. If resolution is unclear
If a conflict is ambiguous and you can't confidently determine the right merge:
- Show both sides to the user
- Explain what each side is doing
- Ask the user which approach to take before resolving

---

## Phase 5: Verify

### 5a. Verify the rebase succeeded
```bash
git log --oneline -5
git status
```
Confirm no lingering conflict state.

### 5b. Verify no changes were lost
```bash
git diff origin/<BASE-BRANCH>...HEAD --stat
```
Confirm the branch still contains all the intended changes from the original work.

### 5c. Restore local dev-file overrides
If the repo has an override procedure (Phase 2), restore the overrides from their canonical
copies now, before reporting.

### 5d. Report to user
- Summarize what was rebased
- If there were conflicts, report how many were resolved and highlight any ambiguous resolutions
- If it was a clean rebase, just confirm success
- Say whether dev overrides were restored, if the repo has them
- Remind the user to force-push when ready: `git push --force-with-lease`
