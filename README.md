# dotapps

> Dropbox for vibe-coded apps

dotapps lets a developer package a containerized app into a single shareable
artifact, publish it to a registry, and have a non-technical operator install,
update, and run it from a desktop launcher with one click. No Docker, no
terminal, no build steps on the operator's machine.

This is a hackathon project. It targets macOS on Apple Silicon.

## The three components

| Component | Path | Role |
|-----------|------|------|
| `dotapps` CLI | `src/cli/` | Developer tool: packs a Dockerfile project into a `.apps` artifact and publishes it to the registry |
| Registry | `src/worker/` | Cloudflare Worker + R2 at `registry.dotapps.club`: stores artifacts and serves the catalog |
| Launcher | `src/tauri/` + `src/web/` | The desktop app the operator uses to browse, install, update, and run apps |

## How it works

A developer writes any app with a `Dockerfile` and a `dotapps.json` manifest
(name, slug, version, icon, internal port, description). The `dotapps` CLI
builds the image for `linux/arm64`, then packs it into a `.apps` artifact: a
zstd-compressed tar holding exactly `manifest.json` and `image.tar`. `dotapps
publish` uploads that artifact to the registry, which stores it in R2 and
exposes it through a small HTTP API.

On the other side, the operator opens the launcher. The **Store** tab lists
every published app; the **Library** tab lists what they have installed.
Clicking **Install** downloads the `.apps` artifact, unpacks it, and
`podman load`s the image into a bundled Alpine VM. Clicking **Open** runs the
app as a podman container inside that VM, forwards its port to the host, and
shows it in its own window.

Each app gets a persistent named volume mounted at `/data`. Because the volume
name is derived from the app's slug and carries no version, **data survives
updates**: shipping a new version reloads a fresh image against the same
volume, so the operator's data is still there.

Apps can also be installed straight from a link. A `dotapps://{slug}` deep link
installs the latest version and runs it; `dotapps://{slug}@{version}` pins an
exact version. Clicking such a link (or `open "dotapps://cafe-tracker@2.0.0"`)
boots the VM if needed, installs, runs, opens the window, and focuses the
launcher.

```
developer                          registry                       operator
---------                          --------                       --------
dotapps.json + Dockerfile
  │
  ├─ dotapps pack  ─────────►  .apps (zstd tar: manifest.json + image.tar)
  │
  └─ dotapps publish ───────►  Cloudflare Worker + R2
                                    │
                                    └──────────►  launcher: install → podman load
                                                            run    → podman run -v data:/data
                                                            open   → window on forwarded port
```

## Demo

`docs/DEMO.md` is the full runbook. Two example apps ship under `examples/`
(`cafe-tracker` and `shift-board`): tiny FastAPI + SQLite apps that persist
their data under `/data`, used to show the install / update / data-survival
story.

## Development

The Makefile is the only supported dev entrypoint. A lockfile at
`~/.dotapps/dev.lock` prevents concurrent dev sessions from corrupting VM
state. Runtime data (installed apps, app artifacts, VM image) lives under
`~/.dotapps/`.

### Prerequisites

- Rust (latest stable)
- Node.js 20+ and pnpm
- macOS on Apple Silicon (vfkit + Virtualization.framework)

### Common targets

```bash
make install          # install dependencies (Rust + frontend)
make dev-all          # start Vite + the launcher together (typical dev loop)
make restart          # kill everything, reset state, start fresh
make status           # show VM, agent, Vite, and app status
make check            # full quality gate (frontend lint + Rust clippy + fmt + parity)
make test-all         # run all tests (Rust + frontend)
make build            # production build
make vm-image         # build the Alpine base VM image
make worker-deploy    # deploy the registry Worker to Cloudflare
```

The VM image is a read-only Alpine base. At runtime the launcher makes an APFS
copy-on-write clone so the base stays clean. `scripts/seed-vm-image.sh` seeds
`~/.dotapps` with a locally-built base image to skip the first-run download.

### CLI usage (developer side)

```bash
export DOTAPPS_REGISTRY=https://registry.dotapps.club   # optional override
export DOTAPPS_TOKEN=<publish token>

cd examples/cafe-tracker
dotapps pack        # builds the image and writes cafe-tracker-1.0.0.apps
dotapps publish     # uploads the newest .apps artifact to the registry
```

## Architecture

```
src/
  cli/      dotapps CLI (Rust): pack + publish .apps artifacts
  worker/   Cloudflare Worker + R2 registry (TypeScript)
  tauri/    launcher backend (Rust, Tauri 2): install/run apps, VM orchestration
  web/      launcher UI (React 19, TypeScript, Tailwind 4, Vite 7)
  agent/    Rust gRPC server running inside the VM (drives podman)
  proto/    Protocol Buffer definitions (ttrpc)
```

The launcher runs podman exclusively through the VM agent's `ExecHost` RPC.
The frontend follows a three-layer pattern: **hooks** (data fetching) feed
**containers** (state wiring) which compose **components** (presentational).

### Internal names (do not be surprised)

The launcher is built from a fork. Several internal identifiers keep the
fork's original `opnble` name and are intentionally left unchanged: the Rust
crates (`opnble`, `opnble_lib`, `opnble-agent`), the bundled binary
(`dotapps.app/Contents/MacOS/opnble`), the in-VM container prefix (`opnble-`),
and the VirtioFS mount tag (`opnble-repos`). These are baked into the VM image
and the build, so they stay even though the product is `dotapps`.

## Registry API

```
GET  /v1/apps                                → { apps: [{ manifest }] }
GET  /v1/apps/{slug}/latest                  → { manifest, downloadUrl }
GET  /v1/apps/{slug}/versions/{version}      → { manifest, downloadUrl }
POST /v1/apps/{slug}/versions                → { uploadUrl, completeUrl }   (Bearer)
PUT  {uploadUrl}                             → upload the .apps blob        (Bearer on fallback)
POST {completeUrl}                           → { ok: true }                 (Bearer)
```

Publish endpoints require `Authorization: Bearer $DOTAPPS_TOKEN`. When R2
credentials are configured the registry hands out presigned URLs so blob bytes
bypass the Worker; otherwise upload and download fall back to Worker-served
`/v1/blob/*` routes.

### Worker deploy secrets

| Secret | Purpose |
|--------|---------|
| `CLOUDFLARE_API_TOKEN` | API token with Workers Scripts Edit (and route access for `dotapps.club`) |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare account ID |

Local deploy: `make worker-deploy` (after `wrangler login` or with the env vars
above).

## License

MIT
