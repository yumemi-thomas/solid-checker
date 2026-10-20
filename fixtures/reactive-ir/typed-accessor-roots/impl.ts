import { createSignal, type Accessor } from "solid-js";
import type { DeclaredApi } from "./declared";

// A member an interface in a `.d.ts` declares, assigned a plain function.
export const external: DeclaredApi = { orientation: () => "h" };

// Factories: one returns a plain function, one a signal accessor.
export function makePlain(): Accessor<string> {
  return () => "h";
}
export function makeCount(): Accessor<number> {
  const [count] = createSignal(0);
  return count;
}

// A project helper that calls its typed parameter.
export function parameterRead(get: Accessor<string>) {
  return get();
}
