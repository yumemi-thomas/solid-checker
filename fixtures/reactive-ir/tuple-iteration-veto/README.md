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
