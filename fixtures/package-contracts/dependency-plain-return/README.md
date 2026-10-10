# dependency-plain-return

ADR 0155's tracer, certified in the published graph by
`contract_certification::tests::a_return_of_a_composed_dependency_call_restates_its_closed_plain_return`.
Not a contract-corpus fixture: the test builds both archives from these files,
with the generator's own proposals (every domain open, `returns` from the value-
and valueless-completion walks), and certifies the root with `leaf.js` as its
one dependency node.

| root | returns | closes |
| --- | --- | --- |
| `forward.js` | `count(items)`, the leaf's `returns` closed plain | yes |
| `nothing.js` | `reset(items)`, the leaf's `returns: []` closed | yes |
| `conditional.js` | a conditional of two `count` calls | no |
| `bound.js` | a `const` holding a `count` call | no |
| `unclosed.js` | `widened(key)`, whose plain return the leaf withholds | no |

No `solid-js` stub: nothing here imports Solid, and the dialect is not what the
test measures.
