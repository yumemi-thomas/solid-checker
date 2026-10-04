import { createEffect, createSignal, onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// A component receives `ref` as an ordinary prop and calls it from its own
// body here, under its owner, so the callback is not an ownerless ref
// application. The compiler's ref-application role holds only for intrinsic
// elements, whose `ref` the runtime applies through `ref()` with no owner.
function Box(props: { ref?: (element: HTMLInputElement) => void }) {
  const element = document.createElement("input");
  props.ref?.(element);
  return <span />;
}

// Negative: owned by Box's body.
function ComponentRef() {
  const [count] = createSignal(0);
  return (
    <Box
      ref={(element) => {
        onCleanup(() => element.remove());
        createEffect(
          () => count(),
          (value) => {
            element.value = String(value);
          },
        );
      }}
    />
  );
}

// Positive control: an intrinsic element's ref callback runs with no owner.
function IntrinsicRef() {
  return <input ref={(element) => onCleanup(() => element.remove())} />;
}

render(() => <ComponentRef />, document.body);
render(() => <IntrinsicRef />, document.body);
