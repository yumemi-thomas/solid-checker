import { onCleanup, onSettled } from "solid-js";
import { spread } from "@solidjs/web";

// `spread` and `assign` write the props of an object onto an element under their keys, `onFocus` among them: a use of either is a registration.
const h = () => {
  onCleanup(() => {});
};
export function Leaf() {
  const input = document.createElement("input");
  spread(input, { onFocus: h });
  onSettled(() => {
    input.focus();
  });
  return <div ref={(box) => { box.append(input); }} />;
}
