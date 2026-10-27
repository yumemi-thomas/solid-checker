// inferay useRepositoryWorkbench.tsx:742-854 and shared/lib/dom.tsx:280-292:
// the one synchronous dispatch of this project, on an element, in a keydown
// listener registered through a project wrapper of `window.addEventListener`.
// The listener runs only from event delivery, but the wrapper's parameter is
// not followed, so the site withholds every handler's owner and read proofs
// (ADR 0356), including those of the unrelated toolbar below.
import { createEffect, createSignal } from "solid-js";

function listenWindowEvent(type: string, listener: (event: KeyboardEvent) => void): () => void {
  const eventListener = listener as EventListener;
  window.addEventListener(type, eventListener);
  return () => window.removeEventListener(type, eventListener);
}

export function Workbench(props: { active: boolean }) {
  let rail: HTMLElement | undefined;
  const [mode, setMode] = createSignal<"split" | "unified">("split");
  const handleDiffKeyboardNavigation = (event: KeyboardEvent) => {
    if (event.key !== "j") return;
    rail?.querySelector("[aria-label='history']")?.dispatchEvent(new CustomEvent("graph-keyboard", { detail: event }));
  };
  createEffect(
    () => props.active,
    (active) => (active ? listenWindowEvent("keydown", handleDiffKeyboardNavigation) : undefined),
  );
  return (
    <section ref={rail}>
      <button onClick={() => setMode(mode() === "split" ? "unified" : "split")}>{mode()}</button>
    </section>
  );
}
