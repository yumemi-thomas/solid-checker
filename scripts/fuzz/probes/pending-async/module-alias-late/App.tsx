// ADR 0511 negative: the alias is declared after the effect, whose first run meets it in its temporal dead zone.
import { createRenderEffect, createSignal } from "solid-js";
import { a1 } from "./helpers";

export function C1() {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    useIt();
  });
  const useIt = a1;
  return (
    <div>
    </div>
  );
}
