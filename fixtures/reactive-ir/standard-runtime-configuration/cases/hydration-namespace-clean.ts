import * as Solid from "solid-js/internal";
export const active = Solid.sharedConfig.hydrating;
if ((Solid.sharedConfig as typeof Solid.sharedConfig).context) console.log("server");
