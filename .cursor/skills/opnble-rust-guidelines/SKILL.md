---
name: opnble-rust-guidelines
description: Use when writing, refactoring, or reviewing Rust in Opnble and Microsoft Pragmatic Rust Guidelines may apply - maps Microsoft guideline themes onto Opnble's stricter local Rust rules without importing noisy compliance rituals.
---

# Opnble Rust Guidelines

Use this skill for Rust work after reading `.cursor/rules/rust-backend.mdc`. Opnble's local rules are authoritative when they are stricter or more specific.

## Source Priority

1. User request.
2. `.cursor/rules/rust-backend.mdc` and `.cursor/rules/opnble-verification.mdc`.
3. Microsoft Pragmatic Rust Guidelines, applied by intent.
4. General Rust preference.

Do not add `// Rust guideline compliant ...` comments. They create churn and do not prove compliance.

Bias toward zero comments. Comment discipline lives in `.cursor/rules/opnble-core.mdc` ("Comments") and `.cursor/rules/rust-backend.mdc` ("Doc Comments"). The short version: rename or extract before commenting; numbered step comments inside a function body are forbidden; private fn rustdoc is one short sentence at most.

## Microsoft Rules We Adopt

- `M-LINT-OVERRIDE-EXPECT`: prefer `#[expect(..., reason = "...")]` for intentional lint exceptions. Opnble already enforces documented suppressions.
- `M-LOG-STRUCTURED`: use structured logging. Tauri host uses `tracing`; VM agent uses `log`.
- `M-PANIC-IS-STOP` and `M-PANIC-ON-BUG`: panics are for programming bugs only. Opnble additionally denies explicit `panic`.
- `M-UNSAFE`, `M-UNSAFE-IMPLIES-UB`, `M-UNSOUND`: avoid unsafe. Opnble denies unsafe code.
- `M-DONT-LEAK-TYPES`: do not expose third-party error or transport types across Opnble APIs unless they are the point of the API.
- `M-TYPES-SEND`: design async and cross-thread types to be `Send` unless there is a concrete reason not to.
- `M-MODULE-DOCS` and `M-FIRST-DOC-SENTENCE`: public API docs should explain when to use the item and keep the first sentence short.

## Microsoft Rules To Apply Selectively

- `M-APP-ERROR`: applications may use `anyhow` or `eyre`, but Opnble uses structured `AppError` and `thiserror`. Do not introduce a second application error style without explicit direction.
- `M-MIMALLOC-APPS`: do not add a global allocator just because the guideline suggests it. Require a measured allocation hot path and project approval.
- `M-CANONICAL-DOCS`: use canonical sections for public reusable APIs and unsafe docs. Do not blanket-add verbose docs to private app internals.

## When You Need More Detail

Use `reference.md` for the topic map and upstream links. Load only the relevant topic, not the entire guideline set.

## Verification

Use the `rust-validate` skill for Rust gates. Report concrete command evidence before claiming completion.
