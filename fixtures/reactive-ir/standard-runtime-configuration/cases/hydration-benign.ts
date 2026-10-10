import { sharedConfig as host } from "solid-js/internal";
if (host) console.log("host");
if (host.context) console.log("server");
export const absent = !host.context;
export const tier = host.context ? "server" : "client";
host.context && console.log("server");
