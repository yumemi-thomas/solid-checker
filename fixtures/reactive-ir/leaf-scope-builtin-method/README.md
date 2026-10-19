# Built-in methods in a leaf scope

A call written directly in an `onSettled` callback runs in that leaf scope,
so the checker must know it creates no primitive and registers no cleanup.
A standard-library method is such a call, but only its resolved declaration
says so, and Type Facts resolved an argumentless call only when asked
(ADR 0192).

- `BuiltinMethods`: `dialog.focus()` and
  `document.body.getBoundingClientRect()` take no argument and are
  standard-library methods. `getBoundingClientRect` is clean; `focus()`
  dispatches focus events synchronously, so it is `SC9012` uncertifiable
  again (ADR 0329; clean between ADR 0192 and ADR 0329).
- `OptionalBuiltin`: `maybeDialog?.focus()` stays `SC9012`. Type Facts states
  no resolved call for an optional call, with or without arguments; that is
  the producer's remaining gap.
- `UnknownMethod`: `renderer.setSize()` is a method of a declared object
  with no body in the project. It stays `SC9012` uncertifiable.

The stub `solid-js.d.ts` is `leaf-owner`'s, whose `onSettled` signature is
the published one.
