# A forwarded callback whose callee has no body leaves the domain open

`schedule` hands its callback to an ambient `declare function` -- a declaration
with no runtime artifact behind it, so the timing of the invocation is not
available anywhere in this package. `plain` is the sibling that has nothing to
do with callbacks.

Both exports are published, and `schedule`'s summary closes *nothing*: an empty
`callbacks` list with `callbacks` absent from `closed` is the open answer, and
the review plan carries the corresponding open claim. Silence with the domain
*closed* would be the negative claim "invokes no caller-supplied callback",
which is false here.

The stable-v1 documents no longer carry a per-export `callbacks: { status }`
field; open versus proved is expressed by whether the domain appears in the
summary's `closed` set and, in the plan, by whether the claim lands in
`unresolvedClaims` or in `closureCandidates`. `unresolved-callee-callback` pins
the neighbouring case where the callee is a real call target that cannot be
resolved, rather than a declaration with no body at all.
