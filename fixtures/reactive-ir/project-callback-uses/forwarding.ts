import { inline, mixedSafe, tracked } from "./wrappers";

export function forwardTracked(read: () => number) {
  return tracked(read);
}
export function forwardMixed(read: () => number) {
  return mixedSafe(read);
}
export const forwardInline = (read: () => number) => inline(read);
