import { Exported } from "./App";

// The consumer the package's author does not see (oracle.json `external`): a call with no owner.
setTimeout(() => {
  Exported();
}, 0);
