# vibox — design (hackathon, 3-hour build)

## Problem

A small company needs specific, locally-hosted software operated by a non-technical person.
Developers can vibe-code such apps in minutes, but there is no sane way to deliver, run,
update, and persist them on the operator's machine.

## Product

"Dropbox for vibe-coded apps", Steam-shaped. Three components:

```
cli-pack ── publish/update ──> API (registry, users) ── install/update ──> launcher
                                                                     (steam-like, startup run)
```

1. **vibox-cli** (Rust, workspace crate): `pack` a Dockerfile project into a `.vibox`
   artifact; `publish` it to the registry.
2. **Registry** (Cloudflare Worker + R2): versioned app registry. Metadata in R2 JSON,
   blobs in R2 via presigned URLs (tarballs never traverse the Worker). Auth: hardcoded
   bearer tokens.
3. **Launcher** (fork of opnble, Tauri 2 + React 19): non-tech operator installs apps
   from the registry, apps run as containers in the bundled Alpine VM, each opens in its
   own desktop window, installed apps auto-start with the launcher, one-click updates
   that preserve data.

## Hard constraints

- macOS Apple Silicon only. Demo machine = this Mac.
- Reuse opnble's existing VM image (`~/.opnble/vm/alpine-base.img`, CoW-cloned to
  `~/.vibox/vm/`). **No agent, proto, or VM image changes.** All podman operations go
  through the existing `ExecHost` RPC (`sh -c` in VM, returns exit/stdout/stderr).
- Container names keep internal `opnble-` prefixes where the agent validates them;
  invisible to users.
- opnble must not run concurrently (gvproxy MAC + vsock socket conflicts).

## .vibox format

`tar.zst` containing:

- `manifest.json` — `{ "name", "slug", "version" (semver), "icon" (emoji), "internal_port", "description" }`
- `image.tar` — `docker save` output, built `--platform linux/arm64`

Manifest types live in the CLI crate and are mirrored in the launcher's Rust side
(single workspace = single source of truth).

## Registry API (Worker)

Base: `https://<worker>.workers.dev`. Auth: `Authorization: Bearer <token>` (publish only;
install is unauthenticated for demo).

- `POST /v1/apps/{slug}/versions` body=manifest JSON → `{ upload_url }` (presigned R2 PUT
  for the .vibox blob) ; finalize with `POST /v1/apps/{slug}/versions/{version}/complete`
- `GET /v1/apps` → `[ { manifest, latest_version } ]`
- `GET /v1/apps/{slug}/latest` → `{ manifest, download_url }` (presigned R2 GET)

R2 layout: `apps/{slug}/{version}/app.vibox` + `apps/{slug}/{version}/manifest.json` +
`apps/{slug}/latest` (pointer file). No DB; `GET /v1/apps` lists R2.

## Launcher: app lifecycle (all via existing gateway/AgentClient)

State: `~/.vibox/state.json` — installed apps `{ slug, version, manifest, host_port, status }`.
Artifacts: `~/.vibox/apps/{slug}/` (VirtioFS-mounted into VM at `/repos`, tag unchanged
`opnble-repos`; guest path `/repos/{slug}/image.tar`).

- **install(slug)**: GET latest → download .vibox → unpack to `~/.vibox/apps/{slug}/` →
  `ExecHost: podman load -i /repos/{slug}/image.tar` → save state.
- **run(slug)**: allocate host_port (reuse opnble allocator) →
  `ExecHost: podman volume create vibox-{slug}; podman rm -f vibox-{slug} 2>/dev/null;
  podman run -d --name vibox-{slug} -p {host_port}:{internal_port} -v vibox-{slug}:/data <image>`
  → host-side vsock port-forward {host_port} (existing mechanism) → status Running on TCP probe.
- **open(slug)**: new Tauri `WebviewWindow` at `http://localhost:{host_port}`, title = app name.
- **update(slug)**: install new version (same flow) → `podman rm -f` old → run. Volume
  `vibox-{slug}` untouched ⇒ **data survives updates** (demo beat).
- **startup run**: after VM ready, auto-run all installed apps.

Tauri command contract (UI ↔ Rust, fixed now for parallel work):
`registry_apps() -> Vec<StoreApp>`, `installed_apps() -> Vec<InstalledApp>`,
`install_app(slug)`, `run_app(slug)`, `stop_app(slug)`, `open_app(slug)`,
`update_app(slug)`, events: `app-status-{slug}`, `install-progress-{slug}`.

## Launcher UI (Steam-like, replaces opnble pages)

Two tabs: **Library** (installed: tile = icon, name, status dot, Open / Update-available
badge) and **Store** (registry list: Install button + progress). No auth gate, no git,
no import flow. Dark, calm, big tiles.

## Strip list (fork hygiene, minimal viable)

Bypass/remove from UI + command registry: GitHub OAuth gate, import flow, branch/sync,
tunnel, publish, snapshots, Cursor MCP. Rust modules stay compiled where harmless
(deny-warnings workspace: prefer bypassing wiring over deep deletion under time pressure).

## Demo script (3 min)

1. Launcher open on "operator's" screen: Library shows `Delivery Tracker v1` running (real data in it).
2. Vibe-code moment: show app source (Dockerfile + FastAPI/sqlite in /data), bump a feature with Claude.
3. `vibox pack && vibox publish` → version 2 hits the registry.
4. Operator's Library shows **Update** badge → click → seconds later v2 runs, **old data intact**.
5. Store tab: install a second app live, opens in its own window.

## Risks / cuts

- Workers blob limit: dodged via presigned URLs.
- Image size: demo apps build FROM alpine-based images; tarballs ~50MB.
- If registry misbehaves at demo time: CLI `--registry http://localhost:8787` (wrangler dev) fallback.
- Cut: real users/auth, drag-and-drop install, multi-platform, signing, log streaming UI.

## Time budget (start T+0)

- T+0:00–0:30 — Phase A (serial): fork compiles, `~/.vibox` home, VM image CoW clone, VM boots, agent pings.
- T+0:30–2:00 — Phase B (parallel agents): Rust lifecycle commands · React Steam UI · CF Worker · CLI crate · 2 demo apps.
- T+2:00–3:00 — Phase C: integrate, end-to-end demo run, rehearse, fix.
