Kill all dev processes, reset state, and start the dev stack fresh.

Run `make restart`. After it returns, run `make status` and show the table so we can confirm Vite, Tauri, and the VM are all healthy.

If `make restart` itself fails, run `make kill-dev` then `make reset-state` separately and report which step failed.
