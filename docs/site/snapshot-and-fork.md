# Snapshot and fork

Status: v1.1 feature, shipped behind the operator MCP tool surface.
Audience: developers in the SF validation cohort who want to know exactly what these features do, where they help, and where they stop helping.

Opnble v1.1 adds two operator capabilities for the in-IDE agent loop: instant snapshots of a running project, and forks that branch off a parallel copy of the same project. Both are implemented on top of APFS `clonefile(2)` on macOS, with a recursive copy fallback on other filesystems. This page documents what they capture, what they do not capture, and what kind of workflows they are good for.

## What snapshot captures

`snapshot_project` clones the on-disk project tree at `~/.opnble/repos/<project-id>/` to a snapshot directory. On macOS APFS this is one `clonefile(2)` syscall: byte-for-byte copy-on-write, instant regardless of tree size. On non-APFS filesystems we fall back to a recursive directory copy that preserves symlinks via `std::os::unix::fs::symlink` (this matters for pnpm-style `node_modules`).

The clone includes everything on disk under the repo root:

- Source files, configs, package manifests.
- `node_modules/` in full, including hoisted layouts and pnpm symlink farms.
- `.next/`, `dist/`, `build/`, and any other build artifacts.
- `.git/` with full history, refs, and any in-progress index state.
- Hidden files, dotfiles, and lockfiles.

The dev server is not paused while the snapshot runs. Snapshots are byte-on-disk captures at clonefile time; per file they are atomic, so the directory is consistent even mid-write, but a snapshot does not reflect anything the dev server has only in memory.

## What snapshot does NOT capture

This is the honest list. None of these matter for typical Node/React/Next projects, which is the v1.1 target. They start mattering when you add a database or modify the container OS.

- **Running dev-server process state.** PID, open sockets, in-flight HTTP responses, and the JS heap are not in the snapshot. Rolling back restarts the dev server from a cold start.
- **Container-internal mutations outside the bind mount.** Example: `podman exec opnble-proj-X apk add foo` modifies layers inside the container, not in `~/.opnble/repos/<id>/`. That change is gone the next time the container restarts anyway, and it is not in the snapshot either way.
- **Data directories at non-bind-mounted paths inside containers.** Concrete examples: Postgres `/var/lib/postgresql`, Redis `/data`. If you run a database alongside your Node app and the database lives inside its own container, snapshots do not roll the database back. The Node tree on disk rolls back; the database keeps moving forward.

For a single-container Node app with hot reload, none of the above limitations is real. For a multi-container compose-style setup with a stateful sibling, treat the snapshot as covering the Node side only.

## Rollback semantics

`rollback_project` brings the project back to a prior snapshot in four steps:

1. Force-stop and remove the project container.
2. Replace the project tree from the snapshot (`clonefile` back, or recursive copy fallback).
3. Restart the dev server on the same port.
4. Return `interruption_ms`, the wall-clock time the dev server was unavailable.

Typical interruption windows we have measured: 5 to 15 seconds, dominated by container startup, not the file operation. The clone itself is sub-second on APFS even for multi-gigabyte `node_modules` trees.

A rollback against a snapshot you took 10 minutes ago discards everything between then and now, including any commits made in the working tree. There is no partial rollback in v1.1; you get the whole tree as it was, or nothing.

## Fork semantics

A fork is a new first-class Opnble project, not a branch or a worktree. `fork_project` produces a child that:

- Has its own `project_id`, allocated port, container, and slug.
- Has slug `<parent-slug>-<short_id>`, where `short_id` is a 6-character suffix derived from the new project id. Collision-free by construction since project ids are timestamped.
- Carries `parent_project_id` and `forked_at` on the child Project, persisted in `state.json`.
- Inherits `installed = true` from the parent so the cloned `node_modules` is reused. There is no second `npm install`, which is the whole point.
- Boots in its own container and is returned only once status flips to `Ready`.

A single `fork_project` call accepts up to 4 children. To produce more, call the tool again. Children are created sequentially.

To remove a fork, call `mcp_discard_fork(forkId)` or use the sidebar UI. That force-stops the container, removes the project from `state.json`, and deletes `~/.opnble/repos/<child-id>/`. The parent is untouched.

## What forking does NOT do

- **Forks do not share tunnels or GitHub Pages publication state with the parent.** Each fork is independent from the orchestrator's perspective. If you want a preview link on a fork, call `create_preview_link` on the fork's slug.
- **Forks do not share a process or filesystem with the parent at runtime.** After the clone, edits in one are invisible to the other. They share storage blocks on APFS via copy-on-write until either side writes, which is a disk-space win, not a coordination mechanism.
- **Forking is rejected if the parent is not `Ready`.** The orchestrator returns `PROJECT_NOT_RUNNING` if the parent is mid-install, stopped, or in an error state. Required so the child starts from a known-good baseline.

The pnpm symlink case is worth calling out: on APFS the clone preserves symlinks natively, and on the fallback path we re-create them with `std::os::unix::fs::symlink`. Either way, a pnpm `node_modules` layout in the parent works in the child without re-installing.

## When the demo loop works well

| Workflow | How well it works | Why |
|---|---|---|
| Node, React, Next, Vite dev with hot reload | Works well | Single container, file-on-disk state, fast restart. |
| Refactor experiments where you want to A/B two approaches | Works well | Fork twice, edit each independently, compare in two browser tabs. |
| Full-stack with a local database in a sibling container | Works partially | The Node tree rolls back. The database does not. |
| Python or Go projects | Does not work yet | The bundled container image is Node-only in v1.1. |
| Anything that mutates `/etc/`, system packages, or the container OS | Does not work | Those changes live in the container layer, not in the bind mount. |

The canonical loop the v1.1 design targets: refactor, broke something, snapshot the broken state for postmortem, rollback to the last known-good snapshot, try a different approach. Snapshot turns "git stash plus npm install plus pray" into an idempotent operation that takes seconds.

## Tool reference

Six MCP tools cover the surface. Full schemas, error codes, and annotations live in `docs/plans/2026-05-19-v1.1-snapshot-fork-operator-tools-design.md`.

| Tool | Purpose |
|---|---|
| `snapshot_project({ slug, label? })` | Clone the on-disk project tree. Returns snapshot id, label, size, created-at. |
| `list_snapshots({ slug })` | Enumerate snapshots for a project, newest first. |
| `rollback_project({ slug, snapshot_id })` | Stop, restore tree, restart, report `interruption_ms`. |
| `delete_snapshot({ slug, snapshot_id })` | Remove a snapshot directory. Does not touch the live tree. |
| `fork_project({ slug, count?, label? })` | Create up to 4 child projects from the parent's current tree. Children reach `Ready` before the call returns. |
| `discard_fork({ fork_id })` | Stop and delete a fork. Parent is untouched. |

All tools follow the rest of the MCP surface: `slug` resolves locally, errors come back as JSON-RPC envelopes with structured `data: { code, hint? }`, destructive tools are marked as such in their annotations.

## Known gaps for v1.1

- Snapshots are not garbage-collected automatically. Delete them when you are done.
- There is no snapshot diff view. To compare two snapshots, diff the trees directly.
- Fork count is capped at 4 per call. Call again for more.
- Multi-container database state is out of scope.

These are tracked for v1.2.
