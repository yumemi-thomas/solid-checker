# ADR 0103: an export that is a default-library member

Status: accepted (2026-09-14). Handshake protocol 57. Closes **148** recipe-less
`reads` rows on the corpus (1,884 -> 1,736) with no row below baseline; the
188-row estimate and the 40-row shortfall are both accounted for under
"Measured result".

## The shape

Utility packages re-export built-ins by identity:

~~~js
// @solid-primitives/utils@6.4.1
export const keys = Object.keys;
export const entries = Object.entries;

// @floating-ui/utils@0.2.12
export const floor = Math.floor;
export const max = Math.max;
~~~

The runtime value of each is fully determined — it *is* the built-in — and yet
every one of them leaves its call transcript open, for two different reasons:

- `keys` and `entries` open with **`callSignatureNotUnique`**. `Object.keys`
  and `Object.entries` are overloaded, so the checker cannot select "the"
  signature, and the transcript returns before it ever looks for a body.
- `floor` and `max` open with **`implementationUnavailable`**. Their only
  declaration is a body-less signature in `lib.es5.d.ts`, so the alias hop
  finds nothing to census.

Neither refusal is a statement about the runtime value. Both are statements
about what the checker could say from the declaration, and the runtime value
is not in doubt.

Measured on the 2026-09-14 pin: 188 recipe-less `reads` detail rows across 14
(case, export) pairs — `entries` 62, `keys` 62, `floor`/`max`/`min`/`round` 16
each. That is the largest single premise on a 1,884-row frontier, and still
only 10% of it.

## The two-reason correction

The 2026-09-13 depth plan described this premise's refusal as
`implementationUnavailable` alone: "the producer's alias hop lands on
`lib.es2017.object.d.ts`, which has no body to census". A census run on
2026-09-14 showed that is true of the `Math.*` aliases and **false of the
`Object.*` ones**, which never reach the implementation path at all.

A premise built to the plan's description would have closed
`floor`/`max`/`min`/`round` — 64 rows — and left `keys` and `entries` exactly
where they were, which is 124 of the 188. The fact therefore has to be stated
on both sides of the signature check, and the consumer has to accept either
reason.

## What the producer states

`DefaultLibraryAlias { container, member }`, on the export's implementation
transcript, when *all* of the following hold:

- the binding is a variable declaration with an initializer, never assigned
  anywhere in the file;
- the initializer, after identity-preserving unwrapping, is a **property
  access whose object is a plain identifier** — a computed member states
  nothing, because which member it reads is not syntactically fixed;
- the container symbol and the member symbol are both declared only in
  default-library files, and the member is declared on that container's own
  interface (`ObjectConstructor` for `Object`, `Math` for `Math`), so a local
  `const Object = {…}` shadowing the global states nothing;
- neither the container nor the member is written, deleted, or used as
  anything but a read or a call anywhere in the file, by the same
  whole-file conservatism `immutableAliasLibrarySourceIsStable` already
  applies to callee aliases.

The fact is stated **beside** the existing open reason, never instead of it.
The transcript stays open; a consumer that does not recognize the member keeps
refusing exactly as protocol 56 did.

## What the certifier decides

The producer states identity and stops there, deliberately. `Object.keys`
invokes no callable its caller supplied; `Array.prototype.map` invokes one per
element. **The producer states those two identically**, because the syntax is
identical, so deciding which members may close a domain is a separate,
reviewed act and it lives with the certifier.

`REVIEWED_DEFAULT_LIBRARY_ALIASES` holds the members audited against three
questions, admitted only when all three answer no:

1. Does it invoke a callable its caller supplied? (`Array.prototype.map`,
   `Object.defineProperty` with accessor descriptors — excluded.)
2. Does it read anything but its arguments' own properties? Reading an
   argument's own property — which `Object.entries` does, running a getter the
   caller installed — is the *caller's* read under ADR 0034. Reading ambient
   state is not. (`Date.now`, `Math.random` — excluded.)
3. Can it run caller code through a trap? (`JSON.stringify` calls `toJSON` —
   excluded.)

A member outside the table leaves the fact unread and the transcript refusing.
Growing the table means answering the three questions, not noticing that a
corpus row would close.

## Which domains close, and which does not

`reads`, `creates` and `callbacks` close on an **empty** enumeration. A
reviewed member invokes no caller callable, builds no reactive source, and
reads nothing but its arguments' own properties.

`returns` is deliberately **not** closed. These members do return values —
`Object.keys` returns a fresh array — and whether that value is one the
`returns` domain denies is a different question this fact does not answer.
Leaving it open costs nothing: no corpus row's `returns` is withheld behind
this fact.

## The veto

`Observation::DefaultLibraryAlias(index)` synthesizes a module that observes
`Object.is(subject, Container.member)` in the probe realm and emits
`alias-identity` on inequality. This is an **exact identity witness**, not a
sample: it decides the whole claim in one comparison, where a sampled veto can
only probe behaviour one tuple at a time. The index is into the reviewed
table, so a veto can never name a member the certifier has not audited.

It cannot see a realm whose built-in differs from the probe realm's, and it
says nothing about what the member *does* — that is the reviewed decision
above, not this observation's.

## Measured result: 148 rows, and why not 188

Full corpus, release binary, `--timeout 1800`, compared against the pin before
it was moved: **no row below baseline, no status move**, and the recipe-less
`reads` frontier falls from **1,884 to 1,736**. Wall 585 s.

| export | rows before | after |
| --- | ---: | ---: |
| `@solid-primitives/utils@6.4.1` `entries` | 42 | **0** |
| `@solid-primitives/utils@6.4.1` `keys` | 42 | **0** |
| `@floating-ui/utils@0.2.12` `floor` | 16 | **0** |
| `@floating-ui/utils@0.2.12` `max` | 16 | **0** |
| `@floating-ui/utils@0.2.12` `min` | 16 | **0** |
| `@floating-ui/utils@0.2.12` `round` | 16 | **0** |
| **total** | **148** | **0** |

No `(package, export)` pair gained a recipe-less row. Both refusal reasons the
premise was built to lift are represented: `callSignatureNotUnique` for the
`Object.*` half, `implementationUnavailable` for the `Math.*` half.

### The 40-row shortfall is the stability check, working as designed

The estimate was 188. The 40 rows that did not close are all
`@solid-primitives/utils@**7.0.0-next.4**` — `keys` 20, `entries` 20 — and the
cause is one line of that bundle:

~~~js
const defaultEquals = Object.is.bind(Object);   // dist/index.js:18
const entries = Object.entries;                 // :141
const keys = Object.keys;                       // :145
~~~

`immutableAliasLibrarySourceIsStable` requires that the container is not used
as anything but a read or a call **anywhere in the file**. Passing `Object` as
an argument to `.bind()` is neither, so the whole file refuses and no alias
fact is stated. The 6.4.1 bundle does not spell `defaultEquals` that way, which
is why the same two exports close there.

This is an over-refusal in the narrow sense — a `this` argument to `bind`
cannot rewrite `Object.keys` — but relaxing whole-file conservatism is a
separate reviewed decision about what an argument position may do to a
container, not a detail of this premise. Recorded here, not worked around.

### What was eliminated on the way, so it is not re-run

A stale producer (binary stamp), the published-declaration shape
(`…ThroughAPublishedDeclaration` builds the real `.js` + generic `.d.ts` pair
and the fact is stated), and one real defect that is fixed here: the batch
path's synthesis entry guard admitted an export only on a unique call signature
or a `NotCallableValue` fact, so an ADR 0103 export never entered the pass.

One process note worth more than the bug it caused. An earlier draft of this
section reported the premise as closing **zero** rows. That reading came from
`benchmarks/ecosystem/report.json` as pinned at 12:48, and the protocol 57
producer was not built until 14:02 — the pin could not contain the change whose
effect was being read off it. A premise's reach is only ever measured by a
corpus run made *after* the binary that implements it, and a two-pass scaffold
run is not a substitute: `pass2.mjs`'s first pass passes no
`--probe-recipe-corpus` at all, so synthesis is disabled by construction there
and every candidate reports `no recipe in corpus` whatever the census decided.

## A gap this records rather than papers over

The end-to-end pair — one alias the table admits, one it does not, both
certified through the same fixture — could not be built.
`fixtures/package-contracts/value-exports` already carries `entries`, which
now closes; adding a *second* default-library alias beside it (tried with
`JSON.stringify` and with `Object.getOwnPropertyNames`) refuses in the
unrelated `recursive-value-shape` family — "export root is not compiler-proved
non-callable and non-constructable" — which fails the whole fixture row for a
reason that has nothing to do with either premise.

So the admitted half is end-to-end (`a_reviewed_default_library_alias_closes_by_identity`)
and the refused half is a unit test over the table
(`the_reviewed_default_library_alias_table_admits_only_audited_members`). The
producer side of the refused half *is* covered end-to-end, in
`TestDefaultLibraryAliasStatesIdentityForBuiltInReExports`, which pins that a
computed member, a shadowed container, a written binding, a written container
member, a local namespace and an identifier alias all state nothing.

What is not covered by any test today: a certification in which the producer
states an identity the certifier's table refuses, proving the census arm
returns "not reached" rather than closing. Closing that gap needs the
`recursive-value-shape` interaction understood first, and it is a real
weakness in the safety half of this ADR.

## Amendment, 2026-09-23: the generator proposes all three domains

"Which domains close" above was true of the certifier and not of the generator.
The alias census closes `reads`, `creates` and `callbacks`, but the generator
only ever *proposed* `reads`: an alias has no body, so no walk clears its
`creates`, and the summary raised for a callable value leaves `callbacks` open.
`@solid-primitives/utils`' `entries` and `keys` therefore kept `creates` and
`callbacks` open at every import, 59 consumer sites
(`phase22/2026-09-23-what-holds-an-import-open.md`), with a census arm ready to
close both.

The generator now marks the shape, from syntax only, as
`ContractExport::member_alias_initializer`: a `const` binding of one
identifier, found by symbol so `export { entries }` reaches its declaration,
whose initializer is exactly a non-computed member access. For such a summary
it proposes `creates: []` and `callbacks: []` beside `reads: []`. The mark
decides nothing: syntax cannot say whose member it is, so a package's own
`helpers.run` is proposed too, and this census refuses it for want of a stated
identity. The raised summary's own open `callbacks` is not changed, only what
the proposal offers. `returns` stays unproposed, as above.

`fixtures/package-contracts/member-alias-proposals` pins which shapes are
marked, and `a_reviewed_default_library_alias_closes_by_identity` now certifies
all three domains for `value-exports`' `entries`.

## Amendment, 2026-09-24: `callbacks` stays open for a member that runs caller code

Question 2 above answered two domains at once, and only one of them correctly.
A getter the caller installed is the caller's code, so what it reads and builds
is the caller's (ADR 0034): `reads` and `creates` close. But running it is this
export invoking a callable its caller supplied, which is exactly what
`callbacks` enumerates. `semantic-model.md` § callbacks names "a getter or
setter reached by property access" and "a coercion reaching
`Symbol.toPrimitive`, `valueOf`, or `toString`", and the ADR 0100 census refuses
both on a parameter in ordinary code. So the table closed `callbacks: []` on
members the census would have refused written out by hand, and the 2026-09-23
amendment made it reachable: `entries` certified `callbacks: []` although
`entries({ get a() { … } })` runs the getter.

`DEFAULT_LIBRARY_ALIAS_ARGUMENT_REACH` now lists the reviewed members that run
caller code through an argument, with how, and the census refuses `callbacks`
for them by name:

- `Object.entries` and `Object.values` read each own enumerable property's
  value, which runs a getter;
- every `Math` member converts its arguments with ToNumber, which runs an
  object's `Symbol.toPrimitive`, `valueOf` or `toString`.

`reads` and `creates` still close for them. `Object.keys` reads descriptors and
never values, so an ordinary object runs nothing; `Array.isArray` reads a
proxy's target without a trap; `Number.isFinite`, `Number.isInteger`,
`Number.isNaN` and `Object.is` never convert. Those keep `callbacks: []`. A
`Proxy` argument's traps stay the standing blind spot they are for every
property read (§ callbacks, Decision 2026-09-03), not a reach of any member.

The generator is unchanged: it still proposes `callbacks: []` for every member
alias, because syntax cannot say whose member it is, and the refusal is the
certifier's. `value-exports`' `entries` now pins it:
`a_reviewed_default_library_alias_closes_by_identity` closes `reads` and
`creates` and withholds `callbacks` on the getter reach.

The same reach exists where no alias is involved, and this amendment does not
close it: the call walk excuses a standard-library call's argument that is
rooted at a parameter (`census_standard_library_admits`), so `Math.max(n, …)` in
`clamp` and `Object.entries(o)` in a function body run caller code the
`callbacks` census does not count. That is recorded in
`docs/precision-backlog.md` (2026-09-24) as its own question.
