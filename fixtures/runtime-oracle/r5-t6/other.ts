export const x = { run() {} };
const k = String(Math.random());
(x as Record<string, unknown>)[k] = 1;
