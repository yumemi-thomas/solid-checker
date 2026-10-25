"use server";
import { logged } from "./mine";

// Positive: not functions — registration throws when the server boots.
export const LIMIT = 10;
export const NAME = `users`;
export const LIST = [1, 2];
export const CONFIG = { version: 1 };

// Positive: re-exports are a compile error under the directive.
export { helper } from "./mine";
export * from "./mine";

// Negative: functions, and a wrapped export (registered by value).
export async function ok() {
  return 1;
}
export const arrow = async () => 2;
export const composed = logged(async () => 3);
