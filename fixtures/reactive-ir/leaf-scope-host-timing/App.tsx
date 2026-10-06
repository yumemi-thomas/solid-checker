import { onCleanup, onSettled } from "solid-js";

function register() {
  onCleanup(() => {});
}

export function PromiseExecutorForbidden() {
  onSettled(() => { new Promise<void>(() => { register(); }); });
  return <div />;
}
export function PromiseExecutorClean() {
  onSettled(() => { new Promise<void>(() => {}); });
  return <div />;
}
export function PromiseResolveStillOpen() {
  onSettled(() => { new Promise<void>((resolve) => { resolve(); }); });
  return <div />;
}
export function NodeListForbidden() {
  onSettled(() => {
    const host = document.createElement("div");
    host.appendChild(document.createElement("span"));
    const nodes: NodeList = host.childNodes;
    nodes.forEach(register);
  });
  return <div />;
}
export function NodeListClean() {
  onSettled(() => {
    document.querySelectorAll("div").forEach((node) => {
      node.setAttribute("data-ready", "yes");
    });
  });
  return <div />;
}
export function MediaActionClean() {
  onSettled(() => {
    navigator.mediaSession.setActionHandler("play", () => {});
  });
  return <div />;
}
export function MediaQueryClean() {
  onSettled(() => {
    window.matchMedia("(min-width: 1px)").addListener(() => {});
  });
  return <div />;
}
export function GlobalListenerClean() {
  onSettled(() => {
    addEventListener("resize", () => {});
    globalThis.addEventListener("resize", () => {});
  });
  return <div />;
}
export function ExistingObserverRows() {
  onSettled(() => {
    new MutationObserver(() => {});
    new ResizeObserver(() => {});
    new IntersectionObserver(() => {});
    new PerformanceObserver(() => {});
    new ReportingObserver(() => {});
  });
  return <div />;
}
export function MediaQuerySynchronousDispatch() {
  onSettled(() => {
    const query = window.matchMedia("(min-width: 1px)");
    query.addListener(register);
    query.dispatchEvent(new Event("change"));
  });
  return <div />;
}
export function GlobalSynchronousDispatch() {
  onSettled(() => {
    addEventListener("leaf-test", register);
    dispatchEvent(new Event("leaf-test"));
  });
  return <div />;
}
export function ListenerObjectStillOpen() {
  onSettled(() => {
    addEventListener("leaf-test", { handleEvent: register });
  });
  return <div />;
}
export function WrappedNodeListCallback() {
  onSettled(() => {
    const host = document.createElement("div");
    host.appendChild(document.createElement("span"));
    host.childNodes.forEach((register satisfies () => void));
  });
  return <div />;
}
declare const customHost: {
  setActionHandler(action: string, callback: () => void): void;
};
export function SameNamedProjectMethod() {
  onSettled(() => { customHost.setActionHandler("play", register); });
  return <div />;
}
export function ShadowedGlobal(props: {
  addEventListener: (name: string, callback: () => void) => void;
}) {
  const addEventListener = props.addEventListener;
  onSettled(() => { addEventListener("resize", register); });
  return <div />;
}
