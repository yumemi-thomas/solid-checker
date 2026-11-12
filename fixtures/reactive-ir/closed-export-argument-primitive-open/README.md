# closed-export-argument-primitive-open

Open twin of `closed-export-argument-primitive` (same files; no `package.json`, so open-world per ADR 0193 rule 4): the closed-program discharge does not run, so the `SC9012` `parameter-use-unmodeled` rows stand at `App.tsx:4`, `App.tsx:5` and `helpers.ts:17` (`record(value)` in `relay`). Stubs from `closed-export-member-dispatch`.
