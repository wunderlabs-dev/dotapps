Start the Opnble dev stack (Vite + Tauri + VM agent).

Run `make dev-all`. If that fails with a lockfile or VM error, run `make status` first to diagnose, then propose the next step rather than guessing.

Do not run `pnpm dev`, `cargo tauri dev`, or `vite` directly: the `~/.opnble/dev.lock` lockfile prevents concurrent dev sessions and raw commands corrupt VM state.
