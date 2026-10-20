import { sharedConfig } from "solid-js/internal";
if (sharedConfig.context) console.log("server");
sharedConfig.load = () => 1;
