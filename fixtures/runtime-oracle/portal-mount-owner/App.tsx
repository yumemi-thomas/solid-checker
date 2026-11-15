import { createMemo } from "solid-js";
import { Portal } from "@solidjs/web";

// ADR 0992: `mount` is no tracked-children slot: the render effect's apply reads it again under `untrack(mount)`, so the `createMemo` written in it is created on an apply's
// stack (PRIMITIVE_IN_EFFECT_CALLBACK, Chrome rc.21) and the checker must keep its rows. Its own case: the halt that error causes must not hide a diagnostic of another.
export function Mounted() {
  return <Portal mount={createMemo(() => document.body)()}>mounted</Portal>;
}
