---
name: bootstrap
description: "Turn a recurring frustration into a durable fix by writing a rule,
  reminder, new skill, or config change into the right place — CLAUDE.md,
  AGENTS.md, an existing or new skill, or user memory — so the bad behavior
  stops recurring. Applies the fix directly — invoking it is the authorization —
  then reports what changed and why that location; confirms first only when the
  change would rewrite existing guidance, contradict a written rule, land
  outside the repo, or alter executable behavior. TRIGGER when: user expresses
  frustration about repeated bad behavior (\"you keep doing X\", \"every time
  you...\", \"this keeps happening\", \"stop doing X\"), asks to bootstrap a
  fix, \"make sure Claude doesn't do X again\", \"remember to...\", \"add a rule
  so...\", \"make this a skill/convention\", or \"put this in
  CLAUDE.md/AGENTS.md\". SKIP when: user is reporting a one-off mistake they've
  already moved past, or asking about a coding bug rather than Claude's
  behavior."
---

# Bootstrap

Prevent a recurring frustration from happening again by adding a prompt, command modification, or other Claude configuration fix. The argument describes what went wrong or what keeps happening.

$ARGUMENTS

If no argument is provided, ask the user what keeps frustrating them.

---

## Phase 1: Understand the Problem

### 1a. Analyze the frustration
Parse the argument to understand:
- **What happened** — the specific bad behavior or recurring issue
- **Why it's frustrating** — is it wasted time, broken code, wrong approach, etc.?
- **When it occurs** — during what kind of task or workflow does this happen?

### 1b. Ask for clarification if needed
If the description is vague, ask the user to give a concrete example of the last time this happened.

---

## Phase 2: Determine the Fix

**Default to project-portable locations** (`CLAUDE.md`, `AGENTS.md`, `.claude/skills/`) so other team members and Claude instances on this codebase get the same fix. Only use user-level files (the user's `~/.claude/CLAUDE.md`, memory) when the user explicitly directs you to, or when the rule is genuinely user-specific (personal env quirks, individual workflow preferences nobody else needs).

**Sort the fix by what it is, not by how many places need it.** A **rule** — something always true, that a reader has to know to act correctly — belongs in `AGENTS.md`/`CLAUDE.md`, which are loaded every session. A **procedure** — an ordered sequence you follow when doing a particular job — belongs in a skill, which is loaded when that job comes up. "Several skills need this" is not a third category: state a rule once in the agent docs and let the skills rely on it, rather than creating a middle-tier reference doc that every skill has to point at. That middle tier is how the same rule ends up summarized in the agent docs *and* stated in full somewhere else, with a reader who can't tell which is authoritative.

Evaluate which mechanism is the best fit to prevent recurrence:

### Option A: Add to an existing skill
If the issue happens during a specific workflow that already has a skill, add a rule, reminder, or step to that skill's `SKILL.md`. If the workflow is covered by a **shared marketplace plugin** rather than a repo-local skill, fix it there instead — one edit reaches every repo, and a repo-local copy of a plugin-owned rule drifts. Check the installed plugins before adding to a local skill.

### Option B: Add to CLAUDE.md
If the issue is a broad behavioral rule that should apply in all conversations (not just specific skills), add it to the project-level `CLAUDE.md` (which often `@`-imports `AGENTS.md`). (User-level `~/.claude/CLAUDE.md` only when the user explicitly directs you to — see the Phase 2 preamble.)

### Option C: Add to AGENTS.md
If the issue is a coding convention, pattern, gotcha, or working agreement that affects how code is written or how work is shipped (not just one skill's steps), add it to the appropriate section of `AGENTS.md`. This is also the home for a rule that **several skills depend on** — state it here once in full rather than in a doc the skills reference.

### Option D: Create a new skill
If the issue describes a workflow that should be automated and no existing skill covers it, create a new skill in `.claude/skills/<name>/SKILL.md`. Include YAML frontmatter with a `description` (with TRIGGER/SKIP language) so Claude can auto-invoke it when relevant. If the workflow isn't specific to this repo, propose it as a **marketplace plugin skill** instead.

### Option E: Add to memory
Only if the issue is **specific to this particular user** and not applicable to the wider team or project (e.g., personal workflow preferences, local environment quirks). Memory isn't committed to the repo, so if the fix would benefit other developers or agents working on this codebase, use one of the other options instead (CLAUDE.md, AGENTS.md, or a skill).

---

## Phase 3: Apply it, and show your work

**Invoking this skill is the authorization — don't ask "want me to apply this?".** The user
came here to get a fix written; a yes/no round trip to confirm a reversible text edit spends a
turn and re-asks a question they already answered. Additive guidance in `CLAUDE.md`,
`AGENTS.md`, a skill, or memory is cheap, version-controlled, and trivially revertable — so
**make the edit** (Phase 4), then show what changed:

- **What you changed** — which file(s), and the exact text added
- **Why that location** — why this mechanism catches the issue
- **What you decided** — if two locations were plausible, name the runner-up and why you
  passed on it, so the user can redirect in one message instead of two

State any assumption you had to make rather than stopping to ask about it: pick the reading
that matches the user's words most literally, say which you picked, and invite a correction.

### Confirm first only when the fix is genuinely not reversible-by-text

Pause **before** editing only when the change would:

- **Rewrite or delete existing guidance**, rather than add to it — reversing someone else's
  documented decision needs their input, especially if git blame shows it was deliberate.
- **Contradict a rule already written down elsewhere.** Surface the conflict and let the user
  settle it; don't silently pick a winner. (Recording the conflict in the file as unreconciled
  is a fine additive move in the meantime.)
- **Land outside this repo** — a shared plugin, another repo, or anything published. Those
  follow that destination's own review flow.
- **Touch executable behavior** rather than instructions — a hook, CI config, or a script that
  runs without a human in the loop.

Frustration expressed twice about the same thing is a strong signal the user wants the fix
applied, not re-litigated.

---

## Phase 4: Apply the Fix

### 4a. Make the change
Edit the chosen file(s). Match the surrounding document's voice and formatting — a rule that
reads like the file it lives in gets followed; one that reads like a bolted-on note gets
skimmed past.

### 4b. Verify
- If editing a skill: read it back to confirm it makes sense in context
- If editing CLAUDE.md/AGENTS.md: ensure the new rule doesn't conflict with existing rules
- If creating a memory: ensure any memory index the repo maintains is updated

### 4c. Report back
Tell the user what was changed and how it will prevent the issue going forward.
