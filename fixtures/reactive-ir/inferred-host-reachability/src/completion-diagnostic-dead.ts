import { startClosed } from "reactive-package";
function stop() { if (!import.meta.env.SSR) throw 0; }
stop();
setTimeout(() => startClosed(), 0); // certain exit prevents owner violation authority
