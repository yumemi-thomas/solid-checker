import { createMemo, createSignal, createTrackedEffect } from "solid-js";
import { mk } from "./barrel";

function helper() {
  mk(() => 1);
}

export function App() {
  const [a] = createSignal(0);
  createTrackedEffect(() => {
    a();
    // Proved: the binder names the primitive.
    createMemo(() => 1);
  });
  createTrackedEffect(() => {
    a();
    // The binder names `mk`, so the attribution is an obligation.
    mk(() => 1);
  });
  createTrackedEffect(() => {
    a();
    // The same, through a helper.
    helper();
  });
  return <p>x</p>;
}
