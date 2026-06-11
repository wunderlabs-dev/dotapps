# Microsoft Pragmatic Rust Guidelines Map

Upstream: <https://microsoft.github.io/rust-guidelines/>

Agent combined source: <https://microsoft.github.io/rust-guidelines/agents/all.txt>

Use the upstream source for rationale and examples. Apply the spirit of each guideline, then check Opnble's local rules for stricter project policy.

## Topic Routing

- AI and agent-friendly APIs: `M-DESIGN-FOR-AI`.
- Application errors and allocators: `M-APP-ERROR`, `M-MIMALLOC-APPS`.
- Documentation: `M-CANONICAL-DOCS`, `M-FIRST-DOC-SENTENCE`, `M-MODULE-DOCS`, `M-DOC-INLINE`.
- FFI and dynamic libraries: `M-ISOLATE-DLL-STATE`.
- Performance: `M-HOTPATH`, `M-THROUGHPUT`, `M-YIELD-POINTS`.
- Safety: `M-UNSAFE`, `M-UNSAFE-IMPLIES-UB`, `M-UNSOUND`.
- Universal style: `M-CONCISE-NAMES`, `M-DOCUMENTED-MAGIC`, `M-LINT-OVERRIDE-EXPECT`, `M-LOG-STRUCTURED`, `M-PANIC-IS-STOP`, `M-PANIC-ON-BUG`, `M-PUBLIC-DEBUG`, `M-PUBLIC-DISPLAY`, `M-REGULAR-FN`, `M-SMALLER-CRATES`, `M-STATIC-VERIFICATION`, `M-UPSTREAM-GUIDELINES`.
- Library building: `M-FEATURES-ADDITIVE`, `M-OOBE`, `M-SYS-CRATES`.
- Library interoperability: `M-DONT-LEAK-TYPES`, `M-ESCAPE-HATCHES`, `M-TYPES-SEND`.
- Library resilience and UX: avoid hidden statics, keep I/O mockable, design clear error types, and provide escape hatches only where needed.

## Opnble Adaptation Notes

- Treat Opnble as an application plus internal crates, not a public library ecosystem.
- Prefer `AppError` and crate-local structured errors over `anyhow` or `eyre`.
- Do not add global allocator changes without profiling evidence.
- Do not add compliance comments.
- Prefer concise docs on internal APIs; reserve full canonical sections for public reusable APIs, fallible public APIs, and any future unsafe boundary.
