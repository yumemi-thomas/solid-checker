import { createRoot } from "solid-js";
import { native } from "fnstrpkg";

// Silent: `fnstrpkg` names `Function` only as `Function.toString.call(fn)`, which generates no code, so its closure is clear.
createRoot(() => {
  native(() => {});
});
