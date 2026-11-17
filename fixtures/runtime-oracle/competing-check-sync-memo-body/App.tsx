import { createMemo } from "solid-js";

export function C1() {
  createMemo(async () => 1, { sync: true });
  return (
    <div>
    </div>
  );
}
