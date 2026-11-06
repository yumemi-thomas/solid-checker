window.setTimeout = ((f: () => void) => { f(); return 0; }) as unknown as typeof setTimeout;
const proto = Promise.prototype;
proto.then = function (this: unknown, f: () => void) { f(); return this; } as unknown as typeof proto.then;
Object.defineProperty(globalThis, "requestAnimationFrame", { value: (f: () => void) => { f(); } });
Object.assign(window, { unrelated: 1 });
const g = globalThis as unknown as Record<symbol, unknown>;
g[Symbol.for("slot")] = 1;
