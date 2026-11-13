import { onCleanup } from "solid-js";

// R1 of the re-review of #221, the module: each component is written as a declaration, an arrow or a function expression and published by an export specifier, which is no export statement that
// contains the function. A tag in `Plain` renders each of them.
function Spec() {
  onCleanup(() => {});
  return <p>spec</p>;
}
function Aliased() {
  onCleanup(() => {});
  return <p>aliased</p>;
}
const Arrow = () => {
  onCleanup(() => {});
  return <p>arrow</p>;
};
// N1 of the second re-review of #221: a `const` bound to a function expression that carries a name of its own. The specifier resolves to the `const`; `Own` (and `Same`, the
// same spelling twice) is visible inside the body only, and `function_binding_name` gave it first.
const Named = function Own() {
  onCleanup(() => {});
  return <p>named</p>;
};
const Same = function Same() {
  onCleanup(() => {});
  return <p>same</p>;
};
function Inner() {
  onCleanup(() => {});
  return <p>inner</p>;
}
export { Spec, Aliased as Renamed, Arrow as ArrowOut, Named, Same };
export function Plain() {
  return (
    <p>
      <Spec />
      <Aliased />
      <Arrow />
      <Named />
      <Same />
      <Inner />
    </p>
  );
}
