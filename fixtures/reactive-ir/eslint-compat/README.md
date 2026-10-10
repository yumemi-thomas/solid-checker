# The eslint-era surface, answered by the Solid 2.0 catalog

Ported off its 1.x stub during the Solid 1.x retirement (step 1). Seven
findings before, seven after, and five of them are the same claim at the same
span under a rule name that lost its `v1/` prefix. The other two moved, and the
movement is **deliberate dialect behaviour**, not a regression. Both halves are
worth stating, because this fixture is now the only pin for either under 2.0.

| | 1.x | 2.0 |
| --- | --- | --- |
| `jsx-no-duplicate-props` SC8003 at 1618, 2213 | reported | **not reported** |
| `reactive-handler-frozen` SC1007 at 3924, 3938 | not reported | **reported** |

**The duplicate-props pair.** `upstream_compat/solid1x_syntax.rs` folds an
intrinsic element's props into DOM template slots only when the dialect
`carries_eslint_era_rules()` — 1.x's compiler makes `onSave`, `on:save` and
`attr:class` collide in one slot, and 2.0's does not. Two spellings that are
one slot under 1.x are two props under 2.0, so there is no duplicate to report.

**The frozen-handler pair.** `Dialect::static_event_values_are_attributes()` is
true for 1.x: a statically known string or number in a native `on*` position is
emitted as an *attribute*, not installed as a listener, so the handler-value
rule must leave it alone. 2.0 installs it as a listener, which is exactly the
frozen-handler shape the rule exists to report.

Neither difference is a property of this source. Both are the dialect answering
a question about the same bytes, which is why the fixture is kept rather than
deleted with the 1.x catalog: after the v1 dialect is removed these two
mechanics have no other regression pin.
