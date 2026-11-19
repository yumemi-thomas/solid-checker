# after-await-setter-callee

**Claim (ADR 1097, reopened narrowly).** An argument-less call after an await whose callee does nothing but call exact setters runs no project code and reads no source, as a direct setter call there does (`library-runs-nothing`). `AfterAwaitSafe::SetterOnlyCallee` (`after_await_accounting/setter_callee.rs`) asks for exactly this form:
- the callee is a same-file `const` that is never written and not exported, or a `const x = y` alias of one;
- it holds a parameterless function literal that is neither async nor a generator, or an action whose generator has no `yield`;
- its body holds no function, class, member read, spread, iteration, `yield`, `await` or tagged template, and references no identifier except the callees of its calls;
- every call in it is an exact setter call that runs nothing (inert arguments, `equals` absent or `false`). In a wrapper's body only, a call may also be an argument-less call of such an action.

Clean: every call in `Clean`'s handler (`wrapUse`, `wrapShort`, the action alias `useIt`, `callsAction`, the local closure).

Uncertifiable, one twin per excluded shape, in `Refused`:
- a member read, another identifier, another call, an updater function, iteration, a class;
- a signal with its own `equals`;
- a helper that runs a callback, a parameter;
- a reassigned `let`, an action generator that yields, an export, an IIFE.

Violation or uncertifiable, never clean: `Twin`'s wrapper reads after the memo's await.
