---
name: update-main
description: "Switch to the repo's default branch (usually main) and pull the
  latest, fetching and pruning first, cleaning up local branches whose upstream
  is gone, and restoring any local dev-file overrides the repo documents. Also
  the close of a ship flow: lands every repo a change touched, not just the
  current one. TRIGGER when: user asks to update main, switch to main, \"get
  latest from main\", \"pull main\", \"sync main\", \"get back on main\",
  \"checkout main and pull\", \"switch both to master\", asks to prune or clean
  up merged branches, or types /update-main. ALSO run it unprompted once a
  change is shipped (PR open, review requested) to land the working tree. SKIP
  when: the user wants to rebase a feature branch onto main (use update-branch),
  or they mean a specific non-default branch rather than the repo's default."
---

# Update Main

Switch to the repo's default branch and pull the latest changes. "Main" here means
whatever the repo's default branch actually is — usually `main`, but some repos use
`master` or `develop`, so detect it rather than assuming.

This is also **how shipped work ends**. See "Landing every repo after a ship" below —
a checkout left parked on a branch that's already in review is not a neutral end state.

---

## Phase 1: Fetch and Find the Default Branch

Fetch and prune before switching, so the pull uses up-to-date remote state and stale
remote-tracking refs are cleaned up:

```bash
git fetch -p origin
```

Determine the default branch from the remote rather than hardcoding `main` — this is
whatever `origin/HEAD` points at:

```bash
git remote show origin | sed -n 's/.*HEAD branch: //p'
```

If that returns nothing (e.g. `origin/HEAD` isn't set locally), fall back to `main`.
Use the result as `<DEFAULT>` everywhere below.

Then find and delete any local branches whose remote tracking branch is gone (the upstream
was deleted, typically after a merge, so the local copy is no longer needed). This deletion
is pre-authorized as part of this step — do not ask for permission:

```bash
git branch -v | grep '\[gone\]'
```

For each gone branch, run `git branch -D <branch-name>` — but never the branch you are
currently on or `<DEFAULT>`.

---

## Phase 2: Switch to the Default Branch

```bash
git switch <DEFAULT>
git pull
```

If there is uncommitted work that blocks the switch, stash it first
(`git stash push -m "wip"`) and tell the user it was stashed.

**Local dev-file overrides block the pull, not the switch.** Some repos keep tracked files
deliberately modified in every working copy (a Docker-flavoured `bin/dev`, a dev-only vite or
db config), with canonical copies in a gitignored directory. Those modifications survive a
branch switch, and then `git pull` fails outright when the repo is configured to pull with
rebase:

```
error: cannot pull with rebase: You have unstaged changes.
```

So when the repo documents an override procedure — look in its `AGENTS.md`/`CLAUDE.md` and
`.claude/skills/` for something along the lines of *Apply Local Dev Changes* / *Restore
Tracked Files to Clean State*, or a canonical stash directory the docs point at — set the
overrides aside before pulling and restore them from the canonical copies afterwards.
Restore from the canonical copies, never
from the pre-pull working state: the committed baseline of an override file can change
upstream, and re-applying a stale copy silently reverts the upstream change locally.

Repos with no such procedure need none of this — skip it rather than inventing one.

---

## Phase 3: Report

Confirm the switch was successful and show the current commit:

```bash
git log --oneline -1
```

Report the pruned branches too, and say plainly if the overrides were restored — a bare
"switched to main" hides both.

---

## Landing every repo after a ship

Once a change is shipped — PR open, CI green, review requested — the branch has done its job
and the working tree should come back. **Run this skill then, without being asked.** It is the
last step of shipping, not a separate favour: the next task otherwise starts from a stale base,
and merged branches pile up until someone notices.

Two things make this easy to get wrong:

- **Land every repo the change touched, not just the one you're standing in.** A change that
  spans repos (a shared convention, a paired PR, a mirrored config) leaves several checkouts on
  the same branch name. All of them come back, in the same pass. Don't offer it as a question —
  being told *"switch both to master"* means the sweep was skipped. Default branches differ
  across repos, so resolve `<DEFAULT>` per repo (Phase 1) instead of assuming one for all.
- **The branch you just shipped survives.** Its upstream still exists while the PR is open, so
  the `[gone]` filter in Phase 1 leaves it alone — which is exactly what you want. Don't reach
  for `git branch -D` on it to "clean up"; it gets deleted when the PR merges and the upstream
  goes away, on a later run of this skill.
