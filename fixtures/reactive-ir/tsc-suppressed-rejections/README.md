# A rejection nothing reports (ADR 0830)

Invariant 1 keeps a rule silent where `tsc` reports the same code. Under a
`// @ts-ignore` or `// @ts-expect-error` on the line above, or a leading
`// @ts-nocheck`, `tsc` reports nothing there, and the runtime still throws
(rc.21 in Chrome: MISSING_EFFECT_FN for an absent apply, REACTIVITY_HALTED for
a non-callable apply or a non-function cleanup).

- `Exact.ts`, `exact`: a one-argument `createEffect` (TS2554 at the callee), a
  numeric apply (TS2345) and a `null` apply, each under a directive both
  compilers honour on that line: `missing-effect-function` violations.
- `Exact.ts`, `elsewhere`: the directive covers the line above, which is code,
  so TS2554 stands on the call: silent.
- `Exact.ts`, `returns`: an apply returning a number and `onSettled(() => 1)` on
  a suppressed line: `SC9012` uncertifiable, `solid-call-rejected-unchecked`.
  A function literal returned there is a cleanup: nothing.
- `Block.ts`: a block directive over two lines, which tsc 5 files under its last
  line and tsgo under its first: only one may suppress, uncertifiable.
- `Nocheck.ts`: the leading `// @ts-nocheck` pragma: a violation.
- `Positions.tsx`: `uncalled-accessor`'s positions TypeScript closes (a typed
  native attribute, a class object value, a computed key, an arithmetic
  operand), each suppressed: uncertifiable; with the directive on another
  line (`Elsewhere`), silent.
- `Server.ts`: `server-function-rich-argument`'s arity arm (TS2554)
  suppressed: uncertifiable; with the directive elsewhere, silent.

The stub is as strict as the published typings for these calls
(`EffectFunction` returns `(() => void) | void`, no one-argument overload,
`onSettled(callback: () => void | (() => void))`).
