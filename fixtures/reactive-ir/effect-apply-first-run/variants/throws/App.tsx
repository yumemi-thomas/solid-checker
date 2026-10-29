import { createEffect, createMemo } from "solid-js";

declare const broken: boolean;

// Uncertifiable (SC1001 stays): the body may throw after creating the
// effect, and the mount fails before its flush.
export function C() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  if (broken) throw new Error("broken");
  return <div />;
}
