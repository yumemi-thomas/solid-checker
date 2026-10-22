// Negative: `require` of a module with a top-level `await` throws
// ERR_REQUIRE_ASYNC_MODULE, so nothing after the load runs.
import { createRoot } from "solid-js";
import A = require("./tla-provider");

createRoot(() => {
  A.helper();
});
