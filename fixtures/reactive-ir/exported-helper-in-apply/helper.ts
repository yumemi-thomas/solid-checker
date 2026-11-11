import { createMemo } from "solid-js";

export function createsMemo(): void {
  createMemo(() => 1);
}

export const arrowCreates = (): void => {
  createMemo(() => 2);
};

export function onlyFromHandler(): void {
  createMemo(() => 3);
}
