import { createSignal, createMemo } from "solid-js";

export function collection<T>(initial: T): { rows: () => T; selected: () => T; plain: () => number } {
  const [rows] = createSignal<T>(initial as Exclude<T, Function>);
  return { rows, selected: () => rows(), plain: () => 7 };
}

export function several(flag: boolean) {
  const [rows] = createSignal(1);
  if (flag) return { rows };
  return { rows: () => 0 };
}

export function computed(key: string) {
  const [rows] = createSignal(1);
  return { [key]: rows };
}

export function spread() {
  const [rows] = createSignal(1);
  return { ...{ rows }, rows: () => 0 };
}

export function memo(): { value: () => number } {
  const value = createMemo(() => 2);
  return { value };
}
