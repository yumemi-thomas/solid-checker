function stop() { if (import.meta.env.SSR) throw 0; }
stop();
void "unchanged env supplies client continuation";
setTimeout(() => { void "unchanged env supplies feasible timer"; }, 0);
