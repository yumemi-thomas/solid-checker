// Negative: the provider is loaded through a bridge, still before the alias.
import "./aliast-bridge";
import { createRoot } from "solid-js";

createRoot(() => {
  A.helper();
});
import A = require("./aliast-provider");
