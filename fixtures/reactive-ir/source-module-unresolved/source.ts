import { createSignal } from "solid-js";
// @ts-ignore: generated, and absent here.
import { absentLocale } from "~/absent-runtime";

// `createSignal(fn)` makes a compute, and a value from a module nothing resolves
// may be a function: nothing proves this source synchronous.
const [locale] = createSignal(absentLocale());
export const useLocale = locale;

export function translate(message: () => string): string {
  useLocale();
  return message();
}
