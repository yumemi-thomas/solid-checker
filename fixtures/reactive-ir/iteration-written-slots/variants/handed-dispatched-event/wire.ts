const ev = new Event("x");
window.dispatchEvent(ev);
(ev.target as any).Function("0")();
export {};
