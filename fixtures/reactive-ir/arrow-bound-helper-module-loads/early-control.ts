// Positive: the same load written before the createRoot is initialized.
import { createRoot } from "solid-js";
import A = require("./early-provider2");

createRoot(() => {
  A.helper();
});
