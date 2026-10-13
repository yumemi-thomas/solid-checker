# What is the last blocker of a @solid-primitives export

Integrated update, 2026-09-30, source `6208b85f`: the complete retained
checkpoint host runs measure **101 / 101 / 131 clean exports** after ADR 0170.
`sse:number` now closes; the sibling-barrel recommendation below is historical.
The strict ADR 0162 witness delivers no `owned-memo` export: the factories in
this corpus resolve into the unaudited `solid-js` hydration/server bodies.
The fresh none-host distribution is 101 clean, 64 with one cause, 44 with two,
46 with three and 466 with four. Of the sole blockers, ten remain
`recursive-value-shape`, five have no proposed returns shape, and two are
`callable-path`. The following tables preserve their original measurement.

2026-09-30. Base 816d6f58 (ADR 0165). The checkpoint's three host runs (the
`make primitives-checkpoint` procedure, release binary, the pinned 97 packages,
721 exports, without the misuse ledger) read **clean 99 / 99 / 100** (none /
browser / node). The ranking script is beside this note
(`2026-09-30-primitives-last-blockers.mjs`; `bun ... measure.json
measure-browser.json measure-node.json`); it reads the checkpoint's own
`measure*.json`, so these are the checkpoint's causes, not a new classifier.

The first ranking was made at d88e3b50 (clean 90 / 90 / 100), where the second
sole cause was `date`'s 22 exports, refused whole for a veto contradiction that
ADR 0165 then closed (99 / 99 / 100). What follows is at the new base.

An export is clean only when every domain closes, so a cause is worth what it is
*alone* worth. Each open export carries a set of distinct causes (`domain: key`;
a package-level refusal counts as one). **Sole** counts the exports whose set is
exactly that cause, **pair** the exports whose set is exactly those two. Closing
a cause that is in neither moves nothing.

## The shape of the wall

| host | clean | open, 1 cause | 2 | 3 | 4 |
| --- | ---: | ---: | ---: | ---: | ---: |
| none | 99 | 66 | 42 | 44 | 470 |
| browser | 99 | 66 | 42 | 51 | 463 |
| node | 100 | 72 | 38 | 49 | 462 |

470 of 721 exports carry a cause in *every* domain. The whole prize of the sole
column is 66 exports (none, browser) and 72 (node); 36 of the 66 are a package
no export of which can be measured (below).

## Sole blockers (none; browser's ten rows are the same)

| exports | cause | examples | what it takes |
| ---: | --- | --- | --- |
| 24 | package: refused at artifact-case | `drag-drop` (13), `favicon` (11) | Owner: a published-package defect. Both import `solid-js/web`, which `solid-js@2.0.0-rc.9` does not export (the checkpoint already lists it). |
| 11 | returns: `recursive-value-shape` | `utils` (8), `date:getDate`, `refs:mergeRefs`, `intersection-observer:getOccurrence` | Mostly a value shape no census decides: the key is the demand family's name and the reasons differ (below). One, `utils:number`, is closed by this change. |
| 9 | package: published-artifact | `controlled-props` (7), `virtual` (2) | Owner: the same defect as the first row. |
| 5 | returns: never proposed | `clipboard:makeClipboard`, `writeClipboard`, `jsx-tokenizer:createTokenizer`, `scroll:getScrollPosition`, `vibrate:frequencyToPattern` | Value shapes with no census: a tuple of functions, an object of primitives, an array. |
| 3 | package: no accepted case | `analytics` relay entrypoints | Attribution, not a cause: the case has no certified closure, so its per-domain refusals are hidden. Each would be a four-domain export. |
| 3 | reads: accessor installation unbounded | `i18n` `template`, `identityResolveTemplate`, `missingKeyAsPath` | The case-wide `Proxy` hazard (`proxyTranslator`) these exports never reach; the producer states no reachability for an unbounded site. Producer work. |
| 2 | reads: no probe recipe | `event-bus:batchEmits`, `set:readonlySet` | The synthesized veto needs every parameter sampled; a generic bus and a class instance are not sampled. The owner declined hand recipes. |
| 2 | returns: `callable-path` | `promise:changed`, `timer:makeTimer` | A described callable whose nested call is not stated to run exactly once (a `try`/`catch`, a coercion). |
| 2 | callbacks: closure describes `coerce` uses | `sortable:ascending`, `descending` | A `coerce` item the walk cannot confirm across a local declaration (depth 1). |
| 2 | callbacks: invokes a caller-supplied callable | `utils:detectColorFormat`, `video:setVideoSrc` | The census found `value.trim()` and property reads on a parameter typed `string`. Narrowing by the declared signature exists for `coerce` uses and not for these: a premise decision (ADR 0157 asks the same question for returns). |

Node adds one row: **3 exports of `lifecycle` refused whole** (below), and has 14
`recursive-value-shape` and 6 never-proposed sole exports (the ones whose other
cause is the host).

## Pairs (none; node's differ by `callbacks` + `returns: veto did not complete`)

| exports | causes | examples |
| ---: | --- | --- |
| 12 | callbacks: invokes a caller-supplied callable + returns: `recursive-value-shape` | `cookies:parseCookie`, `date:getTime`, `flux-store:createActions` |
| 3 | reads: veto did not complete + returns: `recursive-value-shape` | `clipboard:newClipboardItem`, `readClipboard`, `styles:getRemSize` (DOM at import) |
| 3 | callbacks: proposed, not certified + returns: `recursive-value-shape` | `refs:getFirstChild`, `sortable:by`, `utils:dropRight` |
| 3 | callbacks: never proposed + returns: `recursive-value-shape` | `rootless:createCallback`, `utils:arrayEquals`, `asAccessor` |
| 2 | callbacks: invokes ... + returns: veto did not complete | `date:getDateDifference`, `utils:contains` |
| 2 | callbacks: domain-exhaustiveness + returns: fallback-all | `sse:pipe`, `sse:safe` |

Every large pair contains a `returns` cause that needs an object, tuple or array
shape. The `callbacks` half of the biggest pair is the same declared-signature
premise as the sole row above, so the two are one owner question with 14
exports behind it in none (2 sole plus 12 in a pair).

## What `recursive-value-shape` is

The metric's key is the name of the proof-demand family the `returns` operation
census asked, not a reason. The reasons behind the eleven sole exports, from the
audit:

| reason | exports |
| --- | --- |
| the completion is not a primitive the producer proved (an object, an array, a function or `any`) | `utils:getColorChannels`, `merge`, `shallowObjectCopy`, `withObjectCopy`, `json`, `refs:mergeRefs`, `date:getDate` |
| a returned parameter the producer types `any` (`normalizeHue`'s `if (hue === 360) return hue`) | `utils:normalizeHue` |
| a member read of an enum-like object (`Occurrence.Inside`) | `intersection-observer:getOccurrence` |
| a call the census names no reviewed row for (`Number(raw)`) | `utils:number` (closed here) |
| a described callable whose nested call is not stated to run exactly once | `utils:safe` |

## The returned accessor (the lead's candidate)

ADR 0164's table counts 25 exports that return an accessor. They are neither
unproposed nor uncensused. The generated documents of the none run propose ADR
0145/0146's described callable for **38** exports and the census decides each
one; **2** close `returns` (`utils:createIdGenerator`, `utils:pipe`; node adds
`lifecycle:createIsMounted`). The other 36 are withheld by name, and the reason
is almost never the returned callable itself:

| withheld for | exports | examples |
| --- | ---: | --- |
| the export's own `creates` census: `property-access-unknown-accessor` | 10 | `gestures` `longPress`/`pan`/`pinch`, `event-dispatcher:createEventDispatcher` |
| the same census: `coercion` | 5 | `gestures:doubleTap`, `i18n:scopedTranslator`, `utils:defer`, `utils:reverseChain` |
| a call through a parameter or local binding, a standard-library member with a callable slot, iteration, other | 7 | `flux-store:createAction`, `sensors:makeAccelerometer`, `utils:chain` |
| the completion is neither `plain` nor `read-value` | 3 | `tween:createTween`, `websocket:createWSMessage`, `createWSState` |
| the literal's nested call is not stated to run exactly once | 2 | `utils:safe` (a `try`/`catch`), `timer:makeTimer` |
| other withheld `returns` operations, and one unaccepted dependency (`fs/promises`) | 9 | `filesystem:makeChokidarWatcher` |

A further 14 exports are proposed `reactive` (an accessor a `createMemo` or
`createHydratableSignal` returns), which no census decides (ADR 0146's
inertness premise, ADR 0157, an owner decision). And `returns` is never the last
cause for them: of the 228 open exports whose contract or type misuse class is a
returned accessor, 2 have `returns` alone open and 4 more `returns` with one
other domain. The lever the hint names is already pulled; what stops those
exports is the `creates` and `callbacks` census over the same body, whose
`property-access-unknown-accessor` and `coercion` forms are 87 and 45 exports
wide and are never a sole cause.

## The whole-package refusals

A veto contradiction refuses a whole package because a mandatory veto
contradicted a closure the census had proved. It is the census and the runtime
disagreeing, which ADR 0163 says is worth a refusal.

- `date` (none, browser) was that at d88e3b50: 22 exports, 9 of them clean
  before ADR 0163. `createCountdown` reads a memo it created at the call, the
  census dispositioned no call, and the veto's tracking memo gained the
  dependency. ADR 0165 walks calls in the `reads` census and the package
  certifies again (99 / 99 / 100).
- `lifecycle` (node, 3 exports) remains, refused since before ADR 0163. The claim
  is `onElementConnect`'s ADR 0116 return, `invocation-result 1`. Under `node`
  the body is `if (isServer) return;`, a valueless completion, which the census
  leaves undescribed by design (a valueless completion is no `return`
  operation); the container veto's `holds` treats the resulting `undefined` as
  outside the claimed containers. The two disagree on valueless completions.
  Closing it moves no clean export (all three keep other domains open) and would
  weaken the veto's reach over an `undefined` value-carrying return, so it is
  recorded and not changed.

## What was closed

The only sole cause closable without an owner decision, a new value shape, a
producer flow analysis or a hand recipe is a *reviewed row*: `utils:number` is
`(raw) => Number(raw)`, and the plain `returns` census reviews a default-library
call only as `Receiver.member`, so a global function called by name stated
nothing. ADR 0167 states the receiverless call and reviews seven total
functions. It closes `utils:number`.

`sse:number` was expected to follow through its re-export and does not. Its
`returns` is held by an attribution widening, not by `utils`: the import
`import { json, lines, ndjson, number, pipe, safe } from
"@solid-primitives/utils"` sits in `dist/transform.js`, a sibling barrel of the
entry, one of its bindings (`lines`) leaves `ownerRequirements` open in the
accepted `utils` node, and ADR 0133's `reexported-import` rung answers only a binding in the
entry's own file, so the obligation is marked on every export that reaches the
file (10 in `sse`). `number` is certified in the accepted `utils` node with all
four domains closed, so a rung that follows a binding through a sibling module
whose every reference is an `export { … }` would attribute it exactly. That is
+1 clean in every host (`sse:number`, whose only open cause it is), no claim
form, and the next lever of this kind.

## Measured

The checkpoint's own host procedure, the same release binary flags and the same
97 probes per host, with the checker built at 816d6f58 and at 816d6f58 plus ADR
0167 (each with its own rebuilt `bin/solid-typefacts`). Run the same way at
d88e3b50 the pair read 90 / 90 / 100 and 91 / 91 / 101.

| host | before (816d6f58) | after |
| --- | ---: | ---: |
| none | 99 | 100 |
| browser | 99 | 100 |
| node | 100 | 101 |

The only export that moves, in every host, is `@solid-primitives/utils` `number`
(`partial` to `clean`); a full diff of the three `measure*.json` files finds no
other change. The misuse ledger was not re-run: no contract it reads changes.

## Left, with counts (none host)

| cause | exports it is alone or in a pair for | needs |
| --- | ---: | --- |
| a `returns` value shape (object, tuple, array; `recursive-value-shape`, never proposed) | 10 + 5 sole, 22 in pairs | a new census over `ValueShape::Object` / `Tuple` / `Array`, or the owner's "declared signature discharges" decision |
| published-package defects | 33 sole, 9 more | owner |
| `callbacks` parameter-rooted on a primitive-typed parameter | 2 sole, 15 in pairs | owner: declared-signature premise beyond `coerce` |
| `reads` accessor installation unbounded | 3 sole | producer reachability for an unbounded site |
| `reads` no probe recipe | 2 sole | owner: sampling an unsampleable slot |
| `sse:number`, an attribution widening at a sibling barrel | 1 sole | extend ADR 0133's rung through a sibling module |
| `lifecycle` veto contradiction (node) | 3 sole | veto vs census on a valueless completion |
