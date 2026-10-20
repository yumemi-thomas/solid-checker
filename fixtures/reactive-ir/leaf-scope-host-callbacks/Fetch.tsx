import { createTrackedEffect, onCleanup, onSettled } from "solid-js";

declare const unknownFetch: typeof fetch;
declare const unknownAfterAwait: () => void;

export function NativeFetch() {
  onSettled(() => {
    void fetch("/api/status");
    void (fetch as typeof fetch)("/api/status", { method: "GET" });
  });
  return <div />;
}

export function AsyncFetch() {
  const load = async () => {
    await fetch("/api/status");
    unknownAfterAwait();
  };
  onSettled(() => { void load(); });
  return <div />;
}

export function OpaqueFetch() {
  onSettled(() => { void unknownFetch("/api/status"); });
  return <div />;
}

export function ShadowedFetch() {
  function fetch(_input: RequestInfo | URL) {
    onCleanup(() => {});
    return 0;
  }
  createTrackedEffect(() => { void fetch("/api/status"); });
  return <div />;
}

export function FetchWithGetter() {
  onSettled(() => {
    void fetch("/api/status", {
      get method() { onCleanup(() => {}); return "GET"; },
    });
  });
  return <div />;
}
