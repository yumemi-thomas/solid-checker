# runtime-oracle

Cases for `scripts/runtime-oracle.mjs`: each runs in Chrome on the Solid 2 dev build, and the runtime's diagnostics are compared with the checker's verdict at the same site. A dev diagnostic where the checker reports nothing is an unproven clean claim. Cases run in `../solid-checker-rc13-corpus/apps/helge-dev` (rc.13; `--install` overrides) against the real packages: `*.d.ts` stubs and `node_modules` are not copied, and a case `tsc` rejects is `type-error`.

A case is a directory with an `App.tsx` and no `tsconfig.json`; every exported component is rendered and events are dispatched on every element. Optional `oracle.json`: `note`, `steps`, `install` (corpus app with the same Solid version), `open` (known unproven clean sites `{ code, site, reason }`, or `{ rule, site, reason }` for an observation; a site that stops firing is `stale-open`), and `behaviour` (below).

Verdicts: `covered-violation` and `covered-uncertifiable` (a finding on the diagnostic's line or an application frame), `unproven-clean` (exit 1), `known-open`, `no-rule`; `static-only` (a checker violation with no dev diagnostic and no observation: inspect it before calling it a false positive); `behaviour-confirmed`, `behaviour-refuted`, `behaviour-clean`; collector failures are `harness-error`.

The `c*`, `r*` and `oracle-*` cases are reviewer counterexamples and probes (adaptations in each `note`); archive cases needing package metadata, CommonJS or declaration-only values are not represented.

## Behavioural observations (`behaviour`)

A static violation with no dev code may be real or false, so a case can observe the page for it. `behaviour` is a list in `oracle.json`, one entry per observation:

```json
{ "rule": "uncalled-accessor", "site": "App.tsx:11",
  "act": [{ "click": "[data-probe=violation-write]" }],
  "observe": { "text": "[data-probe=violation-label]" },
  "defect": { "pattern": "^Count: .*(function|=>)" },
  "correct": "Count: 7" }
```

- `rule` and `site` (`App.tsx:<line>`) name the checker rule and the line it covers; a finding of that rule whose lines include `site` is the claim under test.
- `act` is optional: scripted clicks, each followed by a flush. The probe runs before the event exercise, so acts start from the initial render. Observations share one page and run in list order, each from the state the earlier ones left.
- `observe` names exactly one kind: `{ "text": selector }` (`textContent` after the acts, `null` when nothing matches); `{ "window": "a.b" }` (the value at that path of `window`); `{ "identity": selector }` (`same`, `replaced` or `missing`); `{ "thrown": true }` (`threw` or `none`, the message kept as `detail`); `{ "reflected": selector }` (below).
- `defect` and `correct` are the value that shows the defect and the value that shows correct behaviour: a JSON literal, or `{ "pattern": regex }` for `text` or `window`.
- Refused at load, as a `harness-error` with the reason (other cases still run): `behaviour` not a non-empty list; a `rule` that is neither a `codeRules` value nor a file in `docs/rules/`; an empty or uncompilable pattern; `defect` equal to `correct`; a literal side the other side's pattern accepts. Two patterns need `defectExample` and `correctExample`: page texts matching their own side and not the other. Examples are checked, not proved.

### Write-reflected observations

`{ "reflected": selector }` reads one element's `textContent` before and after each write. Its `writes` replace `act`: `{ "click": selector, "expect": "lost" | "reflected" }`, each click followed by a flush. Changed text is `reflected`, unchanged text `lost`. The defect is `lost` and the correct behaviour is `reflected`, so the spec names neither.

- The page shows the defect when any write is `lost`, the correct behaviour when every write is `reflected`; the table below applies. `expect` is the case's prediction: it is reported per write as `match`, and the verdict follows the page, as a text observation's does.
- A write must change its source, or `lost` may be a repeated value. Buttons a reflected observation uses set a fresh value on each click (a counter), and no other observation uses them. An update that lands after the flush reads as `lost`, so the kind is for synchronous writes.
- A click that throws, a missing element, or a step still running after 2 s is `harness-error`. `flush()` is synchronous in rc.13, so the 2 s bound binds only an asynchronous flush; a page that never returns from a step ends the feedback run, also `harness-error`.
- Refused at load: no `writes` or an empty list; a write not exactly `{ click, expect }`, or whose `expect` is not `lost` or `reflected`; a click listed twice; `act`, `defect` or `correct` beside it; `writes` on another kind.

Each observation gets a row by what the page shows and whether a finding of its rule covers its site:

| page shows | finding of `rule` covers `site` | no such finding |
| --- | --- | --- |
| `defect` | `behaviour-confirmed` | `unproven-clean` (exit 1), or `known-open` when listed |
| `correct` | `behaviour-refuted`: a false-positive candidate | `behaviour-clean` |
| neither, or no result | `behaviour-unexpected` (`harness-error`, exit 1) | |

A static violation no dev diagnostic covers is classified by the observations of its rule at its lines: `behaviour-confirmed`, `behaviour-refuted`, or `static-only` when there are none. Observations that disagree about one finding are `behaviour-conflict` (`harness-error`). `behaviour-refuted` is reported ahead of `covered-violation` and does not exit 1: it is a candidate, and the report names it.

## Behavioural cases

| case | rule | observation | verdict |
| --- | --- | --- | --- |
| `oracle-behaviour-uncalled-accessor` | SC1005 | text: the violation keeps the accessor's source; reflected: its write is `lost`, the called control's is `reflected` | `behaviour-confirmed` |
| `oracle-behaviour-no-direct-mutation` | SC2003 | text and reflected: the direct write is dropped (`Ada`), the setter write shows (`Grace`) | `behaviour-confirmed` |
| `oracle-behaviour-primitive-in-directive` | SC6001 | window: the apply-phase effect runs again after its element is hidden (2, not 1); the control's does not | `behaviour-refuted` (below) |
| `oracle-behaviour-jsx-no-duplicate-props` | SC8003 | reflected: a `textContent` write beside children is `lost`; the innerHTML markup write is `reflected`, its children write `lost`; the one-source controls `reflect` | `behaviour-confirmed`; both controls `behaviour-clean` |
| `oracle-behaviour-components-return-once` | SC1004 | text: the early return stays on screen after the write (`logged out`) | `covered-violation`; behaviour rows confirmed and clean |
| `oracle-frame-covered-sc1004` | SC1004 | none: the frame pin (below) | `covered-violation`, empty `staticOnly` |

The `oracle-behaviour-primitive-in-directive` control's `missing-owner` finding (`App.tsx:18`) is refuted: the runtime disposes the effect when its `Show` branch hides, as the rule page's correct example does. It is a false-positive candidate, recorded in `docs/open-work.md`, not fixed here. `NO_OWNER_EFFECT` is claimed by `primitive-in-directive-application` as well as `missing-owner`, since that rule's page names the warning for an apply-phase `createEffect`.

## Frame attribution

A diagnostic covers a finding when the finding's lines include the diagnostic's site or one of its application frames, and its code claims the finding's rule. The runtime attributes a read to the call that led to it, so the read's own line is a frame, not the site. `oracle-frame-covered-sc1004` pins this: a `components-return-once` finding at a props read (`App.tsx:9`) is a frame of the `STRICT_READ_UNTRACKED` diagnostic sited at `Parent`'s call (`App.tsx:17`), and must not print as `static-only`.

The `identity` and `thrown` observation kinds are implemented, but no committed case uses them yet.
