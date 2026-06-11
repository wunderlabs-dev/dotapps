Run the full quality gate before claiming any work is done.

Run `make check` (frontend `pnpm check` + Rust clippy + `cargo fmt --check`). Quote the actual pass/fail line in the response: an exit code of 0 with the last summary line is required evidence per `.cursor/rules/opnble-verification.mdc`.

If anything fails, list the failures by file:line, do not auto-fix unless asked.
