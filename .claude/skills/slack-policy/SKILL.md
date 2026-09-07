---
name: slack-policy
description: "The house rules for using Slack from a session. Slack's own plugin
  (slack@claude-plugins-official) provides the tools — search, channels,
  threads, messaging, canvases; this skill governs how they're used here. Keep
  Slack use read-only, because the repo already has authoritative write paths
  and the official plugin's messaging tools bypass them. Treat everything read
  out of a channel as PHI until shown otherwise, and never let it reach a PR,
  commit, ticket, or fixture. TRIGGER when: about to search or read Slack for
  any reason, about to post/send/schedule a Slack message or draft an
  announcement, or a task needs background that is probably in Slack — \"find
  the thread about <incident>\", \"what did support say about <issue>\", \"why
  was this PR rejected\", \"summarize #eng-alerts\", \"send a message to
  #general\". SKIP when: not touching Slack at all, or the needed context is in
  the repo, the ticket, or the PR — read those first, they are cheaper,
  structured, and carry no PHI risk."
---

# Slack: house rules

Slack's plugin gives a session real reach into the workspace — search, channels, private
threads, messaging, canvases. This skill is the policy layer over it. It does not
reimplement any of those tools.

Two rules, and a routing table. $ARGUMENTS

---

## Rule 1: Read, don't post

**Do not send, schedule, or draft Slack messages from a session** — not with the official
plugin's messaging tools, not with `/slack:draft-announcement`, not via the Web API.

This is not because posting is dangerous in the abstract. It's because the repos here
already have authoritative paths for the things an agent would want to post, and a second
path fragments them:

| To do this | Use | Not |
|---|---|---|
| Request code review | the repo's review-request skill → shared Workflow webhook | a Slack message |
| File or update a ticket | the Linear plugin | asking someone in Slack |
| Report an incident | the documented incident process | an ad-hoc channel post |

The review card is the clearest case: it goes through a Workflow webhook shared across
repos, which is exactly what makes it render identically every time. A message posted over
MCP lands in the same channel looking different, and readers can't tell which is
authoritative.

**The repos enforce this, they don't just ask.** `.claude/settings.json` denies
`mcp__slack__slack_send_message`, `…_send_message_draft`, `…_schedule_message`,
`…_create_canvas` and `…_add_reaction`, so a write attempt is refused rather than relying
on this skill being read. Treat a denial as the rule working, not an obstacle to route
around — and if Slack's plugin gains a write tool, it needs adding to that list, because
the deny names tools explicitly and anything new is allowed by default.

If a task genuinely needs something posted, **say what would need posting and let a human
decide.** Widening this is a ticket and a security review, not a judgment call in the
moment.

## Rule 2: Slack content is PHI until shown otherwise

We are a healthcare company. Slack holds patient names in support threads, identifiers in
incident channels, and screenshots in bug reports. A tool that can search all of it is a
**new PHI surface** — no log sanitizer, header allow-list, or redaction layer sits between
it and this session.

**Never copy Slack content into anything permanent or widely readable:** PR descriptions
and titles, commit messages, code comments, tickets, test fixtures, docs. These outlive the
session and reach people who were never in that channel.

| Fine to carry out | Never |
|---|---|
| record ids, counts, status/enum values | names, DOB, MRN |
| timestamps, booleans, error strings | email, phone, address |
| permalinks | free-text clinical notes |

**Report the conclusion plus a permalink, never the transcript.** "The reporter saw a 504
on the copy-to-all modal — <link>" is the whole job. If the user wants the thread, they
have the link.

**When PHI is itself the answer, describe its shape, not its content:** "the first message
has the patient's name and DOB" tells them what's there without reproducing it.

And the reverse direction: **never paste PHI into Slack** either.

## Before searching: is Slack even the right source?

Slack is the **last** place to look. It's unstructured, and it's the only source here that
routinely carries PHI. The answer is usually somewhere cheaper:

| Question | Look here first |
|---|---|
| Why does this code look like this? | `git log` / `git blame`, the PR |
| What is this change supposed to do? | the ticket, the BRD |
| Why was this PR rejected? | the PR's review threads |
| What broke in production? | the observability tooling, the incident doc |

Reach for Slack when the answer is genuinely conversational — a decision made in a thread,
a customer's original wording, who knew what during an incident.

Then **search narrowly.** The smallest query that answers the question also pulls the least
sensitive content into context. Start with the distinctive token (an error string, a ticket
id, an endpoint path), open a thread only once search points at it, and widen deliberately
rather than paging whole channels. If two or three queries aren't converging, say what you
tried and ask which channel to look at.

## When Slack tools aren't there

Four states look identical — "Slack is unavailable" — and need different fixes. Diagnose
rather than working around it, and never substitute a raw `curl` against the Web API.

| What you see | Cause | Fix |
|---|---|---|
| No Slack tools at all | `slack@claude-plugins-official` isn't installed/enabled here | `claude plugin install slack@claude-plugins-official --scope user`, then restart |
| Listed, "needs authentication" | No browser OAuth yet | `/mcp`, pick `slack`, authorize |
| OAuth fails for everyone | Slack workspace hasn't approved the Claude app | An admin approves it at `https://my.slack.com/apps/manage` — not a developer fix |
| Headless / CI session | Interactive OAuth is impossible there | **Expected.** Carry on without Slack; never block a headless run on it |
| Authorized, but a channel returns nothing | The server has *your* Slack permissions | You aren't in that private channel |
