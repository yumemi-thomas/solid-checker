import { onCleanup, onSettled } from "solid-js";

// An impostor receiver, cast to the element type: the exact `lib.dom` declaration is what the types name where the receiver arrives as a
// prop, and the object holds a `focus` of its own.
const fake = {
  focus() {
    onCleanup(() => {});
  },
} as unknown as HTMLInputElement;

function Focuser(props: { target: HTMLInputElement }) {
  onSettled(() => {
    props.target.focus();
  });
  return <div />;
}

export function Leaf() {
  return <Focuser target={fake} />;
}
