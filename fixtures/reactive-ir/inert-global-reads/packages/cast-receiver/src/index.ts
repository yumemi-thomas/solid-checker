import "solid-js";

interface ViewProps {
  readonly defaultView?: "pages" | "infinite" | undefined;
}

// The assertion types the read as a string, but the value under it is the document: its `defaultView` is the global object.
export const view = (document as unknown as ViewProps).defaultView ?? "pages";
