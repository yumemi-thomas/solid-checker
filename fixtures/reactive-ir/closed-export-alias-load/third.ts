import * as shim from "fs";
export type Shimmed = typeof shim.shimmed;

// `fs` is mapped onto a project file, so a bare `require("fs")` may load it.
export function viaShim(item: { label(): string }) {
  shim.shimmed({ label() { return "fixed"; } });
  const ns = require("fs") as Record<string, (item: { label(): string }) => string>;
  let result = "";
  for (const f of Object.values(ns)) result = f(item);
  return result;
}
