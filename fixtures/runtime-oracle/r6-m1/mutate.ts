import * as ns from "./barrel";
const k = "re" + "ad";
export function patch(src: Record<string, unknown>) {
  Object.assign(ns.api, src);
  (ns.api as Record<string, unknown>)[k] = ns.reader.read;
}
