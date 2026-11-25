# effect-creation-host-entry

**Claim (ADR 1213).** rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK only while an effect's apply runs synchronously (`setEffectCallback(true)` around the
apply's call, cleared in its `finally`: `@solidjs/signals@2.0.0-rc.21` `dist/dev.js:2772-2801`). A creation reached only from a host entry, through layers
that run in place on that entry's stack, never meets the flag: SC6002's site ends in `effect-creation-runs-off-apply`
(`owner_emission::runs_on_host_entry`). The entries are an element's inline event handler literal the host delivers (ADR 1159's, where no apply may
dispatch an event) and a callback a reviewed scheduler runs on a fresh stack (`setTimeout(fn)`, `queueMicrotask(fn)`). The layers between are those
`layer_run` places in place (an IIFE, a `const` function whose one run is a direct call there, an inline standard-library callback) and a callback a
primitive's slot runs in place (`untrack(fn)`).

| component | outcome | Chrome |
|---|---|---|
| `HandlerNamed` (a local `const` the handler calls), `HandlerUntrack`, `HandlerTimer`, `HandlerMicro` | silent (main: uncertifiable `effect-creation-unaccounted`) | quiet |
| `HandlerExported` (an exported helper: an unseen importer may call it from an apply) | `effect-creation-unaccounted`, as before | quiet |
| `variants/apply` `ApplyTimer` (`globalThis.setTimeout` in an apply) | silent | quiet |
| `variants/apply` `ApplyUntrack`, `ApplyNamed` (in place on the apply's stack), `ApplyShadowed` (a module `setTimeout` that runs its argument at once), `ApplyMember` (an object's `setTimeout` member that does) | SC6002 violation | PRIMITIVE_IN_EFFECT_CALLBACK |

Chrome: `fixtures/runtime-oracle/effect-creation-host-entry` (the handlers, clicked) and `effect-creation-host-entry-apply-*` (one apply each: the first
throw halts the reactive system). Stubs: `owner-call-effect-creation/variants/inline-handler`'s reduced `solid-js`, `@solidjs/signals` and `@solidjs/web`
declarations, with `untrack` re-exported from `solid-js` as rc.21's `types/index.d.ts` does.
