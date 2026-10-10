# Creates census refusals: measured families and limits

The retained 418-probe report ending **2026-09-12 06:14:11.062 UTC** has
**1,630 creates census-refusal rows, 368 distinct claim IDs, and 189 attributed
package/version/export identities**. Another 136 rows lack a usable node
identity; the inventory leaves their package attribution unknown. These counts
describe first refusals, not necessarily defects or recoverable closures.

| Family | Repeated rows | Distinct claim IDs | Attributed export identities |
| --- | ---: | ---: | ---: |
| Unknown accessor | 444 | 115 | 58 |
| Coercion | 280 | 33 | 24 |
| Initializer value tracing | 204 | 12 | 9 |
| Caller-supplied callable | 196 | 16 | 7 |
| Outside authenticated runtime | 143 | 66 | 28 |
| Iteration protocol | 96 | 17 | 7 |
| Standard invoker callback resolution | 56 | 10 | 6 |
| `instanceof` | 49 | 9 | 9 |

The script reports the remaining families too. Family claim/export counts need
not sum to their global totals: one claim can encounter different first
refusals in different attempts. Export counts exclude missing node identities;
they never substitute the root probe's package.

## Candidate investigations, not promised gains

- **Floating UI: module lookup followed by coercion.**
  `@floating-ui/utils@0.2.12`'s `getAlignmentSides`, `getExpandedPlacements`, and
  `getOppositePlacement` share the refusal at
  `dist/floating-ui.utils.mjs:3193..3245` (byte offsets):
  `oppositeSideMap[side] + placement.slice(side.length)`. They contribute
  **48 rows / three claims**. The map's literal values alone do **not** prove
  that a generic string lookup returns a primitive: unknown keys, inherited
  properties, and the key's admitted values need an exact proof. Keep the
  refusal unless that proof exists. The shortest root probe observed carrying
  these refusals was `@corvu-next/popover@0.1.5|solid2|only`, 105.553 seconds
  overall and 96.989 seconds in certification.
- **Motion: exact factory-returned callables.**
  `motion-utils@12.39.0`'s `anticipate` calls `backIn`, initialized by
  `reverseEasing(backOut)` in `dist/es/easing/back.mjs`; `backOut` itself comes
  from `cubicBezier`. This contributes **26 rows / one claim**. Resolving that
  exact factory chain and its captured callees is a possible verifier extension,
  but clearing this refusal does not guarantee the next census succeeds. The
  shortest root probe carrying it was `motion-solidjs@0.6.0|solid1|only`,
  228.949 seconds overall and 225.068 seconds in certification.

The additional **100 Motion DOM rows are not one uniform factory case**:
60 rows across `isWaapiSupportedEasing`, `mapEasingToNativeEasing`, and
`startWaapiAnimation` reach a memoized DOM feature probe; 20 `mixValues` rows
capture imported easing functions through `compress`; 20 `stagger` rows obtain
an easing function from caller options. They share the `motion-utils`
dependency, not one proven capture rule. Likewise, the 60 `reverseChain` rows
inside initializer value tracing eventually invoke caller callbacks; that
family total is not an implementation opportunity estimate.

## Depth exhaustion is absent from both measured reports

The script recognizes only the verifier's explicit diagnostic:

```text
^census refused: creates census exceeds ([0-9]+) local-recursion hops at
```

It found **zero rows / zero claims** in both the current report and the
screenshot's `report-0094.json`. Mentions such as a missing control-flow census
at depth 1 or a declaration outside authenticated runtime at depth 0 are
different refusals. These reports do not support an estimate of 174 closures
recoverable by raising the local-recursion budget.

## Provenance and reproduction

| Report | Finished UTC | Census-refusal rows | Claim IDs | Explicit depth-exhaustion rows |
| --- | --- | ---: | ---: | ---: |
| `rust/target/ecosystem-regression/report.json` | 2026-09-12 06:14:11.062 | 1,630 | 368 | 0 |
| `rust/target/ecosystem-regression/report-0094.json` | 2026-09-12 04:21:48.819 | 1,635 | 371 | 0 |

SHA-256 digests, in that order:

```text
ffd1d818e021479102c6d9636ad389b63da9dbd4eb692b04e9b3537920ca44f9
7b673664a95292e40b206f39bdb820cd7df80e2f690c533db62afb2aeb76de5c
```

Run the [read-only inventory](2026-09-12-creates-refusal-inventory.py) against
each retained report:

```sh
python3 docs/package-contract-v2/phase21/2026-09-12-creates-refusal-inventory.py rust/target/ecosystem-regression/report.json
python3 docs/package-contract-v2/phase21/2026-09-12-creates-refusal-inventory.py rust/target/ecosystem-regression/report-0094.json
```

Both commands completed successfully for this measurement. The script reads
only root `results`, selects `domain: creates` with a `census refused:` reason,
classifies explicit refusal wording, and emits counts plus representative
attributed exports and their shortest observed root probes. It never runs the
analyzer or certifier. No build, installation, ecosystem rerun, fixture,
snapshot, or public contract change was made for this inventory. The current
report covers the shared dirty worktree; its differences from the screenshot
are not attributed to one implementation change.
