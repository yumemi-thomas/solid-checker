# B. `@solidjs/web` premises and the compiler helper interface, rc.13 → rc.17

Bytes: the npm installs in `/tmp/sc-rc17-audit/inst13` and `inst17`, whose integrities are
checked in § 1. [R] means read in the bytes. [M] means measured by
`scripts/forkB/probe-web.mjs`, which runs the dev client builds under happy-dom 15.11.7 in
Node with `--conditions=development --conditions=browser`, or by `tsc` 5.9.3 against the
published typings. Every rc.13 citation below was checked again on the inst13 bytes, and
each one matched.

## B.1 Fact table

| # | Premise (checker site) | rc.13 citation (verified) | rc.17 citation | Verdict |
|---|---|---|---|---|
| W1 | ADR 0353: `render` calls `code()` inside `createRoot` (`render_creates_root`) | `web.dev.js:488-495` | `web.dev.js:542` `render`, `:551` `createRoot(dispose => {`, `:555`/`:558` `const tree = code();`. The same in `web.js:397/401` and `web.observe.js:408/412` | **Holds** [R][M]. Probe: the code ran during the call, `getOwner()` was a root, and a signal setter in `code` threw `REACTIVE_WRITE_IN_OWNED_SCOPE` on both releases. |
| W2 | ADR 0353: `hydrate` reaches `render` on every path | `web.dev.js:1521-1620` | `web.dev.js:1626-1735`, `web.js:1383-1478`, `web.observe.js:1434-1529` | **Holds, with a new timing path** [R]. Every path still ends in `render(code, …)`. rc.17 adds an early branch: `if (hy.s && !hy.s[id] && !hy.s[""] && !hy.done && document.readyState === "loading") { … const start = () => started \|\| (started = true, disposer = hydrate(code, element, options)); … document.addEventListener("DOMContentLoaded", start, { once: true }); return () => {…}; }` (`web.dev.js:1631-1645`). On that path `code` runs after `hydrate` returns, from a `DOMContentLoaded` listener or a `queueMicrotask` (`hy.sw`), not during the call. The root claim (W1) still holds there. The `callback_executions` row `Render \| Hydrate => (0, Inline)` ("invoke the code callback once, immediately") is **no longer true for `hydrate` on rc.17**: the claim needs to move to `render` only, or `hydrate` needs to be left unanswered, before rc.17 can be audited. The asset-preload path (`p.then(() => … render(code, …))`, `:1712-1733`) was already deferred on rc.13. |
| W3 | B2: `dynamic(source, { static: true })` is `staticDynamic(untrack(source))`, a truthiness test, no memo, the caller's owner (`call_form`, `callback_owners` `DynamicStatic => Inherits`, `Inline`) | rc.13 `web.dev.js:2296-2297` (`function dynamic(source, options) { if (options?.static) return staticDynamic(untrack(source));`). The rc.9 lines the code cites are rc.9's own. | **Moved to `solid-js`.** In every web build `dynamic` is now `function dynamic(source, options) { return dynamicCore(source, options, staticElement); }` (`web.dev.js:2466-2467`, `web.js:2154`, `web.observe.js:2205`; the server builds pass `ssrTag`: `server.js:4435`, `server.dev.js:4778`, `server.observe.js:4590`). `dynamicCore` is imported from `solid-js/internal` (`web.dev.js:3`). The test is in `solid-js`: `if (options?.static) return staticDynamic(untrack(source), tagArm);` (`solid.dev.js:1445`, `solid.js:1383`, `solid.observe.js:1418`), and `if (options?.static) { const component = untrack(source); …` (`server.js:2375`, `server.dev.js:2587`, `server.observe.js:2480`). | **Behaviour holds** [R][M]: the probe ran the source once, under the caller's owner, and a later write did not re-run it. **The owner changed**: the answer now comes from `solid-js`'s `dynamicCore`, with `@solidjs/web` supplying only the string arm. `releases.rs` takes `dynamic_options` from `@solidjs/web` alone. rc.17 web's import of `dynamicCore` needs `solid-js` that exports it (rc.13 `solid-js/internal` does not), so a mixed rc.17-web/rc.13-solid-js install fails to link. B2 should be keyed on the `solid-js` release, or on both, and the doc comments on B2, `call_form`, `callback_owners`, `callback_execution_word_at` and `runs_callback_deferred` should cite the `solid-js` lines above. |
| W4 | `static-dynamic-async-source` (SC rule) and `static_api.rs:255-258`: the dev builds throw on a thenable static source, and prod falls through to `() => undefined` | rc.13 `web.dev.js:2381`, `server.dev.js:4698` (the rc.9 cites `:2249`/`:3979` are rc.9's) | `solid.dev.js:1530` and `server.dev.js:2589`: `throw new Error("dynamic(): a static source must resolve synchronously, not to a promise")`. Prod: `solid.js` `staticDynamic` has no throw and ends `return () => undefined;`; the `server.js:2375-2380` and `server.observe.js` static branches have no throw and also end `return () => undefined;` | **Holds, moved to `solid-js`** [R]. Rule page and comment citations need to move to `solid-js`. |
| W5 | ADR 0313 / `intrinsic_tag_renderers`: `Dynamic(props)` = `createComponent(dynamic(() => props.component), omit(props, "component"))`, and a string becomes `staticElement`, which calls `spread(el, props)` | `web.dev.js:2403`, `:2375`, `:2396-2399`; `web.js:2137` | `Dynamic` `web.dev.js:2476-2479`, `web.js:2164`, `web.observe.js:2215`; `staticElement` `web.dev.js:2469`, `spread(el, props)` `:2472` (`web.js:2157/2160`, `web.observe.js:2208/2211`). The string arm is in `solid-js`: `case "string": return tagArm ? tagArm(component, props) : undefined;` (`solid.dev.js:1524`, `solid.js:1459`, `solid.observe.js:1494`), with the static form at `:1542`/`:1473`/`:1508`. Server `Dynamic`: `server.js:4438`, `server.dev.js:4781`, `server.observe.js:4593` | **Holds** [R][M]. The probe showed `<Dynamic component="button" onClick>` installing a listener on rc.17. |
| W6 | ADR 0358 / `spread_key_is_event`: an un-namespaced `on…` spread key, **lowercase included**, installs a listener (`key.starts_with("on") && !key.contains(':')`) | `web.dev.js:1803` `if (!hasNamespace && prop.slice(0, 2) === "on") {` (`web.js:1578`, `web.observe.js:1626`) | `web.dev.js:1943` `if (!hasNamespace && prop.startsWith("on") && (c = prop.charCodeAt(2)) > 64 && c < 91) {` (`web.js:1643`, `web.observe.js:1694`, server `isEventKey` `server.js:65`) | **CHANGED** [R][M]. rc.17 makes a key an event only if its third character is `A`-`Z`. Lowercase `onclick` goes to `setAttribute`: the function's source text becomes the attribute, with a dev warning `LOWERCASE_EVENT_ATTRIBUTE` ("`onclick` received a function, but `onclick` is an attribute in Solid 2.0, not an event handler", `lowercaseEventAttribute`, `web.dev.js` `setAttribute`). Probe: spread `{ onclick }` clicked 1 time on rc.13, 0 times on rc.17 (attribute `() => calls++`); `{ onClick }` 1 on both; `on:click` 0 on both (attribute); `onClickCapture` 0 on both. `tsc` over the rc.17 typings now **accepts** `<div {...{ onclick: () => {} }} />` (rc.13: TS2559), so this is code that type-checks. Required: `spread_key_is_event` on rc.17 must answer `key.len() > 2 && key.starts_with("on") && key.as_bytes()[2].is_ascii_uppercase() && !key.contains(':')`. That needs a release-aware grammar, not the current boolean `spread_event_keys_audited`, which is `true` only for web rc.13. Keeping rc.13's grammar on rc.17 would prove listener and owner edges for lowercase keys that never fire, which is a false positive. The `missing-owner.md` text ("including lowercase") needs a release qualifier. |
| W7 | `ChildProperties` lists `innerText`, so the property is set (jsx-no-duplicate-props, `upstream_compat/syntax.rs:162-191`) | `web.js:33` `new Set(["innerHTML", "textContent", "innerText", "children"])` | Unchanged at `web.js:33`, `web.dev.js:33`, `web.observe.js:33`, `server*.js:36`. The assignProp property branch still writes `node[prop] = value` (`web.dev.js:1954-1960`) and now also wraps it in `checkStatefulWrite`/`recordStatefulWrite` (dev bookkeeping only). The hydration skip `else if (isHydrating(node)) return value;` is the same as on rc.13 | **Holds** [R]. |
| W8 | Typings: only `innerHTML` is typed, and `innerhtml` is TS2322 | rc.13 `types/jsx.d.ts:900` `innerHTML?: string;` | `types/jsx.d.ts:906` `innerHTML?: string;`, `:907` `textContent`, `:959` `innerText` | **Holds** [M]: `tsc` reports TS2322 for `<div innerhtml="x"/>` on both, and TS2322 for `<div onclick>` on both. |
| W9 | Host constant fold: `isServer` is a module-level `const` with one literal in every target (`HostBooleanExport { version: "2.0.0-rc.13", … }` in `solid_2.rs:234-250`, `host_constants.rs`) | (rc.9/rc.13 per the code) | `const isServer = false;` `web.js:2100`, `web.dev.js:2412`, `web.observe.js:2151`; `const isServer = true;` `server.js:4432`, `server.dev.js:4775`, `server.observe.js:4587`, each exported. `exports["."]` is byte-identical to rc.13, and there is no top-level `browser` field | **Holds** [R]. **Bookkeeping**: `inferred_host/execution.rs` requires `row.package_version == candidate_export.version`, so on rc.17 the fold is skipped and precision is lost (no false claim). Add an rc.17 entry, or make `version` a list. |
| W10 | `httpStatus`/`httpHeader` retract on `!response.committed` (`server_rules.rs:38-39`, `http-response-after-flush.md`) | rc.13 `server.js:4398`/`:4432` | `server.js:4490`/`:4524`. The 73-line span is byte-identical to rc.13 | **Holds** [R]. |
| W11 | `clientOnly` (callback owner `Conditional`) and `useHead` | rc.13 `web.dev.js` | `clientOnly` body identical (only the trailing export list differs); `useHead` identical (31 lines compared) | **Holds** [R]. |
| W12 | `createComponent` re-exported unchanged from `solid-js` (`renders_component_argument` cites `web.js:2`) | rc.13 `web.dev.js:2` | `web.dev.js:2` `export { …, createComponent, dynamicComponent, getOwner, merge as mergeProps, untrack } from 'solid-js';` (all six builds) | **Holds** [R]. **New**: `dynamicComponent` (= `solid-js` `dynamicCore`, `solid.dev.js:1545` `const dynamicComponent = dynamicCore;`) is re-exported from both packages. |
| W13 | Delegated event dispatch: the handler runs with no owner (fresh stack); ADR 0356's synchronous `dispatchEvent`/`click` veto | `eventHandler` rc.13 `web.dev.js` | `eventHandler` `web.dev.js:1979-…` | **Holds, with one narrowing** [R]. `handleNode` still calls `handler.call(node, e)` without `runWithOwner`. Changed: rc.13 ran delegated handlers with no delegated root (`state && !owner` / `e[$$EVENT_OWNER] = owner \|\| true`); rc.17 returns early (`if (!owner) return;`) when no registered delegated root owns the target. So a delegated handler outside every `render`/`hydrate` root no longer runs. This makes fewer handlers run; no checker claim was found that depends on it running. Unverified: whether any SC4001/SC1001 proof assumes a delegated handler outside a render root executes. |

## B.2 New exports (callback-taking or relevant)

| Export | rc.17 behaviour (read) | Note |
|---|---|---|
| `dispatchAsInteraction(e, fn)` | prod `web.js:484` `return fn();`, `server.js:4423` the same. dev/observe `web.dev.js:653` / `web.observe.js:510` `OBSERVE.attribution.withInteraction({...}, fn)`, which runs `fn()` inline (`@solidjs/signals` `dev-shared.js:335-347`, assuming that is the function `OBSERVE.attribution` exposes: **unverified**) | Calls `fn` synchronously in the caller's owner and listener. Not in `TABLE`, so it is unmodelled, and the callback-taking-export completeness rule needs a modelled-or-excluded decision. |
| `dynamicComponent(source, options?)` (from `solid-js`, re-exported by web) | `dynamicCore` without `tagArm`: the same function and static forms as `dynamic`, and a string renders nothing | Not in `TABLE`. Its source has `dynamic`'s semantics. It is unmodelled, which states nothing (no false claim), but it is a precision gap and a completeness-test item. |
| `setLinkClaim(handler)` | client `web.dev.js:534` returns `false`, ignoring the argument. server `server.js:4125` stores the handler on the render context, and `ssrLinkClaim` (`server.js:3508-3512`) later calls `ctx.linkClaim(attrs)` while serializing an element with `href` | The callback is retained and invoked later during SSR serialization; unmodelled. |
| `broadcast(fn)` (server-functions) | client `client.js:1073` `return live(fn);`, server `server.js:1742` `live(fn)` plus a metadata flag | Same shape as `live`; `fn` is a server function reference. |
| `setStatefulProperty(node, name, value)`, `hasServerLinkState()`, `installServerComponentHydration()`, `ssrLinkClaim(attrs)`, `ssrStyleProperties(...entries)`, `reportRequestFailure` | no callback argument (client stubs are no-ops) | Need no vocabulary row; they are new compiler helpers (§ B.3). |
| `registerElementClaim(handler, options?)`, `claimElement(node, owner?)` | `claimElement(node, owner)`: when `owner !== undefined`, it runs the claim handlers in `runWithOwner(owner, …)`, and it now runs them once per node (`node._$claimed`) | Changed signature, additive. The handlers run under a supplied owner when the compiler passes one. |

## B.3 Compiler fork helper interface (pinned `3ad4bbe`, rc.13-era)

Helpers the fork can emit (from `packages/compiler/src`, `_$` aliases, plus `effect`, `memo` and
the built-ins `For Show Switch Match Loading Reveal Portal Repeat Dynamic Errored`) were
checked against the ESM export lists of all six rc.17 web bundles:

- Client bundles (`web*.js`): every client helper is exported, as on rc.13. The same three
  names are missing on both releases, all SSR-only: `sharedConfig`, `ssrClaim`,
  `ssrSelectValues`.
- **Server bundles (`server*.js`): rc.17 removed `ssrClaim` and `sharedConfig`** (and also
  `CLAIMS_DOCUMENT`, `CLAIMS_STREAM`, `SLOT_*`, `isSlotValue`, `slotValue`, `scriptNonce`,
  `styleNonce`) [R]. The fork emits both together whenever an SSR element has a `ref` or an
  `on*` attribute (`ssr/transform.rs:2243-2259` collects the claims, `:572-575` imports
  `ssrClaim` and `sharedConfig`, and `:3361-3375` emits
  `_$sharedConfig.context && _$sharedConfig.context.claims ? _$ssrClaim({...}) : ""`).
  Against rc.17, such output imports bindings `@solidjs/web`'s server entry does not export,
  so the ESM module fails to link. rc.13 `server.js:3394` `function ssrClaim(map)` existed.
  So **SSR compiler facts for `ref`/`on*` elements describe a lowering the rc.17 runtime
  cannot load**. The "rc.17 can be audited on the current compiler pin" conclusion of the
  rc.13 review (§ 4) **does not carry over to SSR**.
- The published compiler confirms the drift. `@solidjs/compiler` `next` is `2.0.0-rc.17`
  (2026-10-22). In its native binary (`@solidjs/compiler-darwin-arm64`, `npm pack`), rc.13
  contains the strings `ssrClaim` and `sharedConfig`, and rc.17 contains neither. rc.17
  contains `ssrLinkClaim`, `ssrStyleProperties` and `setStatefulProperty`, which rc.13 does
  not [M, string presence only: the compiler was not executed, so whether and when each is
  emitted is **unverified**]. These three are new rc.17 helpers the pinned fork does not
  know, so a consumer compiling with the rc.17 compiler produces calls (`setStatefulProperty`
  for stateful DOM properties, `ssrLinkClaim`, `ssrStyleProperties`) that the checker's
  compiler facts never describe.
- Client helper signatures (additive, compatible with the fork's positional calls) [R, `.d.ts` diff]:
  - `createComponent(Comp, props, name?, loc?)`, `spread(…, name?, loc?)`,
    `insert(…, options?: { …, loc? })` and `effect(fn, effectFn, options?: { scope?, name?, loc? })`
    each gained a trailing `loc`.
  - `claimElement(node, owner?)` gained `owner` (see § B.2).
  - `registerElementClaim(handler, options?)` gained `options`.
  - No helper the fork uses lost a parameter.

## B.4 Required checker changes (web side)

1. `spread_key_is_event` needs an rc.17 grammar: `on` followed by an ASCII uppercase letter,
   no colon. Do not set `spread_event_keys_audited` for rc.17 under the rc.13 grammar
   (W6, measured).
2. `callback_executions`: `Hydrate`'s `(0, Inline)` is false on rc.17's deferred
   `DOMContentLoaded` path. Answer it only for rc.13, or drop it (W2).
3. B2 (`dynamic_options`) and the static-dynamic-async premise now live in `solid-js`
   `dynamicCore`/`staticDynamic`. Re-key the answer on `solid-js` (or on `solid-js` and web
   together), and move citations in `solid_2.rs`, `releases.rs`, `static_api.rs`,
   `rules.rs:163` and `docs/rules/static-dynamic-async-source.md` (W3, W4).
4. `HostBooleanExport` `version: "2.0.0-rc.13"`: add rc.17 (same six targets and values) (W9).
5. Vocabulary/completeness: `dynamicComponent` (dynamic semantics) and `dispatchAsInteraction`
   (synchronous `fn`) need modelled-or-excluded decisions. `setLinkClaim` retains a callback
   the server invokes during SSR.
6. Compiler: rebase the semantic-facts fork onto the rc.17 compiler before claiming SSR facts
   for `ref`/`on*` elements, or for `setStatefulProperty`/`ssrLinkClaim`/`ssrStyleProperties`
   output, on rc.17 (§ B.3). Until then the rc.13 pin's SSR claim lowering does not match
   any rc.17 runtime.
7. `modules()`: rc.17 adds the subpaths `@solidjs/web/frames/{assets,regions,trace,wire}`
   (and `solid-js/internal/container-trace`). They are not owned today. Reported for
   completeness; vocabulary impact not assessed here.

## B.5 Not verified

- The happy-dom probe is not Chrome. Delegated `click` dispatch worked, but `innerText`
  replacement semantics were not distinguished by the probe (W7 rests on reading).
- No SSR or hydration execution. W2's deferred path was read, not run.
- Which helpers the rc.17 native compiler emits, and when (string presence only).
- `OBSERVE.attribution.withInteraction` identity in dev (B.2).

## B.6 Addendum: the uncommitted rc.17 compiler pin (`6c1983c`)

The worktree holds uncommitted changes, made by someone else, that move the pin to fork `6c1983c264394c892aafb5277f8afc125376101b` ("Bind RC.17 semantic trace to its implementation revision"). It is checked out read-only at `~/.cargo/git/checkouts/solid-6c05d7779c3c2780/6c1983c`. Diff of its helper imports (`import_named`/`import_wrapper_helper`) against the committed `3ad4bbe` [R]:

- removed: `sharedConfig`, `ssrClaim`
- added: `setStatefulProperty` (`dom/template.rs:258`), `ssrStyleProperties` (`ssr/transform.rs:582`), `ssrLinkClaim` (`ssr/transform.rs:603`)

Every helper the `6c1983c` pin imports is exported by every rc.17 bundle of its mode. The only gaps are mode-specific and are the same as on rc.13: `ssrSelectValues` is SSR-only and missing from the `web*.js` bundles; `getFirstChild` and `getNextSibling` are DOM-only and missing from the `server*.js` bundles. So **the `6c1983c` pin is compatible with the rc.17 runtime helper interface**, and the § B.3 incompatibility applies only to the committed `3ad4bbe` pin. B.4 item 6 becomes "land the `6c1983c` pin together with the rc.17 runtime move". Its trace, conformance and finding deltas were not reviewed here.
