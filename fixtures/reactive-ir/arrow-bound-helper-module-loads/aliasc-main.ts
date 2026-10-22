// Positive: the alias statement is written before the call.
import "./aliasc-provider";
import { createRoot } from "solid-js";
import A = require("./aliasc-provider");

createRoot(() => {
  A.helper();
});
