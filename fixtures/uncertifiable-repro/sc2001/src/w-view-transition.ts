// app-game folder-tabs createScreenRoute.ts:36-41, 69: the closure that writes is handed to
// lib.dom's `document.startViewTransition`, and is also called directly.
import { createSignal } from "solid-js";

export function createFullscreen(screen: () => HTMLElement | undefined) {
  const [fullscreen, setFullscreen] = createSignal(false);
  function update(next: boolean) {
    const element = screen();
    const commit = () => {
      element?.setAttribute("data-route-layout", "");
      setFullscreen(next);
    };
    if (typeof document.startViewTransition !== "function") {
      commit();
      return;
    }
    void document.startViewTransition(commit).finished;
  }
  return { fullscreen, update };
}
