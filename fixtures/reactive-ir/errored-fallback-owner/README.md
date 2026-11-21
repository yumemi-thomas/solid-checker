# errored-fallback-owner

**Claim (ADR 1153).** `Errored` calls its `fallback` function, written in place as the whole value of the prop, in a computation of its own, created under the owner that encloses the tag
(rc.21 `@solidjs/signals` `dist/dev.js` `createBoundary`, `fallback(b)`: `runWithOwner(out._parent, () => computed(() => b._show(b), { id: out.id, lazy: 1 }))`; `solid-js`
`dist/solid.dev.js` `Errored`: `f(err, reset)`, `err` an accessor). A creation site written directly in that function, and in a callee only it reaches, has an owner whoever owns the
tag, so the rows that read the owner of such a site (SC4001, and SC3001 and SC6002 where the program holds a leaf opener or an effect's apply) end in the silence they have for a
`For` row: the owner graph gets an `Owned` edge for the function and the entry walk stops at it (`owners.control_flow_prop_function`; `SOLID_CHECKER_IR_ABLATE_SILENCE` drops both).
A function that is not that, or may run elsewhere, keeps every row. Chrome: `fixtures/runtime-oracle/errored-fallback-*` (quiet where the edge applies; NO_OWNER_CLEANUP, covered, where it does not).

| site (App.tsx) | outcome | why |
|---|---|---|
| `Message` (line 16), `Parenthesized` (25), `ViaNamespace` (70), `NamedQuiet` (79): a render effect for the dynamic child | no row | the textbook shape of `id-async-memo-loading`; parentheses and type assertions around the literal are no other function; `S.Errored` names the same exported primitive; a named function expression that never uses its name cannot hand itself on |
| `Creates` (34): `onCleanup`, `createMemo`, `createEffect` written directly in the function | no SC4001, SC3001 or SC6002 row; the SC5003 `async-provenance-unproven` at line 44 (`label()`, a read of an accessor the program does not prove) stays | the sites are owned by the fallback's computation (Chrome: quiet, `errored-fallback-owned`); another rule's row is not the edge's |
| `ViaHelper` (56) with `release` (53): a callee only the fallback calls | no row at the helper's `onCleanup` | the helper is reached only from the owned run (`errored-fallback-sync-helper`) |
| `Spread` (93, row at 95) | SC4001 `function-run-unplaced` | with a spread `props.fallback` is a merge: that this literal is what the tag calls is not proved |
| `Async` (102, rows at 107-108) | SC4001 violation at the `onCleanup` after the `await`; SC3001, SC1002, SC4001 uncertifiable | the code after the first `await` runs with no owner (Chrome NO_OWNER_CLEANUP, `errored-fallback-async`); the edge excludes async functions |
| `ByName` (118, row at 117): `fallback={show}` | SC3001, SC4001 `owner-callers-unenumerated`, SC6002, SC9012 `handed-member-dispatch` | a name is a value the tag may call or not, and `show` has other references |
| `Conditional` (127, row at 129): `fallback={flag ? (error) => … : <p/>}` | SC3001, SC4001, SC6002 | the function is not the whole value of the prop |
| `NamedSelf` (137, row at 142): a named function expression that stores itself, called by `Host`'s apply | SC3001, SC4001, SC6002 | it can hand itself on (ADR 0971, B1): it runs where it is called, with no owner |
| `Nested` (152): `createEffect` and `createTrackedEffect` written directly in the function; a `createMemo` in the effect's apply (line 159); an `onCleanup` in the tracked callback (163) | the two creations written directly in the function: no row; SC6002 violation at 159 and SC3001 violation at 163, as on main | the layers written inside the function are their own: PRIMITIVE_IN_EFFECT_CALLBACK and CLEANUP_IN_FORBIDDEN_SCOPE (Chrome `errored-fallback-nested-apply`, `-nested-tracked`, `covered-violation` before and after the edge) |
| `Deferred` (220) with `releaseLater` (217), `DeferredThen` (232) | SC4001 **violation** at the `onCleanup` the fallback hands to a timer (`setTimeout(releaseLater, 0)`, line 218) and at the one in the arrow it hands to a promise (`Promise.resolve().then(run)`, line 237); their SC3001 rows stay uncertifiable | the callback runs later with no owner (Chrome rc.21 NO_OWNER_CLEANUP, `errored-fallback-deferred`, `covered-violation`). Main kept an uncertifiable row there: the fallback's own owner was unknown, so was the owner of what it defers. The edge makes it provable, a true positive (the review of ADR 1153) |
| `UserComponent` (181, row at 185), `Shadowed` (195, row at 206) | SC4001 violation, SC3001 uncertifiable | a user component's `fallback` prop, and a local component named `Errored`, are not the dialect's primitive: their own code calls the function, here in a handler with no owner; since ADR 1171 that call is the literal's one entry (`errored-fallback-userprop`, `-shadow`: Chrome NO_OWNER_CLEANUP) |

Not covered, each a row here or in `docs/open-work.md`: a fallback handed by name or call result, a generator, the server build (`dist/server.js` calls the fallback under `runWithOwner(outputOwner)`
with a render context and directly without one; the oracle is CSR, as for ADR 0883's `For` and `Show` children), hydration, `reset`'s semantics.

`node_modules/solid-js/index.d.ts` reduces `solid-js@2.0.0-rc.21` `types/client/flow.d.ts` byte-faithfully for `Errored` (and carries the other flow components of the fixture it was copied from, unused here);
`solid-js.d.ts` has the real `RenderedElement` shape (`types/types.d.ts`: any object that is no function, so a Promise is an element and an `async` fallback type-checks).

The proof-log process test `an_errored_fallback_written_in_place_is_an_owned_run` pins the nine functions that get the edge (one `silent` line of SC4001 each, proof `owners.control_flow_prop_function`),
the helpers named at their own sites, the six that do not get it, and the ablation (the rows come back; the only rows the edge adds are the two deferred violations). The fuzz cells are `er-namedfb-apply` and `er-fbnest-apply` and their never-run twins (`er-*-apply-never`).
