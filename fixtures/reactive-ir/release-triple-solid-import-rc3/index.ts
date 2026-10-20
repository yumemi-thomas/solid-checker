// The control for `release-triple-solid-free-rc3`: the same shape, with one
// import from `solid-js`. The project uses the runtime, so every open gap of
// the rc.3 triple is its own.
import { untrack } from "solid-js";
import { label } from "./label";

export const title = untrack(() => label("kobalte"));
