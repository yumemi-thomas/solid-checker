import type { Component } from "solid-js";

function withTheme<T>(component: T): T {
  return component;
}

// Bound through a call: the initializer is the `withTheme(...)` call rather than
// the function, so `initializer_function` is false. Resolution sees through the
// call initializer, so this is classified as a component and its destructured
// props are reported — destructuring here breaks reactivity exactly as it does
// in an unwrapped component. This case was silent before ADR 0002 widened the
// resolution.
export const Wrapped = withTheme(({ name }: { name: string }) => <h1>{name}</h1>);

// Mitigated separately: a props-named parameter is recognised without the name,
// via `named_for_props` on the ESLint-compatible surface.
export const WrappedPropsNamed = withTheme((props: { name: string }) => (
  <h1>{props.name}</h1>
));

// Must NOT be classified: object and array initializers merely *contain* a
// function, so naming their arrow after the binding would mint components out of
// data. Only function and call initializers qualify.
export const Handlers = { onClick: ({ name }: { name: string }) => name };
export const Callbacks = [({ name }: { name: string }) => name];
