# closed-export-member-dispatch-open

Open twin of `closed-export-member-dispatch` (same `App.tsx`/`helpers.tsx`; no `package.json`, so open-world per ADR 0193 rule 4): every exported helper keeps its declaration obligation, so `describe` (entered only through calls) has `SC9012` at `helpers.tsx:5` where the closed twin has none; `escapes`, `Card`, `describe(props.item)` keep `SC9012` in both; `describe({ label() {...} })` clean in both. Stubs from `feedback-tiers`.
