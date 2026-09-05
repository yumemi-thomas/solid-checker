# ADR 0034: An accessor reached through a parameter is the caller's code

- Status: accepted and implemented (2026-09-05); written before implementation
- Date: 2026-09-05
- Owners: Type Facts producer and the policy-2 `creates` census
- Relation: extends ADR 0008's `parameter-rooted` disposition from calls to
  read accessors, under ADR 0029's binding premises. It does not touch the
  host-realm question (`navigator`, `HTMLElement[Symbol.hasInstance]`, DOM
  getters on elements the package creates), which stays refused.

## Context

The `creates` implementation census (ADR 0008) certifies `creates: []` only when
every invoking form in the export's transitive census is enumerated and
dispositioned. It already holds one principle about caller-supplied code: a call
whose callee is proven to be a parameter of the censused declaration is
`parameter-rooted` — "what that callable does is the caller's behavior, in the
caller's artifact, under the caller's own contract; this export's act is the
invocation." The standard-library disposition rests on the same principle from
the other side: `Array.prototype.map` invoking its callback is not a
counterexample, because the callback's body is code written elsewhere and
dispositioned where it is written.

The census does not extend that principle to accessors. Every uncensused
invoking form at the `MayExecute` floor refuses by kind and location, and the
producer records no fact about *whose object* the form runs on. Reading
`polygon[i]` when `polygon` is a parameter refuses as
`property-access-unknown-accessor` exactly as reading `window.navigator.platform`
does, although the first can only run code the caller put on the object it
passed and the second runs code on a host global the package did not receive.

Measured on the seventeen native census refusals that remain across the
original forty Kobalte candidates (see the browser-boundary measurement
report), the accessor-through-a-parameter shape accounts for:

| candidate | form | subject |
| --- | --- | --- |
| 0.9.2 `isString` | `CallableFunction.call` (by-reference owner) | `Object.prototype.toString.call(value)`: the only user code reachable is `value[Symbol.toStringTag]`, and `value` is the parameter |
| 0.9.2 and alpha `isPointInPolygon` | `property-access-unknown-accessor` (element access) | `polygon[i]`, `polygon[j]`, `polygon[j === 0 ? l - 1 : j - 1]`; `polygon` is an unwritten parameter typed `Point[]`, whose index signature the compiler cannot bind to a data property |
| 0.9.2 `scrollIntoView`, `scrollIntoViewport` | `property-access-unknown-accessor` (computed member) | `child[prop]` in `relativeOffset` — but `child` is **reassigned** in the loop (`child = child.offsetParent`), so the read is not rooted at the parameter binding this ADR admits |

The same shape is pinned in the repository already:
`implementation-census-creates`'s `memberParameterRooted` (`source.read()`) is
refused on the accessor form even though the call itself would have been
`parameter-rooted`; ADR 0008 recorded that as "the walk and the census agree,
and the blocker is the producer's inability to tell an accessor from a data
property on an untyped receiver."

## Decision

**A read accessor whose subject is rooted at an unwritten parameter of the
censused declaration is dispositioned `parameter-rooted-accessor` instead of
refusing.** The code it can run — a getter, a Proxy `get` trap, a
`Symbol.toStringTag` getter — was placed on that object by the caller and is
analyzed in the caller's own artifact; the export's act is the read.

### What "rooted at a parameter" means

The subject expression of the form is, after identity-preserving unwrapping
(parentheses, non-null and `as` assertions, which the erasure profiles already
treat as transparent), either

- a reference to a parameter binding of the censused declaration — the export
  itself, or a `local-recursion` target frame — or
- a chain of property reads, element reads, and optional-chain reads whose
  innermost receiver is such a reference,

and that parameter binding satisfies ADR 0029's binding premises: exact symbol
identity in the authenticated runtime source, a plain identifier binding (no
destructuring pattern, no rest element), **no write anywhere in the declaration**
(no assignment, compound assignment, update expression, or `for…in`/`for…of`
head naming it), no competing declaration, and no reference from the
declaration's `arguments` object. A binding written after the read still
refuses: the disposition is not flow-sensitive, and it says so.

Every intermediate read in the chain is itself a form the census must
disposition; each is rooted at the same parameter and takes the same
disposition, or is a bound data property and records no form at all.

### Which forms it admits

Exactly two of the producer's uncensused-form kinds, in **read position** only:

- `get-accessor` — the resolved member is declared with a getter;
- `property-access-unknown-accessor` — the compiler binds no data property for
  the read: an untyped receiver, an index signature, a computed key, a rest or
  computed destructuring element.

Nothing else. `set-accessor` and any accessor in write position stay refused
(writes into a caller's object are a `writes`-domain question this ADR does not
open). `iteration-protocol`, `coercion`, `await-then` and `instanceof` on a
parameter-rooted operand stay refused: each has its own protocol reach and
deserves its own decision, and `instanceof` in particular runs code on the
*right-hand* operand, which in every measured case is a host global.

### `Function.prototype.call` and `apply` on an exact default-library receiver

`Object.prototype.toString.call(value)` refuses today under the by-reference
owner rule (`CallableFunction.`), which exists because `render.call(null, …)`
transfers control to a callable the census cannot see. That rule is correct and
stays. This ADR adds one narrow admission in front of it:

When the receiver of `.call` or `.apply` resolves — by default-library symbol
identity, never by spelling — to a member of a **reviewed this-protocol table**,
the call is dispositioned as an invocation of *that member* with the first
argument as its subject. The table names members whose only reach into user
code is a protocol read on their `this` value, and states that read:

| member | reach on `this` |
| --- | --- |
| `Object.prototype.toString` | `Get(this, @@toStringTag)` — one getter or trap on the subject, nothing else |

The subject then takes `parameter-rooted-accessor` under the rule above when it
is parameter-rooted, and refuses otherwise. `.call`/`.apply` with any other
receiver, any `.bind`, `Reflect.apply`, `eval` and `Function` refuse exactly as
before. The table is grown by review, one member per row, with the ECMAScript
step that names the reach; it is not derived from `lib.*.d.ts`.

### What the disposition means for the certificate

`creates: []` continues to mean: this invocation of this export, in this
artifact case, under this guard, registers no version-1 resource into a runtime
outside the invocation. Code the caller attached to an object it passed is not
this export's registration any more than a callback it passed is; if such a
getter calls `createEffect`, that call is in the caller's file, where ordinary
analysis already sees it with its own owner and timing. The certificate makes
no claim about that code and never did. What the disposition removes is a
refusal that treated the caller's object as if it were the package's.

The soundness boundary is the parameter root. A read whose receiver is a module
variable, a captured value, a value returned by a call, an element of a
parameter reached through a **nested callable's own parameter** (`items.filter(x
=> x.value)`), or a parameter the declaration writes, is not admitted, because
in each of those cases the census cannot prove the object is one the caller
handed to *this* invocation.

## Alternatives considered

- **Trust declared types.** Treat a member declared as a data property on a
  `lib.dom` or package interface as inert, or an array type as excluding a
  Proxy. Rejected: a declaration is not the bytes that run, and the census's
  standing rule is that it may not be read as one. The parameter root is a
  fact about *provenance*, which declarations cannot supply.
- **Make the disposition flow-sensitive** so `relativeOffset`'s reassigned
  `child` qualifies while it still aliases the parameter. Rejected for this
  ADR: it needs a definite-assignment analysis in the producer and a new class
  of premise; the unwritten-binding rule is the one ADR 0029 already reviewed.
  It is the natural next extension and would admit the `scrollIntoView` pair.
- **Admit nested-callable parameters** whose invoker is a standard-library
  member handed a parameter-rooted collection. Rejected here: it composes two
  dispositions and needs the invoker table to state which argument each slot
  receives. Deferred, not refused on principle.
- **Admit host-global accessors** by a host-realm integrity premise. Rejected:
  no checker premise can establish that a page did not redefine `navigator` or
  `HTMLElement[Symbol.hasInstance]`; this is the accessor-census limitation the
  earlier slices deliberately left unbroadened, and it stays that way.
- **Leave the refusal.** Sound and simplest, but it refuses on a fact the census
  already accepts for calls, and ADR 0008 records the inconsistency by name.

## Implementation (2026-09-05)

- **Producer** (`apps/solid-typefacts`, handshake protocol 17 → 18, schema
  digest moved). `UncensusedInvokingForm.subjectParameter` is stated for a
  `get-accessor` or `property-access-unknown-accessor` form on a property or
  element access in read position (not an assignment target, not a `delete`
  operand) whose receiver chain — property, element and optional-chain reads
  after identity-preserving unwrapping — roots at a plain-identifier parameter
  with no initializer and no rest token, written nowhere in its file
  (`symbolIsAssignedLocked`), in a declaration mentioning neither `arguments`
  nor `eval`. `ImplementationCall.callReceiver` and `thisParameter` are stated
  for a `.call`/`.apply` whose resolved callee is a default-library
  `Function.prototype` member. The producer decides nothing: it states facts
  the verifier reviews.
- **Verifier** (`type_facts.rs`). `CensusDisposition::ParameterRootedAccessor`
  (wire `parameter-rooted-accessor`) is taken for exactly those two form kinds
  on those two node kinds with a stated subject, with a `census-form:` witness
  site; every other form refuses as before. `census_standard_library_admits`
  consults `CENSUS_THIS_PROTOCOL_MEMBERS` — seeded with `Object.prototype
  .toString`, qualified `Object.toString`, whose only reach is
  `Get(this, @@toStringTag)` — before the by-reference owner rule, and admits
  only a rooted `this`. `CENSUS_PARAMETER_ROOTED_SUBJECTS_PROTOCOL = 18` refuses
  a build that speaks less.
- **Generator walk** (`creates_walk.rs`). The `parameter-rooted` decline is no
  longer taken when the member callee's receiver roots, with no alias hop, at a
  plain, uninitialized parameter of the outermost function containing the
  call; alias, nested-parameter and computed shapes keep declining. Across the
  corpus this removed 29 `parameter-rooted` declines and the 15
  `refusing-callee-fixpoint` declines that depended on them; the only other
  change to any `expected.json` is the added `creates` closure label on those
  exports.
- **Fixtures.** `implementation-census-creates` gains `toStringTagViaCall`
  (certifies) and seven boundary exports that refuse (`writtenBeforeRead`,
  `writtenAfterRead`, `moduleReceiverRead`, `nestedCallableParameterRead`,
  `callNonLibraryReceiver`, `callLibraryOutsideTable`, `setterOnParameter`);
  `memberParameterRooted` certifies. `creates-decline-records` keeps `parameterRooted`
  as its control and no longer records a decline for it.

Measured on the seventeen remaining native refusals
(`/private/tmp/parameter-rooted-accessor-measurement/results.json`, SHA-256
`d625d54f15618aed4b73ebd300cdf872f1d1f8402a579c7f41e0ff7326589277`): Kobalte
0.9.2 `isString` and both `isPointInPolygon` cases now pass the census and
complete the import-free controlled profile with no contradiction — **3 of 3
targeted**. `scrollIntoView` and `scrollIntoViewport` still refuse, on the
accessor form of the parameter `relativeOffset` reassigns, exactly as
predicted. Fourteen refusals remain.

## Consequences

- **Yield.** Three of the seventeen remaining native refusals become
  census-complete: `isString` (through the this-protocol table) and both
  `isPointInPolygon` cases. They then need only the existing import-free
  controlled profile to complete their gates. The `scrollIntoView` pair stays
  refused on the written binding; the other twelve are host-realm refusals this
  ADR does not touch. `exportsProven` does not move: closing `creates` alone
  closes no export.
- **Producer.** Each uncensused invoking form gains an optional `subjectParameter`
  (the frame parameter index the subject is rooted at, present only when every
  premise above holds) and, for `.call`/`.apply` sites, the resolved receiver's
  default-library identity. The wire table shape changes additively; the
  handshake protocol moves from 17 to 18 because an absent field on a
  protocol-17 producer must not be read as "not rooted" by a verifier that
  expects the fact. The source-manifest pin moves with it.
- **Verifier.** `CensusDisposition` gains `ParameterRootedAccessor`, with wire
  name `parameter-rooted-accessor`, applied to the two admitted kinds in read
  position before the refusal-by-name; `census_standard_library_admits` consults
  the this-protocol table before the by-reference owner rule. Receipt witnesses
  carry the new disposition name at its site, so only rows that newly certify
  change identity.
- **Fixtures.** `implementation-census-creates`'s `memberParameterRooted`
  becomes the positive pin and certifies. New negative pins in the same fixture:
  a parameter written before the read, a parameter written after the read, a
  read on a module-level object, a read through a nested callable's parameter,
  `.call` on a non-library receiver, `.call` on a library member outside the
  table, and an accessor in write position. `creates-decline-records` keeps
  its `parameterRooted` control unchanged. CONTEXT.md's census-disposition
  entry lists the new disposition.
- **Vocabulary.** "Parameter-rooted" now names one principle applied to two
  forms — calls and read accessors — and the ADR 0008 text that called the
  accessor case a blocker is amended to point here.
- **Not changed.** The `reads` domain, the host-realm refusals, the reviewed
  invoker table for callable slots, every controlled execution profile, and
  every receipt identity for rows that certified before this ADR.
