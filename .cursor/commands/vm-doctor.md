Diagnose VM start failures.

Run `make vm-doctor` and read its output carefully. Common patterns:

- vfkit zombie processes from a previous session: `make kill-dev` + reboot if the zombie is stuck in `UE` state
- Stale EFI store: `make reset-state` clears it
- Lockfile owned by a dead process: remove `~/.opnble/dev.lock` after verifying no real dev session is running
- Image clone missing: rebuild with `make vm-image` (macOS) or `make wsl-image` (Windows)

Report the diagnostic output verbatim plus the most likely cause and the single next step. Do not propose a reboot unless `make vm-doctor` explicitly says no fix is possible without one.
