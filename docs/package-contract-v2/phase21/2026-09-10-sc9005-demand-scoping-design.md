# Scoping SC9005 to what the call site reads

- **Status:** design. Nothing implemented.
- **Date:** 2026-09-10.
- **Why now:** `reads` will not close in bulk
  ([veto design § 6](2026-09-10-reads-veto-observation-design.md)), and
  SC9005 demands it of **every** bound import. One permanently-unclosable
  conjunct therefore taxes every certified import forever. Of 29 rule
  identities, **6 need `reads` and 13 need `returns`** — and **8 need
  `returns` without needing `reads` at all**.

## 1. The pattern already exists: `callbacks`

`callbacks` is *not* one of SC9005's conjuncts, and it is not unreported. It is
demand-scoped, and the machinery is three pieces:

1. **A per-symbol openness query** —
   `Indexes::unknown_contract_callback_export(symbol)`
   ([indexes.rs:468](../../../rust/crates/solid-reactive-ir/src/indexes.rs)),
   which answers only when the binding's `callbacks` is open *or* `open_claims`
   carries the domain.
2. **A call-site condition** —
   [interproc.rs:1415](../../../rust/crates/solid-reactive-ir/src/interproc.rs)
   raises the obligation only when that call actually hands over a
   potentially-callable argument: not a proven non-callable literal, not a bare
   identifier. A call that passes no callback raises nothing.
3. **Deference elsewhere** —
   [execution_role.rs:1336](../../../rust/crates/solid-reactive-ir/src/execution_role.rs)
   reads the same query and explicitly does not duplicate the obligation:
   *"The call site already carries…"*.

So the shape is settled repository practice, not a new idea. The four SC9005
conjuncts are simply the ones that never got it.

## 2. What each conjunct's call-site demand is

`push_unknown_contract_claims`
([contracts.rs:859](../../../rust/crates/solid-reactive-ir/src/contracts.rs))
fires at **binding** time, in three places inside
`resolve_contract_imports_inner`, before anything knows how the import is used.
Each conjunct has a demand that is decidable at the site instead:

| conjunct | consumed by | the site condition |
| --- | --- | --- |
| `returns` | `source_discovery.rs:225, 621, 918`, `static_rules.rs:257` | a **call whose result is bound or used** — the accessor/store/async identity of a discarded result is consulted by nobody |
| `reads` | `local_access.rs` parameter/direct read handling, `interproc.rs` read summaries | a call in a position the reactive-read analysis classifies |
| `creates` (`ownerRequirements`) | `indexes.rs:539` → `owners.rs` | a call whose **owner context is analyzed** |
| `asyncBehavior` | `owners.rs:1890` | derived from `returns`; folds into that row |

`returns` is the first slice: it has the most rule consumers (13, of which 8
need nothing else), its consumers all funnel through `.known()`, and its site
condition — *is the result used?* — is the cheapest of the four to decide.

## 3. The change, per slice

1. Add `Indexes::unknown_contract_returns_export(symbol)`, a byte-for-byte
   analogue of the callbacks query against `summary.returns` /
   `ClaimDomain::Returns`.
2. Raise the obligation at the sites that consult the return identity, under
   the condition above, into `contract_consumer_obligations` — the same vector
   the callbacks obligation uses.
3. **Only then** drop `returns` from `push_unknown_contract_claims`.

Steps 2 and 3 must land together. Dropping the conjunct without the site
obligation does not reduce noise — it **loses** a fail-closed answer, which is
strictly worse than the noise it removes. That is the one way this change can
do harm, and it is why each domain is its own slice with its own fixture.

## 4. What it is expected to buy

Nothing for a consumer that genuinely uses every domain. For the common case —
an import called once, result used, no callback passed, owner not in question
— it should reduce four conjuncts to one, and for the 8 rules that need only
`returns` it removes the `reads` tax entirely.

That is a prediction, and it should be **measured the way the findings-delta
was**: run the corpus before and after, count SC9005 by
`analysisContext`, and report the change per domain. If the count does not
move, the site conditions are too weak and the slice should be reverted rather
than kept for tidiness.

## 5. Why this is not being landed in the same pass as the design

Gate 3 was landed on a "this is small, the machinery already refuses it"
judgement that turned out to be wrong, and was withdrawn the same day. This
change is four domains across five modules, and its failure mode is silent
under-reporting rather than a loud test failure. It wants a fresh pass, one
slice at a time, each with the before/after count in § 4.

## 6. Attempted (2026-09-10): the `returns` slice needs a fact that does not exist

Two findings from starting it, both of which change § 3 rather than the
direction.

### 6.1 There is no "the result is discarded" fact

`CallFact` ([ast/mod.rs:219](../../../rust/crates/solid-facts/src/ast/mod.rs))
carries `span`, `callee`, `direct_callee`, `type_arguments`, `arguments`,
`static_callee`, `owned_write_option`. Nothing says whether the call sits in
expression-statement position.

The site condition "*the result is bound or used*" therefore has no fact to
read. Its sound complement — a call **is** an expression statement, so the
result is discarded — is one bit and is the right one to add, because
everything else is a use. That is an extraction addition in `solid-facts`,
not a rethread of the consumer, but it is a producer change with its own
review rather than a consumer-side slice.

The three sites that consume a returned identity
(`source_discovery.rs:225, 621, 918`) already work the other way round: they
start from a *declaration initializer* and ask `ast_index.call_by_span(...)`.
That index proves "this call's result is bound"; it cannot enumerate the calls
whose result is not.

### 6.2 `returns` is demanded from two places, not one

`asyncBehavior` derives from `returns`, and
`computation_is_async_with_contracts`
([owners.rs:1886](../../../rust/crates/solid-reactive-ir/src/owners.rs))
consults it at an **argument** span:

~~~rust
let contracted_async_at = |span| { … binding.summary.async_behavior.known() … };
if contracted_async_at(argument) { return true; }
~~~

So a binding passed as the callback of a computation — `createEffect(imported)`
— demands `returns` while being neither called nor having its result used. A
site condition written only from § 3's table would drop the obligation there
and silently under-report.

The demand for `returns` is therefore: *the result of a call to it is used*,
**or** *it appears where a computation's async behavior is analyzed*. Both
halves have to be in the first slice.

### 6.3 What this means for the design as a whole

The pattern in § 1 is still right, and `callbacks` still proves it works. What
§ 3 understated is the prerequisite: demand scoping needs **call-site use
facts** that the consumer does not currently have, and each domain's demand is
a set of positions rather than a single one. The order should be:

1. Add the discarded-result bit to `CallFact` and its extraction, with its own
   fixtures. Nothing about SC9005 changes yet.
2. Enumerate each domain's demand positions the way § 6.2 did for `returns` —
   by reading every consumer, not by reasoning from the rule matrix.
3. Only then the per-domain slices, each with the before/after SC9005 count.

Nothing was landed. Step 1 is the next concrete task, and it is a
`solid-facts` change rather than a contracts one.

## 7. Step 1 landed (2026-09-10): `CallFact::result_discarded`

~~~rust
/// Whether this call's own result provably reaches nothing: the call
/// **is** the expression of an `ExpressionStatement`.
///
/// One direction only. `true` proves the result is discarded; `false`
/// proves nothing and is the default …
#[serde(default)]
pub result_discarded: bool,
~~~

Recorded in both construction sites (`visit_call_expression` and
`visit_new_expression`) by span equality against the expression the innermost
enclosing `ExpressionStatement` discards. Span equality rather than a depth
counter, so `f(g())` as a statement answers `true` for `f(...)` and `false`
for `g()` with no unwinding.

**The trap, and the reason the negative half of the test is the load-bearing
half.** Oxc models a concise arrow body as a body holding one
`ExpressionStatement`, so `() => f()` and `{ f(); }` are the *same node shape*
— and the first returns its value. The first implementation marked
`const concise = () => f()` as discarded, which is precisely the direction
this bit may never be wrong in. `Collector::concise_arrow_bodies` now excludes
them, and the test pins nine cases:

| source | `result_discarded` |
| --- | --- |
| `f();` | true |
| `const bound = f();` | false |
| `await f();` | false — the `await` consumes it |
| `void f();` | false — the unary consumes it |
| `f(f())` as a statement | true (outer), false (inner) |
| `if (flag) { f(); }` | true |
| `const concise = () => f()` | **false** |
| `new Date();` | true |

**Nothing consumes it yet**, deliberately: no SC9005 behaviour changed, and
no snapshot moved. facts-lib 83, ir-lib 236, backend-lib 450,
contract-process 13/37, diagnostics green, coverage 94 projects / 547
findings unchanged, contract corpus 97 unchanged, clippy `--all-targets`
clean, fmt clean.

Step 2 of § 6.3 — enumerating each domain's demand positions by reading every
consumer — is next, and is still a reading task rather than a coding one.

## 8. Step 2 (2026-09-10): every consumer, read

Every site in the analysis crate that reads one of SC9005's four conjuncts.
Generator-side uses (`main.rs`, `inferred_contract.rs`), model validation
(`lib.rs:1269`, `:1335`) and the generator's own composed-owner rewrite
(`contracts.rs:2205`, `:2217`) are excluded: they do not consume a binding on
behalf of a rule.

### `returns` — five sites, one condition

| site | what it is |
| --- | --- |
| `source_discovery.rs:225` | an inner call reached while computing an effective return |
| `source_discovery.rs:621` | a declaration whose **initializer** is a call |
| `source_discovery.rs:918` | a name bound from a call's tuple element |
| `source_discovery.rs:1011` | a **member access on a call's result** — `call().prop` |
| `static_rules.rs:248` | a `call_initializer` |

All five are "the result went somewhere". `!result_discarded`
(§ 7) covers every one of them and over-approximates, which is the safe
direction.

### `asyncBehavior` — one site, and it is *not* a result use

| site | what it is |
| --- | --- |
| `owners.rs:1886` | `contracted_async_at(argument)` — the binding at an **argument** span |

`createEffect(imported)` demands `returns` through this path while the binding
is neither called nor has a result. Confirmed as § 6.2 predicted; it must be
the second half of the `returns` slice's condition.

### `creates` (`ownerRequirements`) — two sites, and the condition is nearly vacuous

| site | what it is |
| --- | --- |
| `owners.rs:789` | a call whose owner context is being decided and is not already root-owned |
| `owners.rs:1121` | a call not inside an owner-providing region |

Both key on `lookup.callee_symbol(file, call.callee)`, so the demand is
essentially **"the binding is called"**. Scoping buys nothing here except for
an import that is never called.

### `reads` — one consumption point, two maps

| site | what it is |
| --- | --- |
| built at `source_discovery.rs:1367-1392` | `contract_reads` (kinds `accessor`/`store-path`) and `contract_parameter_reads` |
| consumed at `local_access.rs:612`, `:643` | at a **call**, gated on `!inside_non_component_function` |

Demand: the binding is called **outside a non-component function**. Broad, but
not vacuous — a call in a plain helper sheds it.

## 9. What step 2 changes about the expected yield

The § 4 prediction was "four conjuncts down to one for the common case". Read
against the actual sites, that is too optimistic:

| conjunct | sheds the obligation when |
| --- | --- |
| `returns` | the call's result is discarded **and** the binding is never a computation argument |
| `reads` | the binding is only called inside non-component functions, or never called |
| `creates` | the binding is never called |

For the shape that motivated this — an imported primitive called for effect
inside a component, e.g. `createEffect(imported)` or `render(...)` — `creates`
and `reads` both stay demanded, and `returns` stays demanded through the
argument path. **That case sheds nothing.**

Where it does pay: a binding called for effect in a plain helper (sheds
`reads`), and a binding whose result is discarded and never passed as a
computation argument (sheds `returns` and `asyncBehavior`).

**So the honest expectation is a partial reduction on some imports, not a
collapse to one conjunct.** That is worth having — it is the difference
between an unconditional tax and a conditional one — but the measurement in
§ 4 should be run before the slice is called a success, and the bar should be
set from this table rather than from § 4's guess.

The order in § 6.3 stands. Step 1 is landed; step 3's first slice is
`returns`, whose condition is now exactly:
`(!result_discarded at some call to the binding) || (binding appears at a computation argument span)`.

## 10. The measurement vehicle (2026-09-10)

§ 4 asked for a before/after SC9005 count. **There is no baseline to count.**
Measured: every SC9005 in the repository is
`obsolete-policy1-receipt: policy 1 cannot authorize analyzer semantics`
(6 live across the contract fixtures, 37 in snapshots with the context not
retained). That path fires *before* `push_unknown_contract_claims`, so a
demand-scoping change would move no existing number.

So the bar changed from a count to a **control**, which is the stronger test
anyway: it pins the condition instead of aggregating over it.

### The control

`fixtures/reactive-ir/package-return-consumer` now exports two bindings whose
contracts are **byte-identical** — the same declared signature and the same
contract summary id — differing only in where the consumer puts them:

| binding | use | § 8 says |
| --- | --- | --- |
| `createCount` | result bound at module scope | four consumers can reach its `returns` |
| `createLabel` | `createLabel();` as a whole statement, never an argument | **no consumer can reach its `returns`**; `CallFact::result_discarded` is `true` at its only call |

Any difference in what is reported about them is attributable to the use
position alone, because nothing else differs.

### The assertion, written before the slice

`contracts_process::contract_closure_process::an_open_domain_is_reported_against_every_binding_including_one_no_consumer_reads`
mints a policy-2 receipt with `returns` reopened and asserts **both** bindings
are reported today:

~~~rust
assert!(reported("createCount"), "the bound result keeps its obligation");
assert!(reported("createLabel"), "PRE-SLICE STATE. … When `returns` demand
    scoping lands this assertion inverts to `!reported(\"createLabel\")`;
    until then, its passing is what says the obligation is unscoped");
~~~

Written before the slice deliberately, so the change lands as a visible
inversion in an assertion rather than as a claim in a commit message. If a
slice cannot flip it, the slice does not work.

### Cost of the vehicle

One export added to the fixture package (declarations, contract summary map,
catalog resolver answer), one consumer function, and one snapshot line: the
fixture now reports two policy-1 obligations instead of one, which is the
correct per-export count. facts 83, ir 236, dialect 63, contract-process
14/37, coverage 94 projects / 548 findings, corpus 97, clippy and fmt clean.

The vehicle is reusable: the `reads` and `creates` slices need the same two
bindings in different positions, and can add their own controls to the same
fixture.

## 11. The `returns` slice, landed (2026-09-10)

`returns_shed_symbols(facts, entities)` in `contracts.rs`, computed once per
project, and one gate on the conjunct.

### The predicate

Both § 8 demands reduce to the same question about a *reference*: is it the
callee of a call that throws its result away, or is it anything else? So a
symbol sheds `returns` only when it has references and **every** one of them
is a discarded call's callee. An argument, a member base, a re-export, or a
reference that resolves to no symbol all keep it.

Two properties make that decidable and safe:

- **File-local.** An import's binding symbol is file-local, so every reference
  to it is in the file that imported it. "Every one of them" is answerable
  without a project-wide alias analysis.
- **Fails toward reporting.** Shedding wrongly drops a fail-closed answer with
  no test going red, so the predicate is written so that anything it cannot
  classify keeps the obligation. The argument path § 6.2 warned about is
  covered by that default rather than by a special case: a reference used as
  an argument is simply not a discarded callee.

### The control flipped

`an_open_returns_is_reported_only_where_a_consumer_can_read_it` was written in
§ 10 asserting **both** bindings were reported. With the gate in place it
failed exactly once, on `createLabel`, and was then inverted. `createCount` —
byte-identical contract, result bound — still reports. That is the whole
evidence for the slice, and it is a diff in an assertion rather than a claim.

### Scope

`returns` only. `creates` and `reads` stay unconditional: § 9 measured their
demand as "the binding is called" and "called outside a non-component
function", so scoping them buys almost nothing and would spend the same
silent-under-report risk for it.

### Verification

facts 83, ir 236, backend-lib 450, contracts_process 14, dialects 15,
diagnostics 37, coverage 94 projects / 548 findings unchanged, corpus 97
unchanged, clippy `--all-targets` clean, fmt clean. 77 insertions in
`contracts.rs`.

No existing finding moved, which is expected and is *not* evidence the slice
does nothing: every SC9005 in the corpus is the policy-1 rejection (§ 10), so
the only place the gate can be observed is the minted-receipt control.
