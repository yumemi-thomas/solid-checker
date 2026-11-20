import { onCleanup, onSettled } from "solid-js";
import { register } from "fz-typed-listener";

// An installed package imported by name resolves to its declaration file, and its runtime source (which registers the listener) is no analysed file: code the build
// does not read.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    register(el);
  }} />;
}
