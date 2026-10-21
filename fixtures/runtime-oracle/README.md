# runtime-oracle

Cases for `scripts/runtime-oracle.mjs`, which runs each in Chrome on the Solid 2 dev build and compares the runtime's diagnostics with the checker's verdict at the same site; a dev diagnostic where the checker reports nothing is an unproven clean claim. Cases run in `../solid-checker-rc13-corpus/apps/helge-dev` (rc.13; override with `--install`), against the real packages: `*.d.ts` stubs and `node_modules` are not copied, and a case `tsc` rejects is `type-error`.

A case is a directory with an `App.tsx` (no `tsconfig.json`); every exported component is rendered and events are dispatched on every element. Optional `oracle.json`: `note`, `steps`, `install` (corpus app with the same Solid version), `open` (known unproven clean sites `{ code, site, reason }`, or `{ rule, site, reason }` for a behavioural observation; a site that stops firing is `stale-open`), and `behaviour` (observations of the page, below).

Verdicts: `covered-violation`/`covered-uncertifiable` (a finding on the diagnostic's line or an application stack frame), `unproven-clean` (exit 1), `known-open`, `no-rule`; `static-only` (checker violation, no runtime diagnostic and no behavioural observation: inspect before calling it a false positive, e.g. a store silently drops a direct write); `behaviour-confirmed`, `behaviour-refuted`, `behaviour-clean` (see below); collector failures are `harness-error`. Quiet cases are controls, not proof every branch executed.

The `c*`, `r*`, `review-r*` and `oracle-*` cases are reviewer counterexamples and minimal probes; adaptations (mount wrappers, no-op sink witnesses, `on:click` to `onClick`) are in each `note`; r10 cases use `install: "helge-dev"` (`@solidjs/router@2.0.0-next.26`). Archive cases needing package metadata, CommonJS exports or declaration-only values are not represented.

## Behavioural observations (`behaviour`)

A static violation with no dev code is not a false positive by default: the page may show the defect or correct behaviour. A case can observe the page for it. `behaviour` is a list in `oracle.json`, one entry per observation:

```json
{ "rule": "uncalled-accessor", "site": "App.tsx:11",
  "act": [{ "click": "[data-probe=violation-write]" }],
  "observe": { "text": "[data-probe=violation-label]" },
  "defect": { "pattern": "^Count: .*(function|=>)" },
  "correct": "Count: 7" }
```

- `rule` and `site` (`App.tsx:<line>` inside the case) name the checker rule the observation substantiates and the line it covers. A finding of that rule whose lines include `site` is the claim under test.
- `act` is optional: scripted clicks, `{ "click": selector }`, run in order with a flush after each. The probe runs before the event exercise, so the acts start from the initial render.
- `observe` names exactly one kind:
  - `{ "text": selector }`: the element's `textContent` after the acts, `null` when nothing matches;
  - `{ "window": "a.b" }`: the value at that path of `window`, a counter the case exposes there;
  - `{ "identity": selector }`: `same`, `replaced` or `missing`, against the element found before the acts;
  - `{ "thrown": true }`: `threw` when an uncaught error occurred during the acts, otherwise `none` (the message is kept as `detail`).
- `defect` and `correct` are the value that shows the defect and the value that shows correct behaviour. Each is a JSON literal, or `{ "pattern": regex }` for `text`. They must differ, and `identity` and `thrown` take only the names above.

The probe is generated only for a case with a `behaviour` list; other cases' pages are unchanged. Its results return through the feedback run: a `wait-for-selector` step on `#oracle-behaviour` records the JSON report as a checkpoint.

Each observation gets a row by what the page shows and whether a finding of its rule covers its site:

| page shows | finding of `rule` covers `site` | no such finding |
| --- | --- | --- |
| `defect` | `behaviour-confirmed` | `unproven-clean` (exit 1), or `known-open` when listed |
| `correct` | `behaviour-refuted`: a false-positive candidate | `behaviour-clean` |
| neither, or no result | `behaviour-unexpected` (`harness-error`, exit 1) | |

A static violation no dev diagnostic covers is classified by the observations of its rule at its lines: `behaviour-confirmed`, `behaviour-refuted`, or `static-only` when there are none. Observations that disagree about one finding are `behaviour-conflict` (`harness-error`). `behaviour-refuted` is reported ahead of `covered-violation`. It does not exit 1: it is a candidate, and the report names it.

A known miss is listed as `{ "rule": ..., "site": ..., "reason": ... }` in `open`. Once the page stops showing the defect, the entry is `stale-open`.

## Behavioural cases

| case | rule | observation | verdict |
| --- | --- | --- | --- |
| `oracle-behaviour-uncalled-accessor` | SC1005 | text: the violation keeps the accessor's source after the write; the control shows `Count: 7` | `behaviour-confirmed` |
| `oracle-behaviour-no-direct-mutation` | SC2003 | text: the direct store write is dropped (`Ada`); the setter write shows `Grace` | `behaviour-confirmed` |
| `oracle-behaviour-primitive-in-directive` | SC6001 | window: the apply-phase effect runs again after its element is hidden (2, not 1); the control's setup-phase effect does not | `behaviour-refuted` (below) |
| `oracle-behaviour-jsx-no-duplicate-props` | SC8003 | text: one content slot shows `b`; the children win and the `textContent` value is lost | `behaviour-confirmed` |
| `oracle-behaviour-components-return-once` | SC1004 | text: the early return stays on screen after the write (`logged out`) | `covered-violation` (dev `STRICT_READ_UNTRACKED` covers the finding); behaviour rows confirmed and clean |
| `oracle-frame-covered-sc1004` | SC1004 | none: the frame pin (below) | `covered-violation`, empty `staticOnly` |

The `oracle-behaviour-primitive-in-directive` control's `missing-owner` finding (`App.tsx:18`) is refuted by its observation: the runtime disposes the effect when its `Show` branch hides. The rule page's own correct example has the same shape. The finding is a false-positive candidate, recorded with the known false positives in `docs/open-work.md`, not fixed here.

`NO_OWNER_EFFECT` is claimed by `primitive-in-directive-application` as well as `missing-owner`, because that rule's page names the warning for an apply-phase `createEffect`.

## Frame attribution

A diagnostic covers a finding when the finding's lines include the diagnostic's site or one of its application frames, and the diagnostic's code claims the finding's rule. The runtime attributes a read to the call that led to it, so the read's own line is a frame, not the site. `oracle-frame-covered-sc1004` pins this: a `components-return-once` finding at a props read (`App.tsx:9`) is a frame of the `STRICT_READ_UNTRACKED` diagnostic sited at `Parent`'s call (`App.tsx:17`). It must not print as `static-only`, and its verdict is `covered-violation`.

The `identity` and `thrown` observation kinds are implemented, but no committed case uses them yet.
