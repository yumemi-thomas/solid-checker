# after-await-setter-callee

**Claim (ADR 1097, reopened narrowly).** An argument-less call after an await whose callee does nothing but call exact setters runs no project code and reads no source, as a direct setter call there does (`library-runs-nothing`). `AfterAwaitSafe::SetterOnlyCallee` (`after_await_accounting/setter_callee.rs`) asks for exactly this form:
- the callee is a same-file `const` that is never written and not exported, or a `const x = y` alias of one;
- it holds a parameterless function literal that is neither async nor a generator, or an action whose generator has no `yield`;
- its body holds no function, class, member read, spread, iteration, `yield`, `await` or tagged template, and references no identifier except the callees of its calls;
- every call in it is an exact setter call that runs nothing (inert arguments, `equals` absent or `false`). In a wrapper's body only, a call may also be an argument-less call of such an action.

Clean: every call in `Clean`'s handler (`wrapUse`, `wrapShort`, the action alias `useIt`, `callsAction`, the local closure).

Uncertifiable, one twin per excluded shape, in `Refused`:
- a member read, another call, an updater function, iteration, a class;
- a signal with its own `equals`;
- a helper that runs a callback, a parameter;
- a reassigned `let`, an action generator that yields, an export.

Since step 1, two `Refused` calls are clean: `otherIdentifier` (`setW(n)`, where `n` is a `const` primitive, an inert argument) and the IIFE.

Violation or uncertifiable, never clean: `Twin`'s wrapper reads after the memo's await.

**Widened (step 1 of the ruling of 2026-11-19).** A qualifying body may also call:
- other qualifying callees, transitively, at most four deep and never one already on the chain;
- IIFEs whose bodies qualify;
- setters through exact `const` aliases;
- `refresh(accessor)`, ADR 1089 C's untouched target;
- a plain store setter with a literal-only updater (ADR 1089 D, `setter_runs_nothing`).

Clean: `Widened`'s handler.

Uncertifiable, in `WidenedRefused`: a callee that calls a reader, recursion, a chain past the depth bound, a `let` alias, and `refresh` of a member.
