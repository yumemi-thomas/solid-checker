# Dependency source: project JavaScript stays out (ADR 0284, review round 2)

Claim: the sibling tsconfig's `allowJs` admits the selected package's closure only. This project has no `include`
(TypeScript's default `**/*`) and does not set `allowJs`, so `public/sw.js` is not in its program and dependency
source mode must not pull it in (it once did, reporting SC4001 there). `App.tsx` (positive): SC1001 at
`useLabel(name)`. `solid-js.d.ts` is `context-member-identity`'s stub; `App.tsx` passes `tsc --noEmit`.
