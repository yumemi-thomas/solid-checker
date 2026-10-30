import { createEffect } from "solid-js";
import { useData } from "./hooks";

// Violation (SC5001): the hook's object is kept and its member read in a user
// effect's apply, which the first render's tail flush runs while the memo the
// hook created is pending.
export function HookMember() {
  const vals = useData();
  createEffect(
    () => 1,
    () => {
      vals.data();
    },
  );
  return <p>static</p>;
}
