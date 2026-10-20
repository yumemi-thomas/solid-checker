// A named import of one of the five names rc.9's typings re-export without
// declaring: under skipLibCheck it is untyped, and the call is not the
// primitive, so the gap is this project's.
import { createErrorBoundary, untrack } from "solid-js";

export const view = createErrorBoundary(
  () => untrack(() => "ready"),
  () => "failed"
);
