export const unrelated = 42;

export function untouched(value: number): number {
  return value + unrelated;
}
