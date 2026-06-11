# Frontend Bugbot rules

Applies to `src/web/**`. Authoritative rules: [web-frontend.mdc](../../../.cursor/rules/web-frontend.mdc).

## Architecture

- Hooks -> containers -> components. A component must not call a Tauri command directly; it goes through a hook.
- One component per file. Co-locate `.test.tsx` next to the component.
- Tauri command calls live in hooks under `src/web/hooks/`, never in components.

## State

- Server state through TanStack Query, never raw `useState` mirrors of remote data.
- Pending state for mutations comes from `useMutation` (`isPending`), never from local `Set<string>` of in-flight IDs.
- Optimistic updates use `onMutate` + `onError` rollback. No `setState` shadows of the cache.

## TypeScript

- No `any`. Use `unknown` and narrow.
- No type assertions except for branding, and only with a comment explaining why.
- `import type` for type-only imports.
- Use the generated bindings in `src/web/gen/tauri.ts` for command calls. Do not redeclare command argument or return types by hand.

## Styling

- Tailwind classes only. No raw CSS files outside `src/web/styles/`.
- No inline `style={{}}` props for layout or color. Use Tailwind utilities or extend the design tokens.
- Use design-token classes (`bg-surface`, `text-muted`) where they exist instead of raw color utilities.

## Accessibility

- Every interactive element has a label (`aria-label`, `<label>`, or visible text).
- Buttons that mutate state must be disabled while pending.
- Focus visible on every interactive element. Do not remove `outline` without replacing it.

## Imports

- Absolute imports from `@/` for cross-directory references; relative for siblings.
- No barrel files in feature directories. Re-export only from package roots.
