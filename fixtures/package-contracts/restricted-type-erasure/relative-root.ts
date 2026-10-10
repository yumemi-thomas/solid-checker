import { value } from "./dependency";

export function noop(input: number): number {
  return input + value - 1;
}
