# forwarded-prop-backing

**Claim (ADR 0216).** A prop a parent forwards (`<Inner value={props.value} />`) is exactly as reactive in the child as the parent's own prop: reactive when some call of the parent passes a reactive value, unresolved when the parent's prop is unresolved or the parent escapes enumeration, static otherwise. Before, every forwarded prop was reactive, so a plain value forwarded once became a proven strict read that Solid never raises.

| Component | Body read | Finding | Why |
| --- | --- | --- | --- |
| `PlainInner` | `props.value`, `props.label` | none | `PlainOuter`'s only tag passes `{ text: "plain" }` and `"static"` |
| `LiveInner` | `props.count` | `strict-read-untracked` violation | `LiveOuter`'s only tag passes `count()` |
| `EscapingInner` | `props.count` | `strict-read-untracked` uncertifiable | `EscapingOuter` is exported |
| `WholeInner` | `props.source` | `strict-read-untracked` uncertifiable | `WholeOuter` passes its whole props object |
| `MergedInner` | `props.value` | `strict-read-untracked` uncertifiable | forwarded from a `merge` view, not the parent's props |
| `SuffixInner` | `props.value` | `strict-read-untracked` uncertifiable | forwarded `props.box.b`: a static head does not certify `.b` |
| `StoreInner` | `props.state` | `strict-read-untracked` uncertifiable | a store passed whole: reading the prop reads no key |
| `ChildrenInner` | `props.children` | `strict-read-untracked` uncertifiable | several children compile to memo accessors |

Runtime: the `PlainInner` shape was run in Chrome on rc.13 (probus-hk install): no `STRICT_READ_UNTRACKED`, while the previous checker reported both reads as violations.

`solid-js.d.ts` is copied from `props-callers`, with `merge` and its types added, copied from `closed-export-member-dispatch` (byte-faithful to `@solidjs/signals@2.0.0-rc.13`). The second group of cases is the 2026-10-06 review's (`rust/target/research/review-forwarded-prop.md`): each would otherwise be a wrong clean result or a false positive.
