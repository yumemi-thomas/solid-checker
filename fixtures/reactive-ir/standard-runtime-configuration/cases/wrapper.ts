import * as Solid from "solid-js";
const copy = (Solid as typeof Solid).DEV;
if (copy) copy.hooks.onGraph = () => {};
