---
name: rust-validate
description: Use when validating Rust changes in Opnble, before committing Rust work, after Rust refactors, or when clippy, fmt, or tests fail - runs the Makefile-backed Rust quality gate and reports evidence.
---

# Rust Validate

Validate Rust through the repo Makefile. Do not run raw dev workflows.

## Workflow

1. Run `make fmt-check`.
2. Run `make lint-rust`.
3. Run `make test`.
4. Stop on the first failure, read the output, fix the issue, and rerun from the failed step.

Use `make check` when Rust changes may affect generated bindings, frontend contracts, or shared quality gates.

## Evidence

Before saying the work is complete, report:

- The exact command.
- The exit code or final pass/fail line.
- Any skipped gate and why it was skipped.

## Common Fixes

- Formatting failure: run `cargo fmt --all`, then rerun `make fmt-check`.
- Clippy failure: fix the warning. Use `#[expect(clippy::lint_name, reason = "...")]` only when the exception is intentional.
- Test failure: fix the behavior or the test. Never delete a failing test to pass the gate.

## Opnble Rules

Always apply `.cursor/rules/rust-backend.mdc` and `.cursor/rules/opnble-verification.mdc` while using this skill.
