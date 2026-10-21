# owner-invoked-literal-forms

**Claim (ADR 0347).** An invoked function literal runs its body inside the call, so the call is an owner edge under the code around it. Also edges: `.call` and `.apply` on a literal, `new` on a plain function expression (an arrow, an async function or a method throws before its body), a comma expression's last operand (`(0, () => …)()`), and a static field or static block initializer. An instance field initializer runs at construction and stays out of the edges (`docs/open-work.md`).

Reported at module scope: `viaCall`, `viaApply`, `viaNew`, `viaComma`, `Static.x`, and `viaContinuation` (an async literal's code after its first `await`, ADR 0333).

Silent: `viaGenerator` (calling a generator runs none of its body); `Comp` (rendered by a tag, so its body is owned).
