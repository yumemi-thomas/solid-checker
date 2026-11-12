import { onCleanup } from "solid-js";

// R1 of the re-review of #221, the module: each component is written as a declaration (or an arrow) and published by an export specifier, which is no export statement that
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
function Inner() {
  onCleanup(() => {});
  return <p>inner</p>;
}
export { Spec, Aliased as Renamed, Arrow as ArrowOut };
export function Plain() {
  return (
    <p>
      <Spec />
      <Aliased />
      <Arrow />
      <Inner />
    </p>
  );
}
