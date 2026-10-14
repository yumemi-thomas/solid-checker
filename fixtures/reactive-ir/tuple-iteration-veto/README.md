# Returned-tuple iteration veto (ADR 0282 amendment)

The review's project iterator replacement (`nsb`) yields two harmless closures.
The call to `peek` must therefore be uncertifiable (SC9012), never the SC1001
strict-read violation that the intact iterator proves. Mutation in another
project file also withdraws that proof.

`AccessorRead` pins the companion typed-source proof: the returned accessor
slot is also replaced, so its direct call is SC9012 uncertifiable, with no
proven read retained from the declared accessor type.

`cases.json` seeds isolated variants of `mutations.ts` in the process test.
Each positive variant must retain the uncertifiable call and no violation;
each negative variant must retain the original proven read. They run as separate
projects because this is a project-wide visible-veto premise: a positive write
in the same project would also veto a negative control's tuple.

The variants include assignment, immutable aliases, all supported reflective
mutations, iterator prototypes, prototype replacement, dynamic keys, unknown
targets, and shadowed/aliased mutation callees. Negative controls cover project
class prototypes, separate fresh allocations, and named keys iteration never
reads. The signal declaration is copied from the existing tuple fixture's
published typings; every variant is also checked against the installed published
solid-js 2.0.0-rc.13 typings in an isolated scratch project.

Round 2 adds reflective call/apply/bind/Reflect.apply, direct/indirect eval and
Function construction, plus source-resolved reflective negative controls.
Context primitives and an exact source-derived StoreSetter must preserve the
intact tuple read; an opaque typed foreign argument must withhold it. Zero- and
primitive-argument opaque calls pin the bounded handoff premise described in
the ADR, not a claim that unseen callees cannot mutate globals.
Exact global Proxy construction preserves the read; a replaced constructor or
a declared-only ProxyConstructor type does not grant that exception.

The `AccessorRead` row specifically exercises source-slot uncertainty with the
fixture's descriptor projection. With real rc.13 typings, that direct accessor
shape has no row on either baseline or changed checker; the `TupleRead` closure
is the published-typing false-positive regression. No stub signature is looser
than rc.13 in a way that creates a finding.

Round 3 retains erased global-member, static-computed, namespace-alias,
destructured and function-constructor gateway possibilities. The minimized
string replacement of global Proxy must remain SC9012. Source factories that
return a gateway or mutator cannot discharge the returned callable by the
factory's own declaration. Closed local methods named Function/eval/constructor
and shadowed realm names remain controls; unknown members and reassigned
gateway bindings withhold authority. Retained-session edits insert and remove
gateways around intact Proxy and reflective-call controls in both directions.
