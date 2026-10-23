import type { Accessor } from "solid-js";

export const count: Accessor<number> = () => 1;

export function double(value: number): number {
  return value * 2;
}

export async function loadCount(): Promise<number> {
  const value = count();
  return double(value);
}
