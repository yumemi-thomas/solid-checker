import { createSignal, type ComponentProps } from "solid-js";

const [count] = createSignal(0);
const [items] = createSignal(["a", "b"]);

// Positive: a type query is erased at runtime, so `typeof TypeRef` in a type
// is no reference that could call the function.
function TypeRef() {
  const value = count();
  return <b>{value}</b>;
}
export type TypeRefProps = ComponentProps<typeof TypeRef>;
export type TypeRefParameters = Parameters<typeof TypeRef>;
export type TypeRefReturn = ReturnType<typeof TypeRef>;

// Uncertifiable: the type query is accounted for, but `map` still calls it.
function TypeMapped(item?: unknown) {
  const value = count();
  return <b>{value}:{item}</b>;
}
export type TypeMappedProps = ComponentProps<typeof TypeMapped>;

// Uncertifiable: a runtime `typeof` is a value reference, not a type query.
function RuntimeTypeof() {
  const value = count();
  return <b>{value}</b>;
}
export const isFunction = typeof RuntimeTypeof === "function";

export function TypesPage() {
  return (
    <ul>
      <TypeRef />
      <TypeMapped />
      {items().map(TypeMapped)}
      <RuntimeTypeof />
    </ul>
  );
}
