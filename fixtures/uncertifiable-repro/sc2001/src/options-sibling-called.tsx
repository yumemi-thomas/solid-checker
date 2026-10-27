// app-game solid-svg-editor createViewportInteractions.ts:363 (options handed to createDefaultViewportTools): a function in an options
// object that a project function receives; the callee also calls another member (`options.drag()`), a signal accessor (an array-pattern
// binding of `createSignal`) that the proof cannot show is free of `this`.
import { createSignal } from "solid-js";

type Tools = { finish: () => void; drag: () => string | undefined };

function createTools(options: Tools) {
  return { handle: () => (options.finish(), options.drag()) };
}

export function createViewport() {
  const [drag, setDrag] = createSignal<string | undefined>("pan");
  function finish(): void {
    setDrag(undefined);
  }
  const tools = createTools({ finish, drag });
  return { tools, drag };
}
