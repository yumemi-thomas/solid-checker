import { notify } from "./notify.js";

// The control for `asset-query-import`: the same shipped file, imported only as
// a module. The callback runs on this stack, so the case proves what the
// query-suffixed sibling deliberately leaves open.
export function runModule(callback) {
  notify(callback);
}
