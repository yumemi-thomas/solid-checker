import { onCleanup } from "solid-js";

// B2 (the review of #221), the module: `Comp` is exported and rendered by a tag in `Plain`; `Inner` is not exported and only a tag renders it (the control).
export function Comp() {
  onCleanup(() => {});
  return <p>comp</p>;
}
function Inner() {
  onCleanup(() => {});
  return <p>inner</p>;
}
// R1 (the re-review of #221): the same components published by an export specifier, which is no export statement that contains the function: a declaration (`Spec`), a renamed
// one (`Aliased` as `Renamed`) and an arrow bound to a `const` (`Arrow` as `ArrowOut`). A tag in `Plain` renders each of them.
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
// N1 (the second re-review of #221): a `const` bound to a function expression that carries a name of its own. The specifier resolves to the `const`; `Own` (and `Same`, the same
// spelling twice) is visible inside the body only, and `function_binding_name` gave it first, so the export was not seen.
const Named = function Own() {
  onCleanup(() => {});
  return <p>named</p>;
};
const Same = function Same() {
  onCleanup(() => {});
  return <p>same</p>;
};
export { Spec, Aliased as Renamed, Arrow as ArrowOut, Named, Same };

export function Plain() {
  return (
    <p>
      <Comp />
      <Inner />
      <Spec />
      <Aliased />
      <Arrow />
      <Named />
      <Same />
    </p>
  );
}
