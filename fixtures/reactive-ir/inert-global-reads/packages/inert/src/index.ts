// Loads Solid, so a global this package wrote would run code that may call an action. It writes none: each read below leaves the global object where it is.
import "solid-js";

export interface ViewProps {
  readonly defaultView?: "pages" | "infinite" | undefined;
}

// (a) A string-literal `in` test of the unshadowed global object reads nothing off it (@tanstack/query-core's `isServer`).
export const isServer = typeof window === "undefined" || "Deno" in globalThis;

// (b) A type query is erased: nothing of it runs (kui `RecordRow.tsx:82`).
let timer: ReturnType<typeof globalThis.setTimeout> | undefined;
export function stop(): void {
  if (timer !== undefined) globalThis.clearTimeout(timer);
  timer = undefined;
}

// (c) `defaultView` of a value whose type is no document, read as a string (kui `MessagesTab.tsx`).
export const viewOf = (props: ViewProps): "pages" | "infinite" => props.defaultView ?? "pages";
