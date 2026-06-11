---
name: create-component
description: Use when creating or moving Opnble React components, containers, hooks, UI primitives, barrels, or design-system files under src/web - scaffolds files according to the hooks -> containers -> components architecture.
---

# Create Component

Follow `.cursor/rules/web-frontend.mdc` first. This skill is a quick scaffold reference, not a replacement for the frontend rule.

## Choose the Layer

- Hook: data fetching, subscriptions, business logic. File: `src/web/hooks/use-name.ts`.
- Container: state wiring and composition. File: `src/web/containers/name.tsx`.
- Component: presentational feature UI. File: `src/web/components/name.tsx`.
- UI primitive: reusable design-system primitive. File: `src/web/components/ui/name.tsx`.

Use kebab-case filenames. Export PascalCase components from the file and from the appropriate barrel when that directory has a public barrel.

## Hooks

- Return a readonly object interface.
- Returned callbacks use `useCallback`.
- Tauri calls use generated `commands` plus the local Result bridge.
- Error handling goes through `translateError()` or the established Result helper.
- Hooks return data and callbacks, never JSX.

## Containers

- Compose hooks and pass concrete props to presentational components.
- No raw Tailwind or raw HTML layout when a component should own it.
- No data fetching directly in presentational components.
- Name by feature role, not generic `Manager`, `Handler`, or `Wrapper`.

## Components

- One React component per `.tsx` file unless the frontend rule's explicit compound-component exception applies.
- All data arrives through readonly props.
- Local UI state only.
- Root element has `data-slot` when it is a reusable primitive or stable styling/test hook.
- Use `Typography` for text and design-system primitives for controls.

## UI Primitives

- Put primitives in `src/web/components/ui/`.
- Root element has `data-slot`.
- Add `data-variant` and `data-size` when those props exist.
- Use existing tokens from `src/web/index.css`; do not invent arbitrary Tailwind values.

## After Creating

1. Add barrel exports where the directory expects them.
2. Run `cd src/web && pnpm check` after substantive frontend changes.
3. If Rust command bindings are involved too, run `make check` instead.
