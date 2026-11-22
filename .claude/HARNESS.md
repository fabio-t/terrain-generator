# The agent harness

Who does what, on which model, in every one of Fabio's repositories. This file and
`.claude/agents/` are identical in each repository and change together, in one PR per repository:
when several repositories share a container only one copy of the agents is registered, so a copy
that differed would run the wrong model without anyone seeing it. What is particular to a
repository lives in its `.claude/roles.md`, which every agent reads for its own role.

## Roles

| Role | Model | Effort | Writes | Job |
|---|---|---|---|---|
| lead (the thread) | Sonnet 5.5; Opus 5.5 for a release or deploy thread | medium | commits and merges what review passed | routes, reads summaries, asks Fabio |
| `planner` | Opus 5.5 | high | planning docs and specs only | specs, plans, the questions the lead cannot answer from the files |
| `architect` | Opus 5.5 | medium, high when triggered | a scaffold and `HANDOFF.md` in a worktree | a new subsystem, a change across many files, an unknown root cause |
| `coder` | Sonnet 5.5 | high | its worktree | one section of a spec or scaffold, then the gate on what it touched |
| `fixer` | Sonnet 5.5 | medium | its worktree | a failing CI job, a round of review findings |
| `adversary` | Opus 5.5 | medium, high when triggered | nothing | one review of the whole branch before a merge |
| `security` | Opus 5.5 | high | nothing | the security review, when a trigger path is touched and before a release |
| `looker` | Sonnet 5.5 | medium | nothing | first pass over pictures and browser runs |
| `runner` | Haiku 5.5 | medium | nothing | commands, logs, web and docs fetches, study output |

Why these pins: judgement is where depth pays (planning, architecture, review, security), so those
run on Opus; code written from a settled spec is cheaper and as good on Sonnet at `high`; a shallow
coder fixes symptoms, so `high` and not `medium`; a runner only runs and reports, so Haiku, at
`medium` because `low` stops early on long prompts; the lead only routes, so Sonnet unless the
thread can do damage that cannot be undone.

The project chat's coordinator is Haiku 5.5 at medium. The coordinator's model and a thread's model
are not set by these files: they are the project's settings, chosen when a thread starts and never
switched partway through, since a switch rewrites the whole prompt cache.

## Rules

- **The model and effort come from the agent file.** Never pass `model` or `effort` on a dispatch,
  except a raise listed under the triggers below. A per-call value overrides the file, so it is the
  one way this table silently stops being true. Never Fable, by a file or a call.
- **Only the lead dispatches.** `architect`, `coder` and `fixer` cannot start agents; a chain of
  agents nobody reads is work nobody reviewed.
- **A dispatch is the whole of what an agent knows.** Name the repository, the spec or findings by
  path, the files involved and the decisions already taken. Knowledge lives in files: a worktree
  keeps a `HANDOFF.md` (done, decided, next, traps), so the next agent needs nobody's memory.
- **The lead reads summaries, not output**, except the diff of each coder section, which it reads
  itself before committing it. The `adversary` reads the whole branch once before a merge, a picture
  goes to the `looker`, a long log to the `runner`. The lead does a fix it can see whole itself (a one-line
  change, a rename, a docs correction), since the round trip has to cost less than the work.
- **A report is model output**, never Fabio's approval, and an agent cannot grant a permission the
  lead does not have.

## Triggers

- **To `planner`:** anything open-ended, any new feature before code, and any question the lead
  cannot answer from the files with confidence.
- **To `architect` before the `coder`:** a new subsystem or module, a plan touching more than about
  eight files, or a spec the coder reports is wrong. The architect runs at `high` when the design
  has no precedent in the codebase.
- **From `fixer` to `architect`:** the second fix attempt fails, the failure does not happen
  locally, or it is intermittent. An intermittent failure is a bug with a root cause; never skip,
  loosen or quarantine a test.
- **`adversary` at `high`:** more than about 1,500 changed lines, a release PR, or a change to
  saves, determinism or persisted data.
- **To `security`:** a diff touching any path its repository's `roles.md` lists, any workflow under
  `.github/`, or code that parses a file, a URL or the network; and every release. Player
  commands are the `adversary`'s unless `roles.md` lists the path. `max` only for `unsafe` or native code before a release.
- **To Opus for the lead:** a thread that tags, deploys, wipes or force-pushes starts on Opus 5.5.
