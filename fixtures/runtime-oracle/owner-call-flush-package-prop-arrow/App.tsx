import { flush } from "solid-js";
import { Pkg } from "steplib";

// B2 (the review of #225, F1): a `flush()` written in a multi-line arrow that is a prop of a component of an unread package. The package runs the arrow in a
// `createTrackedEffect` callback, a leaf owner's: Chrome rc.21 throws FLUSH_IN_TRACKED_QUEUE_CALLBACK at the flush (the arrow is the only application frame). The checker's row for the flush
// (SC3001 `leaf-operation-unaccounted`) yielded to the JSX element's own row, which covers the flush by span but is reported at the element's start, so no row stood at the flush.
export function Prop() {
  return (
    <Pkg
      render={() => {
        flush();
      }}
    />
  );
}
