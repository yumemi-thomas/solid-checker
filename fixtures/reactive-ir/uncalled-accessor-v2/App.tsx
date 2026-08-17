// SC1005 `uncalled-accessor` after the 2026-08-17 narrowing
// (docs/precision-backlog.md).
//
// Three of the rule's six value positions were ones the type system closes, in
// both dialects, so they are gone:
//
//   a native JSX attribute      `<div title={count} />` -- TS2322,
//                               `Accessor<number>` is not assignable to the
//                               attribute's type;
//   a class object value        `<div class={{ active: isActive }} />` --
//                               TS2322 against 2.0's
//                               `Record<string, boolean>`;
//   a computed property access  `table[label]` -- TS2538, "cannot be used as
//                               an index type".
//
// What this fixture pins is the other half: the positions where an uncalled
// accessor is *legal* to TypeScript and silently wrong at runtime.
import { createSignal } from "solid-js";

// String concatenation accepts any operand, so `tsc` is silent and the
// rendered text is the accessor's own source — finding.
export function ConcatOperand() {
  const [label] = createSignal("world");
  return <div>{"hello " + label}</div>;
}

// A unary operand is likewise not rejected — finding.
export function UnaryOperand() {
  const [count] = createSignal(0);
  return <div>{!count ? "empty" : "full"}</div>;
}

// A template literal stringifies each interpolation whatever its type — finding.
export function TemplateInterpolation() {
  const [count] = createSignal(0);
  return <div>{`count is ${count}`}</div>;
}

// Called accessors read the value reactively — silent.
export function CalledAccessor() {
  const [label] = createSignal("world");
  return <div>{"hello " + label()}</div>;
}

// Passing the accessor on hands the callee something it can call later — silent.
export function PassedOnAccessor() {
  const [count] = createSignal(0);
  const show = (read: () => number) => read();
  return <div>{show(count)}</div>;
}

// Now-silent negatives, kept because they are the exact markup the narrowing
// was made for. Each is a TypeScript diagnostic in its own right, which is why
// the checker no longer speaks: a class-object value, a native value
// attribute, and a computed key.
export function TypedPositions() {
  const [isActive] = createSignal(false);
  const [count] = createSignal(0);
  const [label] = createSignal("a");
  const table: Record<string, number> = { a: 1 };
  return (
    <div class={{ active: isActive }} title={count}>
      {table[label]}
    </div>
  );
}

// The `children` attribute rides child insertion, which calls zero-argument
// functions (code-read on `@solidjs/web@2.0.0-rc.0`: `insert`/`flatten`), so an
// uncalled accessor there is correct usage. It was already silent; the dialect
// predicate that expressed it went with the native-attribute position it
// qualified, and this case now stays silent for the simpler reason that the
// position itself is gone.
export function ChildrenAttributeAccessor() {
  const [count] = createSignal(0);
  return <div children={count} />;
}
