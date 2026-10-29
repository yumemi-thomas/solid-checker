import { createEffect, createMemo } from "solid-js";
import "./setup";

// Uncertifiable (SC1001 stays): the app loads a module only declared
// (`setup.d.ts`), whose evaluation no analyzed file shows.
export function C() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  return <div />;
}
