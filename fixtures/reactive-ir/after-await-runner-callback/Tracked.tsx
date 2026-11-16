import { createEffect, createMemo } from "solid-js";
import { pause } from "./signals";

// The program holds a tracked async computation with something after a suspension, so no census could end the sites of the test file (there is no census reason anyway).
export function Tracked() {
  const m = createMemo(async () => {
    await pause();
    return 1;
  });
  createEffect(
    () => m(),
    () => {},
  );
  return <div />;
}
