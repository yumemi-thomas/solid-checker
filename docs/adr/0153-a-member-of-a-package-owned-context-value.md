# ADR 0153: A member of a package-owned context value

- Status: accepted (owner, 2026-09-28); implemented in slices, see
  *Implementation status* at the end
- Date: 2026-09-28
- Owners: the Type Facts producer (`uncensused_invoking_forms.go`,
  `context_member.go`, handshake protocol 70), the policy-2 implementation
  census (`type_facts.rs`), and the dialect's context vocabulary
  (`solid_2.rs`)
- Relation: the first wall of
  `docs/package-contract-v2/phase22/2026-09-28-router-scope.md`. It extends
  ADR 0044's own-literal premise and ADR 0053's local literal result to a
  value that reaches the export through the dialect's context mechanism. It is
  not ADR 0151 (a wrapper citing a dependency's accepted claim): the context
  value is the package's own allocation, and no dependency claim exists to
  cite.

## Context

`@solidjs/router`'s three most-imported exports are one line each:

```js
const RouterContextObj = createContext();
const useRouter = () => invariant(useContext(RouterContextObj), "…");
const useNavigate = () => useRouter().navigatorFactory();
const useLocation = () => useRouter().location;
function useParams(_path) { return useRoute().params; }
```

Measured on next.30, next.26 and next.18/next.21 in all three hosts, each of
the four consumer domains of these exports stops at the same fact. The
producer records `useRouter().location` as a `property-access-unknown-accessor`
form, because TypeScript resolves the member to an interface property in
`types.d.ts`, which may be a getter. It states no subject derivation for it
(`call-result`), so the `creates` and `callbacks` censuses refuse the form.
The `reads` domain is withdrawn case-wide by `runtime-accessor-installation`,
and `returns` has no form for the object the member holds.

The question underneath all four is what `useContext(C)` can return. Nothing
in the checker can answer it today.

## What the dialect guarantees

Read against the audited archives `solid-js@2.0.0-rc.3` and `rc.9` (all
bundles: `dist/solid.js`, `dist/dev.js`, `dist/server.js` and the observe and
dev variants) and `@solidjs/signals` rc.3, rc.6 and rc.9 (`dist/prod/core/context.js`):

- `createContext(defaultValue, options)` returns a fresh function, `provider`,
  carrying a fresh `Symbol` as `provider.id` and `defaultValue` as
  `provider.defaultValue`. `provider(props)` calls
  `setContext(provider, props.value)` under a new root.
- `useContext(context)` is `getContext(context)`. It returns the value the
  nearest owner's context map holds at `context.id`, else
  `context.defaultValue`, and throws `ContextNotFoundError` when that is
  `undefined`. The server bundle rethrows the same error. So
  `useContext(C)` returns either `C.defaultValue` or a value some
  `setContext` stored under `C.id`.
- `setContext(context, value, owner)` stores `value`, or `context.defaultValue`
  when `value` is `undefined`, under `context.id` in a copy of the owner's
  map. `createComponent(Comp, props)` is `Comp(props || {})`, under
  `untrack` in the client bundles.

`C.id` is a fresh symbol. Storing a value under it needs `C` itself: through
`C`'s provider (as a component, by `createComponent`, by JSX, or by a direct
call), or through `@solidjs/signals`' exported `setContext(C, …)`.

**Stated assumption (as in ADR 0044's "The hole, stated").** Code that forges
`C.id` from an owner's private context map (`Object.getOwnPropertySymbols` of
the mangled field) is out of scope. So is code that installs an accessor on
an object a package handed it. Neither hole is new: ADR 0044 and ADR 0053
already take the second one for every own literal.

## Decision

**A member read `S.m` whose subject is, provably, the value of a package-owned
context `C` is a read of a data property of an object this package allocated,
provided every provider of `C` is a site in the package and the consumer
provides `C` nowhere.** It has four parts. Each part refuses by name when it
cannot hold.

### 1. The subject is a context value (producer)

The producer states the derivation `context-member` for a get-accessor,
set-accessor or property-access-unknown-accessor form in read position. The
subject must be one of the following, after the identity-preserving unwraps
the other derivations use:

- a call to the dialect's `useContext` whose argument is a reference to `C`;
- an unwritten local alias of such a value;
- a call to a local function whose every completion returns such a value,
  directly or through an identity helper. An identity helper is a local
  function whose every `return` hands back its unwritten first parameter, and
  whose last statement is a `return` (`invariant`).

A function declaration after a function's last `return` is hoisted, not
executed, so `return {…}; function helper() {}` ends in its return. Anything
else states nothing. That includes a join (`a || b`), a conditional, an optional
call, an `async` or generator function, and a function that mentions
`arguments` or `eval`. Every
call in the chain is stated with its callee, so the consumer binds each one to
a call row it has already censused. The census, and not the premise, is what
covers what the chain executes.

### 2. The provider census (producer)

`C` must be a module-level `const` in the package's runtime source, declared
once, initialized by a call to the dialect's `createContext` with **no
arguments**. So its default is `undefined`, and `useContext(C)` returns only
provided values.

The producer enumerates every reference to `C`'s symbol, through import
aliases, in every runtime source file of `C`'s own installed package (the path
up to its last `node_modules/<name>`). Each reference must be one of the
following, or the premise refuses:

- the declaration itself;
- the argument of a dialect `useContext` call (a read);
- the argument of a local **read helper**: a function whose parameter at that
  position is plain and unwritten, and whose every reference is the one
  argument of a dialect `useContext` call (the router's `useOptionalContext`,
  which catches the throw). The helper's reads join the provision's reads;
- an import specifier, followed like any other alias;
- a **provider**: the first argument of a dialect `createComponent` call whose
  second argument is an object literal with exactly one `value` member. That
  member is a plain property assignment, and the literal has no spread and no
  computed key. Its expression is either an object literal itself, or an
  unwritten single-declaration `const` initialized by a call to a local
  function whose every completion is `return <object literal>`;
- an **export**, local (`export { C as X }`) or re-exported: the context
  escapes, and its exported names are recorded.

A JSX element whose tag is `C`, a direct call `C(props)`, a `setContext`
argument, a shorthand property `{ C }`, an alias `const D = C`, a namespace
member access, a computed access, and every other position refuse. The JSX refusal is deliberate: what a JSX element passes as
props is the dialect compiler's lowering, and this ADR does not state it.

For each provided literal, the member `m` must be one own non-accessor member:

- a property assignment, a shorthand property or a method;
- with no `get m` or `set m` beside it;
- with no computed-key accessor anywhere in the literal;
- in a literal with no `__proto__:` member.

A spread before or after it keeps the member a data property
(`CopyDataProperties`), so a spread does not refuse.

The producer also scans the package's runtime source for the calls that can
turn a data member into an accessor after creation. Every one must name its
key by a string literal different from `m`, or the premise refuses:

- `Object.defineProperty` and `Reflect.defineProperty`;
- `Object.defineProperties` with a non-literal or `m`-keyed descriptor map;
- `__defineGetter__` and `__defineSetter__`.

Each spelling is matched by name, so an occurrence this scan cannot place in
such a call (an alias, `Object["defineProperty"]`) refuses. A `delete` of the
member is admitted: the read then reaches `Object.prototype`, whose one
accessor is the engine's `__proto__`. It would matter only beside a change of
prototype, so every spelling that can change an existing object's prototype
refuses instead: `setPrototypeOf` anywhere, and a write to `__proto__`.

The scan covers the package, not its dependencies. The stated assumption above
covers consumers and dependencies alike.

### 3. The consumer provides `C` nowhere (fail closed)

When `C` escapes through an export, a consumer can provide its own value:
`<RouterContext value={mock}>` is an ordinary testing idiom. The premise then
holds only in a program that does not do that. The certified claim carries the
condition: the case records `C`'s exported names. Consumer admission refuses
the claims that rest on the premise in any program that does either of these:

- references the exported context anywhere other than as the argument of a
  dialect `useContext` call;
- installs another package that declares a dependency (of any kind) on the
  certified package.

**How the condition is carried.** The premise is a field of the export's call
semantics, `contextPremises`: the exported names of each escaping context the
export's claims rest on. It is a condition, not a claim. It names no claim, so
no claim id and no recipe address moves, and weakening a closure keeps it. It
is signed like every other semantic: a document that states one is in its own
digest family (`solid-checker:semantic-context-premises:v1`), and a document
that states none hashes as before.

**Certification.** The census admits an escaping context exactly when the
export states a premise for every name the context escapes under, and checks
that clause last, so a premise is only ever asked for a read every other
clause admits. Otherwise it refuses in one spelling, `context premise
required: [names] (…)`. The transaction reads that spelling back, states the
premise on the export, and re-plans; the next census admits the read. A
requirement already stated is not progress, and its refusal is withheld like
any other, which keeps the loop bounded. In the published-graph lane only a
node that no other node depends on states a premise: a dependency's receipt is
composed as its accepted proposal with withheld domains opened, and a premise
is not an opening, so a premise refusal at a dependency is withheld.

**Consumer admission.** For each package whose accepted contract states a
premise, the analysis collects, over the whole program, every use of each
premise name imported from the package. A use is allowed only as the one
argument of a dialect `useContext` call (by exact call identity, TypeScript
sugar peeled). Everything else counts as providing the context: a JSX tag
naming it or dotted from it, `createComponent(C, …)`, an alias, any other
argument, a re-export (`export { C }`, `export *`), `import … = require`, a
dynamic `import()` or `require` of the package. Through a namespace import,
the namespace object escaping or a computed member provides every premise. A
binding the analysis cannot name provides every premise of its package. The
backend adds the second clause: it walks every `node_modules` directory Node
resolution can reach from the project (scoped, nested, and pnpm's store), and
a package there that declares a dependency of any kind on the certified
package provides every premise. A manifest it cannot read or parse, and a tree
deeper than it descends, fail closed the same way.

An import site of an export whose premise is provided loses **every** claim of
that export and reports `SC9005`, uncertifiable, at error severity (not ADR
0119's open-claims warning: the claims are unusable here, not partial). Its
message names the premise. Exports without a premise keep their claims.

`<RouterContext.Provider>` needs no case of its own: against the real
typings it is `TS2339`, TypeScript's to report. `<RouterContext value={…}>`
and `createComponent(RouterContext, …)` type-check and are the cases.

### 4. The member's value (`returns`)

A read of `m` that is a data read says nothing about what `m` holds. `returns`
needs the held value's shape:

- `location`: a literal of getters that read memos, plus a `query` member
  whose value the consumer's history utilities may choose;
- `params`: a `createMemoObject` proxy.

Stating either needs two new claim shapes: a tracked-getter object and a
memo-object proxy. It also needs a proof that no code writes `m` on the
provided literal after creation. Neither exists yet, and this part is **not
decided** here (see status).

### What each domain gains

- `creates` and `callbacks`: the form is dispositioned
  (`context-member-accessor`) once the chain binds and the premise's dialect
  calls are the audited archive's `createContext`, `useContext` and
  `createComponent`. The read reaches a data property, so it runs no code.
- `reads`: the form is a data read, but the case-wide
  `runtime-accessor-installation` hazard still withdraws the domain (router
  `dist/index.js` carries three `new Proxy` and three `Object.defineProperty`).
  Lifting that is a separate per-export bound (status, item C).
- `returns`: part 4, not decided.

## Alternatives considered

- **Refuse any escaping context, for good.** Sound, and it leaves the router
  refused forever, because `RouterContext` is exported in every release. The
  owner chose the consumer-side condition instead (part 3).
- **Treat a consumer-provided value as the consumer's, as ADR 0034 treats an
  argument.** A consumer analysis sees the argument at the call it passes it
  to. It does not see a provider three components up. A getter on a mock
  context would run at every `useLocation()` without any claim naming it.
  Rejected.
- **Accept JSX providers.** They are the `solid` condition's shape
  (`dist/index.jsx`). The props a JSX element passes are the compiler's
  lowering (`get value() { … }` or a plain member, depending on the
  expression). Stating them belongs to the dialect's compiler vocabulary, not
  to this premise. Refused until that exists. The default-condition case
  (`dist/index.js`, precompiled `createComponent`) is what certification
  selects today.

## Consequences

- Handshake protocol 68 → 70 (69 is reserved for ADR 0152):
  `UncensusedInvokingForm.contextMember`, with the schema digest and the Rust
  client. The dialect gains
  `Dialect::context_role` (`createContext`, `useContext`, `createComponent`),
  and the census a disposition, `context-member-accessor`.
- Measured on the real `@solidjs/router` bytes at next.26 (rc.9) and next.18
  (rc.3), host free. The producer states the premise for `useLocation` and
  `useIsRouting` (the `useRouter()` member reads through `invariant`), in both
  `callbacks` and `creates`. The census refuses both with "the context escapes
  the package as `RouterContext`". No router export moves, as part 3's
  fail-closed default intends. `useNavigate` stops earlier, at the member
  *call* `navigatorFactory()`, whose callee resolves to an interface member.
  `useParams` stops at `useRoute()`'s join (`useOptionalContext(…) ||
  useRouter().base`).
- Coverage is unchanged: 131 fixture projects, 709 findings.
- Tests: the producer's
  `TestContextMemberPremiseNamesTheChainTheProvidersAndTheReads` (positive, a
  getter member, a read helper), `…StatesAnEscapeAndRefusesEveryOtherReference`
  (escape, exported declaration, direct call, alias, shorthand, default value,
  a written binding, a parameter or call value, a getter or `__proto__`
  literal, spread props, a shadowed `useContext`, a join, hoisted helpers) and
  `…RefusesAnInstallationThatCouldReplaceTheMember` (defineProperty keys,
  aliases, descriptor maps, deletes, `setPrototypeOf`, `__proto__` writes);
  the census's
  `creates_census_binds_a_context_member_premise_and_refuses_each_broken_leg`
  (each binding leg broken in turn).

## Implementation status

- **A. Parts 1 and 2 (producer and census), with part 3's fail-closed
  default.** Landed with this ADR, for `creates` and `callbacks`. The `reads`
  census does not yet disposition the form: it walks no calls, so it cannot
  bind the chain, and for the router the case-wide hazard withdraws `reads`
  first anyway (item C).
- **B. Part 3's admission gate.** Landed. Model, wire (`contextPremises`,
  schema), digest family, the census clause, the transaction's re-plan in the
  value-only and published-graph lanes, the consumer's reference check and
  the backend's installed-dependents walk. No handshake bump: the producer
  already stated every escape at protocol 70. Measured on the release binary
  (host free and `browser`, next.30, next.26, next.18): `useLocation` and
  `useIsRouting` now close `callbacks` and `creates` under the premise
  `RouterContext`. `reads` (item C) and `returns` (item D) stay open, so no
  export is clean yet; `useLocation` moves from degenerate to partial. Under
  `node` the two domains move from the census refusal to "veto did not
  complete" at next.26 only. Fixtures: `package-context-premise-met` (clean)
  and five that each break the premise one way (`-provided`,
  `-create-component`, `-namespace`, `-reexport`, `-dependent`), against the
  published router next.26 typings, `tsc` silent against the real rc.9
  install. Tests: `context_premise_digest_family_is_separate_frozen_and_moves_no_claim`,
  `context_premises_round_trip_in_their_own_digest_family`,
  `a_context_premise_is_stated_once_and_a_repeat_is_not_progress`, and the
  census test's `escaped-stated`, `escaped-other-stated` and
  `escaped-stated-broken` legs.
- **C. A per-export bound on `runtime-accessor-installation`** for `reads`.
  It needs a complete enumeration of the member accesses the export executes,
  each on a receiver the census can prove is not a hazard site's object. Not
  landed.
- **D. Part 4.** Not landed. It needs its own claim-shape ADR.
