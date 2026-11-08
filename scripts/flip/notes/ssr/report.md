# Premise audit: SSR, hydration, server functions, test runners, production

Code: `claude/flip-reads` at `2a9ec9310`. Runtime: Solid rc.21 and `@solidjs/vite-plugin` next.51. Nothing in any repo changed.

## How it was measured

About 310 programs. Nothing ran through `cargo`, `make`, grids or `sc-heavy`.

| Host | File | What it is |
|---|---|---|
| Server | `h/srv.mjs`, `work/srv-one.mjs` | The pinned compiler's `generate: "ssr"`, real `server.dev.js`, a fresh Node process per program, `renderToStream` in a request event |
| Fuzz SSR host | `scripts/fuzz/ssr.mjs`, output in `ssrhost/` | Run once: 22 programs, 0 false positives and 0 false cleans by its own judge. It only judges SC7005 |
| Hydration | `h/hyd.mjs` | Real server HTML, plugin `ssr: true` client compile, `hydrate()` in real Chrome |
| Streaming | `h/str.mjs` | Chunks reach Chrome while `hydrate` runs; chunks measured at 14 ms and 213 ms |
| Server functions | `h/sf.mjs`, `h/sfssr.mjs` | The plugin's directive pass, a real `/_server` endpoint in Vite's SSR environment, real `fetch` from Chrome. `sfssr` also renders in process and hydrates |
| Production | `h/prod.mjs` | A real `vite build`, then Chrome |
| Test runners | `vt/`, `bun1/` | vitest 4.1.11, jsdom 25, `@solidjs/testing-library` 1.0.0-rc.2 (solid-groove's install, read-only, cache in `/tmp`); bun 1.4.4 |

- **Checker:** `/tmp/sc-scratch.noindex/flip-reads/r1004`, the newest flip build. I did not record its exact commit.
- **Not a flip regression:** 114 of the witnesses also ran on `premise-audit/bin` (`5b8a1d7b5`), with no difference in finding code or kind.
- **`tsc`:** every program named as a defect below passed `tsc --noEmit`. Earlier batches imported `type JSX` from `solid-js`, which `tsc` rejects, so the witnesses were stripped and rerun in `wit/`. `b7/d03` and `b5/co-pending` failed `tsc` and are not evidence.

## 1. Inventory

Status counts: **15 broken, 22 held, 7 not tested.**

| # | Premise | Where | Rests on it | What would break it | Status |
|---|---|---|---|---|---|
| 1 | Under server selection "an effect's apply never runs" | `lib.rs:289`, `static_api.rs:397,439,517` | SC2008, SC6002 | `createRenderEffect` | **broken**: `serverEffect` runs `effectFn` (s16, h33). `flush` being empty held |
| 2 | Server selection affects only client-effect rules | `selects_server_build` has one caller | all client rules | Any client claim on the server build | **broken**: D2 |
| 3 | Node selection may drop client rules | `static_api.rs:40` | SC2008, SC6002 | A hydrated project | **broken**: D13 |
| 4 | No selection means client | `lib.rs:292` | every rule | A project with only server entries | **broken**: D2, `s-nohost` |
| 5 | A project file is analysed as written | `fixtures/reactive-ir/host-dead-return/factories.ts`, ADR 0284 | every rule | `if (isServer)` | **broken**: D1 |
| 6 | Reach is decided from literals | ADR 0458, `ast/unreachable.rs` | every rule | Build constants, `typeof window`, `isDev`, `NODE_ENV` | **broken**: D1 |
| 7 | `NoHydration` children are ordinary children | no row in the dialect | SC1001, SC2001 | Client `if (isHydrating()) return undefined` | **broken**: D3 |
| 8 | Every hydrated body is under the strict window | SC1001 | SC1001 | Hydration | **held**: h31, h32, s02 warn; a pending body read is the exception, row 9 |
| 9 | A pending read in a body throws on the client | SC5001 | SC5001 | Serialized value; lazy resume | **broken for lazy** (D7). Own source throws (h01, h29, l03). An outer source resumes and warns (h02, l02), and the checker is `u` there |
| 10 | The write guard is the same under hydration | SC2001 | SC2001 | Hydration | **held**: s01, h33, co-write throw |
| 11 | A handler has no owner and no window, replay included | SC1001, SC2001 | handlers | Replay on `hydrate`'s stack | **held**: replay is a microtask with owner `null` (h08, h34, h37, l07) |
| 12 | An apply in a resumed boundary has owner `null` | ADR 0613 | writes in applies | `servedPass` | **held**: l04 |
| 13 | `onSettled` keeps its leaf owner during hydration | SC3001 | leaf rules | `isHydrating()` is true | **held**: l05 |
| 14 | `onHydrationEnd` callback owner | `solid_2.rs:1396` | SC2001, SC1001 | Boundary drain; server | **held**: `null` in h13, h14, l06. The server also runs it, on a microtask |
| 15 | Hydration mismatch changes no proof | none | handlers | A handler never attached | **held**: h25, h26. No proof claims it attaches |
| 16 | `hydrate` code timing unstated | `hydrate_code_inline` | SC2001 | Deferred hydrate | **held**: `u` |
| 17 | `ssrSource: "client"` hole | SC5003, ADR 0430 | SC5003 | Read outside a boundary | **held**: h19, h22 throw; h20, h21 quiet. Recall note in section 3 |
| 18 | SC7005 stays uncertifiable | `server_rules.rs` | SC7005 | Outer `deferStream`, unboundaried async, a slow fallback | **held**: 11 programs plus the host. Status 410 where it applies, always `u`. ADR 0431's "becomes a violation" is out of date |
| 19 | `Errored` on the server | none | none | Streamed throw, rejection | **held**: h27, h28, s35, l10, l11 |
| 20 | Out-of-order `Loading` | none | none | Stream order | **held**: l09 |
| 21 | SC7006: non-function export fails boot | `valid_use_server.rs` | SC7006 | Real boundary | **held**: k1 returns 500 |
| 22 | SC7006 is complete on compile errors | rule page | SC7006 | Closure capture | **broken**: D12 |
| 23 | Importing `rich-args` is the opt-in | `server_rules.rs:913`, rule page | SC7007 | Import without a call | **broken**: D10 |
| 24 | A finite numeric literal is JSON-safe | `server_rules.rs:547` | SC7007 | `-0` | **broken**: D11 |
| 25 | An `undefined`-only domain always throws | rule page | SC7007 | Leading arg with a trailing body | **broken**: D4 |
| 26 | Only `"use server"` scopes are in-process | rule page | SC7007 | SSR render; hydration `subFetch` | **broken**: D5 |
| 27 | Proven server calls are all the calls | `server_rules.rs:301` | SC7007 | Namespace, alias, `.call`, `.bind` and others | **broken**: D9 |
| 28 | Rich values reject, binary and `FormData` arguments pass | rule page, rc.0 probes | SC7007 | rc.21 | **held**: `Date`, `bigint`, `symbol`, `NaN`, a function, a stream, `DataView`, `Request` reject (`Map`, `Set`, `RegExp` not rerun); `FormData`, `Blob`, `File`, `URLSearchParams`, `ArrayBuffer` pass, and those five are `u` |
| 29 | A test callback has no owner and no window | ADR 0438 | test files | vitest 4.1.11 | **held**: t01, t02, t19-t23, and `it` inside `createRoot` (t26) |
| 30 | Dispatch in a test: stack-sensitive | ADR 0438 | SC2001 | `fireEvent` under `createRoot` or `runWithOwner` | **held**: t10-t12 throw and the checker is `u` |
| 31 | `setTimeout` is a fresh stack | fresh-stack schedulers | every timer proof | Fake timers | **broken**: D14 |
| 32 | jsdom matches the premises | ADR 0438 | test files | jsdom | **held** for what ran here. happy-dom was not run with the testing library |
| 33 | Client build in tests | none | test files | Default bun, or plugin deps left unconfigured | **held, recorded**: default `bun test` gives `isServer` true; the first vitest config failed with "Client-only API" |
| 34 | `lazy` body has a window (CSR, SSR through the plugin) | SC1001, SC2001 | lazy | Real `import()` | **held**: lz-read, lz-write, ls-* |
| 35 | Production only removes dev checks | rule pages | every rule | Real build | **held in code**: findings identical; "in dev" holds in messages. Page gaps in section 3 |
| 36 | Hydratable compile output changes nothing | none | every rule | `ssr: true` | **held**: every hydration run |
| 37 | `hydrate` in an apply or `onSettled` | open-work:148 | SC2001 | Nested hydrate | **not tested**: needs a sub-root with a `renderId` |
| 38 | `clientOnly` mounted after load, in hydration | SC1001, SC5001 | as listed | Real `import()` | **held**: cs-read warns, cs-pending throws. Before load is already in `open-defects.json` |
| 39 | Concurrent requests | `currentOwner` and `sharedConfig.context` are module globals on the server | server rules | Two overlapping renders | **not tested** |
| 40 | Server components, `createLiveHoles`, `enableServerComponentHydration` | none | none | Needs `serverFunctions.components` | **not tested** |
| 41 | `deno` and `worker` conditions | `selects_server_build` | as row 1 | Those runtimes | **not tested** |
| 42 | jest, mocha, `node:test`, playwright, bun with JSX; `vi.useFakeTimers` on `queueMicrotask` | ADR 0438 | test files | Each runner | **not tested** |
| 43 | Streaming with a client abort, `SSR_STREAM_ABANDONED` | none | none | Disconnect | **not tested** |
| 44 | `renderToString` with a pending source under `Loading` | SC5003 string mode | SC5003 | String mode freezes the fallback | **not tested against a rule**: s15 stays on the fallback in string mode, and no rule claims it |

## 2. Defects

Findings are written `code:kind`. `v` means violation, `u` means uncertifiable.

### False positives

| # | Rule | Program | Runtime | Checker |
|---|---|---|---|---|
| D1 | SC1001, SC2003, SC5001, SC2002, SC2001, SC7007 | an arm only the server or production runs | ran quietly, or never ran | `v` |
| D2 | SC1001, SC2003, SC5001, SC2002, SC2001 | a server-only project | quiet on the server build | `v` |
| D3 | SC1001, SC2001 | `<NoHydration>` child | ran on the server, never on the client | `v` |
| D4 | SC7007 | `f(undefined, bytes)` | resolves | `v` |
| D5 | SC7007 | a server function called in a memo | in process, quiet in hydration | `v` |
| D6 | SC1001 | `render(Local, el)` | silent | `v` |
| D7 | SC5001 | pending body read under `lazy` with SSR | warns, does not throw | `v` |

**D1. Code in a server-only or production-only arm is analysed as live.**

```tsx
export function App() {
  const [st] = createStore({ a: 1 });
  if (isServer) { st.a = 2; }
  return <p>{st.a}</p>;
}
```

- **Runtime (`a01`):** the server ran it, landed the write, and raised no diagnostic. Chrome never ran the line.
- **Checker:** `SC2003:v` and `SC1001:v`, under no flags, browser and node alike.

| Witness | Arm | Finding |
|---|---|---|
| a02, g01 | `isServer` read | `SC1001:v` |
| g06 | `typeof window === "undefined"` | `SC1001:v` |
| g14, g16 | ternary; early `if (!isServer) return` | `SC1001:v` |
| a03 | pending read in the arm; the server retried | `SC5001:v` |
| a05 | `go()` of an action | `SC2002:v` |
| a06 | `setA(1)`: `SERVER_WRITE` warns, nothing throws | `SC2001:v`, says it throws |
| i03 | `isServer ? await date(new Date()) : …`: in process, never reaches the transport | `SC7007:v` |
| g10, g09 | `!isDev`, `NODE_ENV === "production"`: unexecuted in dev, quiet in production | `SC1001:v` |

- **Tried and not reported:** aliases, namespace, rename, helper, `Boolean()`, `===` and `while` forms give the same `v` (`b2`). The props forms a04, a07, a08 and a09 are certified, which is correct.
- **Cause:** the host fold runs only on dependency files (`dependency_program.rs:221`). `SiteReach` knows only literals (ADR 0458).
- **Reach:** corpus, measured by joining `sweep-cost20.json` to a line window on `isServer`: **0 `v`**, 3 `u`. The corpus has 129 `isServer` lines and 371 `typeof window/document` lines. The arm-regex join is coarse; the line-window join is the one I trust.
- **Fix:** proved. `isServer`'s value is byte-proved already and needs no declared host: its truthy arm never runs in a client build. Treat that arm as server-only, and answer it from D2's table. `typeof window` and `isDev` arms become uncertifiable, blocker `build-constant-arm`, unless `--runtime-realm window` or the selected build decides them.
- **Fuzz shape:** a guard layer (`isServer`, negated, ternary, `&&`, early return, `typeof window`, `isDev`) around every payload, judged on **both** hosts. The generators miss it because they run one host, and that host's dead arm is `unexecuted`.

**D2. Client claims stand on the server build.**

```tsx
function I() {
  const m = createMemo(async () => { await wait(); return 1; });
  const v = m();
  return <p>{v}</p>;
}
export const App = () => <Loading fallback="f"><I /></Loading>;
```

With only `renderToStream` in the project:

| Witness | Runtime | Checker, under `streaming-ssr` and under no flags |
|---|---|---|
| s15, above | body ran twice, `v 1`, no diagnostic | `SC5001:v` "cannot suspend or retry" |
| s02 body read | silent | `SC1001:v` |
| s11 `st.a = 2` | silent, lands | `SC2003:v`, `SC1001:v` |
| s24 `go()` of an action | silent; `action` returns `fn` and the generator never starts | `SC2002:v` |
| s31 `refresh(m)` | silent | `SC2001:v` |
| s01 `setA(1)` | `SERVER_WRITE` warns, lands | `SC2001:v`, says it throws |
| e05 fixture in a vitest node test | passes, no diagnostic | `SC1001:v` (run under `browser`, `csr` flags only) |

- **Sharpest evidence:** the fuzzer's own legal template `l-settled-first` gets `SC5001:v` on a program the host ran to completion, and `m-body-after-read` depends on the same retry the finding says cannot happen. `judgeSsr` reads only SC7005, so nothing noticed.
- **Cause:** `selects_server_build` has one caller, and it gates three checks.
- **Reach:** the sweep runs every project as `browser`, `csr`, so it cannot measure this. The corpus has 37 files in viviana-ui-web that hydrate or render on the server (grep), all but two of them test files. No sweep finding sits in a `.ssr.test.` file, so they are outside the swept tsconfig.
- **Fix:** add a program fact, "a client root exists": any `render` or `hydrate` call. Under server selection with no client root, a client-only claim is **proved** silent, using the table in section 3. With no selection and only server entries, it is **uncertifiable**, blocker `client-root-unproven`, because a client entry may be outside the project. A project with a client root keeps its findings (see D13), and each finding should say it is the client's.
- **Fuzz shape:** the misuse grid rendered through `ssr-host.mjs` with a per-program runtime diagnostic judge. The missing piece is a judge, not a generator.

**D3. `NoHydration` subtree.**

```tsx
function I() { const [a] = createSignal(0); const v = a(); return <i>{v}</i>; }
export const App = () => <main><NoHydration><I /></NoHydration></main>;
```

- **Runtime (h10, h12):** server ran it silently. Chrome: `NoHydration` returns `undefined` while hydrating, so `I` never ran.
- **Checker:** `SC1001:v`. h11 `setA(1)`: `SC2001:v`, says it throws.
- **Reach:** 0, no `<NoHydration` in the corpus.
- **Fix:** uncertifiable, blocker `no-hydration-subtree`. It cannot be a proof, because a non-hydrating mount does run the children (source, not run).
- **Fuzz shape:** a `NoHydration` container on the hydration host. No generator has hydration.

**D4. SC7007: `undefined` before a trailing body.**

- **Program (`f16`):** `one(undefined, new Uint8Array(2))`, with `one(a: number | undefined, b?: Uint8Array)`.
- **Runtime:** at the real boundary it resolves `null:2`. Rc.21 maps leading `undefined` to `null`.
- **Checker:** `SC7007:v`, "throws at the transport".
- **Fix:** proved. The trailing-body rule checks `leading.map(undefined → null)`, not `undefined` itself. The silent change to `null` is a separate fact worth a note on the page.
- **Reach:** 0 `v`. SC7007 has 2 `u` in the corpus (skyjtx-website).

**D5. SC7007 on a call the transport does not see.**

- **Program (`i01`):** `createMemo(async () => await date(new Date()))` in a hydrated app.
- **Runtime:** the server ran it in process and got `Date`. Hydration used the serialized value; the compute ran under `subFetch`, the rejection was swallowed, and nothing showed. `i02` is the control: a later `refresh(m)` does reject in Chrome. i04 in a handler rejects too.
- **Checker:** `SC7007:v`.
- **Fix:** a compute's first run under hydration or SSR is not a transport call. Uncertifiable, blocker `transport-run-unproven`, unless the site is a handler or a re-running compute. In a server-only project, proved silent.
- **Reach:** 0.

**D6. `render(Local, el)`.** This is a wrong promise: the hint claims a warning, like the D3 of the earlier audits.

- **Program (e01, t16, t17):** a local component passed straight to `render`.
- **Runtime:** ran, no window, no warning.
- **Checker:** `SC1001:v`, "Solid warns". The imported form `render(App, el)` (rd1) is certified, so the two forms disagree.
- **Fix:** the `render` argument's own body is a root body, not a component body.
- **Reach:** not measured.

**D7. SC5001 under `lazy` with SSR.**

- **Program (`lz-pending`, through the plugin with a real manifest):** a lazy component with an own async memo read in its body.
- **Runtime:** server `xp 5`. Chrome `xp 5` with `STRICT_READ_UNTRACKED`, no throw.
- **Checker:** `SC5001:v`, says it throws. The non-lazy form (h01) does throw, so this is the lazy resume only. The read is still a real stale read, so the right finding is SC1001 or `u`.
- **Reach:** not measured.

### False cleans

| # | Rule | Program | Runtime | Checker |
|---|---|---|---|---|
| D9 | SC7007 | `api.date(new Date())` and 9 other carriers | rejects | certified |
| D10 | SC7007 | `enableRichArguments` imported, not called | rejects | certified |
| D11 | SC7007 | `f(-0)` | rejects | certified |
| D12 | SC7006 | a closure capture | compile error | certified |
| D13 | SC2008, SC6002 | hydrated project under node selection | warns or throws in Chrome | certified |
| D14 | SC2001, SC1001 | timer fired by `vi.advanceTimersByTime` in owned code | throws or warns | certified |
| D15 | SC5001 | `render(App, el)` with a pending body read | uncaught `NotReadyError`, blank page | certified |
| D16 | none | async memo returning JSX; `resolve` in a body | server error | certified |

**D9. SC7007 misses calls that aren't a named direct call.** All reject in Chrome and all are certified:

| Witness | Shape |
|---|---|
| r17, and `b6/cases` (every `Date`, `bigint`, `undefined` case) | namespace import, `api.date(new Date())` |
| r22, r24 | `await import("./api")`, member or destructured |
| r09 | `const f = date; f(…)` |
| r14 | `const o = { date }; o.date(…)` |
| r11, r25 | `.call`, `.bind` |
| r12 | `[new Date()].map(date)` |
| r18 | `GET(date)` then `g(new Date())` |
| r26 | `invoke(date, {}, new Date())` |

- **Held, correctly `v`:** a barrel re-export, default import, function-level directive, optional call, an action.
- **Plan gap:** the plan's edit covers references outside callee position. It does not cover a namespace member call, which **is** a callee, nor dynamic import.
- **Fix:** resolve the namespace member to the export; proved. Dynamic import and handed references: uncertifiable, blocker `server-function-value-handed`, as the plan names it.
- **Fuzz shape:** the carrier layers over a server-function payload on `sf.mjs`. Reach: grep level, 1 project has `"use server"`.

**D10. The opt-in is the call, not the import.** `rich-args.js:100` installs inside `enableRichArguments()`.

| Witness | Shape | Runtime | Checker |
|---|---|---|---|
| r01, r06 | imported, never called; bare import | rejects | certified |
| r02 | called under `if (isServer)` | rejects | certified |
| r03 | called only in a server-side file | rejects | certified |
| r04 | called after the first call | rejects | certified |
| r05 | in a function nothing calls | rejects | certified |
| r07 | called at module scope, control | passes | certified |
| r08 | `configureServerFunctionsClient` in an uncalled function | rejects | `v`, correct |

- **Fix:** silent only for a call that runs on the client before the site. Module scope in a client-reached file is proved. Anything else is uncertifiable, blocker `rich-args-call-unproven`. The rule page's "importing the entry is the opt-in" must go.
- **Reach:** 0 corpus uses.

**D11. `-0`.** rc.21's `isJSONSafe` has `Object.is(v, -0)`; the page rests on an rc.0 probe. `zero(-0)` and `const z = -0; zero(z)` (a: `0`) reject and are certified. `server_rules.rs:547` needs to refuse negative zero. Proved. Reach: 0.

**D12. SC7006 misses the directive pass's capture error.**

```tsx
export function App() {
  const [a] = createSignal(0);
  async function f() { "use server"; return a(); }
  return <button onClick={() => f()}>x</button>;
}
```

- **Runtime:** `transformDirectives` throws "server functions cannot capture non-top-level variables" on both client and server (d01, d02, d24, d25; default parameters too).
- **Checker:** certified. `tsc` passes.
- **Fix:** proved from the binder: a free variable bound in an enclosing function. Module-scope `const` and `let` compile (d04, d05). Reach: 0.

**D13. Node selection hides client defects.**

- **Program (b01, b02):** `createEffect(() => a(), () => { flush(); })` and the same with `createRoot`, in a project that calls `hydrate`.
- **Runtime:** Chrome raises `FLUSH_IN_EFFECT_CALLBACK` and `PRIMITIVE_IN_EFFECT_CALLBACK` (a throw).
- **Checker:** `SC2008:v` and `SC6002:v` under browser, **certified** under `--runtime-target node --rendering streaming-ssr`.
- **Cause:** no selection can describe a hydrated app. `--rendering` with browser is refused, and node turns client rules off for these two only.
- **Fix:** gate `client_build` on "no client root" (D2's fact). Recommended companion: a new `--rendering hydrated` that selects both sides.

**D14. Fake timers break the fresh-stack premise.** Programs are in `b15`.

```tsx
vi.useFakeTimers();
const [, setA] = createSignal(0);
setTimeout(() => { setA(1); }, 10);
createRoot(() => { vi.advanceTimersByTime(10); });
```

- **Runtime:** the test fails with `REACTIVE_WRITE_IN_OWNED_SCOPE` (e02, and e03 in a component body). e04 warns `STRICT_READ_UNTRACKED`.
- **Checker:** certified.
- **Fix:** an owned call to a timer-draining member of `vi` (or `jest`, `sinon`) withholds the timer proofs, like the dispatch veto. Contrived, and the most natural case is outside an owner, where it is legal.

**D15. `render(App, el)` with a pending body read (rd2).** Uncaught `NotReadyError`, blank page, no Solid diagnostic. Certified. This is rd1's body-placement gap. Uncertifiable is the minimum.

**D16. Server-only failures with no rule.** `scripts/fuzz/codes.mjs` marks 17 runtime codes out of scope as SSR, so the judge never asks.

- **c01:** `createMemo(async () => <b>x</b>)`: `ASYNC_MEMO_ELEMENT` plus a contained error on the server, `REACTIVITY_HALTED` in Chrome. Certified.
- **s10, c03:** `resolve(...)` in a body: "resolve is not implemented on the server". Client fine. Certified. `until` is the same throw (source).
- **s29:** `isPending` rethrows on the server where the client returns a boolean.

These are rule proposals, not premise failures.

**Already known, confirmed with a real `import()`:** `clientOnly` before load. co2 is quiet and renders, `SC5001:v`; co-read is silent, `SC1001:v`. After load, mounted later, cs-read warns and cs-pending throws, so the plan's "`clientOnly` uncertain" is right for both.

## 3. Premises to record

**What the server build does.** This table belongs in `docs/open-work.md`, or a page of its own, and D2's fix reads from it.

| API | Server build, rc.21 |
|---|---|
| `createEffect` | compute runs once, apply never |
| `createRenderEffect` | compute and apply both run |
| `createTrackedEffect`, `onSettled` | never run |
| `createReaction` | tracking callback runs, reaction never |
| signal setter | `SERVER_WRITE` warn, lands, no re-render, no throw |
| store setter | `SERVER_WRITE` warn, lands |
| direct store write | silent, lands |
| `flush`, `refresh` | no-ops, in effect |
| `untrack`, `latest` | call `fn` |
| `resolve`, `until` | throw "not implemented on the server" |
| `isPending` | rethrows a pending read |
| `action` | returns `fn` unchanged |
| `render`, `hydrate`, `insert`, `ref` | throw "Client-only API" |
| `onHydrationEnd` | `queueMicrotask` |
| strict window, write guard | absent |
| a pending read in a body | retried by the boundary; the body runs twice |
| `onCleanup` with no owner | silent |

Also record:

- **A compute under hydration:** with a serialized value, the compute still runs, under `subFetch`: `window.fetch` and global `Promise` are replaced by a mock (`solid.dev.js:465`). Code past an awaited mock never runs (h07: `s` logged, `t` not). `new Promise` executors in that compute do not run (source, not run).
- **Memo value under hydration:** the served one. A sync memo returning something else keeps the server's value (h06).
- **A body runs twice** on the server when it reads pending (s15, h29). A signal created before the read is a new signal on the retry.
- **The sweep's `rendering: csr`** is false for the projects that server-render, so `ProvenClientOnly` hides SC5003's SSR variant across the corpus (k3: `SC5003:v` under browser, certified under `csr`).
- **`hydrate` plus a server entry** makes SC5003 `u` where hydrate alone is `v` (h19 against k3). Recall only.
- **Production** (Chrome, real build):

| Program | Dev | Production |
|---|---|---|
| write in a body | throws | lands |
| direct store write | warns, dropped | dropped silently |
| `PRIMITIVE_IN_EFFECT_CALLBACK` | throws | runs |
| `SYNC_NODE_RECEIVED_ASYNC` | throws | renders empty |
| `resolve` in a memo | throws | works |
| `FLUSH_IN_ACTION` | throws | no throw |
| own pending body read (p05) | throws | stays on the fallback after 100 ms; not run longer |

- **`--runtime-build production` moves no finding** (16 of 16 identical). It only changes dependency selection. Pages with no production line, though the runtime differs: `no-write-in-pure-computation`, `no-store-mutation-outside-setter`, `pending-async-unsuspendable-read`, `action-called-in-owned-scope`. SC5001's page says "Solid therefore throws" with no build named. 10 of 40 pages mention production at all.
- **Test builds:** default `bun test` is the server build (`isServer` true, effects never apply, no throw). `--conditions=browser` is the production client. Vitest's plugin compiles non-hydratable; `ssr` tests run server output.
- **A hydrated app has no selection.** `csr` is false, node hides client rules, and SSR with browser is refused.
- **`export let f` reassigned** at module scope is fine at the real boundary (k2 `OK 1`): registration takes the value at the declaration.

## 4. Fuzzer blind spots

1. **Host.** Every generator runs one client mount. There is no hydration host, no server judge beyond SC7005, and no two-sided verdict, which D1, D3 and D13 need.
2. **`judgeSsr` reads one code.** `SC5001:v` on a legal SSR template passes today.
3. **Runtime build.** No production column.
4. **Build-constant guards.** `isServer`, `typeof window`, `isDev`, `import.meta.env`, `NODE_ENV`.
5. **Server functions.** No payload, no carriers, no `rich-args` axis. `sf.mjs` here is a working base.
6. **Compiler-oracle rules.** SC7006's directive errors are not fuzzed against `transformDirectives`.
7. **Test-runner axis.** Fake timers, `renderHook`, `fireEvent` under an owner.
8. **Hydration containers.** `NoHydration`, `Hydration`, `lazy` with a manifest, `clientOnly`.
9. **Streaming time.** The chunk gap: handlers and applies during a resumed boundary. All held here, but none are generated.
10. **`codes.mjs` SSR out-of-scope list:** 17 codes nothing checks.
11. **Sweep selection.** One `csr` run over projects that server-render.

## 5. What the flip tracks should change

**Track W**

- **SC7007** (`PLAN.md:129`):
  - The plan's "Runtime not measured" can go: this report measured it at the real boundary, with `h/sf.mjs` as the harness.
  - Add to the edit: namespace member calls and dynamic import (D9), the call-not-import opt-in (D10), `-0` (D11), the `undefined` leading arg (D4), the first-run-under-SSR case (D5), and arms (D1).
  - Keep `v` for handlers and re-running computes.
- **SC7006** (`PLAN.md:119`):
  - Add the capture error (D12).
  - The `export let` edit can stay as `Unresolved`, but it is not a boot failure, as k2 measured.
  - The class and destructuring exports (d13, d20) are `u` where the compiler errors, so they can be proved.
- **SC7005:** no change to status. Fix ADR 0431's text. The plan's four witnesses stand.
- **SC2008, SC6002:** replace `client_build` with "no client root" before building on it (D13). Otherwise the new `else` arms inherit the hole. The doc comment at `lib.rs:289` is wrong for `createRenderEffect`.
- **SC2003, SC2002, SC2001:** each needs the arm rule (D1) and the server-only rule (D2). SC2001's "throws" is false on the server: `SERVER_WRITE` warns. Do D1 and D2 before SC2001 lands last, or the new obligations stack on arms that never run. The same applies to `refresh` and `action` in a server-only project.
- **Fuzz:** add `isServer` to `flip-write.mjs`'s guards. It needs a two-host judge to count.

**Track R**

- **SC5001 and SC5003** (`PLAN.md:263`):
  - Under server selection with no client root, SC5001 is false by the host's own templates (D2); make it silent there.
  - Add `lazy` with SSR to `mount-form-window`, since D7 warns rather than throws.
  - `clientOnly` uncertain is right; I confirmed both paths with a real `import()`.
- **SC1001** (`PLAN.md:282`): widening `unreached_read` to `unplaced_run` doesn't reach D1 or D3, because `SiteReach` is `MayRun` there. Add the arm fact and a `NoHydration` blocker. Also the `render(Local, el)` body (D6) and rd2 (D15). Do this before **SC9012** closes the track, because the census will read these sites.
- **SC5003:** consider not reading `rendering: csr` as proof when the sweep sets it for a project with `hydrate` (section 3).

**Both**

- One shared fact is needed: **`client_root_present`**, plus the **server-only arm**. Both are small. I'd land them on `claude/flip-base` before more rows build on the old answer.
- Neither track's `witnesses.json` has a server or hydration witness. `wit/hyd.json` and `wit/srv.json` are ready to convert, with a server column added to `check.mjs`.

## Limits

- **Measured against inferred:** every row marked with a witness ran. "Source" means read, not run.
- **Hydration timing:** static-HTML hydration used `h/hyd.mjs`; only the `b13` programs had real chunk gaps.
- **Corpus numbers:** from `sweep-cost20.json`, or grep where stated. No sweep ran.
- **The `s-nohost` results** rely on host inference staying off in those small projects.
