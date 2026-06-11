# Opnble

> Run any React/Next.js project locally with zero setup

Opnble is a native desktop app for macOS, Windows, and Linux that lets anyone run web projects from git repositories. No Node.js, npm, git, or terminal knowledge required: everything is bundled.

Paste a GitHub URL, click Start, and see the project running in seconds.

## Features

- **Zero prerequisites**: Node.js, npm, git, and containers are all bundled inside the app
- **One-click projects**: paste a repo URL or browse your GitHub repos, then click Start
- **Cross-platform**: macOS (Apple Silicon and Intel), Windows (WSL2), Linux (native Podman)
- **Private repositories**: GitHub OAuth for accessing private repos
- **Multi-project management**: run several projects simultaneously, each on its own port
- **Live log streaming**: real-time container output piped to the UI
- **Git operations**: switch branches, pull latest changes, automatic dependency sync
- **Tunnel sharing**: share a running project via a public Cloudflare Tunnel URL (one click)
- **Editor integration**: open any project in Cursor directly from the sidebar
- **Resource monitoring**: CPU and memory usage for the container runtime

## How it works

Opnble bundles a lightweight container runtime per platform:

| Platform | Runtime |
|----------|---------|
| macOS (Apple Silicon) | [vfkit](https://github.com/crc-org/vfkit) (Virtualization.framework) with Alpine Linux VM |
| macOS (Intel) | QEMU with Alpine Linux VM |
| Windows | WSL2 with Alpine distribution |
| Linux | Native Podman (no VM needed) |

When you start a project, the app clones the repo, mounts it into a container with Node.js pre-installed, installs dependencies, starts the dev server, and streams logs back to the UI. Port allocation, process lifecycle, and cleanup are all handled automatically.

## Download

Pre-built binaries are produced by the `Release` workflow on tag pushes.

| Platform | Status | Link |
|----------|--------|------|
| macOS (Apple Silicon) | Released (.dmg + auto-updater) | [Download .dmg](https://github.com/wunderlabs-dev/openable/releases/latest) |
| macOS (Intel) | Buildable from source | (no pre-built binary yet) |
| Windows | Buildable from source | (no pre-built binary yet) |
| Linux | Buildable from source | (no pre-built binary yet) |

Each platform's runtime path is implemented (vfkit / QEMU / WSL2 / native
Podman); only the macOS ARM build is currently wired into the release
matrix. Other platforms can be built locally with `make build`.

## Development

### Prerequisites

- Rust (latest stable)
- Node.js 20+ and pnpm
- Platform tools: vfkit (macOS ARM), QEMU (macOS Intel), Podman (Linux)

### Quick start

```bash
make install          # install all dependencies (Rust + frontend)
make dev-frontend     # start Vite dev server (separate terminal)
make dev              # start Tauri app (requires Vite running)
```

Or run both together:

```bash
make dev-all          # start Vite + Tauri in one command
```

### Useful commands

```bash
make restart          # kill everything, reset state, start fresh
make status           # show VM, agent, Vite, and project status
make check            # full quality gate (frontend lint + Rust clippy + fmt + tests)
make test-all         # run all tests (Rust + frontend)
make build            # production build
```

### VM image (required for macOS/Windows)

```bash
make vm-image         # build the Alpine + Node.js base VM image
```

The VM image is a read-only base. At runtime, the app creates an APFS copy-on-write clone (macOS) or WSL2 distribution (Windows) so the base image stays clean.

## Architecture

```
src/
  web/              React 19 + TypeScript + Tailwind 4 + Vite 7
  tauri/            Rust backend (Tauri 2.x)
  agent/            Rust gRPC server running inside the VM
  worker/           Cloudflare Worker for tunnel coordination
  proto/            Protocol Buffer definitions (ttrpc)
```

**Data flow**: user adds repo URL, React hook calls Tauri command, Rust clones via libgit2 to `~/.opnble/repos/`, container starts with repo mounted, logs stream via Tauri events, frontend renders the output.

The frontend follows a three-layer pattern: **hooks** (data fetching, subscriptions) feed **containers** (state wiring) which compose **components** (presentational, props-only).

## CI/CD

GitHub Actions workflows:

- **Frontend Quality Gate**: Biome + ESLint + TypeScript + Vitest (on `src/web/` changes)
- **Tauri Command Parity**: validates all Tauri commands are registered across platforms
- **Worker**: typechecks on PRs; deploys `src/worker/` to Cloudflare on push to `main`
- **Release** (on tags `v*`): builds and uploads binaries for macOS ARM. Intel Mac, Windows, and Linux jobs are not yet wired into the matrix; those platforms are buildable from source via `make build`.

### Worker deploy secrets

Add these repository secrets for automatic worker publishing:

| Secret | Purpose |
|--------|---------|
| `CLOUDFLARE_API_TOKEN` | API token with Workers Scripts Edit (and route access for `openable.dev`) |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare account ID |

Runtime worker secrets (`CF_API_TOKEN`, `CF_ACCOUNT_ID`, `CF_ZONE_ID`) are set once via `wrangler secret put` and are not overwritten by CI deploys.

Local deploy: `make worker-deploy` (after `wrangler login` or with the env vars above).

## License

MIT
