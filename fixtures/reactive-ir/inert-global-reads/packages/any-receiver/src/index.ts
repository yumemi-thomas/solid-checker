import "solid-js";

// An `any` receiver may be a document: its `defaultView` may be the global object.
export const viewOf = (props: any): unknown => props.defaultView ?? "pages";
