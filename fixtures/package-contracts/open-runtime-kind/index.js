export const hiddenCallable = Object.assign(callback => callback(), Object.create(null));
export const hiddenObject = Object.assign(Object.create(null), { ok: true });
export function noop() {}
