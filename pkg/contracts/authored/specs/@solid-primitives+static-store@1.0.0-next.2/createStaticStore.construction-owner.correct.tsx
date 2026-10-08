import { createEffect, createRoot } from "solid-js";
import { createStaticStore } from "@solid-primitives/static-store";

// These ownerless construction-only controls must remain warning-free.
createStaticStore({});
createStaticStore({ count: 0, nullable: null, fn: () => {
  throw new Error("function seed invoked during construction");
} });
createRoot(dispose => {
  createStaticStore({
    get count() {
      createEffect(() => 0, () => {});
      return 0;
    }
  });
  dispose();
});

export default function App() { return <p>done</p>; }
