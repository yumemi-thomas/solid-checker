import { DEV as Development } from "solid-js";
const copy = Development;
if (copy) copy.hooks.onOwner = () => {};
