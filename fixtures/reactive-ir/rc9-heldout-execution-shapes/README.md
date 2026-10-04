# rc9-heldout-execution-shapes

**Claim.** Five shapes from the held-out sweep (40 repositories never used to
develop the rules) do not run in the component body, so they are not proven
strict reads or owned-scope writes:

- a primitive's inline callback (`flush`) inside a component callback prop:
  the composed role needs the call itself to run during the body
  (`execution_role::inline_callback_execution_role`);
- `latest()` inside a helper that runs only after an `await`;
- a default-parameter initializer, which runs only when its function is
  called (`nested_literal_runs_during_body`);
- a function source of `merge()`, which rc.9 wraps in `createMemo`
  (`dist/dev.js:4290`, `Dialect::wraps_function_arguments_in_memo`);
- a write in a listener handed to an unknown subscriber inside a memo
  compute (`attribute_function_within` for tracked primitive callbacks).

Positives: `latest()` called directly in the body, and a default parameter of
a helper called in the body, stay proven `strict-read-untracked`.

`solid-js.d.ts` extends `fp-exec-control-flow-callback`'s rc.9-faithful stub
with `Disposable`, `onCleanup`, `flush`, `latest` and `merge` (with its helper
types) copied verbatim from `@solidjs/signals@2.0.0-rc.9` `dist/types`.
`App.tsx` passes `tsc --noEmit` against the stub and the real rc.9 install.
