# Cursor Cloud Agent Automations for Opnble

These two automations are configured in the user's Cursor account, not in this repo. This file is the human-readable source of truth: the exact workflow proto shape lives in Cursor's UI.

To create or update them, open Cursor Settings → Automations and use the values below. The agent can `list_automations` / `get_automation` / `update_automation` via the `cursor-backend-control` MCP once they exist.

## 1. PR sentinel

**Goal**: when a PR is opened against this repo, run an automatic review pass against `.cursor/rules/*.mdc` and `.cursor/BUGBOT.md` and post the verdict as a PR comment. Complements Bugbot: this one runs even before Bugbot fires and focuses on the rules our humans actually care about.

**Trigger**: `git prOpened` (repo: `vtemian/opnble`, base branch: `main`)

**Prompt** (paste verbatim into the automation):

> Review the diff of this PR against `.cursor/rules/opnble-core.mdc`, `.cursor/rules/opnble-makefile.mdc`, `.cursor/rules/opnble-verification.mdc`, `.cursor/rules/opnble-canvas.mdc`, `.cursor/BUGBOT.md`, and any nested `BUGBOT.md` files in the changed subtrees.
>
> Return a single PR comment with two sections:
>
> **Rule violations**: bullet list, one per finding, with file path, line range, the rule it violates, and a one-line fix.
> **Verification gate**: which gate from `opnble-verification.mdc` this PR needs (`pnpm check`, `make lint-rust && make test`, or `make check && make test-all`), and whether the PR description mentions running it.
>
> If there are no violations and the verification gate is acknowledged, say "Looks clean" and nothing else. Be terse. No em-dashes.

**Action**: post comment on the PR (Cursor's `git PR` action type).

**Model**: use the default unless review quality drops.

## 2. Weekly rules-compliance audit

**Goal**: catch drift between the codebase and `.cursor/rules/*.mdc` that PR-time reviews miss. Mirror the manual `rules-compliance-auditor` pattern from past sessions, but on a schedule.

**Trigger**: `schedule` (cron: every Monday at 09:00 UTC). The repo target is `vtemian/opnble` on the `main` branch.

**Prompt** (paste verbatim into the automation):

> Run the `rules-compliance-auditor` skill against the current `main` branch of `vtemian/opnble`. Spawn parallel sub-audits for:
>
> - Makefile contract (`.cursor/rules/opnble-makefile.mdc`): any raw `pnpm dev` / `cargo tauri dev` / `pkill -f tauri` / direct vfkit calls slipped past the hook
> - Verification gate (`.cursor/rules/opnble-verification.mdc`): commits in the last 7 days whose stack the gate would have caught
> - Core engineering (`.cursor/rules/opnble-core.mdc`): em-dashes in any file, `*Manager` types, copy-pasted `map_err` closures
> - Rust backend (`.cursor/rules/rust-backend.mdc`): `.unwrap()` outside test code, `tokio::spawn` without a tracked handle, `MutexGuard` held across `.await`
> - Frontend (`.cursor/rules/web-frontend.mdc`): `any` types, `*Ids: Set<>` patterns, inline `style={{}}` for layout/color, components calling Tauri commands directly
>
> Open a GitHub issue titled `Weekly rules-compliance audit YYYY-MM-DD` with the merged findings. Group by rule, link each finding to file:line. If there are no findings, do not open the issue.

**Action**: open GitHub issue (Cursor's `git issue` action type if available, else `git PR` opening a docs-only PR).

**Model**: use the default.

## How to wire these up

1. In Cursor: Settings → Automations → New Automation.
2. Pick the trigger (PR opened or schedule).
3. Paste the prompt verbatim.
4. Pick the action (PR comment or new issue).
5. Save.
6. After creation, run the `cursor-backend-control` MCP tool `list_automations` to confirm; record the new `automationId` here once the workflow shape stabilizes.

## How an agent can manage them later

- `list_automations` returns metadata only (redacted bodies).
- `get_automation` returns the workflow shape redacted, so it is useful for sanity-checking enabled/disabled state but not for diffing prompts.
- `update_automation` accepts a `workflow` proto. Round-trip changes through the Cursor UI for now; the workflow proto schema is not documented externally as of this writing.
