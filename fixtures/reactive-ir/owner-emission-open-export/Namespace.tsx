import { render } from "@solidjs/web";
import * as ns from "./lib";

// R2. The program's own timer enumerates the namespace of `./lib` with no name and calls every export with no owner: `Comp`'s `onCleanup` warns NO_OWNER_CLEANUP (Chrome rc.21).
// The namespace census returns nothing for an open program, so B2's test (a closed program's) found no entry for `Comp` and certified it though the enumerating caller is visible.
// `Inner` is no export: the namespace does not reach it.
render(() => <ns.Plain />, document.body);
setTimeout(() => {
  for (const component of Object.values(ns)) {
    (component as () => unknown)();
  }
}, 0);
