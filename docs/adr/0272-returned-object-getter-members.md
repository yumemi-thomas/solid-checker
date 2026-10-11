# ADR 0272: Returned plain objects may describe scalar getter members

- Status: accepted owner decision; implemented (2026-10-11).
- Owners: normalized value semantics, document decoder/encoder and schema,
  authored validation/probe identity, return projection and local Get analysis.
- Extends schema version 1 additively; existing object/data-member meaning and
  canonical bytes stay unchanged. Extends ADR 0271's premised next.26 claims.

## Vocabulary and scope

An exact returned `object` may contain `{ "kind": "getter", "reactive": true }`
or the same leaf with `false`. Each leaf asserts an **own, enumerable accessor
property**. Non-enumerable or inherited accessors cannot be described with it;
no descriptor metadata is added, and existing data-member meanings stay unchanged.
A reactive getter performs a reactive read on
every Get and yields plain data. An inert getter yields plain data without a
reactive read. A data member continues to use its existing value shape: its
Get is inert, and the retrieved value has that shape (for example `store`).
These three alternatives describe different operations. A getter is never an
accessor function, and its returned scalar is never branded as a store.

The initial supported container is a complete whole-object return with direct
getter members and plain/store/reactive data members. Unknown, nested, partial,
input, tuple and callable-graph placements are refused. The new canonical tag
is 28; older tags and digest families are unchanged. Certification refuses the
leaf: no implementation census proves it. Authored claims need installed-byte
citations in `getterClosures[return-operation]` showing the own enumerable
definition (for example an object-literal getter), as well as Get behavior.
The citation must explicitly state "own enumerable"; validation requires these
words and the source location, while source review establishes their truth.
Fresh probe identities bind
the entire claim, both programs, artifact cases and runtime pins.
Projection also requires one unguarded, same-stack, exactly-once call-triggered
completion in call scope. Optional, deferred or partial returns remain open.

Reuse: ordinary object properties already describe the data values, so only a
getter leaf is added. ADRs 0235/0249 describe calls, not Gets; property-get
operations describe caller inputs, not returned members. ADR 0255's lazy getter
object describes observer-dependent cache priming and cannot prove an
untracked Get. ADR 0264 does not change that limitation. None is reinterpreted.

## Consumer behavior

Exact immutable receivers, exact resolved package callees (including namespace
imports), direct factory-member expressions and transparent TypeScript receiver
wrappers preserve the shape. A reactive Get is a read at the member expression
or destructuring site. The owner-decided round-3 consumer proves only plain
placements. A Get written directly in its innermost component/rendering body
produces SC1001 at the existing strict-read-untracked boundary. Direct
compiler-tracked JSX expressions/attributes remain clean. An inline arrow or
function literal that is itself an exact Solid primitive's tracked compute
argument remains clean; no intervening function is admitted. Direct inline
event handlers and explicit untrack callback arguments retain their non-strict
policy. Direct effect apply retains a violation only where the existing
accessor/store execution model already makes apply a strict-read scope.

Everything else receives `returned getter execution` SC9012: named functions
(even memo-only callbacks), helpers, returned/stored/nested functions, inline
wrappers such as flush, onSettled and unknown calls. A primitive callback must
be supplied directly from a component/rendering body or module scope, rather
than from another callback/helper. The consumer does not project invocations
or compose execution witnesses. This deliberately gives up clean named-only,
inline-wrapper and onSettled cases to avoid borrowing execution roles.
Shared accessor/store classification is unchanged. Inert getters do not read.

Retrieving a store data member is inert. Direct static Gets on its proxy and
one exact immutable local binding/destructuring of that proxy retain store
read behavior. Computed proxy keys and unsupported transports remain explicit
uncertifiable dispatch obligations. Data accessor transport is not projected
in this slice and remains an obligation.

The returned object is plain: ownKeys and has are inert. Exact standard-library
Object.keys/Reflect.ownKeys and literal-key Reflect.has/Object.hasOwn calls are
recognized via their resolved library declarations, not callee spelling.
The signature alone is insufficient: the exact default-library runtime receiver
and a project-wide escape/write check are required. Structural builtin typings
and aliased method values remain uncertifiable. Exact symbol metadata comes
through the semantic lookup, preserving the fact-table boundary.
The `in` syntax currently lacks a normalized operand fact here and remains
uncertifiable; that is not a reactive read claim.

Exact object destructuring runs the selected getters. Object spread, one-source
Object.assign into a fresh empty literal and Object.values/entries run the own
enumerable getters only when every recipe member is a getter or plain/store
data member, and the same direct placement policy holds. Other reactive data
members withhold traversal. JSON.stringify always remains SC9012:
**serialization is not modeled**. No serialization proof or toJSON/prototype
premise/veto remains in this consumer.
More elaborate copying signatures, reflective descriptors, rest/computed
bindings, mutable receivers, replaced members, aliases, unknown-call escapes,
export, unproven helper/callback execution and missing identities retain obligations. Escaping or mutated receivers
cannot supply a proven subsequent getter violation. Copied nested reactive
values are not granted new provenance by the traversal.

Object binding diagnostics are classified as value access by the runtime
feedback collector; calls in initializers/defaults keep their separate
package-call classification. The probe ledger's acceptance condition is unchanged.

## Router claims

Under ADR 0271's Assumed standard router integration, next.26 useLocation closes
returns as this object. pathname/search/hash/state are reactive getter leaves,
key is inert, query is a store data member. Source authority is the exact
next.26 install's `dist/routing.js:35,79,490-528,724-751` and
`dist/utils.js:124-147`; the latter proxy traps belong only to query.
The location accessors are object-literal getters at `dist/routing.js:509-527`,
which establishes their own enumerable definitions.
The existing exact visible integration veto withholds the entire hook claim,
including this return shape. Throws, lifetime and other router protocols remain
open. No vocabulary or package-specific rule is added to the Solid dialect.

Next.16/17/18 remain fully open: each useLocation returns
`useRouter().location` verbatim (`routing.js:61`, `:64`, `:65` respectively),
and an application-provided RouterContext may substitute that object. This
decision adopts no new older-version premise. Their standard constructors'
similar getters cannot prove their hook results in every admitted application.

## Evidence and limits

Focused synthetic authorized fixture `package-getter-member-consumer` pins the
Get/traversal/inert/escape branches. Published next.26/rc.13 pairs typecheck
strictly with skipLibCheck=false. All 26 next.26 Chrome pairs pass, including
twelve useLocation pairs. Native pair and fixture results are recorded in
`rust/target/research/getter-members/RESULT.md`; no corpus sweep is claimed.

Round-3 controls retain the thirteen published next.26/rc.13 cases; direct
onSettled intentionally changes from clean to SC9012. The inline flush wrapper,
named memo-only/memo-plus-body/memo-plus-escape and own-toJSON controls remain
uncertifiable with no getter violation. Direct body, memo and tracked JSX
controls preserve their outcomes. JSON.stringify with or without toJSON is
uncertifiable. Exact snapshot changes and focused verification are recorded
in `rust/target/research/getter-members/RESULT-3.md`.

Source prediction: next.26's Kui TopicsRoute.tsx:425 signal seed is a real
untracked getter defect and should regain its violation. The real next.16
WaitlistRoute.tsx:33 seed remains withheld because that version's hook shape
cannot be claimed honestly. Aggregate corpus measurements remain lead-owned.
