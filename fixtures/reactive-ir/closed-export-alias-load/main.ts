import { helper } from "./helpers";
export type Signature = typeof helper;

export async function enter(item: { label(): string }) {
  helper({ label() { return "fixed"; } });
  const module = await import("@entry");
  let result = "";
  for (const f of Object.values(module)) result = f(item);
  return result;
}
