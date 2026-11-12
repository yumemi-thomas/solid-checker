import { render } from "@solidjs/web";
import * as ns from "./lib";

// B2 (the review of #221). The namespace of `./lib` is enumerated with no name (`Object.values(ns)`): a timer calls every export, `Comp` included, with no owner, so its
// `onCleanup` warns NO_OWNER_CLEANUP (Chrome rc.21). The census (ADR 0347) holds that entry; no tag accounts for it. `Inner` is no export: the namespace does not reach it.
render(() => <ns.Plain />, document.body);
setTimeout(() => {
  for (const component of Object.values(ns)) {
    (component as () => unknown)();
  }
}, 0);
