// The 2.0-only SC1005 edges, decided against the pinned @solidjs/web rc.0:
// class-object values are truthiness-coerced (probed: ssrClassName renders
// the key for a function value), so an uncalled accessor there is
// always-truthy; the children attribute on a native element rides child
// insertion, which calls zero-argument functions (code-read: insert/flatten),
// so an uncalled accessor there is correct usage.
import { createSignal } from "solid-js";

// Object form: the accessor object is always truthy — finding.
export function ClassObjectValue() {
  const [isActive] = createSignal(false);
  return <div class={{ active: isActive }}>row</div>;
}

// Array form with a nested object entry: the same coercion — finding.
export function ClassArrayObjectValue() {
  const [isSelected] = createSignal(false);
  return <div class={["base", { selected: isSelected }]}>row</div>;
}

// Called accessors read the value reactively — silent.
export function ClassObjectValueCalled() {
  const [isActive] = createSignal(false);
  return <div class={{ active: isActive() }}>row</div>;
}

// The children attribute goes through child insertion, which calls the
// accessor reactively — silent in the 2.0 catalog.
export function ChildrenAttributeAccessor() {
  const [count] = createSignal(0);
  return <div children={count} />;
}

// Other native value attributes still receive the function object — the
// pre-existing SC1005 position stays a finding.
export function TitleAttributeAccessor() {
  const [count] = createSignal(0);
  return <div title={count}>count</div>;
}
