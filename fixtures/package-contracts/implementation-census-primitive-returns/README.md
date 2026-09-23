# implementation-census-primitive-returns

The end-to-end tracer for ADR 0113
(`docs/adr/0113-a-returns-closure-over-a-primitive-completion.md`): a consuming
package whose function exports each hand their caller a value, and a certifier
that proves `returns` closed over one `plain` return for the ones whose every
completion is a primitive, and withholds the claim by name for the rest.

This is a native certification fixture, not a generated-main snapshot, like
`../returned-parameter-identity`. The test builds each export's summary exactly
as the generator does for a plain function its valueless-completion walk
declined on a value (`returns` described as nothing,
`returns_value_completion` set) and runs it through the generator's own
normalization, so the proposal it certifies is the one the emit boundary
publishes. The generated side is pinned by `../implementation-census-returns`'
`expected.json`, whose three value-yielding exports propose the same closure.
Every other domain is left open, so the only closure candidates are the ones
under test, and no hand recipe ships: each is served by ADR 0113's synthesized
primitive-return veto.

The certifier's side is pinned by
`the_primitive_returns_census_certifies_exactly_the_primitive_completions` in
`rust/crates/solid-facts-backend/src/contract_certification.rs`:

| export | result | why |
| --- | --- | --- |
| `trueFn` | certifies | an expression body typed `boolean` |
| `voidFn` | certifies | `void 0` is `undefined`, a primitive, and still a value-carrying completion |
| `clamp` | certifies | `Math.min(…)` is `number` from the default library, whatever the arguments are |
| `label` | certifies | two reachable returns, both strings |
| `isObject` | certifies | a comparison is a boolean whatever its operands are |
| `sign` | certifies | a bare `return;` beside a number; `value > 0` makes the producer premise the transcript, and the return sites are still read off the original program |
| `box` | refused | an object; the producer does not prove the completion primitive |
| `passThrough` | refused | the caller's argument, which an unannotated parameter types `any` |
| `annotatedBox` | refused | `@returns {number}` over `return {}`: the checker takes the annotation at its word, so the completion *is* stated primitive, and the return site's own type is what refuses |
| `add` | refused | `a + b` always yields a primitive at run time, but TypeScript types it `any` over `any` operands; the named approximation of the census |
| `widened` | refused | the body is a primitive and the declaration promises `number \| object`; the operation's positive fact refuses the declared result |

Every refusal withdraws the `return` operation itself (`operation census
refused: …`): the operation's positive fact reads the same evidence as the
closure census, so the document stops stating a plain return and the domain
opens with it. The census premises one by one — the completion form, an
incomplete control-flow census, a dead object-typed return, `never`, a callable
value — are pinned on synthesized transcripts by
`primitive_returns_census_requires_the_completion_and_every_site_primitive` in
`type_facts.rs`.

What a consumer does with the certified claim is the other half, pinned by
`../../reactive-ir/package-plain-return-consumer` on an authorized contract: a
closed plain return leaves nothing open at the import.

No `node_modules/solid-js` stub: the package imports nothing and declares no
peer dependency, and nothing here depends on dialect vocabulary.
