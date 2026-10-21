# owner-invoked-literal-forms

**Claim (ADR 0347 amendment).** An invoked function literal runs its body inside the call, so the call is an owner edge under the code around it (ADR 0347). Four more forms are edges: `.call` and `.apply` on a literal, `new` on a plain function expression (an arrow, an async function or a method throws before its body runs), a comma expression whose last operand is the literal (`(0, () => …)()`), and a static field or static block initializer, which runs at class evaluation under the code around the class. An instance field initializer runs at construction and stays out of the edges (a recorded gap in `docs/open-work.md`).

Reported at module scope, where no owner is current:
- `viaCall` and `viaApply` (`.call`, `.apply`), `viaNew` (`new`), `viaComma` (comma), `Static.x` (static field).
- `viaContinuation`: an async literal's code after its first `await` is a continuation with no owner (ADR 0333).

Silent controls:
- `viaGenerator`: calling a generator runs none of its body.
- `Comp`: a component, because a tag renders it (`rendered`), so its body is owned; the same forms run under the component's owner.
