import { createSignal } from "solid-js";
import { runFirst, runSecond } from "./App";

const [other] = createSignal(1);

export function third() {
  runFirst(() => {
    other();
  });
  runSecond(() => {
    other();
  });
  runSecond(() => {
    other();
  });
}
