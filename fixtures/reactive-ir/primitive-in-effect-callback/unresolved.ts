import { createEffect, createRoot } from "solid-js";
import { apply } from "./not-there";

// The apply is a name from a module that does not resolve: nothing is placed,
// so SC4005 stays silent (the unresolved apply is SC4001's and SC7001's
// uncertifiable result, pinned in the snapshot).
createRoot(() => {
  createEffect(() => 1, apply);
});
