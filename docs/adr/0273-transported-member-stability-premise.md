# ADR 0273: transported member stability premise

> Status: contract-related decisions superseded by [ADR 0312](0312-package-contracts-are-removed.md). Source analysis and runtime premises remain subject to that ADR.

- Status: accepted owner decision; implemented (2026-10-11).
- Owner: Reactive IR's project-owned accessor transport and per-analysis veto index.
- Extends the narrowed returned-member/context transport; follows ADR 0266/0271.

## Decision

Assume **transported member stability**: a member of an object returned by an
admitted factory or supplied by a context provider is not reassigned, deleted
or redefined after the object is created. There are exactly two states per
member name: **Assumed** (default) and **Vetoed(site)**. Absence of a visible
veto is not a proof of stability. This is an explicit conditional premise,
not a complete receiver/alias/consumer analysis.

Unresolved identities, dependency internals, excluded sources, unsupported
reflection, eval, bundler injection and prior/HMR code are outside the premise,
as in ADR 0266. The application must ensure stability in those channels.
Missing facts do not veto or add a third knowledge state.

## Visible veto

Across every analyzed project source, including unimported sources, index
static member writes by **name on any receiver**. Normalized write targets
cover assignment, compound/logical assignment, ++/--, destructuring leaves,
for-in/for-of targets and delete. Static dotted/optional member targets and
literal computed keys use their syntax facts, including cooked string keys.
Transparent TypeScript wrappers (as, satisfies, non-null, type assertions and
parentheses) do not erase literal computed keys. Substitution-free templates
also supply their cooked literal key; templates with substitutions do not.
Receiver Gets, destructuring keys/defaults and computed-key reads are not writes.

Exact standard-library signature and runtime receiver declaration identities
identify Object.defineProperty, Object.defineProperties, Reflect.set,
Reflect.defineProperty, Reflect.deleteProperty and Object.assign. Their static
literal keys veto the named members. Known computed descriptor-object keys are
recorded after the same wrapper peeling, without closing the object shape.
Object.assign with a spread argument in **any position**, including its first
written slot (`Object.assign(...args)`),
spread/open literal or nonliteral source vetoes **every** transported object
member in the project. So does any spread argument to any of these builtins —
a tuple spread shifts every later position, so no key position is known — and
a single-key call (Object.defineProperty, Reflect.set, Reflect.defineProperty,
Reflect.deleteProperty) whose key the argument-literal fact cannot name (a
template literal, an identifier), and an Object.defineProperties call whose descriptor map is not a
literal or contains a spread: each could write any member, and over-vetoing only
withholds. Local homonyms and local ObjectConstructor-typed values are not
standard-library runtime receiver identities. Unsupported aliases, computed
callees and nonliteral computed keys inside a literal descriptor map remain
outside the premise.

Deliberately obfuscated Unicode escapes in dotted member names or mutation
callee names (`x.st\u0061te`, `Object.ass\u0069gn`) are outside this premise.
The dotted-member fact records source spans, and this veto compares their raw
spellings; it does not currently carry the parser's cooked IdentifierName.
This exclusion does not apply to cooked escapes inside literal computed strings
or templates, which are normalized and vetoed. Applications must ensure
stability in the excluded obfuscation channel too.

A nonliteral computed write/delete on a transported object's own exact binding
already refuses that receiver via the existing reference census. Computed
nonliteral writes on other receivers are outside the premise. This is deliberate;
the name index does not guess their key or receiver identity.

Name-based over-veto is safe in this direction: it only withholds transport,
never supplies reactive origin, execution-role evidence or a finding. An
unrelated object's `state` write may withhold a context's `state` read. Ordinary
uncertifiable results/nothing and definition-site obligations remain; the veto
does not claim that a defect or an actual replacement occurred.

The three review examples contain visible writes: nested alias/helper mutation
writes `rows`, a second context consumer writes `state`, and
`replaceAccount(useAccount)` writes `state` through a callback result before a
direct useContext consumer reads it. Each name is vetoed regardless of whether
the receiver census joins that alias/callback. Focused controls acquire no
violation. Nested transport remains refused independently.

## Mechanism and limits

One written-name/site index and one optional wildcard site are computed per
analysis in accessor_provenance::Resolver. Member selection consults this index
before projecting the original literal's callable. Exact accessor/source and
admitted function identities, consumer execution roles, context cardinality,
single literal return, one-level projection and receiver census remain required.
The census alone does not close member stability. No synthetic nodes, nested
projection, arbitrary callback dataflow or package trust are added.

Retained analyses recompute the index. Existing result reuse compares current
transport and unavailable-target maps, and actual callable summary dependencies;
a cross-file Assumed/Vetoed transition cannot reuse an old transported result.
Contract export fragments also invalidate on a transport-map transition: a
removed edge is absent from the current reverse graph and cannot dirty its
former callers through that graph. File-local graph contributions remain reusable.
The focused retained/cold regression exercises both directions, including a
write on an unrelated receiver, first-slot tuple and later-slot argument spreads,
wrapped literal writes/deletes/descriptors and substitution-free template keys.
Addition and removal both compare the full retained/cold Program. Callback-fed
context mutation controls also cover the review's separate direct consumer.
No premise state is retained independently.

Returned provider arrays additionally require one exact factory's single array
literal of closed object literals and source-discovered synchronous arrow
members. A standard-library-identified map/forEach with an inline synchronous
arrow may invoke one exact member on its element parameter as its entire body
expression. Conditional/block callbacks and maps inside named/stored callback
definitions remain refused. Its execution role
comes from the iteration call site; the member's definition supplies no role.
The exact immutable array binding has only immediate map/forEach receiver uses
or an admitted single-call parameter handoff with the same receiver census.
Index replacement/deletion, length writes, push/pop/splice/shift/unshift/sort/
reverse/fill/copyWithin, Object/Reflect mutation and any alias, unsupported helper argument,
storage, export, spread, destructuring or iteration use veto that binding.
The same written-name/wildcard index vetoes the selected element member and
visible map/forEach replacement. No array element aliases are inferred; holes,
spreads and alternative returns are refused.
This retains the existing unpatched standard-library runtime premise.
Array-derived violations additionally require an exact local runtime source
origin (including admitted memo wrappers). Type-only or external origins stay
attributed through ordinary helper hops. The per-analysis source-origin set
participates in retained transport identity even when a summary is unchanged.

An exact context wrapper may forward its immutable intermediate binding via
its single return, with no other use beyond the ordinary receiver census.
This handoff still joins the context-wide census and member stability veto.
Pure truthiness and nullish equality tests of the exact context binding do not
escape it. A discarded nullish-coalescing expression may test it too; a
coalescing result assigned, returned or passed onward is still refused. The
wrapper must retain its one exact unconditional return. Aliases and opaque
escapes remain refused.

A const object pattern may select direct callable members with plain identifier
bindings only: no defaults, rest, computed keys or nesting. Every selected
key must itself be an unescaped static identifier (quoted keys stay refused).
Every selected binding is used solely as an immediate callee. Selection consults the same
written-name/wildcard veto, and the context-wide receiver join includes these
bindings. An escaped destructured member also refuses its source receiver.

A plain unwritten parameter may carry an exact transported object or array
from one exact direct call to an unexported current function. The function's
complete source/binder and Type Facts reference censuses must contain that one
call alone; aliases, stored/passed values, a second call, argument spreads,
default/rest parameters and any `arguments`/`eval` use refuse the channel. ADR 0193's
open-program rule still refuses published-library exports; this bounded
channel additionally refuses app exports. Parameter and argument censuses join
in both directions required for withholding, including chained handoffs.
Parameter provenance supplies no execution role: an async helper's post-await
invocations remain uncertifiable. No callback-result transport is added.

Direct accessor calls, callable context values and other independent reads are
outside this object-member premise. Definition obligations are never suppressed
by another invocation. Transport-derived preference dependencies remain refused.
Defaulted/multiple providers, computed/spread/getter/method literals, nested,
mutable/aliased/escaped/exported receivers, unsupported destructuring,
nonliteral/multiple factory returns, async members and implementations absent from source discovery
remain withheld. Unpatched built-in/runtime premises retain their existing scope.
