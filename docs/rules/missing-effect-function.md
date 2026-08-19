# missing-effect-function

`SC7001` · **error** · violation

`createEffect` is called without an effect (apply) function — the removed Solid 1.x
single-callback form.

## What it does

Flags the published, deprecated one-argument overload and values that only
type-check because a TypeScript assertion hid their runtime shape. An absent
apply function throws `MISSING_EFFECT_FN` in dev; a cast-hidden non-function
crashes when the runtime reads `.effect` off it or calls it. Raw `undefined`,
`null`, primitive, array, and invalid object arguments are already rejected by
the published types and therefore stay silent. The `{ effect, error }` object
form is legal only when `effect` is callable; a cast-hidden literal in that
field remains a proven runtime failure. A compiler-proven callable identifier
stays silent. An `any`, unresolved value, non-null assertion over a nullable
value, or non-exact object whose runtime apply function is not known produces
an **uncertifiable** finding: it may be callable, but silence would falsely
certify that it is.

When an otherwise-reporting call is covered by `"use server"`, the finding is
**uncertifiable**. No core Solid package reads that directive — it is a
framework and bundler convention — so the spelling alone proves neither client
nor server execution. The call fails on the client, while Solid's server entry
evaluates through `serverEffect` and ignores the apply argument. The v1 rule
preserves the same uncertainty.

A type-correct spread call is likewise **uncertifiable**.
`createEffect(...operands)` hides the call's arity, so neither a missing apply
function nor a safe one is proven. An invalid spread call remains TypeScript's
diagnostic and stays silent.

## Why is this bad?

Solid 2.0 split effects into two phases: `createEffect(compute, apply)`. The
compute function runs in the tracking phase and returns a value; the apply function
runs after flush, receives that value, performs the side effect, and may return
cleanup. The 1.x single-callback form no longer exists — with only one function,
there is no apply phase to run the side effect in, and mixing tracking with side
effects is exactly what the split removed.

## Examples

Examples of **incorrect** code for this rule:

```tsx
// Solid 1.x form — no apply function.
createEffect(() => {
  console.log(name());
});

// Type assertions can hide non-callable runtime values from TypeScript.
createEffect(() => name(), null as unknown as (value: string) => void);
createEffect(() => name(), {} as unknown as (value: string) => void);
createEffect(() => name(), {
  effect: 5 as unknown as (value: string) => void,
  error: (error) => reportError(error),
});
```

Examples of **correct** code for this rule:

```tsx
createEffect(
  () => name(), // compute: tracks dependencies, returns a value
  (value) => {
    // apply: side effect, runs untracked after flush
    const id = setInterval(() => console.log(value), 1000);
    return () => clearInterval(id); // optional cleanup
  },
);

// With error handling, the second argument is an object:
createEffect(() => data(), {
  effect: (value) => render(value),
  error: (err, cleanup) => reportError(err),
});
```

## How to fix

Split the callback: reactive reads go in the compute function, the side effect in
the apply function, and cleanup is returned from apply. Two adjacent changes from
1.x to keep in mind: the `initialValue` second argument is gone (use a default
parameter, `(prev = 0) => ...`), and the apply phase runs untracked, so extract
everything it needs in compute.

## Related

- [strict-read-untracked](strict-read-untracked.md) — reads in the apply phase
- [no-owner-effect](no-owner-effect.md) — an effect with no owner at all
