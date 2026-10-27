// compass-ui apps/ui/src/store.ts:1109 (applyRoute): route-application writes reached from a bound navigate and a createEffect apply.
import { createEffect, createSignal } from "solid-js";

export function createRouteStore() {
  const [view, setView] = createSignal("bridge");
  const [channelId, setChannelId] = createSignal<string | null>(null);
  const [path, setPath] = createSignal("/");
  let navigate = (to: string): void => {
    setPath(to);
    applyRoute(to);
  };
  const go = (to: string): void => navigate(to);
  const bindRouter = (r: { navigate: (path: string) => void; currentPath: () => string }): void => {
    navigate = r.navigate;
    createEffect(
      () => r.currentPath(),
      (to) => applyRoute(to),
    );
  };
  function applyRoute(to: string): void {
    const [head, param] = to.split("/").filter(Boolean);
    if (head === "channel" && param) applyChannel(param);
    else setView("bridge");
  }
  function applyChannel(id: string): void {
    setView("channel");
    if (id === "") return void go("/");
    setChannelId(id);
  }
  return { view, channelId, path, go, bindRouter };
}
