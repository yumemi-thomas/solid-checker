// Positive: the same load of a module with no top-level `await`.
import { createRoot } from "solid-js";
import A = require("./tla-control-provider");

createRoot(() => {
  A.helper();
});
