# implementation-census-described-callables

The tracer for ADR 0145
(`docs/adr/0145-a-returned-callable-carries-its-own-call-claims.md`): exports
that hand their caller a fresh function literal propose `returns` closed over
one `return` whose output is a **described callable** -- the literal's own call
claims -- and the certifier proves or withdraws each from the literal's own
transcript.

The generated side is pinned by this fixture's `expected.json`, as every corpus
fixture is: the walk proposes a described callable for every export whose each
value-carrying completion is a function literal, and ADR 0113's `plain` for
`throughMutableBinding`, whose return is an identifier.

The certifier's side is pinned by
`the_described_callable_census_certifies_exactly_the_literals_own_claims` in
`rust/crates/solid-facts-backend/src/contract_certification.rs`, which plans the
same package with each export's described callables set by hand (the walk's
own answer for every export but `makeSilent`) and certifies against the real
producer with the synthesized described-callable veto as the only probe:

| export | result | why |
| --- | --- | --- |
| `createIdGenerator` | certifies `returns: [plain]` | `@solid-primitives/utils`' own bytes: default-library calls, captured plain bindings, a template string |
| `makeNoop` | certifies `returns: []` | an empty literal |
| `makeTicker` | certifies `returns: [plain]` | `++ticks` is a number by its grammar |
| `choose` | certifies `returns: [plain]` | both arms of the conditional are literals showing the same claims |
| `makeCounter` | withdrawn | `return count` is typed `number` from the binding's declaration, which an unchecked JavaScript write never widens, so the type is no proof |
| `makeSilent` | withdrawn | claimed `returns: [plain]`; its literal completes without a value |
| `invokesCaptured` | withdrawn | the literal calls a parameter it captured: its caller's code |
| `invokesOwnArgument` | withdrawn | the literal calls its own argument |
| `returnsObject` | withdrawn | the literal hands back an object |
| `readsCapturedMember` | withdrawn | the literal reads a member of a value the caller handed the export, which may run a getter |
| `throughMutableBinding` | not proposed a described callable | the returned value is a binding the body writes |

`index.d.ts` states each export's real result type. The census reads
`index.js`; a declared result that is not callable would refuse the positive
fact.
