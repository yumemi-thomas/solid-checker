# Dependency source: the analyzed program (ADR 0284, round 2)

Dependency source mode analyzes exactly the project files the project's own program holds plus the selected package's closure, binding only imports that resolve into the selected installed copy: `App.tsx` is SC1001 at `useLabel(name)`; `e2e/Harness.tsx` lies outside the `include` inherited through `extends` and is never analyzed; `legacy/Legacy.tsx` imports another installed copy (`legacy/node_modules/dep-hooks` 2.0.0, reading nothing): SC9011 in both modes. Stub `context-member-identity`'s (published rc.13 signatures).
