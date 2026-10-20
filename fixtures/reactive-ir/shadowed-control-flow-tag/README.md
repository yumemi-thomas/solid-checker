# shadowed-control-flow-tag

**Claim.** A JSX tag names a dialect primitive only by symbol identity. A local component spelled `For`
shadows the imported `For`; it never has to call its child, so a read in that child is not a strict read
(`jsx_primitive_name`, `solid-reactive-ir/src/lib.rs`; the spelling fallback still applies to a tag the
entity table does not resolve).

- `Shadowed`: `SC1001` uncertifiable (the child is the local component's `children` prop; nothing proves
  when it runs it, ADR 0326).
- `Real`: violation (the imported `For` runs its child while rendering).

Stubs are copied from `prop-head-get`.
