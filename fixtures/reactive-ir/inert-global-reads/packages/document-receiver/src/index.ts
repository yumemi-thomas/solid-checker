import "solid-js";

// A document's `defaultView` is the global object, handed on here.
export const viewOf = (element: HTMLElement): Window | null => element.ownerDocument.defaultView ?? null;
