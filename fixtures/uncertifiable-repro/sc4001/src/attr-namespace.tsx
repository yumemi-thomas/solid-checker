// app-game sequence-editor panel-resize-handle.tsx:6-12,71-73: a Solid 1 `attr:` namespaced attribute on a custom
// element declared with that attribute, which the Solid 2 compiler refuses.
import type { ComponentProps } from "@solidjs/web";

declare module "@solidjs/web" {
  namespace JSX {
    interface IntrinsicElements {
      "resize-handle": ComponentProps<"div"> & { "attr:which": string };
    }
  }
}

export function ResizeHandle() {
  return <resize-handle attr:which="Bottom" />;
}
