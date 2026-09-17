# Captured parameter-member reads do not prove direct call behavior

`captured` hands an arrow to a local function that only returns it; nobody calls
that arrow. Its parameter-member read must not be emitted as an untracked,
same-stack operation of `captured`. `direct` performs the matching member call
in its own body and retains its read proposal. `mixed` has both paths; the
compact read summary cannot represent the captured execution uncertainty, so
its read domain must remain open rather than silently closing around the
direct sibling. The signatures use ordinary structural TypeScript types.

The corpus pins proposal generation. The native test verifies the generated
direct-read evidence and checks that copying its read claim onto `captured`
still refuses with `parameter-rooted read has no exact implementation call or
use`. The generator retains uncertainty; the verifier has no new acceptance
rule. Incremental cache identity includes the invocation's execution context.
