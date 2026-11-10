// ADR 0942: SC4001 leaves a site that runs under a leaf owner to SC3001's claim (the leaf owner refuses the call), and ends it only where
// SC3001 has a row at the call. `Direct` is that row (a proven SC3001 violation, SC4001 silent). `Layered` runs the same `onCleanup`
// inside `untrack`, which keeps the leaf as the owner (Chrome reports CLEANUP_IN_FORBIDDEN_SCOPE), but the SC3001 walk does not follow the
// layer: there is no row to hand the site to, so SC4001 answers with an uncertifiable row of its own instead of silence.
import { createTrackedEffect, onCleanup, untrack } from "solid-js";

export function Direct() {
  createTrackedEffect(() => {
    onCleanup(() => {});
  });
  return <div />;
}

export function Layered() {
  createTrackedEffect(() => {
    untrack(() => {
      onCleanup(() => {});
    });
  });
  return <div />;
}
