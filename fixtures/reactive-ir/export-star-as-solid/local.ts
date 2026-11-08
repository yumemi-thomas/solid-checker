export function createSignal(value: number): [() => number] {
  return [() => value];
}
