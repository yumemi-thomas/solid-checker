import { createEffect } from "solid-js";
import { createStaticStore } from "@solid-primitives/static-store";

// Copying a function-valued seed must not invoke it during construction.
createStaticStore({});
createStaticStore({ count: 0, nullable: null, fn: () => {
  throw new Error("function seed invoked during construction");
} });
createStaticStore({
  get count() {
    createEffect(() => 0, () => {});
    return 0;
  }
});

export default function App() { return <p>done</p>; }
