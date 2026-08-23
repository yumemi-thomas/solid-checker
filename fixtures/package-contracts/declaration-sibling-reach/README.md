# A sibling `.d.ts` splits the helper's identity, so the reach enumeration says so

**The trap this fixture exists for: `channel.d.ts` must stay beside
`channel.js`.** Delete it and the fixture silently stops testing anything — the
call graph resolves the edge exactly again and `.` narrows back to `forwarded`
alone, which is what `parameter-member-forwarded` already pins.

`index.js` writes `import { channelFor } from "./channel.js"`. TypeScript
resolves that specifier to `channel.d.ts`, not to `channel.js`: a declaration
file wins over an adjacent implementation in every resolution mode the
generator uses. The two files are unrelated modules to the compiler, so the
call in `forwarded` carries the *declaration's* runtime identity and the
implementation's symbol has no reference outside `channel.js` at all.

Everything downstream of that identity split fails in the same direction:

- `function_call_sites` finds no caller for `channel.js`'s `channelFor`, so the
  call graph enumerates the helper alone;
- `entered_only_through_calls` walks the same symbol's references, finds only
  the declaration and the export specifier, and reports the enumeration
  **complete**;
- the reachability rung then resolved the obligation to *no export*, and both
  `forwarded` — which really does reach it — and `Isolated` were published
  certified.

Since a published package almost always ships a `.d.ts` beside each runtime
module, that silent certification was the normal case, not the exotic one.

There is no fact that pairs a declaration file with the runtime module it
describes. `ImportFact` carries only the specifier text, and the compiler holds
no link between the two files, so the edge cannot be recovered exactly. The
enumeration therefore reports itself **incomplete** instead of dropping
members: a reaching function that is decided *not* an export of this
entrypoint, is published by its own module, and has no reference anywhere else
in the project, cannot have had its entry set enumerated
(`module_surface_is_unaccounted` in
rust/crates/solid-facts-backend/src/main.rs). Attribution widens to
`fallback-all` and the marker records the widening.

- `.` — `forwarded` and `Isolated` both go unknown. `Isolated` reaches nothing
  and is over-marked; that is the honest cost of the widening, recorded in
  docs/precision-backlog.md. The direction that matters is that `forwarded` is
  never certified.
- `./direct` — the control, and the half that stays exact. Its entry file *is*
  `channel.js`, so `channelFor` resolves to an export name and the module
  surface question never arises: the export publishes its exact
  `parameter-member` row and carries no unknown claim, exactly as in
  `parameter-member-forwarded`. A regression that widened unconditionally would
  break here.
