# vibox Implementation Plan (3-hour hackathon build)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Phase A is serial and MUST land on `main` first. Phase B tracks run as parallel subagents in isolated git worktrees (one branch per track). Phase C is serial integration on `main`.

**Goal:** Turn the opnble fork into vibox: a Steam-like desktop launcher where a non-technical operator installs and runs vibe-coded Dockerfile apps published to a registry by a developer CLI.

**Architecture:** Three components. (1) `vibox` CLI (new Rust workspace crate) packs a Dockerfile project into a `.vibox` artifact (zstd tar of `manifest.json` + `docker save` image tar) and publishes it to (2) a Cloudflare Worker + R2 registry (rewrite of the existing `src/worker/`). (3) The launcher (this repo's Tauri app) installs from the registry and runs each app as a podman container inside the already-working Alpine VM, driving podman exclusively through the existing `ExecHost` ttrpc RPC — **zero changes to `src/agent/`, `src/proto/`, or the VM image**. Apps get a persistent named volume at `/data`, open in their own `WebviewWindow`, and auto-start with the launcher.

**Tech Stack:** Rust (Tauri 2, tauri-specta, reqwest, tar, zstd, clap), React 19 + TanStack Router/Query + Tailwind 4, Cloudflare Workers + R2 (TypeScript, aws4fetch), podman-in-VM via vfkit.

---

## AMENDMENTS (decided mid-execution; supersede conflicting text below)

**A. Rebrand vibox → dotapps** (Vlad, after all Phase B tracks were dispatched). The B-track
sections below still say "vibox" — they were executed as written; the rename is ONE mechanical
pass in the new Phase C0 (after the last track merges, so it never races a running agent):

| Surface | Was | Becomes |
|---|---|---|
| Product name / identifier / window title / UI wordmark | vibox, com.vibox.app | dotapps, com.dotapps.app |
| Home dir | ~/.vibox | ~/.dotapps |
| CLI crate / bin / src dir | vibox-cli, `vibox` | dotapps-cli, `dotapps` |
| Artifact extension / project manifest | .vibox, vibox.json | **.apps**, dotapps.json |
| Env vars | VIBOX_REGISTRY, VIBOX_TOKEN | DOTAPPS_REGISTRY, DOTAPPS_TOKEN |
| Tauri commands / frontend client | vibox_*, viboxApi | dotapps_*, dotappsApi |
| Worker name / R2 bucket | vibox-registry | dotapps-registry (redeploy + new bucket; old ones deleted after cutover) |
| Image tag / container / volume prefix | vibox/{slug}, vibox-{slug}, vibox-{slug}-data | dotapps/{slug}, dotapps-{slug}, dotapps-{slug}-data |
| Repo directory name | vibox | unchanged until after the hackathon (live worktrees/session) |
| Rust crates opnble/opnble-agent/opnble_lib, container prefix `opnble-`, mount tag `opnble-repos` | — | unchanged (internal; baked into VM image) |

**B. Registry domain**: `registry.dotapps.club` (zone being added to Cloudflare; GoDaddy NS
flip in progress — background poll watches for activation, then bind a custom domain to the
worker). Until/unless active by demo time: workers.dev URL is used but NEVER shown on screen.

**C. Security hardening (done, commit 57d0183)** from the automated review of the CLI:
token only sent to registry-origin URLs; https-only registries (loopback http allowed for
`wrangler dev`); slug/version/port validated at both CLI entry points; random exclusive
temp dir for pack. The Worker already enforced slug/version validation server-side.

**D. Phase C is now:**
- **C0 Rename pass** (after B1 merges): apply table A across Rust/TS/worker/examples/docs;
  redeploy worker as dotapps-registry with fresh PUBLISH_TOKEN; update docs/registry-deploy.md;
  re-seed `~/.dotapps` (seed script rename); full quality gates (cargo clippy+test, pnpm check,
  worker tsc) and a fresh registry smoke test.
- **C1 Merge + build** (as originally written, but order became B3→B5→B4→B2→[B1] due to
  actual completion times; Cargo.lock conflicts resolved at each Rust merge).
- **C2 End-to-end rehearsal** (as written, with dotapps names + registry.dotapps.club).
- **C3 Demo insurance** (as written; local fallback uses `dotapps publish --registry http://localhost:8787`).

**Non-negotiable constraints:**
- Container name prefix `opnble-` (`constants.rs` `containers::NAME_PREFIX`) and VirtioFS mount tag `opnble-repos` (`VIRTIOFS_MOUNT_TAG`) **must not change** — they are baked into the VM image's agent validation and `/etc/fstab`. (`ExecHost`-launched containers named `vibox-*` are fine: prefix validation only applies to the agent's own container RPCs.)
- The Rust crate names (`opnble`, `opnble-agent`, `opnble_lib`) **stay** — renaming breaks logging filters and the specta export. Cosmetic only.
- Do not run the opnble app while vibox's VM is up (gvproxy MAC + socket conflicts).
- Workspace lints are strict (`unwrap_used` deny, `panic` deny, `indexing_slicing` deny…). Use `?` + `AppError`/`anyhow`; `.expect("reason")` is the existing escape hatch pattern.

**Frozen wire contracts (all tracks build against these; do not drift):**

`Manifest` (JSON, camelCase):
```json
{ "name": "Cafe Tracker", "slug": "cafe-tracker", "version": "1.0.0", "icon": "☕", "internalPort": 8000, "description": "Log coffee bean deliveries" }
```

Registry HTTP API (base = Worker URL; publish endpoints require `Authorization: Bearer $VIBOX_TOKEN`):
```
GET  /v1/apps                                   → { "apps": [ { "manifest": Manifest } ] }
GET  /v1/apps/{slug}/latest                     → { "manifest": Manifest, "downloadUrl": "<absolute url>" }
POST /v1/apps/{slug}/versions     body=Manifest → { "uploadUrl": "<absolute url>", "completeUrl": "<absolute url>" }
PUT  {uploadUrl}                  body=.vibox blob (Bearer required when uploadUrl points back at the Worker)
POST {completeUrl}                              → { "ok": true }
```

Tauri commands (snake_case names; frontend calls them via `invoke()` directly — no dependency on specta regen):
```
vibox_registry_apps()            -> Vec<StoreApp>      // StoreApp = { manifest: Manifest }
vibox_installed_apps()           -> Vec<InstalledApp>  // InstalledApp = { manifest, hostPort: u16|null, running: bool }
vibox_install_app(slug: String)  -> InstalledApp       // also used for updates
vibox_run_app(slug: String)      -> u16                // host port
vibox_stop_app(slug: String)     -> ()
vibox_open_app(slug: String)     -> ()
```

`.vibox` file: `zstd`-compressed tar containing exactly `manifest.json` and `image.tar` at the root. Image inside is tagged `vibox/{slug}:{version}`, built `--platform linux/arm64`.

R2 key layout: `apps/{slug}/{version}/app.vibox`, `apps/{slug}/{version}/manifest.json`, `apps/{slug}/latest` (JSON `{"version":"1.0.0"}`).

Host disk layout: `~/.vibox/repos/{slug}/image.tar` + `manifest.json` (VirtioFS-mounted in VM at `/repos/{slug}/`), installed-state in `~/.vibox/apps.json`.

---

## Phase A — Foundation (SERIAL, lands on `main` before anything else)

### Task A1: Identity + home-dir rename

**Files:**
- Modify: `src/tauri/src/constants.rs:103` (`BASE_DIR`)
- Modify: `src/tauri/tauri.conf.json:3,5` and window block `:14-23`
- Modify: `Makefile:13` and every `~/.opnble` literal (lines ~45-57, 67, 124, 205-207, 246)
- Modify: `src/tauri/src/app/logging.rs:28`
- Already modified (uncommitted): `src/tauri/src/app/bootstrap.rs` (global `~/.cursor/mcp.json` install no-op'd — keep it)

**Step 1:** In `src/tauri/src/constants.rs` change:
```rust
pub const BASE_DIR: &str = ".opnble";
```
to
```rust
pub const BASE_DIR: &str = ".vibox";
```
Do **not** touch `containers::NAME_PREFIX`, `BUNDLED_IMAGE`, or `VIRTIOFS_MOUNT_TAG` in the same file.

**Step 2:** In `src/tauri/tauri.conf.json` set `"productName": "vibox"`, `"identifier": "com.vibox.app"`, and in the `windows[0]` block `"title": "vibox"`. Leave the updater block alone (best-effort background task; harmless in dev — recon: `updater.rs:43` 30s delay, silent failure).

**Step 3:** In `Makefile`, replace every `~/.opnble` / `$(HOME)/.opnble` with the `.vibox` equivalent (`LOCKFILE`, `kill-dev` rm lines, `reset-state`, `status`, `setup-dirs`, `clean-all`). Verify with:
```bash
grep -n "\.opnble" Makefile        # expected: no output
```

**Step 4:** In `src/tauri/src/app/logging.rs:28` change `"opnble.log"` → `"vibox.log"`. Do NOT change the `opnble_lib=` tracing filter strings (crate name unchanged).

**Step 5:** Sanity-grep for remaining runtime-affecting paths (test-only and cosmetic hits are fine; Windows/WSL files are out of scope — macOS only):
```bash
grep -rn "\.opnble" src/tauri/src --include="*.rs" | grep -v test
```
Expected: no hits that affect macOS runtime paths.

**Step 6:** Build check + commit:
```bash
cd src/tauri && cargo build 2>&1 | tail -5   # expected: Finished `dev` profile
cd ../.. && git add -A && git commit -m "Rename app identity and home dir to vibox"
```

### Task A2: Seed the VM image into ~/.vibox

**Files:**
- Create: `scripts/seed-vm-image.sh`

**Step 1:** Create `scripts/seed-vm-image.sh` (mode 755):
```bash
#!/usr/bin/env bash
# Seeds ~/.vibox/vm with the locally-built opnble base image so first launch
# skips the 4GB download. APFS clonefile makes the copy instant.
set -euo pipefail

SRC="$HOME/.opnble/vm/alpine-base.img"
DST_DIR="$HOME/.vibox/vm"
[ -f "$SRC" ] || { echo "missing $SRC — build or download it via opnble first"; exit 1; }

mkdir -p "$DST_DIR"
[ -f "$DST_DIR/alpine-base.img" ] || cp -c "$SRC" "$DST_DIR/alpine-base.img"

# Stamp the version the live manifest currently advertises so the
# downloader's version check (vm/image_downloader.rs status()) is satisfied.
VERSION=$(curl -fsS --max-time 10 https://openable.dev/vm/manifest.json | python3 -c 'import json,sys; print(json.load(sys.stdin)["imageVersion"])' || echo "local-seed")
printf '{"imageVersion":"%s"}\n' "$VERSION" > "$DST_DIR/image-version.json"
echo "seeded $DST_DIR (version: $VERSION)"
```
Note the stamp field name: confirm against the `InstalledStamp` struct in `src/tauri/src/vm/image_downloader.rs:52-55` — it serializes `image_version`; check its `#[serde(rename_all)]` attribute and match the JSON key exactly (recon saw camelCase stores elsewhere; 30 seconds of reading beats a wrong guess).

**Step 2:** Run it:
```bash
bash scripts/seed-vm-image.sh && ls -la ~/.vibox/vm/
```
Expected: `alpine-base.img` (4GB, instant copy) + `image-version.json`.

**Step 3:** Commit: `git add scripts/seed-vm-image.sh && git commit -m "Add VM image seed script"`

### Task A3: Boot gate — prove the fork runs

**Step 1:** Frontend deps are installed (`src/web/node_modules` exists). Start dev:
```bash
make dev-all
```
**Step 2:** Verify: app appears in tray; open dashboard from tray; VM status reaches Running (agent ping OK) — watch `~/.vibox/logs/vibox.log` for `agent` ping success; the GitHub login screen appearing is EXPECTED at this point (Track 2 removes it). vfkit/gvproxy processes present:
```bash
ps aux | grep -E "vfkit|gvproxy" | grep -v grep   # expected: both processes
```
**Step 3:** Quit the app (tray → Quit). `make kill-dev` to clean sockets. If the VM does NOT boot, STOP and debug before dispatching Phase B — everything depends on this.

---

## Phase B — Parallel tracks (each = one subagent in its own worktree/branch)

Merge order back to main: **B1 → B4 → B2 → B3/B5** (B1 and B4 both touch Cargo.lock; B2/B3/B5 are disjoint).

---

### Track B1: Rust host — apps module (branch `track/apps-rust`)

**Files:**
- Create: `src/tauri/src/apps/mod.rs`, `types.rs`, `store.rs`, `registry.rs`, `commands.rs`
- Modify: `src/tauri/src/lib.rs` (add `pub mod apps;`)
- Modify: `src/tauri/src/app/register.rs` (register 6 commands in the `specta_builder_with!` macro, lines 6-74)
- Modify: `src/tauri/Cargo.toml` (add `tar = "0.4"`; `zstd` and `reqwest` are already deps — verify, and add if missing)
- Modify: `src/tauri/src/vm/lifecycle.rs` (or whichever managed type `init_vm` uses — expose `exec_host` + vsock path; see Step 3)
- Modify: `src/tauri/src/app/platform/macos.rs:72-76` (auto-start installed apps after VM ready)
- Test: inline `#[cfg(test)]` in `store.rs` and `types.rs`

**Step 1: Types** — `src/tauri/src/apps/types.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub name: String,
    pub slug: String,
    pub version: String,
    pub icon: String,
    pub internal_port: u16,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub manifest: Manifest,
    pub host_port: Option<u16>,
    #[serde(default)]
    pub running: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StoreApp {
    pub manifest: Manifest,
}

impl Manifest {
    pub fn image_ref(&self) -> String {
        format!("vibox/{}:{}", self.slug, self.version)
    }
    pub fn container_name(&self) -> String {
        format!("vibox-{}", self.slug)
    }
    pub fn volume_name(&self) -> String {
        format!("vibox-{}-data", self.slug)
    }
}
```
Add a test that a JSON manifest with camelCase keys round-trips (`internalPort` → `internal_port`). Run `cargo test -p opnble apps` — must pass before continuing.

**Step 2: Store** — `src/tauri/src/apps/store.rs`. Self-contained JSON store at `~/.vibox/apps.json` (do NOT entangle with `JsonProjectStore`); follow its atomic-write idea but keep it minimal:
```rust
use super::types::InstalledApp;
use crate::app::errors::AppError; // adjust path: grep `pub enum AppError` for the real module
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

const FIRST_PORT: u16 = 4100;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct State {
    apps: HashMap<String, InstalledApp>, // slug -> app
}

pub struct AppStore {
    path: PathBuf,
    state: Mutex<State>,
}

impl AppStore {
    pub fn load(path: PathBuf) -> Self {
        let state = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path, state: Mutex::new(state) }
    }

    pub fn list(&self) -> Result<Vec<InstalledApp>, AppError> { /* lock, clone values, sort by name */ }
    pub fn get(&self, slug: &str) -> Result<Option<InstalledApp>, AppError> { /* lock + clone */ }
    pub fn upsert(&self, app: InstalledApp) -> Result<(), AppError> { /* insert + save() */ }
    pub fn set_running(&self, slug: &str, running: bool) -> Result<(), AppError> { /* mutate + save() */ }

    /// Stable port per slug: reuse if assigned, else first free >= FIRST_PORT.
    pub fn allocate_port(&self, slug: &str) -> Result<u16, AppError> { /* see test */ }

    fn save(&self, state: &State) -> Result<(), AppError> {
        // write to .tmp sibling, then rename (atomic on same volume)
    }
}
```
Write tests FIRST (failing), then implement: (a) upsert + list round-trip via a tempdir path, (b) `allocate_port` returns 4100 for first app, 4101 for second, and the same port on repeat calls for the same slug. Mirror the existing test style at the bottom of `src/tauri/src/projects/store.rs` (including its clippy `allow` attributes for tests). Run `cargo test -p opnble apps::store` — green before continuing. Commit: `feat: vibox app types and store`.

**Step 3: Expose ExecHost to commands.** Find how the frontend's `init_vm` command reaches the VM (grep `fn init_vm` in `src/tauri/src`). That command's managed state (the type wrapping `Arc<Mutex<Option<VmGateway>>>` — per recon it's the lifecycle in `src/tauri/src/vm/lifecycle.rs`, with `VmRuntime` doing the same at `vm/runtime.rs:21-43`) is where you add two methods (adapt names to the real struct):
```rust
pub async fn exec_host(&self, cmd: &str) -> Result<(i32, String, String), AppError> {
    let guard = self.manager.lock().await;
    let gw = guard.as_ref().ok_or_else(/* reuse the existing vm-not-running AppError, see vm/runtime.rs:70-90 */)?;
    let agent = gw.agent();
    agent.exec_host(cmd).await
}
pub async fn vsock_path(&self) -> Option<std::path::PathBuf> { /* from gw.vsock_socket_path() */ }
```
`AgentClient::exec_host(&self, command: &str) -> Result<(i32, String, String), AppError>` already exists (`vm/agent_client.rs:426-439`, 120s timeout — fine for `podman load` of ~100MB tars). Whatever type you extend must already be `.manage()`d in `bootstrap.rs:109-165`; if only `Arc<dyn Runtime>` is managed, manage the concrete Arc additionally where it's constructed.

**Step 4: Registry client** — `src/tauri/src/apps/registry.rs`. Registry base URL: `std::env::var("VIBOX_REGISTRY")` falling back to the deployed Worker URL (placeholder constant `pub const DEFAULT_REGISTRY: &str = "https://vibox-registry.REPLACE.workers.dev";` — integration phase fills the real subdomain):
```rust
pub async fn fetch_store_apps() -> Result<Vec<StoreApp>, AppError>   // GET {base}/v1/apps, parse { apps: [...] }
pub async fn fetch_latest(slug: &str) -> Result<(Manifest, String), AppError>  // GET .../latest → (manifest, downloadUrl)
pub async fn download_vibox(url: &str, dest: &std::path::Path) -> Result<(), AppError>  // stream to file
pub fn unpack_vibox(file: &std::path::Path, dest_dir: &std::path::Path) -> Result<Manifest, AppError>
// unpack: zstd::stream::Decoder over File -> tar::Archive::unpack(dest_dir)
// then read dest_dir/manifest.json; reject archive entries with path components other than the two known names
```
Use the same reqwest client pattern as `vm/image_downloader.rs` (it already streams + verifies large downloads — copy its style). Test: build a tiny `.vibox` in a tempdir (write manifest.json + dummy image.tar, tar+zstd it with the same crates) and assert `unpack_vibox` round-trips. Commit: `feat: vibox registry client`.

**Step 5: Commands** — `src/tauri/src/apps/commands.rs`. All six per the frozen contract. Core logic (pseudocode-level, write real Rust matching `projects/commands.rs` idioms — `#[tauri::command] #[specta::specta]`, `State<'_, Arc<...>>` params):
```text
vibox_install_app(slug):
  (manifest, url) = registry::fetch_latest(slug)
  dir = paths::repos_dir().join(&manifest.slug)        # constants.rs path helpers
  download to dir/app.vibox; unpack_vibox -> dir/{manifest.json,image.tar}; rm app.vibox
  vm.exec_host(&format!("podman load -i /repos/{}/image.tar", manifest.slug))  -> err if exit != 0 (include stderr in AppError)
  store.upsert(InstalledApp { manifest, host_port: existing-or-None, running: false })

vibox_run_app(slug):
  app = store.get(slug) else err; port = store.allocate_port(slug)
  cmd = format!(
    "podman volume create {vol} >/dev/null 2>&1; podman rm -f {name} >/dev/null 2>&1; \
     podman run -d --name {name} -p {port}:{internal} -v {vol}:/data {image}",
     vol=.., name=.., port=port, internal=app.manifest.internal_port, image=app.manifest.image_ref())
  vm.exec_host(&cmd) -> err if exit != 0
  start port forward: vm::port_forward::start_forwarding(&vsock_path, port, port)   # vm/port_forward.rs:24-40
  keep JoinHandle in a managed Mutex<HashMap<String, JoinHandle<()>>> (mirror vm/runtime.rs port_forwards)
  store.upsert(host_port=Some(port)); store.set_running(slug, true); Ok(port)

vibox_stop_app(slug):
  vm.exec_host("podman stop -t 5 vibox-{slug}"); abort+remove forward handle; set_running(false)

vibox_open_app(slug):
  port = store.get(slug)?.host_port else err
  WebviewWindowBuilder::new(&app, format!("app-{slug}"), WebviewUrl::External(format!("http://localhost:{port}").parse()?))
      .title(&manifest.name).inner_size(1100.0, 750.0).build()
  # pattern: tray.rs:294-310; if window with that label exists, .set_focus() instead (get_webview_window)

vibox_registry_apps(): registry::fetch_store_apps()
vibox_installed_apps(): store.list(); for each running flag refresh is OPTIONAL (skip; frontend treats store as truth)
```
Update is just `vibox_install_app` again — the volume name has no version in it, so data survives; frontend calls install then run.

**Step 6:** Register module + commands: `src/tauri/src/lib.rs` add `pub mod apps;`. In `app/register.rs` add the six `crate::apps::commands::...` entries to the macro list. In `bootstrap.rs` `.manage(Arc::new(AppStore::load(paths::base_dir().join("apps.json"))))` and the forward-handle map. Build:
```bash
cd src/tauri && cargo clippy --all-targets 2>&1 | tail -5   # expected: no warnings (deny-lints workspace)
cargo test -p opnble 2>&1 | tail -5                          # expected: all pass
```
Commit: `feat: vibox app lifecycle commands`.

**Step 7: Auto-start ("startup run").** In `src/tauri/src/app/platform/macos.rs` success branch after VM ready (lines 72-76), spawn a task: read the managed `AppStore`, for each installed app invoke the same run path as `vibox_run_app` (factor the body into `pub async fn run_app_inner(...)` in `apps/commands.rs` so both call it). Failures: log + continue (one broken app must not block others). Commit: `feat: auto-run installed apps on startup`.

**Step 8:** Regen TS bindings so the types exist for later (frontend doesn't depend on it, but keep it green): `cd src/tauri && cargo run --example regen_bindings`. Commit if `src/web/gen/tauri.ts` changed.

---

### Track B2: Frontend — Steam-like launcher (branch `track/frontend`)

**Files:**
- Modify: `src/web/hooks/use-app-gate.ts` (skip login), `src/web/lib/setup-utils.ts` (skip vm-image welcome gate)
- Modify: `src/web/routes.ts:32-48` (index → LauncherPage; drop import route)
- Create: `src/web/lib/vibox.ts`, `src/web/pages/launcher-page.tsx`, `src/web/components/launcher/app-tile.tsx`, `library-tab.tsx`, `store-tab.tsx`
- Modify: `src/web/layouts/app-shell.tsx` + `src/web/components/layout.tsx` (drop the w-64 sidebar)

**Step 1: Kill the auth gate.** In `use-app-gate.ts` (lines 148-190): in the setup-complete transition, set phase straight to `"ready"` (never `"login"`), with `user` a stub `{ login: "operator", avatar_url: "" }` if the GateContext type requires one. Remove now-unused OAuth imports until `pnpm typecheck` is clean. The login screen component files can stay on disk unrouted.

**Step 2: Kill the vm-image welcome gate.** In `setup-utils.ts` `setupMacOS` (lines 56-82): delete the `vmImageStatus()`/`vm-image-welcome` branch (lines ~69-74); go straight to `initVm()` then `onComplete()`. (Phase A pre-seeded the image; belt and braces.)

**Step 3: Typed client** — `src/web/lib/vibox.ts` (decoupled from specta regen on purpose):
```typescript
import { invoke } from "@tauri-apps/api/core";

export interface Manifest {
  name: string; slug: string; version: string; icon: string;
  internalPort: number; description: string;
}
export interface InstalledApp { manifest: Manifest; hostPort: number | null; running: boolean }
export interface StoreApp { manifest: Manifest }

export const viboxApi = {
  registryApps: () => invoke<StoreApp[]>("vibox_registry_apps"),
  installedApps: () => invoke<InstalledApp[]>("vibox_installed_apps"),
  install: (slug: string) => invoke<InstalledApp>("vibox_install_app", { slug }),
  run: (slug: string) => invoke<number>("vibox_run_app", { slug }),
  stop: (slug: string) => invoke<void>("vibox_stop_app", { slug }),
  open: (slug: string) => invoke<void>("vibox_open_app", { slug }),
};
```

**Step 4: Launcher page.** `launcher-page.tsx`: header (`vibox` wordmark text, `text-foreground`, accent dot) + Radix Tabs (`components/ui/tabs`) with **Library** and **Store**. Both tabs use TanStack Query:
```typescript
const installed = useQuery({ queryKey: ["vibox","installed"], queryFn: viboxApi.installedApps, refetchInterval: 2000 });
const store = useQuery({ queryKey: ["vibox","store"], queryFn: viboxApi.registryApps, refetchInterval: 5000 });
```
`library-tab.tsx`: grid `grid grid-cols-3 gap-4` of `AppTile`s. Tile (Card primitive, `components/ui/card`): big emoji icon, name, version, green status dot when `running`. Buttons (Button primitive): **Open** (run-if-needed then `viboxApi.open`), **Update** badge shown when the store has the same slug with a different version → onClick `install(slug)` then `run(slug)` (show toast "Updated to vX — data preserved"). `store-tab.tsx`: same grid; **Install** button (busy state while pending, success toast, switch to Library). Empty states: Library → "No apps yet — install one from the Store"; Store unreachable → friendly error card with the registry URL. Use `useToast()` from `context/toast-context.tsx` for all outcomes. Use mutations via `useMutation` and `installed.refetch()` after each (codebase pattern: manual refetch, recon frontend §3).

**Step 5: Rewire routes + drop sidebar.** `routes.ts`: `indexRoute.component = LauncherPage`; delete the import route from the tree (settings can stay). `app-shell.tsx`/`layout.tsx`: remove the `w-64` aside so main content is full width.

**Step 6: Verify + commit.**
```bash
cd src/web && pnpm check 2>&1 | tail -10
```
Expected: biome + eslint + tsc + vitest all pass. If a vitest suite covers deleted wiring (e.g. import page tests), fix by removing only tests for code you unrouted-and-deleted; do NOT delete passing tests for files that remain. Commit: `feat: steam-like launcher UI, no auth gate`.

---

### Track B3: Registry Worker (branch `track/registry`)

**Files:**
- Modify: `src/worker/wrangler.toml` (rename, unbind openable.dev routes, new bucket)
- Rewrite: `src/worker/src/index.ts`
- Modify: `src/worker/package.json` (add `aws4fetch`)

**Step 1: wrangler.toml.** `name = "vibox-registry"`. **Delete the `routes` array entirely** (serve from workers.dev; the old routes belong to live openable.dev — touching them is forbidden). Keep `account_id`. Replace R2 bindings with:
```toml
[[r2_buckets]]
binding = "REGISTRY"
bucket_name = "vibox-registry"
```

**Step 2: index.ts.** Keep/adapt the existing `isSafeR2Key` guard (old index.ts lines 268-274) and the `serveR2` streaming helper (lines 325-410). Implement the frozen API. Env interface: `{ REGISTRY: R2Bucket, PUBLISH_TOKEN: string, R2_ACCESS_KEY_ID?: string, R2_SECRET_ACCESS_KEY?: string, ACCOUNT_ID?: string }`. Auth helper: `request.headers.get("authorization") === \`Bearer ${env.PUBLISH_TOKEN}\``→ else 401.

- `POST /v1/apps/{slug}/versions` (auth): parse Manifest JSON; validate `slug` matches `^[a-z0-9-]+$` and equals path slug; store nothing yet; return `uploadUrl` + `completeUrl`:
  - **Presigned path** (when `R2_ACCESS_KEY_ID` set): `aws4fetch` `AwsClient({ accessKeyId, secretAccessKey })`, sign `PUT https://{ACCOUNT_ID}.r2.cloudflarestorage.com/vibox-registry/apps/{slug}/{version}/app.vibox` with `aws.sign(url, { method: "PUT", aws: { signQuery: true } })`, 1h expiry.
  - **Fallback** (no creds): `uploadUrl = {origin}/v1/blob/apps/{slug}/{version}/app.vibox` (Worker streams `request.body` → `env.REGISTRY.put(key, body)`; auth required).
  - Stash the manifest at `apps/{slug}/{version}/manifest.json` immediately (simplifies complete).
- `POST /v1/apps/{slug}/versions/{version}/complete` (auth): verify `head(app.vibox key)` exists → write `apps/{slug}/latest` = `{"version": "..."}` → `{ ok: true }`.
- `GET /v1/apps`: `list({ prefix: "apps/", delimiter: "/" })` → for each slug prefix read `latest` then that version's `manifest.json` → `{ apps: [...] }`. Cache-Control 30s.
- `GET /v1/apps/{slug}/latest`: read pointer + manifest; `downloadUrl` presigned-GET when creds present, else `{origin}/v1/blob/...` (GET blob route is unauthenticated, served via `serveR2` with Range support).
- `PUT|GET /v1/blob/{...key}` fallback routes, guarded by `isSafeR2Key`.

**Step 3: Deploy + smoke test** (run from `src/worker/`):
```bash
npx wrangler r2 bucket create vibox-registry
echo "<generate: openssl rand -hex 16>" | npx wrangler secret put PUBLISH_TOKEN
npx wrangler deploy           # note the printed https://vibox-registry.<sub>.workers.dev URL
TOKEN=<same value>
BASE=https://vibox-registry.<sub>.workers.dev
curl -s $BASE/v1/apps                                  # expected: {"apps":[]}
curl -s -X POST $BASE/v1/apps/smoke/versions -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"name":"Smoke","slug":"smoke","version":"0.0.1","icon":"🧪","internalPort":8000,"description":""}'
# expected: {"uploadUrl":"...","completeUrl":"..."}; then PUT a small file to uploadUrl, POST completeUrl,
# and GET /v1/apps shows smoke; GET /v1/apps/smoke/latest returns a downloadUrl that curls back the file.
```
Record `BASE` and `TOKEN` in `docs/plans/registry-deploy.md` (gitignored? it is NOT — fine, token is throwaway demo auth) for Phase C. R2 keys for presigning are OPTIONAL (Vlad creates in dashboard if time allows: R2 → Manage API Tokens → Object Read & Write on vibox-registry; then `wrangler secret put R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY` / `ACCOUNT_ID` and redeploy) — the fallback path keeps everything working without them.

**Step 4:** Commit: `feat: vibox registry worker`.

---

### Track B4: CLI (branch `track/cli`)

**Files:**
- Modify: root `Cargo.toml` (workspace members += `"src/cli"`)
- Create: `src/cli/Cargo.toml`, `src/cli/src/main.rs` (+ small modules if taste demands)

**Step 1:** `src/cli/Cargo.toml`:
```toml
[package]
name = "vibox-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "vibox"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
reqwest = { version = "0.12", features = ["blocking", "json", "rustls-tls"], default-features = false }
tar = "0.4"
zstd = "0.13"

[lints]
workspace = true
```
(Workspace lints deny `unwrap`; use `anyhow::Context` everywhere.)

**Step 2: TDD the manifest + archive core.** Failing tests first in `main.rs` `#[cfg(test)]`: (a) `Manifest` parses the frozen camelCase JSON; (b) `create_vibox(manifest_path, image_tar_path, out)` produces a file that, when decoded with `zstd::stream::Decoder` + `tar::Archive`, contains exactly `manifest.json` and `image.tar`. Implement, `cargo test -p vibox-cli` green. Commit.

**Step 3: Subcommands.**
```text
vibox pack [--dir .] [--out {slug}-{version}.vibox]
  reads {dir}/vibox.json (the Manifest; "version" required)
  runs: docker build --platform linux/arm64 -t vibox/{slug}:{version} {dir}     (std::process::Command, inherit stdio)
  runs: docker save -o {tmp}/image.tar vibox/{slug}:{version}
  writes manifest.json + image.tar into the zstd tar → out file; prints size + path

vibox publish [--file <auto-detect newest *.vibox in cwd>] [--registry $VIBOX_REGISTRY] 
  token from $VIBOX_TOKEN (required; clear error if missing)
  reads manifest.json out of the archive (decode in memory)
  POST {registry}/v1/apps/{slug}/versions (Bearer) body=manifest → { uploadUrl, completeUrl }
  PUT uploadUrl body=file bytes (Bearer header too — harmless on presigned, required on fallback)
  POST completeUrl (Bearer) → expect {"ok":true}; print "published {slug} {version}"
```
Errors must be human: every `Command` failure prints captured stderr; non-2xx HTTP prints status + body.

**Step 4: Verify end-to-end against a throwaway dir** (after B3 deploys; if running before that, stop at pack):
```bash
cargo build -p vibox-cli && cargo clippy -p vibox-cli --all-targets 2>&1 | tail -3
mkdir -p /tmp/vibox-smoke && cd /tmp/vibox-smoke
cat > vibox.json <<'EOF'
{ "name":"Smoke","slug":"smoke-cli","version":"0.0.1","icon":"🧪","internalPort":8000,"description":"" }
EOF
cat > Dockerfile <<'EOF'
FROM alpine:3.20
CMD ["sh","-c","while true; do printf 'HTTP/1.1 200 OK\r\n\r\nok' | nc -l -p 8000; done"]
EOF
<repo>/target/debug/vibox pack && ls -la *.vibox    # expected: a few MB file
```
Commit: `feat: vibox CLI pack and publish`.

---

### Track B5: Demo apps (branch `track/demo-apps`)

**Files:**
- Create: `examples/cafe-tracker/{Dockerfile,vibox.json,app.py}`
- Create: `examples/cafe-tracker/v2-app.py` (the live-demo upgrade, applied during the demo)
- Create: `examples/shift-board/{Dockerfile,vibox.json,app.py}`

**Step 1: cafe-tracker.** `vibox.json`: `{ "name":"Cafe Tracker","slug":"cafe-tracker","version":"1.0.0","icon":"☕","internalPort":8000,"description":"Log coffee bean deliveries" }`. `Dockerfile`:
```dockerfile
FROM python:3.12-alpine
RUN pip install --no-cache-dir fastapi uvicorn
WORKDIR /srv
COPY app.py .
EXPOSE 8000
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8000"]
```
`app.py`: FastAPI, sqlite3 (stdlib) at `/data/cafe.db` (`os.makedirs("/data", exist_ok=True)`; table `deliveries(id, supplier, kg, roast, created_at)`); `GET /` returns a single inline-HTML page (dark, matches launcher vibe: `#191919` bg, gold `#c17812` accents) with a form (supplier, kg, roast) and the deliveries table, posting to `POST /deliveries` then redirecting `/`. No JS build, no external assets. `v2-app.py`: same plus a "This week" totals banner (`SELECT roast, SUM(kg) ... WHERE created_at > date('now','-7 day') GROUP BY roast`) and bump nothing else — during the demo: `cp v2-app.py app.py`, edit `vibox.json` version to `2.0.0`, re-pack, re-publish.

**Step 2: shift-board.** Same skeleton: `{ "name":"Shift Board","slug":"shift-board","icon":"📅","internalPort":8000, ... }`; `shifts(id, person, day, slot)` in `/data/shifts.db`; one page, add/list shifts.

**Step 3: Verify both locally with real Docker (TDD for containers = run them):**
```bash
cd examples/cafe-tracker
docker build --platform linux/arm64 -t vibox/cafe-tracker:1.0.0 . && \
docker run -d --rm --name ct-smoke -p 18000:8000 -v ct-smoke:/data vibox/cafe-tracker:1.0.0
curl -s localhost:18000 | head -5     # expected: HTML with "Cafe Tracker"
curl -s -X POST localhost:18000/deliveries -d 'supplier=Acme&kg=12&roast=dark' # expected: 303/200
docker rm -f ct-smoke && docker volume rm ct-smoke
```
Repeat pattern for shift-board. Commit: `feat: demo apps`.

---

## Phase C — Integration + demo (SERIAL, on `main`)

### Task C1: Merge + build
Merge order: `track/apps-rust` → `track/cli` (rebuild to settle `Cargo.lock`) → `track/frontend` → `track/registry` → `track/demo-apps`. After each merge with Rust changes: `cargo clippy --all-targets && cargo test`. After frontend: `pnpm check`. Set the real Worker URL in `apps/registry.rs::DEFAULT_REGISTRY` (from B3's deploy output). `cd src/tauri && cargo run --example regen_bindings`. Commit each merge.

### Task C2: End-to-end rehearsal (the actual demo, twice)
```bash
export VIBOX_REGISTRY=https://vibox-registry.<sub>.workers.dev
export VIBOX_TOKEN=<from B3>
# Developer side:
cd examples/cafe-tracker && vibox pack && vibox publish
cd ../shift-board       && vibox pack && vibox publish
# Operator side:
make dev-all    # launcher up, VM boots, Store tab shows both apps
# Install cafe-tracker → Open → add 2-3 deliveries → close window → Open again (data there)
# Developer: cp v2-app.py app.py; bump vibox.json to 2.0.0; vibox pack && vibox publish
# Operator: Update badge appears (≤5s poll) → Update → Open → totals banner AND old rows present  ← MONEY SHOT
# Store: install shift-board → opens in second window
```
Verify the data-survival explicitly: `~/.vibox` untouched between update; volume `vibox-cafe-tracker-data` persists (`podman volume ls` via tray debug or just the visible rows). Fix whatever breaks; re-run until clean twice in a row. Commit fixes individually.

### Task C3: Demo insurance
- `wrangler dev` in `src/worker/` = local registry fallback (`VIBOX_REGISTRY=http://localhost:8787`, CLI + `DEFAULT_REGISTRY` env override path already supports it).
- Pre-publish v2 under slug `cafe-tracker-backup` in case live re-pack fails on stage.
- `make kill-dev && bash scripts/seed-vm-image.sh` = clean-slate reset (delete `~/.vibox/apps.json` + `~/.vibox/repos/*` for a fully fresh operator story).
- Do NOT launch opnble during the demo.

**Known cut lines if the clock wins (in cut order):** auto-start on launch (B1 Step 7) → Update badge (manual reinstall via Store instead) → shift-board → presigned URLs (fallback already default without creds).
