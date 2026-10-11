# ADR 0273: transported member stability premise

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
Receiver Gets, destructuring keys/defaults and computed-key reads are not writes.

Exact standard-library signature and runtime receiver declaration identities
identify Object.defineProperty, Object.defineProperties, Reflect.set,
Reflect.defineProperty, Reflect.deleteProperty and Object.assign. Their static
literal keys veto the named members. Object.assign with a spread argument,
spread/open literal or nonliteral source vetoes **every** transported object
member in the project. Local homonyms and local ObjectConstructor-typed values
are not standard-library runtime receiver identities. Unsupported aliases,
computed callees and nonliteral reflective keys remain outside the premise.

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
write on an unrelated receiver. No premise state is retained independently.

Direct accessor calls, callable context values and other independent reads are
outside this object-member premise. Definition obligations are never suppressed
by another invocation. Transport-derived preference dependencies remain refused.
Defaulted/multiple providers, computed/spread/getter/method literals, nested,
mutable/aliased/escaped/exported/destructured receivers, nonliteral/multiple
factory returns, async members and implementations absent from source discovery
remain withheld. Unpatched built-in/runtime premises retain their existing scope.
