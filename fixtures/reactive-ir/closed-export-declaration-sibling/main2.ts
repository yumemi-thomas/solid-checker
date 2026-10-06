import { helper2 } from "./helpers2";
import * as bridge2 from "./bridge2";
export type Signature2 = typeof helper2;

// `./bridge2` names a directory: its `main` selects the runtime module, which
// the compiler, reading `types`, never sees.
export function enter2(item: { label(): string }) {
  helper2({ label() { return "fixed"; } });
  let result = "";
  for (const f of Object.values(bridge2)) result = f(item);
  return result;
}
