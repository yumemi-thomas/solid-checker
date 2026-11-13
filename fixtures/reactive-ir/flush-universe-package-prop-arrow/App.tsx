import { flush } from "solid-js";
import { Pkg } from "steplib";

// Batch 11, B2 (the review of #225, F1): a flush's own row is not ended by another site's row that covers it. `steplib` is a package the build does not read (its manifest names no
// Solid package) and `Pkg` runs its `render` prop in a `createTrackedEffect` callback, a leaf owner's: rc.21 throws FLUSH_IN_TRACKED_QUEUE_CALLBACK at the flush, and the arrow is the only
// application frame. The flush keeps its uncertifiable row of each code (`leaf-operation-unaccounted` is the one that claims it), and the element's JSX creation (an owner-call site of its
// own, ADR 0971) has its rows at the element. A first build of ADR 0971 ended the flush's row on the element's, which covers the flush by span but is reported at the element's start, where
// the judge places a multi-line finding: no row stood at the flush.

// A multi-line arrow: the flush is two lines below the element's start.
export function Prop() {
  return (
    <Pkg
      render={() => {
        flush();
      }}
    />
  );
}

// A one-line arrow: the flush is on the element's line, at another column.
export function OneLine() {
  return <Pkg render={() => flush()} />;
}
