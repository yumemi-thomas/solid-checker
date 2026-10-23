// The edit target. Nothing references it, so editing it leaves every other file
// outside the affected set.

export const island = 1;

export function touch(value: number): number {
  return value + island;
}
