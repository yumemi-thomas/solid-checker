// app-game typegpu-draw-app-example ui/PointerDebugOverlay.tsx:137-144,299-303: a child hands a callback prop to `window.addEventListener`
// behind a presence check (the app uses solid-primitives' `makeEventListener`; lib.dom's is the same hand-over).
import { createSignal, onSettled } from "solid-js";

function WindowEvents(props: { onPointerDown?: (event: PointerEvent) => void }) {
  onSettled(() => {
    if (props.onPointerDown) window.addEventListener("pointerdown", props.onPointerDown);
    return () => {
      if (props.onPointerDown) window.removeEventListener("pointerdown", props.onPointerDown);
    };
  });
  return null;
}

export function Overlay() {
  const [last, setLast] = createSignal<string | null>(null);
  return (
    <>
      <WindowEvents onPointerDown={(event) => setLast(event.pointerType)} />
      <p>{last()}</p>
    </>
  );
}
