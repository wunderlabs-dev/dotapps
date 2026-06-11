# dotapps — demo runbook

Two personas, one Mac: **developer** (terminal + Claude) and **operator** (the
dotapps launcher window). The registry lives at
`https://dotapps-registry.isopusoktoday.workers.dev` (also bound to
`registry.dotapps.club` once NS propagates); the launcher and CLI never show
the URL on screen.

## One-time setup

```bash
# VM image seeded into ~/.dotapps (instant APFS clone of the opnble base image)
bash scripts/seed-vm-image.sh

# Developer-side env (publish credentials)
export DOTAPPS_REGISTRY=https://dotapps-registry.isopusoktoday.workers.dev
export DOTAPPS_TOKEN=$(cat /tmp/dotapps_token.txt)   # see docs/registry-deploy.md

BIN=target/debug/dotapps        # or target/release/bundle/macos/dotapps.app/Contents/MacOS/dotapps
```

The registry already has `cafe-tracker@1.0.0` and `shift-board@1.0.0` published.

## Launching the operator app

- **Demo build (deep links work):** open `target/release/bundle/macos/dotapps.app`.
  First launch registers the `dotapps://` scheme with Launch Services.
- **Dev build:** `DOTAPPS_REGISTRY=… make dev-all` (deep links use the plugin's
  runtime `register_all`, which is finicky on macOS — prefer the .app for links).

The launcher opens to **Library** (empty first run) and **Store** (lists the
two published apps).

## Beat 1 — install from the Store (the "dropbox" moment)

Operator clicks **Install** on Cafe Tracker → it downloads, `podman load`s in the
VM, and lands in Library. Click **Open** → it runs as a container and opens in
its own window. Add 2–3 bean deliveries. Close the window, Open again → the rows
are still there (data lives in the `dotapps-cafe-tracker-data` volume at `/data`).

## Beat 2 — update preserves data (the money shot)

Developer ships a new version with a weekly-totals view:

```bash
cd examples/cafe-tracker
cp v2-app.py app.py
# bump "version" in dotapps.json to 2.0.0
"$BIN" pack && "$BIN" publish        # publishes cafe-tracker 2.0.0
```

Within ~5s the operator's Library shows an **Update** badge on Cafe Tracker.
Click it → v2 installs and runs in seconds → Open → the new "This week" banner is
there **and every delivery row from v1 is still present.** Same volume, new image.

## Beat 3 — deep-link distribution (one click installs a specific app)

Send a link / QR; clicking it (or running `open`) installs + runs + opens the app
and focuses the launcher:

```bash
open "dotapps://shift-board"           # latest
open "dotapps://cafe-tracker@2.0.0"    # exact version (after Beat 2 publishes it)
```

`dotapps://{slug}` installs latest; `dotapps://{slug}@{version}` installs an exact
version via `GET /v1/apps/{slug}/versions/{version}`.

## Reset to a clean operator state

```bash
make kill-dev
rm -f ~/.dotapps/apps.json
rm -rf ~/.dotapps/repos/*
bash scripts/seed-vm-image.sh
```

## Insurance

- **Registry down on stage:** `cd src/worker && npx wrangler dev` and
  `export DOTAPPS_REGISTRY=http://localhost:8787` (CLI + launcher both honor it;
  the CLI allows plain http only for loopback).
- **Do NOT run the opnble app** while dotapps's VM is up (shared gvproxy MAC +
  vsock sockets conflict).
- **Live re-pack fails:** `cafe-tracker@2.0.0` can be pre-published as a fallback.
