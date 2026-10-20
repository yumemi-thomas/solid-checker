import { onCleanup } from "solid-js";

export function pure() {
  return 1;
}
export function forbidden() {
  onCleanup(() => {});
}
export const constant = () => {
  onCleanup(() => {});
};
export function supplied(callback: () => void) {
  callback();
}
export function pureWithDefault(value = forbidden()) {
  return value;
}
