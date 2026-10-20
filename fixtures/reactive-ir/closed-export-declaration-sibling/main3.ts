import { helper3 } from "./node_modules/linked/helpers";
import * as linked from "linked";
export type Signature3 = typeof helper3;

// `linked` is installed under node_modules, but its source is part of this
// program, so its runtime `main` may expose `helper3`.
export function enter3(item: { label(): string }) {
  helper3({ label() { return "fixed"; } });
  let result = "";
  for (const f of Object.values(linked)) result = f(item);
  return result;
}
