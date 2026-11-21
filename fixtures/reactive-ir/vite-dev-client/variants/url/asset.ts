// Review F2: `new URL(…, import.meta.url)` names a module the page may load.
export const worker = new URL("./main.tsx", import.meta.url);
