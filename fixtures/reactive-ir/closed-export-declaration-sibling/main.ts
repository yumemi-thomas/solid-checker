import { helper } from "./helpers";
import * as bridge from "./bridge.js";
export type Signature = typeof helper;

export function enter(item: { label(): string }) {
  helper({ label() { return "fixed"; } });
  let result = "";
  for (const f of Object.values(bridge)) result = f(item);
  return result;
}
