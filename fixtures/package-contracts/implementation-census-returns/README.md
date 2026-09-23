# implementation-census-returns

The end-to-end tracer for the `returns` implementation census
(`docs/adr/0035-returns-census-for-valueless-completion.md`): a consuming
package whose function exports complete in every way the census must tell
apart, and a certifier that proves `returns: []` for the valueless ones and
refuses it by name for the rest.

It is a generator-corpus fixture, so `expected.json` and
`expected-proposal.json` pin the generator's side: the four exports whose
valueless-completion walk is clean — `bareCompletion`, `earlyBareReturn`,
`bareReturnInLoop`, `nestedReturnsValue` — propose `returns: []` beside
`creates: []` (`closed: ["creates", "returns"]`, `proposedClosures: ["creates",
"returns"]`), and the plan carries one `{kind: "call", domain: "returns"}`
closure candidate for each. Of the five others, `asyncVoid` and
`generatorVoid` propose `creates: []` only: an `async` modifier or a generator
asterisk hands the caller a promise or an iterator, and silence is "do not
propose". The three whose walk saw a value-carrying return or an expression
body — `returnsValue`, `expressionArrow`, `valueReturnInLoop` — propose, since
ADR 0113, `returns` closed over one `return` whose output is `plain`, beside
`creates: []`. That is the walk's other positive answer, and it claims nothing
the walk can see: whether each value is a primitive is the census's to decide
from the producer's types, and `../implementation-census-primitive-returns` is
where it does. (`expressionArrow` is a `const` arrow; the generator binds an
anonymous callable's walk verdicts through its declarator, so it proposes like
its siblings.) No export's `returns` knowledge is a sidecar candidate unless one
of the two walks answered, because the generator's `Known(None)` means "no
reactive return described", not "yields nothing".

What the certifier does with each candidate is pinned by
`the_probe_gate_tracer_returns_census_*` in
`rust/crates/solid-facts-backend/src/contract_certification.rs`, which hands
the census a hand-closed `returns: []` for every export, including the ones the
generator never proposes:

| export | census | why |
| --- | --- | --- |
| `bareCompletion` | certifies | the body falls off its end |
| `earlyBareReturn` | certifies | a bare `return;` and the end of the body |
| `bareReturnInLoop` | certifies | a bare `return;` under a `reachability-lower-bound` construct, reach `unknown` |
| `nestedReturnsValue` | certifies | the value-carrying return is a nested callable's completion |
| `returnsValue` | refuses | `value-carrying completion`, reach `reachable` |
| `expressionArrow` | refuses | the expression body is a value-carrying completion over the body |
| `asyncVoid` | refuses | `async implementation`: every completion hands the caller a promise |
| `generatorVoid` | refuses | `generator implementation`: every completion hands the caller an iterator |
| `valueReturnInLoop` | refuses | `value-carrying completion`, reach `unknown` |

A value-carrying return the producer proves unreachable is admitted with a
`value-unreachable` witness; that premise is pinned by the unit test
`returns_census_admits_only_bare_or_unreachable_completions` on a synthesized
transcript, because the generator's walk — which has no reachability — would
never propose such an export.

The recipes ship with the fixture. `probe-recipes/valueless.mjs` calls every
certifying export with a finite sample and emits the corpus's falsification
marker if any call hands back a value; `probe-recipes/refused-export.mjs`
exists so the refusal tests plan a candidate the census actually reads instead
of one withheld for want of a recipe.

`node_modules/solid-js` is the audited Solid 2.0 stub that selects the dialect
and satisfies the peer dependency; it is byte-identical to the one in
`../implementation-census-creates` and carries the same `.gitignore` exception.
