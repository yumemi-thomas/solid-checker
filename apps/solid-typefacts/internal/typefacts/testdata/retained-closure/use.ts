import { count, double, loadCount } from "./source";

export const initial = count();

// A local alias to an async function declared in another file. The async fact
// for `loader` resolves to source.ts while the demand sits here, which is what
// makes cross-path async facts reachable.
const loader = loadCount;

export async function refresh(): Promise<number> {
  const raw = await loader();
  return double(raw);
}
