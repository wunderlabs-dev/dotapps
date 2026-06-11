---
name: sharp-edges
description: Use when security-auditing Opnble, reviewing IPC boundaries, VM lifecycle code, credentials, filesystem access, process management, or dangerous defaults - finds footguns, silent failures, and exploitable edge cases.
---

# Sharp Edges

Audit APIs through three adversaries:

- Scoundrel: malicious actor looking for exploitation.
- Lazy developer: takes shortcuts and follows dangerous defaults.
- Confused developer: misunderstands an API that looks safer than it is.

## Surface Scan

Use repo search before judging:

- Tauri IPC: `#[tauri::command]`.
- Public Rust APIs: `pub fn`, `pub async fn`.
- Frontend inputs: forms, handlers, listeners, command calls.
- Network and process edges: `reqwest`, tunnels, URLs, child processes.
- Filesystem edges: `~/.opnble`, clone paths, symlinks, cleanup.

## Opnble-Specific Checks

- IPC params: validate `projectId`, repo URLs, ports, and user-controlled names before side effects.
- VM races: start while stopping, duplicate starts, shutdown during in-flight work, stale sockets and lockfiles.
- Credentials: no token leaks in logs or errors, keychain failures are explicit, OAuth tokens are validated before use.
- Filesystem: prevent path traversal, unsafe symlink following, partial cleanup, and accidental deletion outside `~/.opnble`.
- Processes: track children, handle crashes, avoid zombies, and clean up vfkit, QEMU, gvproxy, cloudflared, and app processes through Makefile-owned paths.

## Language Checks

Rust:

- No `unsafe` in this repo.
- No panics or unwraps on user-controlled data.
- Numeric conversions use `TryFrom` or `From`.
- Shared state is injected, not global mutable state.

TypeScript:

- External data enters as `unknown` and is narrowed.
- No `as Type` for user or IPC data.
- Async Tauri calls are awaited and handled through Result helpers.
- Listener fan-out failures cannot break the whole loop.

## Findings Format

Lead with confirmed risks only:

```markdown
## [SEVERITY] Finding Title

Location: `path`
Category: IPC / VM / Credentials / Filesystem / Process
Adversary: Scoundrel / Lazy Developer / Confused Developer

Description: What can go wrong.
Trigger: How it can happen.
Recommendation: Specific fix.
```

Use `CRITICAL`, `HIGH`, `MEDIUM`, or `LOW`. If evidence is incomplete, label it as an open question instead of a finding.
