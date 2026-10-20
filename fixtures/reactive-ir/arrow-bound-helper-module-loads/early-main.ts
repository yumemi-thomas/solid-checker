// Negative: `import A = require(...)` runs where it is written, after this
// createRoot, so `A.helper` is read before the provider ran.
import { createRoot } from "solid-js";

createRoot(() => {
  A.helper();
});
import A = require("./early-provider");
