<!--
  Title format: `<type>(<scope>): <what changed>`
  Examples:
    feat(updater): manual check button in Settings
    fix(vm): handle vfkit exit during gvproxy start
    chore(ci): pin tauri-cli to 2.11.2
-->

## Problem

<!-- One paragraph: what was wrong, what's missing, or what you want to enable. Link the issue / plan section if applicable. -->

## What changed

<!-- Bullets, one per logical chunk. Match commits if possible. -->

- 

## Verification

<!-- Paste the actual output of the matching gate from .cursor/rules/opnble-verification.mdc:
  - Frontend-only: `cd src/web && pnpm check`
  - Rust-only: `make lint-rust` (+ `make test` if applicable)
  - Both stacks: `make check`
-->

```
$ make check
...
```

## Risk

<!-- Low / Medium / High and one sentence on the failure mode. -->
