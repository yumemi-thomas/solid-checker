import { createSignal, onCleanup } from "solid-js";
const [count] = createSignal(0);
export const timer = { useTicker(fn: () => void) { onCleanup(fn); } };
export const reader = { readCount: () => count() };
export function throughReader() { return reader.readCount(); }
export function maybeReader(flag: boolean) { return flag ? reader.readCount() : 0; }
export async function laterReader() { await Promise.resolve(); return reader.readCount(); }
export const quoted = { "quotedRead": () => count() };
export const frozen = Object.freeze({ frozenRead: function () { return count(); } });
function shorthandRead() { return count(); }
export const shorthand = { shorthandRead };
export default { defaultRead() { return count(); } };
