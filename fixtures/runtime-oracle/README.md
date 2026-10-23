# runtime-oracle

Cases for `scripts/runtime-oracle.mjs`: each runs in Chrome on the Solid 2 dev build, and its diagnostics are compared with the checker's verdict at the same site. An unmatched dev diagnostic is an unproven clean claim. Cases run in `../solid-checker-rc17-corpus/apps/helge-dev` (rc.17; `--install` overrides) against the real packages: `*.d.ts` stubs and `node_modules` are not copied, and a case `tsc` rejects is `type-error`. `driver.ts`, loaded after `main.tsx`, outside the tsconfig, sends the events: no dispatch site (ADR 0361).

A case is a directory with an `App.tsx` and no `tsconfig.json`; all exported components render and all elements receive events. Optional `oracle.json`: `note`, `steps`, `programBoundary` (`open` or `closed`), `install` (corpus app with the same Solid version), `open` (known unproven clean sites `{ code, site, reason }`, or `{ rule, site, reason }` for an observation; a listed site covered only by an uncertifiable finding is `known-open` too; a site that stops firing, or that a violation covers, is `stale-open`), `behaviour`, and the context fields (below).

Verdicts: `covered-violation` and `covered-uncertifiable` (a finding on the diagnostic's line or frame), `unproven-clean` (exit 1), `known-open`, `no-rule`; `static-only` (a violation with no dev diagnostic and no observation: inspect it before calling it a false positive); `behaviour-confirmed`, `behaviour-refuted`, `behaviour-clean`, `behaviour-unexpected`; collector failures are `harness-error`.

The `c*`, `r*` and `oracle-*` cases are reviewer probes (see each `note`). The `withheld-*` cases reproduce corpus sites adjudicated `confirmed-violation` whose proof is withheld (`scripts/corpus/reviewed-findings.json`).

## Context fields

Optional in `oracle.json`; they say what execution a verdict stands for:

- `scenario`: `application` (a real app's shape), `api-example` (a documented usage) or `constructed-misuse`.
- `requires`: a non-empty list of strings: the props, providers or boundaries the case depends on.
- `reaches`: a string: how the case shows the target path ran (assert-text step id, dev diagnostic, driver event).

They are descriptive: no verdict or exit code depends on them. A malformed one is that case's `harness-error`. Report rows carry them (`null` if absent) and the run prints cases per `scenario` (`unspecified` for none).

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
- `act` is optional: scripted clicks, each followed by a flush, before the event exercise. Observations share one page and run in list order.
- `observe` names exactly one kind: `{ "text": selector }`, `{ "window": "a.b" }`, `{ "identity": selector }` (`same`, `replaced` or `missing`), `{ "thrown": true }` (`threw` or `none`), or `{ "reflected": selector }` (below).
- `defect` and `correct` are the value that shows the defect and the value that shows correct behaviour: a JSON literal, or `{ "pattern": regex }` for `text` or `window`.
- Refused at load (that case is a `harness-error`, the others run): `behaviour` not a non-empty list; a `rule` that is neither a `codeRules` value nor in `docs/rules/`; an empty or uncompilable pattern; `defect` equal to `correct`; a literal side the other side's pattern accepts. Two patterns need `defectExample` and `correctExample`.

### Write-reflected observations

`{ "reflected": selector }` reads one element's settled `textContent` around each write. Its `writes` replace `act`: `{ "click", "expect": "lost" | "reflected", "value", "probe" }`, each click followed by a flush.

- `value` is the text the write sets. `reflected`: the text changed, now contains `value`, and did not before. `lost`: the text is unchanged. Anything else is `behaviour-unexpected`. Text is whitespace-normalised.
- `probe` is a separate node bound to the write's source; if it does not change, the write was a no-op (`behaviour-unexpected`). A refused direct store write shows in no binding: the handler fills the probe with the value it assigned.
- Each side of a write settles first: text and probe must hold across two `setTimeout(0)` ticks (500 ms at most), else `behaviour-unexpected`. Writes must be synchronous: a later effect reads as `lost`.
- `expect` gates the verdict: a write whose outcome differs is `behaviour-unexpected` (`harness-error`). The page shows the defect when any write is `lost`, the correct behaviour when every write is `reflected`.
- A click that throws, a missing element, or a step running past 2 s is `harness-error`, as is a page that never returns from a step.
- Refused at load: no `writes` or an empty list; a write not exactly `{ click, expect, value, probe }`, a blank `value`, or a `probe` equal to the observed selector; a click listed twice; `act`, `defect` or `correct` beside it; `writes` on another kind.

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
| `oracle-behaviour-no-direct-mutation` | SC2003 | text and reflected: the direct write is refused (`Ada`), the setter write shows (`Grace`) | `behaviour-confirmed` |
| `oracle-behaviour-primitive-in-directive` | SC6001 | window: the apply-phase effect runs again after its element is hidden (2, not 1); the control's does not | `covered-violation`; rows confirmed and clean (below) |
| `oracle-behaviour-jsx-no-duplicate-props` | SC8003 | reflected: five two-source elements each lose one write; two one-source controls reflect | `static-only` overall: five sites confirmed; the static site is unobserved (below) |
| `oracle-behaviour-components-return-once` | SC1004 | text: the early return stays on screen after the write (`logged out`) | `covered-violation` (dev `STRICT_READ_UNTRACKED` covers the finding); behaviour rows confirmed and clean |
| `oracle-dispatch-type-sets` | SC4001 | window: no dispatching-member family fires an event outside the checker's set (ADR 0361) and none fires `scroll` | `covered-violation`; both rows confirmed |
| `oracle-frame-covered-sc1004` | SC1004 | none: the frame pin (below) | `covered-violation`, empty `staticOnly` |

The `StaticText` site (static `textContent` beside children) has no write to show a loss, so it stays static-only.

The `oracle-behaviour-primitive-in-directive` control is `behaviour-clean` (its refuted `missing-owner` finding was removed, ADR 0345).

`NO_OWNER_EFFECT` is also claimed by `primitive-in-directive-application`, whose page names that warning.

## Frame attribution

A diagnostic covers a finding when the finding's lines include the diagnostic's site or one of its application frames, and its code claims the finding's rule. The runtime attributes a read to the call that led to it, so the read's own line is a frame, not the site. `oracle-frame-covered-sc1004` pins this: a `components-return-once` finding at a props read (`App.tsx:9`) is a frame of the `STRICT_READ_UNTRACKED` diagnostic sited at `Parent`'s call (`App.tsx:17`), and must not print as `static-only`.
