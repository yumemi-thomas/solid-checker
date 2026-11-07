import { createMemo } from "solid-js";
import { render } from "@solidjs/web";

// No catch in this module: the NotReadyError ends the module's evaluation.
const memo = createMemo(async () => 1);
const Uncaught = () => {
  memo();
  return <i />;
};
render(Uncaught, document.createElement("div"));
