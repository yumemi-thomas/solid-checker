import { sharedConfig as host } from "solid-js/internal";
export const active = host.hydrating;
export const complete = (host as typeof host).done;
