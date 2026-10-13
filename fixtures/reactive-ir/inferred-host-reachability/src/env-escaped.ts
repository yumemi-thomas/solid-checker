// Type-correct mutation of Vite dev's injected env object invalidates SSR=false.
Object.assign(import.meta.env, { SSR: true });
function stop() { if (import.meta.env.SSR) throw 0; }
stop();
void "escaped env cannot grant continuation authority";
setTimeout(() => { void "escaped env cannot register browser timer"; }, 0);
