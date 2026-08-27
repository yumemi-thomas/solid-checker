import { initial, refresh } from "./use";

export const doubled = initial * 2;

function report(value: number): number {
  return value + doubled;
}

export async function run(): Promise<number> {
  const next = await refresh();
  return report(next);
}
