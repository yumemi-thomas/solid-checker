import { DEV } from "solid-js";
if (DEV) console.log("development");
export const mode = DEV ? "development" : "production";
DEV && console.log("development");
