// Negative: the static import loads the provider first, but the alias `A`
// is in its own temporal dead zone until its statement runs: `A.helper()`
// throws ReferenceError, whatever the provider's initialization.
import "./alias-provider";
import { createRoot } from "solid-js";

createRoot(() => {
  A.helper();
});
import A = require("./alias-provider");
