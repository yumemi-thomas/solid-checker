// A named function the runtime reaches only through a primitive's *options
// object* -- `createEffect(compute, { effect, error })`. The AST records the
// object's identifier-valued `effect`/`error` properties, and reachability has
// to follow them: without that edge `applyValue` is unreachable and the write
// through `setTotal` inside it disappears from the IR.
//
// Declared as a function statement, module-scoped, lowercase and not exported
// on purpose: no other reachability root covers for the options-object edge.
import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);
const [total, setTotal] = createSignal(0);

function applyValue(value: number) {
  setTotal(value + total());
}

function reportError() {
  setTotal(total());
}

createEffect(() => count(), { effect: applyValue, error: reportError });
