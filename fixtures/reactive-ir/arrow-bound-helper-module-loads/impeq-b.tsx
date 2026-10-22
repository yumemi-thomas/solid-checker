// Negative: impeq-a.ts loads this module through `import b = require(...)`
// before it declares `helper`, so a call here at module init may run with
// `helper` in its temporal dead zone.
import { createRoot } from "solid-js";
import { helper } from "./impeq-a";

createRoot(() => {
  helper();
});
