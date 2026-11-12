import { onCleanup } from "solid-js";

// R2, the module: `Comp` is exported and a tag in `Plain` renders it; `Inner` is not exported.
export function Comp() {
  onCleanup(() => {});
  return <p>comp</p>;
}
function Inner() {
  onCleanup(() => {});
  return <p>inner</p>;
}
export function Plain() {
  return (
    <p>
      <Comp />
      <Inner />
    </p>
  );
}
