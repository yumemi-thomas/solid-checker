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
export function Plain() {
  return (
    <p>
      <Comp />
      <Inner />
    </p>
  );
}
